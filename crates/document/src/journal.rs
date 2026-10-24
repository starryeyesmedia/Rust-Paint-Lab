//! Bounded append-only recovery journal of complete validated document states.
//!
//! This is deliberately a snapshot journal, not an operation WAL: each record
//! replaces the prior state during recovery, so retrying a record cannot apply
//! an edit twice. The existing native recovery snapshot and its `.bak` remain
//! the fallback and compaction safety net.
use crate::{PaintError, State, storage};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::path::Path;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

const MAGIC: &[u8; 8] = b"RPLJRN01";
const HEADER: usize = 24;
const DIGEST: usize = 32;
const MAX_JOURNAL: usize = 2 * storage::MAX_FILE + 2 * (HEADER + DIGEST);

/// Deterministic storage failure points for recovery tests and fault injection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JournalFault {
    None,
    AfterTailTruncate,
    AfterPartialAppend,
    BeforeCompactionReplace,
}

#[derive(Debug)]
struct Scan {
    latest: Option<(u64, [u8; DIGEST], Option<State>)>,
    valid_bytes: usize,
    damaged_tail: bool,
}

fn digest(bytes: &[u8]) -> [u8; DIGEST] {
    Sha256::digest(bytes).into()
}

fn write_record(out: &mut impl Write, seq: u64, payload: &[u8]) -> std::io::Result<()> {
    let header = record_header(seq, payload.len());
    let mut hasher = Sha256::new();
    hasher.update(header);
    hasher.update(payload);
    out.write_all(&header)?;
    out.write_all(payload)?;
    out.write_all(&hasher.finalize())
}

fn record_header(seq: u64, payload_len: usize) -> [u8; HEADER] {
    let mut header = [0u8; HEADER];
    header[..8].copy_from_slice(MAGIC);
    header[8..16].copy_from_slice(&seq.to_le_bytes());
    header[16..24].copy_from_slice(&(payload_len as u64).to_le_bytes());
    header
}

fn injected_failure(label: &'static str) -> PaintError {
    PaintError::Io(std::io::Error::other(format!(
        "injected journal failure: {label}"
    )))
}

fn scan(bytes: &[u8], retain_state: bool) -> Scan {
    let mut offset = 0;
    let mut previous_seq = 0;
    let mut latest = None;
    let mut damaged_tail = false;
    while offset < bytes.len() {
        let start = offset;
        if bytes.len() - offset < HEADER || &bytes[offset..offset + 8] != MAGIC {
            damaged_tail = true;
            break;
        }
        let seq = u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap());
        let len_u64 = u64::from_le_bytes(bytes[offset + 16..offset + 24].try_into().unwrap());
        let Ok(len) = usize::try_from(len_u64) else {
            damaged_tail = true;
            break;
        };
        let Some(end) = offset.checked_add(HEADER).and_then(|v| v.checked_add(len)) else {
            damaged_tail = true;
            break;
        };
        let Some(record_end) = end.checked_add(DIGEST) else {
            damaged_tail = true;
            break;
        };
        if seq <= previous_seq || len > storage::MAX_FILE || record_end > bytes.len() {
            damaged_tail = true;
            break;
        }
        let expected = digest(&bytes[start..end]);
        if expected != bytes[end..record_end] {
            damaged_tail = true;
            break;
        }
        let payload = &bytes[offset + HEADER..end];
        let Ok(state) = storage::decode(payload) else {
            damaged_tail = true;
            break;
        };
        latest = Some((seq, digest(payload), retain_state.then_some(state)));
        previous_seq = seq;
        offset = record_end;
    }
    Scan {
        latest,
        valid_bytes: offset,
        damaged_tail,
    }
}

/// Serialized file limit is at most two native project snapshots. Appending a
/// later checkpoint compacts to that checkpoint if the cap would be exceeded.
pub struct Journal {
    path: PathBuf,
    max_bytes: usize,
}

