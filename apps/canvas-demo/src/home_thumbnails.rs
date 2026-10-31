//! Small local previews for Home. These cache files are disposable UI data,
//! bounded independently from the editable project and never enter exports.
use paint_document::Engine;
use std::{
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

pub const WIDTH: u32 = 256;
pub const HEIGHT: u32 = 160;
pub const MAX_CACHE_FILES: usize = 8;
const MAX_PNG_BYTES: u64 = 256 * 1024;

fn file_key(project: &Path) -> Option<String> {
    let metadata = std::fs::metadata(project).ok()?;
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::fs::canonicalize(project).ok()?.hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    modified.hash(&mut hasher);
    Some(format!("rpl-{:016x}.png", hasher.finish()))
}

pub fn cache_path(directory: &Path, project: &Path) -> Option<PathBuf> {
    Some(directory.join(file_key(project)?))
}

pub fn save(directory: &Path, project: &Path, engine: &Engine) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let path = cache_path(directory, project).ok_or("project cache identity unavailable")?;
    let (width, height, pixels) = engine
        .thumbnail_rgba(WIDTH, HEIGHT)
        .map_err(|error| error.to_string())?;
    let image =
        image::RgbaImage::from_raw(width, height, pixels).ok_or("invalid thumbnail buffer")?;
    image.save(&path).map_err(|error| error.to_string())?;
    prune(directory, Some(&path));
    Ok(())
}

pub fn load(
    directory: &Path,
    project: &Path,
    context: &egui::Context,
) -> Option<egui::TextureHandle> {
    let path = cache_path(directory, project)?;
    let metadata = std::fs::metadata(&path).ok()?;
    if metadata.len() > MAX_PNG_BYTES {
        let _ = std::fs::remove_file(path);
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(WIDTH);
    limits.max_image_height = Some(HEIGHT);
    limits.max_alloc = Some((WIDTH * HEIGHT * 4) as u64);
    reader.limits(limits);
    let decoded = reader.decode().ok()?.to_rgba8();
    let [width, height] = [decoded.width(), decoded.height()];
    if width == 0 || height == 0 || width > WIDTH || height > HEIGHT {
        return None;
    }
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [width as usize, height as usize],
        decoded.as_raw(),
    );
    Some(context.load_texture(
        format!("home-thumbnail:{}", project.display()),
        image,
        egui::TextureOptions::LINEAR,
    ))
}

fn prune(directory: &Path, preserve: Option<&Path>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "png")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("rpl-"))
            {
                Some((entry.metadata().ok()?.modified().ok()?, path))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, _)| *modified);
    while files.len() > MAX_CACHE_FILES {
        let index = files
            .iter()
            .position(|(_, path)| Some(path.as_path()) != preserve)
            .unwrap_or(0);
        let (_, path) = files.remove(index);
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paint_document::storage::Store;

    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "rpl-home-thumbnails-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn cache_is_bounded_loadable_and_invalidated_by_project_change() {
        let root = dir();
        let cache = root.join("cache");
        std::fs::create_dir_all(&root).unwrap();
        let project = root.join("sample.rpl");
        let engine = Engine::new(100, 50, 96).unwrap();
        Store::new(project.clone()).save(engine.state()).unwrap();
        let first = cache_path(&cache, &project).unwrap();
        save(&cache, &project, &engine).unwrap();
        let texture = load(&cache, &project, &egui::Context::default()).unwrap();
        assert_eq!(texture.size(), [100, 50]);
        std::fs::write(&project, b"changed size invalidates key").unwrap();
        assert_ne!(cache_path(&cache, &project).unwrap(), first);

        for index in 0..MAX_CACHE_FILES + 3 {
            std::fs::write(cache.join(format!("rpl-{index:02}.png")), b"bounded").unwrap();
        }
        prune(&cache, None);
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), MAX_CACHE_FILES);
        std::fs::remove_dir_all(root).unwrap();
    }
}
