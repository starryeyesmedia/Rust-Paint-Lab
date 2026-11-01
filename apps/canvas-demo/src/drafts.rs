//! Local persistent project drafts for unnamed documents.

use paint_document::{PaintError, State, storage::Store};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// Number of drafts shown in each Home-screen browsing page.
pub const DISPLAY_BATCH_SIZE: usize = 8;
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
    drafts
}

/// Return a bounded Home-screen slice while keeping every discovered draft
/// reachable through previous/next page controls.
pub fn display_range(total: usize, page: usize) -> std::ops::Range<usize> {
    let start = page.saturating_mul(DISPLAY_BATCH_SIZE).min(total);
    start..start.saturating_add(DISPLAY_BATCH_SIZE).min(total)
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

/// Give close-time draft failures enough context to recover without hiding the
/// underlying filesystem or document error behind a generic close dialog.
pub fn close_save_error(directory: &Path, error: &PaintError) -> String {
    match error {
        PaintError::Io(error) => format!(
            "Could not write an untitled project draft in {}: {error}",
            directory.display()
        ),
        error => format!(
            "Could not save an untitled project draft in {}: {error}",
            directory.display()
        ),
    }
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
    fn drafts_are_native_projects_discoverable_after_restart() {
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

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn saving_and_reopening_with_8_9_32_and_100_existing_drafts_preserves_all_files_and_backups() {
        fn state_with_pixel(pixel: [u8; 4]) -> State {
            let mut state = State::new(8, 8, 96).unwrap();
            state.layers[0].tiles.insert(
                (0, 0),
                std::sync::Arc::new({
                    let mut tile = vec![0; 64 * 64 * 4];
                    tile[0..4].copy_from_slice(&pixel);
                    tile
                }),
            );
            state
        }

        for existing_count in [8, 9, 32, 100] {
            let dir = temp_dir();
            let initial = state_with_pixel([17, 31, 47, 255]);
            let (mut first_store, first_path) =
                create_in(&initial, "existing with backup", &dir).unwrap();
            let updated_first = state_with_pixel([29, 43, 61, 255]);
            first_store.save(&updated_first).unwrap();
            let first_backup_path = first_path.with_extension("rpl.bak");
            let first_backup = fs::read(&first_backup_path).unwrap();

            let mut existing_paths = vec![first_path];
            for index in 1..existing_count {
                let (_, path) = create_in(&initial, &format!("existing {index}"), &dir).unwrap();
                existing_paths.push(path);
            }
            let original_files = existing_paths
                .iter()
                .map(|path| (path.clone(), fs::read(path).unwrap()))
                .collect::<Vec<_>>();

            let new_artwork = state_with_pixel([211, 157, 83, 255]);
            let (_new_store, new_path) = create_in(&new_artwork, "new after many", &dir).unwrap();

            let discovered = list_in(&dir);
            assert_eq!(discovered.len(), existing_count + 1);
            assert!(discovered.contains(&new_path));
            for (path, original_bytes) in original_files {
                assert_eq!(fs::read(&path).unwrap(), original_bytes, "changed {path:?}");
                let (_store, reopened) = Store::open(path).unwrap();
                assert_eq!(reopened.width, 8);
            }
            assert_eq!(fs::read(&first_backup_path).unwrap(), first_backup);
            let (_store, reopened_new) = Store::open(new_path).unwrap();
            assert_eq!(
                &reopened_new.layers[0].tiles[&(0, 0)][0..4],
                &[211, 157, 83, 255]
            );
            fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn draft_browser_exposes_all_entries_in_bounded_pages() {
        assert_eq!(display_range(0, 0), 0..0);
        assert_eq!(display_range(8, 0), 0..8);
        assert_eq!(display_range(9, 1), 8..9);
        assert_eq!(display_range(100, 0), 0..8);
        assert_eq!(display_range(100, 11), 88..96);
        assert_eq!(display_range(100, 12), 96..100);
        assert_eq!(display_range(100, usize::MAX), 100..100);
    }

    #[test]
    fn untitled_draft_round_trip_preserves_raster_mask_and_vector_artwork() {
        let dir = temp_dir();
        let mut state = State::new(64, 48, 300).unwrap();
        state.layers[0].tiles.insert(
            (0, 0),
            std::sync::Arc::new({
                let mut tile = vec![0; 64 * 64 * 4];
                tile[0..4].copy_from_slice(&[12, 34, 56, 255]);
                tile
            }),
        );
        state.layers[0].mask = Some(paint_document::LayerMask {
            enabled: true,
            inverted: false,
            tiles: std::collections::BTreeMap::from([(
                (0, 0),
                std::sync::Arc::new(vec![127; 64 * 64]),
            )]),
        });
        state.layers[0].vector_paths.push(
            paint_document::vector::VectorPath::from_polyline(
                1,
                &[[2.0, 3.0], [21.0, 17.0]],
                [190, 32, 210, 255],
                3.5,
            )
            .unwrap(),
        );
        state.next_vector_id = 2;
        state.validate().unwrap();

        let (store, path) = create_in(&state, "layer mask vector draft", &dir).unwrap();
        drop(store);
        let (_store, reopened) = Store::open(path.clone()).unwrap();
        assert_eq!(reopened.width, 64);
        assert_eq!(reopened.layers[0].tiles[&(0, 0)][0..4], [12, 34, 56, 255]);
        let mask = reopened.layers[0].mask.as_ref().unwrap();
        assert!(mask.enabled);
        assert_eq!(mask.tiles[&(0, 0)][..3], [127, 127, 127]);
        assert_eq!(reopened.layers[0].vector_paths.len(), 1);
        assert_eq!(reopened.layers[0].vector_paths[0].segments.len(), 1);
        assert_eq!(reopened.layers[0].vector_paths[0].width, 3.5);
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

    #[test]
    fn close_save_errors_report_document_limits_and_io_failures() {
        let dir = Path::new("/local/state/bitchpixel/drafts");
        let limit = close_save_error(dir, &PaintError::Limit);
        assert!(limit.contains("exceeds the alpha's document"));
        assert!(limit.contains(&dir.display().to_string()));

        let io = PaintError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "access denied",
        ));
        let message = close_save_error(dir, &io);
        assert!(message.contains("Could not write an untitled project draft"));
        assert!(message.contains("access denied"));
    }

    #[test]
    fn close_save_errors_keep_permission_and_disk_full_diagnostics() {
        let dir = Path::new("/local/state/bitchpixel/drafts");
        for (kind, detail) in [
            (std::io::ErrorKind::PermissionDenied, "permission denied"),
            (std::io::ErrorKind::StorageFull, "disk full"),
        ] {
            let error = PaintError::Io(std::io::Error::new(kind, detail));
            let message = close_save_error(dir, &error);
            assert!(message.contains(&dir.display().to_string()));
            assert!(message.contains(detail));
            assert!(message.contains("Could not write an untitled project draft"));
        }
    }
}
