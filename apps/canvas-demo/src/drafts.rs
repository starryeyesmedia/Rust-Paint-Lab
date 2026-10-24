//! Local, bounded persistent project drafts for unnamed documents.

use paint_document::{PaintError, State, storage::Store};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_DRAFTS: usize = 8;
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub fn directory() -> Result<PathBuf, PaintError> {
    let recovery = paint_document::recovery::directory()?;
    Ok(recovery.parent().ok_or(PaintError::Invalid)?.join("drafts"))
}

pub fn list() -> Vec<PathBuf> {
    directory().map(|path| list_in(&path)).unwrap_or_default()
}

fn is_draft(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.starts_with("draft-") && path.extension().is_some_and(|ext| ext == "rpl")
}

pub fn display_name(path: &Path) -> String {
    let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
        return "Untitled draft".into();
    };
    let mut parts = name.splitn(5, '-');
    if parts.next() == Some("draft") {
        let _timestamp = parts.next();
        let _process = parts.next();
        let _id = parts.next();
        let rest = parts.next().unwrap_or("Untitled");
        return format!("Untitled — {}", rest.replace('-', " "));
    }
    "Untitled draft".into()
}

pub fn list_in(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut drafts = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).ok()?;
            (metadata.is_file() && is_draft(&path)).then_some(path)
        })
        .collect::<Vec<_>>();
    drafts.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    drafts.truncate(MAX_DRAFTS);
    drafts
}

/// Remove one explicitly selected draft and its storage backup. The candidate
/// must be a direct, regular child of the draft directory; symlinks and paths
/// outside that directory are never followed or deleted.
pub fn remove_in(directory: &Path, candidate: &Path) -> Result<(), PaintError> {
    if candidate.parent() != Some(directory) || !is_draft(candidate) {
        return Err(PaintError::Invalid);
    }
    let metadata = fs::symlink_metadata(candidate)?;
    if !metadata.file_type().is_file() {
        return Err(PaintError::Invalid);
    }
    fs::remove_file(candidate)?;
    let backup = candidate.with_extension("rpl.bak");
    match fs::symlink_metadata(&backup) {
        Ok(metadata) if metadata.file_type().is_file() => fs::remove_file(backup)?,
        Ok(_) => return Err(PaintError::Invalid),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

pub fn remove(candidate: &Path) -> Result<(), PaintError> {
    remove_in(&directory()?, candidate)
}

pub fn create(state: &State, display: &str) -> Result<(Store, PathBuf), PaintError> {
    create_in(state, display, &directory()?)
}

pub fn create_in(
    state: &State,
    display: &str,
    directory: &Path,
) -> Result<(Store, PathBuf), PaintError> {
    fs::create_dir_all(directory)?;
    if list_in(directory).len() >= MAX_DRAFTS {
        return Err(PaintError::Limit);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| PaintError::Invalid)?
        .as_nanos();
    for _ in 0..16 {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let name = crate::document_setup::sanitized_document_stem(display);
        let path = directory.join(format!(
            "draft-{now}-{}-{id}-{name}.rpl",
            std::process::id()
        ));
        let mut store = Store::new(path.clone());
        match store.save(state) {
            Ok(()) => return Ok((store, path)),
            Err(PaintError::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(PaintError::Busy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);
        let temp_root = std::env::temp_dir();
        loop {
            let nonce = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after the Unix epoch")
                .as_nanos();
            let path = temp_root.join(format!("rpl-drafts-{}-{nanos}-{nonce}", std::process::id()));
            // Atomic directory creation makes the test path unique even when
            // parallel tests observe the same clock tick on coarse filesystems.
            match fs::create_dir(&path) {
                Ok(()) => return path,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("could not create test draft directory {path:?}: {error}"),
            }
        }
    }

    #[test]
    fn drafts_are_native_projects_discoverable_after_restart_and_bounded() {
        let dir = temp_dir();
        let state = State::new(64, 48, 300).unwrap();
        let (store, path) = create_in(&state, "first sketch", &dir).unwrap();
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("draft-")
        );
        assert_eq!(display_name(&path), "Untitled — first sketch");
        drop(store);
        assert_eq!(list_in(&dir), vec![path.clone()]);
        let (_, reopened) = Store::open(path.clone()).unwrap();
        assert_eq!(
            [reopened.width, reopened.height, reopened.dpi],
            [64, 48, 300]
        );

        for _ in 1..MAX_DRAFTS {
            create_in(&state, "another", &dir).unwrap();
        }
        assert_eq!(list_in(&dir).len(), MAX_DRAFTS);
        assert!(matches!(
            create_in(&state, "another", &dir),
            Err(PaintError::Limit)
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn parallel_test_directories_are_atomically_unique() {
        let workers = (0..32)
            .map(|_| std::thread::spawn(temp_dir))
            .collect::<Vec<_>>();
        let directories = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(directories.len(), 32);
        assert!(directories.iter().all(|directory| directory.is_dir()));
        for directory in directories {
            fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn draft_discovery_ignores_unrelated_files() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("artwork.rpl"), b"not a draft").unwrap();
        fs::write(dir.join("draft-not-a-project.txt"), b"ignore").unwrap();
        assert!(list_in(&dir).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn explicit_removal_only_deletes_a_draft_and_its_backup() {
        let dir = temp_dir();
        let state = State::new(8, 8, 96).unwrap();
        let (mut store, path) = create_in(&state, "remove me", &dir).unwrap();
        store.save(&state).unwrap();
        let backup = path.with_extension("rpl.bak");
        assert!(backup.exists());
        remove_in(&dir, &path).unwrap();
        assert!(!path.exists());
        assert!(!backup.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn removal_rejects_non_draft_and_outside_paths() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let artwork = dir.join("artwork.rpl");
        fs::write(&artwork, b"keep").unwrap();
        assert!(matches!(
            remove_in(&dir, &artwork),
            Err(PaintError::Invalid)
        ));
        let outside = dir.parent().unwrap().join("draft-1-2-3-outside.rpl");
        fs::write(&outside, b"keep").unwrap();
        assert!(matches!(
            remove_in(&dir, &outside),
            Err(PaintError::Invalid)
        ));
        assert!(artwork.exists());
        assert!(outside.exists());
        fs::remove_file(outside).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}
