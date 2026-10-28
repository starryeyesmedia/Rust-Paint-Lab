//! Bounded, checksummed editable alpha format. Atomic replacement preserves the
//! prior valid file and backup; GPU/platform objects are never serialized.
use crate::{MAX_TILES, PaintError, State, TILE, TILE_BYTES};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const MAGIC: &[u8; 8] = b"RPLALP01";
pub(crate) const MAX_FILE: usize = 160 * 1024 * 1024;
const MAX_BRUSH_FILE: usize = 16 * 1024;
static NONCE: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    None,
    AfterWrite,
    BeforeReplace,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
pub fn encode(state: &State) -> Result<Vec<u8>, PaintError> {
    state.validate()?;
    let meta = serde_json::to_vec(state)?;
    if meta.len() > 65536 {
        return Err(PaintError::Limit);
    }
    let mut bytes = Vec::with_capacity(meta.len() + state.tile_count() * (TILE_BYTES + 16) + 48);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(meta.len() as u32).to_le_bytes());
    bytes.extend(meta);
    bytes.extend_from_slice(&(state.tile_count() as u32).to_le_bytes());
    for l in &state.layers {
        for (&(x, y), tile) in &l.tiles {
            bytes.extend_from_slice(&l.id.to_le_bytes());
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.extend_from_slice(tile);
        }
    }
    bytes.extend(hash(&bytes));
    Ok(bytes)
}
pub fn decode(bytes: &[u8]) -> Result<State, PaintError> {
    if bytes.len() < 48
        || bytes.len() > MAX_FILE
        || &bytes[..8] != MAGIC
        || hash(&bytes[..bytes.len() - 32]) != bytes[bytes.len() - 32..]
    {
        return Err(PaintError::Invalid);
    }
    let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if len > 65536 || 12 + len + 4 > bytes.len() - 32 {
        return Err(PaintError::Invalid);
    }
    let mut state: State = serde_json::from_slice(&bytes[12..12 + len])?;
    state.validate()?;
    let count = u32::from_le_bytes(bytes[12 + len..16 + len].try_into().unwrap()) as usize;
    if count > MAX_TILES || 16 + len + count * (16 + TILE_BYTES) + 32 != bytes.len() {
        return Err(PaintError::Limit);
    }
    let mut offset = 16 + len;
    for _ in 0..count {
        let id = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        let x = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());
        let y = u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap());
        offset += 16;
        let l = state
            .layers
            .iter_mut()
            .find(|l| l.id == id)
            .ok_or(PaintError::Invalid)?;
        if l.tiles.contains_key(&(x, y))
            || x >= state.width.div_ceil(TILE)
            || y >= state.height.div_ceil(TILE)
        {
            return Err(PaintError::Invalid);
        }
        l.tiles.insert(
            (x, y),
            std::sync::Arc::new(bytes[offset..offset + TILE_BYTES].to_vec()),
        );
        offset += TILE_BYTES;
    }
    state.validate()?;
    Ok(state)
}
pub fn load(path: &Path) -> Result<State, PaintError> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_FILE as u64 + 1)
        .read_to_end(&mut bytes)?;
    decode(&bytes)
}

