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
        std::env::temp_dir().join(format!(
            "rpl-drafts-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
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
    fn draft_discovery_ignores_unrelated_files() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("artwork.rpl"), b"not a draft").unwrap();
        fs::write(dir.join("draft-not-a-project.txt"), b"ignore").unwrap();
        assert!(list_in(&dir).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
}