impl Journal {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            max_bytes: MAX_JOURNAL,
        }
    }

    fn lock(&self) -> Result<File, PaintError> {
        let lock_path = self.path.with_extension("journal.lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        lock.try_lock().map_err(|_| PaintError::Busy)?;
        Ok(lock)
    }

    pub fn append(&mut self, state: &State) -> Result<(), PaintError> {
        self.append_fault(state, JournalFault::None)
    }

    /// Append with a deterministic failure point. Production callers should
    /// use `append`; fault modes exist so tests can exercise torn-write recovery.
    fn append_fault(&mut self, state: &State, fault: JournalFault) -> Result<(), PaintError> {
        let _lock = self.lock()?;
        let mut bytes = Vec::new();
        if self.path.exists() {
            File::open(&self.path)?
                .take(self.max_bytes as u64 + 1)
                .read_to_end(&mut bytes)?;
        }
        if bytes.len() > self.max_bytes {
            return Err(PaintError::Limit);
        }
        let prior = scan(&bytes, false);
        let payload = storage::encode(state)?;
        if prior
            .latest
            .as_ref()
            .is_some_and(|(_, last_hash, _)| last_hash == &digest(&payload))
        {
            return Ok(());
        }
        let seq = match prior.latest.as_ref() {
            Some((last, _, _)) => last.checked_add(1).ok_or(PaintError::Limit)?,
            None => 1,
        };
        let record_len = HEADER.saturating_add(payload.len()).saturating_add(DIGEST);
        let total = prior.valid_bytes.saturating_add(record_len);
        if total <= self.max_bytes {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(&self.path)?;
            file.set_len(prior.valid_bytes as u64)?;
            if fault == JournalFault::AfterTailTruncate {
                file.sync_all()?;
                return Err(injected_failure("after tail truncation"));
            }
            file.seek(SeekFrom::End(0))?;
            if fault == JournalFault::AfterPartialAppend {
                let header = record_header(seq, payload.len());
                file.write_all(&header)?;
                let partial = payload.len().div_ceil(2);
                file.write_all(&payload[..partial])?;
                file.sync_all()?;
                return Err(injected_failure("after partial append"));
            }
            write_record(&mut file, seq, &payload)?;
            file.sync_all()?;
        } else {
            // A fully synced replacement is atomic; the previous verified
            // recovery snapshot remains independently available as fallback.
            let tmp = storage::temporary(&self.path);
            let result = (|| {
                let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
                write_record(&mut file, seq, &payload)?;
                file.sync_all()?;
                if fault == JournalFault::BeforeCompactionReplace {
                    return Err(std::io::Error::other(
                        "injected journal failure before compaction replace",
                    ));
                }
                fs::rename(&tmp, &self.path)?;
                #[cfg(unix)]
                File::open(
                    self.path
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or_else(|| Path::new(".")),
                )?
                .sync_all()?;
                Ok::<_, std::io::Error>(())
            })();
            if tmp.exists() {
                let _ = fs::remove_file(tmp);
            }
            result?;
        }
        Ok(())
    }

    /// Returns the latest complete valid checkpoint. A truncated or corrupt
    /// tail falls back to the preceding complete record and is reported.
    pub fn restore(&self) -> Result<(State, bool), PaintError> {
        let mut bytes = Vec::new();
        File::open(&self.path)?
            .take(self.max_bytes as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > self.max_bytes {
            return Err(PaintError::Limit);
        }
        let scan = scan(&bytes, true);
        scan.latest
            .and_then(|(_, _, state)| state.map(|s| (s, scan.damaged_tail)))
            .ok_or(PaintError::Invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "rpl-journal-{}-{}.journal",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    fn revise(engine: &mut Engine, name: &str) {
        engine
            .edit(|state| {
                state.layers[0].name = name.into();
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn append_restores_latest_and_duplicate_checkpoints_are_idempotent() {
        let path = path();
        let mut journal = Journal::new(path.clone());
        let mut engine = Engine::new(32, 32, 96).unwrap();
        revise(&mut engine, "first");
        journal.append(engine.state()).unwrap();
        let first = fs::metadata(&path).unwrap().len();
        journal.append(engine.state()).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len(), first);
        revise(&mut engine, "second");
        journal.append(engine.state()).unwrap();
        let (restored, fallback) = journal.restore().unwrap();
        assert!(!fallback);
        assert_eq!(
            storage::encode(&restored).unwrap(),
            storage::encode(engine.state()).unwrap()
        );
        fs::remove_file(&path).unwrap();
        let _ = fs::remove_file(path.with_extension("journal.lock"));
    }

    #[test]
    fn truncated_or_corrupt_tail_recovers_previous_complete_state() {
        let path = path();
        let mut journal = Journal::new(path.clone());
        let mut engine = Engine::new(32, 32, 96).unwrap();
        revise(&mut engine, "first");
        let first = engine.state().clone();
        journal.append(&first).unwrap();
        revise(&mut engine, "second");
        journal.append(engine.state()).unwrap();
        let len = fs::metadata(&path).unwrap().len();
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(len - 10)
            .unwrap();
        let (restored, fallback) = journal.restore().unwrap();
        assert!(fallback);
        assert_eq!(
            storage::encode(&restored).unwrap(),
            storage::encode(&first).unwrap()
        );
        // Reappend the later state, then corrupt its checksum. Scanning must
        // reject that entire record and continue to serve the prior snapshot.
        journal.append(engine.state()).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 0x80;
        fs::write(&path, bytes).unwrap();
        let (restored, fallback) = journal.restore().unwrap();
        assert!(fallback);
        assert_eq!(
            storage::encode(&restored).unwrap(),
            storage::encode(&first).unwrap()
        );
        fs::remove_file(&path).unwrap();
        let _ = fs::remove_file(path.with_extension("journal.lock"));
    }

    #[test]
    fn injected_partial_append_and_tail_truncation_preserve_last_checkpoint() {
        let path = path();
        let mut journal = Journal::new(path.clone());
        let mut engine = Engine::new(32, 32, 96).unwrap();
        revise(&mut engine, "first");
        let first = storage::encode(engine.state()).unwrap();
        journal.append(engine.state()).unwrap();

        revise(&mut engine, "second");
        let second = storage::encode(engine.state()).unwrap();
        assert!(
            journal
                .append_fault(engine.state(), JournalFault::AfterPartialAppend)
                .is_err()
        );
        let (restored, fallback) = journal.restore().unwrap();
        assert!(fallback);
        assert_eq!(storage::encode(&restored).unwrap(), first);
        journal.append(engine.state()).unwrap();
        assert_eq!(
            storage::encode(&journal.restore().unwrap().0).unwrap(),
            second
        );

        // Inject failure after removing a torn tail but before appending the
        // replacement record. The preceding complete checkpoint remains valid.
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(b"torn header");
        fs::write(&path, bytes).unwrap();
        revise(&mut engine, "third");
        assert!(
            journal
                .append_fault(engine.state(), JournalFault::AfterTailTruncate)
                .is_err()
        );
        let (restored, fallback) = journal.restore().unwrap();
        assert!(!fallback);
        assert_eq!(storage::encode(&restored).unwrap(), second);
        fs::remove_file(&path).unwrap();
        let _ = fs::remove_file(path.with_extension("journal.lock"));
    }

    #[test]
    fn journal_compacts_to_latest_state_at_its_bound() {
        let path = path();
        let mut engine = Engine::new(32, 32, 96).unwrap();
        revise(&mut engine, "one");
        let record_size = HEADER + storage::encode(engine.state()).unwrap().len() + DIGEST;
        let mut journal = Journal {
            path: path.clone(),
            max_bytes: 2 * record_size,
        };
        journal.append(engine.state()).unwrap();
        revise(&mut engine, "two");
        journal.append(engine.state()).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len() as usize, 2 * record_size);
        revise(&mut engine, "tre");
        journal.append(engine.state()).unwrap();
        assert!(fs::metadata(&path).unwrap().len() as usize <= 2 * record_size);
        let (restored, fallback) = journal.restore().unwrap();
        assert!(!fallback);
        assert_eq!(
            storage::encode(&restored).unwrap(),
            storage::encode(engine.state()).unwrap()
        );
        fs::remove_file(&path).unwrap();
        let _ = fs::remove_file(path.with_extension("journal.lock"));
    }

    #[test]
    fn failed_compaction_keeps_old_journal_and_removes_temporary_file() {
        let path = path();
        let mut engine = Engine::new(32, 32, 96).unwrap();
        revise(&mut engine, "one");
        let record_size = HEADER + storage::encode(engine.state()).unwrap().len() + DIGEST;
        let mut journal = Journal {
            path: path.clone(),
            max_bytes: 2 * record_size,
        };
        journal.append(engine.state()).unwrap();
        revise(&mut engine, "two");
        let second = storage::encode(engine.state()).unwrap();
        journal.append(engine.state()).unwrap();
        let before = fs::read(&path).unwrap();
        revise(&mut engine, "tre");
        assert!(
            journal
                .append_fault(engine.state(), JournalFault::BeforeCompactionReplace)
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(
            storage::encode(&journal.restore().unwrap().0).unwrap(),
            second
        );
        let file_name = path.file_name().unwrap().to_string_lossy();
        let prefix = format!(".{file_name}.tmp-");
        assert!(
            !fs::read_dir(path.parent().unwrap())
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
        );
        fs::remove_file(&path).unwrap();
        let _ = fs::remove_file(path.with_extension("journal.lock"));
    }

    #[test]
    fn concurrent_journal_writer_is_rejected() {
        let path = path();
        let lock_path = path.with_extension("journal.lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();
        lock.try_lock().unwrap();
        let mut journal = Journal::new(path.clone());
        let engine = Engine::new(32, 32, 96).unwrap();
        assert!(matches!(
            journal.append(engine.state()),
            Err(PaintError::Busy)
        ));
        drop(lock);
        let _ = fs::remove_file(lock_path);
    }
}