/// Read only the bounded JSON header for lightweight recent-project labels.
/// The result is informational and is never used to allocate or open artwork;
/// `load` still performs complete size, structure and checksum validation.
pub fn peek_dimensions(path: &Path) -> Option<(u32, u32, u32)> {
    #[derive(serde::Deserialize)]
    struct Header {
        width: u32,
        height: u32,
        dpi: u32,
    }

    let mut file = File::open(path).ok()?;
    if file.metadata().ok()?.len() > MAX_FILE as u64 {
        return None;
    }
    let mut prefix = [0u8; 12];
    file.read_exact(&mut prefix).ok()?;
    if &prefix[..8] != MAGIC {
        return None;
    }
    let metadata_len = u32::from_le_bytes(prefix[8..12].try_into().ok()?) as usize;
    if metadata_len == 0 || metadata_len > 65536 {
        return None;
    }
    let mut metadata = vec![0; metadata_len];
    file.read_exact(&mut metadata).ok()?;
    let header: Header = serde_json::from_slice(&metadata).ok()?;
    if !(1..=4096).contains(&header.width)
        || !(1..=4096).contains(&header.height)
        || !(1..=9600).contains(&header.dpi)
    {
        return None;
    }
    Some((header.width, header.height, header.dpi))
}
pub(crate) fn temporary(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        ".{}.tmp-{}-{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ))
}
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    path.with_file_name(format!(
        "{}{suffix}",
        path.file_name().unwrap_or_default().to_string_lossy()
    ))
}
pub struct Store {
    pub path: PathBuf,
    expected: Option<[u8; 32]>,
}
impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            expected: None,
        }
    }
    pub fn open(path: PathBuf) -> Result<(Self, State), PaintError> {
        let mut bytes = Vec::new();
        File::open(&path)?
            .take(MAX_FILE as u64 + 1)
            .read_to_end(&mut bytes)?;
        let state = decode(&bytes)?;
        let expected = Some(hash(&bytes));
        Ok((Self { path, expected }, state))
    }
    pub fn save(&mut self, state: &State) -> Result<(), PaintError> {
        self.save_fault(state, Fault::None)
    }
    pub fn save_fault(&mut self, state: &State, fault: Fault) -> Result<(), PaintError> {
        if self.path.extension().is_none_or(|e| e != "rpl") {
            return Err(PaintError::Invalid);
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(sibling(&self.path, ".lock"))?;
        lock.try_lock().map_err(|_| PaintError::Busy)?;
        if self.path.exists() {
            if fs::symlink_metadata(&self.path)?.file_type().is_symlink() {
                return Err(PaintError::Conflict);
            }
            let mut old = Vec::new();
            File::open(&self.path)?
                .take(MAX_FILE as u64 + 1)
                .read_to_end(&mut old)?;
            decode(&old)?;
            if self.expected != Some(hash(&old)) {
                return Err(PaintError::Conflict);
            }
        } else if self.expected.is_some() {
            return Err(PaintError::Conflict);
        }
        let bytes = encode(state)?;
        let tmp = temporary(&self.path);
        let result = (|| {
            let mut out = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
            out.write_all(&bytes)?;
            out.sync_all()?;
            if fault == Fault::AfterWrite {
                return Err(PaintError::Io(std::io::Error::other(
                    "injected after-write failure",
                )));
            }
            if self.path.exists() {
                let backup = sibling(&self.path, ".bak");
                let backup_tmp = temporary(&backup);
                fs::copy(&self.path, &backup_tmp)?;
                OpenOptions::new()
                    .write(true)
                    .open(&backup_tmp)?
                    .sync_all()?;
                fs::rename(&backup_tmp, &backup)?;
            }
            if fault == Fault::BeforeReplace {
                return Err(PaintError::Io(std::io::Error::other(
                    "injected before-replace failure",
                )));
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
            Ok(())
        })();
        if tmp.exists() {
            let _ = fs::remove_file(&tmp);
        }
        result?;
        self.expected = Some(hash(&bytes));
        Ok(())
    }
}
pub fn export_png(path: &Path, engine: &crate::Engine) -> Result<(), PaintError> {
    if engine.active_count() > 0 {
        return Err(PaintError::Busy);
    }
    if path.extension().is_none_or(|e| e != "png") {
        return Err(PaintError::Invalid);
    }
    if path.exists() {
        return Err(PaintError::Conflict);
    }
    let tmp = temporary(path);
    let result = (|| {
        let file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        let mut encoder = png::Encoder::new(file, engine.state().width, engine.state().height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let ppm = (f64::from(engine.state().dpi) / 0.0254).round() as u32;
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: png::Unit::Meter,
        }));
        let mut writer = encoder
            .write_header()
            .map_err(|e| PaintError::Png(e.to_string()))?;
        writer
            .write_image_data(&engine.rgba())
            .map_err(|e| PaintError::Png(e.to_string()))?;
        writer
            .finish()
            .map_err(|e| PaintError::Png(e.to_string()))?;
        OpenOptions::new().write(true).open(&tmp)?.sync_all()?;
        // Hard-link creation is an atomic no-clobber publish on supported local
        // filesystems. A concurrently-created export is never overwritten.
        fs::hard_link(&tmp, path)?;
        #[cfg(unix)]
        File::open(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )?
        .sync_all()?;
        Ok(())
    })();
    if tmp.exists() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Save one validated, versioned brush preset without replacing another file.
