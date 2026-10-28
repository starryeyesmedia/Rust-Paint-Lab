//! Small bounded diagnostic sink for the desktop application.
//!
//! Windows GUI-subsystem builds have no console by default. Diagnostics are
//! still mirrored to stderr when the launcher supplied a handle, and persisted
//! to one capped local log file for troubleshooting.

use std::{
    fmt::Write as FmtWrite,
    fs::{self, File, OpenOptions},
    io::Write as IoWrite,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_LOG_BYTES: u64 = 1024 * 1024;
const MAX_MESSAGE_BYTES: usize = 8 * 1024;

static LOG_FILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();

/// Initialize a fresh bounded log for this process. `RPL_LOG_DIR` is used by
/// GUI startup automation; normal installs use the platform's local log area.
pub fn initialize() {
    let directory = std::env::var_os("RPL_LOG_DIR")
        .map(PathBuf::from)
        .or_else(default_log_directory);
    let file = directory.and_then(|directory| {
        fs::create_dir_all(&directory).ok()?;
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(directory.join("canvas-demo.log"))
            .ok()
    });
    let _ = LOG_FILE.set(Mutex::new(file));
}

fn default_log_directory() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("BitchPixel").join("logs"))
            .or_else(|| Some(std::env::temp_dir().join("BitchPixel").join("logs")));
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join("Library").join("Logs").join("BitchPixel"))
            .or_else(|| Some(std::env::temp_dir().join("BitchPixel").join("logs")));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|path| path.join(".local").join("state"))
            })
            .map(|path| path.join("BitchPixel").join("logs"))
            .or_else(|| Some(std::env::temp_dir().join("BitchPixel").join("logs")));
    }
    #[allow(unreachable_code)]
    Some(std::env::temp_dir().join("BitchPixel").join("logs"))
}

/// Write one diagnostic line. Both output routes are best-effort: logging must
/// never panic or become a startup dependency when a stream or disk is absent.
pub fn write(arguments: std::fmt::Arguments<'_>) {
    let mut message = String::with_capacity(256);
    let _ = message.write_fmt(arguments);
    let message = bounded_message(message);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let line = format!("{timestamp} {message}\n");
    let _ = std::io::stderr().lock().write_all(line.as_bytes());

    if let Some(file) = LOG_FILE.get()
        && let Ok(mut file) = file.lock()
        && let Some(file) = file.as_mut()
        && let Ok(length) = file.metadata().map(|metadata| metadata.len())
        && length < MAX_LOG_BYTES
    {
        let allowed = (MAX_LOG_BYTES - length) as usize;
        let prefix = utf8_prefix_len(&line, allowed);
        let _ = file.write_all(&line.as_bytes()[..prefix]);
        let _ = file.flush();
    }
}

fn bounded_message(mut message: String) -> String {
    if message.len() > MAX_MESSAGE_BYTES {
        let mut boundary = MAX_MESSAGE_BYTES;
        while !message.is_char_boundary(boundary) {
            boundary -= 1;
        }
        message.truncate(boundary);
    }
    message
}

fn utf8_prefix_len(value: &str, limit: usize) -> usize {
    let mut end = value.len().min(limit);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_log_destination_is_local_and_named_for_product() {
        let path = default_log_directory().expect("fallback log directory");
        assert!(path.ends_with("logs"));
        assert!(path.to_string_lossy().contains("BitchPixel"));
    }

    #[test]
    fn log_messages_are_capped_on_utf8_boundaries() {
        let message = bounded_message("🎨λ".repeat(MAX_MESSAGE_BYTES));
        assert!(message.len() <= MAX_MESSAGE_BYTES);
        assert!(message.is_char_boundary(message.len()));
    }

    #[test]
    fn log_truncation_never_splits_a_utf8_codepoint() {
        let value = "BitchPixel λ 🎨";
        for limit in 0..=value.len() {
            let prefix = utf8_prefix_len(value, limit);
            assert!(value.is_char_boundary(prefix));
            assert!(prefix <= limit);
            assert!(value[..prefix].is_char_boundary(prefix));
        }
        assert_eq!(utf8_prefix_len(value, 2), 2);
        assert_eq!(utf8_prefix_len(value, value.len() + 5), value.len());
    }
}
