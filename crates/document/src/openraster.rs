//! Restricted, documented OpenRaster interoperability. Never extracts ZIP paths.
use crate::{Blend, Engine, Layer, PaintError, State, TILE, TILE_BYTES, TileMap};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Arc,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
const MAX_FILE: u64 = 256 * 1024 * 1024;
fn format_error(message: impl std::fmt::Display) -> PaintError {
    PaintError::Format(message.to_string())
}
fn u16le(b: &[u8]) -> u16 {
    u16::from_le_bytes(b[..2].try_into().unwrap())
}
fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes(b[..4].try_into().unwrap())
}
/// Bound ZIP central directory allocation BEFORE invoking the library parser.
/// Reject ZIP64, encryption, multidisk and excessive/ambiguous entries.
fn preflight(file: &mut File) -> Result<(), PaintError> {
    let size = file.metadata()?.len();
    if !(22..=MAX_FILE).contains(&size) {
        return Err(PaintError::Limit);
    }
    let tail_size = size.min(65557) as usize;
    file.seek(SeekFrom::End(-(tail_size as i64)))?;
    let mut tail = vec![0; tail_size];
    file.read_exact(&mut tail)?;
    let end = (0..=tail.len() - 22)
        .rev()
        .find(|&i| {
            &tail[i..i + 4] == b"PK\x05\x06"
                && i + 22 + usize::from(u16le(&tail[i + 20..])) == tail.len()
        })
        .ok_or(PaintError::Invalid)?;
    let footer = &tail[end..];
    let count = usize::from(u16le(&footer[10..]));
    let length = u64::from(u32le(&footer[12..]));
    let offset = u64::from(u32le(&footer[16..]));
    let absolute_end = size - tail_size as u64 + end as u64;
    if u16le(&footer[4..]) != 0
        || u16le(&footer[6..]) != 0
        || usize::from(u16le(&footer[8..])) != count
        || count > 40
        || count == 0
        || length > 65536
        || offset + length != absolute_end
    {
        return Err(format_error(
            "OpenRaster ZIP64/multidisk/large directory is unsupported",
        ));
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut directory = vec![0; length as usize];
    file.read_exact(&mut directory)?;
    let mut at = 0;
    let mut names = HashSet::new();
    for _ in 0..count {
        if at + 46 > directory.len() || &directory[at..at + 4] != b"PK\x01\x02" {
            return Err(PaintError::Invalid);
        }
        let h = &directory[at..at + 46];
        let name_len = usize::from(u16le(&h[28..]));
        let extra = usize::from(u16le(&h[30..]));
        let comment = usize::from(u16le(&h[32..]));
        let end = at + 46 + name_len + extra + comment;
        if end > directory.len()
            || name_len > 256
            || extra > 4096
            || u16le(&h[8..]) & 1 != 0
            || u16le(&h[34..]) != 0
            || (u32le(&h[38..]) >> 16) & 0o170000 == 0o120000
            || !matches!(u16le(&h[10..]), 0 | 8)
            || u32le(&h[24..]) > 80 * 1024 * 1024
            || u32le(&h[42..]) == u32::MAX
        {
            return Err(PaintError::Limit);
        }
        let name =
            std::str::from_utf8(&directory[at + 46..at + 46 + name_len]).map_err(format_error)?;
        if name.contains('\0')
            || name.starts_with('/')
            || name.contains('\\')
            || name.contains(':')
            || name.split('/').any(|p| p == "..")
            || !names.insert(name.to_owned())
        {
            return Err(PaintError::Invalid);
        }
        let mut extra_at = at + 46 + name_len;
        let extra_end = extra_at + extra;
        while extra_at < extra_end {
            if extra_at + 4 > extra_end {
                return Err(PaintError::Invalid);
            }
            let n = usize::from(u16le(&directory[extra_at + 2..]));
            if u16le(&directory[extra_at..]) == 1 {
                return Err(format_error("ZIP64 OpenRaster is unsupported"));
            }
            extra_at += 4 + n;
            if extra_at > extra_end {
                return Err(PaintError::Invalid);
            }
        }
        at = end;
    }
    if at != directory.len() {
        return Err(PaintError::Invalid);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(())
}
fn entry(archive: &mut ZipArchive<File>, name: &str, limit: usize) -> Result<Vec<u8>, PaintError> {
    let file = archive.by_name(name).map_err(format_error)?;
    if file.size() > limit as u64 || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
        return Err(PaintError::Limit);
    }
    let mut data = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut data)?;
    if data.len() > limit {
        return Err(PaintError::Limit);
    }
    Ok(data)
}
fn png_encode(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, PaintError> {
    let mut data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut data, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().map_err(format_error)?;
        writer.write_image_data(rgba).map_err(format_error)?;
        writer.finish().map_err(format_error)?;
    }
    Ok(data)
}