pub fn save_brush_preset(path: &Path, brush: &crate::brush::Brush) -> Result<(), PaintError> {
    if path.extension().is_none_or(|e| e != "rplbrush") {
        return Err(PaintError::Invalid);
    }
    let data = brush.encode()?.into_bytes();
    if data.len() > MAX_BRUSH_FILE {
        return Err(PaintError::Limit);
    }
    let tmp = temporary(path);
    let result = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(&data)?;
        file.sync_all()?;
        match fs::hard_link(&tmp, path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(PaintError::Conflict);
            }
            Err(error) => return Err(error.into()),
        }
        #[cfg(unix)]
        File::open(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )?
        .sync_all()?;
        Ok(())
    })();
    if tmp.exists() {
        let _ = fs::remove_file(tmp);
    }
    result
}

/// Load a bounded, validated versioned brush preset.
pub fn load_brush_preset(path: &Path) -> Result<crate::brush::Brush, PaintError> {
    if path.extension().is_none_or(|e| e != "rplbrush") {
        return Err(PaintError::Invalid);
    }
    let input = File::open(path)?;
    if input.metadata()?.len() > MAX_BRUSH_FILE as u64 {
        return Err(PaintError::Limit);
    }
    let mut data = Vec::with_capacity(MAX_BRUSH_FILE.min(4096));
    input
        .take(MAX_BRUSH_FILE as u64 + 1)
        .read_to_end(&mut data)?;
    if data.len() > MAX_BRUSH_FILE {
        return Err(PaintError::Limit);
    }
    let text = std::str::from_utf8(&data).map_err(|_| PaintError::Invalid)?;
    crate::brush::Brush::decode(text)
}

pub fn import_png(path: &Path, engine: &mut crate::Engine) -> Result<(), PaintError> {
    let input = File::open(path)?;
    if input.metadata()?.len() > MAX_FILE as u64 {
        return Err(PaintError::Limit);
    }
    let mut decoder = png::Decoder::new(std::io::BufReader::new(input));
    decoder.set_limits(png::Limits {
        bytes: 80 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| PaintError::Png(e.to_string()))?;
    let info = reader.info();
    if info.width == 0
        || info.height == 0
        || info.width > engine.state().width
        || info.height > engine.state().height
    {
        return Err(PaintError::Limit);
    }
    let mut bytes = vec![0; reader.output_buffer_size().ok_or(PaintError::Limit)?];
    let frame = reader
        .next_frame(&mut bytes)
        .map_err(|e| PaintError::Png(e.to_string()))?;
    let channels = match frame.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        _ => return Err(PaintError::Invalid),
    };
    let mut tiles = crate::TileMap::new();
    for y in 0..frame.height {
        for x in 0..frame.width {
            let i = ((y * frame.width + x) * channels) as usize;
            let p = &bytes[i..i + channels as usize];
            let c = match channels {
                4 => [p[0], p[1], p[2], p[3]],
                3 => [p[0], p[1], p[2], 255],
                2 => [p[0], p[0], p[0], p[1]],
                _ => [p[0], p[0], p[0], 255],
            };
            if c[3] == 0 {
                continue;
            }
            let tile = tiles
                .entry((x / TILE, y / TILE))
                .or_insert_with(|| std::sync::Arc::new(vec![0; TILE_BYTES]));
            let t = std::sync::Arc::make_mut(tile);
            let j = ((y % TILE * TILE + x % TILE) * 4) as usize;
            for k in 0..3 {
                t[j + k] = ((u32::from(c[k]) * u32::from(c[3]) + 127) / 255) as u8;
            }
            t[j + 3] = c[3];
        }
    }
    engine.edit(|s| {
        let id = s.layers.iter().map(|l| l.id).max().unwrap_or(0) + 1;
        s.layers.push(crate::Layer {
            id,
            name: "Imported PNG".into(),
            visible: true,
            locked: false,
            opacity: 255,
            blend: crate::Blend::Normal,
            tiles,
        });
        s.selected = id;
        Ok(())
    })
}

