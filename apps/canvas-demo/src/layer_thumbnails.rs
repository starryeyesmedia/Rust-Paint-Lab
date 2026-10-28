//! Bounded raster-layer previews for the floating layer palette.
//!
//! Thumbnail pixels are derived from document tiles, then kept in a small CPU
//! and egui texture cache. A project revision triggers a cheap tile-identity
//! scan; only layers whose raster content or opacity changed are resampled.

use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

use egui::{Color32, Context, TextureHandle, TextureOptions};
use paint_document::{Engine, Layer, TILE};

pub const MAX_THUMBNAIL_WIDTH: usize = 96;
pub const MAX_THUMBNAIL_HEIGHT: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThumbnailImage {
    pub size: [usize; 2],
    pub pixels: Vec<Color32>,
}

struct CachedThumbnail {
    source_signature: u64,
    // Weak refs keep the Arc control blocks alive so an allocator cannot reuse
    // a dropped tile's address and accidentally make stale content look fresh.
    source_tiles: std::collections::BTreeMap<(u32, u32), std::sync::Weak<Vec<u8>>>,
    source_mask_tiles: std::collections::BTreeMap<(u32, u32), std::sync::Weak<Vec<u8>>>,
    image: ThumbnailImage,
    texture: TextureHandle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThumbnailUpdate {
    pub layer_id: u64,
    pub fingerprint: u64,
    pub covered_pixels: usize,
    pub colored_pixels: usize,
}

/// Reuses previews across frames and updates only layers whose raster content
/// or visible opacity changed. The revision avoids walking tile maps at frame
/// rate; Arc identity detects which individual layer needs a new downsample.
#[derive(Default)]
pub struct LayerThumbnailCache {
    generation: Option<(u64, u64)>,
    entries: HashMap<u64, CachedThumbnail>,
    rebuild_count: usize,
}

impl LayerThumbnailCache {
    pub fn invalidate(&mut self) {
        self.generation = None;
        self.entries.clear();
    }

    pub fn refresh(
        &mut self,
        context: &Context,
        document_epoch: u64,
        project: &Engine,
    ) -> Vec<ThumbnailUpdate> {
        let generation = (document_epoch, project.revision);
        if self.generation == Some(generation) {
            return Vec::new();
        }

        let state = project.state();
        let document_size = [state.width, state.height];
        let mut present = std::collections::HashSet::with_capacity(state.layers.len());
        let mut updates = Vec::new();

        for layer in &state.layers {
            present.insert(layer.id);
            let signature = layer_signature(layer, document_size);
            if self.entries.get(&layer.id).is_some_and(|cached| {
                cached.source_signature == signature && source_tiles_match(cached, layer)
            }) {
                continue;
            }

            let image = layer_thumbnail_image(layer, document_size);
            let fingerprint = image_fingerprint(&image);
            let covered_pixels = image.pixels.iter().filter(|pixel| pixel.a() > 0).count();
            let colored_pixels = image
                .pixels
                .iter()
                .filter(|pixel| {
                    pixel.a() > 0
                        && [pixel.r(), pixel.g(), pixel.b()]
                            .iter()
                            .any(|value| *value < 250)
                })
                .count();
            let egui_image = egui::ColorImage::new(image.size, image.pixels.clone());
            if let Some(cached) = self.entries.get_mut(&layer.id) {
                cached.texture.set(egui_image, TextureOptions::LINEAR);
                cached.source_signature = signature;
                cached.source_tiles = layer
                    .tiles
                    .iter()
                    .map(|(key, tile)| (*key, std::sync::Arc::downgrade(tile)))
                    .collect();
                cached.source_mask_tiles = layer
                    .mask
                    .as_ref()
                    .into_iter()
                    .flat_map(|mask| mask.tiles.iter())
                    .map(|(key, tile)| (*key, std::sync::Arc::downgrade(tile)))
                    .collect();
                cached.image = image;
            } else {
                let texture = context.load_texture(
                    format!("layer-thumbnail-{}", layer.id),
                    egui_image,
                    TextureOptions::LINEAR,
                );
                self.entries.insert(
                    layer.id,
                    CachedThumbnail {
                        source_signature: signature,
                        source_tiles: layer
                            .tiles
                            .iter()
                            .map(|(key, tile)| (*key, std::sync::Arc::downgrade(tile)))
                            .collect(),
                        source_mask_tiles: layer
                            .mask
                            .as_ref()
                            .into_iter()
                            .flat_map(|mask| mask.tiles.iter())
                            .map(|(key, tile)| (*key, std::sync::Arc::downgrade(tile)))
                            .collect(),
                        image,
                        texture,
                    },
                );
            }
            self.rebuild_count += 1;
            updates.push(ThumbnailUpdate {
                layer_id: layer.id,
                fingerprint,
                covered_pixels,
                colored_pixels,
            });
        }

        self.entries
            .retain(|layer_id, _| present.contains(layer_id));
        self.generation = Some(generation);
        updates
    }

