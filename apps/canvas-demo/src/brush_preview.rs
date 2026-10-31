//! Bounded brush-library thumbnails rendered through the real raster engine.
//! This sample is ephemeral UI state; it never enters an artist's document.

use paint_document::{Engine, PaintError, State, brush::Brush};
use paint_stroke::{StrokeCommand, StrokeId, StrokeSample, coordinates::DocumentPoint};

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 96;
pub const MAX_PREVIEW_BYTES: usize = WIDTH as usize * HEIGHT as usize * 4;
pub const PRESET_COLOR: [u8; 3] = [197, 92, 255];

/// Show a cached raster-engine preview and expose its active settings as an
/// accessible image label.
pub fn show(ui: &mut egui::Ui, texture: &egui::TextureHandle, brush: &Brush) -> egui::Response {
    show_with_max_width(ui, texture, brush, 300.0)
}

/// Smaller live sample for narrow floating panels, leaving the preset list in
/// view without reducing the preview below a useful 120-point width.
pub fn show_compact(
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,
    brush: &Brush,
) -> egui::Response {
    show_with_max_width(ui, texture, brush, 220.0)
}

fn show_with_max_width(
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,
    brush: &Brush,
    max_width: f32,
) -> egui::Response {
    let width = ui.available_width().min(max_width).clamp(120.0, 440.0);
    let size = egui::vec2(width, width * HEIGHT as f32 / WIDTH as f32);
    let label = format!(
        "Raster-engine preview of {} at {:.0} pixel size and {:.0}% opacity",
        brush_label(brush.kind),
        brush.size,
        brush.opacity * 100.0,
    );
    let response = ui.add(
        egui::Image::new((texture.id(), size))
            .fit_to_exact_size(size)
            .alt_text(label.clone()),
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label.clone()));
    response.on_hover_text(
        "Rendered with the current brush settings; this sample is temporary and is not saved to your artwork.",
    )
}

fn brush_label(kind: paint_document::brush::BrushKind) -> &'static str {
    match kind {
        paint_document::brush::BrushKind::Round => "Hard round",
        paint_document::brush::BrushKind::Ink => "Ink pen",
        paint_document::brush::BrushKind::Pencil => "Pencil",
        paint_document::brush::BrushKind::Marker => "Marker",
        paint_document::brush::BrushKind::Airbrush => "Airbrush",
        paint_document::brush::BrushKind::Soft => "Soft brush",
        paint_document::brush::BrushKind::Textured => "Textured brush",
        paint_document::brush::BrushKind::Calligraphy => "Calligraphy",
        paint_document::brush::BrushKind::Eraser => "Eraser",
        paint_document::brush::BrushKind::Wash => "Wash · experimental",
    }
}

/// Render a deterministic pressure-graded sample with the selected brush's
/// actual raster settings. Eraser previews cut a colored under-stroke so the
/// erasure is visible against the transparent UI surface.
pub fn render_rgba(brush: &Brush) -> Result<Vec<u8>, PaintError> {
    render_sample(brush, WIDTH, HEIGHT)
}

/// Render a bounded thumbnail for one built-in preset using its actual raster
/// brush behavior. The brand color is fixed so the gallery remains stable while
/// an artist changes the active artwork color.
pub fn render_preset_rgba(kind: paint_document::brush::BrushKind) -> Result<Vec<u8>, PaintError> {
    let mut brush = Brush::preset(kind);
    brush.color = PRESET_COLOR;
    render_sample(&brush, WIDTH, HEIGHT)
}

fn render_sample(brush: &Brush, width: u32, height: u32) -> Result<Vec<u8>, PaintError> {
    if !brush.validate() {
        return Err(PaintError::Invalid);
    }
    let Some(bytes) = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .filter(|bytes| {
            width > 0 && height > 0 && width <= 512 && height <= 256 && *bytes <= 512 * 1024
        })
    else {
        return Err(PaintError::Limit);
    };

    let mut state = State::new(width, height, 72)?;
    state.background = None;
    let mut engine = Engine::from_state(state)?;

    if brush.kind == paint_document::brush::BrushKind::Eraser {
        let mut under_stroke = Brush::preset(paint_document::brush::BrushKind::Round);
        under_stroke.color = [196, 91, 235];
        under_stroke.size = 18.0;
        under_stroke.pressure_size = false;
        preview_stroke(&mut engine, &under_stroke, StrokeId(1), 0, |t| {
            [
                width as f64 * (0.06 + 0.88 * t),
                height as f64 * (0.5 + 0.075 * (t * std::f64::consts::TAU).sin()),
                1.0,
            ]
        })?;
        preview_stroke(&mut engine, brush, StrokeId(2), 32, |t| {
            [width as f64 * (0.38 + 0.26 * t), height as f64 * 0.5, 1.0]
        })?;
    } else {
        preview_stroke(&mut engine, brush, StrokeId(1), 0, |t| {
            let pressure = 0.18 + 0.82 * (std::f64::consts::PI * t).sin().powi(2);
            [
                width as f64 * (0.04 + 0.92 * t),
                height as f64 * (0.5 + 0.145 * (t * std::f64::consts::TAU).sin()),
                pressure,
            ]
        })?;
    }

    let pixels = engine.rgba();
    if pixels.len() != bytes {
        return Err(PaintError::Limit);
    }
    Ok(pixels)
}