/// Decode a bounded PNG or JPEG into straight-alpha 8-bit RGBA for a pending
/// placement. The caller can show a preview and cancel before changing artwork.
pub fn decode_placement_image(path: &Path) -> Result<(u32, u32, Vec<u8>), PaintError> {
    let reader = image::ImageReader::open(path)
        .map_err(|error| PaintError::Image(error.to_string()))?
        .with_guessed_format()
        .map_err(|error| PaintError::Image(error.to_string()))?;
    let mut reader = reader;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(80 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|error| PaintError::Image(error.to_string()))?
        .to_rgba8();
    let (width, height) = decoded.dimensions();
    if width == 0 || height == 0 || decoded.as_raw().len() > 64 * 1024 * 1024 {
        return Err(PaintError::Limit);
    }
    Ok((width, height, decoded.into_raw()))
}

#[cfg(test)]
mod recent_header_tests {
    use super::*;

    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "rpl-header-{}-{}.rpl",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn recent_dimension_peek_reads_only_bounded_valid_header_fields() {
        let path = path();
        let state = State::new(320, 240, 144).unwrap();
        fs::write(&path, encode(&state).unwrap()).unwrap();
        assert_eq!(peek_dimensions(&path), Some((320, 240, 144)));
        fs::write(&path, b"RPLALP01\xff\xff\xff\xff").unwrap();
        assert_eq!(peek_dimensions(&path), None);
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod image_placement_decode_tests {
    use super::decode_placement_image;
    use std::sync::atomic::Ordering;
    use std::{fs, path::PathBuf};

    fn path(extension: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rpl-placement-{}-{}.{}",
            std::process::id(),
            super::NONCE.fetch_add(1, Ordering::Relaxed),
            extension
        ))
    }

    #[test]
    fn bounded_placement_decoder_reads_png_and_jpeg() {
        let pixels = image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 64]).unwrap();
        let png = path("png");
        pixels
            .save_with_format(&png, image::ImageFormat::Png)
            .unwrap();
        assert_eq!(
            decode_placement_image(&png).unwrap(),
            (2, 1, pixels.as_raw().clone())
        );
        fs::remove_file(png).unwrap();

        let jpeg = path("jpg");
        image::RgbImage::from_raw(2, 1, vec![255, 0, 0, 0, 0, 255])
            .unwrap()
            .save_with_format(&jpeg, image::ImageFormat::Jpeg)
            .unwrap();
        let (width, height, rgba) = decode_placement_image(&jpeg).unwrap();
        assert_eq!((width, height), (2, 1));
        assert_eq!(rgba.len(), 8);
        assert_eq!(rgba[3], 255);
        assert_eq!(rgba[7], 255, "JPEG has no alpha channel");
        fs::remove_file(jpeg).unwrap();
    }

    #[test]
    fn placement_decoder_rejects_corrupt_and_overlarge_images() {
        let corrupt = path("png");
        fs::write(&corrupt, b"not a PNG or JPEG").unwrap();
        assert!(decode_placement_image(&corrupt).is_err());
        fs::remove_file(corrupt).unwrap();

        let overlarge = path("png");
        image::RgbaImage::new(4097, 1)
            .save_with_format(&overlarge, image::ImageFormat::Png)
            .unwrap();
        assert!(decode_placement_image(&overlarge).is_err());
        fs::remove_file(overlarge).unwrap();
    }
}