/// Encode one layer's sparse premultiplied tiles as a straight-alpha RGBA image.
/// The layer's visibility, opacity, and blend mode belong in stack.xml and must
/// not be baked into its source pixels. Iterating only stored tiles also avoids
/// cloning and revalidating the full document once per exported layer.
fn layer_rgba(width: u32, height: u32, layer: &Layer) -> Result<Vec<u8>, PaintError> {
    let byte_count = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or(PaintError::Limit)?;
    let mut rgba = vec![0; byte_count];
    for (&(tile_x, tile_y), tile) in &layer.tiles {
        let origin_x = tile_x.checked_mul(TILE).ok_or(PaintError::Limit)?;
        let origin_y = tile_y.checked_mul(TILE).ok_or(PaintError::Limit)?;
        let valid_width = TILE.min(width.saturating_sub(origin_x));
        let valid_height = TILE.min(height.saturating_sub(origin_y));
        for y in 0..valid_height {
            for x in 0..valid_width {
                let source = ((y * TILE + x) * 4) as usize;
                let source_alpha = u32::from(tile[source + 3]);
                if source_alpha == 0 {
                    continue;
                }
                let alpha = if let Some(mask) = layer.mask.as_ref().filter(|mask| mask.enabled) {
                    let mask_offset = (y * TILE + x) as usize;
                    let value = mask
                        .tiles
                        .get(&(tile_x, tile_y))
                        .map_or(255, |mask_tile| mask_tile[mask_offset]);
                    u32::from(if mask.inverted { 255 - value } else { value })
                } else {
                    255
                };
                let alpha = (source_alpha * alpha + 127) / 255;
                if alpha == 0 {
                    continue;
                }
                let destination = (((origin_y + y) * width + origin_x + x) * 4) as usize;
                for channel in 0..3 {
                    rgba[destination + channel] = ((u32::from(tile[source + channel]) * 255
                        + source_alpha / 2)
                        / source_alpha)
                        .min(255) as u8;
                }
                rgba[destination + 3] = alpha as u8;
            }
        }
    }
    Ok(rgba)
}

fn solid_rgba(width: u32, height: u32, color: [u8; 3]) -> Result<Vec<u8>, PaintError> {
    let byte_count = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or(PaintError::Limit)?;
    let mut rgba = vec![0; byte_count];
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[color[0], color[1], color[2], 255]);
    }
    Ok(rgba)
}

