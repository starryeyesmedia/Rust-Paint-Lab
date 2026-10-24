//! Bounded background snapshot writer. Immutable Arc tiles prevent races with edits.
use crate::{PaintError, State, storage::Store};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};
struct Request {
    state: State,
    reply: Option<SyncSender<Result<(), String>>>,
}
pub struct Recovery {
    sender: Option<SyncSender<Request>>,
    result: Receiver<Result<(), String>>,
    worker: Option<JoinHandle<()>>,
    pub path: PathBuf,
    _owner: File,
}
impl Recovery {
    pub fn start(directory: &Path) -> Result<Self, PaintError> {
        fs::create_dir_all(directory)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| PaintError::Invalid)?
            .as_nanos();
        let path = directory.join(format!("recovery-{stamp}-{}.rpl", std::process::id()));
        let owner = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path.with_extension("owner"))?;
        owner.try_lock().map_err(|_| PaintError::Busy)?;
        let (tx, rx) = mpsc::sync_channel::<Request>(1);
        let (done, result) = mpsc::sync_channel(2);
        let savepath = path.clone();
        let worker = thread::spawn(move || {
            let mut store = Store::new(savepath);
            while let Ok(request) = rx.recv() {
                let result = store.save(&request.state).map_err(|e| e.to_string());
                let _ = done.try_send(result.clone());
                if let Some(reply) = request.reply {
                    let _ = reply.send(result);
                }
            }
        });
        Ok(Self {
            sender: Some(tx),
            result,
            worker: Some(worker),
            path,
            _owner: owner,
        })
    }
    pub fn submit(&self, state: State) -> bool {
        self.sender
            .as_ref()
            .is_some_and(|s| s.try_send(Request { state, reply: None }).is_ok())
    }
    pub fn flush(&self, state: State) -> Result<(), PaintError> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.sender
            .as_ref()
            .ok_or(PaintError::Busy)?
            .send(Request {
                state,
                reply: Some(tx),
            })
            .map_err(|_| PaintError::Busy)?;
        rx.recv()
            .map_err(|_| PaintError::Busy)?
            .map_err(|e| PaintError::Io(std::io::Error::other(e)))
    }
    pub fn poll(&self) -> Vec<Result<(), String>> {
        self.result.try_iter().collect()
    }
}
impl Drop for Recovery {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn directory() -> Result<PathBuf, PaintError> {
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(p).join("RustPaintLab/recovery"));
    }
    if let Some(p) = std::env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(p).join("rust-paint-lab/recovery"));
    }
    let home = std::env::var_os("HOME").ok_or(PaintError::Invalid)?;
    #[cfg(target_os = "macos")]
    let suffix = "Library/Application Support/RustPaintLab/recovery";
    #[cfg(not(target_os = "macos"))]
    let suffix = ".local/state/rust-paint-lab/recovery";
    Ok(PathBuf::from(home).join(suffix))
}
pub fn available(directory: &Path) -> Vec<PathBuf> {
    let Ok(items) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files = Vec::with_capacity(21);
    for item in items.filter_map(Result::ok) {
        let path = item.path();
        if path.extension().is_none_or(|e| e != "rpl") {
            continue;
        }
        let Ok(owner) = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("owner"))
        else {
            continue;
        };
        if owner.try_lock().is_err() {
            continue;
        }
        files.push(path);
        files.sort();
        files.reverse();
        files.truncate(20);
    }
    files
}
/// Explicit user-confirmed cleanup only. Refuse arbitrary paths and active owners.
pub fn remove(directory: &Path, path: &Path) -> Result<(), PaintError> {
    if path.parent() != Some(directory)
        || path.extension().is_none_or(|e| e != "rpl")
        || !path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("recovery-"))
    {
        return Err(PaintError::Invalid);
    }
    let owner_path = path.with_extension("owner");
    let owner = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&owner_path)?;
    owner.try_lock().map_err(|_| PaintError::Busy)?;
    let lock_path = path.with_extension("rpl.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    lock.try_lock().map_err(|_| PaintError::Busy)?;
    fs::remove_file(path)?;
    let backup = path.with_extension("rpl.bak");
    if backup.exists() {
        fs::remove_file(backup)?;
    }
    drop(lock);
    drop(owner);
    let _ = fs::remove_file(lock_path);
    let _ = fs::remove_file(owner_path);
    Ok(())
}