    pub fn texture(&self, layer_id: u64) -> Option<&TextureHandle> {
        self.entries.get(&layer_id).map(|cached| &cached.texture)
    }

    pub fn image(&self, layer_id: u64) -> Option<&ThumbnailImage> {
        self.entries.get(&layer_id).map(|cached| &cached.image)
    }

    #[cfg(test)]
    fn rebuild_count(&self) -> usize {
        self.rebuild_count
    }
}

pub fn layer_thumbnail_image(layer: &Layer, document_size: [u32; 2]) -> ThumbnailImage {
    let width = document_size[0].max(1);
    let height = document_size[1].max(1);
    let [preview_width, preview_height] = fit_size(width, height);
    let mut pixels = Vec::with_capacity(preview_width * preview_height);

    for y in 0..preview_height {
        for x in 0..preview_width {
            let mut channels = [0u32; 4];
            for sample_y in 0..2u64 {
                for sample_x in 0..2u64 {
                    let source_x = ((4 * x as u64 + 2 * sample_x + 1) * u64::from(width)
                        / (4 * preview_width as u64))
                        .min(u64::from(width - 1)) as u32;
                    let source_y = ((4 * y as u64 + 2 * sample_y + 1) * u64::from(height)
                        / (4 * preview_height as u64))
                        .min(u64::from(height - 1)) as u32;
                    let key = (source_x / TILE, source_y / TILE);
                    let Some(tile) = layer.tiles.get(&key) else {
                        continue;
                    };
                    let offset = ((source_y % TILE * TILE + source_x % TILE) * 4) as usize;
                    let mask_alpha = layer
                        .mask
                        .as_ref()
                        .filter(|mask| mask.enabled)
                        .map(|mask| {
                            let value = mask.tiles.get(&key).map_or(255, |mask_tile| {
                                mask_tile[(source_y % TILE * TILE + source_x % TILE) as usize]
                            });
                            if mask.inverted { 255 - value } else { value }
                        })
                        .unwrap_or(255);
                    for channel in 0..4 {
                        channels[channel] +=
                            (u32::from(tile[offset + channel]) * u32::from(mask_alpha) + 127) / 255;
                    }
                }
            }
            let opacity = u32::from(layer.opacity);
            let channels = channels
                .map(|channel| ((channel * opacity + (4 * 255) / 2) / (4 * 255)).min(255) as u8);
            pixels.push(Color32::from_rgba_premultiplied(
                channels[0],
                channels[1],
                channels[2],
                channels[3],
            ));
        }
    }

    ThumbnailImage {
        size: [preview_width, preview_height],
        pixels,
    }
}

fn fit_size(width: u32, height: u32) -> [usize; 2] {
    let max_width = MAX_THUMBNAIL_WIDTH as u64;
    let max_height = MAX_THUMBNAIL_HEIGHT as u64;
    let (width, height) = (u64::from(width), u64::from(height));
    let scale_is_width_limited = width * max_height >= height * max_width;
    let (preview_width, preview_height) = if scale_is_width_limited {
        (max_width, (height * max_width / width).max(1))
    } else {
        ((width * max_height / height).max(1), max_height)
    };
    [preview_width as usize, preview_height as usize]
}

fn layer_signature(layer: &Layer, document_size: [u32; 2]) -> u64 {
    let mut hasher = DefaultHasher::new();
    layer.id.hash(&mut hasher);
    document_size.hash(&mut hasher);
    layer.opacity.hash(&mut hasher);
    for (key, tile) in &layer.tiles {
        key.hash(&mut hasher);
        (std::sync::Arc::as_ptr(tile) as *const u8 as usize).hash(&mut hasher);
    }
    if let Some(mask) = &layer.mask {
        mask.enabled.hash(&mut hasher);
        mask.inverted.hash(&mut hasher);
        for (key, tile) in &mask.tiles {
            key.hash(&mut hasher);
            (std::sync::Arc::as_ptr(tile) as *const u8 as usize).hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn source_tiles_match(cached: &CachedThumbnail, layer: &Layer) -> bool {
    cached.source_tiles.len() == layer.tiles.len()
        && cached.source_mask_tiles.len() == layer.mask.as_ref().map_or(0, |mask| mask.tiles.len())
        && cached.source_mask_tiles.iter().all(|(key, source)| {
            layer
                .mask
                .as_ref()
                .and_then(|mask| mask.tiles.get(key))
                .is_some_and(|tile| std::ptr::eq(source.as_ptr(), std::sync::Arc::as_ptr(tile)))
        })
        && layer.tiles.iter().all(|(key, tile)| {
            cached
                .source_tiles
                .get(key)
                .is_some_and(|source| std::ptr::eq(source.as_ptr(), std::sync::Arc::as_ptr(tile)))
        })
}

fn image_fingerprint(image: &ThumbnailImage) -> u64 {
    let mut hasher = DefaultHasher::new();
    image.size.hash(&mut hasher);
    for pixel in &image.pixels {
        pixel.to_array().hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use paint_document::{Blend, Engine, editing::GradientKind};
    use paint_input::Tool;
    use paint_stroke::{StrokeCommand, StrokeId, StrokeSample, coordinates::DocumentPoint};
    use std::sync::Arc;

    fn sample(x: f64, y: f64, time_ms: u64) -> StrokeSample {
        StrokeSample {
            source_counter: None,
            pointer_id: 1,
            tool: Tool::Pen,
            position_client_px: [x, y],
            position_document: DocumentPoint([x, y]),
            pressure: Some(1.0),
            tilt_degrees: [None, None],
            buttons: Default::default(),
            source_time_ms: time_ms,
            time_ms,
        }
    }

    fn layer_with_solid_tile(color: [u8; 4], opacity: u8) -> Layer {
        let mut bytes = vec![0; paint_document::TILE_BYTES];
        for pixel in bytes.chunks_exact_mut(4) {
            pixel.copy_from_slice(&color);
        }
        Layer {
            id: 1,
            name: "Paint 1".into(),
            visible: true,
            locked: false,
            opacity,
            blend: Blend::Normal,
            mask: None,
            tiles: std::collections::BTreeMap::from([((0, 0), Arc::new(bytes))]),
        }
    }

    #[test]
    fn thumbnail_is_bounded_aspect_correct_and_applies_layer_opacity() {
        let layer = layer_with_solid_tile([64, 0, 0, 128], 128);
        let image = layer_thumbnail_image(&layer, [64, 64]);
        assert_eq!(image.size, [64, 64]);
        assert_eq!(image.pixels.len(), 64 * 64);
        assert!(
            image
                .pixels
                .iter()
                .all(|pixel| pixel.to_array() == [32, 0, 0, 64])
        );

        let blank = Layer {
            tiles: std::collections::BTreeMap::new(),
            ..layer
        };
        let tall = layer_thumbnail_image(&blank, [64, 4096]);
        assert_eq!(tall.size, [1, 64]);
        assert!(tall.pixels.len() <= MAX_THUMBNAIL_WIDTH * MAX_THUMBNAIL_HEIGHT);
    }

    #[test]
    fn mask_edits_invalidate_cached_layer_thumbnail_and_change_effective_alpha() {
        let context = Context::default();
        let mut project = Engine::new(64, 64, 300).unwrap();
        project
            .edit(|state| {
                let mut pixels = vec![0; paint_document::TILE_BYTES];
                for pixel in pixels.chunks_exact_mut(4) {
                    pixel.copy_from_slice(&[80, 0, 0, 255]);
                }
                state.layers[0].tiles.insert((0, 0), Arc::new(pixels));
                Ok(())
            })
            .unwrap();
        let mut cache = LayerThumbnailCache::default();
        cache.refresh(&context, 0, &project);
        let before = cache.image(1).unwrap().pixels[0];
        assert_eq!(before.a(), 255);

        project.add_layer_mask(1).unwrap();
        project
            .edit(|state| {
                let mask = state.layers[0].mask.as_mut().unwrap();
                let mut values = vec![255; paint_document::MASK_TILE_BYTES];
                values[0] = 0;
                mask.tiles.insert((0, 0), Arc::new(values));
                Ok(())
            })
            .unwrap();
        let updates = cache.refresh(&context, 0, &project);
        assert_eq!(updates.len(), 1);
        assert_eq!(cache.image(1).unwrap().pixels[0].a(), 0);
        assert!(cache.rebuild_count() >= 2);

        project.set_layer_mask_enabled(1, false).unwrap();
        cache.refresh(&context, 0, &project);
        assert_eq!(cache.image(1).unwrap().pixels[0].a(), 255);
    }

    #[test]
    fn cache_reuses_frames_and_invalidates_only_changed_layer_pixels() {
        let context = Context::default();
        let mut project = Engine::new(128, 64, 300).unwrap();
        let mut cache = LayerThumbnailCache::default();
        let first = cache.refresh(&context, 0, &project);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].covered_pixels, 0);
        assert!(cache.refresh(&context, 0, &project).is_empty());
        assert_eq!(cache.rebuild_count(), 1);

        project.add_layer().unwrap();
        let added_layer = cache.refresh(&context, 0, &project);
        assert_eq!(added_layer.len(), 1);
        assert_eq!(added_layer[0].layer_id, 2);
        assert_eq!(cache.rebuild_count(), 2);

        project.brush.color = [240, 20, 30];
        project
            .apply(&StrokeCommand::BeginStroke {
                stroke_id: StrokeId(1),
                sample: sample(16.0, 16.0, 1),
            })
            .unwrap();
        project
            .apply(&StrokeCommand::CommitStroke {
                stroke_id: StrokeId(1),
                sample: sample(96.0, 48.0, 2),
            })
            .unwrap();
        let painted = cache.refresh(&context, 0, &project);
        assert_eq!(painted.len(), 1);
        assert!(painted[0].covered_pixels > 0);
        let painted_fingerprint = painted[0].fingerprint;
        assert_eq!(cache.rebuild_count(), 3);

        project
            .draw_shape(
                paint_document::editing::ShapeKind::Rectangle,
                [20.0, 10.0],
                [110.0, 54.0],
                false,
            )
            .unwrap();
        let shaped = cache.refresh(&context, 0, &project);
        assert_eq!(shaped.len(), 1);
        assert_ne!(shaped[0].fingerprint, painted_fingerprint);
        assert_eq!(cache.rebuild_count(), 4);

        project
            .draw_gradient(
                GradientKind::Linear,
                [0.5, 32.0],
                [127.5, 32.0],
                [10, 120, 240],
            )
            .unwrap();
        let gradient = cache.refresh(&context, 0, &project);
        assert_eq!(gradient.len(), 1);
        let gradient_fingerprint = gradient[0].fingerprint;
        assert_ne!(gradient_fingerprint, shaped[0].fingerprint);

        assert!(project.undo());
        let undo = cache.refresh(&context, 0, &project);
        assert_eq!(undo.len(), 1);
        assert_eq!(undo[0].fingerprint, shaped[0].fingerprint);
        assert!(project.redo());
        let redo = cache.refresh(&context, 0, &project);
        assert_eq!(redo.len(), 1);
        assert_eq!(redo[0].fingerprint, gradient_fingerprint);

        static NEXT_DOCUMENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "bitchpixel-layer-thumbnail-{}-{}.rpl",
            std::process::id(),
            NEXT_DOCUMENT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut store = paint_document::storage::Store::new(path.clone());
        store.save(project.state()).unwrap();
        let (_opened_store, reopened_state) =
            paint_document::storage::Store::open(path.clone()).unwrap();
        let reopened = Engine::from_state(reopened_state).unwrap();
        cache.invalidate();
        let reopened_update = cache.refresh(&context, 1, &reopened);
        assert_eq!(reopened_update.len(), 2);
        let reopened_paint_layer = reopened_update
            .iter()
            .find(|update| update.layer_id == 2)
            .unwrap();
        assert_eq!(reopened_paint_layer.fingerprint, gradient_fingerprint);
        assert_eq!(reopened_update[0].layer_id, 1);
        assert_eq!(reopened_update[0].covered_pixels, 0);
        assert_eq!(
            image_fingerprint(cache.image(2).unwrap()),
            gradient_fingerprint
        );
        drop(store);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("rpl.lock"));
        let _ = std::fs::remove_file(path.with_extension("rpl.bak"));
    }

    #[test]
    fn thumbnail_retains_brush_marks_at_typical_document_scale() {
        let context = Context::default();
        let mut project = Engine::new(1600, 1000, 300).unwrap();
        project.brush.color = [24, 72, 220];
        project.brush.size = 18.0;
        project
            .apply(&StrokeCommand::BeginStroke {
                stroke_id: StrokeId(9),
                sample: sample(5.0, 200.0, 1),
            })
            .unwrap();
        project
            .apply(&StrokeCommand::UpdateStroke {
                stroke_id: StrokeId(9),
                sample: sample(75.0, 270.0, 2),
            })
            .unwrap();
        project
            .apply(&StrokeCommand::CommitStroke {
                stroke_id: StrokeId(9),
                sample: sample(143.0, 340.0, 3),
            })
            .unwrap();

        let mut cache = LayerThumbnailCache::default();
        let update = cache.refresh(&context, 0, &project);
        let image = cache.image(1).unwrap();
        let colored_pixels = image
            .pixels
            .iter()
            .filter(|pixel| pixel.b() > pixel.r().saturating_add(40) && pixel.a() > 0)
            .count();

        assert_eq!(update.len(), 1);
        assert!(
            colored_pixels >= 5,
            "thumbnail lost the brush mark: {colored_pixels} pixels"
        );
        assert!(update[0].colored_pixels >= 5);
    }
}