fn png_tiles(data: &[u8], width: u32, height: u32) -> Result<TileMap, PaintError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.set_limits(png::Limits {
        bytes: 80 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(format_error)?;
    let info = reader.info();
    if info.width > width || info.height > height {
        return Err(PaintError::Limit);
    }
    let mut pixels = vec![0; reader.output_buffer_size().ok_or(PaintError::Limit)?];
    let frame = reader.next_frame(&mut pixels).map_err(format_error)?;
    let channels = match frame.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        _ => return Err(PaintError::Invalid),
    };
    let mut tiles = TileMap::new();
    for y in 0..frame.height {
        for x in 0..frame.width {
            let i = ((y * frame.width + x) * channels) as usize;
            let p = &pixels[i..i + channels as usize];
            let rgba = match channels {
                4 => [p[0], p[1], p[2], p[3]],
                3 => [p[0], p[1], p[2], 255],
                2 => [p[0], p[0], p[0], p[1]],
                _ => [p[0], p[0], p[0], 255],
            };
            if rgba[3] == 0 {
                continue;
            }
            let t = tiles
                .entry((x / TILE, y / TILE))
                .or_insert_with(|| Arc::new(vec![0; TILE_BYTES]));
            let t = Arc::make_mut(t);
            let i = ((y % TILE * TILE + x % TILE) * 4) as usize;
            for c in 0..3 {
                t[i + c] = ((u32::from(rgba[c]) * u32::from(rgba[3]) + 127) / 255) as u8;
            }
            t[i + 3] = rgba[3];
        }
    }
    Ok(tiles)
}
pub fn load(path: &Path) -> Result<State, PaintError> {
    let mut file = File::open(path)?;
    preflight(&mut file)?;
    let mut archive = ZipArchive::with_config(
        zip::read::Config {
            archive_offset: zip::read::ArchiveOffset::Known(0),
        },
        file,
    )
    .map_err(format_error)?;
    if entry(&mut archive, "mimetype", 64)? != b"image/openraster" {
        return Err(PaintError::Invalid);
    }
    let xml = entry(&mut archive, "stack.xml", 65536)?;
    let xml = std::str::from_utf8(&xml).map_err(format_error)?;
    let tree = roxmltree::Document::parse(xml).map_err(format_error)?;
    let image = tree.root_element();
    if !image.has_tag_name("image") {
        return Err(PaintError::Invalid);
    }
    let number = |name| {
        image
            .attribute(name)
            .ok_or(PaintError::Invalid)?
            .parse::<u32>()
            .map_err(format_error)
    };
    let width = number("w")?;
    let height = number("h")?;
    let resolution = |name| {
        image
            .attribute(name)
            .unwrap_or("300")
            .parse::<f64>()
            .map_err(format_error)
    };
    let dpi = resolution("xres")?;
    let yres = image
        .attribute("yres")
        .map(|y| y.parse::<f64>().map_err(format_error))
        .transpose()?
        .unwrap_or(dpi);
    if !dpi.is_finite() || !(1.0..=9600.0).contains(&dpi) || (dpi - yres).abs() > 0.0001 {
        return Err(format_error(
            "Invalid or unequal X/Y resolution is unsupported",
        ));
    }
    let dpi = dpi.round() as u32;
    let mut state = State::new(width, height, dpi)?;
    state.background = None;
    state.layers.clear();
    let stacks: Vec<_> = image.children().filter(|n| n.is_element()).collect();
    if stacks.len() != 1 || !stacks[0].has_tag_name("stack") {
        return Err(format_error("OpenRaster requires one root stack"));
    }
    let nodes: Vec<_> = stacks[0].children().filter(|n| n.is_element()).collect();
    if nodes.is_empty() || nodes.len() > crate::MAX_LAYERS {
        return Err(PaintError::Limit);
    }
    for node in nodes.into_iter().rev() {
        if !node.has_tag_name("layer") || node.children().any(|n| n.is_element()) {
            return Err(format_error(
                "OpenRaster groups/masks/vector layers are not supported; import refused",
            ));
        }
        if node.attribute("x").unwrap_or("0") != "0" || node.attribute("y").unwrap_or("0") != "0" {
            return Err(format_error(
                "Offset/cropped OpenRaster layers are not yet supported",
            ));
        }
        let src = node.attribute("src").ok_or(PaintError::Invalid)?;
        if !src.starts_with("data/") || src.contains("..") || src.len() > 256 {
            return Err(PaintError::Invalid);
        }
        let blend = match node.attribute("composite-op").unwrap_or("svg:src-over") {
            "svg:src-over" => Blend::Normal,
            "svg:multiply" => Blend::Multiply,
            "svg:screen" => Blend::Screen,
            "svg:overlay" => Blend::Overlay,
            _ => {
                return Err(format_error(
                    "Unsupported OpenRaster blend mode; import refused",
                ));
            }
        };
        let opacity = node
            .attribute("opacity")
            .unwrap_or("1")
            .parse::<f64>()
            .map_err(format_error)?;
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err(PaintError::Invalid);
        }
        let visibility = node.attribute("visibility").unwrap_or("visible");
        if !matches!(visibility, "visible" | "hidden") {
            return Err(PaintError::Invalid);
        }
        let data = entry(&mut archive, src, 80 * 1024 * 1024)?;
        let tiles = png_tiles(&data, width, height)?;
        if state.tile_count() + tiles.len() > crate::MAX_TILES {
            return Err(PaintError::Limit);
        }
        let id = state.layers.len() as u64 + 1;
        state.layers.push(Layer {
            id,
            name: node.attribute("name").unwrap_or("Imported layer").into(),
            visible: visibility == "visible",
            locked: false,
            opacity: (opacity * 255.0).round() as u8,
            blend,
            mask: None,
            tiles,
        });
    }
    state.selected = state.layers.last().unwrap().id;
    state.validate()?;
    Ok(state)
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
pub fn export(path: &Path, engine: &Engine) -> Result<(), PaintError> {
    if engine.active_count() > 0 {
        return Err(PaintError::Busy);
    }
    if path.extension().is_none_or(|e| e != "ora") || path.exists() {
        return Err(PaintError::Conflict);
    }
    let state = engine.state();
    state.validate()?;
    if state.layers.iter().any(|l| {
        l.name.chars().any(|c| {
            (c < ' ' && !matches!(c, '\t' | '\r' | '\n')) || matches!(c, '\u{fffe}' | '\u{ffff}')
        })
    }) {
        return Err(PaintError::Invalid);
    }
    if state.layers.len() + usize::from(state.background.is_some()) > crate::MAX_LAYERS {
        return Err(PaintError::Limit);
    }
    // OpenRaster has no portable layer-mask edit model in this restricted
    // importer. Export the effective per-layer alpha into its PNG layer and
    // report the loss in the application status; the native .rpl remains the
    // editable source of truth.
    let temp = crate::storage::temporary(path);
    let result = (|| {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("mimetype", options).map_err(format_error)?;
        zip.write_all(b"image/openraster")?;
        let mut xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><image version=\"0.0.3\" w=\"{}\" h=\"{}\" xres=\"{}\" yres=\"{}\"><stack>",
            state.width, state.height, state.dpi, state.dpi
        );
        for (index, layer) in state.layers.iter().rev().enumerate() {
            let pixels = layer_rgba(state.width, state.height, layer)?;
            let png = png_encode(state.width, state.height, &pixels)?;
            let src = format!("data/layer{index}.png");
            zip.start_file(&src, options).map_err(format_error)?;
            zip.write_all(&png)?;
            let blend = match layer.blend {
                Blend::Normal => "svg:src-over",
                Blend::Multiply => "svg:multiply",
                Blend::Screen => "svg:screen",
                Blend::Overlay => "svg:overlay",
            };
            xml.push_str(&format!("<layer name=\"{}\" src=\"{}\" opacity=\"{}\" visibility=\"{}\" composite-op=\"{}\" x=\"0\" y=\"0\"/>",escape(&layer.name),src,f64::from(layer.opacity)/255.0,if layer.visible{"visible"}else{"hidden"},blend));
        }
        if let Some(background) = state.background {
            let png = png_encode(
                state.width,
                state.height,
                &solid_rgba(state.width, state.height, background)?,
            )?;
            zip.start_file("data/background.png", options)
                .map_err(format_error)?;
            zip.write_all(&png)?;
            xml.push_str("<layer name=\"Background\" src=\"data/background.png\" opacity=\"1\" visibility=\"visible\" composite-op=\"svg:src-over\" x=\"0\" y=\"0\"/>");
        }
        xml.push_str("</stack></image>");
        zip.start_file("stack.xml", options).map_err(format_error)?;
        zip.write_all(xml.as_bytes())?;
        zip.start_file("mergedimage.png", options)
            .map_err(format_error)?;
        zip.write_all(&png_encode(state.width, state.height, &engine.rgba())?)?;
        let file = zip.finish().map_err(format_error)?;
        if file.metadata()?.len() > MAX_FILE {
            return Err(PaintError::Limit);
        }
        file.sync_all()?;
        fs::hard_link(&temp, path)?;
        Ok(())
    })();
    let _ = fs::remove_file(temp);
    result
}