fn preview_stroke(
    engine: &mut Engine,
    brush: &Brush,
    id: StrokeId,
    start_time: u64,
    path: impl Fn(f64) -> [f64; 3],
) -> Result<(), PaintError> {
    engine.brush = brush.clone();
    const SAMPLES: u64 = 12;
    for index in 0..SAMPLES {
        let t = index as f64 / (SAMPLES - 1) as f64;
        let [x, y, pressure] = path(t);
        let time = start_time + index;
        let sample = StrokeSample {
            pointer_id: 1,
            source_counter: None,
            tool: paint_input::Tool::Pen,
            position_client_px: [x, y],
            position_document: DocumentPoint([x, y]),
            pressure: Some(pressure as f32),
            tilt_degrees: if brush.tilt {
                [Some(28.0), Some(-14.0)]
            } else {
                [None, None]
            },
            buttons: paint_input::Buttons::default(),
            source_time_ms: time,
            time_ms: time,
        };
        let command = if index == 0 {
            StrokeCommand::BeginStroke {
                stroke_id: id,
                sample,
            }
        } else if index + 1 == SAMPLES {
            StrokeCommand::CommitStroke {
                stroke_id: id,
                sample,
            }
        } else {
            StrokeCommand::UpdateStroke {
                stroke_id: id,
                sample,
            }
        };
        engine.apply(&command)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use paint_document::brush::BrushKind;

    #[test]
    fn preview_is_deterministic_and_uses_active_brush_settings() {
        let brush = Brush::preset(BrushKind::Ink);
        let original = render_rgba(&brush).unwrap();
        assert_eq!(original.len(), MAX_PREVIEW_BYTES);
        assert_eq!(original, render_rgba(&brush).unwrap());

        let mut changed = brush.clone();
        changed.size = 52.0;
        changed.color = [20, 205, 130];
        changed.opacity = 0.3;
        let changed = render_rgba(&changed).unwrap();
        assert_ne!(original, changed);
        assert!(changed.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }

    #[test]
    fn every_built_in_preset_renders_a_deterministic_bounded_sample() {
        let kinds = [
            BrushKind::Round,
            BrushKind::Ink,
            BrushKind::Pencil,
            BrushKind::Marker,
            BrushKind::Airbrush,
            BrushKind::Soft,
            BrushKind::Textured,
            BrushKind::Calligraphy,
            BrushKind::Eraser,
            BrushKind::Wash,
        ];
        let mut samples = Vec::with_capacity(kinds.len());
        for kind in kinds {
            let sample = render_preset_rgba(kind).unwrap();
            assert_eq!(sample.len(), MAX_PREVIEW_BYTES);
            assert!(
                sample.chunks_exact(4).any(|pixel| pixel[3] > 0),
                "{kind:?} preview is empty"
            );
            assert_eq!(
                sample,
                render_preset_rgba(kind).unwrap(),
                "{kind:?} preview is nondeterministic"
            );
            samples.push(sample);
        }
        for left in 0..samples.len() {
            for right in left + 1..samples.len() {
                assert_ne!(
                    samples[left], samples[right],
                    "preset samples {left} and {right} are identical"
                );
            }
        }
    }

    #[test]
    fn eraser_preview_shows_a_cut_in_an_underlying_sample() {
        let eraser = render_rgba(&Brush::preset(BrushKind::Eraser)).unwrap();
        let alpha_at = |x: u32, y: u32| eraser[((y * WIDTH + x) * 4 + 3) as usize];
        assert_eq!(
            alpha_at(160, 48),
            0,
            "the eraser pass should cut the sample"
        );
        assert!(alpha_at(48, 48) > 0, "the under-stroke remains visible");
        assert!(
            alpha_at(280, 48) > 0,
            "the un-erased endpoint remains visible"
        );
    }

    #[test]
    fn preview_rejects_invalid_brush_values_before_allocating_tiles() {
        let brush = Brush {
            size: f64::NAN,
            ..Brush::default()
        };
        assert!(matches!(render_rgba(&brush), Err(PaintError::Invalid)));
    }

    #[test]
    fn active_preview_has_an_accessible_label_with_current_settings() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let texture = context.load_texture(
            "test brush preview",
            egui::ColorImage::filled([8, 8], egui::Color32::from_rgb(110, 40, 180)),
            egui::TextureOptions::LINEAR,
        );
        let brush = Brush {
            size: 42.0,
            opacity: 0.5,
            ..Brush::preset(paint_document::brush::BrushKind::Ink)
        };
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(ui, &texture, &brush);
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("brush preview emits an accessibility update");
        let labels = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            labels.contains("Raster-engine preview of Ink pen at 42 pixel size and 50% opacity"),
            "active brush preview is missing its descriptive name: {labels}"
        );
    }
}
