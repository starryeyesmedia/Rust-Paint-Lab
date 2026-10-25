//! On-demand previews for validated native recovery checkpoints.
use egui::{Context, TextureHandle, TextureOptions};
use paint_document::{Engine, storage};
use std::path::Path;

const MAX_WIDTH: u32 = 180;
const MAX_HEIGHT: u32 = 120;

/// Load through the native storage validator before sampling a bounded preview.
/// Only the returned small texture is retained by the workspace.
pub fn load(path: &Path, context: &Context) -> Option<TextureHandle> {
    let state = storage::load(path).ok()?;
    let engine = Engine::from_state(state).ok()?;
    let (width, height, pixels) = engine.thumbnail_rgba(MAX_WIDTH, MAX_HEIGHT).ok()?;
    let image =
        egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &pixels);
    Some(context.load_texture(
        format!("recovery-preview:{}", path.display()),
        image,
        TextureOptions::LINEAR,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use paint_document::storage::Store;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn directory() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rpl-recovery-preview-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn preview_reads_only_valid_native_checkpoints_and_preserves_dimensions() {
        let directory = directory();
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("recovery-sample.rpl");
        let engine = Engine::new(300, 150, 144).unwrap();
        Store::new(path.clone()).save(engine.state()).unwrap();
        let texture = load(&path, &Context::default()).unwrap();
        assert_eq!(texture.size(), [180, 90]);

        std::fs::write(&path, b"bad checksum").unwrap();
        assert!(load(&path, &Context::default()).is_none());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