#[cfg(test)]
mod layer_rgba_tests {
    use super::layer_rgba;
    use crate::{Engine, LayerMask, MASK_TILE_BYTES, TILE_BYTES};
    use std::sync::Arc;

    #[test]
    fn sparse_layer_export_unpremultiplies_alpha_and_clips_edge_tiles() {
        let mut engine = Engine::new(65, 65, 300).unwrap();
        engine
            .edit(|state| {
                state.layers[0].opacity = 17;
                let mut first = vec![0; TILE_BYTES];
                first[..4].copy_from_slice(&[40, 20, 0, 128]);
                let mut edge = vec![0; TILE_BYTES];
                edge[..4].copy_from_slice(&[12, 24, 36, 64]);
                edge[4..8].copy_from_slice(&[50, 0, 0, 50]);
                state.layers[0].tiles.insert((0, 0), Arc::new(first));
                state.layers[0].tiles.insert((1, 1), Arc::new(edge));
                Ok(())
            })
            .unwrap();

        let image = layer_rgba(65, 65, &engine.state().layers[0]).unwrap();
        assert_eq!(image.len(), 65 * 65 * 4);
        assert_eq!(&image[..4], &[80, 40, 0, 128]);
        let edge_pixel = ((64 * 65 + 64) * 4) as usize;
        assert_eq!(&image[edge_pixel..edge_pixel + 4], &[48, 96, 143, 64]);
    }

