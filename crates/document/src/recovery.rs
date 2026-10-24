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
    let mut files: Vec<_> = items
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "rpl"))
        .collect();
    files.sort();
    files.reverse();
    files.truncate(20);
    files.retain(|path| {
        let Ok(owner) = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("owner"))
        else {
            return false;
        };
        owner.try_lock().is_ok()
    });
    files
}