    #[test]
    fn openraster_layer_pixels_apply_enabled_and_inverted_masks() {
        let mut engine = Engine::new(4, 4, 300).unwrap();
        engine
            .edit(|state| {
                let mut pixels = vec![0; TILE_BYTES];
                pixels[..4].copy_from_slice(&[100, 0, 0, 100]);
                let mut alpha = vec![255; MASK_TILE_BYTES];
                alpha[0] = 0;
                state.layers[0].tiles.insert((0, 0), Arc::new(pixels));
                state.layers[0].mask = Some(LayerMask {
                    enabled: true,
                    inverted: false,
                    tiles: [((0, 0), Arc::new(alpha))].into_iter().collect(),
                });
                Ok(())
            })
            .unwrap();
        let layer = &engine.state().layers[0];
        let hidden = layer_rgba(4, 4, layer).unwrap();
        assert_eq!(&hidden[..4], &[0, 0, 0, 0]);
        assert_eq!(&hidden[4..8], &[0, 0, 0, 0]);

        let mut inverted = layer.clone();
        inverted.mask.as_mut().unwrap().inverted = true;
        let revealed = layer_rgba(4, 4, &inverted).unwrap();
        assert_eq!(&revealed[..4], &[255, 0, 0, 100]);
        assert_eq!(&revealed[4..8], &[0, 0, 0, 0]);

        inverted.mask.as_mut().unwrap().enabled = false;
        let disabled = layer_rgba(4, 4, &inverted).unwrap();
        assert_eq!(&disabled[..4], &[255, 0, 0, 100]);
    }
}
