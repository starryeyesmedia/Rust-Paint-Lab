//! Functional alpha controls. No widget is presented without an implemented action.
use crate::{
    gpu::{Failure, Renderer},
    interaction::{CanvasSession, MouseEvent},
    scene::CanvasMapping,
};
use paint_document::{
    Blend, Engine,
    brush::{Brush, BrushKind},
    editing::{
        GradientKind, GradientRamp, Selection, SelectionKind, ShapeKind, gradient_parameter,
        selection_row_spans,
    },
    recovery::Recovery,
    storage::{self, Store},
};
use std::{path::PathBuf, time::Instant};
use winit::window::Window;

struct ReferenceImage {
    layout: crate::workspace::ReferenceLayout,
    texture: egui::TextureHandle,
    source_size: [u32; 2],
    viewport: Option<egui::Rect>,
    touches: crate::navigation::TouchNavigation,
}

#[derive(Clone)]
struct RecentProjectCard {
    path: PathBuf,
    bytes: Option<u64>,
    dimensions: Option<(u32, u32, u32)>,
    opened_unix_seconds: u64,
    thumbnail: Option<egui::TextureHandle>,
}

struct BrushPreviewCache {
    brush: Brush,
    texture: egui::TextureHandle,
}

struct BrushPresetPreviewCache {
    kind: BrushKind,
    texture: egui::TextureHandle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct GradientPreviewKey {
    document: [u32; 2],
    kind: GradientKind,
    start: [u64; 2],
    end: [u64; 2],
    colors: [[u8; 3]; 2],
    opacity: u8,
    selection: u64,
}

fn gradient_preview_key(
    document: [u32; 2],
    kind: GradientKind,
    start: [f64; 2],
    end: [f64; 2],
    colors: [[u8; 3]; 2],
    opacity: u8,
    selection: Option<&Selection>,
) -> GradientPreviewKey {
    use std::hash::{Hash, Hasher};
    let selection = selection.map_or(0, |selection| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        selection.kind().hash(&mut hasher);
        for point in selection.points() {
            point[0].to_bits().hash(&mut hasher);
            point[1].to_bits().hash(&mut hasher);
        }
        hasher.finish()
    });
    GradientPreviewKey {
        document,
        kind,
        start: start.map(f64::to_bits),
        end: end.map(f64::to_bits),
        colors,
        opacity,
        selection,
    }
}

/// Generate a bounded, document-space color preview. It never allocates more
/// than 512×512 texels and uses scanline selection spans for lasso efficiency.
fn gradient_preview_image(
    document: [u32; 2],
    kind: GradientKind,
    start: [f64; 2],
    end: [f64; 2],
    colors: [[u8; 3]; 2],
    opacity: u8,
    selection: Option<&Selection>,
) -> egui::ColorImage {
    let longest = document[0].max(document[1]).max(1);
    let (width, height) = if longest <= 512 {
        (document[0], document[1])
    } else if document[0] >= document[1] {
        (
            512,
            ((u64::from(document[1]) * 512) / u64::from(document[0])).max(1) as u32,
        )
    } else {
        (
            ((u64::from(document[0]) * 512) / u64::from(document[1])).max(1) as u32,
            512,
        )
    };
    let ramp = GradientRamp::new(colors[0], colors[1]);
    let mut pixels = vec![egui::Color32::TRANSPARENT; (width * height) as usize];
    for y in 0..height {
        let doc_y = (f64::from(y) + 0.5) * f64::from(document[1]) / f64::from(height);
        let spans = selection_row_spans(selection, doc_y, document[0]);
        for x in 0..width {
            let doc_x = (f64::from(x) + 0.5) * f64::from(document[0]) / f64::from(width);
            let selected = spans
                .iter()
                .any(|(left, right)| doc_x >= f64::from(*left) && doc_x < f64::from(*right));
            if !selected {
                continue;
            }
            let color = ramp.sample(gradient_parameter(kind, start, end, [doc_x, doc_y]));
            pixels[(y * width + x) as usize] =
                egui::Color32::from_rgba_unmultiplied(color[0], color[1], color[2], opacity);
        }
    }
    egui::ColorImage::new([width as usize, height as usize], pixels)
}

type AppearanceKey = (
    crate::workspace::AppearanceMode,
    crate::workspace::AccentScheme,
    Option<[u8; 3]>,
    bool,
    bool,
    bool,
    u32,
);

fn compact_floating_panel_id(name: &str, compact: bool, layout_key: usize) -> egui::Id {
    if compact {
        egui::Id::new((name, "compact", layout_key))
    } else {
        egui::Id::new(name)
    }
}

fn reference_panel_id(path: &std::path::Path, compact: bool, layout_key: usize) -> egui::Id {
    if compact {
        egui::Id::new(("reference-board-compact", path.to_owned(), layout_key))
    } else {
        egui::Id::new(("reference-board", path.to_owned()))
    }
}

fn activate_brush_preset(brush: &mut Brush, kind: BrushKind) {
    let color = brush.color;
    *brush = Brush::preset(kind);
    brush.color = color;
}

fn client_point_to_egui(client_px: [f64; 2], pixels_per_point: f32) -> Option<egui::Pos2> {
    if !pixels_per_point.is_finite()
        || pixels_per_point <= 0.0
        || !client_px[0].is_finite()
        || !client_px[1].is_finite()
    {
        return None;
    }
    let point = egui::pos2(
        (client_px[0] / f64::from(pixels_per_point)) as f32,
        (client_px[1] / f64::from(pixels_per_point)) as f32,
    );
    (point.x.is_finite() && point.y.is_finite()).then_some(point)
}

fn recent_project_cards(
    recent: &paint_document::recent::Recent,
    context: &egui::Context,
    thumbnail_directory: Option<&std::path::Path>,
) -> Vec<RecentProjectCard> {
    recent
        .paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let metadata = std::fs::metadata(path)
                .ok()
                .filter(|metadata| metadata.is_file());
            RecentProjectCard {
                path: path.clone(),
                bytes: metadata.map(|metadata| metadata.len()),
                dimensions: storage::peek_dimensions(path),
                opened_unix_seconds: recent.opened_unix_seconds.get(index).copied().unwrap_or(0),
                thumbnail: thumbnail_directory
                    .and_then(|directory| crate::home_thumbnails::load(directory, path, context)),
            }
        })
        .collect()
}

fn recent_opened_label(timestamp: u64) -> String {
    if timestamp == 0 {
        return "Opened date unavailable".into();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let days = now.saturating_sub(timestamp) / 86_400;
    match days {
        0 => "Opened today".into(),
        1 => "Opened yesterday".into(),
        days => format!("Opened {days} days ago"),
    }
}

fn recent_project_matches(path: &std::path::Path, query: &str) -> bool {
    let query = query.trim();
    query.is_empty()
        || path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
            .contains(&query.to_lowercase())
}

fn brush_size_after_shortcut(current: f64, increase: bool, fine: bool) -> f64 {
    let current = if current.is_finite() {
        current.clamp(1.0, 256.0)
    } else {
        1.0
    };
    let factor = if fine { 1.01 } else { 1.1 };
    if increase {
        (current * factor).min(256.0)
    } else {
        (current / factor).max(1.0)
    }
}

fn recent_gallery_max_columns(available_width: f32) -> usize {
    match available_width {
        width if width >= 1500.0 => 4,
        width if width >= 1000.0 => 3,
        width if width >= 600.0 => 2,
        _ => 1,
    }
}

fn home_open_card_needed(project_count: usize, max_columns: usize, search_is_empty: bool) -> bool {
    search_is_empty && project_count > 0 && !project_count.is_multiple_of(max_columns.max(1))
}

fn recent_gallery_layout(available_width: f32, item_count: usize) -> (usize, f32) {
    let columns = recent_gallery_max_columns(available_width).min(item_count.max(1));
    let gaps = 14.0 * (columns - 1) as f32;
    // `card_width` is the content width. Leave space for the shared card's
    // 12-point inset on each side so the outer card stays inside the grid cell.
    // Without this allowance, four 500-point cards overflow a 1920-point Home
    // and the rightmost project is silently clipped from the screenshot/window.
    let card_width = (((available_width - gaps) / columns as f32) - 24.0).clamp(180.0, 476.0);
    (columns, card_width)
}

fn recent_artwork_preview_height(card_width: f32) -> f32 {
    (card_width * 0.48).clamp(128.0, 176.0)
}

fn document_format_grid(available_width: f32) -> (usize, f32) {
    let columns = match available_width {
        width if width >= 780.0 => 4,
        width if width >= 560.0 => 3,
        width if width >= 350.0 => 2,
        _ => 1,
    };
    let gaps = 12.0 * (columns - 1) as f32;
    let card_width = ((available_width - gaps) / columns as f32).clamp(108.0, 190.0);
    (columns, card_width)
}

fn document_format_card_height(viewport_height: f32) -> f32 {
    if viewport_height.is_finite() && viewport_height <= 560.0 {
        112.0
    } else {
        // Two rows of seven formats, the preview, and the primary action should
        // fit in a regular laptop dialog without forcing the artist to scroll.
        132.0
    }
}

pub(crate) fn quick_document_card_layout(available_width: f32) -> (usize, f32) {
    let columns = if available_width >= 1150.0 {
        5
    } else if available_width >= 760.0 {
        3
    } else if available_width >= 420.0 {
        2
    } else {
        1
    };
    let card_width =
        ((available_width - 12.0 * (columns - 1) as f32) / columns as f32).clamp(172.0, 410.0);
    (columns, card_width)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormatIllustration {
    Portrait,
    Landscape,
    Square,
    Print,
    Comic,
    Screen,
    Custom,
}

fn format_illustration(label: &str) -> FormatIllustration {
    match label {
        "Portrait" => FormatIllustration::Portrait,
        "Landscape" => FormatIllustration::Landscape,
        "Square" => FormatIllustration::Square,
        "A4 Print" => FormatIllustration::Print,
        "Comic Page" => FormatIllustration::Comic,
        "Full HD" => FormatIllustration::Screen,
        _ => FormatIllustration::Custom,
    }
}

fn mix_rgb(left: [u8; 3], right: [u8; 3], amount: f32) -> egui::Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    egui::Color32::from_rgb(
        channel(left[0], right[0]),
        channel(left[1], right[1]),
        channel(left[2], right[2]),
    )
}

fn format_point(paper: egui::Rect, x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(
        paper.left() + paper.width() * x,
        paper.top() + paper.height() * y,
    )
}

fn fit_paper_to_box(bounds: egui::Vec2, width: f32, height: f32) -> egui::Vec2 {
    let aspect = if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
        (width / height).clamp(0.05, 20.0)
    } else {
        1.0
    };
    if aspect >= bounds.x / bounds.y.max(1.0) {
        egui::vec2(bounds.x, bounds.x / aspect)
    } else {
        egui::vec2(bounds.y * aspect, bounds.y)
    }
}

fn format_polygon(
    painter: &egui::Painter,
    paper: egui::Rect,
    points: &[(f32, f32)],
    color: egui::Color32,
) {
    painter.add(egui::Shape::convex_polygon(
        points
            .iter()
            .map(|(x, y)| format_point(paper, *x, *y))
            .collect(),
        color,
        egui::Stroke::NONE,
    ));
}

/// Render an original, lightweight format illustration inside the paper
/// silhouette. These are schematic format cues, not artwork or project
/// thumbnails. Each supported preset gets a distinct miniature composition.
fn paint_format_illustration(
    painter: &egui::Painter,
    paper: egui::Rect,
    visuals: &egui::Visuals,
    label: &str,
) {
    let kind = format_illustration(label);
    let paper_fill = if matches!(kind, FormatIllustration::Print | FormatIllustration::Comic) {
        egui::Color32::from_rgb(0xF9, 0xF5, 0xEF)
    } else {
        egui::Color32::from_rgb(0x20, 0x18, 0x31)
    };
    painter.rect_filled(paper, 3.0, paper_fill);
    let art = painter.with_clip_rect(paper.shrink(1.0));
    let (top, bottom) = match kind {
        FormatIllustration::Portrait => ([0x53, 0x2B, 0x72], [0xF0, 0x8E, 0xBC]),
        FormatIllustration::Landscape => ([0x39, 0x55, 0x9C], [0xF1, 0xA3, 0xC1]),
        FormatIllustration::Square => ([0x4C, 0x2B, 0x80], [0xDB, 0x74, 0xBC]),
        FormatIllustration::Screen => ([0x17, 0x1A, 0x43], [0x66, 0x42, 0xA4]),
        FormatIllustration::Print | FormatIllustration::Comic => {
            ([0xFF, 0xF9, 0xF0], [0xF2, 0xE8, 0xE1])
        }
        FormatIllustration::Custom => ([0x42, 0x2E, 0x76], [0x76, 0x56, 0xB0]),
    };
    // Six thin bands give the tiny preview a soft depth without allocating
    // textures or adding another renderer dependency.
    for band in 0..6 {
        let y0 = paper.top() + paper.height() * band as f32 / 6.0;
        let y1 = paper.top() + paper.height() * (band + 1) as f32 / 6.0;
        let amount = band as f32 / 5.0;
        art.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(paper.left(), y0),
                egui::pos2(paper.right(), y1 + 0.4),
            ),
            0.0,
            mix_rgb(top, bottom, amount),
        );
    }

    let scale = paper.width().min(paper.height()).max(1.0);
    let violet = if visuals.dark_mode {
        egui::Color32::from_rgb(0xCE, 0x82, 0xFF)
    } else {
        egui::Color32::from_rgb(0x82, 0x3D, 0xC8)
    };
    let pink = egui::Color32::from_rgb(0xF5, 0x78, 0xC4);
    let teal = egui::Color32::from_rgb(0x68, 0xDF, 0xD2);
    let ink = egui::Color32::from_rgb(0x31, 0x20, 0x43);

    match kind {
        FormatIllustration::Portrait => {
            format_polygon(
                &art,
                paper,
                &[
                    (0.12, 1.0),
                    (0.2, 0.75),
                    (0.39, 0.62),
                    (0.67, 0.63),
                    (0.89, 0.82),
                    (0.94, 1.0),
                ],
                violet,
            );
            art.circle_filled(format_point(paper, 0.54, 0.43), scale * 0.2, ink);
            art.circle_filled(format_point(paper, 0.55, 0.42), scale * 0.15, pink);
            art.circle_filled(format_point(paper, 0.76, 0.24), scale * 0.07, teal);
            art.line_segment(
                [
                    format_point(paper, 0.26, 0.52),
                    format_point(paper, 0.38, 0.38),
                ],
                egui::Stroke::new((scale * 0.045).max(1.0), teal),
            );
        }
        FormatIllustration::Landscape => {
            art.circle_filled(format_point(paper, 0.77, 0.28), scale * 0.13, pink);
            format_polygon(
                &art,
                paper,
                &[
                    (0.0, 0.75),
                    (0.24, 0.43),
                    (0.4, 0.62),
                    (0.64, 0.34),
                    (1.0, 0.71),
                    (1.0, 1.0),
                    (0.0, 1.0),
                ],
                violet,
            );
            format_polygon(
                &art,
                paper,
                &[
                    (0.0, 0.84),
                    (0.31, 0.65),
                    (0.55, 0.8),
                    (0.82, 0.57),
                    (1.0, 0.72),
                    (1.0, 1.0),
                    (0.0, 1.0),
                ],
                teal,
            );
        }
        FormatIllustration::Square => {
            art.circle_filled(format_point(paper, 0.51, 0.51), scale * 0.3, pink);
            art.circle_filled(format_point(paper, 0.51, 0.51), scale * 0.2, violet);
            art.circle_filled(format_point(paper, 0.51, 0.51), scale * 0.1, teal);
            art.rect_filled(
                egui::Rect::from_min_max(
                    format_point(paper, 0.12, 0.77),
                    format_point(paper, 0.88, 0.86),
                ),
                2.0,
                ink,
            );
        }
        FormatIllustration::Print => {
            let margin = paper.shrink2(egui::vec2(paper.width() * 0.16, paper.height() * 0.14));
            art.rect_stroke(
                margin,
                0.0,
                egui::Stroke::new(0.8, egui::Color32::from_rgb(0xB8, 0xA9, 0xB5)),
                egui::StrokeKind::Inside,
            );
            art.circle_filled(format_point(margin, 0.73, 0.27), scale * 0.09, pink);
            format_polygon(
                &art,
                margin,
                &[
                    (0.05, 0.7),
                    (0.32, 0.43),
                    (0.5, 0.62),
                    (0.74, 0.38),
                    (0.96, 0.7),
                    (0.96, 0.82),
                    (0.05, 0.82),
                ],
                violet,
            );
            for line in 0..3 {
                let y = 0.9 + line as f32 * 0.035;
                art.line_segment(
                    [format_point(margin, 0.1, y), format_point(margin, 0.88, y)],
                    egui::Stroke::new(0.6, egui::Color32::from_rgb(0x8E, 0x7E, 0x8A)),
                );
            }
        }
        FormatIllustration::Comic => {
            let panels = [
                (0.08, 0.08, 0.47, 0.43, violet),
                (0.54, 0.08, 0.92, 0.43, teal),
                (0.08, 0.48, 0.92, 0.92, pink),
            ];
            for (left, top, right, bottom, color) in panels {
                let panel = egui::Rect::from_min_max(
                    format_point(paper, left, top),
                    format_point(paper, right, bottom),
                );
                art.rect_filled(panel.shrink(1.0), 1.0, color);
                art.rect_stroke(
                    panel,
                    1.0,
                    egui::Stroke::new(1.0, ink),
                    egui::StrokeKind::Inside,
                );
            }
            art.circle_filled(
                format_point(paper, 0.34, 0.27),
                scale * 0.1,
                egui::Color32::WHITE,
            );
            art.circle_filled(
                format_point(paper, 0.75, 0.27),
                scale * 0.1,
                egui::Color32::WHITE,
            );
            art.line_segment(
                [
                    format_point(paper, 0.18, 0.73),
                    format_point(paper, 0.42, 0.58),
                ],
                egui::Stroke::new(1.8, ink),
            );
            art.line_segment(
                [
                    format_point(paper, 0.42, 0.58),
                    format_point(paper, 0.68, 0.75),
                ],
                egui::Stroke::new(1.8, ink),
            );
        }
        FormatIllustration::Screen => {
            format_polygon(
                &art,
                paper,
                &[
                    (0.0, 0.7),
                    (0.25, 0.4),
                    (0.52, 0.72),
                    (0.78, 0.31),
                    (1.0, 0.62),
                    (1.0, 1.0),
                    (0.0, 1.0),
                ],
                teal,
            );
            art.circle_filled(format_point(paper, 0.22, 0.28), scale * 0.08, pink);
            let center = format_point(paper, 0.51, 0.53);
            art.circle_filled(center, scale * 0.19, egui::Color32::from_black_alpha(150));
            art.add(egui::Shape::convex_polygon(
                vec![
                    center + egui::vec2(-scale * 0.05, -scale * 0.08),
                    center + egui::vec2(scale * 0.1, 0.0),
                    center + egui::vec2(-scale * 0.05, scale * 0.08),
                ],
                egui::Color32::WHITE,
                egui::Stroke::NONE,
            ));
        }
        FormatIllustration::Custom => {
            art.circle_filled(format_point(paper, 0.72, 0.28), scale * 0.12, pink);
            format_polygon(
                &art,
                paper,
                &[
                    (0.0, 0.78),
                    (0.28, 0.48),
                    (0.47, 0.64),
                    (0.71, 0.43),
                    (1.0, 0.71),
                    (1.0, 1.0),
                    (0.0, 1.0),
                ],
                teal,
            );
            art.line_segment(
                [
                    format_point(paper, 0.12, 0.2),
                    format_point(paper, 0.43, 0.2),
                ],
                egui::Stroke::new((scale * 0.04).max(1.0), violet),
            );
        }
    }

    painter.rect_stroke(
        paper,
        3.0,
        egui::Stroke::new(1.0, visuals.window_stroke.color),
        egui::StrokeKind::Inside,
    );
}

fn home_document_card_with_rect(
    ui: &mut egui::Ui,
    name: &str,
    dimensions: [u32; 2],
    card_width: f32,
) -> (bool, egui::Rect) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(card_width, 132.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!(
                "New {name} artwork, {} by {} pixels",
                dimensions[0], dimensions[1]
            ),
        )
    });
    let visuals = ui.visuals();
    let fill = if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        visuals.faint_bg_color
    };
    ui.painter().rect_filled(rect, 12.0, fill);
    ui.painter().rect_stroke(
        rect,
        12.0,
        if response.has_focus() || response.hovered() {
            visuals.selection.stroke
        } else {
            visuals.window_stroke
        },
        egui::StrokeKind::Inside,
    );

    let max_paper = egui::vec2(
        (card_width * 0.22).clamp(54.0, 82.0),
        (card_width * 0.18).clamp(44.0, 66.0),
    );
    let paper_size = fit_paper_to_box(max_paper, dimensions[0] as f32, dimensions[1] as f32);
    let paper =
        egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 39.0), paper_size);
    paint_format_illustration(ui.painter(), paper, visuals, name);
    let fg = crate::appearance::readable_text_color(visuals.text_color(), fill);
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 28.0),
        egui::Align2::CENTER_CENTER,
        name,
        crate::ui_tokens::font_id(crate::ui_tokens::TypeRole::Button),
        fg,
    );
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 11.0),
        egui::Align2::CENTER_CENTER,
        format!("{} × {} px", dimensions[0], dimensions[1]),
        crate::ui_tokens::font_id_at(crate::ui_tokens::TypeRole::Metadata, 11.5),
        crate::appearance::readable_text_color(visuals.weak_text_color(), fill),
    );
    (
        response.clicked()
            || (response.has_focus()
                && ui.input(|input| {
                    input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::Space)
                })),
        rect,
    )
}

#[cfg(test)]
pub(crate) fn home_document_card(
    ui: &mut egui::Ui,
    name: &str,
    dimensions: [u32; 2],
    card_width: f32,
) -> bool {
    home_document_card_with_rect(ui, name, dimensions, card_width).0
}

pub(crate) fn home_custom_document_card(ui: &mut egui::Ui, card_width: f32) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(card_width, 132.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            "Create artwork with a custom canvas size",
        )
    });
    let visuals = ui.visuals();
    let fill = if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        visuals.faint_bg_color
    };
    ui.painter().rect_filled(rect, 12.0, fill);
    ui.painter().rect_stroke(
        rect,
        12.0,
        if response.has_focus() || response.hovered() {
            visuals.selection.stroke
        } else {
            visuals.window_stroke
        },
        egui::StrokeKind::Inside,
    );
    let plus_center = egui::pos2(rect.center().x, rect.top() + 37.0);
    ui.painter().line_segment(
        [
            plus_center - egui::vec2(11.0, 0.0),
            plus_center + egui::vec2(11.0, 0.0),
        ],
        egui::Stroke::new(2.5, visuals.hyperlink_color),
    );
    ui.painter().line_segment(
        [
            plus_center - egui::vec2(0.0, 11.0),
            plus_center + egui::vec2(0.0, 11.0),
        ],
        egui::Stroke::new(2.5, visuals.hyperlink_color),
    );
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 23.0),
        egui::Align2::CENTER_CENTER,
        "Custom size",
        crate::ui_tokens::font_id(crate::ui_tokens::TypeRole::Button),
        visuals.text_color(),
    );
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 8.0),
        egui::Align2::CENTER_CENTER,
        "Set exact dimensions",
        crate::ui_tokens::font_id_at(crate::ui_tokens::TypeRole::Metadata, 11.5),
        crate::appearance::readable_text_color(visuals.weak_text_color(), fill),
    );
    response.clicked()
        || (response.has_focus()
            && ui.input(|input| {
                input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::Space)
            }))
}

pub(crate) fn home_open_project_card(
    ui: &mut egui::Ui,
    content_width: f32,
    test_mode: bool,
) -> bool {
    let size = egui::vec2(content_width + 24.0, 314.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let visuals = ui.visuals();
    let fill = if response.hovered() {
        visuals.selection.bg_fill.gamma_multiply(0.34)
    } else {
        visuals.faint_bg_color
    };
    let stroke = if response.hovered() || response.has_focus() {
        visuals.selection.stroke
    } else {
        visuals.window_stroke
    };
    ui.painter().rect_filled(rect, 12.0, fill);
    ui.painter()
        .rect_stroke(rect, 12.0, stroke, egui::StrokeKind::Inside);

    let center = egui::pos2(rect.center().x, rect.top() + 96.0);
    let accent = visuals.hyperlink_color;
    ui.painter()
        .circle_filled(center, 32.0, accent.gamma_multiply(0.18));
    ui.painter().line_segment(
        [
            center - egui::vec2(12.0, 0.0),
            center + egui::vec2(12.0, 0.0),
        ],
        egui::Stroke::new(3.0, accent),
    );
    ui.painter().line_segment(
        [
            center - egui::vec2(0.0, 12.0),
            center + egui::vec2(0.0, 12.0),
        ],
        egui::Stroke::new(3.0, accent),
    );
    let foreground = crate::appearance::readable_text_color(visuals.text_color(), fill);
    let muted = crate::appearance::readable_text_color(visuals.weak_text_color(), fill);
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 65.0),
        egui::Align2::CENTER_CENTER,
        "Open another project",
        crate::ui_tokens::font_id(crate::ui_tokens::TypeRole::CardTitle),
        foreground,
    );
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 38.0),
        egui::Align2::CENTER_CENTER,
        "Browse saved artwork on this device",
        crate::ui_tokens::font_id(crate::ui_tokens::TypeRole::Metadata),
        muted,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            "Open another project from this device",
        )
    });
    if test_mode {
        crate::studio_log!(
            "UI_TEST_HOME_OPEN_CARD_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
            rect.left(),
            rect.top(),
            rect.width(),
            rect.height(),
            ui.ctx().pixels_per_point(),
        );
    }
    response.clicked()
        || (response.has_focus()
            && ui.input(|input| {
                input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::Space)
            }))
}

fn home_featured_artwork(
    ui: &mut egui::Ui,
    featured: Option<&RecentProjectCard>,
    open_recent: &mut Option<PathBuf>,
    screen_height: f32,
) -> bool {
    let Some(card) = featured.filter(|card| card.thumbnail.is_some()) else {
        return false;
    };
    let Some(texture) = card.thumbnail.as_ref() else {
        return false;
    };
    let width = ui.available_width().clamp(240.0, 640.0);
    let height = (width * 0.46)
        .clamp(150.0, 260.0)
        .min((screen_height * 0.36).clamp(120.0, 240.0));
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let visuals = ui.visuals();
    let size = texture.size_vec2();
    let available = rect.shrink(8.0);
    let scale = (available.width() / size.x).min(available.height() / size.y);
    let image_size = size * scale;
    let image = egui::Rect::from_center_size(rect.center(), image_size);
    ui.painter()
        .rect_filled(rect, 16.0, visuals.extreme_bg_color);
    ui.painter().image(
        texture.id(),
        image,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
    let caption = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 58.0), rect.max);
    ui.painter()
        .rect_filled(caption, 0.0, egui::Color32::from_black_alpha(210));
    let title = card.path.file_stem().unwrap_or_default().to_string_lossy();
    ui.painter().text(
        egui::pos2(caption.left() + 16.0, caption.top() + 18.0),
        egui::Align2::LEFT_CENTER,
        title.as_ref(),
        crate::ui_tokens::font_id(crate::ui_tokens::TypeRole::CardTitle),
        egui::Color32::WHITE,
    );
    ui.painter().text(
        egui::pos2(caption.left() + 16.0, caption.top() + 40.0),
        egui::Align2::LEFT_CENTER,
        "Continue creating",
        crate::ui_tokens::font_id(crate::ui_tokens::TypeRole::Metadata),
        egui::Color32::from_rgb(230, 218, 240),
    );
    if response.clicked() {
        *open_recent = Some(card.path.clone());
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!("Continue artwork {}", card.path.display()),
        )
    });
    let border = if response.hovered() || response.has_focus() {
        visuals.selection.stroke
    } else {
        visuals.window_stroke
    };
    ui.painter()
        .rect_stroke(rect, 16.0, border, egui::StrokeKind::Inside);
    true
}

fn home_sections_stacked(screen_width: f32) -> bool {
    !(1050.0..1700.0).contains(&screen_width)
}

fn home_hero_height(screen_width: f32) -> f32 {
    if screen_width < 760.0 { 280.0 } else { 340.0 }
}

pub(crate) fn recent_search_field(ui: &mut egui::Ui, query: &mut String) {
    let response = ui.add(
        egui::TextEdit::singleline(query)
            .desired_width(ui.available_width().min(420.0))
            .hint_text("Find recent artwork by filename…")
            .interactive(true),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::TextEdit,
            true,
            "Find recent artwork by filename",
        )
    });
    response.on_hover_text("Searches projects in this device's recent-artwork list.");
}

fn recovery_updated_label(timestamp: u64) -> String {
    if timestamp == 0 {
        return "Checkpoint time unavailable".into();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    match now.saturating_sub(timestamp) / 86_400 {
        0 => "Updated today".into(),
        1 => "Updated yesterday".into(),
        days => format!("Updated {days} days ago"),
    }
}

fn recovery_checkpoint_label(path: &std::path::Path, index: usize) -> String {
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let suffix = stem
        .strip_prefix("recovery-")
        .unwrap_or(stem)
        .trim_matches(['-', '_', ' ']);
    let readable: String = suffix
        .chars()
        .filter(|character| {
            character.is_ascii_alphabetic() || matches!(*character, ' ' | '-' | '_')
        })
        .take(48)
        .collect();
    if readable
        .chars()
        .any(|character| character.is_ascii_alphabetic())
    {
        let readable = readable.replace(['_', '-'], " ");
        let mut characters = readable.chars();
        return characters
            .next()
            .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
            .unwrap_or_else(|| "Artwork checkpoint".into());
    }
    if index == 0 {
        "Artwork checkpoint · Most recent".into()
    } else {
        format!("Artwork checkpoint · {}", index + 1)
    }
}

fn recovery_source_label(has_snapshot: bool, has_journal: bool, has_backup: bool) -> &'static str {
    if has_snapshot {
        "Snapshot available"
    } else if has_journal {
        "Journal checkpoint · validated on recovery"
    } else if has_backup {
        "Backup checkpoint · validated on recovery"
    } else {
        "Recovery data unavailable"
    }
}

pub(crate) fn activate_focusable_card(ui: &egui::Ui, response: &egui::Response) -> bool {
    response.clicked()
        || (response.has_focus()
            && ui.input(|input| {
                input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::Space)
            }))
}

#[cfg(test)]
pub(crate) fn document_format_card(
    ui: &mut egui::Ui,
    label: &str,
    width: f32,
    height: f32,
    dpi: u32,
    selected: bool,
    card_width: f32,
) -> bool {
    document_format_card_with_preview(ui, label, width, height, dpi, selected, card_width).0
}

fn document_format_card_with_preview(
    ui: &mut egui::Ui,
    label: &str,
    width: f32,
    height: f32,
    dpi: u32,
    selected: bool,
    card_width: f32,
) -> (bool, egui::Rect, egui::Rect) {
    let card_height = document_format_card_height(ui.ctx().content_rect().height());
    let size = egui::vec2(card_width, card_height);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let accessible_name = format!("{label}, {width:.0} by {height:.0} pixels at {dpi} DPI");
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::RadioButton,
            ui.is_enabled(),
            selected,
            accessible_name.clone(),
        )
    });
    let visuals = ui.visuals();
    let fill = if selected {
        visuals.selection.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        visuals.widgets.inactive.bg_fill
    };
    let outline = if selected || response.has_focus() {
        visuals.selection.stroke
    } else {
        visuals.widgets.inactive.bg_stroke
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 10.0, fill);
    painter.rect_stroke(rect, 10.0, outline, egui::StrokeKind::Inside);
    if selected {
        let center = egui::pos2(rect.right() - 15.0, rect.top() + 15.0);
        let badge = visuals.selection.stroke.color;
        let check = crate::appearance::readable_text_color(egui::Color32::WHITE, badge);
        painter.circle_filled(center, 9.0, badge);
        painter.line_segment(
            [
                center + egui::vec2(-4.0, 0.0),
                center + egui::vec2(-1.0, 3.0),
            ],
            egui::Stroke::new(2.0, check),
        );
        painter.line_segment(
            [
                center + egui::vec2(-1.0, 3.0),
                center + egui::vec2(4.0, -4.0),
            ],
            egui::Stroke::new(2.0, check),
        );
    }

    // A paper silhouette makes format choice legible at a glance even when
    // the label is translated or the dialog is narrow.
    let paper_max = egui::vec2(
        (card_width - 24.0).min(104.0),
        (card_height * 0.52).min(82.0),
    );
    let paper_size = fit_paper_to_box(paper_max, width, height);
    let paper = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + card_height * 0.30),
        paper_size,
    );
    paint_format_illustration(painter, paper, visuals, label);
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 35.0),
        egui::Align2::CENTER_CENTER,
        label,
        crate::ui_tokens::font_id(crate::ui_tokens::TypeRole::Button),
        crate::appearance::readable_text_color(visuals.text_color(), fill),
    );
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 20.0),
        egui::Align2::CENTER_CENTER,
        format!("{}×{} px", width as u32, height as u32),
        crate::ui_tokens::font_id_at(crate::ui_tokens::TypeRole::Metadata, 11.5),
        crate::appearance::readable_text_color(visuals.weak_text_color(), fill),
    );
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 6.0),
        egui::Align2::CENTER_CENTER,
        format!("{dpi} DPI"),
        crate::ui_tokens::font_id_at(crate::ui_tokens::TypeRole::Metadata, 11.5),
        crate::appearance::readable_text_color(visuals.weak_text_color(), fill),
    );
    (
        activate_focusable_card(ui, &response),
        paper.intersect(rect),
        rect,
    )
}

fn log_format_preview_rect(ui: &egui::Ui, label: &str, paper: egui::Rect) {
    crate::studio_log!(
        "UI_TEST_FORMAT_PREVIEW_RECT|{label}|{:.1} {:.1} {:.1} {:.1} {:.2}",
        paper.left(),
        paper.top(),
        paper.width(),
        paper.height(),
        ui.ctx().pixels_per_point(),
    );
}

fn log_format_card_rect(ui: &egui::Ui, label: &str, rect: egui::Rect) {
    crate::studio_log!(
        "UI_TEST_FORMAT_CARD_RECT|{label}|{:.1} {:.1} {:.1} {:.1} {:.2}",
        rect.left(),
        rect.top(),
        rect.width(),
        rect.height(),
        ui.ctx().pixels_per_point(),
    );
}

pub(crate) fn brush_preset_card_with_preview(
    ui: &mut egui::Ui,
    kind: BrushKind,
    selected: bool,
    color: [u8; 3],
    card_width: f32,
    card_height: f32,
    preview: Option<egui::TextureId>,
) -> (bool, egui::Rect) {
    let name = brush_display_name(kind);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(card_width.max(120.0), card_height.clamp(60.0, 82.0)),
        egui::Sense::click(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::RadioButton,
            ui.is_enabled(),
            selected,
            name,
        )
    });
    let visuals = ui.visuals();
    let fill = if selected {
        visuals.selection.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        visuals.widgets.inactive.bg_fill
    };
    let stroke = if selected || response.has_focus() {
        visuals.selection.stroke
    } else {
        visuals.widgets.inactive.bg_stroke
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 8.0, fill);
    painter.rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Inside);
    let line_color = if kind == BrushKind::Eraser {
        egui::Color32::from_rgb(239, 91, 124)
    } else {
        egui::Color32::from_rgb(color[0], color[1], color[2])
    };
    let y = rect.top() + 31.0;
    let left = rect.left() + 18.0;
    let right = rect.right() - 18.0;
    if let Some(texture) = preview {
        let sample_rect = egui::Rect::from_min_max(
            egui::pos2(left, rect.top() + 8.0),
            egui::pos2(right, rect.top() + 51.0),
        );
        painter.image(
            texture,
            sample_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        match kind {
            BrushKind::Soft | BrushKind::Airbrush => {
                for index in 0..5 {
                    let x = left + (right - left) * index as f32 / 4.0;
                    let radius = if kind == BrushKind::Soft { 5.5 } else { 4.0 };
                    let alpha = if kind == BrushKind::Soft { 95 } else { 52 };
                    painter.circle_filled(
                        egui::pos2(x, y),
                        radius,
                        egui::Color32::from_rgba_unmultiplied(
                            line_color.r(),
                            line_color.g(),
                            line_color.b(),
                            alpha,
                        ),
                    );
                }
            }
            BrushKind::Textured => {
                for index in 0..11 {
                    let x = left + (right - left) * index as f32 / 10.0;
                    if index % 2 == 0 {
                        painter.circle_filled(egui::pos2(x, y), 2.6, line_color);
                    }
                }
            }
            BrushKind::Wash | BrushKind::Marker => {
                painter.line_segment(
                    [egui::pos2(left, y), egui::pos2(right, y)],
                    egui::Stroke::new(
                        if kind == BrushKind::Wash { 5.0 } else { 7.0 },
                        egui::Color32::from_rgba_unmultiplied(
                            line_color.r(),
                            line_color.g(),
                            line_color.b(),
                            150,
                        ),
                    ),
                );
            }
            _ => {
                let steps = 8;
                let points: Vec<_> = (0..=steps)
                    .map(|index| {
                        let t = index as f32 / steps as f32;
                        egui::pos2(left + (right - left) * t, y + (t - 0.5).powi(2) * 8.0 - 2.0)
                    })
                    .collect();
                for (index, segment) in points.windows(2).enumerate() {
                    let weight = index.min(steps - 1 - index) as f32 / (steps as f32 * 0.5);
                    let width = match kind {
                        BrushKind::Calligraphy => 1.2 + weight * 6.0,
                        BrushKind::Pencil => 0.8 + weight * 1.0,
                        BrushKind::Ink => 1.2 + weight * 3.5,
                        _ => 1.8 + weight * 2.5,
                    };
                    painter.line_segment(
                        [segment[0], segment[1]],
                        egui::Stroke::new(width, line_color),
                    );
                }
                if kind == BrushKind::Eraser {
                    painter.circle_stroke(
                        egui::pos2((left + right) * 0.5, y),
                        6.0,
                        egui::Stroke::new(1.5, visuals.text_color()),
                    );
                }
            }
        }
    }
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 13.0),
        egui::Align2::CENTER_CENTER,
        name,
        crate::ui_tokens::font_id(crate::ui_tokens::TypeRole::ToolName),
        visuals.text_color(),
    );
    if selected {
        let center = egui::pos2(rect.right() - 11.0, rect.top() + 11.0);
        painter.circle_filled(
            center,
            5.0,
            if visuals.override_text_color.is_some() {
                visuals.selection.stroke.color
            } else {
                visuals.hyperlink_color
            },
        );
        painter.circle_stroke(center, 7.0, egui::Stroke::new(1.0, visuals.window_fill));
    }
    (activate_focusable_card(ui, &response), rect)
}

fn active_brush_preview(
    ui: &mut egui::Ui,
    brush: &Brush,
    texture: &egui::TextureHandle,
    compact: bool,
) -> egui::Response {
    ui.horizontal(|ui| {
        let foreground = crate::appearance::readable_text_color(
            ui.visuals().text_color(),
            ui.visuals().window_fill,
        );
        ui.label(crate::ui_tokens::type_text(
            "Current brush",
            crate::ui_tokens::TypeRole::ToolName,
            foreground,
        ));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let metadata = crate::appearance::readable_text_color(
                ui.visuals().weak_text_color(),
                ui.visuals().window_fill,
            );
            ui.add_sized(
                egui::vec2(
                    ui.available_width(),
                    crate::ui_tokens::font_id(crate::ui_tokens::TypeRole::Metadata).size * 1.6,
                ),
                egui::Label::new(crate::ui_tokens::type_text(
                    format!(
                        "{:.0} px · {:.0}% opacity",
                        brush.size,
                        brush.opacity * 100.0
                    ),
                    crate::ui_tokens::TypeRole::Metadata,
                    metadata,
                ))
                .truncate(),
            );
        });
    });
    let response = if compact {
        crate::brush_preview::show_compact(ui, texture, brush)
    } else {
        crate::brush_preview::show(ui, texture, brush)
    };
    ui.add_space(6.0);
    response
}

fn color_swatch(ui: &mut egui::Ui, color: [u8; 3], label: &str, selected: bool, size: f32) -> bool {
    color_swatch_response(ui, color, label, selected, size).clicked()
}

pub(crate) fn color_swatch_response(
    ui: &mut egui::Ui,
    color: [u8; 3],
    label: &str,
    selected: bool,
    size: f32,
) -> egui::Response {
    let color32 = egui::Color32::from_rgb(color[0], color[1], color[2]);
    let response = ui.add_sized(
        [size, size],
        egui::Button::new(
            egui::RichText::new(" ")
                .size(1.0)
                .color(egui::Color32::TRANSPARENT),
        )
        .fill(color32),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let outline = if selected || response.has_focus() {
        ui.visuals().selection.stroke
    } else {
        ui.visuals().widgets.inactive.bg_stroke
    };
    ui.painter().rect_stroke(
        response.rect.shrink(1.0),
        6.0,
        outline,
        egui::StrokeKind::Inside,
    );
    response.on_hover_text(label)
}

fn saved_color_palette_contents(
    ui: &mut egui::Ui,
    current_color: &mut [u8; 3],
    swatches: &mut Vec<[u8; 3]>,
    test_mode: bool,
    ready_logged: &mut bool,
    workspace_changed: &mut bool,
    status: &mut String,
) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.strong("Saved palette");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let add = ui.button("+ Save current");
            add.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    "Save current brush color to the workspace palette",
                )
            });
            if test_mode {
                crate::studio_log!(
                    "UI_TEST_COLOR_PALETTE_ADD_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                    add.rect.left(),
                    add.rect.top(),
                    add.rect.width(),
                    add.rect.height(),
                    ui.ctx().pixels_per_point(),
                );
            }
            if add.clicked() {
                if test_mode {
                    crate::studio_log!("UI_TEST_COLOR_PALETTE_ADD_CLICKED");
                }
                if swatches.contains(current_color) {
                    *status = "That color is already in the saved palette".into();
                } else if crate::workspace::add_saved_color_swatch(swatches, *current_color) {
                    *status = "Color saved to workspace palette".into();
                    *workspace_changed = true;
                } else {
                    *status = format!(
                        "Saved palette is limited to {} colors",
                        crate::workspace::MAX_SAVED_COLOR_SWATCHES,
                    );
                }
            }
        });
    });
    if test_mode && !*ready_logged {
        *ready_logged = true;
        crate::studio_log!("UI_TEST_COLOR_PALETTE_READY: {}", swatches.len());
    }
    if swatches.is_empty() {
        ui.weak("Save colors here for reuse across projects.");
        return;
    }

    let mut remove_swatch = None;
    ui.horizontal_wrapped(|ui| {
        for (index, color) in swatches.iter().copied().enumerate() {
            ui.horizontal(|ui| {
                let hex = crate::color_controls::format_hex_rgb(color);
                let swatch = color_swatch_response(
                    ui,
                    color,
                    &format!("Use saved color {hex}"),
                    *current_color == color,
                    38.0,
                );
                if index == 0 && test_mode {
                    crate::studio_log!(
                        "UI_TEST_COLOR_SWATCH_RECT: {:.1} {:.1} {:.1} {:.1} {:.2} {} {} {}",
                        swatch.rect.left(),
                        swatch.rect.top(),
                        swatch.rect.width(),
                        swatch.rect.height(),
                        ui.ctx().pixels_per_point(),
                        color[0],
                        color[1],
                        color[2],
                    );
                }
                if test_mode && color == [35, 65, 180] {
                    crate::studio_log!(
                        "UI_TEST_COLOR_ORIGINAL_SWATCH_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                        swatch.rect.left(),
                        swatch.rect.top(),
                        swatch.rect.width(),
                        swatch.rect.height(),
                        ui.ctx().pixels_per_point(),
                    );
                }
                if swatch.clicked() {
                    *current_color = color;
                    if test_mode {
                        crate::studio_log!("UI_TEST_COLOR_SWATCH_APPLIED: {}", hex);
                    }
                }
                let remove = ui.add_sized([32.0, 38.0], egui::Button::new("×"));
                let remove_label = format!("Remove saved color {hex}");
                remove.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        &remove_label,
                    )
                });
                let remove = remove.on_hover_text(&remove_label);
                if index == 0 && test_mode {
                    crate::studio_log!(
                        "UI_TEST_COLOR_PALETTE_REMOVE_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                        remove.rect.left(),
                        remove.rect.top(),
                        remove.rect.width(),
                        remove.rect.height(),
                        ui.ctx().pixels_per_point(),
                    );
                }
                if remove.clicked() {
                    remove_swatch = Some(index);
                    if test_mode {
                        crate::studio_log!("UI_TEST_COLOR_SWATCH_REMOVAL_REQUESTED: {hex}");
                    }
                }
            });
        }
    });
    if let Some(index) = remove_swatch
        && let Some(removed) = crate::workspace::remove_saved_color_swatch(swatches, index)
    {
        *status = format!(
            "Removed {} from saved palette",
            crate::color_controls::format_hex_rgb(removed),
        );
        *workspace_changed = true;
    }
}

pub(crate) fn saturation_value_field(
    ui: &mut egui::Ui,
    hue: f32,
    saturation: f32,
    value: f32,
) -> Option<[f32; 2]> {
    const COLUMNS: usize = 24;
    const ROWS: usize = 14;
    let width = ui.available_width().clamp(1.0, 224.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, 132.0), egui::Sense::click_and_drag());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            "Saturation and brightness color field",
        )
    });
    let painter = ui.painter_at(rect);
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let left = rect.left() + rect.width() * column as f32 / COLUMNS as f32;
            let right = rect.left() + rect.width() * (column + 1) as f32 / COLUMNS as f32;
            let top = rect.top() + rect.height() * row as f32 / ROWS as f32;
            let bottom = rect.top() + rect.height() * (row + 1) as f32 / ROWS as f32;
            let s = (column as f32 + 0.5) / COLUMNS as f32;
            let v = 1.0 - (row as f32 + 0.5) / ROWS as f32;
            let color = crate::color_controls::hsv_to_rgb([hue, s, v]);
            painter.rect_filled(
                egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom)),
                0.0,
                egui::Color32::from_rgb(color[0], color[1], color[2]),
            );
        }
    }
    painter.rect_stroke(
        rect,
        6.0,
        ui.visuals().window_stroke,
        egui::StrokeKind::Inside,
    );
    let marker = egui::pos2(
        rect.left() + rect.width() * saturation.clamp(0.0, 1.0),
        rect.bottom() - rect.height() * value.clamp(0.0, 1.0),
    );
    painter.circle_stroke(marker, 5.0, egui::Stroke::new(2.5, egui::Color32::BLACK));
    painter.circle_stroke(marker, 3.5, egui::Stroke::new(1.5, egui::Color32::WHITE));
    if (response.clicked() || response.dragged())
        && let Some(position) = response.interact_pointer_pos()
    {
        return crate::color_controls::saturation_value_at(rect, position);
    }
    None
}

pub(crate) fn hue_selection_wheel(
    ui: &mut egui::Ui,
    hue: f32,
    saturation: f32,
    value: f32,
) -> (Option<f32>, egui::Response) {
    const SEGMENTS: usize = 96;
    let diameter = ui.available_width().clamp(120.0, 152.0);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(diameter, diameter),
        egui::Sense::click_and_drag(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Slider,
            ui.is_enabled(),
            format!(
                "Hue selection wheel, {:.0} degrees",
                hue.rem_euclid(1.0) * 360.0
            ),
        )
    });

    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5 - 5.0;
    let inner_radius = radius * 0.72;
    let painter = ui.painter_at(rect);
    for index in 0..SEGMENTS {
        let start =
            index as f32 / SEGMENTS as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let end = (index + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU
            - std::f32::consts::FRAC_PI_2;
        let point = |distance: f32, angle: f32| {
            center + egui::vec2(angle.cos() * distance, angle.sin() * distance)
        };
        let rgb =
            crate::color_controls::hsv_to_rgb([(index as f32 + 0.5) / SEGMENTS as f32, 1.0, 1.0]);
        painter.add(egui::Shape::convex_polygon(
            vec![
                point(inner_radius, start),
                point(radius, start),
                point(radius, end),
                point(inner_radius, end),
            ],
            egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
            egui::Stroke::NONE,
        ));
    }

    let clean_hue = if hue.is_finite() {
        hue.rem_euclid(1.0)
    } else {
        0.0
    };
    let selected_color = crate::color_controls::hsv_to_rgb([
        clean_hue,
        saturation.clamp(0.0, 1.0),
        value.clamp(0.0, 1.0),
    ]);
    let sample_fill =
        egui::Color32::from_rgb(selected_color[0], selected_color[1], selected_color[2]);
    painter.circle_filled(center, inner_radius - 2.0, sample_fill);
    painter.circle_stroke(center, inner_radius - 2.0, ui.visuals().window_stroke);
    let sample_text =
        crate::appearance::readable_text_color(ui.visuals().text_color(), sample_fill);
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        format!("{:.0}°", clean_hue * 360.0),
        crate::ui_tokens::font_id(crate::ui_tokens::TypeRole::Numeric),
        sample_text,
    );

    let marker_angle = clean_hue * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    let marker = center
        + egui::vec2(
            marker_angle.cos() * (radius + inner_radius) * 0.5,
            marker_angle.sin() * (radius + inner_radius) * 0.5,
        );
    painter.circle_filled(marker, 6.5, egui::Color32::WHITE);
    painter.circle_stroke(marker, 6.5, egui::Stroke::new(1.5, egui::Color32::BLACK));
    let response = response.on_hover_text(
        "Choose a hue by clicking or dragging around the ring; use the Hue slider for precise keyboard adjustment.",
    );

    let selected = if response.clicked() || response.dragged() {
        response
            .interact_pointer_pos()
            .and_then(|position| crate::color_controls::hue_wheel_value_at(rect, position))
    } else {
        None
    };
    (selected, response)
}

const MAX_REFERENCE_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REFERENCE_SOURCE_EDGE: u32 = 16_384;
const MAX_REFERENCE_DECODE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_REFERENCE_PREVIEW_EDGE: u32 = 1_448;
const MAX_REFERENCE_TEXTURE_BYTES: usize = 8 * 1024 * 1024;

fn load_reference_texture(
    context: &egui::Context,
    layout: crate::workspace::ReferenceLayout,
) -> Result<ReferenceImage, String> {
    let metadata = std::fs::metadata(&layout.path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_REFERENCE_FILE_BYTES {
        return Err(
            "reference file exceeds 32 MiB; resize or export a smaller JPEG/PNG and try again"
                .into(),
        );
    }
    let mut reader = image::ImageReader::open(&layout.path)
        .and_then(|reader| reader.with_guessed_format())
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_REFERENCE_SOURCE_EDGE);
    limits.max_image_height = Some(MAX_REFERENCE_SOURCE_EDGE);
    limits.max_alloc = Some(MAX_REFERENCE_DECODE_BYTES);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|error| match error {
        image::ImageError::Limits(_) => format!(
            "Reference exceeds safe decode limits ({} px per edge / {} MiB source memory); resize it or export a smaller JPEG/PNG",
            MAX_REFERENCE_SOURCE_EDGE,
            MAX_REFERENCE_DECODE_BYTES / (1024 * 1024)
        ),
        error => error.to_string(),
    })?;
    let source_size = [decoded.width(), decoded.height()];
    if source_size.contains(&0) {
        return Err("Reference image has an empty dimension".into());
    }
    let preview = if source_size[0] > MAX_REFERENCE_PREVIEW_EDGE
        || source_size[1] > MAX_REFERENCE_PREVIEW_EDGE
    {
        decoded.thumbnail(MAX_REFERENCE_PREVIEW_EDGE, MAX_REFERENCE_PREVIEW_EDGE)
    } else {
        decoded
    };
    let rgba = preview.into_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let decoded_bytes = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "Reference preview dimensions overflow the safe limit".to_string())?;
    if decoded_bytes > MAX_REFERENCE_TEXTURE_BYTES {
        return Err("Reference preview exceeds the 8 MiB GPU texture budget".into());
    }
    let image = egui::ColorImage::from_rgba_unmultiplied([width, height], rgba.as_raw());
    let name = layout
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("reference image");
    let texture = context.load_texture(name, image, egui::TextureOptions::LINEAR);
    Ok(ReferenceImage {
        layout,
        texture,
        source_size,
        viewport: None,
        touches: Default::default(),
    })
}

fn add_reference_path(
    context: &egui::Context,
    references: &mut Vec<ReferenceImage>,
    path: PathBuf,
) -> Result<(), String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "reference image".into());
    if references.len() >= crate::workspace::MAX_REFERENCES {
        return Err(format!(
            "{name}: the reference board is full ({} images maximum)",
            crate::workspace::MAX_REFERENCES
        ));
    }
    if references
        .iter()
        .any(|reference| reference.layout.path == path)
    {
        return Err(format!(
            "{name}: this image is already on the reference board"
        ));
    }
    let index = references.len();
    let layout = crate::workspace::ReferenceLayout {
        path,
        position: [40.0 + index as f32 * 28.0, 80.0 + index as f32 * 24.0],
        size: [360.0, 300.0],
        opacity: 0.72,
        view_zoom: 1.0,
        view_pan: [0.0, 0.0],
        position_locked: false,
    };
    load_reference_texture(context, layout)
        .map(|reference| references.push(reference))
        .map_err(|error| format!("{name}: {error}"))
}

fn reference_contains_client(
    layout: &crate::workspace::ReferenceLayout,
    client_px: [f64; 2],
    pixels_per_point: f32,
) -> bool {
    if !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
        return false;
    }
    let point = [
        client_px[0] / f64::from(pixels_per_point),
        client_px[1] / f64::from(pixels_per_point),
    ];
    point.iter().all(|value| value.is_finite())
        && layout.position.iter().all(|value| value.is_finite())
        && layout
            .size
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
        && point[0] >= f64::from(layout.position[0])
        && point[1] >= f64::from(layout.position[1])
        && point[0] <= f64::from(layout.position[0] + layout.size[0])
        && point[1] <= f64::from(layout.position[1] + layout.size[1])
}

fn reference_image_rect(
    viewport: egui::Rect,
    source_size: egui::Vec2,
    zoom: f32,
    pan: [f32; 2],
) -> Option<egui::Rect> {
    if !viewport.is_finite()
        || viewport.width() <= 0.0
        || viewport.height() <= 0.0
        || !source_size.is_finite()
        || source_size.x <= 0.0
        || source_size.y <= 0.0
        || !zoom.is_finite()
        || !pan.iter().all(|value| value.is_finite())
    {
        return None;
    }
    let fit = (viewport.width() / source_size.x).min(viewport.height() / source_size.y);
    let size = source_size
        * fit
        * zoom.clamp(
            crate::workspace::MIN_REFERENCE_ZOOM,
            crate::workspace::MAX_REFERENCE_ZOOM,
        );
    let center = viewport.center()
        + egui::vec2(
            pan[0].clamp(-10000.0, 10000.0),
            pan[1].clamp(-10000.0, 10000.0),
        );
    Some(egui::Rect::from_center_size(center, size))
}

fn clamp_reference_position(
    position: [f32; 2],
    size: [f32; 2],
    viewport: egui::Rect,
) -> egui::Pos2 {
    let max_x = (viewport.max.x - size[0].min(viewport.width())).max(viewport.min.x);
    let max_y = (viewport.max.y - size[1].min(viewport.height())).max(viewport.min.y);
    egui::pos2(
        position[0].clamp(viewport.min.x, max_x),
        position[1].clamp(viewport.min.y, max_y),
    )
}

fn centered_reference_position(size: [f32; 2], viewport: egui::Rect) -> egui::Pos2 {
    clamp_reference_position(
        [
            viewport.center().x - size[0] * 0.5,
            viewport.center().y - size[1] * 0.5,
        ],
        size,
        viewport,
    )
}

fn default_reference_size(viewport: egui::Rect) -> [f32; 2] {
    [
        (viewport.width() - 32.0).clamp(1.0, 360.0),
        (viewport.height() - 96.0).clamp(1.0, 300.0),
    ]
}

/// Choose a visible corner for a newly imported reference image. Favor the
/// side away from the primary palette, then minimize overlap with existing
/// boards. This keeps the first image off the artist's controls and cascades
/// later images into unused corners instead of stacking them exactly.
fn default_reference_position(
    viewport: egui::Rect,
    size: [f32; 2],
    palette: Option<egui::Rect>,
    occupied: &[egui::Rect],
) -> egui::Pos2 {
    let margin = 16.0;
    let left = viewport.left() + margin;
    let right = viewport.right() - size[0] - margin;
    let top = viewport.top() + margin;
    let bottom = viewport.bottom() - size[1] - margin;
    let prefer_right = palette
        .map(|rect| rect.center().x < viewport.center().x)
        .unwrap_or(true);
    let corners = if prefer_right {
        [[right, top], [right, bottom], [left, top], [left, bottom]]
    } else {
        [[left, top], [left, bottom], [right, top], [right, bottom]]
    };
    let area = |rect: egui::Rect| rect.width().max(0.0) * rect.height().max(0.0);
    let overlap = |a: egui::Rect, b: egui::Rect| area(a.intersect(b));
    corners
        .into_iter()
        .map(|position| clamp_reference_position(position, size, viewport))
        .min_by(|left_pos, right_pos| {
            let score = |position: egui::Pos2| {
                let rect = egui::Rect::from_min_size(position, egui::vec2(size[0], size[1]));
                let palette_overlap = palette.map(|other| overlap(rect, other)).unwrap_or(0.0);
                // Palette overlap is more disruptive than board overlap, while
                // a lighter board penalty still spreads multi-image imports.
                palette_overlap * 4.0
                    + occupied
                        .iter()
                        .map(|other| overlap(rect, *other) * 1.5)
                        .sum::<f32>()
            };
            score(*left_pos).total_cmp(&score(*right_pos))
        })
        .unwrap_or_else(|| clamp_reference_position([left, top], size, viewport))
}

fn reference_zoom_about(
    layout: &mut crate::workspace::ReferenceLayout,
    viewport: egui::Rect,
    from: egui::Pos2,
    to: egui::Pos2,
    ratio: f32,
) -> bool {
    if !ratio.is_finite() || ratio <= 0.0 || !from.is_finite() || !to.is_finite() {
        return false;
    }
    let old_zoom = layout.view_zoom;
    let next_zoom = (old_zoom * ratio).clamp(
        crate::workspace::MIN_REFERENCE_ZOOM,
        crate::workspace::MAX_REFERENCE_ZOOM,
    );
    if !old_zoom.is_finite() || old_zoom <= 0.0 {
        return false;
    }
    let ratio = next_zoom / old_zoom;
    let origin = viewport.center() + egui::vec2(layout.view_pan[0], layout.view_pan[1]);
    let next_origin = to - (from - origin) * ratio;
    let next_pan = next_origin - viewport.center();
    if !next_pan.is_finite() {
        return false;
    }
    let changed = layout.view_zoom != next_zoom || layout.view_pan != [next_pan.x, next_pan.y];
    layout.view_zoom = next_zoom;
    layout.view_pan = [
        next_pan.x.clamp(-10000.0, 10000.0),
        next_pan.y.clamp(-10000.0, 10000.0),
    ];
    changed
}

#[cfg(test)]
mod reference_tests {
    use super::*;

    fn overlaps(left: egui::Rect, right: egui::Rect) -> bool {
        let intersection = left.intersect(right);
        intersection.width() > 0.0 && intersection.height() > 0.0
    }

    #[test]
    fn new_reference_boards_avoid_palette_and_stagger_around_existing_boards() {
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0));
        let size = default_reference_size(viewport);
        assert_eq!(size, [360.0, 300.0]);
        let left_palette =
            egui::Rect::from_min_size(egui::pos2(8.0, 48.0), egui::vec2(210.0, 700.0));
        let first = default_reference_position(viewport, size, Some(left_palette), &[]);
        let first_rect = egui::Rect::from_min_size(first, egui::vec2(size[0], size[1]));
        assert!(first.x > viewport.center().x);
        assert!(!overlaps(first_rect, left_palette));

        let second = default_reference_position(viewport, size, Some(left_palette), &[first_rect]);
        let second_rect = egui::Rect::from_min_size(second, egui::vec2(size[0], size[1]));
        assert_ne!(first_rect, second_rect);
        assert!(!overlaps(first_rect, second_rect));

        let right_palette =
            egui::Rect::from_min_size(egui::pos2(1060.0, 48.0), egui::vec2(210.0, 700.0));
        let left_side = default_reference_position(viewport, size, Some(right_palette), &[]);
        assert!(left_side.x < viewport.center().x);
        assert!(!overlaps(
            egui::Rect::from_min_size(left_side, egui::vec2(size[0], size[1])),
            right_palette
        ));

        let top_palette =
            egui::Rect::from_min_size(egui::pos2(300.0, 90.0), egui::vec2(680.0, 120.0));
        let below_top_palette = default_reference_position(viewport, size, Some(top_palette), &[]);
        assert!(!overlaps(
            egui::Rect::from_min_size(below_top_palette, egui::vec2(size[0], size[1])),
            top_palette
        ));
    }

    #[test]
    fn new_reference_board_fits_a_narrow_window_without_covering_left_palette() {
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0));
        let size = default_reference_size(viewport);
        let palette = egui::Rect::from_min_size(egui::pos2(8.0, 32.0), egui::vec2(210.0, 440.0));
        let position = default_reference_position(viewport, size, Some(palette), &[]);
        let board = egui::Rect::from_min_size(position, egui::vec2(size[0], size[1]));
        assert!(viewport.contains_rect(board));
        assert!(!overlaps(board, palette));
    }

    #[test]
    fn reference_window_bounds_block_canvas_input_in_physical_pixels() {
        let layout = crate::workspace::ReferenceLayout {
            path: PathBuf::from("reference.png"),
            position: [10.0, 20.0],
            size: [100.0, 80.0],
            opacity: 0.7,
            view_zoom: 1.0,
            view_pan: [0.0, 0.0],
            position_locked: false,
        };
        assert!(reference_contains_client(&layout, [220.0, 100.0], 2.0));
        assert!(!reference_contains_client(&layout, [19.0, 100.0], 2.0));
        assert!(!reference_contains_client(&layout, [0.0, 0.0], 0.0));
        assert!(!reference_contains_client(&layout, [f64::NAN, 0.0], 1.0));
    }

    #[test]
    fn reference_image_zoom_and_pan_map_to_bounded_viewport_geometry() {
        let viewport = egui::Rect::from_min_size(egui::pos2(20.0, 30.0), egui::vec2(200.0, 100.0));
        let fitted =
            reference_image_rect(viewport, egui::vec2(100.0, 50.0), 1.0, [0.0, 0.0]).unwrap();
        assert_eq!(fitted, viewport);
        let zoomed =
            reference_image_rect(viewport, egui::vec2(100.0, 50.0), 2.0, [10.0, -5.0]).unwrap();
        assert_eq!(zoomed.size(), egui::vec2(400.0, 200.0));
        assert_eq!(zoomed.center(), viewport.center() + egui::vec2(10.0, -5.0));
        assert!(reference_image_rect(viewport, egui::Vec2::ZERO, 1.0, [0.0, 0.0]).is_none());
        assert!(
            reference_image_rect(viewport, egui::vec2(1.0, 1.0), f32::NAN, [0.0, 0.0]).is_none()
        );
    }

    #[test]
    fn offscreen_reference_panels_are_clamped_to_the_current_viewport() {
        let viewport = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1000.0, 700.0));
        assert_eq!(
            clamp_reference_position([50_000.0, -50_000.0], [360.0, 300.0], viewport),
            egui::pos2(640.0, 0.0)
        );
        assert_eq!(
            clamp_reference_position([-50_000.0, 50_000.0], [1200.0, 900.0], viewport),
            egui::Pos2::ZERO
        );
        assert_eq!(
            centered_reference_position([360.0, 300.0], viewport),
            egui::pos2(320.0, 200.0)
        );
    }

    #[test]
    fn reference_gesture_keeps_the_focal_artwork_point_under_the_fingers() {
        let viewport = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(200.0, 100.0));
        let mut layout = crate::workspace::ReferenceLayout {
            path: PathBuf::from("reference.png"),
            position: [0.0, 0.0],
            size: [360.0, 300.0],
            opacity: 0.7,
            view_zoom: 1.0,
            view_pan: [0.0, 0.0],
            position_locked: false,
        };
        let from = egui::pos2(130.0, 70.0);
        let to = egui::pos2(140.0, 75.0);
        assert!(reference_zoom_about(&mut layout, viewport, from, to, 2.0));
        let new_origin = viewport.center() + egui::vec2(layout.view_pan[0], layout.view_pan[1]);
        let old_relative = from - viewport.center();
        assert!((new_origin.x + old_relative.x * layout.view_zoom - to.x).abs() < 0.001);
        assert!((new_origin.y + old_relative.y * layout.view_zoom - to.y).abs() < 0.001);
        assert!(!reference_zoom_about(
            &mut layout,
            viewport,
            from,
            to,
            f32::NAN
        ));
    }

    #[test]
    fn external_reference_decodes_into_a_ui_texture() {
        let path = std::env::temp_dir().join(format!(
            "rpl-reference-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let image = image::RgbaImage::from_pixel(2, 3, image::Rgba([240, 16, 120, 255]));
        image.save(&path).unwrap();
        let loaded = load_reference_texture(
            &egui::Context::default(),
            crate::workspace::ReferenceLayout {
                path: path.clone(),
                position: [0.0, 0.0],
                size: [300.0, 300.0],
                opacity: 0.5,
                view_zoom: 1.0,
                view_pan: [0.0, 0.0],
                position_locked: false,
            },
        )
        .unwrap();
        assert_eq!(loaded.texture.size(), [2, 3]);
        assert_eq!(loaded.layout.path, path);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn reference_board_adds_jpeg_and_downscales_photos_with_partial_failure_feedback() {
        let dir = std::env::temp_dir().join(format!(
            "rpl-reference-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let large_jpeg = dir.join("phone-photo.jpg");
        image::RgbImage::from_pixel(2400, 1800, image::Rgb([120, 80, 200]))
            .save(&large_jpeg)
            .unwrap();
        let missing = dir.join("missing.png");
        let mut references = Vec::new();
        let context = egui::Context::default();

        add_reference_path(&context, &mut references, large_jpeg.clone()).unwrap();
        let reference = &references[0];
        assert_eq!(reference.source_size, [2400, 1800]);
        assert!(reference.texture.size()[0] <= MAX_REFERENCE_PREVIEW_EDGE as usize);
        assert!(reference.texture.size()[1] <= MAX_REFERENCE_PREVIEW_EDGE as usize);
        assert_eq!(references[0].layout.path, large_jpeg);

        let error = add_reference_path(&context, &mut references, missing).unwrap_err();
        assert!(error.contains("missing.png"));
        assert_eq!(
            references.len(),
            1,
            "successful imports survive later errors"
        );
        let duplicate = references[0].layout.path.clone();
        assert!(
            add_reference_path(&context, &mut references, duplicate)
                .unwrap_err()
                .contains("already on the reference board")
        );

        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LassoAppend {
    Unchanged,
    Added,
    LimitReached,
}

fn append_lasso_point(points: &mut Vec<[f64; 2]>, position: [f64; 2]) -> LassoAppend {
    let Some(last) = points.last().copied() else {
        return LassoAppend::LimitReached;
    };
    if (position[0] - last[0]).hypot(position[1] - last[1]) < 2.0 {
        return LassoAppend::Unchanged;
    }
    if points.len() >= paint_document::editing::MAX_SELECTION_POINTS {
        return LassoAppend::LimitReached;
    }
    points.push(position);
    LassoAppend::Added
}

fn native_project_path(path: &std::path::Path) -> PathBuf {
    path.with_extension("rpl")
}

fn brush_display_name(kind: BrushKind) -> &'static str {
    match kind {
        BrushKind::Round => "Hard round",
        BrushKind::Ink => "Ink pen",
        BrushKind::Pencil => "Pencil",
        BrushKind::Marker => "Marker",
        BrushKind::Airbrush => "Airbrush",
        BrushKind::Soft => "Soft brush",
        BrushKind::Textured => "Textured brush",
        BrushKind::Calligraphy => "Calligraphy",
        BrushKind::Eraser => "Eraser",
        BrushKind::Wash => "Wash · experimental",
    }
}

#[derive(Clone)]
pub(crate) enum Action {
    Fill(bool),
    Deselect,
    Transform([f64; 6]),
    Crop,
    New,
    Open,
    Save,
    SaveAs,
    RetryClose,
    SaveBrush,
    LoadBrush,
    Export,
    Import,
    ImportOra,
    ExportOra,
    Recover(PathBuf),
    Recent(PathBuf),
    RemoveRecovery(PathBuf),
    RemoveDraft(PathBuf),
    AddReference(PathBuf),
    Undo,
    Redo,
    Add,
    Duplicate,
    Delete,
    DuplicateLayer(u64),
    CopySelectionToLayer,
    DeleteLayer(u64),
    ClearLayer(u64),
    Clear,
    Select(u64),
    Move(i32),
    MoveLayer(u64, i32),
    MoveLayerTo(u64, usize),
    Layer(u64, String, bool, bool, u8, Blend),
}

#[derive(Clone)]
struct LayerMenuTarget {
    id: u64,
    layer_count: usize,
    layer_index: usize,
    emit_test_markers: bool,
    name: String,
    visible: bool,
    locked: bool,
    selected: bool,
    has_selection: bool,
    opacity: u8,
    blend: Blend,
}

pub(crate) struct LayerDragUi<'a> {
    pub handles: &'a mut Vec<(u64, egui::Rect)>,
    pub drop_targets: &'a mut Vec<(u64, egui::Rect)>,
    pub source: Option<u64>,
    pub target: Option<u64>,
}

fn layer_actions_menu(
    ui: &mut egui::Ui,
    target: LayerMenuTarget,
    actions: &mut Vec<Action>,
    focus_rename: &mut bool,
) {
    if target.emit_test_markers {
        crate::studio_log!(
            "UI_TEST_LAYER_MENU_STATE: {} visible={} locked={}",
            target.id,
            target.visible,
            target.locked
        );
    }
    let select = ui.button("Select layer");
    if target.emit_test_markers {
        crate::studio_log!(
            "UI_TEST_LAYER_SELECT_MENU_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
            select.rect.left(),
            select.rect.top(),
            select.rect.width(),
            select.rect.height(),
            ui.ctx().pixels_per_point(),
        );
    }
    if select.clicked() {
        actions.push(Action::Select(target.id));
        ui.close();
    }
    if ui.button("Rename layer…").clicked() {
        actions.push(Action::Select(target.id));
        *focus_rename = true;
        ui.close();
    }
    let mut visible = target.visible;
    let visibility = ui.checkbox(&mut visible, "Visible");
    if target.emit_test_markers {
        crate::studio_log!(
            "UI_TEST_LAYER_VISIBLE_MENU_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
            visibility.rect.left(),
            visibility.rect.top(),
            visibility.rect.width(),
            visibility.rect.height(),
            ui.ctx().pixels_per_point(),
        );
    }
    if visibility.changed() {
        if target.emit_test_markers {
            crate::studio_log!(
                "UI_TEST_LAYER_VISIBILITY_CHANGED: {} {}",
                target.id,
                visible
            );
        }
        actions.push(Action::Layer(
            target.id,
            target.name.clone(),
            visible,
            target.locked,
            target.opacity,
            target.blend,
        ));
    }
    let mut locked = target.locked;
    let locking = ui.checkbox(&mut locked, "Locked");
    if target.emit_test_markers {
        crate::studio_log!(
            "UI_TEST_LAYER_LOCKED_MENU_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
            locking.rect.left(),
            locking.rect.top(),
            locking.rect.width(),
            locking.rect.height(),
            ui.ctx().pixels_per_point(),
        );
    }
    if locking.changed() {
        if target.emit_test_markers {
            crate::studio_log!("UI_TEST_LAYER_LOCK_CHANGED: {} {}", target.id, locked);
        }
        actions.push(Action::Layer(
            target.id,
            target.name.clone(),
            target.visible,
            locked,
            target.opacity,
            target.blend,
        ));
    }
    ui.separator();
    let duplicate = ui.button("Duplicate layer");
    if target.emit_test_markers {
        crate::studio_log!(
            "UI_TEST_LAYER_DUPLICATE_MENU_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
            duplicate.rect.left(),
            duplicate.rect.top(),
            duplicate.rect.width(),
            duplicate.rect.height(),
            ui.ctx().pixels_per_point(),
        );
    }
    if duplicate.clicked() {
        if target.emit_test_markers {
            crate::studio_log!("UI_TEST_LAYER_DUPLICATE_CLICKED: {}", target.id);
        }
        actions.push(Action::DuplicateLayer(target.id));
        ui.close();
    }
    let copy_selection = ui
        .add_enabled(
            target.selected && target.has_selection && target.visible && !target.locked,
            egui::Button::new("Copy selected pixels to new layer"),
        )
        .on_hover_text(if !target.has_selection {
            "Make a rectangle, ellipse, or lasso selection first"
        } else if !target.selected {
            "Select this layer before copying selected pixels"
        } else if !target.visible || target.locked {
            "The active source layer must be visible and unlocked"
        } else {
            "Copy selected pixels above this layer; the source remains unchanged"
        });
    if target.emit_test_markers {
        crate::studio_log!(
            "UI_TEST_LAYER_COPY_SELECTION_MENU_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
            copy_selection.rect.left(),
            copy_selection.rect.top(),
            copy_selection.rect.width(),
            copy_selection.rect.height(),
            ui.ctx().pixels_per_point(),
        );
    }
    if copy_selection.clicked() {
        if target.emit_test_markers {
            crate::studio_log!("UI_TEST_LAYER_COPY_SELECTION_CLICKED: {}", target.id);
        }
        actions.push(Action::CopySelectionToLayer);
        ui.close();
    }
    let clear = ui.add_enabled(!target.locked, egui::Button::new("Clear layer contents"));
    if target.emit_test_markers {
        crate::studio_log!(
            "UI_TEST_LAYER_CLEAR_MENU_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
            clear.rect.left(),
            clear.rect.top(),
            clear.rect.width(),
            clear.rect.height(),
            ui.ctx().pixels_per_point(),
        );
    }
    if clear.clicked() {
        if target.emit_test_markers {
            crate::studio_log!("UI_TEST_LAYER_CLEAR_CLICKED: {}", target.id);
        }
        actions.push(Action::ClearLayer(target.id));
        ui.close();
    }
    let delete = ui.add_enabled(target.layer_count > 1, egui::Button::new("Delete layer"));
    if target.emit_test_markers {
        crate::studio_log!(
            "UI_TEST_LAYER_DELETE_MENU_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
            delete.rect.left(),
            delete.rect.top(),
            delete.rect.width(),
            delete.rect.height(),
            ui.ctx().pixels_per_point(),
        );
    }
    if delete.clicked() {
        actions.push(Action::DeleteLayer(target.id));
        ui.close();
    }
    ui.separator();
    let bring_forward = ui.add_enabled(
        target.layer_index + 1 < target.layer_count,
        egui::Button::new("Bring forward"),
    );
    if target.emit_test_markers {
        crate::studio_log!(
            "UI_TEST_LAYER_BRING_FORWARD_MENU_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
            bring_forward.rect.left(),
            bring_forward.rect.top(),
            bring_forward.rect.width(),
            bring_forward.rect.height(),
            ui.ctx().pixels_per_point(),
        );
    }
    if bring_forward.clicked() {
        if target.emit_test_markers {
            crate::studio_log!("UI_TEST_LAYER_MOVE_MENU_CLICKED: {} 1", target.id);
        }
        actions.push(Action::MoveLayer(target.id, 1));
        ui.close();
    }
    let send_backward = ui.add_enabled(target.layer_index > 0, egui::Button::new("Send backward"));
    if target.emit_test_markers {
        crate::studio_log!(
            "UI_TEST_LAYER_SEND_BACKWARD_MENU_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
            send_backward.rect.left(),
            send_backward.rect.top(),
            send_backward.rect.width(),
            send_backward.rect.height(),
            ui.ctx().pixels_per_point(),
        );
    }
    if send_backward.clicked() {
        if target.emit_test_markers {
            crate::studio_log!("UI_TEST_LAYER_MOVE_MENU_CLICKED: {} -1", target.id);
        }
        actions.push(Action::MoveLayer(target.id, -1));
        ui.close();
    }
}

#[derive(Clone, Copy)]
enum FileRequest {
    OpenProject,
    SaveProjectAs,
    SaveBrush,
    LoadBrush,
    ExportPng,
    ImportPng,
    ImportOra,
    ExportOra,
    AddReferences,
}
impl FileRequest {
    fn action(self) -> Option<Action> {
        Some(match self {
            Self::OpenProject => Action::Open,
            Self::SaveProjectAs => Action::SaveAs,
            Self::SaveBrush => Action::SaveBrush,
            Self::LoadBrush => Action::LoadBrush,
            Self::ExportPng => Action::Export,
            Self::ImportPng => Action::Import,
            Self::ImportOra => Action::ImportOra,
            Self::ExportOra => Action::ExportOra,
            Self::AddReferences => return None,
        })
    }
}

fn selected_layer_settings(
    ui: &mut egui::Ui,
    project: &mut Engine,
    actions: &mut Vec<Action>,
    test_mode: bool,
) {
    if let Some(layer) = project
        .state()
        .layers
        .iter()
        .find(|layer| layer.id == project.state().selected)
    {
        let id = layer.id;
        let mut percent = f32::from(layer.opacity) * (100.0 / 255.0);
        let mut opacity = layer.opacity;
        let mut blend = layer.blend;
        let mut changed = false;
        let settings_width = ui.available_width().clamp(180.0, 260.0);
        let settings = egui::CollapsingHeader::new("Selected layer settings")
            .id_salt(("layer-settings", id))
            .default_open(false)
            .show(ui, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(settings_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Opacity");
                            ui.label(format!("{percent:.0}%"));
                        });
                        ui.spacing_mut().slider_width = settings_width;
                        let opacity_control = ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::Slider::new(&mut percent, 0.0..=100.0).show_value(false),
                        );
                        opacity_control.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Slider,
                                ui.is_enabled(),
                                format!("Layer opacity, {percent:.0}%"),
                            )
                        });
                        if test_mode {
                            crate::studio_log!(
                                "UI_TEST_LAYER_OPACITY_SLIDER_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                opacity_control.rect.left(),
                                opacity_control.rect.top(),
                                opacity_control.rect.width(),
                                opacity_control.rect.height(),
                                ui.ctx().pixels_per_point(),
                            );
                        }
                        if opacity_control.changed() {
                            opacity = (percent * 2.55).round() as u8;
                            changed = true;
                        }
                        let blend_control = egui::ComboBox::from_id_salt(("flyout-blend", id))
                            .selected_text(format!("{blend:?}"))
                            .show_ui(ui, |ui| {
                                for mode in [
                                    Blend::Normal,
                                    Blend::Multiply,
                                    Blend::Screen,
                                    Blend::Overlay,
                                ] {
                                    let option = ui.selectable_value(
                                        &mut blend,
                                        mode,
                                        format!("{mode:?}"),
                                    );
                                    if test_mode {
                                        crate::studio_log!(
                                            "UI_TEST_LAYER_BLEND_OPTION_RECT: {mode:?} {:.1} {:.1} {:.1} {:.1} {:.2}",
                                            option.rect.left(),
                                            option.rect.top(),
                                            option.rect.width(),
                                            option.rect.height(),
                                            ui.ctx().pixels_per_point(),
                                        );
                                    }
                                }
                            });
                        if test_mode {
                            crate::studio_log!(
                                "UI_TEST_LAYER_BLEND_COMBO_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                blend_control.response.rect.left(),
                                blend_control.response.rect.top(),
                                blend_control.response.rect.width(),
                                blend_control.response.rect.height(),
                                ui.ctx().pixels_per_point(),
                            );
                        }
                        changed |= blend != layer.blend;
                    },
                );
            });
        if test_mode {
            crate::studio_log!(
                "UI_TEST_LAYER_SETTINGS_HEADER_RECT: {} {:.1} {:.1} {:.1} {:.1} {:.2}",
                settings.body_returned.is_some(),
                settings.header_response.rect.left(),
                settings.header_response.rect.top(),
                settings.header_response.rect.width(),
                settings.header_response.rect.height(),
                ui.ctx().pixels_per_point(),
            );
        }
        if changed {
            actions.push(Action::Layer(
                id,
                layer.name.clone(),
                layer.visible,
                layer.locked,
                opacity,
                blend,
            ));
        }
    }
}

pub(crate) struct LayerThumbnailContext<'a> {
    pub cache: &'a mut crate::layer_thumbnails::LayerThumbnailCache,
    pub document_epoch: u64,
}

pub(crate) fn layer_flyout_contents(
    ui: &mut egui::Ui,
    project: &mut Engine,
    thumbnail_context: &mut LayerThumbnailContext<'_>,
    actions: &mut Vec<Action>,
    test_mode: bool,
    compact: bool,
    drag_ui: &mut LayerDragUi<'_>,
) {
    drag_ui.handles.clear();
    drag_ui.drop_targets.clear();
    let thumbnail_updates =
        thumbnail_context
            .cache
            .refresh(ui.ctx(), thumbnail_context.document_epoch, project);
    if test_mode {
        for update in thumbnail_updates {
            crate::studio_log!(
                "UI_TEST_LAYER_THUMBNAIL_UPDATED: {} {:016x} {} {}",
                update.layer_id,
                update.fingerprint,
                update.covered_pixels,
                update.colored_pixels
            );
        }
    }
    let selected = project.state().selected;
    let selected_index = project
        .state()
        .layers
        .iter()
        .position(|layer| layer.id == selected)
        .unwrap_or(0);
    let layer_count = project.state().layers.len();
    let layers: Vec<_> = project
        .state()
        .layers
        .iter()
        .rev()
        .map(|layer| {
            (
                layer.id,
                layer.name.clone(),
                layer.visible,
                layer.locked,
                thumbnail_context.cache.texture(layer.id).cloned(),
            )
        })
        .collect();
    let mut layer_actions = |ui: &mut egui::Ui| {
        let new = ui
            .button(if compact { "+" } else { "New" })
            .on_hover_text("New layer");
        new.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "New layer")
        });
        if new.clicked() {
            actions.push(Action::Add);
        }
        let duplicate = ui
            .button(if compact { "Copy" } else { "Duplicate" })
            .on_hover_text("Duplicate selected layer");
        duplicate.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                "Duplicate selected layer",
            )
        });
        if duplicate.clicked() {
            actions.push(Action::Duplicate);
        }
        let delete = ui
            .add_enabled(
                layers.len() > 1,
                egui::Button::new(if compact { "Del" } else { "Delete" }),
            )
            .on_hover_text("Delete selected layer");
        delete.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                "Delete selected layer",
            )
        });
        if delete.clicked() {
            actions.push(Action::Delete);
        }
        let clear = ui
            .button("Clear")
            .on_hover_text("Clear the selected layer; Undo restores its pixels");
        clear.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                "Clear selected layer",
            )
        });
        if clear.clicked() {
            actions.push(Action::Clear);
        }
    };
    // Compact labels keep all four common actions on one row, preserving
    // vertical room for the selected layer's visibility and lock controls.
    ui.horizontal(&mut layer_actions);
    if !compact {
        ui.horizontal(|ui| {
            ui.label("Order");
            if ui
                .add_enabled(selected_index + 1 < layer_count, egui::Button::new("Raise"))
                .on_hover_text("Move the selected layer toward the front")
                .clicked()
            {
                actions.push(Action::Move(1));
            }
            if ui
                .add_enabled(selected_index > 0, egui::Button::new("Lower"))
                .on_hover_text("Move the selected layer toward the back")
                .clicked()
            {
                actions.push(Action::Move(-1));
            }
        });
        ui.separator();
        ui.small("Frontmost layer appears first.");
    } else {
        ui.separator();
    }
    if !compact {
        selected_layer_settings(ui, project, actions, test_mode);
        ui.separator();
    }
    for (row_index, (id, mut name, mut visible, mut locked, thumbnail)) in
        layers.into_iter().enumerate()
    {
        crate::ui_tokens::card_frame(ui.visuals(), id == selected)
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                let layer_index = project
                    .state()
                    .layers
                    .iter()
                    .position(|layer| layer.id == id)
                    .unwrap_or(0);
                let (opacity, blend) = project
                    .state()
                    .layers
                    .iter()
                    .find(|layer| layer.id == id)
                    .map(|layer| (layer.opacity, layer.blend))
                    .unwrap_or((255, Blend::Normal));
                let menu_target = LayerMenuTarget {
                    id,
                    layer_count,
                    layer_index,
                    emit_test_markers: test_mode && row_index == 0,
                    name: name.clone(),
                    visible,
                    locked,
                    selected: id == selected,
                    has_selection: project.state().selection.is_some(),
                    opacity,
                    blend,
                };
                let rename_id = egui::Id::new(("layer-flyout-name", id));
                let mut focus_rename = false;
                let row = ui.horizontal(|ui| {
                    let drag_handle = crate::studio_icons::layer_drag_handle(
                        ui,
                        &name,
                        if compact { [20.0, 44.0] } else { [36.0, 44.0] },
                    );
                    drag_ui.handles.push((id, drag_handle.rect));
                    if test_mode {
                        crate::studio_log!(
                            "UI_TEST_LAYER_DRAG_HANDLE_RECT: {} {} {:.1} {:.1} {:.1} {:.1} {:.2}",
                            row_index,
                            id,
                            drag_handle.rect.left(),
                            drag_handle.rect.top(),
                            drag_handle.rect.width(),
                            drag_handle.rect.height(),
                            ui.ctx().pixels_per_point(),
                        );
                    }
                    let thumbnail_rect =
                        layer_thumbnail(ui, thumbnail.as_ref(), id == selected, compact);
                    if test_mode && row_index == 0 {
                        crate::studio_log!(
                            "UI_TEST_LAYER_THUMBNAIL_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                            thumbnail_rect.left(),
                            thumbnail_rect.top(),
                            thumbnail_rect.width(),
                            thumbnail_rect.height(),
                            ui.ctx().pixels_per_point(),
                        );
                    }
                    let more = if compact {
                        egui::containers::menu::MenuButton::new("⋯")
                            .config(
                                egui::containers::menu::MenuConfig::new()
                                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                            )
                            .ui(ui, |ui| {
                                layer_actions_menu(
                                    ui,
                                    menu_target.clone(),
                                    actions,
                                    &mut focus_rename,
                                );
                            })
                            .0
                    } else {
                        ui.menu_button("Actions", |ui| {
                            layer_actions_menu(ui, menu_target.clone(), actions, &mut focus_rename);
                        })
                        .response
                    };
                    let more = more.on_hover_text("Layer actions");
                    more.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            ui.is_enabled(),
                            "Layer actions",
                        )
                    });
                    if test_mode && row_index == 0 && more.clicked() {
                        crate::studio_log!("UI_TEST_LAYER_ACTIONS_OPENED: {id}");
                    }
                    if test_mode && row_index == 0 {
                        crate::studio_log!(
                            "UI_TEST_LAYER_MORE_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                            more.rect.left(),
                            more.rect.top(),
                            more.rect.width(),
                            more.rect.height(),
                            ui.ctx().pixels_per_point(),
                        );
                    }
                    let label = ui.selectable_label(
                        id == selected,
                        crate::ui_tokens::type_text(
                            name.clone(),
                            crate::ui_tokens::TypeRole::LayerName,
                            ui.visuals().text_color(),
                        ),
                    );
                    if test_mode && row_index == 0 {
                        crate::studio_log!(
                            "UI_TEST_LAYER_NAME_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                            label.rect.left(),
                            label.rect.top(),
                            label.rect.width(),
                            label.rect.height(),
                            ui.ctx().pixels_per_point(),
                        );
                    }
                    if label.clicked() {
                        if test_mode {
                            crate::studio_log!("UI_TEST_LAYER_SELECTED: {id}");
                        }
                        actions.push(Action::Select(id));
                    }
                    label
                });
                if test_mode {
                    crate::studio_log!(
                        "UI_TEST_LAYER_ROW_RECT: {} {} {:.1} {:.1} {:.1} {:.1} {:.2}",
                        row_index,
                        id,
                        row.response.rect.left(),
                        row.response.rect.top(),
                        row.response.rect.width(),
                        row.response.rect.height(),
                        ui.ctx().pixels_per_point(),
                    );
                }
                if test_mode && row_index == 0 {
                    crate::studio_log!(
                        "UI_TEST_LAYER_FRONT_ROW_RECT: {} {:.1} {:.1} {:.1} {:.1} {:.2}",
                        id,
                        row.response.rect.left(),
                        row.response.rect.top(),
                        row.response.rect.width(),
                        row.response.rect.height(),
                        ui.ctx().pixels_per_point(),
                    );
                }
                drag_ui.drop_targets.push((id, row.response.rect));
                if drag_ui.source.is_some()
                    && drag_ui.target == Some(id)
                    && ui
                        .ctx()
                        .pointer_interact_pos()
                        .is_some_and(|pointer| row.response.rect.contains(pointer))
                {
                    ui.painter().rect_stroke(
                        row.response.rect.shrink(1.0),
                        7.0,
                        ui.visuals().selection.stroke,
                        egui::StrokeKind::Inside,
                    );
                }
                let context_menu = row.inner.context_menu(|ui| {
                    layer_actions_menu(ui, menu_target.clone(), actions, &mut focus_rename);
                });
                if test_mode && row_index == 0 && context_menu.is_some() {
                    crate::studio_log!("UI_TEST_LAYER_NATIVE_CONTEXT_OPENED");
                }
                if focus_rename {
                    ui.ctx()
                        .memory_mut(|memory| memory.request_focus(rename_id));
                }
                let mut layer_state_changed = false;
                ui.horizontal(|ui| {
                    let visibility = if compact {
                        let response = crate::studio_icons::button(
                            ui,
                            if visible {
                                crate::studio_icons::Icon::Eye
                            } else {
                                crate::studio_icons::Icon::EyeOff
                            },
                            "Visible",
                            None,
                            visible,
                            [32.0, 32.0],
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Checkbox,
                                true,
                                visible,
                                "Visible",
                            )
                        });
                        let response = response.on_hover_text(if visible {
                            "Visible — activate to hide this layer"
                        } else {
                            "Hidden — activate to show this layer"
                        });
                        let changed = response.clicked();
                        if changed {
                            visible = !visible;
                        }
                        (changed, response.rect)
                    } else {
                        let response = ui.checkbox(&mut visible, "Visible");
                        (response.changed(), response.rect)
                    };
                    if test_mode && row_index == 0 {
                        crate::studio_log!(
                            "UI_TEST_LAYER_VISIBILITY_BUTTON_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                            visibility.1.left(),
                            visibility.1.top(),
                            visibility.1.width(),
                            visibility.1.height(),
                            ui.ctx().pixels_per_point(),
                        );
                    }
                    if visibility.0 {
                        layer_state_changed = true;
                        if test_mode {
                            crate::studio_log!(
                                "UI_TEST_LAYER_VISIBILITY_BUTTON_CHANGED: {id} {visible}"
                            );
                        }
                    }
                    let locking = if compact {
                        let response = crate::studio_icons::button(
                            ui,
                            if locked {
                                crate::studio_icons::Icon::Lock
                            } else {
                                crate::studio_icons::Icon::Unlock
                            },
                            "Locked",
                            None,
                            locked,
                            [32.0, 32.0],
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Checkbox,
                                true,
                                locked,
                                "Locked",
                            )
                        });
                        let response = response.on_hover_text(if locked {
                            "Locked — activate to unlock this layer"
                        } else {
                            "Unlocked — activate to lock this layer"
                        });
                        let changed = response.clicked();
                        if changed {
                            locked = !locked;
                        }
                        (changed, response.rect)
                    } else {
                        let response = ui.checkbox(&mut locked, "Locked");
                        (response.changed(), response.rect)
                    };
                    if test_mode && row_index == 0 {
                        crate::studio_log!(
                            "UI_TEST_LAYER_LOCK_BUTTON_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                            locking.1.left(),
                            locking.1.top(),
                            locking.1.width(),
                            locking.1.height(),
                            ui.ctx().pixels_per_point(),
                        );
                    }
                    if locking.0 {
                        layer_state_changed = true;
                        if test_mode {
                            crate::studio_log!("UI_TEST_LAYER_LOCK_BUTTON_CHANGED: {id} {locked}");
                        }
                    }
                });
                if layer_state_changed
                    && let Some(layer) = project.state().layers.iter().find(|l| l.id == id)
                {
                    actions.push(Action::Layer(
                        id,
                        layer.name.clone(),
                        visible,
                        locked,
                        layer.opacity,
                        layer.blend,
                    ));
                }
                if id == selected && !compact {
                    let selected_layer = project.state().layers.iter().find(|layer| layer.id == id);
                    let changed = ui
                        .add(
                            egui::TextEdit::singleline(&mut name)
                                .id(rename_id)
                                .char_limit(64)
                                .hint_text("Layer name"),
                        )
                        .changed();
                    if changed && let Some(layer) = selected_layer {
                        actions.push(Action::Layer(
                            id,
                            name.clone(),
                            visible,
                            locked,
                            layer.opacity,
                            layer.blend,
                        ));
                    }
                }
            });
    }
    if compact {
        selected_layer_settings(ui, project, actions, test_mode);
    }
}

fn layer_thumbnail(
    ui: &mut egui::Ui,
    texture: Option<&egui::TextureHandle>,
    selected: bool,
    compact: bool,
) -> egui::Rect {
    let bounds = if compact {
        egui::vec2(46.0, 34.0)
    } else {
        egui::vec2(64.0, 44.0)
    };
    let (rect, _) = ui.allocate_exact_size(bounds, egui::Sense::hover());
    let painter = ui.painter();
    let checker_columns = 8;
    let checker_rows = 6;
    let cell = egui::vec2(
        rect.width() / checker_columns as f32,
        rect.height() / checker_rows as f32,
    );
    for row in 0..checker_rows {
        for column in 0..checker_columns {
            let checker = if (row + column) % 2 == 0 {
                ui.visuals().extreme_bg_color
            } else {
                ui.visuals().faint_bg_color
            };
            let cell_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(column as f32 * cell.x, row as f32 * cell.y),
                cell,
            );
            painter.rect_filled(cell_rect, 0.0, checker);
        }
    }
    if let Some(texture) = texture {
        let image_size = texture.size_vec2();
        let scale = (bounds.x / image_size.x).min(bounds.y / image_size.y);
        let image_rect = egui::Rect::from_center_size(rect.center(), image_size * scale);
        painter.image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    painter.rect_stroke(
        rect,
        4.0,
        if selected {
            ui.visuals().selection.stroke
        } else {
            ui.visuals().widgets.inactive.bg_stroke
        },
        egui::StrokeKind::Inside,
    );
    rect
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EditTool {
    Brush,
    Smudge,
    ColorMix,
    Rectangle,
    Ellipse,
    Lasso,
    Eyedropper,
    ShapeLine,
    ShapeRectangle,
    ShapeEllipse,
    Gradient,
}

fn stroke_mode_for_tool(tool: EditTool, color_mix_strength: f64) -> paint_document::StrokeMode {
    match tool {
        EditTool::Smudge => paint_document::StrokeMode::Smudge,
        EditTool::ColorMix => paint_document::StrokeMode::ColorMix {
            strength: color_mix_strength,
        },
        _ => paint_document::StrokeMode::Paint,
    }
}

pub struct Studio {
    pub context: egui::Context,
    pub input: egui_winit::State,
    painter: egui_wgpu::Renderer,
    pub mapping: Option<CanvasMapping>,
    path: String,
    store: Option<Store>,
    status: String,
    new_name: String,
    new_width: f64,
    new_height: f64,
    new_dpi: u32,
    new_unit: crate::document_setup::DocumentUnit,
    new_background: [u8; 3],
    new_transparent: bool,
    new_custom_size_selected: bool,
    recent_document_sizes: Vec<crate::workspace::RecentDocumentSize>,
    recent_brush_colors: Vec<[u8; 3]>,
    saved_color_swatches: Vec<[u8; 3]>,
    favorite_brushes: Vec<BrushKind>,
    color_mix_strength: f64,
    barrel_action: crate::barrel::BarrelAction,
    barrel_router: crate::barrel::BarrelRouter,
    brush_search: String,
    brush_preview: Option<BrushPreviewCache>,
    brush_preset_previews: Vec<BrushPresetPreviewCache>,
    favorites_only: bool,
    observed_brush_color: Option<[u8; 3]>,
    hex_color: String,
    hex_color_for: Option<[u8; 3]>,
    color_hue: f32,
    new_dialog: bool,
    picker: Option<FileRequest>,
    manual_file_dialog: bool,
    manual_file_action: Option<Action>,
    manual_file_submit: bool,
    test_path_dialog: bool,
    test_reference_import: Option<PathBuf>,
    test_reference_centered: bool,
    test_palette_rect_logged: Option<[i32; 5]>,
    test_palette_grip_rect_logged: Option<[i32; 5]>,
    test_palette_toggle_logged: Option<[i32; 5]>,
    test_brush_rect_logged: bool,
    test_brush_layout_logged: bool,
    test_brush_preview_logged: bool,
    test_brush_window_logged: bool,
    test_brush_panel_visible: bool,
    test_brush_preset_previews_logged: bool,
    test_brush_narrow_card_logged: bool,
    test_brush_narrow_layout_logged: bool,
    test_brush_narrow_visible_card_count: Option<usize>,
    test_color_palette_logged: bool,
    test_palette_clearance_logged: bool,
    test_home_preview_logged: bool,
    test_home_quick_start_logged: bool,
    test_recovery_label_logged: bool,
    test_recovery_preview_hover_logged: bool,
    test_recovery_preview_logged: bool,
    test_reference_preview_logged: bool,
    test_new_artwork_logged: bool,
    test_home_dark_contrast_logged: bool,
    test_home_light_contrast_logged: bool,
    test_home_content_bounds_logged: bool,
    test_home_preset_rect_logged: Option<[i32; 5]>,
    test_hidden_reference_wheel_logged: bool,
    test_strong_text_contrast_logged: bool,
    confirm: Option<Action>,
    pub close_requested: bool,
    pub close_ready: bool,
    recovery: Option<Recovery>,
    recovery_files: Vec<PathBuf>,
    recovery_preview: Option<(PathBuf, Option<egui::TextureHandle>)>,
    recovery_manager: bool,
    recovery_manager_from_home: bool,
    drafts: Vec<PathBuf>,
    recent: paint_document::recent::Recent,
    recent_cards: Vec<RecentProjectCard>,
    recent_search: String,
    thumbnail_directory: Option<PathBuf>,
    recent_path: Option<PathBuf>,
    cleanup: Option<PathBuf>,
    draft_cleanup: Option<PathBuf>,
    last_submitted: u64,
    autosave_schedule: crate::autosave::AutosaveDebounce,
    recovery_saving: bool,
    appearance_mode: crate::workspace::AppearanceMode,
    accent_scheme: crate::workspace::AccentScheme,
    custom_accent_rgb: Option<[u8; 3]>,
    custom_accent_hex: String,
    system_dark: bool,
    appearance_cache: Option<AppearanceKey>,
    high_contrast: bool,
    reduced_motion: bool,
    ui_scale: f32,
    palette_dock: crate::workspace::PaletteDock,
    palette_dragging: bool,
    palette_drag_offset: [f32; 2],
    palette_viewport_height: f32,
    palette_was_horizontal: bool,
    palette_collapsed: bool,
    handedness: crate::workspace::Handedness,
    palette_position: [f32; 2],
    palette_rect: Option<egui::Rect>,
    layers_flyout_position: [f32; 2],
    layers_flyout_size: [f32; 2],
    brush_flyout_position: [f32; 2],
    color_flyout_position: [f32; 2],
    workspace_controls_position: [f32; 2],
    workspace_controls_size: [f32; 2],
    brush_flyout_open: bool,
    layers_flyout_open: bool,
    layer_drag_source: Option<u64>,
    layer_drag_origin: Option<[f64; 2]>,
    layer_drag_moved: bool,
    layer_drag_target: Option<u64>,
    layer_drag_handles: Vec<(u64, egui::Rect)>,
    layer_drop_targets: Vec<(u64, egui::Rect)>,
    pending_layer_move: Option<(u64, usize)>,
    layer_thumbnail_cache: crate::layer_thumbnails::LayerThumbnailCache,
    layer_thumbnail_document_epoch: u64,
    color_flyout_open: bool,
    navigation: crate::navigation::Navigation,
    touches: crate::navigation::TouchNavigation,
    touch_routes: crate::navigation::TouchTargetRouter,
    control_held: bool,
    workspace_needs_save: bool,
    pan_cursor: Option<[f64; 2]>,
    workspace_center: [f64; 2],
    tool: EditTool,
    shape_filled: bool,
    gradient_kind: GradientKind,
    gradient_end_color: [u8; 3],
    gradient_preview_texture: Option<egui::TextureHandle>,
    gradient_preview_key: Option<GradientPreviewKey>,
    edit_pointer: Option<u64>,
    edit_points: Vec<[f64; 2]>,
    transform_offset: [f64; 2],
    transform_scale: f64,
    transform_angle: f64,
    pen_ui: std::collections::BTreeSet<u64>,
    mouse_position: Option<[f64; 2]>,
    ui_drag: bool,
    pub last_upload_us: u128,
    pub last_frame_us: u128,
    preferences_path: Option<PathBuf>,
    references: Vec<ReferenceImage>,
    compact_reference_index: usize,
    brush_panel_width: f32,
    show_controls: bool,
    inspector_tab: crate::workspace::InspectorTab,
    welcome: bool,
    new_from_welcome: bool,
    picker_from_welcome: bool,
}
impl Studio {
    pub fn new(g: &Renderer, window: &Window, format: wgpu::TextureFormat) -> Self {
        let context = egui::Context::default();
        crate::ui_tokens::install(&context);
        let input = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(g.device.limits().max_texture_dimension_2d as usize),
        );
        let dir = paint_document::recovery::directory();
        let files = dir
            .as_ref()
            .map(|p| paint_document::recovery::available(p))
            .unwrap_or_default();
        let recovery = dir.as_ref().ok().and_then(|p| Recovery::start(p).ok());
        let recent_path = dir
            .as_ref()
            .ok()
            .and_then(|p| p.parent())
            .map(|p| p.join("recent.json"));
        let thumbnail_directory = recent_path
            .as_ref()
            .and_then(|path| path.parent())
            .map(|directory| directory.join("home-thumbnails"));
        let recent = recent_path
            .as_ref()
            .and_then(|p| paint_document::recent::Recent::load(p).ok())
            .unwrap_or_default();
        let recent_cards = recent_project_cards(&recent, &context, thumbnail_directory.as_deref());
        let preferences_path = dir
            .as_ref()
            .ok()
            .and_then(|p| p.parent())
            .map(|p| p.join("workspace.json"));
        let test_path_dialog = std::env::var_os("RPL_TEST_PATH_DIALOG").is_some();
        let test_reference_import = test_path_dialog
            .then(|| std::env::var_os("RPL_TEST_REFERENCE_IMPORT_PATH"))
            .flatten()
            .map(PathBuf::from);
        let new_name = if test_path_dialog {
            std::env::var("RPL_TEST_DOCUMENT_NAME").unwrap_or_else(|_| "Untitled".into())
        } else {
            "Untitled".into()
        };
        let preferences = preferences_path
            .as_ref()
            .and_then(|p| crate::workspace::load(p))
            .unwrap_or_default();
        let document_setup = preferences.new_document.clone();
        let recent_document_sizes = preferences.recent_document_sizes.clone();
        let recent_brush_colors = preferences.recent_brush_colors.clone();
        let saved_color_swatches = preferences.saved_color_swatches.clone();
        let favorite_brushes = preferences.favorite_brushes.clone();
        let color_mix_strength = preferences.color_mix_strength;
        let barrel_action = preferences.barrel_action;
        let references: Vec<ReferenceImage> = preferences
            .references
            .iter()
            .cloned()
            .filter_map(|layout| load_reference_texture(&context, layout).ok())
            .collect();
        if test_path_dialog {
            crate::studio_log!("UI_TEST_REFERENCE_COUNT: {}", references.len());
        }
        let appearance_mode = preferences.effective_appearance_mode();
        let custom_accent_rgb = preferences.custom_accent_rgb;
        let custom_accent_hex = custom_accent_rgb
            .map(crate::color_controls::format_hex_rgb)
            .unwrap_or_else(|| "#C55CFF".into());
        let status = if recovery.is_some() {
            "Ready — local recovery enabled".into()
        } else {
            "Recovery unavailable: choose a writable project path and save frequently".into()
        };
        Self {
            context,
            input,
            painter: egui_wgpu::Renderer::new(&g.device, format, Default::default()),
            mapping: None,
            path: "artwork.rpl".into(),
            store: None,
            status,
            new_name,
            new_width: document_setup.width,
            new_height: document_setup.height,
            new_dpi: document_setup.dpi,
            new_unit: document_setup.unit,
            new_background: document_setup.background,
            new_transparent: document_setup.transparent,
            new_custom_size_selected: false,
            recent_document_sizes,
            recent_brush_colors,
            saved_color_swatches,
            favorite_brushes,
            color_mix_strength,
            barrel_action,
            barrel_router: crate::barrel::BarrelRouter::default(),
            brush_search: String::new(),
            brush_preview: None,
            brush_preset_previews: Vec::new(),
            favorites_only: false,
            observed_brush_color: None,
            hex_color: String::new(),
            hex_color_for: None,
            color_hue: 0.0,
            new_dialog: false,
            picker: None,
            manual_file_dialog: false,
            manual_file_action: None,
            manual_file_submit: false,
            test_path_dialog,
            test_reference_import,
            test_reference_centered: false,
            test_palette_rect_logged: None,
            test_palette_grip_rect_logged: None,
            test_palette_toggle_logged: None,
            test_brush_rect_logged: false,
            test_brush_layout_logged: false,
            test_brush_preview_logged: false,
            test_brush_window_logged: false,
            test_brush_panel_visible: false,
            test_brush_preset_previews_logged: false,
            test_brush_narrow_card_logged: false,
            test_brush_narrow_layout_logged: false,
            test_brush_narrow_visible_card_count: None,
            test_color_palette_logged: false,
            test_palette_clearance_logged: false,
            test_home_preview_logged: false,
            test_home_quick_start_logged: false,
            test_recovery_label_logged: false,
            test_recovery_preview_hover_logged: false,
            test_recovery_preview_logged: false,
            test_reference_preview_logged: false,
            test_new_artwork_logged: false,
            test_home_dark_contrast_logged: false,
            test_home_light_contrast_logged: false,
            test_home_content_bounds_logged: false,
            test_home_preset_rect_logged: None,
            test_hidden_reference_wheel_logged: false,
            test_strong_text_contrast_logged: false,
            confirm: None,
            close_requested: false,
            close_ready: false,
            recovery,
            recovery_files: files,
            recovery_preview: None,
            recovery_manager: false,
            recovery_manager_from_home: false,
            drafts: crate::drafts::list(),
            recent,
            recent_cards,
            recent_search: String::new(),
            thumbnail_directory,
            recent_path,
            cleanup: None,
            draft_cleanup: None,
            last_submitted: 0,
            autosave_schedule: Default::default(),
            recovery_saving: false,
            appearance_mode,
            accent_scheme: preferences.accent_scheme,
            custom_accent_rgb,
            custom_accent_hex,
            system_dark: window
                .theme()
                .map(|theme| theme == winit::window::Theme::Dark)
                .unwrap_or(preferences.dark_theme),
            appearance_cache: None,
            high_contrast: preferences.high_contrast,
            reduced_motion: preferences.reduced_motion,
            ui_scale: preferences.ui_scale,
            palette_dock: preferences.palette_dock,
            palette_dragging: false,
            palette_drag_offset: [0.0; 2],
            palette_viewport_height: 0.0,
            palette_was_horizontal: false,
            palette_collapsed: preferences.palette_collapsed,
            handedness: preferences.handedness,
            palette_position: preferences.palette_position,
            palette_rect: None,
            layers_flyout_position: preferences.layers_flyout_position,
            layers_flyout_size: preferences.layers_flyout_size,
            brush_flyout_position: preferences.brush_flyout_position,
            color_flyout_position: preferences.color_flyout_position,
            workspace_controls_position: preferences.workspace_controls_position,
            workspace_controls_size: preferences.workspace_controls_size,
            brush_flyout_open: false,
            layers_flyout_open: false,
            layer_drag_source: None,
            layer_drag_origin: None,
            layer_drag_moved: false,
            layer_drag_target: None,
            layer_drag_handles: Vec::new(),
            layer_drop_targets: Vec::new(),
            pending_layer_move: None,
            layer_thumbnail_cache: Default::default(),
            layer_thumbnail_document_epoch: 0,
            color_flyout_open: false,
            navigation: crate::navigation::Navigation {
                zoom: preferences.canvas_zoom,
                pan: preferences.canvas_pan,
            },
            touches: Default::default(),
            touch_routes: Default::default(),
            control_held: false,
            workspace_needs_save: false,
            pan_cursor: None,
            workspace_center: [0.0; 2],
            tool: EditTool::Brush,
            shape_filled: false,
            gradient_kind: GradientKind::Linear,
            gradient_end_color: [255, 116, 196],
            gradient_preview_texture: None,
            gradient_preview_key: None,
            edit_pointer: None,
            edit_points: Vec::new(),
            transform_offset: [0.0; 2],
            transform_scale: 1.0,
            transform_angle: 0.0,
            pen_ui: std::collections::BTreeSet::new(),
            mouse_position: None,
            ui_drag: false,
            last_upload_us: 0,
            last_frame_us: 0,
            preferences_path,
            compact_reference_index: 0,
            brush_panel_width: preferences.brush_panel_width,
            show_controls: preferences.show_controls,
            inspector_tab: preferences.inspector_tab,
            references,
            welcome: true,
            new_from_welcome: false,
            picker_from_welcome: false,
        }
    }

    pub fn init_accesskit(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window: &Window,
        proxy: winit::event_loop::EventLoopProxy<
            impl From<egui_winit::accesskit_winit::Event> + Send + 'static,
        >,
    ) {
        self.input.init_accesskit(event_loop, window, proxy);
    }

    pub fn accesskit_action_request(&mut self, request: egui::accesskit::ActionRequest) {
        self.input.on_accesskit_action_request(request);
    }
    fn brush_preset_texture(
        &mut self,
        context: &egui::Context,
        kind: BrushKind,
    ) -> Option<egui::TextureId> {
        if let Some(preview) = self
            .brush_preset_previews
            .iter()
            .find(|preview| preview.kind == kind)
        {
            return Some(preview.texture.id());
        }
        if self.brush_preset_previews.len() >= 10 {
            return None;
        }
        let pixels = crate::brush_preview::render_preset_rgba(kind).ok()?;
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [
                crate::brush_preview::WIDTH as usize,
                crate::brush_preview::HEIGHT as usize,
            ],
            &pixels,
        );
        let texture = context.load_texture(
            format!("brush preset sample {kind:?}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        let id = texture.id();
        self.brush_preset_previews
            .push(BrushPresetPreviewCache { kind, texture });
        Some(id)
    }

    fn compact_panel_layout_key(&self, context: &egui::Context) -> usize {
        if crate::studio_layout::compact_workspace_mode(context.content_rect()) {
            if self.brush_flyout_open {
                return 0;
            }
            usize::from(self.layers_flyout_open) + usize::from(!self.references.is_empty())
        } else {
            usize::from(self.layers_flyout_open) + self.references.len()
        }
    }

    fn invalidate_document_thumbnail_cache(&mut self) {
        self.layer_thumbnail_document_epoch = self.layer_thumbnail_document_epoch.saturating_add(1);
        self.layer_thumbnail_cache.invalidate();
    }

    fn visible_workspace_obstacles(
        &self,
        context: &egui::Context,
        except: &str,
        include_references: bool,
    ) -> Vec<egui::Rect> {
        let compact = crate::studio_layout::compact_workspace_mode(context.content_rect());
        let layout_key = self.compact_panel_layout_key(context);
        let active_reference = self
            .compact_reference_index
            .min(self.references.len().saturating_sub(1));
        let mut obstacles: Vec<_> = if include_references {
            self.references
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    !compact || (!self.brush_flyout_open && *index == active_reference)
                })
                .filter_map(|(_, reference)| {
                    let id = reference_panel_id(&reference.layout.path, compact, layout_key);
                    egui::AreaState::load(context, id).map(|state| state.rect())
                })
                .collect()
        } else {
            Vec::new()
        };
        for (name, open) in [
            ("Layers", self.layers_flyout_open),
            ("Brush library", self.brush_flyout_open),
            ("Color controls", self.color_flyout_open),
            ("Workspace controls", self.show_controls),
        ] {
            if name != except
                && open
                && let Some(state) = egui::AreaState::load(
                    context,
                    if name == "Layers" {
                        compact_floating_panel_id(name, compact, layout_key)
                    } else {
                        egui::Id::new(name)
                    },
                )
            {
                obstacles.push(state.rect());
            }
        }
        obstacles
    }
    fn reference_under_client(&self, client_px: [f64; 2]) -> bool {
        self.reference_index_under_client(client_px).is_some()
    }
    /// True when a point lies over an interactive floating surface. Winit
    /// mouse events over the document normally bypass egui so the stroke
    /// router can receive them; overlays must therefore be excluded before
    /// that decision or their controls become canvas clicks.
    fn floating_ui_under_client(&self, client_px: [f64; 2]) -> bool {
        let scale = self.context.pixels_per_point();
        let point = egui::pos2(
            (client_px[0] / f64::from(scale)) as f32,
            (client_px[1] / f64::from(scale)) as f32,
        );
        // Menus and popovers are independent foreground Areas and can extend
        // beyond their owning floating window. The window bounds below alone
        // would route those clicks into the canvas, especially when a bounded
        // Layers scroll area makes its owner shorter than the action menu.
        if self
            .context
            .layer_id_at(point)
            .is_some_and(|layer| layer.order == egui::Order::Foreground)
        {
            return true;
        }
        let compact = crate::studio_layout::compact_workspace_mode(self.context.content_rect());
        let layout_key = self.compact_panel_layout_key(&self.context);
        let active_reference = self
            .compact_reference_index
            .min(self.references.len().saturating_sub(1));
        let mut ids = vec![egui::Id::new("tool palette")];
        ids.extend(
            [
                (
                    "Layers",
                    self.layers_flyout_open && !(compact && self.brush_flyout_open),
                ),
                ("Brush library", self.brush_flyout_open),
                ("Color controls", self.color_flyout_open),
                ("Workspace controls", self.show_controls),
            ]
            .into_iter()
            .filter(|(_, open)| *open)
            .map(|(name, _)| {
                if name == "Layers" {
                    compact_floating_panel_id(name, compact, layout_key)
                } else {
                    egui::Id::new(name)
                }
            }),
        );
        if ids.into_iter().any(|id| {
            egui::AreaState::load(&self.context, id).is_some_and(|state| {
                crate::studio_layout::client_point_in_rect(state.rect(), client_px, scale)
            })
        }) {
            return true;
        }
        self.references
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                !compact || (!self.brush_flyout_open && *index == active_reference)
            })
            .any(|(_, reference)| {
                let id = reference_panel_id(&reference.layout.path, compact, layout_key);
                egui::AreaState::load(&self.context, id).is_some_and(|state| {
                    crate::studio_layout::client_point_in_rect(state.rect(), client_px, scale)
                })
            })
    }
    fn reference_index_under_client(&self, client_px: [f64; 2]) -> Option<usize> {
        let pixels_per_point = self.context.pixels_per_point();
        let compact = crate::studio_layout::compact_workspace_mode(self.context.content_rect());
        let layout_key = self.compact_panel_layout_key(&self.context);
        let active_reference = self
            .compact_reference_index
            .min(self.references.len().saturating_sub(1));
        self.references
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                !compact || (!self.brush_flyout_open && *index == active_reference)
            })
            .rev()
            .find_map(|(index, reference)| {
                let id = reference_panel_id(&reference.layout.path, compact, layout_key);
                let inside = if let Some(state) = egui::AreaState::load(&self.context, id) {
                    crate::studio_layout::client_point_in_rect(
                        state.rect(),
                        client_px,
                        pixels_per_point,
                    )
                } else {
                    reference_contains_client(&reference.layout, client_px, pixels_per_point)
                };
                inside.then_some(index)
            })
    }
    fn reference_viewport_under_client(&self, client_px: [f64; 2]) -> Option<usize> {
        let scale = f64::from(self.context.pixels_per_point());
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let point = egui::pos2((client_px[0] / scale) as f32, (client_px[1] / scale) as f32);
        self.references
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, reference)| {
                reference
                    .viewport
                    .is_some_and(|rect| rect.contains(point))
                    .then_some(index)
            })
    }
    fn clear_touch_routes(&mut self) {
        self.touch_routes.clear();
        self.touches.clear();
        for reference in &mut self.references {
            reference.touches.clear();
        }
    }
    pub fn window_event(
        &mut self,
        window: &Window,
        event: &winit::event::WindowEvent,
        session: &mut CanvasSession,
    ) -> (bool, bool) {
        use winit::event::{ElementState, WindowEvent as E};
        if let E::ModifiersChanged(modifiers) = event {
            self.control_held = modifiers.state().control_key();
        }
        if let E::ThemeChanged(theme) = event {
            self.system_dark = *theme == winit::window::Theme::Dark;
            self.appearance_cache = None;
            window.request_redraw();
            return (true, true);
        }
        // The deterministic path-entry dialog is only enabled by the Xvfb
        // workflow harness. Latch its submit key at the winit boundary: egui
        // text editing and platform shortcut handling can otherwise consume
        // Enter before the modal observes it.
        let test_dialog_enter = self.test_path_dialog
            && self.manual_file_dialog
            && matches!(
                event,
                E::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed
                        && (matches!(
                            event.physical_key,
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Enter
                                    | winit::keyboard::KeyCode::NumpadEnter
                            )
                        )
                        || event.logical_key
                            == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter))
            );
        if test_dialog_enter {
            crate::studio_log!("TEST_PATH_DIALOG: Enter received at winit boundary");
            self.manual_file_submit = true;
            window.request_redraw();
            return (true, true);
        }
        if let E::CursorMoved { position, .. } = event {
            self.mouse_position = Some([position.x, position.y]);
        }
        if matches!(
            event,
            E::Focused(false) | E::CursorLeft { .. } | E::Resized(_) | E::ScaleFactorChanged { .. }
        ) {
            self.layer_drag_source = None;
            self.layer_drag_origin = None;
            self.layer_drag_moved = false;
            self.layer_drag_target = None;
            self.clear_touch_routes();
            self.pan_cursor = None;
        }
        if self.layer_drag_source.is_some() && !self.layers_flyout_open {
            self.layer_drag_source = None;
            self.layer_drag_origin = None;
            self.layer_drag_moved = false;
            self.layer_drag_target = None;
        }
        if self.layers_flyout_open
            && matches!(
                event,
                E::MouseInput {
                    state: ElementState::Pressed,
                    button: winit::event::MouseButton::Left,
                    ..
                }
            )
            && let Some(position) = self.mouse_position
            && let Some(point) = client_point_to_egui(position, self.context.pixels_per_point())
            && let Some((id, _)) = self
                .layer_drag_handles
                .iter()
                .find(|(_, rect)| rect.contains(point))
        {
            self.layer_drag_source = Some(*id);
            self.layer_drag_origin = Some(position);
            self.layer_drag_moved = false;
            self.layer_drag_target = None;
            if self.test_path_dialog {
                crate::studio_log!("UI_TEST_LAYER_DRAG_STARTED: {id}");
            }
            window.request_redraw();
            return (true, true);
        }
        if let Some(source) = self.layer_drag_source {
            if let E::CursorMoved { position, .. } = event {
                let origin = self.layer_drag_origin.unwrap_or([position.x, position.y]);
                let dx = position.x - origin[0];
                let dy = position.y - origin[1];
                self.layer_drag_moved |= dx.mul_add(dx, dy * dy) >= 36.0;
                let point =
                    client_point_to_egui([position.x, position.y], self.context.pixels_per_point());
                self.layer_drag_target = if self.layer_drag_moved {
                    point.and_then(|point| {
                        self.layer_drop_targets
                            .iter()
                            .find_map(|(id, rect)| rect.contains(point).then_some(*id))
                    })
                } else {
                    None
                };
                window.request_redraw();
                return (true, true);
            }
            if matches!(
                event,
                E::MouseInput {
                    state: ElementState::Released,
                    button: winit::event::MouseButton::Left,
                    ..
                }
            ) {
                if self.layer_drag_moved
                    && let Some(target) = self.layer_drag_target
                    && target != source
                    && let Some(target_index) = session
                        .live()
                        .project()
                        .state()
                        .layers
                        .iter()
                        .position(|layer| layer.id == target)
                {
                    self.pending_layer_move = Some((source, target_index));
                    if self.test_path_dialog {
                        crate::studio_log!(
                            "UI_TEST_LAYER_DRAG_DROP: {source} {target_index} {target}"
                        );
                    }
                }
                self.layer_drag_source = None;
                self.layer_drag_origin = None;
                self.layer_drag_moved = false;
                self.layer_drag_target = None;
                window.request_redraw();
                return (true, true);
            }
        }
        if let E::Touch(touch) = event {
            if session.live().active() {
                self.clear_touch_routes();
                return (false, false);
            }
            let p = [touch.location.x, touch.location.y];
            let phase = match touch.phase {
                winit::event::TouchPhase::Started => crate::navigation::TouchPhase::Start,
                winit::event::TouchPhase::Moved => crate::navigation::TouchPhase::Move,
                winit::event::TouchPhase::Ended => crate::navigation::TouchPhase::End,
                winit::event::TouchPhase::Cancelled => crate::navigation::TouchPhase::Cancel,
            };
            let routed = if phase == crate::navigation::TouchPhase::Start {
                let target = if let Some(index) = self.reference_viewport_under_client(p) {
                    Some(crate::navigation::TouchTarget::Reference(
                        self.references[index].layout.path.clone(),
                    ))
                } else if !self.blocks_canvas()
                    && session.mode() == crate::interaction::Mode::Live
                    && self
                        .mapping
                        .is_some_and(|mapping| mapping.contains_client(p))
                    && !self.reference_under_client(p)
                    && !self.floating_ui_under_client(p)
                {
                    Some(crate::navigation::TouchTarget::Canvas)
                } else {
                    None
                };
                match target {
                    Some(target) => Some(self.touch_routes.start(touch.id, target)),
                    None if self.touch_routes.active_target().is_some() => Some(
                        self.touch_routes
                            .start(touch.id, crate::navigation::TouchTarget::Suppressed),
                    ),
                    None => None,
                }
            } else {
                self.touch_routes.target(touch.id)
            };
            if let Some(target) = routed {
                let scale = f64::from(self.context.pixels_per_point());
                match target {
                    crate::navigation::TouchTarget::Canvas => {
                        if let Some(pinch) = self.touches.event(touch.id, phase, p) {
                            self.workspace_needs_save |= self.navigation.pinch(
                                self.workspace_center,
                                pinch.from,
                                pinch.to,
                                pinch.ratio,
                            );
                        }
                    }
                    crate::navigation::TouchTarget::Reference(path) => {
                        if let Some(reference) = self
                            .references
                            .iter_mut()
                            .find(|reference| reference.layout.path == path)
                        {
                            let point = [p[0] / scale, p[1] / scale];
                            if let Some(pinch) = reference.touches.event(touch.id, phase, point)
                                && let Some(viewport) = reference.viewport
                            {
                                let from = egui::pos2(pinch.from[0] as f32, pinch.from[1] as f32);
                                let to = egui::pos2(pinch.to[0] as f32, pinch.to[1] as f32);
                                self.workspace_needs_save |= reference_zoom_about(
                                    &mut reference.layout,
                                    viewport,
                                    from,
                                    to,
                                    pinch.ratio as f32,
                                );
                            }
                        }
                    }
                    crate::navigation::TouchTarget::Suppressed => {}
                }
                if matches!(
                    phase,
                    crate::navigation::TouchPhase::End | crate::navigation::TouchPhase::Cancel
                ) {
                    self.touch_routes.finish(touch.id);
                }
                return (true, true);
            }
        }
        let canvas = self.mouse_position.is_some_and(|p| {
            self.mapping.is_some_and(|m| m.contains_client(p)) && !self.reference_under_client(p)
        }) && !self.blocks_canvas()
            && session.mode() == crate::interaction::Mode::Live;
        let reference_under_pointer = self
            .mouse_position
            .and_then(|position| self.reference_index_under_client(position));
        match event {
            E::MouseWheel { delta, .. } if reference_under_pointer.is_some() => {
                let index = reference_under_pointer.unwrap();
                let scale = f64::from(self.context.pixels_per_point());
                let pointer_px = self.mouse_position.unwrap();
                let pointer = egui::pos2(
                    (pointer_px[0] / scale) as f32,
                    (pointer_px[1] / scale) as f32,
                );
                let reference = &mut self.references[index];
                let Some(viewport) = reference.viewport else {
                    // The reference area state can outlive the visible panel
                    // for a frame when a modal or Home hides it. Forward the
                    // wheel event to egui so the visible modal can scroll;
                    // never unwrap the missing viewport or route it to art.
                    if self.test_path_dialog && !self.test_hidden_reference_wheel_logged {
                        self.test_hidden_reference_wheel_logged = true;
                        crate::studio_log!("UI_TEST_REFERENCE_WHEEL_IGNORED_HIDDEN");
                    }
                    let response = self.input.on_window_event(window, event);
                    return (false, response.repaint);
                };
                let changed = match delta {
                    winit::event::MouseScrollDelta::PixelDelta(delta) if !self.control_held => {
                        reference_zoom_about(
                            &mut reference.layout,
                            viewport,
                            pointer,
                            egui::pos2(
                                pointer.x + delta.x as f32 / scale as f32,
                                pointer.y + delta.y as f32 / scale as f32,
                            ),
                            1.0,
                        )
                    }
                    winit::event::MouseScrollDelta::PixelDelta(delta) => reference_zoom_about(
                        &mut reference.layout,
                        viewport,
                        pointer,
                        pointer,
                        (delta.y / 120.0).clamp(-1.0, 1.0).exp() as f32,
                    ),
                    winit::event::MouseScrollDelta::LineDelta(_, y) => reference_zoom_about(
                        &mut reference.layout,
                        viewport,
                        pointer,
                        pointer,
                        (f64::from(*y).clamp(-8.0, 8.0) * 0.12).exp() as f32,
                    ),
                };
                self.workspace_needs_save |= changed;
                return (true, true);
            }
            E::PinchGesture { delta, .. }
                if reference_under_pointer.is_some() && delta.is_finite() =>
            {
                let index = reference_under_pointer.unwrap();
                let scale = f64::from(self.context.pixels_per_point());
                let pointer_px = self.mouse_position.unwrap();
                let pointer = egui::pos2(
                    (pointer_px[0] / scale) as f32,
                    (pointer_px[1] / scale) as f32,
                );
                let reference = &mut self.references[index];
                if let Some(viewport) = reference.viewport {
                    self.workspace_needs_save |= reference_zoom_about(
                        &mut reference.layout,
                        viewport,
                        pointer,
                        pointer,
                        delta.clamp(-2.0, 2.0).exp() as f32,
                    );
                }
                return (true, true);
            }
            E::PanGesture { delta, .. } if reference_under_pointer.is_some() => {
                let index = reference_under_pointer.unwrap();
                let scale = f64::from(self.context.pixels_per_point());
                let pointer_px = self.mouse_position.unwrap();
                let pointer = egui::pos2(
                    (pointer_px[0] / scale) as f32,
                    (pointer_px[1] / scale) as f32,
                );
                let reference = &mut self.references[index];
                if let Some(viewport) = reference.viewport {
                    self.workspace_needs_save |= reference_zoom_about(
                        &mut reference.layout,
                        viewport,
                        pointer,
                        egui::pos2(
                            pointer.x + delta.x / scale as f32,
                            pointer.y + delta.y / scale as f32,
                        ),
                        1.0,
                    );
                }
                return (true, true);
            }
            E::MouseWheel { delta, .. } if canvas => {
                let p = self.mouse_position.unwrap();
                let changed = match delta {
                    winit::event::MouseScrollDelta::PixelDelta(delta) if !self.control_held => {
                        self.navigation.pinch(
                            self.workspace_center,
                            p,
                            [p[0] + delta.x, p[1] + delta.y],
                            1.0,
                        )
                    }
                    winit::event::MouseScrollDelta::PixelDelta(delta) => self.navigation.pinch(
                        self.workspace_center,
                        p,
                        p,
                        (delta.y / 120.0).clamp(-1.0, 1.0).exp(),
                    ),
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        let lines = f64::from(*y);
                        self.navigation.pinch(
                            self.workspace_center,
                            p,
                            p,
                            (lines.clamp(-8.0, 8.0) * 0.12).exp(),
                        )
                    }
                };
                self.workspace_needs_save |= changed;
                return (true, true);
            }
            E::PinchGesture { delta, .. } if canvas && delta.is_finite() => {
                let p = self.mouse_position.unwrap();
                self.workspace_needs_save |= self.navigation.pinch(
                    self.workspace_center,
                    p,
                    p,
                    delta.clamp(-2.0, 2.0).exp(),
                );
                return (true, true);
            }
            E::PanGesture { delta, .. } if canvas => {
                let p = self.mouse_position.unwrap();
                self.workspace_needs_save |= self.navigation.pinch(
                    self.workspace_center,
                    p,
                    [p[0] + f64::from(delta.x), p[1] + f64::from(delta.y)],
                    1.0,
                );
                return (true, true);
            }
            E::MouseInput {
                state,
                button: winit::event::MouseButton::Middle,
                ..
            } if canvas || self.pan_cursor.is_some() => {
                if *state == ElementState::Released && self.pan_cursor.is_some() {
                    self.workspace_needs_save = true;
                }
                self.pan_cursor = if *state == ElementState::Pressed {
                    self.mouse_position
                } else {
                    None
                };
                return (true, true);
            }
            E::CursorMoved { .. } if self.pan_cursor.is_some() => {
                let from = self.pan_cursor.unwrap();
                let to = self.mouse_position.unwrap();
                self.workspace_needs_save |=
                    self.navigation.pinch(self.workspace_center, from, to, 1.0);
                self.pan_cursor = Some(to);
                return (true, true);
            }
            _ => {}
        }
        use paint_input::Phase;
        let phase = match event {
            E::MouseInput {
                state,
                button: winit::event::MouseButton::Left,
                ..
            } => Some(if *state == ElementState::Pressed {
                Phase::Down
            } else {
                Phase::Up
            }),
            E::CursorMoved { .. } => Some(Phase::Move),
            _ => None,
        };
        if matches!(
            event,
            E::Focused(false) | E::CursorLeft { .. } | E::Resized(_) | E::ScaleFactorChanged { .. }
        ) {
            self.edit_pointer = None;
            self.edit_points.clear();
        }
        if let Some(phase) = phase
            && let Some(position) = self.mouse_position
            && self.editing_pointer(session, 0, phase, position)
        {
            return (true, true);
        }
        let on_canvas = !self.blocks_canvas()
            && self.mouse_position.is_some_and(|p| {
                self.mapping.is_some_and(|m| m.contains_client(p))
                    && !self.reference_under_client(p)
                    && !self.floating_ui_under_client(p)
            });
        match event {
            E::MouseInput {
                state: ElementState::Pressed,
                ..
            } if on_canvas && !self.ui_drag => return (false, false),
            E::MouseInput {
                state: ElementState::Pressed,
                ..
            } => {
                self.ui_drag = true;
                let r = self.input.on_window_event(window, event);
                return (true, r.repaint);
            }
            E::MouseInput {
                state: ElementState::Released,
                ..
            } => {
                let captured = self.ui_drag;
                self.ui_drag = false;
                if !captured {
                    return (false, false);
                }
                let r = self.input.on_window_event(window, event);
                return (true, r.repaint);
            }
            E::Focused(false) | E::CursorLeft { .. } => {
                self.ui_drag = false;
                self.mouse_position = None;
            }
            _ => {}
        }
        let r = self.input.on_window_event(window, event);
        (
            matches!(event, E::CursorMoved { .. }) && (self.ui_drag || self.blocks_canvas()),
            r.repaint,
        )
    }
    pub fn native_pen_ui(
        &mut self,
        s: &paint_input::PointerSample,
        _scale: f64,
        session: &mut CanvasSession,
    ) -> bool {
        let scale = f64::from(self.context.pixels_per_point());
        // Once a pen contact begins on the canvas, keep the native stream with
        // the painting processor while it crosses the paper boundary. Sending
        // these samples to egui would synthesize a Cancel and orphan the stroke.
        if session.live().active() {
            return false;
        }
        if self.editing_pointer(session, s.id, s.phase, s.position_px) {
            return true;
        }
        use paint_input::Phase;
        let outside = self.blocks_canvas()
            || self.reference_under_client(s.position_px)
            || self.floating_ui_under_client(s.position_px)
            || !self
                .mapping
                .is_some_and(|m| m.contains_client(s.position_px));
        if s.phase == Phase::Down && outside && self.pen_ui.len() < 64 {
            self.pen_ui.insert(s.id);
        }
        let captured = self.pen_ui.contains(&s.id);
        if captured || outside {
            let pos = egui::pos2(
                (s.position_px[0] / scale) as f32,
                (s.position_px[1] / scale) as f32,
            );
            let events = &mut self.input.egui_input_mut().events;
            events.push(egui::Event::PointerMoved(pos));
            if matches!(s.phase, Phase::Down | Phase::Up | Phase::Cancel) {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: s.phase == Phase::Down,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        if matches!(s.phase, Phase::Up | Phase::Cancel) {
            self.pen_ui.remove(&s.id);
        }
        captured || outside
    }
    /// Route the native WM_POINTER barrel bit only after the existing UI hit
    /// test has given panels and editing tools first refusal. The sample then
    /// uses CanvasSession's ordinary normalized lifecycle and stroke router.
    pub fn route_native_pen_barrel(
        &mut self,
        sample: &mut paint_input::PointerSample,
        session: &mut CanvasSession,
    ) -> bool {
        use paint_input::{Phase, Tool};
        if sample.tool != Tool::Pen || session.mode() != crate::interaction::Mode::Live {
            return false;
        }
        if sample.phase == Phase::Cancel {
            self.barrel_router.cancel_pointer(sample.id);
            return false;
        }
        if sample.buttons.eraser || sample.buttons.inverted {
            // Native inversion/eraser remains authoritative over the side
            // button assignment and keeps the existing WM_POINTER path intact.
            self.barrel_router.cancel_pointer(sample.id);
            return false;
        }

        let active = session.live().native_pointer_active(sample.id);
        let Some(mapping) = self.mapping else {
            return false;
        };
        if !mapping.contains_window_client(sample.position_px) {
            return false;
        }
        if self.blocks_canvas()
            || self.reference_under_client(sample.position_px)
            || self.floating_ui_under_client(sample.position_px)
        {
            return false;
        }
        if !active
            && (!mapping.contains_client(sample.position_px)
                || !matches!(
                    self.tool,
                    EditTool::Brush | EditTool::Smudge | EditTool::ColorMix
                ))
        {
            return false;
        }

        match session.pen_with_barrel(&mut self.barrel_router, self.barrel_action, *sample) {
            crate::barrel::BarrelEffect::None => {}
            crate::barrel::BarrelEffect::ColorPicked(_) => {
                self.context.request_repaint();
            }
            crate::barrel::BarrelEffect::ColorUnavailable => {
                self.status = "No visible color at the pen position".into();
                self.context.request_repaint();
            }
            crate::barrel::BarrelEffect::Pan(delta) => {
                let previous = self.navigation.pan;
                for (pan, change) in self.navigation.pan.iter_mut().zip(delta) {
                    *pan = (*pan + change).clamp(-32768.0, 32768.0);
                }
                self.workspace_needs_save |= self.navigation.pan != previous;
                self.context.request_repaint();
            }
        }
        true
    }
    pub fn cancel_native_barrel_pointer(&mut self, id: u64) {
        self.barrel_router.cancel_pointer(id);
    }
    pub fn cancel_native_barrel_actions(&mut self) {
        self.barrel_router.cancel_all();
    }
    fn editing_pointer(
        &mut self,
        session: &mut CanvasSession,
        id: u64,
        phase: paint_input::Phase,
        client: [f64; 2],
    ) -> bool {
        use paint_input::Phase;
        let shape_tool = matches!(
            self.tool,
            EditTool::ShapeLine | EditTool::ShapeRectangle | EditTool::ShapeEllipse
        );
        let gradient_tool = self.tool == EditTool::Gradient;
        if matches!(
            self.tool,
            EditTool::Brush | EditTool::Smudge | EditTool::ColorMix
        ) || session.mode() != crate::interaction::Mode::Live
            || self.blocks_canvas()
        {
            return false;
        }
        let captured = self.edit_pointer == Some(id);
        if self.floating_ui_under_client(client) && !captured {
            return false;
        }
        let on_canvas = self.mapping.is_some_and(|m| m.contains_client(client))
            && !self.reference_under_client(client)
            && !self.floating_ui_under_client(client);
        if !on_canvas && !captured {
            return false;
        }
        if phase == Phase::Cancel && !captured {
            return on_canvas;
        }
        if phase == Phase::Cancel {
            self.edit_pointer = None;
            self.edit_points.clear();
            if self.test_path_dialog && gradient_tool {
                crate::studio_log!("UI_TEST_GRADIENT_CANCELLED: pointer");
            }
            return true;
        }
        let Some(mut position) = self
            .mapping
            .and_then(|m| m.routing_viewport().to_document(client).ok())
            .map(|p| p.0)
        else {
            return true;
        };
        if (shape_tool || gradient_tool) && captured {
            let size = session.live().project().state();
            position[0] = position[0].clamp(0.0, f64::from(size.width));
            position[1] = position[1].clamp(0.0, f64::from(size.height));
        } else if !on_canvas {
            self.edit_pointer = None;
            self.edit_points.clear();
            return true;
        }
        match phase {
            Phase::Down if self.edit_pointer.is_none() => {
                session.mouse(MouseEvent::Interrupted, 0);
                if self.tool == EditTool::Eyedropper {
                    match session.live().project().eyedropper(position) {
                        Ok(c) => session.live_mut().project_mut().brush.color = c,
                        Err(e) => self.status = e.to_string(),
                    }
                } else {
                    self.edit_pointer = Some(id);
                    self.edit_points = vec![position, position];
                }
            }
            Phase::Move | Phase::Up if captured => {
                if shape_tool {
                    let start = self.edit_points[0];
                    let position = self.constrain_shape_endpoint(start, position);
                    self.edit_points[1] = position;
                    if phase == Phase::Up {
                        let kind = match self.tool {
                            EditTool::ShapeLine => ShapeKind::Line,
                            EditTool::ShapeRectangle => ShapeKind::Rectangle,
                            EditTool::ShapeEllipse => ShapeKind::Ellipse,
                            _ => unreachable!("shape tool checked"),
                        };
                        let result = session.live_mut().project_mut().draw_shape(
                            kind,
                            start,
                            position,
                            self.shape_filled && kind != ShapeKind::Line,
                        );
                        self.edit_pointer = None;
                        self.edit_points.clear();
                        self.status = match result {
                            Ok(()) => {
                                if self.test_path_dialog {
                                    crate::studio_log!(
                                        "UI_TEST_SHAPE_COMMITTED: {kind:?} {:.2} {:.2} {:.2} {:.2}",
                                        start[0],
                                        start[1],
                                        position[0],
                                        position[1]
                                    );
                                }
                                format!("{kind:?} committed · Ctrl+Z to undo")
                            }
                            Err(error) => error.to_string(),
                        };
                    }
                } else if gradient_tool {
                    let start = self.edit_points[0];
                    self.edit_points[1] = position;
                    if phase == Phase::Up {
                        let result = session.live_mut().project_mut().draw_gradient(
                            self.gradient_kind,
                            start,
                            position,
                            self.gradient_end_color,
                        );
                        self.edit_pointer = None;
                        self.edit_points.clear();
                        self.status = match result {
                            Ok(()) => {
                                if self.test_path_dialog {
                                    crate::studio_log!(
                                        "UI_TEST_GRADIENT_COMMITTED: {:?} {:.2} {:.2} {:.2} {:.2}",
                                        self.gradient_kind,
                                        start[0],
                                        start[1],
                                        position[0],
                                        position[1]
                                    );
                                }
                                format!(
                                    "{:?} gradient applied · Ctrl+Z to undo",
                                    self.gradient_kind
                                )
                            }
                            Err(error) => error.to_string(),
                        };
                    }
                } else if self.tool == EditTool::Lasso {
                    if append_lasso_point(&mut self.edit_points, position)
                        == LassoAppend::LimitReached
                    {
                        self.edit_pointer = None;
                        self.edit_points.clear();
                        self.status =
                            "Lasso cancelled: simplify the outline to 1024 points or fewer".into();
                        return true;
                    }
                } else {
                    self.edit_points[1] = position;
                }
                if phase == Phase::Up && !shape_tool && !gradient_tool {
                    let kind = match self.tool {
                        EditTool::Rectangle => SelectionKind::Rectangle,
                        EditTool::Ellipse => SelectionKind::Ellipse,
                        _ => SelectionKind::Lasso,
                    };
                    let result = Selection::new(kind, std::mem::take(&mut self.edit_points))
                        .and_then(|s| session.live_mut().project_mut().set_selection(Some(s)));
                    self.edit_pointer = None;
                    self.status = match result {
                        Ok(()) => "Selection ready: brush, fill, delete or transform".into(),
                        Err(e) => e.to_string(),
                    };
                    if self.test_path_dialog {
                        crate::studio_log!(
                            "UI_TEST_SELECTION_COMMITTED: {} {}",
                            session.live().project().state().selection.is_some(),
                            self.status
                        );
                    }
                }
            }
            _ => {}
        }
        true
    }

    fn constrain_shape_endpoint(&self, start: [f64; 2], mut end: [f64; 2]) -> [f64; 2] {
        if !self.context.input(|input| input.modifiers.shift) {
            return end;
        }
        match self.tool {
            EditTool::ShapeLine => {
                let delta = [end[0] - start[0], end[1] - start[1]];
                let length = delta[0].hypot(delta[1]);
                let quantum = std::f64::consts::FRAC_PI_4;
                let angle = (delta[1].atan2(delta[0]) / quantum).round() * quantum;
                end = [
                    start[0] + length * angle.cos(),
                    start[1] + length * angle.sin(),
                ];
            }
            EditTool::ShapeRectangle | EditTool::ShapeEllipse => {
                let delta = [end[0] - start[0], end[1] - start[1]];
                let side = delta[0].abs().max(delta[1].abs());
                end = [
                    start[0] + if delta[0] < 0.0 { -side } else { side },
                    start[1] + if delta[1] < 0.0 { -side } else { side },
                ];
            }
            _ => {}
        }
        let mapping = self.mapping.expect("shape tools require a canvas mapping");
        let [width, height] = mapping.document_size();
        end[0] = end[0].clamp(0.0, width);
        end[1] = end[1].clamp(0.0, height);
        end
    }
    pub fn tool_shortcut(&mut self, key: char, session: &mut CanvasSession) {
        session.mouse(MouseEvent::Interrupted, 0);
        self.edit_pointer = None;
        self.edit_points.clear();
        if key == 'c' {
            self.color_flyout_open = !self.color_flyout_open;
            self.brush_flyout_open = false;
            if self.test_path_dialog && self.color_flyout_open {
                crate::studio_log!("UI_TEST_COLOR_FLYOUT_OPENED: shortcut");
            }
            return;
        }
        if key == '0' {
            self.navigation = Default::default();
            self.workspace_needs_save = true;
            return;
        }
        self.tool = match key {
            's' => EditTool::Smudge,
            'x' => EditTool::ColorMix,
            'm' => EditTool::Rectangle,
            'e' => EditTool::Ellipse,
            'k' => EditTool::Lasso,
            'i' => EditTool::Eyedropper,
            'l' => EditTool::ShapeLine,
            'r' => EditTool::ShapeRectangle,
            'o' => EditTool::ShapeEllipse,
            'g' => EditTool::Gradient,
            _ => EditTool::Brush,
        };
        session
            .live_mut()
            .set_stroke_mode(stroke_mode_for_tool(self.tool, self.color_mix_strength));
        if matches!(
            self.tool,
            EditTool::ShapeLine
                | EditTool::ShapeRectangle
                | EditTool::ShapeEllipse
                | EditTool::Gradient
        ) {
            self.brush_flyout_open = false;
            self.color_flyout_open = false;
        }
        if self.test_path_dialog {
            crate::studio_log!("UI_TEST_ACTIVE_TOOL: {:?}", self.tool);
        }
    }
    pub fn adjust_brush_size(&mut self, increase: bool, fine: bool, session: &mut CanvasSession) {
        let brush = &mut session.live_mut().project_mut().brush;
        brush.size = brush_size_after_shortcut(brush.size, increase, fine);
        self.status = format!("Brush size {:.1} px", brush.size);
        if self.test_path_dialog {
            crate::studio_log!("UI_TEST_BRUSH_SIZE: {:.3}", brush.size);
        }
    }
    fn remember(&mut self, path: &std::path::Path) {
        let result = self.recent.remember(path).and_then(|_| {
            self.recent_path
                .as_ref()
                .ok_or(paint_document::PaintError::Invalid)
                .and_then(|p| self.recent.save(p))
        });
        if let Err(e) = result {
            self.status = format!("Artwork file succeeded; recent-list update failed: {e}");
        }
        self.recent_cards = recent_project_cards(
            &self.recent,
            &self.context,
            self.thumbnail_directory.as_deref(),
        );
    }
    fn cache_home_thumbnail(&mut self, path: &std::path::Path, engine: &Engine) {
        if let Some(directory) = &self.thumbnail_directory
            && let Err(error) = crate::home_thumbnails::save(directory, path, engine)
        {
            self.status = format!("Artwork saved; Home preview was not cached: {error}");
        }
    }
    pub fn project_label(&self) -> String {
        self.store
            .as_ref()
            .and_then(|s| s.path.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }
    pub fn test_prompt_label(&self) -> Option<&'static str> {
        if !self.test_path_dialog {
            None
        } else if self.manual_file_dialog {
            Some("TEST FILE PICKER")
        } else if self.new_dialog {
            Some("TEST NEW DOCUMENT")
        } else {
            None
        }
    }
    pub fn blocks_canvas(&self) -> bool {
        self.welcome
            || self.new_dialog
            || self.picker.is_some()
            || self.manual_file_dialog
            || self.confirm.is_some()
            || self.close_requested
            || self.recovery_manager
            || self.cleanup.is_some()
    }
    pub fn shortcut(&mut self, key: char, session: &mut CanvasSession) {
        session.mouse(MouseEvent::Interrupted, 0);
        if key == 'b' {
            self.brush_flyout_open = !self.brush_flyout_open;
            self.color_flyout_open = false;
            if self.test_path_dialog && self.brush_flyout_open {
                crate::studio_log!("UI_TEST_BRUSH_FLYOUT_OPENED: shortcut");
            }
            return;
        }
        if key == 'l' {
            self.layers_flyout_open = !self.layers_flyout_open;
            return;
        }
        if key == 'i' {
            self.show_controls = !self.show_controls;
            self.workspace_needs_save = true;
            return;
        }
        if key == 'g' {
            if self.recovery_manager {
                self.recovery_manager = false;
                if self.recovery_manager_from_home {
                    self.welcome = true;
                }
                self.recovery_manager_from_home = false;
            } else {
                self.recovery_files = paint_document::recovery::directory()
                    .map(|directory| paint_document::recovery::available(&directory))
                    .unwrap_or_default();
                self.recovery_preview = None;
                self.recovery_manager = true;
                self.recovery_manager_from_home = false;
            }
            return;
        }
        if key == 't' || key == 'u' {
            self.path = PathBuf::from(&self.path)
                .with_extension("rplbrush")
                .to_string_lossy()
                .into();
            self.picker = Some(if key == 't' {
                FileRequest::SaveBrush
            } else {
                FileRequest::LoadBrush
            });
            return;
        }
        if key == 'v' {
            if let Some(path) = self.recovery_files.first().cloned() {
                if session.live().project().dirty() {
                    self.confirm = Some(Action::Recover(path));
                } else if let Err(e) = self.execute(Action::Recover(path), session) {
                    self.status = e.to_string();
                }
            } else {
                self.status = "No inactive recovery checkpoints found".into();
            }
            return;
        }
        if key == 'o' || key == 'a' {
            self.picker = Some(if key == 'a' {
                FileRequest::ImportOra
            } else {
                FileRequest::OpenProject
            });
            return;
        }
        if key == 'n' {
            self.new_from_welcome = self.welcome;
            self.welcome = false;
            self.new_dialog = true;
            return;
        }
        if key == 'j' {
            if let Err(error) = self.execute(Action::Add, session) {
                self.status = error.to_string();
            }
            if self.test_path_dialog {
                crate::studio_log!(
                    "UI_TEST_LAYERS_ADDED: {}",
                    session.live().project().state().layers.len()
                );
            }
            self.layers_flyout_open = true;
            return;
        }
        if key == 'q' {
            self.path = PathBuf::from(&self.path)
                .with_extension("ora")
                .to_string_lossy()
                .into();
        }
        if key == 'e' {
            self.path = PathBuf::from(&self.path)
                .with_extension("png")
                .to_string_lossy()
                .into();
        }
        let action = match key {
            'z' => Action::Undo,
            'y' => Action::Redo,
            's' => Action::Save,
            'e' => Action::Export,
            'q' => Action::ExportOra,
            'f' => Action::Fill(false),
            'd' => Action::Deselect,
            'x' => Action::Fill(true),
            _ => return,
        };
        if let Err(e) = self.execute(action, session) {
            self.status = e.to_string();
        }
    }

    /// Dismiss the top contextual panel before Escape can close the application.
    pub fn dismiss_flyout(&mut self) -> bool {
        if egui::Popup::is_any_open(&self.context) {
            egui::Popup::close_all(&self.context);
            if self.test_path_dialog {
                crate::studio_log!("UI_TEST_ESCAPE_DISMISSED: menu");
            }
            return true;
        }
        if self.edit_pointer.take().is_some() {
            self.edit_points.clear();
            self.status = if self.tool == EditTool::Gradient {
                "Gradient preview cancelled".into()
            } else {
                "Shape preview cancelled".into()
            };
            if self.test_path_dialog {
                if self.tool == EditTool::Gradient {
                    crate::studio_log!("UI_TEST_GRADIENT_CANCELLED: escape");
                } else {
                    crate::studio_log!("UI_TEST_SHAPE_CANCELLED");
                }
            }
            return true;
        }
        let dismissed = if self.brush_flyout_open {
            self.brush_flyout_open = false;
            "brush"
        } else if self.color_flyout_open {
            self.color_flyout_open = false;
            "color"
        } else if self.layers_flyout_open {
            self.layers_flyout_open = false;
            "layers"
        } else if self.show_controls {
            self.show_controls = false;
            "workspace controls"
        } else {
            "none"
        };
        if dismissed != "none" {
            self.workspace_needs_save = true;
        }
        if self.test_path_dialog {
            crate::studio_log!("UI_TEST_ESCAPE_DISMISSED: {dismissed}");
        }
        dismissed != "none"
    }
    pub fn preserve(&self, session: &CanvasSession) -> Result<(), Failure> {
        if session.live().project().dirty() {
            if let Some(r) = &self.recovery {
                r.flush(session.live().project().state().clone())?;
            } else {
                return Err(
                    "Cannot preserve unsaved artwork: recovery unavailable; save the project first"
                        .into(),
                );
            }
        }
        Ok(())
    }
    pub fn requires_close_save(&self, session: &CanvasSession) -> bool {
        !self.welcome && (session.live().project().dirty() || self.store.is_none())
    }
    /// Persist artwork before normal close. A conflict/failure leaves the window
    /// open so the user can retry, choose Save As, or cancel.
    pub fn save_for_close(&mut self, session: &mut CanvasSession) -> Result<(), Failure> {
        if self.store.is_some() {
            self.execute(Action::Save, session)?;
        } else {
            let (store, path) = {
                let project = session.live_mut().project_mut();
                project.sync_brush_settings();
                let state = project.state().clone();
                let (store, path) = crate::drafts::create(&state, &self.new_name)?;
                project.saved_revision = project.revision;
                (store, path)
            };
            self.path = path.to_string_lossy().into_owned();
            self.status = format!(
                "Draft saved locally: {}",
                crate::drafts::display_name(&path)
            );
            self.store = Some(store);
            self.drafts = crate::drafts::list();
        }
        self.close_ready = true;
        self.close_requested = false;
        Ok(())
    }
    pub fn mark_close_save_failed(&mut self, error: impl std::fmt::Display) {
        self.status = format!("Automatic save failed: {error}");
        self.close_requested = true;
    }
    pub fn needs_autosave(&self, session: &CanvasSession) -> bool {
        self.autosave_deadline(session)
            .is_some_and(|deadline| Instant::now() >= deadline)
    }
    pub fn autosave_deadline(&self, session: &CanvasSession) -> Option<Instant> {
        let project = session.live().project();
        (self.recovery.is_some()
            && !session.live().active()
            && project.dirty()
            && self.last_submitted != project.revision)
            .then(|| self.autosave_schedule.deadline())
            .flatten()
    }
    pub fn recovery_save_pending(&self) -> bool {
        self.recovery_saving
    }
    fn persist_workspace(&self) -> Result<(), String> {
        let Some(path) = &self.preferences_path else {
            return Err("Workspace preference directory is unavailable".into());
        };
        let mut preferences = crate::workspace::WorkspacePreferences {
            dark_theme: self.appearance_mode.is_dark(self.system_dark),
            appearance_mode: Some(self.appearance_mode),
            accent_scheme: self.accent_scheme,
            custom_accent_rgb: self.custom_accent_rgb,
            brush_panel_width: self.brush_panel_width,
            show_controls: self.show_controls,
            references: self
                .references
                .iter()
                .map(|reference| reference.layout.clone())
                .collect(),
            new_document: crate::workspace::NewDocumentPreferences {
                width: self.new_width,
                height: self.new_height,
                dpi: self.new_dpi,
                unit: self.new_unit,
                background: self.new_background,
                transparent: self.new_transparent,
            },
            recent_document_sizes: self.recent_document_sizes.clone(),
            recent_brush_colors: self.recent_brush_colors.clone(),
            saved_color_swatches: self.saved_color_swatches.clone(),
            favorite_brushes: self.favorite_brushes.clone(),
            color_mix_strength: self.color_mix_strength,
            barrel_action: self.barrel_action,
            canvas_zoom: self.navigation.zoom,
            canvas_pan: self.navigation.pan,
            inspector_tab: self.inspector_tab,
            high_contrast: self.high_contrast,
            reduced_motion: self.reduced_motion,
            ui_scale: self.ui_scale,
            palette_dock: self.palette_dock,
            palette_collapsed: self.palette_collapsed,
            handedness: self.handedness,
            palette_position: self.palette_position,
            layers_flyout_position: self.layers_flyout_position,
            layers_flyout_size: self.layers_flyout_size,
            brush_flyout_position: self.brush_flyout_position,
            color_flyout_position: self.color_flyout_position,
            workspace_controls_position: self.workspace_controls_position,
            workspace_controls_size: self.workspace_controls_size,
            ..Default::default()
        };
        if self.test_path_dialog {
            crate::studio_log!(
                "UI_TEST_WORKSPACE_SAVE: references={}",
                preferences.references.len()
            );
        }
        if !preferences.validate() {
            return Err("Workspace preferences failed validation".into());
        }
        let bytes = serde_json::to_vec_pretty(&preferences).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
        std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        #[cfg(windows)]
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    }
    fn widgets(
        &mut self,
        ctx: &egui::Context,
        session: &mut CanvasSession,
        actions: &mut Vec<Action>,
        central: &mut egui::Rect,
        approved: &mut bool,
    ) {
        let mut workspace_changed = self.workspace_needs_save
            && self.touch_routes.active_target().is_none()
            && self.pan_cursor.is_none();
        if workspace_changed {
            self.workspace_needs_save = false;
        }
        if !self.welcome {
            egui::TopBottomPanel::top("document tools").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    crate::ui_tokens::brand_wordmark(ui, 18.0);
                    ui.separator();
                    egui::MenuBar::new().ui(ui, |ui| {
                        ui.menu_button("File", |ui| {
                            if ui.button("New document…     Ctrl+N").clicked() {
                                self.new_from_welcome = false;
                                self.new_dialog = true;
                                ui.close();
                            }
                            if crate::studio_icons::menu_button(
                                ui,
                                crate::studio_icons::Icon::Open,
                                "Open project…     Ctrl+O",
                            )
                            .clicked()
                            {
                                self.picker_from_welcome = false;
                                self.picker = Some(FileRequest::OpenProject);
                                ui.close();
                            }
                            ui.separator();
                            if crate::studio_icons::menu_button(
                                ui,
                                crate::studio_icons::Icon::Save,
                                "Save     Ctrl+S",
                            )
                            .clicked()
                            {
                                actions.push(Action::Save);
                                ui.close();
                            }
                            if ui.button("Save As…").clicked() {
                                self.picker = Some(FileRequest::SaveProjectAs);
                                ui.close();
                            }
                            ui.menu_button("Import", |ui| {
                                if ui.button("Raster image (PNG)…").clicked() {
                                    self.picker = Some(FileRequest::ImportPng);
                                    ui.close();
                                }
                                if ui.button("OpenRaster project…").clicked() {
                                    self.picker = Some(FileRequest::ImportOra);
                                    ui.close();
                                }
                                if ui.button("Reference images…").clicked() {
                                    self.picker = Some(FileRequest::AddReferences);
                                    ui.close();
                                }
                            });
                            ui.menu_button("Export", |ui| {
                                if ui.button("PNG image…     Ctrl+E").clicked() {
                                    self.path = PathBuf::from(&self.path)
                                        .with_extension("png")
                                        .to_string_lossy()
                                        .into();
                                    self.picker = Some(FileRequest::ExportPng);
                                    ui.close();
                                }
                                if ui.button("OpenRaster project…     Ctrl+Shift+E").clicked() {
                                    self.path = PathBuf::from(&self.path)
                                        .with_extension("ora")
                                        .to_string_lossy()
                                        .into();
                                    self.picker = Some(FileRequest::ExportOra);
                                    ui.close();
                                }
                            });
                            ui.separator();
                            if ui.button("Load brush preset…     Ctrl+Alt+B").clicked() {
                                self.path = PathBuf::from(&self.path)
                                    .with_extension("rplbrush")
                                    .to_string_lossy()
                                    .into();
                                self.picker = Some(FileRequest::LoadBrush);
                                ui.close();
                            }
                            if ui.button("Save brush preset…     Ctrl+Shift+B").clicked() {
                                self.path = PathBuf::from(&self.path)
                                    .with_extension("rplbrush")
                                    .to_string_lossy()
                                    .into();
                                self.picker = Some(FileRequest::SaveBrush);
                                ui.close();
                            }
                            if !self.recent.paths.is_empty() {
                                ui.menu_button("Recent projects", |ui| {
                                    for path in self.recent.paths.clone().into_iter().take(8) {
                                        let label =
                                            path.file_name().unwrap_or_default().to_string_lossy();
                                        if ui
                                            .button(label)
                                            .on_hover_text(path.display().to_string())
                                            .clicked()
                                        {
                                            actions.push(Action::Recent(path));
                                            ui.close();
                                        }
                                    }
                                });
                            }
                            if ui.button("Recovery manager…").clicked() {
                                self.recovery_files = paint_document::recovery::directory()
                                    .map(|directory| {
                                        paint_document::recovery::available(&directory)
                                    })
                                    .unwrap_or_default();
                                self.recovery_preview = None;
                                self.recovery_manager = true;
                                self.recovery_manager_from_home = false;
                                ui.close();
                            }
                        });
                        ui.menu_button("Edit", |ui| {
                            if crate::studio_icons::menu_button_enabled(
                                ui,
                                crate::studio_icons::Icon::Undo,
                                "Undo     Ctrl+Z",
                                session.live().project().undo_count() > 0,
                            )
                            .clicked()
                            {
                                actions.push(Action::Undo);
                                ui.close();
                            }
                            if crate::studio_icons::menu_button_enabled(
                                ui,
                                crate::studio_icons::Icon::Redo,
                                "Redo     Ctrl+Y",
                                session.live().project().redo_count() > 0,
                            )
                            .clicked()
                            {
                                actions.push(Action::Redo);
                                ui.close();
                            }
                            ui.separator();
                            if crate::studio_icons::menu_button(
                                ui,
                                crate::studio_icons::Icon::Deselect,
                                "Deselect     Ctrl+D",
                            )
                            .clicked()
                            {
                                actions.push(Action::Deselect);
                                ui.close();
                            }
                            if crate::studio_icons::menu_button(
                                ui,
                                crate::studio_icons::Icon::Fill,
                                "Fill selection     Ctrl+F",
                            )
                            .clicked()
                            {
                                actions.push(Action::Fill(false));
                                ui.close();
                            }
                            if crate::studio_icons::menu_button(
                                ui,
                                crate::studio_icons::Icon::Eraser,
                                "Erase selection     Ctrl+X",
                            )
                            .clicked()
                            {
                                actions.push(Action::Fill(true));
                                ui.close();
                            }
                        });
                        ui.menu_button("Image", |ui| {
                            if ui.button("Crop to selection").clicked() {
                                actions.push(Action::Crop);
                                ui.close();
                            }
                        });
                        ui.menu_button("Layer", |ui| {
                            if ui.button("New layer").clicked() {
                                actions.push(Action::Add);
                                ui.close();
                            }
                            if ui.button("Duplicate selected layer").clicked() {
                                actions.push(Action::Duplicate);
                                ui.close();
                            }
                            if ui.button("Delete selected layer").clicked() {
                                actions.push(Action::Delete);
                                ui.close();
                            }
                            if ui.button("Clear selected layer").clicked() {
                                actions.push(Action::Clear);
                                ui.close();
                            }
                        });
                        ui.menu_button("View", |ui| {
                            ui.label("Appearance");
                            for (mode, label) in [
                                (crate::workspace::AppearanceMode::Dark, "Dark"),
                                (crate::workspace::AppearanceMode::Light, "Light"),
                                (
                                    crate::workspace::AppearanceMode::FollowSystem,
                                    "Follow System",
                                ),
                            ] {
                                workspace_changed |= ui
                                    .selectable_value(&mut self.appearance_mode, mode, label)
                                    .changed();
                            }
                            ui.separator();
                            ui.label("Windows S Pen barrel button");
                            for (action, label) in [
                                (crate::barrel::BarrelAction::Eraser, "Hold for Eraser"),
                                (crate::barrel::BarrelAction::Eyedropper, "Hold for Eyedropper"),
                                (crate::barrel::BarrelAction::Pan, "Hold to Pan"),
                                (crate::barrel::BarrelAction::Disabled, "Disabled"),
                            ] {
                                workspace_changed |= ui
                                    .selectable_value(&mut self.barrel_action, action, label)
                                    .changed();
                            }
                            ui.separator();
                            ui.label("Accent color");
                            egui::ComboBox::from_id_salt("studio accent scheme")
                                .selected_text(match self.accent_scheme {
                                    crate::workspace::AccentScheme::Violet => "Violet",
                                    crate::workspace::AccentScheme::Blue => "Blue",
                                    crate::workspace::AccentScheme::Teal => "Teal",
                                    crate::workspace::AccentScheme::Coral => "Coral",
                                    crate::workspace::AccentScheme::Green => "Green",
                                    crate::workspace::AccentScheme::Neutral => "Neutral",
                                    crate::workspace::AccentScheme::Custom => "Custom",
                                })
                                .show_ui(ui, |ui| {
                                    for (scheme, label) in [
                                        (crate::workspace::AccentScheme::Violet, "Violet"),
                                        (crate::workspace::AccentScheme::Blue, "Blue"),
                                        (crate::workspace::AccentScheme::Teal, "Teal"),
                                        (crate::workspace::AccentScheme::Coral, "Coral"),
                                        (crate::workspace::AccentScheme::Green, "Green"),
                                        (crate::workspace::AccentScheme::Neutral, "Neutral"),
                                        (crate::workspace::AccentScheme::Custom, "Custom HEX…"),
                                    ] {
                                        workspace_changed |= ui
                                            .selectable_value(
                                                &mut self.accent_scheme,
                                                scheme,
                                                label,
                                            )
                                            .changed();
                                    }
                                });
                            if self.accent_scheme == crate::workspace::AccentScheme::Custom
                                && self.custom_accent_rgb.is_none()
                            {
                                self.custom_accent_rgb = Some([0xA7, 0x8B, 0xFA]);
                                workspace_changed = true;
                            }
                            if self.accent_scheme == crate::workspace::AccentScheme::Custom {
                                ui.horizontal(|ui| {
                                    ui.label("HEX");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.custom_accent_hex)
                                            .char_limit(7)
                                            .desired_width(88.0),
                                    );
                                    let preview = crate::color_controls::parse_hex_rgb(
                                        &self.custom_accent_hex,
                                    )
                                    .or(self.custom_accent_rgb)
                                    .unwrap_or([0xA7, 0x8B, 0xFA]);
                                    let (preview_rect, _) = ui.allocate_exact_size(
                                        egui::vec2(24.0, 24.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(
                                        preview_rect.shrink(2.0),
                                        4.0,
                                        egui::Color32::from_rgb(preview[0], preview[1], preview[2]),
                                    );
                                    ui.painter().rect_stroke(
                                        preview_rect.shrink(2.0),
                                        4.0,
                                        ui.visuals().widgets.noninteractive.bg_stroke,
                                        egui::StrokeKind::Inside,
                                    );
                                    if ui.button("Apply").clicked() {
                                        if let Some(color) = crate::color_controls::parse_hex_rgb(
                                            &self.custom_accent_hex,
                                        ) {
                                            self.custom_accent_rgb = Some(color);
                                            workspace_changed = true;
                                            self.status = "Studio accent updated".into();
                                        } else {
                                            self.status =
                                                "Enter a six-digit accent such as #C55CFF".into();
                                        }
                                    }
                                });
                            }
                            ui.separator();
                            workspace_changed |= ui
                                .checkbox(&mut self.high_contrast, "High contrast controls")
                                .changed();
                            workspace_changed |= ui
                                .checkbox(&mut self.reduced_motion, "Reduce interface motion")
                                .changed();
                            ui.horizontal(|ui| {
                                ui.label("Interface size");
                                workspace_changed |= ui
                                    .add(
                                        egui::Slider::new(&mut self.ui_scale, 0.8..=1.6)
                                            .show_value(true),
                                    )
                                    .changed();
                            });
                            if ui.button("Reset appearance").clicked() {
                                self.appearance_mode = crate::workspace::AppearanceMode::Dark;
                                self.accent_scheme = crate::workspace::AccentScheme::Violet;
                                self.custom_accent_rgb = None;
                                self.custom_accent_hex = "#C55CFF".into();
                                self.high_contrast = false;
                                self.reduced_motion = false;
                                self.ui_scale = 1.0;
                                workspace_changed = true;
                                ui.close();
                            }
                            ui.separator();
                            ui.label("0  Fit canvas");
                            ui.label("F11  Full screen");
                        });
                        ui.menu_button("Window", |ui| {
                            workspace_changed |= ui
                                .checkbox(
                                    &mut self.show_controls,
                                    "Floating workspace controls · F6",
                                )
                                .changed();
                            ui.checkbox(&mut self.layers_flyout_open, "Show floating Layers panel");
                            if ui.button("Reset workspace layout").clicked() {
                                self.show_controls = true;
                                self.brush_panel_width = 260.0;
                                self.inspector_tab = crate::workspace::InspectorTab::Layers;
                                self.palette_dock = match self.handedness {
                                    crate::workspace::Handedness::Right => {
                                        crate::workspace::PaletteDock::Left
                                    }
                                    crate::workspace::Handedness::Left => {
                                        crate::workspace::PaletteDock::Right
                                    }
                                };
                                self.palette_collapsed = false;
                                self.palette_position = [72.0, 64.0];
                                self.navigation = Default::default();
                                for (index, reference) in self.references.iter_mut().enumerate() {
                                    reference.layout.position =
                                        [40.0 + index as f32 * 28.0, 80.0 + index as f32 * 24.0];
                                    reference.layout.size = [360.0, 300.0];
                                    reference.layout.view_zoom = 1.0;
                                    reference.layout.view_pan = [0.0, 0.0];
                                    reference.layout.position_locked = false;
                                }
                                workspace_changed = true;
                                ui.close();
                            }
                            ui.separator();
                            ui.label("Tool palette");
                            for (dock, label) in [
                                (crate::workspace::PaletteDock::Left, "Dock left"),
                                (crate::workspace::PaletteDock::Right, "Dock right"),
                                (crate::workspace::PaletteDock::Top, "Dock top"),
                                (crate::workspace::PaletteDock::Bottom, "Dock bottom"),
                                (crate::workspace::PaletteDock::Floating, "Float palette"),
                            ] {
                                if ui
                                    .selectable_value(&mut self.palette_dock, dock, label)
                                    .changed()
                                {
                                    workspace_changed = true;
                                }
                            }
                            ui.label("Handedness");
                            for (hand, label) in [
                                (crate::workspace::Handedness::Right, "Right handed"),
                                (crate::workspace::Handedness::Left, "Left handed"),
                            ] {
                                if ui
                                    .selectable_value(&mut self.handedness, hand, label)
                                    .changed()
                                {
                                    if matches!(
                                        self.palette_dock,
                                        crate::workspace::PaletteDock::Left
                                            | crate::workspace::PaletteDock::Right
                                    ) {
                                        self.palette_dock =
                                            if hand == crate::workspace::Handedness::Right {
                                                crate::workspace::PaletteDock::Left
                                            } else {
                                                crate::workspace::PaletteDock::Right
                                            };
                                    }
                                    workspace_changed = true;
                                }
                            }
                            ui.separator();
                            let can_add = self.references.len() < crate::workspace::MAX_REFERENCES;
                            if ui
                                .add_enabled(can_add, egui::Button::new("Add reference images…"))
                                .clicked()
                            {
                                self.picker = Some(FileRequest::AddReferences);
                                ui.close();
                            }
                            ui.label(format!(
                                "Reference board: {} / {}",
                                self.references.len(),
                                crate::workspace::MAX_REFERENCES
                            ));
                            ui.label("Brush B · Color C · Layers Ctrl+L");
                        });
                        ui.menu_button("Help", |ui| {
                            ui.label("BitchPixel — painting without permission");
                            ui.separator();
                            ui.label("Brush size [ / ] · fine adjustment: Shift+[ / Shift+]");
                            ui.label("B Brush · G Gradient · C Color · M / E selection");
                            ui.label("K Lasso · I Eyedropper · S Smudge · X Color Mixer · L Line · R Rectangle · O Ellipse");
                            ui.label("Shift constrains line angles and shape proportions");
                            ui.label("Ctrl+N New · Ctrl+O Open · Ctrl+S Save");
                            ui.label("Ctrl+L Floating Layers panel");
                            ui.label("Ctrl+Shift+N New layer");
                            ui.label("Ctrl+Shift+M Artwork recovery manager");
                            ui.label("Ctrl+Z / Ctrl+Y Undo / Redo · Esc Close");
                        });
                    });
                });
            });

            let contextual_options = egui::TopBottomPanel::top("contextual tool options")
                .resizable(false)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(ui.visuals().panel_fill)
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if self.tool == EditTool::Brush {
                                    let project = session.live_mut().project_mut();
                                    let preset_picker = ui.menu_button(
                                        brush_display_name(project.brush.kind),
                                        |ui| {
                                        for kind in [
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
                                        ] {
                                            let preset = ui.selectable_label(
                                                project.brush.kind == kind,
                                                brush_display_name(kind),
                                            );
                                            if self.test_path_dialog {
                                                crate::studio_log!(
                                                    "UI_TEST_QUICK_PRESET_RECT: {kind:?} {:.1} {:.1} {:.1} {:.1} {:.2}",
                                                    preset.rect.left(),
                                                    preset.rect.top(),
                                                    preset.rect.width(),
                                                    preset.rect.height(),
                                                    ui.ctx().pixels_per_point(),
                                                );
                                            }
                                            if preset.clicked() {
                                                activate_brush_preset(&mut project.brush, kind);
                                                if self.test_path_dialog {
                                                    crate::studio_log!("UI_TEST_QUICK_BRUSH_SELECTED: {kind:?}");
                                                }
                                                ui.close();
                                            }
                                        }
                                    },
                                    );
                                    if self.test_path_dialog {
                                        crate::studio_log!(
                                            "UI_TEST_QUICK_BRUSH_MENU_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                            preset_picker.response.rect.left(),
                                            preset_picker.response.rect.top(),
                                            preset_picker.response.rect.width(),
                                            preset_picker.response.rect.height(),
                                            ctx.pixels_per_point(),
                                        );
                                    }
                                    ui.add_sized(
                                        [176.0, 34.0],
                                        egui::Slider::new(&mut project.brush.size, 1.0..=256.0)
                                            .text("Size"),
                                    );
                                    ui.add_sized(
                                        [148.0, 34.0],
                                        egui::Slider::new(&mut project.brush.opacity, 0.0..=1.0)
                                            .text("Opacity"),
                                    );
                                    let color = project.brush.color;
                                    if color_swatch(
                                        ui,
                                        color,
                                        "Current brush color · open color controls",
                                        false,
                                        34.0,
                                    ) {
                                        self.color_flyout_open = !self.color_flyout_open;
                                        self.brush_flyout_open = false;
                                    }
                                } else if self.tool == EditTool::Smudge {
                                    let project = session.live_mut().project_mut();
                                    let control_width = if ui.available_width() < 800.0 {
                                        112.0
                                    } else {
                                        148.0
                                    };
                                    ui.strong("Smudge").on_hover_text(
                                        "Smears pixels on the active unlocked layer; the current selection clips the stroke.",
                                    );
                                    ui.add_sized(
                                        [control_width, 34.0],
                                        egui::Slider::new(&mut project.brush.size, 1.0..=256.0)
                                            .text("Size"),
                                    );
                                    ui.add_sized(
                                        [control_width, 34.0],
                                        egui::Slider::new(&mut project.brush.opacity, 0.0..=1.0)
                                            .text("Strength"),
                                    );
                                    ui.add_sized(
                                        [control_width, 34.0],
                                        egui::Slider::new(&mut project.brush.flow, 0.0..=1.0)
                                            .text("Pickup"),
                                    );
                                    ui.menu_button("Dynamics", |ui| {
                                        ui.checkbox(
                                            &mut project.brush.pressure_size,
                                            "Pressure size",
                                        );
                                        ui.checkbox(
                                            &mut project.brush.pressure_opacity,
                                            "Pressure strength",
                                        );
                                    });
                                } else if self.tool == EditTool::ColorMix {
                                    ui.strong("Color Mixer").on_hover_text(
                                        "Blends the active paint color into the selected layer. Opacity and Paint loading control how much pigment is deposited; Strength controls how far its color moves toward your chosen color.",
                                    );
                                    let project = session.live_mut().project_mut();
                                    let control_width = if ui.available_width() < 800.0 {
                                        92.0
                                    } else {
                                        126.0
                                    };
                                    let flow_width = if ui.available_width() < 800.0 {
                                        100.0
                                    } else {
                                        140.0
                                    };
                                    ui.add_sized(
                                        [control_width, 34.0],
                                        egui::Slider::new(&mut project.brush.size, 1.0..=256.0)
                                            .text("Size"),
                                    );
                                    ui.add_sized(
                                        [control_width, 34.0],
                                        egui::Slider::new(&mut project.brush.opacity, 0.0..=1.0)
                                            .text("Opacity"),
                                    );
                                    ui.add_sized(
                                        [flow_width, 34.0],
                                        egui::Slider::new(&mut project.brush.flow, 0.0..=1.0)
                                            .text("Paint loading"),
                                    );
                                    if ui
                                        .add_sized(
                                            [flow_width, 34.0],
                                            egui::Slider::new(
                                                &mut self.color_mix_strength,
                                                0.0..=1.0,
                                            )
                                            .text("Mix strength"),
                                        )
                                        .changed()
                                    {
                                        workspace_changed = true;
                                    }
                                    ui.menu_button("Dynamics", |ui| {
                                        ui.checkbox(
                                            &mut project.brush.pressure_size,
                                            "Pressure size",
                                        );
                                        ui.checkbox(
                                            &mut project.brush.pressure_opacity,
                                            "Pressure opacity",
                                        );
                                    });
                                } else if matches!(
                                    self.tool,
                                    EditTool::ShapeLine
                                        | EditTool::ShapeRectangle
                                        | EditTool::ShapeEllipse
                                ) {
                                    let kind = match self.tool {
                                        EditTool::ShapeLine => ShapeKind::Line,
                                        EditTool::ShapeRectangle => ShapeKind::Rectangle,
                                        EditTool::ShapeEllipse => ShapeKind::Ellipse,
                                        _ => unreachable!("shape tool checked"),
                                    };
                                    ui.strong(match kind {
                                        ShapeKind::Line => "Line tool",
                                        ShapeKind::Rectangle => "Rectangle tool",
                                        ShapeKind::Ellipse => "Ellipse tool",
                                    });
                                    let project = session.live_mut().project_mut();
                                    ui.add_sized(
                                        [176.0, 34.0],
                                        egui::Slider::new(&mut project.brush.size, 1.0..=256.0)
                                            .text("Stroke width"),
                                    );
                                    if kind != ShapeKind::Line {
                                        ui.checkbox(&mut self.shape_filled, "Fill shape");
                                    }
                                    let color = project.brush.color;
                                    if color_swatch(
                                        ui,
                                        color,
                                        "Current shape color · open color controls",
                                        false,
                                        32.0,
                                    ) {
                                        self.color_flyout_open = !self.color_flyout_open;
                                        self.brush_flyout_open = false;
                                    }
                                    ui.label("Shift constrains angle or proportions");
                                } else if self.tool == EditTool::Gradient {
                                    ui.strong("Gradient");
                                    let linear = ui.selectable_value(
                                        &mut self.gradient_kind,
                                        GradientKind::Linear,
                                        "Linear",
                                    );
                                    if self.test_path_dialog {
                                        crate::studio_log!(
                                            "UI_TEST_GRADIENT_LINEAR_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                            linear.rect.left(),
                                            linear.rect.top(),
                                            linear.rect.width(),
                                            linear.rect.height(),
                                            ui.ctx().pixels_per_point()
                                        );
                                    }
                                    let radial = ui.selectable_value(
                                        &mut self.gradient_kind,
                                        GradientKind::Radial,
                                        "Radial",
                                    );
                                    if self.test_path_dialog {
                                        crate::studio_log!(
                                            "UI_TEST_GRADIENT_RADIAL_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                            radial.rect.left(),
                                            radial.rect.top(),
                                            radial.rect.width(),
                                            radial.rect.height(),
                                            ui.ctx().pixels_per_point()
                                        );
                                        crate::studio_log!("UI_TEST_GRADIENT_MODE: {:?}", self.gradient_kind);
                                    }
                                    let project = session.live_mut().project_mut();
                                    let start_color = project.brush.color;
                                    if color_swatch(
                                        ui,
                                        start_color,
                                        "Gradient start · current brush color",
                                        false,
                                        30.0,
                                    ) {
                                        self.color_flyout_open = true;
                                        self.brush_flyout_open = false;
                                    }
                                    ui.label("Start");
                                    let end_color = ui
                                        .color_edit_button_srgb(&mut self.gradient_end_color)
                                        .on_hover_text("Gradient end color · RGB/HSV picker");
                                    end_color.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::WidgetType::ColorButton,
                                            ui.is_enabled(),
                                            "Gradient end color",
                                        )
                                    });
                                    ui.label("End");
                                    if ui.button("Swap").clicked() {
                                        std::mem::swap(
                                            &mut project.brush.color,
                                            &mut self.gradient_end_color,
                                        );
                                    }
                                    ui.add_sized(
                                        [150.0, 30.0],
                                        egui::Slider::new(&mut project.brush.opacity, 0.0..=1.0)
                                            .text("Opacity"),
                                    );
                                    ui.weak("Drag · Esc cancels");
                                } else {
                                    let has_selection =
                                        session.live().project().state().selection.is_some();
                                    ui.strong(match self.tool {
                                        EditTool::Rectangle => "Rectangle selection",
                                        EditTool::Ellipse => "Ellipse selection",
                                        EditTool::Lasso => "Freehand selection",
                                        EditTool::Eyedropper => "Eyedropper",
                                        EditTool::Brush => "Brush",
                                        EditTool::Smudge => "Smudge",
                                        EditTool::ColorMix => "Color Mixer",
                                        EditTool::ShapeLine
                                        | EditTool::ShapeRectangle
                                        | EditTool::ShapeEllipse
                                        | EditTool::Gradient => unreachable!("handled above"),
                                    });
                                    if ui
                                        .add_enabled(has_selection, egui::Button::new("Fill"))
                                        .clicked()
                                    {
                                        actions.push(Action::Fill(false));
                                    }
                                    if ui
                                        .add_enabled(has_selection, egui::Button::new("Erase"))
                                        .clicked()
                                    {
                                        actions.push(Action::Fill(true));
                                    }
                                    if ui
                                        .add_enabled(has_selection, egui::Button::new("Transform…"))
                                        .clicked()
                                    {
                                        self.show_controls = true;
                                        self.inspector_tab = crate::workspace::InspectorTab::Edit;
                                        workspace_changed = true;
                                    }
                                    if ui
                                        .add_enabled(has_selection, egui::Button::new("Deselect"))
                                        .clicked()
                                    {
                                        actions.push(Action::Deselect);
                                    }
                                }
                            });
                        });
                });

            let old_tool = self.tool;
            let screen = ctx.content_rect();
            // Dock positions are relative to the canvas area beneath both
            // application chrome rows. Using the full window rectangle here
            // placed a top-docked palette over the brush size/opacity strip.
            let mut palette_viewport = screen;
            palette_viewport.min.y = (contextual_options.response.rect.bottom() + 8.0)
                .min(palette_viewport.bottom() - 1.0);
            // Keep the lower canvas controls and status line clear as well as
            // the top application chrome. The palette's scroll area below
            // uses this real viewport height so egui never has to clamp a
            // tall toolbar upward over the brush options.
            palette_viewport.max.y =
                (palette_viewport.bottom() - 72.0).max(palette_viewport.min.y + 120.0);
            let floating_position = egui::pos2(self.palette_position[0], self.palette_position[1]);
            let effective_palette_dock = crate::studio_layout::effective_palette_dock(
                self.palette_dock,
                screen,
                self.palette_dragging,
            );
            let dock_position = if self.palette_dragging {
                floating_position
            } else {
                crate::studio_layout::palette_anchor(
                    palette_viewport,
                    effective_palette_dock,
                    floating_position,
                )
            };
            let horizontal_palette = matches!(
                effective_palette_dock,
                crate::workspace::PaletteDock::Top | crate::workspace::PaletteDock::Bottom
            );
            let palette_area_height = (palette_viewport.height() - 16.0).max(120.0);
            let palette_geometry_changed =
                (self.palette_viewport_height - palette_area_height).abs() > 1.0
                    || self.palette_was_horizontal != horizontal_palette;
            self.palette_viewport_height = palette_area_height;
            self.palette_was_horizontal = horizontal_palette;
            let compact_palette =
                self.palette_collapsed || horizontal_palette || screen.width() < 760.0;
            let area = egui::Area::new(egui::Id::new("tool palette"))
                .movable(false)
                .constrain_to(screen.shrink(4.0))
                .order(egui::Order::Foreground)
                .default_size(egui::vec2(80.0, palette_area_height))
                .sizing_pass(palette_geometry_changed)
                .current_pos(dock_position);
            let mut palette_grip_drag_started = false;
            let mut palette_grip_dragged = false;
            let mut palette_grip_drag_stopped = false;
            let palette_response = area.show(ctx, |ui| {
                crate::ui_tokens::tool_palette_frame(ui.visuals()).show(ui, |ui| {
                    if compact_palette && !horizontal_palette {
                        ui.set_width(58.0);
                    }
                    if horizontal_palette {
                        ui.set_min_width((screen.width() - 16.0).clamp(1.0, 640.0));
                    }
                    let mut draw_tools = |ui: &mut egui::Ui| {
                        if !horizontal_palette {
                            ui.add_space(8.0);
                        }
                            let grip = crate::studio_icons::palette_drag_handle(ui);
                            palette_grip_drag_started = grip.drag_started();
                            palette_grip_dragged = grip.dragged();
                            palette_grip_drag_stopped = grip.drag_stopped();
                            if self.test_path_dialog {
                                let rect = grip.rect;
                                let marker = [
                                    (rect.left() * 100.0).round() as i32,
                                    (rect.top() * 100.0).round() as i32,
                                    (rect.width() * 100.0).round() as i32,
                                    (rect.height() * 100.0).round() as i32,
                                    (ctx.pixels_per_point() * 1000.0).round() as i32,
                                ];
                                if self.test_palette_grip_rect_logged != Some(marker) {
                                    self.test_palette_grip_rect_logged = Some(marker);
                                    crate::studio_log!(
                                        "UI_TEST_PALETTE_GRIP_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                        rect.left(),
                                        rect.top(),
                                        rect.width(),
                                        rect.height(),
                                        ctx.pixels_per_point(),
                                    );
                                }
                            }
                        if !horizontal_palette && screen.width() >= 760.0 {
                            let expanded = !self.palette_collapsed;
                            let response = crate::studio_icons::button(
                                ui,
                                if expanded {
                                    crate::studio_icons::Icon::ChevronLeft
                                } else {
                                    crate::studio_icons::Icon::ChevronRight
                                },
                                if expanded {
                                    "Collapse tools"
                                } else {
                                    "Expand tools"
                                },
                                None,
                                false,
                                [46.0, 36.0],
                            );
                            if self.test_path_dialog {
                                let rect = response.rect;
                                let marker = [
                                    (rect.left() * 100.0).round() as i32,
                                    (rect.top() * 100.0).round() as i32,
                                    (rect.width() * 100.0).round() as i32,
                                    (rect.height() * 100.0).round() as i32,
                                    (ctx.pixels_per_point() * 1000.0).round() as i32,
                                ];
                                if self.test_palette_toggle_logged != Some(marker) {
                                    self.test_palette_toggle_logged = Some(marker);
                                crate::studio_log!(
                                    "UI_TEST_PALETTE_TOGGLE_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                    rect.left(),
                                    rect.top(),
                                    rect.width(),
                                    rect.height(),
                                    ctx.pixels_per_point(),
                                );
                                }
                            }
                            if response.clicked() {
                                self.palette_collapsed = expanded;
                                workspace_changed = true;
                                if self.test_path_dialog {
                                    crate::studio_log!(
                                        "UI_TEST_PALETTE_STATE: collapsed={}",
                                        self.palette_collapsed
                                    );
                                }
                            }
                        }
                        let mut previous_tool_group = None;
                        for (tool, icon, name, shortcut, group) in [
                            (
                                EditTool::Brush,
                                crate::studio_icons::Icon::Brush,
                                "Brush",
                                "B",
                                0,
                            ),
                            (
                                EditTool::Smudge,
                                crate::studio_icons::Icon::Smudge,
                                "Smudge",
                                "S",
                                0,
                            ),
                            (
                                EditTool::ColorMix,
                                crate::studio_icons::Icon::ColorMixer,
                                "Color Mixer",
                                "X",
                                0,
                            ),
                            (
                                EditTool::Rectangle,
                                crate::studio_icons::Icon::RectangleSelection,
                                "Rectangle selection",
                                "M",
                                1,
                            ),
                            (
                                EditTool::Ellipse,
                                crate::studio_icons::Icon::EllipseSelection,
                                "Ellipse selection",
                                "E",
                                1,
                            ),
                            (
                                EditTool::Lasso,
                                crate::studio_icons::Icon::Lasso,
                                "Freehand lasso",
                                "K",
                                1,
                            ),
                            (
                                EditTool::ShapeLine,
                                crate::studio_icons::Icon::Line,
                                "Draw line",
                                "L",
                                2,
                            ),
                            (
                                EditTool::ShapeRectangle,
                                crate::studio_icons::Icon::Rectangle,
                                "Draw rectangle",
                                "R",
                                2,
                            ),
                            (
                                EditTool::ShapeEllipse,
                                crate::studio_icons::Icon::Ellipse,
                                "Draw ellipse",
                                "O",
                                2,
                            ),
                            (
                                EditTool::Gradient,
                                crate::studio_icons::Icon::Gradient,
                                "Gradient",
                                "G",
                                2,
                            ),
                            (
                                EditTool::Eyedropper,
                                crate::studio_icons::Icon::Eyedropper,
                                "Eyedropper",
                                "I",
                                3,
                            ),
                        ] {
                            if previous_tool_group.is_some_and(|previous| previous != group) {
                                ui.separator();
                            }
                            previous_tool_group = Some(group);
                            let response = if compact_palette {
                                crate::studio_icons::button(
                                    ui,
                                    icon,
                                    name,
                                    Some(shortcut),
                                    self.tool == tool,
                                    [46.0, 46.0],
                                )
                            } else {
                                ui.horizontal(|ui| {
                                    let response = crate::studio_icons::button(
                                        ui,
                                        icon,
                                        name,
                                        Some(shortcut),
                                        self.tool == tool,
                                        [46.0, 46.0],
                                    );
                                    ui.label(name);
                                    response
                                })
                                .inner
                            };
                            if tool == EditTool::Brush
                                && self.test_path_dialog
                                && !self.welcome
                                && !self.new_dialog
                                && !self.manual_file_dialog
                                && !self.test_brush_rect_logged
                            {
                                self.test_brush_rect_logged = true;
                                let rect = response.rect;
                                crate::studio_log!(
                                    "UI_TEST_BRUSH_RECT: {:.2} {:.2} {:.2} {:.2} {:.2}",
                                    rect.left(),
                                    rect.top(),
                                    rect.width(),
                                    rect.height(),
                                    self.context.pixels_per_point()
                                );
                            }
                            if response.clicked() {
                                self.tool = tool;
                                session.live_mut().set_stroke_mode(stroke_mode_for_tool(
                                    tool,
                                    self.color_mix_strength,
                                ));
                                if self.test_path_dialog {
                                    crate::studio_log!("UI_TEST_ACTIVE_TOOL: {:?}", self.tool);
                                }
                                if tool == EditTool::Brush {
                                    self.show_controls = false;
                                    self.color_flyout_open = false;
                                    self.brush_flyout_open = true;
                                    ctx.request_repaint();
                                    if self.test_path_dialog {
                                        crate::studio_log!("UI_TEST_BRUSH_FLYOUT_OPENED: palette");
                                    }
                                    workspace_changed = true;
                                } else {
                                    self.brush_flyout_open = false;
                                    if matches!(
                                        tool,
                                        EditTool::ShapeLine
                                            | EditTool::ShapeRectangle
                                            | EditTool::ShapeEllipse
                                            | EditTool::Gradient
                                    ) {
                                        self.color_flyout_open = false;
                                    }
                                }
                            }
                        }
                        ui.separator();
                        if crate::studio_icons::button(
                            ui,
                            crate::studio_icons::Icon::Color,
                            "Color controls",
                            Some("C"),
                            self.color_flyout_open,
                            [46.0, 46.0],
                        )
                        .clicked()
                        {
                            self.brush_flyout_open = false;
                            self.color_flyout_open = !self.color_flyout_open;
                            ctx.request_repaint();
                        }
                        if !compact_palette {
                            let layers = ui
                                .button("Layers")
                                .on_hover_text("Open floating layer panel");
                            if layers.clicked() {
                                self.layers_flyout_open = !self.layers_flyout_open;
                                ctx.request_repaint();
                            }
                        } else if crate::studio_icons::button(
                            ui,
                            crate::studio_icons::Icon::Layers,
                            "Layers",
                            None,
                            false,
                            [46.0, 46.0],
                        )
                        .clicked()
                        {
                            self.layers_flyout_open = !self.layers_flyout_open;
                        }
                        ui.separator();
                        let can_add_reference =
                            self.references.len() < crate::workspace::MAX_REFERENCES;
                        if crate::studio_icons::button(
                            ui,
                            crate::studio_icons::Icon::Reference,
                            "Reference Images",
                            None,
                            false,
                            [46.0, 46.0],
                        )
                        .clicked()
                        {
                            if can_add_reference {
                                self.picker = Some(FileRequest::AddReferences);
                            } else {
                                self.status = format!(
                                    "Reference board is full ({} images maximum)",
                                    crate::workspace::MAX_REFERENCES
                                );
                            }
                        }
                        ui.separator();
                        if crate::studio_icons::button(
                            ui,
                            crate::studio_icons::Icon::Deselect,
                            "Deselect",
                            Some("Ctrl+D"),
                            false,
                            [46.0, 42.0],
                        )
                        .clicked()
                        {
                            actions.push(Action::Deselect);
                        }
                        if crate::studio_icons::button(
                            ui,
                            crate::studio_icons::Icon::Fill,
                            "Fill selection",
                            Some("Ctrl+F"),
                            false,
                            [46.0, 42.0],
                        )
                        .clicked()
                        {
                            actions.push(Action::Fill(false));
                        }
                        if crate::studio_icons::button(
                            ui,
                            crate::studio_icons::Icon::Eraser,
                            "Erase selection",
                            Some("Ctrl+X"),
                            false,
                            [46.0, 42.0],
                        )
                        .clicked()
                        {
                            actions.push(Action::Fill(true));
                        }
                    };
                    if horizontal_palette {
                        ui.horizontal_wrapped(&mut draw_tools);
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height((palette_viewport.height() - 16.0).max(120.0))
                            .auto_shrink([false, false])
                            .show(ui, |ui| ui.vertical_centered(&mut draw_tools));
                    }
                });
            });
            let rect = palette_response.response.rect;
            self.palette_rect = Some(rect);
            let compact_workspace = crate::studio_layout::compact_workspace_mode(screen);
            let active_compact_reference = self
                .compact_reference_index
                .min(self.references.len().saturating_sub(1));
            let compact_companions_visible = !(compact_workspace && self.brush_flyout_open);
            let compact_layout_key = if compact_companions_visible {
                usize::from(self.layers_flyout_open) + usize::from(!self.references.is_empty())
            } else {
                0
            };
            let mut compact_panel_sizes =
                Vec::with_capacity(self.references.len() + usize::from(self.layers_flyout_open));
            if self.layers_flyout_open && compact_companions_visible {
                compact_panel_sizes.push(egui::vec2(
                    self.layers_flyout_size[0],
                    self.layers_flyout_size[1],
                ));
            }
            compact_panel_sizes.extend(self.references.iter().enumerate().filter_map(
                |(index, reference)| {
                    ((!compact_workspace || index == active_compact_reference)
                        && compact_companions_visible)
                        .then_some(egui::vec2(
                            reference.layout.size[0],
                            reference.layout.size[1],
                        ))
                },
            ));
            let compact_companion_panels = crate::studio_layout::compact_workspace_panel_rects(
                screen,
                rect,
                &compact_panel_sizes,
            );
            if self.test_path_dialog && compact_workspace {
                crate::studio_log!(
                    "UI_TEST_COMPACT_PLAN: viewport={:.1}x{:.1} palette={:.1},{:.1},{:.1},{:.1} panels={:?}",
                    screen.width(),
                    screen.height(),
                    rect.left(),
                    rect.top(),
                    rect.width(),
                    rect.height(),
                    compact_companion_panels,
                );
            }
            if self.test_path_dialog && compact_workspace && self.brush_flyout_open {
                crate::studio_log!(
                    "UI_TEST_COMPACT_BRUSH_FOCUS: layers_hidden={} references_hidden={}",
                    self.layers_flyout_open,
                    self.references.len(),
                );
            }
            if self.test_path_dialog {
                let scale = ctx.pixels_per_point();
                let marker = [
                    (rect.left() * 100.0).round() as i32,
                    (rect.top() * 100.0).round() as i32,
                    (rect.width() * 100.0).round() as i32,
                    (rect.height() * 100.0).round() as i32,
                    (scale * 1000.0).round() as i32,
                ];
                if self.test_palette_rect_logged != Some(marker) {
                    self.test_palette_rect_logged = Some(marker);
                    crate::studio_log!(
                        "UI_TEST_PALETTE_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                        rect.left(),
                        rect.top(),
                        rect.width(),
                        rect.height(),
                        scale
                    );
                    crate::studio_log!(
                        "UI_TEST_PALETTE_CHROME_CLEARANCE: {:.1} {:.1}",
                        rect.top(),
                        contextual_options.response.rect.bottom(),
                    );
                }
            }
            if self.test_path_dialog
                && self.palette_dock == crate::workspace::PaletteDock::Top
                && !self.test_palette_clearance_logged
            {
                self.test_palette_clearance_logged = true;
                crate::studio_log!(
                    "UI_TEST_TOP_PALETTE_CLEARANCE: {:.1} {:.1}",
                    rect.top(),
                    contextual_options.response.rect.bottom(),
                );
            }
            let pointer = ctx.input(|input| input.pointer.interact_pos());
            let mut palette_position_updated = false;
            if palette_grip_drag_started {
                self.palette_dragging = true;
                if let Some(pointer) = pointer {
                    self.palette_drag_offset = [pointer.x - rect.left(), pointer.y - rect.top()];
                }
            }
            if (palette_grip_dragged || palette_grip_drag_stopped)
                && let Some(pointer) = pointer
            {
                let clamped = clamp_reference_position(
                    [
                        pointer.x - self.palette_drag_offset[0],
                        pointer.y - self.palette_drag_offset[1],
                    ],
                    [rect.width(), rect.height()],
                    palette_viewport,
                );
                self.palette_position = [clamped.x, clamped.y];
                palette_position_updated = true;
            }
            if palette_grip_drag_stopped {
                let position = if palette_position_updated {
                    egui::pos2(self.palette_position[0], self.palette_position[1])
                } else {
                    rect.min
                };
                let dragged_rect = egui::Rect::from_min_size(position, rect.size());
                self.palette_dock =
                    crate::studio_layout::snap_palette(palette_viewport, dragged_rect);
                self.palette_dragging = false;
                workspace_changed = true;
            }
            if !palette_position_updated {
                let clamped = clamp_reference_position(
                    [rect.min.x, rect.min.y],
                    [rect.width(), rect.height()],
                    palette_viewport,
                );
                self.palette_position = [clamped.x, clamped.y];
            }
            if old_tool != self.tool {
                session.mouse(MouseEvent::Interrupted, 0);
                self.edit_pointer = None;
                self.edit_points.clear();
            }

            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                let project = session.live().project();
                let save_status = if self.recovery_saving {
                    "Saving recovery"
                } else if project.dirty() && self.recovery.is_some() {
                    "Unsaved changes · recovery available"
                } else if project.dirty() {
                    "Unsaved changes"
                } else if self.store.is_some() {
                    "Saved"
                } else {
                    "Untitled document"
                };
                if compact_workspace {
                    ui.horizontal(|ui| {
                        ui.strong(save_status);
                        ui.separator();
                        if let Some(error) = session.live().document_error() {
                            ui.colored_label(egui::Color32::LIGHT_RED, error);
                        } else {
                            ui.small(format!(
                                "{} × {} px · {} dpi",
                                project.state().width,
                                project.state().height,
                                project.state().dpi
                            ));
                        }
                    })
                    .response
                    .on_hover_text(&self.status);
                } else {
                    ui.horizontal(|ui| {
                        ui.strong(save_status);
                        ui.separator();
                        if let Some(error) = session.live().document_error() {
                            ui.colored_label(egui::Color32::LIGHT_RED, error);
                        } else {
                            ui.label(&self.status);
                        }
                    });
                    ui.small(format!(
                        "{} × {} px · {} dpi",
                        session.live().project().state().width,
                        session.live().project().state().height,
                        session.live().project().state().dpi
                    ));
                }
            });

            if self.show_controls && !self.welcome && !self.new_dialog && !self.recovery_manager {
                let mut controls_open = true;
                let mut open_layers_from_controls = false;
                let mut open_brush_from_controls = false;
                let controls_size = egui::vec2(
                    self.workspace_controls_size[0].min((screen.width() - 16.0).max(1.0)),
                    self.workspace_controls_size[1].min((screen.height() - 16.0).max(1.0)),
                );
                let controls_obstacles =
                    self.visible_workspace_obstacles(ctx, "Workspace controls", true);
                let controls_position = crate::studio_layout::flyout_position_avoiding(
                    screen,
                    palette_response.response.rect,
                    controls_size,
                    egui::pos2(
                        self.workspace_controls_position[0],
                        self.workspace_controls_position[1],
                    ),
                    &controls_obstacles,
                );
                let controls_area = egui::AreaState::load(ctx, egui::Id::new("Workspace controls"));
                let controls_need_reanchor = controls_area.is_some_and(|state| {
                    let current = state.rect();
                    !screen.contains_rect(current)
                        || current.intersects(palette_response.response.rect)
                        || controls_obstacles
                            .iter()
                            .any(|obstacle| current.intersects(*obstacle))
                });
                let controls_window = egui::Window::new("Workspace controls")
                    .order(egui::Order::Foreground)
                    .frame(crate::ui_tokens::floating_panel_frame(&ctx.style().visuals))
                    .open(&mut controls_open)
                    .resizable(true)
                    .default_size(controls_size)
                    .max_size(egui::vec2(
                        (screen.width() - 16.0).max(1.0),
                        (screen.height() - 16.0).max(1.0),
                    ))
                    .default_pos(controls_position)
                    .constrain_to(screen.shrink(8.0));
                let controls_window = if controls_need_reanchor {
                    controls_window.current_pos(controls_position)
                } else {
                    controls_window
                };
                let controls_window = controls_window.show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (tab, label) in [
                                (crate::workspace::InspectorTab::Layers, "Layers"),
                                (crate::workspace::InspectorTab::Brush, "Brush"),
                                (crate::workspace::InspectorTab::Edit, "Edit"),
                            ] {
                                let response =
                                    ui.selectable_label(self.inspector_tab == tab, label);
                                if self.test_path_dialog && label == "Brush" {
                                    crate::studio_log!(
                                        "UI_TEST_WORKSPACE_BRUSH_TAB_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                        response.rect.left(),
                                        response.rect.top(),
                                        response.rect.width(),
                                        response.rect.height(),
                                        ui.ctx().pixels_per_point(),
                                    );
                                }
                                if response.clicked() {
                                    self.inspector_tab = tab;
                                    workspace_changed = true;
                                }
                            }
                        });
                        if self.test_path_dialog {
                            crate::studio_log!(
                                "UI_TEST_WORKSPACE_CONTROLS_ACTIVE: {:?}",
                                self.inspector_tab
                            );
                        }
                        ui.separator();
                        let project = session.live_mut().project_mut();

                        if self.inspector_tab == crate::workspace::InspectorTab::Brush {
                            ui.heading("Brush tools");
                            ui.label(brush_display_name(project.brush.kind));
                            ui.small(format!(
                                "{} px · {}% opacity",
                                project.brush.size,
                                (project.brush.opacity * 100.0).round() as u32
                            ));
                            ui.label("Quick size and opacity stay above the canvas. Presets and advanced behavior live together in the Brush Library.");
                            let launch = ui.button("Open Brush Library");
                            if self.test_path_dialog {
                                crate::studio_log!(
                                    "UI_TEST_WORKSPACE_OPEN_BRUSH_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                    launch.rect.left(), launch.rect.top(), launch.rect.width(),
                                    launch.rect.height(), ui.ctx().pixels_per_point(),
                                );
                            }
                            if launch.clicked() {
                                open_brush_from_controls = true;
                            }
                        }

                        if self.inspector_tab == crate::workspace::InspectorTab::Edit {
                            ui.heading("Canvas and selection");
                            let mut transparent = project.state().background.is_none();

                            if ui
                                .checkbox(&mut transparent, "Transparent background")
                                .changed()
                                && let Err(e) = project.edit(|s| {
                                    s.background = if transparent { None } else { Some([255; 3]) };
                                    Ok(())
                                })
                            {
                                self.status = e.to_string();
                            }

                            ui.collapsing("Edit selected pixels", |ui| {
                                ui.label("Selected layer only; outside canvas is clipped.");

                                ui.horizontal(|ui| {
                                    ui.label("Move X/Y");
                                    ui.add(
                                        egui::DragValue::new(&mut self.transform_offset[0])
                                            .range(-4096.0..=4096.0),
                                    );
                                    ui.add(
                                        egui::DragValue::new(&mut self.transform_offset[1])
                                            .range(-4096.0..=4096.0),
                                    );
                                });

                                ui.add(
                                    egui::Slider::new(&mut self.transform_scale, 0.1..=4.0)
                                        .text("Scale"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut self.transform_angle, -180.0..=180.0)
                                        .text("Rotation"),
                                );

                                let bounds = project
                                    .state()
                                    .selection
                                    .as_ref()
                                    .map(Selection::bounds)
                                    .unwrap_or([
                                        0.0,
                                        0.0,
                                        project.state().width as f64,
                                        project.state().height as f64,
                                    ]);

                                let cx = (bounds[0] + bounds[2]) / 2.0;
                                let cy = (bounds[1] + bounds[3]) / 2.0;

                                if ui.button("Apply transform").clicked() {
                                    let a = self.transform_angle.to_radians();
                                    let (sin, cos) = a.sin_cos();
                                    let c = cos * self.transform_scale;
                                    let b = sin * self.transform_scale;
                                    actions.push(Action::Transform([
                                        c,
                                        b,
                                        -b,
                                        c,
                                        cx - c * cx + b * cy + self.transform_offset[0],
                                        cy - b * cx - c * cy + self.transform_offset[1],
                                    ]));
                                }

                                ui.horizontal(|ui| {
                                    if ui.button("Flip H").clicked() {
                                        actions.push(Action::Transform([
                                            -1.0,
                                            0.0,
                                            0.0,
                                            1.0,
                                            2.0 * cx,
                                            0.0,
                                        ]));
                                    }

                                    if ui.button("Flip V").clicked() {
                                        actions.push(Action::Transform([
                                            1.0,
                                            0.0,
                                            0.0,
                                            -1.0,
                                            0.0,
                                            2.0 * cy,
                                        ]));
                                    }
                                });

                                if ui
                                    .add_enabled(
                                        project.state().selection.is_some(),
                                        egui::Button::new("Crop document to selection bounds"),
                                    )
                                    .clicked()
                                {
                                    actions.push(Action::Crop);
                                }
                            });
                        }

                        if self.inspector_tab == crate::workspace::InspectorTab::Layers {
                            let selected = project.state().selected;
                            let layer_count = project.state().layers.len();
                            let selected_name = project
                                .state()
                                .layers
                                .iter()
                                .find(|layer| layer.id == selected)
                                .map_or("No selected layer", |layer| layer.name.as_str());
                            ui.heading("Layer stack");
                            ui.label(format!("{layer_count} layers · {selected_name} selected"));
                            ui.label("Previews, visibility, ordering, opacity, blend mode and layer actions stay together in the floating Layers panel.");
                            let launch = ui.button("Open Layers panel");
                            if self.test_path_dialog {
                                crate::studio_log!(
                                    "UI_TEST_WORKSPACE_OPEN_LAYERS_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                    launch.rect.left(), launch.rect.top(), launch.rect.width(),
                                    launch.rect.height(), ui.ctx().pixels_per_point(),
                                );
                            }
                            if launch.clicked() {
                                open_layers_from_controls = true;
                            }
                        }
                    });
                });
                self.show_controls = controls_open;
                if open_layers_from_controls {
                    self.layers_flyout_open = true;
                    self.show_controls = false;
                    workspace_changed = true;
                    if self.test_path_dialog {
                        crate::studio_log!("UI_TEST_LAYERS_FLYOUT_OPENED: workspace controls");
                        crate::studio_log!("UI_TEST_WORKSPACE_LAUNCHED: Layers");
                    }
                }
                if open_brush_from_controls {
                    self.brush_flyout_open = true;
                    self.show_controls = false;
                    workspace_changed = true;
                    if self.test_path_dialog {
                        crate::studio_log!("UI_TEST_BRUSH_FLYOUT_OPENED: workspace controls");
                        crate::studio_log!("UI_TEST_WORKSPACE_LAUNCHED: Brush");
                    }
                }
                if let Some(controls_window) = controls_window {
                    let panel_position = [
                        controls_window.response.rect.left(),
                        controls_window.response.rect.top(),
                    ];
                    if self.test_path_dialog {
                        crate::studio_log!(
                            "UI_TEST_WORKSPACE_CONTROLS_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                            panel_position[0],
                            panel_position[1],
                            controls_window.response.rect.width(),
                            controls_window.response.rect.height(),
                            self.context.pixels_per_point(),
                        );
                    }
                    if panel_position != self.workspace_controls_position {
                        self.workspace_controls_position = panel_position;
                        workspace_changed = true;
                    }
                    let panel_width = controls_window.response.rect.width();
                    if (panel_width - self.brush_panel_width).abs() > 0.5 {
                        self.brush_panel_width = panel_width.clamp(180.0, 640.0);
                        workspace_changed = true;
                    }
                    let panel_size = [
                        panel_width.clamp(240.0, 900.0),
                        controls_window.response.rect.height().clamp(200.0, 1400.0),
                    ];
                    if panel_size != self.workspace_controls_size {
                        self.workspace_controls_size = panel_size;
                        workspace_changed = true;
                    }
                }
            }

            if self.layers_flyout_open
                && !(self.welcome
                    || self.new_dialog
                    || self.recovery_manager
                    || compact_workspace && self.brush_flyout_open)
            {
                let mut open = self.layers_flyout_open;
                let compact_rect = compact_companion_panels
                    .as_ref()
                    .and_then(|rects| rects.first().copied());
                let compact = compact_rect.is_some();
                let layers_id = compact_floating_panel_id("Layers", compact, compact_layout_key);
                let desired_size = compact_rect.map_or_else(
                    || egui::vec2(self.layers_flyout_size[0], self.layers_flyout_size[1]),
                    |rect| rect.size(),
                );
                let obstacles = self.visible_workspace_obstacles(ctx, "Layers", true);
                let layers_position = compact_rect.map_or_else(
                    || {
                        crate::studio_layout::flyout_position_avoiding(
                            screen,
                            palette_response.response.rect,
                            desired_size,
                            egui::pos2(
                                self.layers_flyout_position[0],
                                self.layers_flyout_position[1],
                            ),
                            &obstacles,
                        )
                    },
                    |rect| rect.min,
                );
                let layers_area = egui::AreaState::load(ctx, layers_id);
                let layers_needs_reanchor = layers_area.is_some_and(|state| {
                    let current = state.rect();
                    !screen.contains_rect(current)
                        || current.intersects(palette_response.response.rect)
                        || compact_rect.is_some_and(|target| {
                            current.width() > target.width() + 24.0
                                || current.height() > target.height() + 24.0
                        })
                        || obstacles
                            .iter()
                            .any(|obstacle| current.intersects(*obstacle))
                });
                let panel = egui::Window::new("Layers")
                    .id(layers_id)
                    .order(egui::Order::Foreground)
                    .frame(crate::ui_tokens::floating_panel_frame(&ctx.style().visuals))
                    .open(&mut open)
                    .resizable(!compact)
                    .default_size(desired_size)
                    .max_size(egui::vec2(
                        if compact {
                            desired_size.x
                        } else {
                            (screen.width() - 16.0).max(1.0)
                        },
                        if compact {
                            desired_size.y
                        } else {
                            (screen.height() - 16.0).max(1.0)
                        },
                    ))
                    .default_pos(layers_position)
                    .constrain_to(screen.shrink(8.0));
                let panel = if compact {
                    let panel = panel.fixed_size(desired_size);
                    if layers_needs_reanchor {
                        panel.current_pos(layers_position)
                    } else {
                        panel
                    }
                } else if layers_needs_reanchor {
                    panel.current_pos(layers_position)
                } else {
                    panel
                }
                .show(ctx, |ui| {
                    let mut drag_ui = LayerDragUi {
                        handles: &mut self.layer_drag_handles,
                        drop_targets: &mut self.layer_drop_targets,
                        source: self.layer_drag_source,
                        target: self.layer_drag_target,
                    };
                    let mut thumbnail_context = LayerThumbnailContext {
                        cache: &mut self.layer_thumbnail_cache,
                        document_epoch: self.layer_thumbnail_document_epoch,
                    };
                    let contents = |ui: &mut egui::Ui| {
                        layer_flyout_contents(
                            ui,
                            session.live_mut().project_mut(),
                            &mut thumbnail_context,
                            actions,
                            self.test_path_dialog,
                            compact,
                            &mut drag_ui,
                        );
                    };
                    egui::ScrollArea::vertical()
                        .id_salt(if compact {
                            "compact-layers-content"
                        } else {
                            "layers-content"
                        })
                        .auto_shrink([false, false])
                        .max_height(if compact {
                            (desired_size.y - 128.0).max(96.0)
                        } else {
                            (screen.height() - 112.0).max(96.0)
                        })
                        .show(ui, contents);
                });
                if let Some(panel) = panel {
                    let rect = panel.response.rect;
                    let position = [rect.left(), rect.top()];
                    let size = [rect.width(), rect.height()];
                    if self.test_path_dialog {
                        crate::studio_log!(
                            "UI_TEST_LAYERS_WINDOW_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                            rect.left(),
                            rect.top(),
                            rect.width(),
                            rect.height(),
                            ctx.pixels_per_point(),
                        );
                    }
                    if !compact
                        && (position != self.layers_flyout_position
                            || size != self.layers_flyout_size)
                    {
                        self.layers_flyout_position = position;
                        self.layers_flyout_size = size;
                        workspace_changed = true;
                    }
                }
                self.layers_flyout_open = open;
            }
            if self.test_path_dialog {
                crate::studio_log!("UI_TEST_LAYERS_PANEL_STATE: {}", self.layers_flyout_open);
            }

            if self.brush_flyout_open && !self.welcome && !self.new_dialog && !self.recovery_manager
            {
                let mut open = self.brush_flyout_open;
                let obstacles =
                    self.visible_workspace_obstacles(ctx, "Brush library", screen.width() >= 760.0);
                // Keep a useful side column open for the reference board on
                // desktops. Small windows get a narrower temporary gallery;
                // persisted desktop dimensions are left untouched.
                let brush_size = crate::studio_layout::flyout_size(
                    screen,
                    palette_response.response.rect,
                    crate::studio_layout::brush_flyout_preferred_size(screen.width()),
                );
                let brush_position = crate::studio_layout::flyout_position_avoiding(
                    screen,
                    palette_response.response.rect,
                    brush_size,
                    egui::pos2(self.brush_flyout_position[0], self.brush_flyout_position[1]),
                    &obstacles,
                );
                let brush_area = egui::AreaState::load(ctx, egui::Id::new("Brush library"));
                let brush_previous_rect = brush_area.map(|state| state.rect());
                let brush_needs_reanchor = brush_area.is_none_or(|state| {
                    let current_rect = state.rect();
                    !screen.contains_rect(current_rect)
                        || crate::studio_layout::flyout_size_exceeds(
                            current_rect.size(),
                            brush_size,
                        )
                        || current_rect.intersects(palette_response.response.rect)
                        || obstacles
                            .iter()
                            .any(|obstacle| current_rect.intersects(*obstacle))
                });
                let panel = egui::Window::new("Brush library")
                    .order(egui::Order::Foreground)
                    .frame(crate::ui_tokens::floating_panel_frame(&ctx.style().visuals))
                    .open(&mut open)
                    .resizable(screen.width() >= 760.0)
                    .default_size(brush_size)
                    .max_size(brush_size)
                    .default_pos(brush_position);
                let panel = if screen.width() < 760.0 {
                    panel.fixed_size(brush_size)
                } else {
                    panel
                };
                let panel = if brush_needs_reanchor {
                    // `Window::default_size` does not shrink a previously
                    // persisted AreaState. When an old or colliding rectangle
                    // needs reanchoring, apply one bounded size reset, then
                    // restore the user's normal resize behavior next frame.
                    panel.current_pos(brush_position).fixed_size(brush_size)
                } else {
                    panel
                }
                    .constrain_to(screen.shrink(8.0))
                    .show(ctx, |ui| {
                        let compact_brush_panel = screen.width() < 760.0;
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_width((brush_size.x - 24.0).max(160.0))
                            // Account for the floating window title and frame;
                            // an unconstrained content viewport can make egui
                            // exceed max_size and slide the panel over the tools.
                            .max_height((brush_size.y - 100.0).max(80.0))
                            .show(ui, |ui| {
                        ui.set_width((brush_size.x - 24.0).max(160.0));
                        ui.set_max_width((brush_size.x - 24.0).max(160.0));
                        let project = session.live_mut().project_mut();
                        if !compact_brush_panel {
                            crate::ui_tokens::section_heading(
                                ui,
                                "Brush library",
                                Some("10 presets"),
                            );
                            ui.label("Pressure data is preserved when the active input provides it.");
                        }
                        if self
                            .brush_preview
                            .as_ref()
                            .is_none_or(|preview| preview.brush != project.brush)
                        {
                            match crate::brush_preview::render_rgba(&project.brush) {
                                Ok(pixels) => {
                                    let image = egui::ColorImage::from_rgba_unmultiplied(
                                        [
                                            crate::brush_preview::WIDTH as usize,
                                            crate::brush_preview::HEIGHT as usize,
                                        ],
                                        &pixels,
                                    );
                                    if let Some(preview) = self.brush_preview.as_mut() {
                                        preview.brush = project.brush.clone();
                                        preview.texture.set(image, egui::TextureOptions::LINEAR);
                                    } else {
                                        self.brush_preview = Some(BrushPreviewCache {
                                            brush: project.brush.clone(),
                                            texture: ui.ctx().load_texture(
                                                "active raster brush preview",
                                                image,
                                                egui::TextureOptions::LINEAR,
                                            ),
                                        });
                                    }
                                }
                                Err(_) => self.brush_preview = None,
                            }
                        }
                        if !compact_brush_panel
                            && let Some(preview) = self.brush_preview.as_ref()
                        {
                            let response = active_brush_preview(
                                ui,
                                &project.brush,
                                &preview.texture,
                                false,
                            );
                            if self.test_path_dialog && !self.test_brush_preview_logged {
                                self.test_brush_preview_logged = true;
                                crate::studio_log!(
                                    "UI_TEST_BRUSH_PREVIEW_BOUNDS: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                    response.rect.left(),
                                    response.rect.top(),
                                    response.rect.width(),
                                    response.rect.height(),
                                    ui.ctx().pixels_per_point(),
                                );
                            }
                        }
                        ui.horizontal(|ui| {
                            if ui.selectable_label(!self.favorites_only, "All brushes").clicked() {
                                self.favorites_only = false;
                            }
                            if ui.selectable_label(self.favorites_only, "★ Favorites").clicked() {
                                self.favorites_only = true;
                            }
                            ui.add_sized(
                                [132.0, 36.0],
                                egui::TextEdit::singleline(&mut self.brush_search)
                                    .hint_text("Find a brush"),
                            );
                        });
                        ui.add_space(4.0);
                        let query = self.brush_search.trim().to_lowercase();
                        let mut visible_count = 0;
                        let mut fully_visible_narrow_cards = 0;
                        for (group, kinds) in [
                            ("Ink and drawing", &[BrushKind::Ink, BrushKind::Pencil, BrushKind::Calligraphy][..]),
                            ("Paint and air", &[BrushKind::Round, BrushKind::Soft, BrushKind::Marker, BrushKind::Airbrush][..]),
                            ("Texture and utility", &[BrushKind::Textured, BrushKind::Wash, BrushKind::Eraser][..]),
                        ] {
                            let filtered: Vec<_> = kinds.iter().copied().filter(|kind| {
                                (!self.favorites_only || self.favorite_brushes.contains(kind))
                                    && (query.is_empty() || brush_display_name(*kind).to_lowercase().contains(&query))
                            }).collect();
                            if filtered.is_empty() { continue; }
                            visible_count += filtered.len();
                            if !compact_brush_panel {
                                crate::ui_tokens::section_heading(ui, group, None);
                            }
                            if screen.width() < 760.0 {
                                for kind in filtered {
                                    ui.horizontal(|ui| {
                                        let selected = project.brush.kind == kind;
                                        let card_width = (ui.available_width() - 44.0).clamp(120.0, 440.0);
                                        let texture = self.brush_preset_texture(ui.ctx(), kind);
                                        let (activated, card_rect) = brush_preset_card_with_preview(
                                            ui,
                                            kind,
                                            selected,
                                            project.brush.color,
                                            card_width,
                                            64.0,
                                            texture,
                                        );
                                        let visible_card = card_rect.intersect(ui.clip_rect());
                                        if visible_card.height() >= 64.0 {
                                            fully_visible_narrow_cards += 1;
                                        }
                                        if self.test_path_dialog
                                            && compact_brush_panel
                                            && !self.test_brush_narrow_card_logged
                                            && visible_card.height() >= 60.0
                                        {
                                            self.test_brush_narrow_card_logged = true;
                                            crate::studio_log!(
                                                "UI_TEST_BRUSH_NARROW_CARD_VISIBLE: {:.1} {:.1} {:.1} {:.1} {:.1}",
                                                card_rect.left(),
                                                card_rect.top(),
                                                visible_card.width(),
                                                visible_card.height(),
                                                ui.ctx().pixels_per_point(),
                                            );
                                        }
                                        if activated {
                                            activate_brush_preset(&mut project.brush, kind);
                                        }
                                        let favorite = self.favorite_brushes.contains(&kind);
                                        let response = ui.add_sized(
                                            [36.0, 44.0],
                                            egui::Button::new(if favorite { "★" } else { "☆" }),
                                        );
                                        response.widget_info(|| egui::WidgetInfo::labeled(
                                            egui::WidgetType::Button,
                                            ui.is_enabled(),
                                            if favorite { "Remove from favorite brushes" } else { "Add to favorite brushes" },
                                        ));
                                        if response.on_hover_text(if favorite { "Remove from favorites" } else { "Add to favorites" }).clicked() {
                                            if let Some(index) = self.favorite_brushes.iter().position(|value| *value == kind) {
                                                self.favorite_brushes.remove(index);
                                            } else if self.favorite_brushes.len() < crate::workspace::MAX_FAVORITE_BRUSHES {
                                                self.favorite_brushes.push(kind);
                                            } else {
                                                self.status = format!("Favorites are limited to {} brushes", crate::workspace::MAX_FAVORITE_BRUSHES);
                                            }
                                            workspace_changed = true;
                                        }
                                    });
                                }
                            } else {
                                // Keep four presets from forcing the entire
                                // flyout wider than its viewport. Horizontal
                                // wrapping in a vertically scrolling area can
                                // inherit an unbounded content width, so form
                                // explicit three-card rows instead.
                                for row in filtered.chunks(3) {
                                    ui.horizontal(|ui| {
                                    for kind in row.iter().copied() {
                                        ui.vertical(|ui| {
                                            let selected = project.brush.kind == kind;
                                            let texture = self.brush_preset_texture(ui.ctx(), kind);
                                            let (activated, _) = brush_preset_card_with_preview(ui, kind, selected, project.brush.color, 148.0, 82.0, texture);
                                            if activated {
                                                activate_brush_preset(&mut project.brush, kind);
                                            }
                                            let favorite = self.favorite_brushes.contains(&kind);
                                            let label = if favorite { "★" } else { "☆" };
                                            let response = ui.add_sized([148.0, 32.0], egui::Button::new(label));
                                            response.widget_info(|| egui::WidgetInfo::labeled(
                                                egui::WidgetType::Button,
                                                ui.is_enabled(),
                                                if favorite { "Remove from favorite brushes" } else { "Add to favorite brushes" },
                                            ));
                                            if response.on_hover_text(if favorite { "Remove from favorites" } else { "Add to favorites" }).clicked() {
                                                if let Some(index) = self.favorite_brushes.iter().position(|value| *value == kind) {
                                                    self.favorite_brushes.remove(index);
                                                } else if self.favorite_brushes.len() < crate::workspace::MAX_FAVORITE_BRUSHES {
                                                    self.favorite_brushes.push(kind);
                                                } else {
                                                    self.status = format!("Favorites are limited to {} brushes", crate::workspace::MAX_FAVORITE_BRUSHES);
                                                }
                                                workspace_changed = true;
                                            }
                                        });
                                    }
                                    });
                                }
                            }
                        }
                        if self.test_path_dialog
                            && compact_brush_panel
                            && self.test_brush_narrow_visible_card_count
                                != Some(fully_visible_narrow_cards)
                        {
                            self.test_brush_narrow_visible_card_count =
                                Some(fully_visible_narrow_cards);
                            crate::studio_log!(
                                "UI_TEST_BRUSH_NARROW_VISIBLE_CARDS: {fully_visible_narrow_cards}"
                            );
                        }
                        if visible_count == 0 {
                            ui.add_space(8.0);
                            ui.weak(if self.favorites_only {
                                "No matching favorites yet. Add a brush with the star button."
                            } else {
                                "No brushes match this search."
                            });
                        }
                        // On narrow windows the searchable preset gallery is
                        // the first thing an artist sees. Keep the large active
                        // sample below it, where it remains available by scroll.
                        if compact_brush_panel
                            && let Some(preview) = self.brush_preview.as_ref()
                        {
                            ui.separator();
                            let response = active_brush_preview(
                                ui,
                                &project.brush,
                                &preview.texture,
                                true,
                            );
                            if self.test_path_dialog && !self.test_brush_preview_logged {
                                self.test_brush_preview_logged = true;
                                crate::studio_log!(
                                    "UI_TEST_BRUSH_PREVIEW_BOUNDS: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                    response.rect.left(),
                                    response.rect.top(),
                                    response.rect.width(),
                                    response.rect.height(),
                                    ui.ctx().pixels_per_point(),
                                );
                            }
                        }
                        if self.test_path_dialog && !self.test_brush_preset_previews_logged {
                            self.test_brush_preset_previews_logged = true;
                            crate::studio_log!(
                                "UI_TEST_BRUSH_PRESET_PREVIEWS: {}",
                                self.brush_preset_previews.len()
                            );
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            let current = egui::Color32::from_rgb(
                                project.brush.color[0], project.brush.color[1], project.brush.color[2],
                            );
                            let swatch = egui::Frame::new().fill(current).corner_radius(6.0)
                                .stroke(ui.visuals().window_stroke).inner_margin(egui::Margin::same(6))
                                .show(ui, |ui| { ui.label(" "); });
                            let _ = swatch;
                            ui.strong(brush_display_name(project.brush.kind));
                            ui.weak("Active");
                        });
                        ui.weak("Size and opacity are available in the canvas options row.");
                        egui::CollapsingHeader::new("Flow and dynamics").show(ui, |ui| {
                            ui.add(egui::Slider::new(&mut project.brush.flow, 0.01..=1.0).text("Flow"));
                            ui.add(egui::Slider::new(&mut project.brush.hardness, 0.0..=1.0).text("Hardness"));
                            ui.add(egui::Slider::new(&mut project.brush.spacing, 0.02..=1.0).text("Spacing"));
                            ui.add(
                                egui::Slider::new(
                                    &mut project.brush.angle,
                                    -std::f64::consts::PI..=std::f64::consts::PI,
                                )
                                .text("Angle"),
                            )
                            .on_hover_text("Rotate the textured brush pattern while painting.");
                            ui.add(
                                egui::Slider::new(&mut project.brush.smoothing, 0.0..=1.0)
                                    .text("Stabilization"),
                            )
                            .on_hover_text("Adaptive position filtering; 0 disables it. Fast movement stays responsive.");
                            ui.checkbox(&mut project.brush.pressure_size, "Pressure controls size");
                            ui.checkbox(&mut project.brush.pressure_opacity, "Pressure controls opacity");
                            ui.checkbox(&mut project.brush.tilt, "Use pen tilt when available");
                        });
                        ui.horizontal(|ui| {
                            ui.label("Color");
                            ui.color_edit_button_srgb(&mut project.brush.color);
                            if let Some(previous) = self.recent_brush_colors.iter().copied().find(|color| *color != project.brush.color) {
                                let response = ui.add_sized(
                                    [28.0, 28.0],
                                    egui::Button::new(egui::RichText::new(" ").size(1.0).color(egui::Color32::TRANSPARENT))
                                        .fill(egui::Color32::from_rgb(previous[0], previous[1], previous[2])),
                                ).on_hover_text("Previous color");
                                if response.clicked() { project.brush.color = previous; }
                            }
                        });
                            });
                    });
                if let Some(panel) = panel {
                    if self.test_path_dialog && !self.test_brush_panel_visible {
                        self.test_brush_panel_visible = true;
                        crate::studio_log!("UI_TEST_BRUSH_PANEL_VISIBLE");
                    }
                    if self.test_path_dialog
                        && screen.width() < 760.0
                        && !self.test_brush_narrow_layout_logged
                    {
                        self.test_brush_narrow_layout_logged = true;
                        let panel_rect = panel.response.rect;
                        let palette_rect = palette_response.response.rect;
                        crate::studio_log!(
                            "UI_TEST_BRUSH_NARROW_LAYOUT: {:.1} {:.1} {:.1} {:.1} {:.1} {:.1} {:.1} {:.1} {:.2}",
                            panel_rect.left(),
                            panel_rect.top(),
                            panel_rect.width(),
                            panel_rect.height(),
                            palette_rect.left(),
                            palette_rect.top(),
                            palette_rect.width(),
                            palette_rect.height(),
                            ctx.pixels_per_point(),
                        );
                    }
                    if self.test_path_dialog && !self.test_brush_window_logged {
                        self.test_brush_window_logged = true;
                        crate::studio_log!(
                            "UI_TEST_BRUSH_WINDOW_RECT: {:.1} {:.1} {:.1} {:.1} {:.2} open={}",
                            panel.response.rect.left(),
                            panel.response.rect.top(),
                            panel.response.rect.width(),
                            panel.response.rect.height(),
                            ctx.pixels_per_point(),
                            open,
                        );
                    }
                    if self.test_path_dialog && !self.test_brush_layout_logged {
                        self.test_brush_layout_logged = true;
                        crate::studio_log!(
                            "UI_TEST_BRUSH_PANEL: screen={:.0}x{:.0} palette={:?} previous={:?} size={:?} desired={:?} actual={:?} reanchor={}",
                            screen.width(),
                            screen.height(),
                            palette_response.response.rect,
                            brush_previous_rect,
                            brush_size,
                            brush_position,
                            panel.response.rect,
                            brush_needs_reanchor
                        );
                    }
                    let position = [panel.response.rect.left(), panel.response.rect.top()];
                    if position != self.brush_flyout_position {
                        self.brush_flyout_position = position;
                        workspace_changed = true;
                    }
                }
                self.brush_flyout_open = open;
            }
            if !self.brush_flyout_open {
                self.test_brush_panel_visible = false;
            }
            if self.color_flyout_open && !self.welcome && !self.new_dialog && !self.recovery_manager
            {
                let mut open = self.color_flyout_open;
                let obstacles = self.visible_workspace_obstacles(ctx, "Color controls", true);
                // Reserve the window frame and any minimum-width controls when
                // picking a collision-free position; the content size remains
                // 360 × 620 below.
                let color_size = egui::vec2(420.0, 660.0);
                let color_position = crate::studio_layout::flyout_position_avoiding(
                    screen,
                    palette_response.response.rect,
                    color_size,
                    egui::pos2(self.color_flyout_position[0], self.color_flyout_position[1]),
                    &obstacles,
                );
                let color_area = egui::AreaState::load(ctx, egui::Id::new("Color controls"));
                let color_needs_reanchor = color_area.is_some_and(|state| {
                    let current = state.rect();
                    !screen.contains_rect(current)
                        || current.intersects(palette_response.response.rect)
                        || obstacles
                            .iter()
                            .any(|obstacle| current.intersects(*obstacle))
                });
                let panel = egui::Window::new("Color controls")
                    .order(egui::Order::Foreground)
                    .frame(crate::ui_tokens::floating_panel_frame(&ctx.style().visuals))
                    .open(&mut open)
                    .resizable(true)
                    .default_size(egui::vec2(360.0, 620.0))
                    .max_size(egui::vec2(
                        (screen.width() - 16.0).max(1.0),
                        (screen.height() - 16.0).max(1.0),
                    ))
                    .default_pos(color_position)
                    .constrain_to(screen.shrink(8.0));
                let panel = if color_needs_reanchor {
                    panel.current_pos(color_position)
                } else {
                    panel
                }
                .show(ctx, |ui| {
                    ui.set_width((screen.width() - 48.0).clamp(220.0, 360.0));
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .max_height((screen.height() - 112.0).max(120.0))
                        .show(ui, |ui| {
                            let project = session.live_mut().project_mut();
                            ui.horizontal(|ui| {
                                ui.label(crate::ui_tokens::type_text(
                                    "Color",
                                    crate::ui_tokens::TypeRole::PanelTitle,
                                    ui.visuals().text_color(),
                                ));
                                ui.weak("sRGB");
                            });
                            ui.label("Pick a color or sample directly from the canvas.");
                            ui.horizontal(|ui| {
                                ui.label("Current");
                                if let Some(previous) = self
                                    .recent_brush_colors
                                    .iter()
                                    .copied()
                                    .find(|color| *color != project.brush.color)
                                {
                                    let _ = color_swatch(
                                        ui,
                                        previous,
                                        "Previous brush color",
                                        false,
                                        42.0,
                                    );
                                    ui.weak("Previous");
                                }
                            });
                            ui.color_edit_button_srgb(&mut project.brush.color);
                            saved_color_palette_contents(
                                ui,
                                &mut project.brush.color,
                                &mut self.saved_color_swatches,
                                self.test_path_dialog,
                                &mut self.test_color_palette_logged,
                                &mut workspace_changed,
                                &mut self.status,
                            );
                            let mut hsv = crate::color_controls::rgb_to_hsv(project.brush.color);
                            if hsv[1] > 0.01 && hsv[2] > 0.01 {
                                self.color_hue = hsv[0];
                            }
                            let mut hue_degrees = self.color_hue * 360.0;
                            let mut wheel_hue = None;
                            let mut hue_changed = false;
                            let mut selected_sv = None;
                            ui.label("Choose a shade");
                            ui.horizontal(|ui| {
                                let (selected_hue, wheel_response) =
                                    hue_selection_wheel(ui, self.color_hue, hsv[1], hsv[2]);
                                wheel_hue = selected_hue;
                                if self.test_path_dialog {
                                    let rect = wheel_response.rect;
                                    crate::studio_log!(
                                        "UI_TEST_HUE_WHEEL_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                        rect.left(),
                                        rect.top(),
                                        rect.width(),
                                        rect.height(),
                                        ui.ctx().pixels_per_point(),
                                    );
                                }
                                ui.vertical(|ui| {
                                    ui.label("Hue");
                                    let hue_response = ui.add_sized(
                                        [ui.available_width().clamp(110.0, 200.0), 26.0],
                                        egui::Slider::new(&mut hue_degrees, 0.0..=360.0)
                                            .suffix("°"),
                                    );
                                    hue_response.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::WidgetType::Slider,
                                            ui.is_enabled(),
                                            "Hue",
                                        )
                                    });
                                    hue_changed = hue_response.changed();
                                    let hue = wheel_hue
                                        .unwrap_or_else(|| (hue_degrees / 360.0).rem_euclid(1.0));
                                    selected_sv = saturation_value_field(ui, hue, hsv[1], hsv[2]);
                                });
                            });
                            let hue =
                                wheel_hue.unwrap_or_else(|| (hue_degrees / 360.0).rem_euclid(1.0));
                            if let Some([saturation, value]) = selected_sv {
                                hsv[1] = saturation;
                                hsv[2] = value;
                            }
                            if wheel_hue.is_some() || hue_changed || selected_sv.is_some() {
                                self.color_hue = hue;
                                project.brush.color =
                                    crate::color_controls::hsv_to_rgb([hue, hsv[1], hsv[2]]);
                            }
                            let saturation_changed = ui
                                .add(egui::Slider::new(&mut hsv[1], 0.0..=1.0).text("Saturation"))
                                .changed();
                            let value_changed = ui
                                .add(egui::Slider::new(&mut hsv[2], 0.0..=1.0).text("Brightness"))
                                .changed();
                            if hue_changed || saturation_changed || value_changed {
                                self.color_hue = hue;
                                project.brush.color =
                                    crate::color_controls::hsv_to_rgb([hue, hsv[1], hsv[2]]);
                            }
                            let current_hex =
                                crate::color_controls::format_hex_rgb(project.brush.color);
                            if self.hex_color_for != Some(project.brush.color) {
                                self.hex_color.clone_from(&current_hex);
                                self.hex_color_for = Some(project.brush.color);
                            }
                            ui.horizontal(|ui| {
                                ui.label("HEX");
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut self.hex_color)
                                        .char_limit(7)
                                        .desired_width(104.0)
                                        .hint_text("#RRGGBB"),
                                );
                                let apply = ui.button("Apply").clicked()
                                    || (response.lost_focus()
                                        && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                                if apply {
                                    if let Some(color) =
                                        crate::color_controls::parse_hex_rgb(&self.hex_color)
                                    {
                                        project.brush.color = color;
                                        self.hex_color_for = Some(color);
                                        self.status = "Brush color updated".into();
                                    } else {
                                        self.status =
                                            "Enter a six-digit color such as #C55CFF".into();
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("RGB");
                                for component in &mut project.brush.color {
                                    ui.add(egui::DragValue::new(component).range(0..=255).speed(1));
                                }
                            });
                            if ui.button("Eyedropper  ·  I").clicked() {
                                self.tool = EditTool::Eyedropper;
                                self.color_flyout_open = false;
                            }
                            ui.separator();
                            ui.strong("Recent colors");
                            ui.horizontal_wrapped(|ui| {
                                if self.recent_brush_colors.is_empty() {
                                    ui.weak("Colors you choose will appear here.");
                                }
                                for (index, color) in
                                    self.recent_brush_colors.clone().into_iter().enumerate()
                                {
                                    let hex = crate::color_controls::format_hex_rgb(color);
                                    if color_swatch(
                                        ui,
                                        color,
                                        &format!("Use color {hex}"),
                                        project.brush.color == color,
                                        38.0,
                                    ) {
                                        project.brush.color = color;
                                    }
                                    if index == crate::workspace::MAX_RECENT_BRUSH_COLORS - 1 {
                                        break;
                                    }
                                }
                            });
                        });
                });
                if let Some(panel) = panel {
                    let position = [panel.response.rect.left(), panel.response.rect.top()];
                    if position != self.color_flyout_position {
                        self.color_flyout_position = position;
                        workspace_changed = true;
                    }
                }
                self.color_flyout_open = open;
            }

            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    *central = ui.available_rect_before_wrap();
                    if session.mode() == crate::interaction::Mode::Live
                        && self.tool == EditTool::Gradient
                        && self.edit_pointer.is_some()
                        && self.edit_points.len() == 2
                        && let Some(mapping) = CanvasMapping::for_document(
                            [
                                (ctx.content_rect().width() * ctx.pixels_per_point()).round()
                                    as u32,
                                (ctx.content_rect().height() * ctx.pixels_per_point()).round()
                                    as u32,
                            ],
                            [
                                f64::from(session.live().project().state().width),
                                f64::from(session.live().project().state().height),
                            ],
                            [
                                f64::from(central.min.x * ctx.pixels_per_point()),
                                f64::from(central.min.y * ctx.pixels_per_point()),
                                f64::from(central.max.x * ctx.pixels_per_point()),
                                f64::from(central.max.y * ctx.pixels_per_point()),
                            ],
                        )
                        .and_then(|mapping| {
                            mapping.navigated(self.navigation.zoom, self.navigation.pan)
                        })
                        .ok()
                    {
                        let start = self.edit_points[0];
                        let end = self.edit_points[1];
                        let project = session.live().project();
                        let state = project.state();
                        let opacity = (project.brush.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
                        let key = gradient_preview_key(
                            [state.width, state.height],
                            self.gradient_kind,
                            start,
                            end,
                            [project.brush.color, self.gradient_end_color],
                            opacity,
                            state.selection.as_ref(),
                        );
                        if self.gradient_preview_key != Some(key) {
                            let preview = gradient_preview_image(
                                [state.width, state.height],
                                self.gradient_kind,
                                start,
                                end,
                                [project.brush.color, self.gradient_end_color],
                                opacity,
                                state.selection.as_ref(),
                            );
                            if let Some(texture) = self.gradient_preview_texture.as_mut() {
                                texture.set(preview, egui::TextureOptions::LINEAR);
                            } else {
                                self.gradient_preview_texture = Some(ctx.load_texture(
                                    "provisional-gradient-preview",
                                    preview,
                                    egui::TextureOptions::LINEAR,
                                ));
                            }
                            self.gradient_preview_key = Some(key);
                            if self.test_path_dialog {
                                crate::studio_log!(
                                    "UI_TEST_GRADIENT_PREVIEW: {:?} {} {} {}",
                                    self.gradient_kind,
                                    start[0],
                                    start[1],
                                    end[0]
                                );
                            }
                        }
                        if let Some(texture) = self.gradient_preview_texture.as_ref()
                            && let (Ok(top_left), Ok(bottom_right)) = (
                                mapping.document_to_client([0.0, 0.0]),
                                mapping.document_to_client([
                                    f64::from(state.width),
                                    f64::from(state.height),
                                ]),
                            )
                        {
                            let scale = f64::from(ctx.pixels_per_point());
                            let image_rect = egui::Rect::from_min_max(
                                egui::pos2(
                                    (top_left[0] / scale) as f32,
                                    (top_left[1] / scale) as f32,
                                ),
                                egui::pos2(
                                    (bottom_right[0] / scale) as f32,
                                    (bottom_right[1] / scale) as f32,
                                ),
                            );
                            ui.painter().image(
                                texture.id(),
                                image_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                            if let (Ok(from), Ok(to)) = (
                                mapping.document_to_client(start),
                                mapping.document_to_client(end),
                            ) {
                                let to_points = |point: [f64; 2]| {
                                    egui::pos2((point[0] / scale) as f32, (point[1] / scale) as f32)
                                };
                                let from = to_points(from);
                                let to = to_points(to);
                                ui.painter().line_segment(
                                    [from, to],
                                    egui::Stroke::new(4.0, egui::Color32::BLACK),
                                );
                                ui.painter().line_segment(
                                    [from, to],
                                    egui::Stroke::new(1.5, egui::Color32::WHITE),
                                );
                                for (point, color) in
                                    [(from, project.brush.color), (to, self.gradient_end_color)]
                                {
                                    let fill =
                                        egui::Color32::from_rgb(color[0], color[1], color[2]);
                                    ui.painter().circle_filled(point, 5.0, fill);
                                    ui.painter().circle_stroke(
                                        point,
                                        6.0,
                                        egui::Stroke::new(1.5, egui::Color32::WHITE),
                                    );
                                }
                            }
                        }
                    }
                });

            let screen_viewport = ctx.content_rect();
            let reference_count = self.references.len();
            let last_reference_index = reference_count.saturating_sub(1);
            let reference_visible_workspace = !(self.welcome
                || self.new_dialog
                || self.recovery_manager
                || compact_workspace && self.brush_flyout_open);
            let mut compact_reference_request = None;
            for (index, reference) in self.references.iter_mut().enumerate() {
                if !reference_visible_workspace {
                    reference.viewport = None;
                    continue;
                }
                if compact_workspace && index != active_compact_reference {
                    reference.viewport = None;
                    continue;
                }
                let file_name = reference
                    .layout
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Reference image")
                    .to_owned();
                // Use a stable, compact panel title instead of letting a long
                // source filename force the floating window wider than its
                // compact viewport slot. Keep the original filename available
                // in the settings menu and image tooltip.
                let title = if compact_workspace && reference_count > 1 {
                    format!("Reference {}/{}", index + 1, reference_count)
                } else {
                    format!("Reference {}", index + 1)
                };
                let reference_id = reference.layout.path.clone();
                let compact_index = usize::from(self.layers_flyout_open)
                    + if compact_workspace { 0 } else { index };
                let compact_rect = compact_companion_panels
                    .as_ref()
                    .and_then(|rects| rects.get(compact_index).copied());
                let compact = compact_rect.is_some();
                if self.test_path_dialog && compact_workspace {
                    crate::studio_log!(
                        "UI_TEST_COMPACT_REFERENCE_ACTIVE: {} {}",
                        index,
                        reference_count
                    );
                }
                if self.test_path_dialog && screen_viewport.width() < 760.0 {
                    crate::studio_log!(
                        "UI_TEST_REFERENCE_COMPACT_TARGET: {} compact={} target={:?} current={:?}",
                        index,
                        compact,
                        compact_rect,
                        compact_rect
                            .and_then(|_| egui::AreaState::load(
                                ctx,
                                reference_panel_id(&reference_id, true, compact_layout_key)
                            ))
                            .map(|state| state.rect()),
                    );
                }
                let id = reference_panel_id(&reference_id, compact, compact_layout_key);
                let mut open = true;
                let mut remove_requested = false;
                let mut center_requested = false;
                let mut reset_position_requested = false;
                let before = reference.layout.clone();
                let window_size = compact_rect.map_or_else(
                    || {
                        [
                            before.size[0].min(screen_viewport.width().max(1.0)),
                            before.size[1].min(screen_viewport.height().max(1.0)),
                        ]
                    },
                    |rect| [(rect.width() - 14.0).max(180.0), rect.height()],
                );
                let desired_pos = compact_rect.map_or_else(
                    || clamp_reference_position(before.position, window_size, screen_viewport),
                    |rect| rect.min,
                );
                let area = egui::AreaState::load(ctx, id);
                let current_rect = area.map(|state| state.rect());
                let current_pos = current_rect.map(|rect| rect.min);
                let correction = current_pos.and_then(|current| {
                    if compact {
                        let current_rect = current_rect.unwrap_or_else(|| {
                            egui::Rect::from_min_size(
                                current,
                                egui::vec2(window_size[0], window_size[1]),
                            )
                        });
                        let overlaps_other_panel = compact_companion_panels
                            .as_ref()
                            .into_iter()
                            .flatten()
                            .enumerate()
                            .any(|(other_index, other)| {
                                other_index != compact_index && current_rect.intersects(*other)
                            });
                        let overlaps_palette = self
                            .palette_rect
                            .is_some_and(|palette| current_rect.intersects(palette));
                        let invalid = !screen_viewport.contains_rect(current_rect)
                            || overlaps_palette
                            || overlaps_other_panel;
                        invalid.then_some(desired_pos)
                    } else {
                        let safe_current = clamp_reference_position(
                            [current.x, current.y],
                            window_size,
                            screen_viewport,
                        );
                        let persisted_differs = current.distance(desired_pos) > 1.0
                            && before.position != [current.x, current.y];
                        let target = if persisted_differs {
                            desired_pos
                        } else {
                            safe_current
                        };
                        (current.distance(target) > 1.0).then_some(target)
                    }
                });
                let mut window = egui::Window::new(egui::RichText::new(title).size(18.0).strong())
                    .order(egui::Order::Foreground)
                    .id(id)
                    .open(&mut open)
                    .movable(!before.position_locked)
                    .resizable(!compact)
                    .max_size(if compact {
                        egui::vec2(window_size[0], window_size[1])
                    } else {
                        screen_viewport.size()
                    })
                    .default_size(egui::vec2(window_size[0], window_size[1]));
                if let Some(pos) = correction {
                    window = window.current_pos(pos);
                } else {
                    window = window.default_pos(desired_pos);
                }
                if compact {
                    window = window.fixed_size(egui::vec2(window_size[0], window_size[1]));
                }
                let response = window.show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if compact {
                            ui.spacing_mut().item_spacing.x = 2.0;
                        }
                        if compact && reference_count > 1 {
                            let previous = ui
                                .add_sized(
                                    [if compact { 24.0 } else { 28.0 }, 32.0],
                                    egui::Button::new("‹"),
                                )
                                .on_hover_text("Show previous reference image");
                            previous.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    ui.is_enabled(),
                                    "Show previous reference image",
                                )
                            });
                            if previous.clicked() {
                                compact_reference_request =
                                    Some((index + reference_count - 1) % reference_count);
                            }
                            ui.small(format!("{}/{}", index + 1, reference_count));
                            let next = ui
                                .add_sized(
                                    [if compact { 24.0 } else { 28.0 }, 32.0],
                                    egui::Button::new("›"),
                                )
                                .on_hover_text("Show next reference image");
                            next.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    ui.is_enabled(),
                                    "Show next reference image",
                                )
                            });
                            if next.clicked() {
                                compact_reference_request = Some((index + 1) % reference_count);
                            }
                            if self.test_path_dialog {
                                crate::studio_log!(
                                    "UI_TEST_REFERENCE_COMPACT_NAV: {} {} {:.1} {:.1} {:.1} {:.1} {:.2}",
                                    index,
                                    reference_count,
                                    next.rect.left(),
                                    next.rect.top(),
                                    next.rect.width(),
                                    next.rect.height(),
                                    ctx.pixels_per_point(),
                                );
                            }
                        }
                        let zoom_out = ui
                            .add_sized(
                                [if compact { 24.0 } else { 36.0 }, 32.0],
                                egui::Button::new("−"),
                            )
                            .on_hover_text("Zoom out reference image");
                        zoom_out.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                "Zoom out reference image",
                            )
                        });
                        if zoom_out.clicked() {
                            if let Some(viewport) = reference.viewport {
                                reference_zoom_about(
                                    &mut reference.layout,
                                    viewport,
                                    viewport.center(),
                                    viewport.center(),
                                    0.8,
                                );
                            } else {
                                reference.layout.view_zoom =
                                    (reference.layout.view_zoom * 0.8).max(0.25);
                            }
                        }
                        let zoom_in = ui
                            .add_sized(
                                [if compact { 24.0 } else { 36.0 }, 32.0],
                                egui::Button::new("+"),
                            )
                            .on_hover_text("Zoom in reference image");
                        zoom_in.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                "Zoom in reference image",
                            )
                        });
                        if zoom_in.clicked() {
                            if let Some(viewport) = reference.viewport {
                                reference_zoom_about(
                                    &mut reference.layout,
                                    viewport,
                                    viewport.center(),
                                    viewport.center(),
                                    1.25,
                                );
                            } else {
                                reference.layout.view_zoom =
                                    (reference.layout.view_zoom * 1.25).min(8.0);
                            }
                        }
                        ui.weak(format!("{:.0}%", reference.layout.view_zoom * 100.0));
                        let settings = ui.menu_button(if compact { "⋯" } else { "Settings" }, |ui| {
                            ui.weak(file_name);
                            ui.separator();
                            let center_image = ui.button("Center image");
                            if self.test_path_dialog {
                                crate::studio_log!(
                                    "UI_TEST_REFERENCE_CENTER_IMAGE_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                    center_image.rect.left(),
                                    center_image.rect.top(),
                                    center_image.rect.width(),
                                    center_image.rect.height(),
                                    ui.ctx().pixels_per_point(),
                                );
                            }
                            if center_image.clicked() {
                                reference.layout.view_pan = [0.0, 0.0];
                                if self.test_path_dialog && self.test_reference_import.is_some() {
                                    self.test_reference_centered = true;
                                }
                                ui.close();
                            }
                            if ui.button("Fit image to window").clicked() {
                                reference.layout.view_zoom = 1.0;
                                reference.layout.view_pan = [0.0, 0.0];
                                ui.close();
                            }
                            if ui.button("Center reference window").clicked() {
                                center_requested = true;
                                ui.close();
                            }
                            if ui.button("Reset window position").clicked() {
                                reset_position_requested = true;
                                ui.close();
                            }
                            ui.checkbox(
                                &mut reference.layout.position_locked,
                                "Lock window position",
                            );
                            ui.add(
                                egui::Slider::new(&mut reference.layout.opacity, 0.05..=1.0)
                                    .text("Opacity"),
                            );
                            if ui.button("Remove reference…").clicked() {
                                remove_requested = true;
                                ui.close();
                            }
                        });
                        settings.response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                "Reference settings",
                            )
                        });
                        let settings_rect = settings.response.rect;
                        settings.response.on_hover_text("Reference settings");
                        if self.test_path_dialog {
                            crate::studio_log!(
                                "UI_TEST_REFERENCE_SETTINGS_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                settings_rect.left(),
                                settings_rect.top(),
                                settings_rect.width(),
                                settings_rect.height(),
                                ctx.pixels_per_point(),
                            );
                        }
                    });
                    let mut available = ui.available_size().max(egui::vec2(48.0, 48.0));
                    if compact {
                        // A reference board can have a fixed minimum body height
                        // for its image viewport. Cap that viewport temporarily
                        // so the decorated window stays above the app status
                        // strip at 640 × 480; larger layouts retain the natural
                        // image area and the saved panel size is untouched.
                        let source_size = reference.texture.size_vec2();
                        let fit_height = if source_size.x > 0.0 {
                            available.x * source_size.y / source_size.x
                        } else {
                            132.0
                        };
                        let panel_content_height = compact_rect
                            .map(|rect| (rect.height() - 132.0).max(48.0))
                            .unwrap_or(132.0);
                        available.y = available
                            .y
                            .min(fit_height)
                            .min(panel_content_height)
                            .max(48.0);
                    }
                    let source_size = reference.texture.size_vec2();
                    let (viewport, image_response) =
                        ui.allocate_exact_size(available, egui::Sense::drag());
                    image_response.clone().on_hover_text(format!(
                        "Original image: {} × {} pixels",
                        reference.source_size[0], reference.source_size[1]
                    ));
                    reference.viewport = Some(viewport);
                    if image_response.hovered() {
                        let scroll_y = ui.input(|input| input.smooth_scroll_delta.y);
                        if scroll_y != 0.0
                            && let Some(pointer) = image_response.hover_pos()
                        {
                            reference_zoom_about(
                                &mut reference.layout,
                                viewport,
                                pointer,
                                pointer,
                                (scroll_y * 0.002).exp(),
                            );
                        }
                    }
                    if image_response.dragged() {
                        let delta = image_response.drag_delta();
                        reference.layout.view_pan[0] =
                            (reference.layout.view_pan[0] + delta.x).clamp(-10000.0, 10000.0);
                        reference.layout.view_pan[1] =
                            (reference.layout.view_pan[1] + delta.y).clamp(-10000.0, 10000.0);
                    }
                    let image_rect = reference_image_rect(
                        viewport,
                        source_size,
                        reference.layout.view_zoom,
                        reference.layout.view_pan,
                    );
                    if let Some(image_rect) = image_rect {
                        let visible_image_rect = image_rect.intersect(viewport);
                        if self.test_path_dialog
                            && reference_visible_workspace
                            && visible_image_rect.is_positive()
                        {
                            if compact_workspace {
                                crate::studio_log!(
                                    "UI_TEST_REFERENCE_COMPACT_IMAGE_RECT: {} {:.1} {:.1} {:.1} {:.1} {:.2}",
                                    index,
                                    visible_image_rect.left(),
                                    visible_image_rect.top(),
                                    visible_image_rect.width(),
                                    visible_image_rect.height(),
                                    ctx.pixels_per_point(),
                                );
                            }
                            crate::studio_log!(
                                "UI_TEST_REFERENCE_IMAGE_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                visible_image_rect.left(),
                                visible_image_rect.top(),
                                visible_image_rect.width(),
                                visible_image_rect.height(),
                                ctx.pixels_per_point(),
                            );
                            if index == last_reference_index {
                                crate::studio_log!(
                                    "UI_TEST_REFERENCE_LAST_IMAGE_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                    visible_image_rect.left(),
                                    visible_image_rect.top(),
                                    visible_image_rect.width(),
                                    visible_image_rect.height(),
                                    ctx.pixels_per_point(),
                                );
                            }
                        }
                        ui.painter().with_clip_rect(viewport).image(
                            reference.texture.id(),
                            image_rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            egui::Color32::from_white_alpha(
                                (reference.layout.opacity * 255.0).round() as u8,
                            ),
                        );
                        if self.test_path_dialog
                            && reference_visible_workspace
                            && !self.test_reference_preview_logged
                        {
                            self.test_reference_preview_logged = true;
                            crate::studio_log!(
                                "UI_TEST_REFERENCE_IMAGE_VISIBLE: {}x{}",
                                reference.source_size[0], reference.source_size[1]
                            );
                        }
                    }
                    if image_response.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if image_response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                });
                if let Some(response) = response {
                    let rect = response.response.rect;
                    if self.test_path_dialog && reference_visible_workspace {
                        crate::studio_log!(
                            "UI_TEST_REFERENCE_WINDOW_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                            rect.left(),
                            rect.top(),
                            rect.width(),
                            rect.height(),
                            ctx.pixels_per_point(),
                        );
                        crate::studio_log!(
                            "UI_TEST_REFERENCE_LAYOUT_RECT: {} {:.1} {:.1} {:.1} {:.1} {:.2}",
                            index,
                            rect.left(),
                            rect.top(),
                            rect.width(),
                            rect.height(),
                            ctx.pixels_per_point(),
                        );
                        if index == last_reference_index {
                            crate::studio_log!(
                                "UI_TEST_REFERENCE_LAST_WINDOW_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                rect.left(),
                                rect.top(),
                                rect.width(),
                                rect.height(),
                                ctx.pixels_per_point(),
                            );
                        }
                    }
                    let next_position = if center_requested {
                        centered_reference_position([rect.width(), rect.height()], screen_viewport)
                    } else if reset_position_requested {
                        clamp_reference_position(
                            [40.0 + index as f32 * 28.0, 80.0 + index as f32 * 24.0],
                            [rect.width(), rect.height()],
                            screen_viewport,
                        )
                    } else {
                        clamp_reference_position(
                            [rect.min.x, rect.min.y],
                            [rect.width(), rect.height()],
                            screen_viewport,
                        )
                    };
                    if !compact || center_requested || reset_position_requested {
                        reference.layout.position = [next_position.x, next_position.y];
                    }
                    if !compact {
                        reference.layout.size = [rect.width(), rect.height()];
                    }
                    if center_requested || reset_position_requested {
                        workspace_changed = true;
                    }
                    workspace_changed |= reference.layout != before;
                }
                if !open || remove_requested {
                    workspace_changed = true;
                    reference.layout.opacity = 0.0;
                }
            }
            if let Some(next_reference) = compact_reference_request
                && next_reference < self.references.len()
            {
                self.compact_reference_index = next_reference;
                if self.test_path_dialog {
                    crate::studio_log!("UI_TEST_REFERENCE_COMPACT_ACTIVE: {next_reference}");
                }
                ctx.request_repaint();
            }
            if self
                .references
                .iter()
                .any(|reference| reference.layout.size[0] == 0.0)
            {
                // Invalid persisted geometry is sanitized by WorkspacePreferences.
                workspace_changed = true;
            }
            self.references
                .retain(|reference| reference.layout.opacity >= 0.05);
            if self.test_path_dialog && reference_visible_workspace {
                crate::studio_log!("UI_TEST_REFERENCE_PANEL_COUNT: {}", self.references.len());
            }
            if workspace_changed
                && !ctx.input(|input| input.pointer.any_down())
                && let Err(error) = self.persist_workspace()
            {
                self.status = format!("Could not save workspace layout: {error}");
            }
        }

        if !self.welcome
            && !self.new_dialog
            && !self.manual_file_dialog
            && !self.brush_flyout_open
            && !self.layers_flyout_open
            && !self.color_flyout_open
        {
            let screen = ctx.content_rect();
            egui::Area::new(egui::Id::new("canvas zoom controls"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(screen.right() - 250.0, screen.bottom() - 48.0))
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(ui.visuals().panel_fill)
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.small_button("−").on_hover_text("Zoom out").clicked() {
                                    self.workspace_needs_save |= self.navigation.pinch(
                                        self.workspace_center,
                                        self.workspace_center,
                                        self.workspace_center,
                                        0.8,
                                    );
                                }
                                ui.weak(format!("{:.0}%", self.navigation.zoom * 100.0));
                                if ui.small_button("+").on_hover_text("Zoom in").clicked() {
                                    self.workspace_needs_save |= self.navigation.pinch(
                                        self.workspace_center,
                                        self.workspace_center,
                                        self.workspace_center,
                                        1.25,
                                    );
                                }
                                if ui.button("Fit canvas").clicked() {
                                    self.navigation = Default::default();
                                    self.workspace_needs_save = true;
                                }
                            });
                        });
                });
        }

        if self.welcome {
            let screen = ctx.content_rect();
            let side_pad = if screen.width() < 720.0 { 16.0 } else { 32.0 };
            let mut open_recent = None;
            let mut open_project_requested = false;
            let mut remove_draft = None;
            let mut start_preset = None;
            let mut custom_size_requested = false;
            let featured_recent = self
                .recent_cards
                .iter()
                .find(|card| card.bytes.is_some() && card.thumbnail.is_some())
                .cloned();
            let has_featured_artwork = featured_recent
                .as_ref()
                .is_some_and(|card| card.thumbnail.is_some());
            egui::Area::new(egui::Id::new("studio home screen"))
                .order(egui::Order::Foreground)
                .fixed_pos(screen.min)
                .interactable(true)
                .show(ctx, |ui| {
                    ui.set_min_size(screen.size());
                    ui.set_max_size(screen.size());
                    egui::Frame::new()
                        .fill(ui.visuals().panel_fill)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                    let content_width = screen.width().min(1600.0);
                                    let content_left = ((screen.width() - content_width) * 0.5).max(0.0);
                                    ui.horizontal(|ui| {
                                        ui.add_space(content_left);
                                        ui.vertical(|ui| {
                                            ui.set_width(content_width);
                                            if self.test_path_dialog
                                                && screen.width() >= 1800.0
                                                && !self.test_home_content_bounds_logged
                                            {
                                                self.test_home_content_bounds_logged = true;
                                                let rect = ui.available_rect_before_wrap();
                                                crate::studio_log!(
                                                    "UI_TEST_HOME_CONTENT_BOUNDS: {:.1} {:.1} {:.1} {:.1}",
                                                    rect.left(), rect.top(), rect.width(), rect.height(),
                                                );
                                            }
                                ui.add_space(if screen.height() < 620.0 { 10.0 } else { 18.0 });
                                let hero_margin = if screen.width() < 720.0 { 16 } else { 28 };
                                crate::ui_tokens::hero_frame(ui.visuals())
                                    .inner_margin(egui::Margin::same(hero_margin))
                                    .show(ui, |ui| {
                                        ui.set_max_height(home_hero_height(screen.width()));
                                            ui.set_min_width((content_width - side_pad * 2.0).max(240.0));
                                        if !self.high_contrast {
                                            let bounds = ui.available_rect_before_wrap();
                                            let accent = ui.visuals().hyperlink_color;
                                            let radius = (bounds.width() * 0.17).clamp(80.0, 230.0);
                                            // A low-contrast violet halo gives the hero depth
                                            // without introducing fake artwork or competing
                                            // with an actual recent-project thumbnail.
                                            ui.painter().circle_filled(
                                                egui::pos2(bounds.right() - radius * 0.34, bounds.top() + radius * 0.24),
                                                radius,
                                                accent.gamma_multiply(if ui.visuals().dark_mode { 0.08 } else { 0.045 }),
                                            );
                                        }
                                        ui.horizontal(|ui| {
                                            crate::ui_tokens::brand_wordmark(ui, 40.0);
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                ui.weak("PAINT  ·  ILLUSTRATE  ·  EXPLORE");
                                            });
                                        });
                                        ui.add_space(if screen.width() < 760.0 { 8.0 } else { 18.0 });
                                        let compact = screen.width() < 760.0;
                                        ui.with_layout(
                                            if compact { egui::Layout::top_down(egui::Align::Min) } else { egui::Layout::left_to_right(egui::Align::Center) },
                                            |ui| {
                                                ui.vertical(|ui| {
                                                    ui.set_width(if compact {
                                                        content_width
                                                    } else {
                                                        content_width * 0.46
                                                    });
                                                    let surface = crate::ui_tokens::hero_surface(ui.visuals());
                                                    let foreground = crate::appearance::readable_text_color(
                                                        ui.visuals().text_color(), surface,
                                                    );
                                                    let muted = crate::appearance::readable_text_color(
                                                        ui.visuals().weak_text_color(), surface,
                                                    );
                                                    let dark_mode = ui.visuals().dark_mode;
                                                    let contrast = crate::appearance::contrast_ratio(foreground, surface);
                                                    let accent = crate::appearance::readable_text_accent(ui.visuals().hyperlink_color, surface);
                                                    let slogan_gradient = if self.high_contrast {
                                                        None
                                                    } else {
                                                        crate::appearance::accessible_text_gradient(
                                                            egui::Color32::from_rgb(165, 111, 255),
                                                            egui::Color32::from_rgb(235, 112, 220),
                                                            surface,
                                                        )
                                                    };
                                                    let slogan_contrast = slogan_gradient.map_or_else(
                                                        || crate::appearance::contrast_ratio(accent, surface),
                                                        |colors| crate::appearance::text_gradient_min_contrast(colors, surface),
                                                    );
                                                    if self.test_path_dialog && contrast >= 4.5 {
                                                        let logged = if dark_mode {
                                                            &mut self.test_home_dark_contrast_logged
                                                        } else {
                                                            &mut self.test_home_light_contrast_logged
                                                        };
                                                        if !*logged {
                                                            *logged = true;
                                                            crate::studio_log!("UI_TEST_HOME_HEADLINE_CONTRAST: {} {:.2}", if dark_mode { "dark" } else { "light" }, contrast);
                                                            crate::studio_log!("UI_TEST_HOME_SLOGAN_CONTRAST: {} {:.2}", if dark_mode { "dark" } else { "light" }, slogan_contrast);
                                                        }
                                                    }
                                                    if let Some(colors) = slogan_gradient {
                                                        let response = ui.add(
                                                            egui::Label::new(crate::ui_tokens::gradient_type_job(
                                                                "No limits. No bullshit.",
                                                                crate::ui_tokens::TypeRole::Slogan,
                                                                colors,
                                                            ))
                                                            .wrap(),
                                                        );
                                                        response.widget_info(|| {
                                                            egui::WidgetInfo::labeled(
                                                                egui::WidgetType::Label,
                                                                true,
                                                                "No limits. No bullshit.",
                                                            )
                                                        });
                                                    } else {
                                                        ui.label(crate::ui_tokens::type_text(
                                                            "No limits. No bullshit.",
                                                            crate::ui_tokens::TypeRole::Slogan,
                                                            accent,
                                                        ));
                                                    }
                                                    ui.add_space(4.0);
                                                    ui.label(crate::ui_tokens::display_text(
                                                        "Create, open, or continue artwork.",
                                                        (ui.available_width() * 0.065).clamp(30.0, 46.0),
                                                        foreground,
                                                    ));
                                                    ui.add_space(8.0);
                                                    ui.label(egui::RichText::new("Start something new, open a project, or continue where you left off.")
                                                        .size(if compact { 16.0 } else { 17.5 }).color(muted));
                                                    ui.add_space(if compact { 8.0 } else { 12.0 });
                                                    ui.horizontal_wrapped(|ui| {
                                                        let action_width = (screen.width() * 0.42).clamp(168.0, 232.0);
                                                        let action_height = if compact { 48.0 } else { 54.0 };
                                                        let accent_fill = ui.visuals().hyperlink_color;
                                                        let action_text = crate::appearance::readable_text_color(
                                                            ui.visuals().text_color(),
                                                            accent_fill,
                                                        );
                                                        let primary = egui::Button::new(crate::ui_tokens::type_text(
                                                            "New artwork",
                                                            crate::ui_tokens::TypeRole::Button,
                                                            action_text,
                                                        ))
                                                            .fill(accent_fill)
                                                            .min_size(egui::vec2(action_width, action_height));
                                                        if ui.add(primary).clicked() {
                                                            self.picker_from_welcome = false;
                                                            if !self.test_path_dialog { self.new_name = "Untitled".into(); }
                                                            self.new_from_welcome = true;
                                                            self.new_dialog = true;
                                                            self.welcome = false;
                                                        }
                                                        if ui.add_sized([action_width, action_height], egui::Button::new("Open project…")).clicked() {
                                                            self.picker_from_welcome = true;
                                                            self.picker = Some(FileRequest::OpenProject);
                                                            self.welcome = false;
                                                        }
                                                    });
                                                    if !compact {
                                                        ui.add_space(5.0);
                                                        ui.label(egui::RichText::new("Offline by default · Your work stays on this device.")
                                                            .size(13.0).color(muted));
                                                    }
                                                });
                                                if has_featured_artwork {
                                                    ui.add_space(18.0);
                                                    let preview_drawn = home_featured_artwork(
                                                        ui,
                                                        featured_recent.as_ref(),
                                                        &mut open_recent,
                                                        screen.height(),
                                                    );
                                                    if self.test_path_dialog
                                                        && preview_drawn
                                                        && !self.test_home_preview_logged
                                                    {
                                                        self.test_home_preview_logged = true;
                                                        crate::studio_log!("UI_TEST_HOME_RECENT_PREVIEW_VISIBLE");
                                                    }
                                                } else if !compact {
                                                    // A first-run Home still uses its generous
                                                    // horizontal hero for real choices, rather
                                                    // than leaving an empty image-sized void.
                                                    ui.add_space(18.0);
                                                    ui.vertical(|ui| {
                                                        ui.heading("Start with a canvas");
                                                        ui.weak("Popular formats · 300 DPI");
                                                        ui.add_space(6.0);
                                                        ui.horizontal_wrapped(|ui| {
                                                            let (_, card_width) =
                                                                quick_document_card_layout(
                                                                    (ui.available_width() - 8.0)
                                                                        .max(240.0),
                                                                );
                                                            for (name, width, height) in [
                                                                ("Portrait", 1800, 2400),
                                                                ("Square", 2048, 2048),
                                                                ("Landscape", 2400, 1800),
                                                            ] {
                                                                let (activated, rect) = home_document_card_with_rect(
                                                                    ui,
                                                                    name,
                                                                    [width, height],
                                                                    card_width,
                                                                );
                                                                let marker = [
                                                                    rect.left().round() as i32,
                                                                    rect.top().round() as i32,
                                                                    rect.width().round() as i32,
                                                                    rect.height().round() as i32,
                                                                    (ctx.pixels_per_point() * 100.0).round() as i32,
                                                                ];
                                                                if self.test_path_dialog
                                                                    && name == "Portrait"
                                                                    && self.test_home_preset_rect_logged != Some(marker)
                                                                {
                                                                    self.test_home_preset_rect_logged = Some(marker);
                                                                    crate::studio_log!(
                                                                        "UI_TEST_HOME_PRESET_CARD_RECT|Portrait|{:.1} {:.1} {:.1} {:.1} {:.2}",
                                                                        rect.left(), rect.top(), rect.width(), rect.height(), ctx.pixels_per_point(),
                                                                    );
                                                                }
                                                                if activated {
                                                                    start_preset =
                                                                        Some((width, height));
                                                                }
                                                            }
                                                        });
                                                        if ui
                                                            .button("Custom or print size…")
                                                            .clicked()
                                                        {
                                                            custom_size_requested = true;
                                                        }
                                                        if self.test_path_dialog
                                                            && !self.test_home_quick_start_logged
                                                        {
                                                            self.test_home_quick_start_logged = true;
                                                            crate::studio_log!("UI_TEST_HOME_QUICK_START_VISIBLE");
                                                        }
                                                    });
                                                }
                                            },
                                        );
                                    });
                                if has_featured_artwork || screen.width() < 760.0 {
                                    ui.add_space(12.0);
                                    ui.horizontal(|ui| {
                                        ui.add_space(side_pad);
                                        crate::ui_tokens::section_heading(
                                            ui,
                                            "Start with a canvas",
                                            Some("Choose a format, then fine-tune it"),
                                        );
                                    });
                                    ui.add_space(8.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.add_space(side_pad);
                                        let (_, card_width) = quick_document_card_layout(
                                            (ui.available_width() - side_pad - 8.0).max(240.0),
                                        );
                                        for (name, width, height) in [
                                            ("Portrait", 1800, 2400),
                                            ("Square", 2048, 2048),
                                            ("Landscape", 2400, 1800),
                                            ("A4 Print", 2480, 3508),
                                        ] {
                                            let (activated, rect) = home_document_card_with_rect(
                                                ui,
                                                name,
                                                [width, height],
                                                card_width,
                                            );
                                            let marker = [
                                                rect.left().round() as i32,
                                                rect.top().round() as i32,
                                                rect.width().round() as i32,
                                                rect.height().round() as i32,
                                                (ctx.pixels_per_point() * 100.0).round() as i32,
                                            ];
                                            if self.test_path_dialog
                                                && name == "Portrait"
                                                && self.test_home_preset_rect_logged != Some(marker)
                                            {
                                                self.test_home_preset_rect_logged = Some(marker);
                                                crate::studio_log!(
                                                    "UI_TEST_HOME_PRESET_CARD_RECT|Portrait|{:.1} {:.1} {:.1} {:.1} {:.2}",
                                                    rect.left(), rect.top(), rect.width(), rect.height(), ctx.pixels_per_point(),
                                                );
                                            }
                                            if activated {
                                                start_preset = Some((width, height));
                                            }
                                        }
                                        if home_custom_document_card(ui, card_width) {
                                            custom_size_requested = true;
                                        }
                                    });
                                }
                                ui.add_space(if screen.height() < 620.0 { 16.0 } else { 24.0 });
                                ui.with_layout(
                                    if home_sections_stacked(screen.width()) {
                                        egui::Layout::top_down(egui::Align::Min)
                                    } else {
                                        egui::Layout::left_to_right(egui::Align::Min)
                                    },
                                    |ui| {
                                    ui.add_space(side_pad);
                                    ui.vertical(|ui| {
                                        crate::ui_tokens::section_heading(
                                            ui,
                                            "Recent artwork",
                                            Some("Ordered by most recently opened"),
                                        );
                                        ui.add_space(8.0);
                                        if self.recent.paths.is_empty() {
                                            crate::ui_tokens::card_frame(ui.visuals(), false).inner_margin(egui::Margin::same(22)).show(ui, |ui| {
                                                ui.set_min_width((screen.width() - 100.0).max(300.0));
                                                ui.label(crate::ui_tokens::type_text(
                                                    "Nothing recent yet",
                                                    crate::ui_tokens::TypeRole::CardTitle,
                                                    ui.visuals().text_color(),
                                                ));
                                                ui.label("Your projects will appear here after you save or open one.");
                                            });
                                        } else {
                                            recent_search_field(ui, &mut self.recent_search);
                                            let visible_recent = self.recent_cards.iter()
                                                .filter(|card| recent_project_matches(&card.path, &self.recent_search))
                                                .cloned()
                                                .collect::<Vec<_>>();
                                            if visible_recent.is_empty() {
                                                ui.add_space(8.0);
                                                ui.label(egui::RichText::new("No recent artwork matches that filename.").color(ui.visuals().weak_text_color()));
                                            } else {
                                            let gallery_width = ui.available_width();
                                            let max_columns = recent_gallery_max_columns(gallery_width);
                                            let displayed_count = visible_recent.len().min(8);
                                            let include_open_card = home_open_card_needed(
                                                displayed_count,
                                                max_columns,
                                                self.recent_search.trim().is_empty(),
                                            );
                                            let (columns, card_width) = recent_gallery_layout(
                                                gallery_width,
                                                displayed_count + usize::from(include_open_card),
                                            );
                                            egui::Grid::new("home recent projects").num_columns(columns).spacing([14.0, 14.0]).show(ui, |ui| {
                                                for (index, card) in visible_recent.iter().take(8).enumerate() {
                                                    let path = &card.path;
                                                    let thumbnail = card.thumbnail.clone();
                                                    let name = path.file_stem().unwrap_or_default().to_string_lossy();
                                                    let details = if let Some(bytes) = card.bytes {
                                                        let size_mb = bytes as f64 / (1024.0 * 1024.0);
                                                        let dimensions = card.dimensions.map(|(w, h, dpi)| format!("{w} × {h} px · {dpi} DPI")).unwrap_or_else(|| "Dimensions unavailable".into());
                                                        format!("{} · {size_mb:.1} MiB\n{}", dimensions, recent_opened_label(card.opened_unix_seconds))
                                                    } else {
                                                        "File moved or unavailable\nUse Open to locate the project again".into()
                                                    };
                                                    let available = card.bytes.is_some();
                                                    let card = crate::ui_tokens::card_frame(ui.visuals(), false).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                                                        ui.set_min_width(card_width);
                                                        ui.set_min_height(290.0);
                                                        ui.vertical(|ui| {
                                                            if let Some(texture) = thumbnail {
                                                                let source_size = texture.size();
                                                                let preview_height = recent_artwork_preview_height(card_width);
                                                                let scale = ((card_width - 30.0) / source_size[0] as f32).min(preview_height / source_size[1] as f32);
                                                                let image_size = egui::vec2(source_size[0] as f32 * scale, source_size[1] as f32 * scale);
                                                                let image_frame = egui::Frame::new().fill(ui.visuals().extreme_bg_color).corner_radius(8.0).inner_margin(4.0).show(ui, |ui| {
                                                                    ui.set_min_size(egui::vec2(card_width - 36.0, preview_height));
                                                                    ui.centered_and_justified(|ui| {
                                                                        ui.add(egui::Image::new((texture.id(), image_size)).fit_to_exact_size(image_size));
                                                                    });
                                                                });
                                                                if self.test_path_dialog
                                                                    && screen.width() >= 1800.0
                                                                    && index == 0
                                                                {
                                                                    let rect = image_frame.response.rect;
                                                                    crate::studio_log!(
                                                                        "UI_TEST_HOME_RECENT_THUMBNAIL_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                                                        rect.left(),
                                                                        rect.top(),
                                                                        rect.width(),
                                                                        rect.height(),
                                                                        ui.ctx().pixels_per_point(),
                                                                    );
                                                                }
                                                            } else {
                                                                egui::Frame::new().fill(ui.visuals().extreme_bg_color).corner_radius(6.0).show(ui, |ui| {
                                                                    ui.set_min_size(egui::vec2(card_width - 30.0, (card_width * 0.62).clamp(140.0, 220.0)));
                                                                    ui.centered_and_justified(|ui| { ui.weak("Artwork preview"); });
                                                                });
                                                            }
                                                            if self.test_path_dialog {
                                                                let badge_fill = ui.visuals().hyperlink_color;
                                                                let badge_text = crate::appearance::readable_text_color(
                                                                    ui.visuals().text_color(),
                                                                    badge_fill,
                                                                );
                                                                let badge = egui::Frame::new()
                                                                    .fill(badge_fill)
                                                                    .corner_radius(4.0)
                                                                    .inner_margin(egui::Margin::symmetric(6, 2))
                                                                    .show(ui, |ui| {
                                                                        ui.label(crate::ui_tokens::type_text(
                                                                            "DEVELOPMENT TEST FIXTURE",
                                                                            crate::ui_tokens::TypeRole::Caption,
                                                                            badge_text,
                                                                        ));
                                                                    });
                                                                if index == 0 {
                                                                    let rect = badge.response.rect;
                                                                    crate::studio_log!(
                                                                        "UI_TEST_HOME_FIXTURE_BADGE_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                                                                        rect.left(),
                                                                        rect.top(),
                                                                        rect.width(),
                                                                        rect.height(),
                                                                        ui.ctx().pixels_per_point(),
                                                                    );
                                                                }
                                                            }
                                                            ui.add_space(4.0);
                                                            ui.label(crate::ui_tokens::type_text(
                                                                name.to_string(),
                                                                crate::ui_tokens::TypeRole::CardTitle,
                                                                ui.visuals().text_color(),
                                                            ));
                                                            ui.label(crate::ui_tokens::type_text(
                                                                details,
                                                                crate::ui_tokens::TypeRole::Metadata,
                                                                ui.visuals().weak_text_color(),
                                                            ));
                                                            if !available { ui.colored_label(ui.visuals().error_fg_color, "Missing project"); }
                                                        });
                                                    });
                                                    let visible_card = card.response.rect.intersect(ui.clip_rect());
                                                    if self.test_path_dialog
                                                        && screen.width() >= 1800.0
                                                    {
                                                        crate::studio_log!(
                                                            "UI_TEST_HOME_RECENT_CARD_GEOMETRY: {:.1} {:.1} {:.1} {:.1} {:.1}",
                                                            card.response.rect.left(),
                                                            card.response.rect.top(),
                                                            card.response.rect.width(),
                                                            visible_card.height(),
                                                            card.response.rect.height(),
                                                        );
                                                    }
                                                    if available {
                                                        let response = ui.interact(
                                                            card.response.rect,
                                                            egui::Id::new(("recent project", index)),
                                                            egui::Sense::click(),
                                                        );
                                                        let project_label = format!("Open project {}", path.file_name().unwrap_or_default().to_string_lossy());
                                                        response.widget_info(|| egui::WidgetInfo::labeled(
                                                            egui::WidgetType::Button,
                                                            ui.is_enabled(),
                                                            project_label.clone(),
                                                        ));
                                                        if response.clicked() {
                                                            open_recent = Some(path.clone());
                                                        }
                                                    }
                                                    if (index + 1) % columns == 0
                                                        || (!include_open_card && index == visible_recent.len().min(8) - 1)
                                                    { ui.end_row(); }
                                                }
                                                if include_open_card {
                                                    if home_open_project_card(ui, card_width, self.test_path_dialog) {
                                                        open_project_requested = true;
                                                    }
                                                    if (visible_recent.len() + 1) % columns == 0 {
                                                        ui.end_row();
                                                    }
                                                }
                                            });
                                            }
                                        }
                                    });
                                    ui.add_space(24.0);
                                    ui.vertical(|ui| {
                                        crate::ui_tokens::section_heading(
                                            ui,
                                            "Continue safely",
                                            None,
                                        );
                                        ui.label("Recovery and unnamed work stay separate from exported artwork.");
                                        if !self.recovery_files.is_empty() {
                                            ui.add_space(8.0);
                                            egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(10.0).inner_margin(12.0).show(ui, |ui| {
                                                ui.label(crate::ui_tokens::type_text(
                                                    "Recoverable work",
                                                    crate::ui_tokens::TypeRole::PanelTitle,
                                                    ui.visuals().text_color(),
                                                ));
                                                if ui.button(format!("Manage {} recovery checkpoints…", self.recovery_files.len())).clicked() {
                                                    self.recovery_files = paint_document::recovery::directory()
                                                        .map(|directory| paint_document::recovery::available(&directory))
                                                        .unwrap_or_default();
                                                    self.recovery_preview = None;
                                                    self.recovery_manager = true;
                                                    self.recovery_manager_from_home = true;
                                                }
                                                for path in self.recovery_files.iter().take(4) {
                                                    let label = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unsaved document").replace(['_', '-'], " ");
                                                    if ui.button(format!("Recover · {label}")).clicked() { actions.push(Action::Recover(path.clone())); self.welcome = false; }
                                                }
                                            });
                                        } else if ui.button("Recovery manager…").clicked() {
                                            self.recovery_files = paint_document::recovery::directory()
                                                .map(|directory| paint_document::recovery::available(&directory))
                                                .unwrap_or_default();
                                            self.recovery_preview = None;
                                            self.recovery_manager = true;
                                            self.recovery_manager_from_home = true;
                                        }
                                        if !self.drafts.is_empty() {
                                            ui.add_space(10.0);
                                            egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(10.0).inner_margin(12.0).show(ui, |ui| {
                                                ui.label(egui::RichText::new("Saved drafts").strong());
                                                for path in self.drafts.iter().take(crate::drafts::MAX_DRAFTS) {
                                                    ui.horizontal(|ui| {
                                                        if ui.button(crate::drafts::display_name(path)).clicked() { self.picker_from_welcome = true; actions.push(Action::Recent(path.clone())); self.welcome = false; }
                                                        if ui.small_button("Remove…").clicked() { remove_draft = Some(path.clone()); }
                                                    });
                                                }
                                            });
                                        }
                                    });
                                    },
                                );
                                ui.add_space(24.0);
                                ui.horizontal(|ui| { ui.add_space(side_pad); ui.small("Ctrl+N New artwork · Ctrl+O Open · Ctrl+Shift+N New layer"); });
                                ui.add_space(22.0);
                                        });
                                    });
                            });
                        });
                });
            if open_project_requested {
                self.picker_from_welcome = true;
                self.picker = Some(FileRequest::OpenProject);
                self.welcome = false;
            }
            if let Some((width, height)) = start_preset {
                self.new_unit = crate::document_setup::DocumentUnit::Pixels;
                self.new_width = f64::from(width);
                self.new_height = f64::from(height);
                self.new_dpi = 300;
                self.new_custom_size_selected = false;
                self.picker_from_welcome = false;
                if !self.test_path_dialog {
                    self.new_name = "Untitled".into();
                }
                self.new_from_welcome = false;
                self.new_dialog = false;
                self.welcome = false;
                actions.push(Action::New);
                if self.test_path_dialog {
                    crate::studio_log!("UI_TEST_HOME_PRESET_CREATE_REQUESTED: {width} {height}");
                }
            }
            if custom_size_requested {
                self.picker_from_welcome = false;
                self.new_from_welcome = true;
                self.new_dialog = true;
                self.welcome = false;
            }
            if let Some(path) = open_recent {
                self.picker_from_welcome = true;
                actions.push(Action::Recent(path));
                self.welcome = false;
            }
            if let Some(path) = remove_draft {
                self.draft_cleanup = Some(path);
            }
        } else if self.recovery_manager && self.cleanup.is_none() {
            let viewport = ctx.content_rect();
            let limits = crate::studio_layout::recovery_modal_limits(viewport);
            let mut modal_content_rect = egui::Rect::NOTHING;
            egui::Modal::new(egui::Id::new("recovery manager")).show(ctx, |ui| {
                ui.set_max_size(limits.max_size);
                ui.set_width_range(limits.min_width..=limits.max_size.x);
                ui.heading("Artwork recovery manager");
                ui.label("These local checkpoints are separate from saved projects. Recover opens a copy as a new unsaved document.");
                if self.recovery_files.is_empty() {
                    ui.label("No inactive recovery checkpoints are available.");
                } else {
                    egui::ScrollArea::vertical().max_height(limits.list_height).show(ui, |ui| {
                        for (index, path) in self.recovery_files.clone().into_iter().enumerate() {
                            let name = recovery_checkpoint_label(&path, index);
                            let recovery_surface = ui.visuals().faint_bg_color;
                            let recovery_label_color = crate::appearance::readable_text_color(
                                ui.visuals().text_color(),
                                recovery_surface,
                            );
                            if self.test_path_dialog && !self.test_recovery_label_logged {
                                self.test_recovery_label_logged = true;
                                crate::studio_log!("UI_TEST_RECOVERY_LABEL: {name}");
                                if index == 0 {
                                let contrast = crate::appearance::contrast_ratio(
                                    recovery_label_color,
                                    recovery_surface,
                                );
                                crate::studio_log!(
                                    "UI_TEST_RECOVERY_LABEL_CONTRAST: {} {:.2}",
                                    if ui.visuals().dark_mode { "dark" } else { "light" },
                                    contrast,
                                );
                                }
                            }
                            let snapshot_metadata = std::fs::metadata(&path)
                                .ok()
                                .filter(|metadata| metadata.is_file());
                            let journal_path = path.with_extension("rpl.journal");
                            let journal_metadata = std::fs::metadata(&journal_path)
                                .ok()
                                .filter(|metadata| metadata.is_file());
                            let backup_path = path.with_extension("rpl.bak");
                            let backup_metadata = std::fs::metadata(&backup_path)
                                .ok()
                                .filter(|metadata| metadata.is_file());
                            let metadata = snapshot_metadata
                                .as_ref()
                                .or(journal_metadata.as_ref())
                                .or(backup_metadata.as_ref());
                            let can_recover = metadata.is_some();
                            let source_label = recovery_source_label(
                                snapshot_metadata.is_some(),
                                journal_metadata.is_some(),
                                backup_metadata.is_some(),
                            );
                            let bytes = metadata.map(std::fs::Metadata::len);
                            let updated = metadata
                                .and_then(|metadata| metadata.modified().ok())
                                .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|duration| recovery_updated_label(duration.as_secs()))
                                .unwrap_or_else(|| "Checkpoint time unavailable".into());
                            let dimensions = storage::peek_dimensions(&path)
                                .or_else(|| {
                                    backup_metadata
                                        .as_ref()
                                        .and_then(|_| storage::peek_dimensions(&backup_path))
                                })
                                .map(|(width, height, dpi)| format!("{width} × {height} px · {dpi} DPI"))
                                .unwrap_or_else(|| {
                                    if journal_metadata.is_some() {
                                        "Dimensions will be checked during recovery".into()
                                    } else {
                                        "Project dimensions unavailable".into()
                                    }
                                });
                            let preview = self.recovery_preview.as_ref()
                                .filter(|(preview_path, _)| preview_path == &path)
                                .and_then(|(_, texture)| texture.clone());
                            let preview_attempted = self.recovery_preview.as_ref()
                                .is_some_and(|(preview_path, _)| preview_path == &path);
                            egui::Frame::new().fill(ui.visuals().faint_bg_color)
                                .corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if let Some(texture) = preview {
                                            let response = ui.add(
                                                egui::Image::new(&texture)
                                                    .max_size(egui::vec2(120.0, 80.0))
                                                    .alt_text("Artwork recovery preview"),
                                            );
                                            if self.test_path_dialog
                                                && !self.test_recovery_preview_logged
                                            {
                                                self.test_recovery_preview_logged = true;
                                                crate::studio_log!("UI_TEST_RECOVERY_PREVIEW_VISIBLE");
                                                crate::studio_log!(
                                                    "UI_TEST_RECOVERY_PREVIEW_IMAGE_RECT: {:.1} {:.1} {:.1} {:.1} {:.2} {:.1}",
                                                    response.rect.left(),
                                                    response.rect.top(),
                                                    response.rect.width(),
                                                    response.rect.height(),
                                                    ctx.pixels_per_point(),
                                                    ctx.content_rect().min.y,
                                                );
                                            }
                                        } else if preview_attempted {
                                            ui.label("Preview unavailable");
                                        } else {
                                            let response = ui.button("Preview");
                                            if self.test_path_dialog && index == 0 {
                                                crate::studio_log!(
                                                    "UI_TEST_RECOVERY_PREVIEW_TARGET: {:.1} {:.1} {:.1} {:.1} {:.2} {:.1}",
                                                    response.rect.left(),
                                                    response.rect.top(),
                                                    response.rect.width(),
                                                    response.rect.height(),
                                                    ctx.pixels_per_point(),
                                                    ctx.content_rect().min.y,
                                                );
                                            }
                                            if self.test_path_dialog && index == 0 {
                                                if response.hovered()
                                                    && !self.test_recovery_preview_hover_logged
                                                {
                                                    self.test_recovery_preview_hover_logged = true;
                                                    crate::studio_log!("UI_TEST_RECOVERY_PREVIEW_HOVERED");
                                                } else if !response.hovered() {
                                                    self.test_recovery_preview_hover_logged = false;
                                                }
                                            }
                                            if response.clicked() {
                                                let texture =
                                                    crate::recovery_previews::load(&path, ctx);
                                                self.recovery_preview = Some((path.clone(), texture));
                                            }
                                        }
                                        ui.vertical(|ui| {
                                            ui.label(crate::ui_tokens::type_text(
                                                name,
                                                crate::ui_tokens::TypeRole::CardTitle,
                                                recovery_label_color,
                                            ));
                                            let metadata = ui.visuals().weak_text_color();
                                            ui.label(crate::ui_tokens::type_text(
                                                source_label,
                                                crate::ui_tokens::TypeRole::Metadata,
                                                metadata,
                                            ));
                                            ui.label(crate::ui_tokens::type_text(
                                                dimensions,
                                                crate::ui_tokens::TypeRole::Metadata,
                                                metadata,
                                            ));
                                            ui.label(crate::ui_tokens::type_text(
                                                updated,
                                                crate::ui_tokens::TypeRole::Metadata,
                                                metadata,
                                            ));
                                            ui.label(crate::ui_tokens::type_text(
                                                bytes.map(|size| format!("{} KiB", size.div_ceil(1024)))
                                                    .unwrap_or_else(|| "Recovery data unavailable".into()),
                                                crate::ui_tokens::TypeRole::Metadata,
                                                metadata,
                                            ));
                                        });
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button("Remove…").clicked() {
                                                self.cleanup = Some(path.clone());
                                            }
                                            if ui.add_enabled(can_recover, egui::Button::new("Recover")).clicked() {
                                                actions.push(Action::Recover(path.clone()));
                                                self.recovery_manager = false;
                                                self.welcome = false;
                                            }
                                        });
                                    });
                                });
                            ui.add_space(6.0);
                        }
                    });
                }
                ui.separator();
                if ui.button("Done").clicked() {
                    self.recovery_manager = false;
                    if self.recovery_manager_from_home {
                        self.welcome = true;
                    }
                    self.recovery_manager_from_home = false;
                }
                modal_content_rect = ui.min_rect();
            });
            if self.test_path_dialog {
                let rect = modal_content_rect;
                crate::studio_log!(
                    "UI_TEST_RECOVERY_MODAL_BOUNDS: {:.1} {:.1} {:.1} {:.1} {:.1} {:.1}",
                    rect.left(),
                    rect.top(),
                    rect.right(),
                    rect.bottom(),
                    viewport.width(),
                    viewport.height(),
                );
            }
        } else if let Some(path) = self.cleanup.clone() {
            egui::Modal::new(egui::Id::new("delete recovery checkpoint")).show(ctx, |ui| {
                ui.heading("Delete recovery checkpoint?");
                ui.label(path.to_string_lossy());
                ui.label("Deletes this checkpoint and its backup. First verify an independent saved project. Active sessions and ordinary artwork files are protected.");
                if ui.button("Delete this recovery and backup").clicked() {
                    actions.push(Action::RemoveRecovery(path));
                    self.cleanup = None;
                }
                if ui.button("Cancel").clicked() {
                    self.cleanup = None;
                }
            });
        } else if let Some(path) = self.draft_cleanup.clone() {
            egui::Modal::new(egui::Id::new("delete persistent draft")).show(ctx, |ui| {
                ui.heading("Remove this saved draft?");
                ui.label(crate::drafts::display_name(&path));
                ui.label("This permanently removes the selected draft and its backup. It does not affect other projects or the active document.");
                if ui.button("Remove draft and backup").clicked() {
                    actions.push(Action::RemoveDraft(path));
                    self.draft_cleanup = None;
                }
                if ui.button("Cancel").clicked() {
                    self.draft_cleanup = None;
                }
            });
        } else if self.new_dialog {
            if self.test_path_dialog && !self.test_new_artwork_logged {
                self.test_new_artwork_logged = true;
                crate::studio_log!("UI_TEST_NEW_ARTWORK_VISIBLE");
            }
            egui::Modal::new(egui::Id::new("new document")).show(ctx, |ui| {
                let dialog_width = (ctx.content_rect().width() - 32.0).clamp(280.0, 880.0);
                ui.set_min_width(dialog_width.min(440.0));
                ui.set_max_width(dialog_width);
                let dialog_view_height = ctx.content_rect().height();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .min_scrolled_height((dialog_view_height - 48.0).clamp(180.0, 720.0))
                    .max_height((dialog_view_height - 24.0).max(120.0))
                    .show(ui, |ui| {
                ui.heading("Create a new canvas");
                ui.horizontal(|ui| {
                    ui.label("Document name");
                    let field_width = (ui.available_width() - 120.0).max(120.0);
                    let response = ui.add_sized(
                        [field_width, 38.0],
                        egui::TextEdit::singleline(&mut self.new_name)
                            .hint_text("Untitled"),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::TextEdit,
                            true,
                            "Document name",
                        )
                    });
                    if self.test_path_dialog {
                        crate::studio_log!(
                            "UI_TEST_NEW_DOCUMENT_NAME_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                            response.rect.left(),
                            response.rect.top(),
                            response.rect.width(),
                            response.rect.height(),
                            ctx.pixels_per_point(),
                        );
                    }
                });
                ui.label("Choose a paper format, then fine-tune dimensions and output settings.");
                ui.add_space(6.0);
                let (_, card_width) = document_format_grid(ui.available_width());
                let formats = [
                    ("Portrait", 1800, 2400, 300),
                    ("Landscape", 2400, 1800, 300),
                    ("Square", 2048, 2048, 300),
                    ("A4 Print", 2480, 3508, 300),
                    ("Comic Page", 2550, 3300, 300),
                    ("Full HD", 1920, 1080, 96),
                ];
                let current_width_px = self.new_unit.to_pixels(self.new_width, self.new_dpi);
                let current_height_px = self.new_unit.to_pixels(self.new_height, self.new_dpi);
                let current_matches_preset = formats.iter().any(|(_, width, height, dpi)| {
                    current_width_px == Some(*width)
                        && current_height_px == Some(*height)
                        && self.new_dpi == *dpi
                });
                ui.horizontal_wrapped(|ui| {
                    for (label, width, height, dpi) in formats {
                        let selected = current_width_px == Some(width)
                            && current_height_px == Some(height)
                            && self.new_dpi == dpi
                            && !self.new_custom_size_selected;
                        let (activated, _paper, card_rect) = document_format_card_with_preview(
                            ui,
                            label,
                            width as f32,
                            height as f32,
                            dpi,
                            selected,
                            card_width,
                        );
                        if self.test_path_dialog {
                            log_format_preview_rect(ui, label, _paper);
                            log_format_card_rect(ui, label, card_rect);
                        }
                        if activated {
                            self.new_unit = crate::document_setup::DocumentUnit::Pixels;
                            self.new_width = f64::from(width);
                            self.new_height = f64::from(height);
                            self.new_dpi = dpi;
                            self.new_custom_size_selected = false;
                        }
                    }
                    let custom_width = current_width_px.unwrap_or(1080);
                    let custom_height = current_height_px.unwrap_or(1080);
                    let (activated, _paper, card_rect) = document_format_card_with_preview(
                        ui,
                        "Custom",
                        custom_width as f32,
                        custom_height as f32,
                        self.new_dpi,
                        self.new_custom_size_selected || !current_matches_preset,
                        card_width,
                    );
                    if self.test_path_dialog {
                        log_format_preview_rect(ui, "Custom", _paper);
                        log_format_card_rect(ui, "Custom", card_rect);
                    }
                    if activated {
                        self.new_custom_size_selected = true;
                    }
                });
                let (preview_document_width, preview_document_height) = match (
                    current_width_px,
                    current_height_px,
                ) {
                    (Some(width), Some(height)) => (width as f32, height as f32),
                    _ => (1.0, 1.0),
                };
                let preview_slot_size = egui::vec2(208.0, 138.0);
                let preview_size = fit_paper_to_box(
                    preview_slot_size - egui::vec2(24.0, 24.0),
                    preview_document_width,
                    preview_document_height,
                );
                ui.horizontal_wrapped(|ui| {
                    let (slot, _) = ui.allocate_exact_size(preview_slot_size, egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(slot, 8.0, ui.visuals().faint_bg_color);
                    let preview = egui::Rect::from_center_size(slot.center(), preview_size);
                    if self.new_transparent {
                        let cell = 10.0;
                        let colors = [ui.visuals().faint_bg_color, ui.visuals().extreme_bg_color];
                        for y in 0..((preview.height() / cell).ceil() as i32) {
                            for x in 0..((preview.width() / cell).ceil() as i32) {
                                let rect = egui::Rect::from_min_size(
                                    preview.min + egui::vec2(x as f32 * cell, y as f32 * cell),
                                    egui::vec2(cell, cell),
                                ).intersect(preview);
                                painter.rect_filled(rect, 0.0, colors[((x + y) & 1) as usize]);
                            }
                        }
                    } else {
                        painter.rect_filled(
                            preview,
                            4.0,
                            egui::Color32::from_rgb(
                                self.new_background[0],
                                self.new_background[1],
                                self.new_background[2],
                            ),
                        );
                    }
                    painter.rect_stroke(preview, 4.0, ui.visuals().window_stroke, egui::StrokeKind::Inside);
                    painter.rect_stroke(slot, 8.0, ui.visuals().window_stroke, egui::StrokeKind::Inside);
                    ui.vertical(|ui| {
                        let preview_heading = crate::appearance::readable_text_color(
                            ui.visuals().text_color(),
                            ui.visuals().window_fill,
                        );
                        ui.label(crate::ui_tokens::display_text(
                            "Canvas preview",
                            17.0,
                            preview_heading,
                        ));
                        if let (Some(width), Some(height)) = (
                            self.new_unit.to_pixels(self.new_width, self.new_dpi),
                            self.new_unit.to_pixels(self.new_height, self.new_dpi),
                        ) {
                            ui.label(egui::RichText::new(format!("{width} × {height} px")).strong());
                        }
                        ui.add_space(4.0);
                        ui.label(if self.new_transparent {
                            "Transparent background"
                        } else {
                            "Solid color background"
                        });
                        ui.weak(format!("{} DPI · sRGB RGBA8", self.new_dpi));
                    });
                });
                egui::CollapsingHeader::new("Advanced dimensions, presets and resolution")
                    .default_open(false)
                    .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Preset");
                    egui::ComboBox::from_id_salt("new-document-preset")
                        .selected_text("Choose preset…")
                        .show_ui(ui, |ui| {
                            for preset in crate::document_setup::PRESETS {
                                if ui.selectable_label(false, preset.name).clicked() {
                                    self.new_unit = crate::document_setup::DocumentUnit::Pixels;
                                    self.new_width = f64::from(preset.width);
                                    self.new_height = f64::from(preset.height);
                                    self.new_dpi = preset.dpi;
                                    self.new_custom_size_selected = false;
                                }
                            }
                        });
                });
                if !self.recent_document_sizes.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label("Recent size");
                        egui::ComboBox::from_id_salt("recent-document-size")
                            .selected_text("Choose recent dimensions…")
                            .show_ui(ui, |ui| {
                                for size in self.recent_document_sizes.iter().copied() {
                                    let label = format!(
                                        "{} × {} px · {} DPI",
                                        size.width_px, size.height_px, size.dpi
                                    );
                                    if ui.selectable_label(false, label).clicked() {
                                        self.new_unit = crate::document_setup::DocumentUnit::Pixels;
                                        self.new_width = f64::from(size.width_px);
                                        self.new_height = f64::from(size.height_px);
                                        self.new_dpi = size.dpi;
                                        self.new_custom_size_selected = false;
                                    }
                                }
                            });
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label("Units");
                    egui::ComboBox::from_id_salt("new-document-units")
                        .selected_text(self.new_unit.label())
                        .show_ui(ui, |ui| {
                            for unit in crate::document_setup::DocumentUnit::ALL {
                                if ui.selectable_label(self.new_unit == unit, unit.label()).clicked() {
                                    let width_px = self.new_unit.to_pixels(self.new_width, self.new_dpi);
                                    let height_px = self.new_unit.to_pixels(self.new_height, self.new_dpi);
                                    self.new_unit = unit;
                                    if let Some(width_px) = width_px {
                                        self.new_width = unit.from_pixels(width_px, self.new_dpi).unwrap_or(self.new_width);
                                    }
                                    if let Some(height_px) = height_px {
                                        self.new_height = unit.from_pixels(height_px, self.new_dpi).unwrap_or(self.new_height);
                                    }
                                }
                            }
                        });
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Width");
                    let width_changed = ui.add(egui::DragValue::new(&mut self.new_width).range(1.0..=100000.0).speed(1.0)).changed();
                    ui.label("Height");
                    let height_changed = ui.add(egui::DragValue::new(&mut self.new_height).range(1.0..=100000.0).speed(1.0)).changed();
                    let swapped = ui.button("Swap").clicked();
                    if swapped { std::mem::swap(&mut self.new_width, &mut self.new_height); }
                    self.new_custom_size_selected |= width_changed || height_changed || swapped;
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Resolution");
                    let dpi_changed = ui.add(egui::DragValue::new(&mut self.new_dpi).range(1..=9600).suffix(" DPI")).changed();
                    self.new_custom_size_selected |= dpi_changed;
                    ui.color_edit_button_srgb(&mut self.new_background);
                    ui.checkbox(&mut self.new_transparent, "Transparent");
                });
                });
                let dimensions = (
                    self.new_unit.to_pixels(self.new_width, self.new_dpi),
                    self.new_unit.to_pixels(self.new_height, self.new_dpi),
                );
                if let (Some(width), Some(height)) = dimensions {
                    ui.label(format!("{} × {} pixels · sparse tiles (up to 128 MiB artwork tile data)", width, height));
                } else {
                    ui.colored_label(egui::Color32::LIGHT_RED, "These settings exceed the current 4096 × 4096 project limit.");
                }
                ui.small("DPI affects print-size conversion; changing it does not add pixel detail. Raster color is sRGB RGBA8.");
                ui.horizontal(|ui| {
                let can_create = dimensions.0.is_some() && dimensions.1.is_some();
                let create_fill = ui.visuals().selection.bg_fill;
                let create_text = crate::appearance::readable_text_color(
                    ui.visuals().text_color(),
                    create_fill,
                );
                let create_button = egui::Button::new(
                    egui::RichText::new("Create document")
                        .strong()
                        .color(create_text),
                )
                .fill(create_fill);
                let create_response = ui.add_enabled(can_create, create_button);
                if self.test_path_dialog {
                    crate::studio_log!(
                        "UI_TEST_NEW_DOCUMENT_CREATE_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                        create_response.rect.left(),
                        create_response.rect.top(),
                        create_response.rect.width(),
                        create_response.rect.height(),
                        ctx.pixels_per_point(),
                    );
                }
                if create_response.clicked()
                    || (can_create && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    actions.push(Action::New);
                    self.new_dialog = false;
                    self.welcome = false;
                }
                if ui.button("Cancel").clicked() {
                    self.new_dialog = false;
                    if self.new_from_welcome {
                        self.welcome = true;
                    }
                    self.new_from_welcome = false;
                }
                });
                    });
            });
        } else if self.manual_file_dialog {
            egui::Modal::new(egui::Id::new("test project file path")).show(ctx, |ui| {
                ui.heading("Test workflow file path");
                // Plain Enter works in the new-document form but can be consumed
                // by this focused text field. Read it before the TextEdit handles
                // the event so the headless workflow can reliably submit the path.
                let submit = ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add(egui::TextEdit::singleline(&mut self.path).char_limit(4096));
                ui.horizontal(|ui| {
                    if ui.button("Continue").clicked() || submit {
                        if let Some(action) = self.manual_file_action.take() {
                            actions.push(action);
                        }
                        self.manual_file_dialog = false;
                        self.manual_file_submit = false;
                    }
                    let cancel = ui.button("Cancel");
                    if self.test_path_dialog {
                        let rect = cancel.rect;
                        crate::studio_log!(
                            "UI_TEST_FILE_PICKER_CANCEL_RECT: {:.1} {:.1} {:.1} {:.1} {:.2}",
                            rect.left(),
                            rect.top(),
                            rect.width(),
                            rect.height(),
                            ui.ctx().pixels_per_point(),
                        );
                    }
                    if cancel.clicked() {
                        if self.test_path_dialog {
                            crate::studio_log!("UI_TEST_FILE_PICKER_CANCELLED");
                        }
                        self.manual_file_action = None;
                        self.manual_file_dialog = false;
                        self.manual_file_submit = false;
                    }
                });
            });
        } else if self.confirm.is_some() {
            egui::Modal::new(egui::Id::new("preserve unsaved artwork")).show(ctx, |ui| {
                ui.heading("Preserve unsaved artwork");
                ui.label(
                    "Save first, or retain a recovery checkpoint before replacing this document.",
                );
                if ui.button("Keep recovery and continue").clicked() {
                    if self.preserve(session).is_ok() {
                        *approved = true;
                        if let Some(action) = self.confirm.take() {
                            actions.push(action);
                        }
                    } else {
                        self.status =
                            "Recovery unavailable: save current artwork before continuing".into();
                    }
                }
                if ui.button("Cancel").clicked() {
                    self.confirm = None;
                }
            });
        } else if self.close_requested {
            egui::Modal::new(egui::Id::new("close artwork")).show(ctx, |ui| {
                ui.heading("Automatic save did not complete");
                ui.label("Your artwork is still open. Retry the save, choose another project location, or cancel closing.");
                if ui.button("Retry save").clicked() {
                    actions.push(Action::RetryClose);
                }
                if ui.button("Save As…").clicked() {
                    self.picker = Some(FileRequest::SaveProjectAs);
                }
                if ui.button("Cancel").clicked() {
                    self.close_requested = false;
                }
            });
        }
    }
    pub fn frame(
        &mut self,
        window: &Window,
        session: &mut CanvasSession,
        size: [u32; 2],
    ) -> egui::FullOutput {
        if session.mode() != crate::interaction::Mode::Live {
            self.touches.clear();
            self.pan_cursor = None;
            self.edit_pointer = None;
            self.edit_points.clear();
        }
        if let Some(theme) = window.theme() {
            self.system_dark = theme == winit::window::Theme::Dark;
        }
        let appearance_key = (
            self.appearance_mode,
            self.accent_scheme,
            self.custom_accent_rgb,
            self.system_dark,
            self.high_contrast,
            self.reduced_motion,
            (self.ui_scale * 1000.0).round() as u32,
        );
        if self.appearance_cache != Some(appearance_key) {
            let mut visuals = crate::appearance::visuals(
                self.appearance_mode,
                self.system_dark,
                self.accent_scheme,
                self.custom_accent_rgb,
            );
            if self.high_contrast {
                visuals = crate::appearance::high_contrast(
                    visuals,
                    self.appearance_mode.is_dark(self.system_dark),
                );
            }
            self.context.set_visuals(visuals);
            self.context.set_zoom_factor(self.ui_scale);
            crate::ui_tokens::install_motion_preference(&self.context, self.reduced_motion);
            self.appearance_cache = Some(appearance_key);
        }
        if self.test_path_dialog && !self.test_strong_text_contrast_logged {
            self.test_strong_text_contrast_logged = true;
            let visuals = self.context.style().visuals.clone();
            crate::studio_log!(
                "UI_TEST_STRONG_TEXT_CONTRAST: {} {:.2}",
                if visuals.dark_mode { "dark" } else { "light" },
                crate::appearance::contrast_ratio(visuals.strong_text_color(), visuals.panel_fill,)
            );
        }
        let raw = self.input.take_egui_input(window);
        let ctx = self.context.clone();
        self.palette_rect = None;
        let mut actions = Vec::new();
        if self.test_path_dialog && !self.new_dialog && self.test_reference_centered {
            self.test_reference_centered = false;
            // This is an integration-test-only injection point. It lets the
            // real import and placement path be exercised without opening a
            // native file picker in the headless Linux workflow.
            if let Some(path) = self.test_reference_import.take() {
                actions.push(Action::AddReference(path));
            }
        }
        if let Some((id, target_index)) = self.pending_layer_move.take() {
            actions.push(Action::MoveLayerTo(id, target_index));
        }
        if self.test_path_dialog && self.manual_file_dialog && self.manual_file_submit {
            self.manual_file_submit = false;
            self.manual_file_dialog = false;
            if let Some(action) = self.manual_file_action.take() {
                crate::studio_log!("TEST_PATH_DIALOG: queued action for deterministic submission");
                actions.push(action);
            }
        }
        let mut central = egui::Rect::NOTHING;
        let mut approved = false;
        let mut output = ctx.run(raw, |ctx| {
            self.widgets(ctx, session, &mut actions, &mut central, &mut approved)
        });
        // Tool controls are edited inside the UI frame. Capture their latest
        // mode parameters before subsequent native pointer events arrive.
        session
            .live_mut()
            .set_stroke_mode(stroke_mode_for_tool(self.tool, self.color_mix_strength));

        if let Some(request) = self.picker.take() {
            if self.test_path_dialog && !matches!(request, FileRequest::AddReferences) {
                self.manual_file_action = request.action();
                self.manual_file_dialog = true;
                self.context.request_repaint();
            } else {
                let mut dialog = rfd::FileDialog::new();
                let (title, filter, extensions, saving) = match request {
                    FileRequest::OpenProject => {
                        ("Open project", "BitchPixel project", vec!["rpl"], false)
                    }
                    FileRequest::SaveProjectAs => {
                        ("Save project as", "BitchPixel project", vec!["rpl"], true)
                    }
                    FileRequest::SaveBrush => (
                        "Save brush preset",
                        "BitchPixel brush",
                        vec!["rplbrush"],
                        true,
                    ),
                    FileRequest::LoadBrush => (
                        "Load brush preset",
                        "BitchPixel brush",
                        vec!["rplbrush"],
                        false,
                    ),
                    FileRequest::ExportPng => ("Export PNG", "PNG image", vec!["png"], true),
                    FileRequest::ImportPng => ("Import image", "Raster image", vec!["png"], false),
                    FileRequest::ImportOra => {
                        ("Open OpenRaster", "OpenRaster image", vec!["ora"], false)
                    }
                    FileRequest::ExportOra => {
                        ("Export OpenRaster", "OpenRaster image", vec!["ora"], true)
                    }
                    FileRequest::AddReferences => (
                        "Add reference images",
                        "Reference images",
                        vec!["png", "jpg", "jpeg"],
                        false,
                    ),
                };
                dialog = dialog.set_title(title).add_filter(filter, &extensions);
                if let Some(parent) = PathBuf::from(&self.path)
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                {
                    dialog = dialog.set_directory(parent);
                }
                if saving && let Some(name) = PathBuf::from(&self.path).file_name() {
                    dialog = dialog.set_file_name(name.to_string_lossy());
                }
                let selected = if matches!(request, FileRequest::AddReferences) {
                    dialog.pick_files().unwrap_or_default()
                } else if saving {
                    dialog.save_file().into_iter().collect()
                } else {
                    dialog.pick_file().into_iter().collect()
                };
                if selected.is_empty() {
                    self.status = "File selection canceled".into();
                    if self.picker_from_welcome {
                        self.welcome = true;
                    }
                    self.picker_from_welcome = false;
                } else if matches!(request, FileRequest::AddReferences) {
                    actions.extend(selected.into_iter().map(Action::AddReference));
                    self.picker_from_welcome = false;
                } else if let Some(path) = selected.into_iter().next() {
                    self.path = path.to_string_lossy().into_owned();
                    if let Some(action) = request.action() {
                        actions.push(action);
                    }
                }
            }
        }

        let mut reference_imported = 0;
        let mut reference_errors = Vec::new();
        for a in actions {
            if matches!(a, Action::RetryClose) {
                if let Err(error) = self.save_for_close(session) {
                    self.status = format!("Automatic save failed: {error}");
                    self.close_requested = true;
                }
                window.request_redraw();
                continue;
            }
            if let Action::AddReference(path) = a {
                let viewport = ctx.content_rect();
                match add_reference_path(&ctx, &mut self.references, path) {
                    Ok(()) => {
                        let index = self.references.len() - 1;
                        let size = default_reference_size(viewport);
                        let occupied: Vec<_> = self
                            .references
                            .iter()
                            .take(index)
                            .map(|existing| {
                                egui::Rect::from_min_size(
                                    egui::pos2(
                                        existing.layout.position[0],
                                        existing.layout.position[1],
                                    ),
                                    egui::vec2(existing.layout.size[0], existing.layout.size[1]),
                                )
                            })
                            .collect();
                        let position = default_reference_position(
                            viewport,
                            size,
                            self.palette_rect,
                            &occupied,
                        );
                        let reference = &mut self.references[index];
                        reference.layout.position = [position.x, position.y];
                        reference.layout.size = size;
                        if self.test_path_dialog {
                            crate::studio_log!(
                                "UI_TEST_REFERENCE_INITIAL_POSITION: {:.1} {:.1} {:.1} {:.1}",
                                position.x,
                                position.y,
                                size[0],
                                size[1]
                            );
                        }
                        reference_imported += 1;
                    }
                    Err(error) => reference_errors.push(error),
                }
                continue;
            }
            if matches!(
                a,
                Action::New
                    | Action::Open
                    | Action::Recover(_)
                    | Action::Recent(_)
                    | Action::ImportOra
            ) && session.live().project().dirty()
                && self.confirm.is_none()
                && !approved
            {
                self.confirm = Some(a);
                continue;
            }
            let opening_from_welcome =
                self.picker_from_welcome && matches!(&a, Action::Open | Action::Recent(_));
            session.mouse(MouseEvent::Interrupted, 0);
            let result = self.execute(a, session);
            if let Err(error) = result {
                self.status = error.to_string();
                if opening_from_welcome {
                    self.welcome = true;
                }
            } else if opening_from_welcome {
                self.picker_from_welcome = false;
            }
            window.request_redraw();
        }
        let brush_color = session.live().project().brush.color;
        if let Some(previous) = self.observed_brush_color
            && previous != brush_color
        {
            if self.test_path_dialog {
                crate::studio_log!(
                    "UI_TEST_COLOR_CURRENT_RGB: {} {} {}",
                    brush_color[0],
                    brush_color[1],
                    brush_color[2]
                );
            }
            crate::workspace::remember_brush_color(&mut self.recent_brush_colors, brush_color);
            if let Err(error) = self.persist_workspace() {
                self.status = format!("Could not save recent brush colors: {error}");
            }
        }
        self.observed_brush_color = Some(brush_color);
        if reference_imported > 0 || !reference_errors.is_empty() {
            let mut status = if reference_imported == 0 {
                "No reference images were added".to_string()
            } else {
                if let Err(error) = self.persist_workspace() {
                    reference_errors.push(format!("layout persistence failed: {error}"));
                }
                format!(
                    "Added {reference_imported} reference image(s); large previews are scaled for the GPU budget"
                )
            };
            if !reference_errors.is_empty() {
                status.push_str(" · ");
                status.push_str(&reference_errors.join(" · "));
            }
            self.status = status;
            // The picker returns after egui has produced this frame's texture
            // delta. Request another frame so newly loaded reference textures
            // are uploaded and the floating image windows become visible.
            window.request_redraw();
        }
        let sf = f64::from(output.pixels_per_point);
        let rect = [
            f64::from(central.min.x) * sf,
            f64::from(central.min.y) * sf,
            (f64::from(central.max.x) * sf).min(size[0] as f64),
            (f64::from(central.max.y) * sf).min(size[1] as f64),
        ];
        let state = session.live().project().state();
        self.mapping =
            CanvasMapping::for_document(size, [state.width as f64, state.height as f64], rect)
                .and_then(|m| m.navigated(self.navigation.zoom, self.navigation.pan))
                .ok();
        self.workspace_center = [(rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0];
        if let Some(m) = self.mapping {
            session.live_mut().set_mapping(m);
        }
        if session.mode() == crate::interaction::Mode::Live
            && let Some(mapping) = self.mapping
        {
            let shape_tool = matches!(
                self.tool,
                EditTool::ShapeLine | EditTool::ShapeRectangle | EditTool::ShapeEllipse
            );
            if self.edit_pointer.is_some() && shape_tool && self.edit_points.len() == 2 {
                let from = self.edit_points[0];
                let to = self.edit_points[1];
                let shape_kind = match self.tool {
                    EditTool::ShapeLine => ShapeKind::Line,
                    EditTool::ShapeRectangle => ShapeKind::Rectangle,
                    EditTool::ShapeEllipse => ShapeKind::Ellipse,
                    _ => unreachable!("shape tool checked"),
                };
                let [left, top, right, bottom] = [
                    from[0].min(to[0]),
                    from[1].min(to[1]),
                    from[0].max(to[0]),
                    from[1].max(to[1]),
                ];
                let document_points: Vec<_> = match shape_kind {
                    ShapeKind::Line => vec![from, to],
                    ShapeKind::Rectangle => {
                        vec![[left, top], [right, top], [right, bottom], [left, bottom]]
                    }
                    ShapeKind::Ellipse => (0..64)
                        .map(|index| {
                            let angle = index as f64 * std::f64::consts::TAU / 64.0;
                            [
                                (left + right) / 2.0 + angle.cos() * (right - left) / 2.0,
                                (top + bottom) / 2.0 + angle.sin() * (bottom - top) / 2.0,
                            ]
                        })
                        .collect(),
                };
                let points: Vec<_> = document_points
                    .into_iter()
                    .filter_map(|point| mapping.to_clip(point).ok())
                    .map(|point| {
                        egui::pos2(
                            (point[0] + 1.0) * size[0] as f32 / 2.0 / output.pixels_per_point,
                            (1.0 - point[1]) * size[1] as f32 / 2.0 / output.pixels_per_point,
                        )
                    })
                    .collect();
                if self.shape_filled && shape_kind != ShapeKind::Line {
                    output.shapes.push(egui::epaint::ClippedShape {
                        clip_rect: central,
                        shape: egui::Shape::convex_polygon(
                            points.clone(),
                            egui::Color32::from_rgba_unmultiplied(168, 93, 255, 72),
                            egui::Stroke::NONE,
                        ),
                    });
                }
                if points.len() >= 2 {
                    let outline = |color, width| {
                        if shape_kind == ShapeKind::Line {
                            egui::Shape::line(points.clone(), egui::Stroke::new(width, color))
                        } else {
                            egui::Shape::closed_line(
                                points.clone(),
                                egui::Stroke::new(width, color),
                            )
                        }
                    };
                    for (width, color) in [(4.0, egui::Color32::BLACK), (1.5, egui::Color32::WHITE)]
                    {
                        output.shapes.push(egui::epaint::ClippedShape {
                            clip_rect: central,
                            shape: outline(color, width),
                        });
                    }
                    if self.test_path_dialog {
                        let bounds =
                            points
                                .iter()
                                .fold(egui::Rect::NOTHING, |mut bounds, point| {
                                    bounds.extend_with(*point);
                                    bounds
                                });
                        crate::studio_log!(
                            "UI_TEST_SHAPE_PREVIEW_BOUNDS: {:.2} {:.2} {:.2} {:.2} {:.2}",
                            bounds.left(),
                            bounds.top(),
                            bounds.right(),
                            bounds.bottom(),
                            output.pixels_per_point,
                        );
                    }
                }
            } else {
                let selection = if self.edit_pointer.is_some() {
                    let kind = match self.tool {
                        EditTool::Ellipse => SelectionKind::Ellipse,
                        EditTool::Lasso => SelectionKind::Lasso,
                        _ => SelectionKind::Rectangle,
                    };
                    Selection::new(kind, self.edit_points.clone()).ok()
                } else {
                    session.live().project().state().selection.clone()
                };
                if let Some(selection) = selection {
                    let to_screen = |p: [f64; 2]| {
                        mapping.to_clip(p).ok().map(|p| {
                            egui::pos2(
                                (p[0] + 1.0) * size[0] as f32 / 2.0 / output.pixels_per_point,
                                (1.0 - p[1]) * size[1] as f32 / 2.0 / output.pixels_per_point,
                            )
                        })
                    };
                    let [x, y, r, b] = selection.bounds();
                    let points: Vec<_> = match selection.kind() {
                        SelectionKind::Rectangle => vec![[x, y], [r, y], [r, b], [x, b]],
                        SelectionKind::Ellipse => (0..64)
                            .map(|i| {
                                let a = i as f64 * std::f64::consts::TAU / 64.0;
                                [
                                    (x + r) / 2.0 + a.cos() * (r - x) / 2.0,
                                    (y + b) / 2.0 + a.sin() * (b - y) / 2.0,
                                ]
                            })
                            .collect(),
                        SelectionKind::Lasso => selection.points().to_vec(),
                    };
                    let points: Vec<_> = points.into_iter().filter_map(to_screen).collect();
                    for (width, color) in [(3.0, egui::Color32::BLACK), (1.0, egui::Color32::WHITE)]
                    {
                        output.shapes.push(egui::epaint::ClippedShape {
                            clip_rect: central,
                            shape: egui::Shape::closed_line(
                                points.clone(),
                                egui::Stroke::new(width, color),
                            ),
                        });
                    }
                }
            }
        }
        if matches!(
            self.tool,
            EditTool::Brush | EditTool::Smudge | EditTool::ColorMix
        ) && session.mode() == crate::interaction::Mode::Live
            && let Some(pen) = session.live().last_pen()
            && pen.phase != paint_input::Phase::Cancel
            && pen.in_range
            && !self.reference_under_client(pen.position_px)
            && let Some(mapping) = self.mapping.filter(|m| m.contains_client(pen.position_px))
        {
            let point = mapping
                .routing_viewport()
                .to_document(pen.position_px)
                .expect("valid mapped hover")
                .0;
            if let (Ok(center), Ok(edge)) = (
                mapping.to_clip(point),
                mapping.to_clip([
                    point[0] + session.live().project().brush.size / 2.0,
                    point[1],
                ]),
            ) {
                let center = egui::pos2(
                    (center[0] + 1.0) * size[0] as f32 / 2.0 / output.pixels_per_point,
                    (1.0 - center[1]) * size[1] as f32 / 2.0 / output.pixels_per_point,
                );
                let radius = ((edge[0] - mapping.to_clip(point).expect("validated point")[0])
                    * size[0] as f32
                    / 2.0
                    / output.pixels_per_point)
                    .abs();
                for (width, color) in [(3.0, egui::Color32::BLACK), (1.0, egui::Color32::WHITE)] {
                    output.shapes.push(egui::epaint::ClippedShape {
                        clip_rect: central,
                        shape: egui::Shape::circle_stroke(
                            center,
                            radius,
                            egui::Stroke::new(width, color),
                        ),
                    });
                }
            }
        }
        session.live_mut().project_mut().sync_brush_settings();
        let now = Instant::now();
        let project = session.live().project();
        self.autosave_schedule
            .observe(project.revision, project.dirty(), now);
        if let Some(recovery) = &self.recovery {
            for result in recovery.poll() {
                self.recovery_saving = false;
                match result {
                    Ok(()) => {
                        if self.test_path_dialog {
                            crate::studio_log!(
                                "UI_TEST_RECOVERY_COMPLETE: {}",
                                self.last_submitted
                            );
                        }
                        self.status = "Recovery checkpoint complete".into();
                    }
                    Err(e) => {
                        if self.test_path_dialog {
                            crate::studio_log!(
                                "UI_TEST_RECOVERY_FAILED: {} {e}",
                                self.last_submitted
                            );
                        }
                        self.last_submitted = u64::MAX;
                        self.autosave_schedule.retry_later(now);
                        self.status = format!("RECOVERY FAILED: {e}; will retry shortly")
                    }
                }
            }
            let project = session.live().project();
            if project.dirty()
                && self.last_submitted != project.revision
                && !session.live().active()
                && self.autosave_schedule.ready(now)
            {
                if recovery.submit(project.state().clone()) {
                    self.last_submitted = project.revision;
                    self.autosave_schedule.submitted();
                    self.recovery_saving = true;
                    if self.test_path_dialog {
                        crate::studio_log!("UI_TEST_RECOVERY_SUBMITTED: {}", self.last_submitted);
                    }
                } else {
                    self.autosave_schedule.retry_later(now);
                }
            }
        }
        self.input
            .handle_platform_output(window, output.platform_output.clone());
        output
    }

    /// Theme surface for the viewport while Home is active. The GPU shell
    /// clears to this color before egui controls so empty space stays cohesive.
    pub fn home_surface_color(&self) -> Option<egui::Color32> {
        self.welcome
            .then(|| self.context.style().visuals.panel_fill)
    }

    fn execute(&mut self, a: Action, session: &mut CanvasSession) -> Result<(), Failure> {
        let project = session.live_mut().project_mut();
        match a {
            Action::ImportOra => {
                let state = paint_document::openraster::load(std::path::Path::new(&self.path))?;
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
                self.invalidate_document_thumbnail_cache();
                *project = Engine::from_state(state)?;
                project.revision = 1;
                self.recovery = Some(next);
                self.store = None;
                self.path = "imported-artwork.rpl".into();
                self.last_submitted = u64::MAX;
                self.status="OpenRaster imported. Save As .rpl to retain native editing. Layer locks/presets/history are not in ORA; resolution rounds to integer DPI".into();
            }
            Action::ExportOra => {
                paint_document::openraster::export(std::path::Path::new(&self.path), project)?;
                self.status="OpenRaster exported: raster layers retained; background becomes a layer. Locks, presets and history are omitted".into();
            }
            Action::RemoveRecovery(path) => {
                paint_document::recovery::remove(&paint_document::recovery::directory()?, &path)?;
                self.recovery_files.retain(|p| p != &path);
                self.status = "Selected recovery checkpoint and backup removed".into();
            }
            Action::RemoveDraft(path) => {
                crate::drafts::remove(&path)?;
                self.drafts = crate::drafts::list();
                self.status = "Selected draft and backup removed".into();
            }
            Action::AddReference(_) => {
                return Err("Reference import is handled by the workspace".into());
            }
            Action::RetryClose => {
                return Err("Close retry must be handled by the workspace".into());
            }
            Action::Fill(erase) => project.fill_selection(erase)?,
            Action::Deselect => project.set_selection(None)?,
            Action::Transform(m) => project.transform_selection(m)?,
            Action::Crop => project.crop_selection()?,
            Action::Undo => {
                project.undo();
                if self.test_path_dialog {
                    let selected = project.state().selected;
                    let opacity = project
                        .state()
                        .layers
                        .iter()
                        .find(|layer| layer.id == selected)
                        .map(|layer| layer.opacity)
                        .unwrap_or(0);
                    crate::studio_log!(
                        "UI_TEST_HISTORY_UNDO: {} {} {} {}",
                        project.revision,
                        selected,
                        project.state().layers.len(),
                        opacity
                    );
                    crate::studio_log!(
                        "UI_TEST_HISTORY_LAYER_COUNT: {}",
                        project.state().layers.len()
                    );
                }
            }
            Action::Redo => {
                project.redo();
                if self.test_path_dialog {
                    let selected = project.state().selected;
                    let opacity = project
                        .state()
                        .layers
                        .iter()
                        .find(|layer| layer.id == selected)
                        .map(|layer| layer.opacity)
                        .unwrap_or(0);
                    crate::studio_log!(
                        "UI_TEST_HISTORY_REDO: {} {} {} {}",
                        project.revision,
                        selected,
                        project.state().layers.len(),
                        opacity
                    );
                }
            }
            Action::Add => project.add_layer()?,
            Action::Duplicate => project.duplicate_layer()?,
            Action::Delete => project.delete_layer()?,
            Action::DuplicateLayer(id) => {
                project.select(id);
                project.duplicate_layer()?;
                if self.test_path_dialog {
                    crate::studio_log!(
                        "UI_TEST_LAYER_CONTEXT_DUPLICATED: {} {}",
                        project.state().layers.len(),
                        project.state().selected,
                    );
                }
            }
            Action::CopySelectionToLayer => {
                let source_id = project.state().selected;
                let copied_id = project.copy_selection_to_new_layer()?;
                if self.test_path_dialog {
                    let layer = project
                        .state()
                        .layers
                        .iter()
                        .find(|layer| layer.id == copied_id)
                        .ok_or(paint_document::PaintError::Invalid)?;
                    crate::studio_log!(
                        "UI_TEST_LAYER_SELECTION_COPIED: {source_id} {copied_id} {}",
                        layer.tiles.len()
                    );
                }
            }
            Action::DeleteLayer(id) => {
                project.select(id);
                project.delete_layer()?;
                if self.test_path_dialog {
                    crate::studio_log!(
                        "UI_TEST_LAYER_CONTEXT_DELETED: {} {}",
                        project.state().layers.len(),
                        project.state().selected,
                    );
                }
            }
            Action::ClearLayer(id) => {
                project.clear_layer_by_id(id)?;
                if self.test_path_dialog {
                    crate::studio_log!("UI_TEST_LAYER_CONTEXT_CLEARED: {id}");
                }
            }
            Action::Clear => project.clear_layer()?,
            Action::Select(id) => project.select(id),
            Action::Move(delta) => project.edit(|s| {
                let i = s
                    .layers
                    .iter()
                    .position(|l| l.id == s.selected)
                    .ok_or(paint_document::PaintError::Invalid)?;
                let j = (i as i32 + delta).clamp(0, s.layers.len() as i32 - 1) as usize;
                s.layers.swap(i, j);
                Ok(())
            })?,
            Action::MoveLayer(id, delta) => {
                project.edit(|s| {
                    let i = s
                        .layers
                        .iter()
                        .position(|layer| layer.id == id)
                        .ok_or(paint_document::PaintError::Invalid)?;
                    let j = (i as i32 + delta).clamp(0, s.layers.len() as i32 - 1) as usize;
                    s.selected = id;
                    s.layers.swap(i, j);
                    Ok(())
                })?;
                if self.test_path_dialog {
                    let index = project
                        .state()
                        .layers
                        .iter()
                        .position(|layer| layer.id == id)
                        .unwrap_or(usize::MAX);
                    crate::studio_log!("UI_TEST_LAYER_CONTEXT_MOVED: {id} {index}");
                }
            }
            Action::MoveLayerTo(id, target_index) => {
                project.edit(|state| {
                    let source_index = state
                        .layers
                        .iter()
                        .position(|layer| layer.id == id)
                        .ok_or(paint_document::PaintError::Invalid)?;
                    let layer = state.layers.remove(source_index);
                    let target_index = target_index.min(state.layers.len());
                    state.layers.insert(target_index, layer);
                    state.selected = id;
                    Ok(())
                })?;
                if self.test_path_dialog {
                    let final_index = project
                        .state()
                        .layers
                        .iter()
                        .position(|layer| layer.id == id)
                        .unwrap_or(usize::MAX);
                    crate::studio_log!(
                        "UI_TEST_LAYER_DRAG_REORDERED: {id} {target_index} {final_index}"
                    );
                }
            }
            Action::Layer(id, name, visible, locked, opacity, blend) => {
                project.edit(|s| {
                    let l = s
                        .layers
                        .iter_mut()
                        .find(|l| l.id == id)
                        .ok_or(paint_document::PaintError::Invalid)?;
                    l.name = name;
                    l.visible = visible;
                    l.locked = locked;
                    l.opacity = opacity;
                    l.blend = blend;
                    Ok(())
                })?;
                if self.test_path_dialog
                    && let Some(layer) = project.state().layers.iter().find(|layer| layer.id == id)
                {
                    crate::studio_log!(
                        "UI_TEST_LAYER_STATE_APPLIED: {} visible={} locked={}",
                        id,
                        layer.visible,
                        layer.locked
                    );
                    crate::studio_log!(
                        "UI_TEST_LAYER_COMPOSITING_APPLIED: {} opacity={} blend={:?}",
                        id,
                        layer.opacity,
                        layer.blend
                    );
                }
            }
            Action::New => {
                // Each document keeps its own recovery lineage. Never replace the
                // previous document checkpoint with the next document snapshot.
                let state = crate::document_setup::build_state(
                    self.new_width,
                    self.new_height,
                    self.new_unit,
                    self.new_dpi,
                    self.new_background,
                    self.new_transparent,
                )?;
                let width = state.width;
                let height = state.height;
                let recent_size = crate::workspace::RecentDocumentSize {
                    width_px: width,
                    height_px: height,
                    dpi: self.new_dpi,
                };
                self.recent_document_sizes
                    .retain(|size| *size != recent_size);
                self.recent_document_sizes.insert(0, recent_size);
                self.recent_document_sizes
                    .truncate(crate::workspace::MAX_RECENT_DOCUMENT_SIZES);
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
                self.recovery = Some(next);
                self.invalidate_document_thumbnail_cache();
                *project = Engine::from_state(state)?;
                self.store = None;
                self.path = format!(
                    "{}.rpl",
                    crate::document_setup::sanitized_document_stem(&self.new_name)
                );
                self.last_submitted = u64::MAX;
                self.welcome = false;
                self.status = format!("Created {width} × {height} px at {} DPI", self.new_dpi);
                if self.test_path_dialog {
                    crate::studio_log!(
                        "UI_TEST_DOCUMENT_CREATED: {width} {height} {} {}",
                        self.new_name,
                        project.state().layers.len(),
                    );
                }
                if let Err(error) = self.persist_workspace() {
                    self.status =
                        format!("Document created; setup preference save failed: {error}");
                }
            }
            Action::Recover(path) => {
                let (state, fallback) = paint_document::recovery::restore(&path)?;
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
                self.invalidate_document_thumbnail_cache();
                *project = Engine::from_state(state)?;
                project.revision = 1;
                self.recovery = Some(next);
                self.path = "recovered-artwork.rpl".into();
                self.store = None;
                self.last_submitted = u64::MAX;
                self.welcome = false;
                self.status = if fallback {
                    "RECOVERED PREVIOUS CHECKPOINT: newest recovery data was invalid; edits after the previous complete checkpoint may be missing. Save As a new native project"
                } else {
                    "Recovery restored into a new unsaved document; Save As a native project"
                }
                .into();
            }
            Action::Open | Action::Recent(_) => {
                let path = if let Action::Recent(p) = &a {
                    p.clone()
                } else {
                    PathBuf::from(&self.path)
                };
                let (store, state) = Store::open(path)?;
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
                self.invalidate_document_thumbnail_cache();
                *project = Engine::from_state(state)?;
                self.recovery = Some(next);
                self.path = store.path.to_string_lossy().into();
                let path = store.path.clone();
                self.store = Some(store);
                self.cache_home_thumbnail(&path, project);
                self.remember(&path);
                self.last_submitted = u64::MAX;
                self.welcome = false;
            }
            Action::Save | Action::SaveAs => {
                project.sync_brush_settings();
                if matches!(a, Action::SaveAs) || self.store.is_none() {
                    let path = native_project_path(std::path::Path::new(&self.path));
                    let mut replacement = Store::new(path.clone());
                    replacement.save(project.state())?;
                    self.store = Some(replacement);
                    self.path = path.to_string_lossy().into_owned();
                } else {
                    self.store.as_mut().unwrap().save(project.state())?;
                }
                project.saved_revision = project.revision;
                if self.close_requested {
                    self.close_ready = true;
                }
                self.status = "Editable project saved; previous valid file retained as .bak".into();
                let path = self.store.as_ref().unwrap().path.clone();
                self.cache_home_thumbnail(&path, project);
                self.remember(&path);
            }
            Action::SaveBrush => {
                let path = native_project_path(std::path::Path::new(&self.path))
                    .with_extension("rplbrush");
                storage::save_brush_preset(&path, &project.brush)?;
                self.path = path.to_string_lossy().into_owned();
                self.status =
                    "Versioned brush preset saved without replacing an existing file".into();
            }
            Action::LoadBrush => {
                let path = native_project_path(std::path::Path::new(&self.path))
                    .with_extension("rplbrush");
                project.brush = storage::load_brush_preset(&path)?;
                self.path = path.to_string_lossy().into_owned();
                self.status = "Validated brush preset loaded".into();
            }
            Action::Export => {
                storage::export_png(std::path::Path::new(&self.path), project)?;
                self.status = "PNG exported (flattened 8-bit sRGB RGBA)".into();
            }
            Action::Import => {
                storage::import_png(std::path::Path::new(&self.path), project)?;
                self.status="PNG imported as 8-bit encoded-sRGB. ICC profiles, animation and 16-bit precision are not preserved".into();
            }
        }
        Ok(())
    }
    pub fn paint(
        &mut self,
        g: &Renderer,
        view: &wgpu::TextureView,
        output: egui::FullOutput,
        size: [u32; 2],
    ) {
        let jobs = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: output.pixels_per_point,
        };
        for (id, delta) in &output.textures_delta.set {
            self.painter.update_texture(&g.device, &g.queue, *id, delta);
        }
        let mut encoder = g.device.create_command_encoder(&Default::default());
        let extra = self
            .painter
            .update_buffers(&g.device, &g.queue, &mut encoder, &jobs, &screen);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("painting controls"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.painter
                .render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        g.queue.submit(extra.into_iter().chain([encoder.finish()]));
        for id in &output.textures_delta.free {
            self.painter.free_texture(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FormatIllustration, LassoAppend, LayerMenuTarget, activate_brush_preset,
        append_lasso_point, brush_size_after_shortcut, document_format_card_height,
        document_format_grid, fit_paper_to_box, format_illustration, home_hero_height,
        home_open_card_needed, home_sections_stacked, layer_actions_menu, native_project_path,
        quick_document_card_layout, recent_artwork_preview_height, recent_gallery_layout,
        recent_project_matches, recovery_checkpoint_label, recovery_source_label,
    };
    use paint_document::brush::{Brush, BrushKind};
    use std::path::{Path, PathBuf};

    #[test]
    fn lasso_keeps_spacing_and_never_silently_truncates() {
        let mut points = vec![[0.0, 0.0]; 2];
        assert_eq!(
            append_lasso_point(&mut points, [1.0, 0.0]),
            LassoAppend::Unchanged
        );
        assert_eq!(
            append_lasso_point(&mut points, [2.0, 0.0]),
            LassoAppend::Added
        );
        assert_eq!(points, vec![[0.0, 0.0], [0.0, 0.0], [2.0, 0.0]]);

        points.resize(paint_document::editing::MAX_SELECTION_POINTS, [2.0, 0.0]);
        let count = points.len();
        assert_eq!(
            append_lasso_point(&mut points, [4.0, 0.0]),
            LassoAppend::LimitReached
        );
        assert_eq!(points.len(), count);
    }

    #[test]
    fn brush_size_shortcuts_are_predictable_and_bounded() {
        let initial = 18.0;
        let larger = brush_size_after_shortcut(initial, true, false);
        assert!((larger - 19.8).abs() < 0.001);
        let restored = brush_size_after_shortcut(larger, false, false);
        assert!((restored - initial).abs() < 0.001);
        assert!(brush_size_after_shortcut(initial, true, true) < larger);
        assert_eq!(brush_size_after_shortcut(256.0, true, false), 256.0);
        assert_eq!(brush_size_after_shortcut(1.0, false, false), 1.0);
        assert_eq!(brush_size_after_shortcut(f64::NAN, true, false), 1.1);
        assert_eq!(brush_size_after_shortcut(f64::INFINITY, false, false), 1.0);
    }

    #[test]
    fn selected_artwork_copy_action_is_named_and_selection_gated_in_accesskit() {
        let context = egui::Context::default();
        crate::ui_tokens::install(&context);
        context.enable_accesskit();
        let mut actions = Vec::new();
        let mut focus_rename = false;
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                layer_actions_menu(
                    ui,
                    LayerMenuTarget {
                        id: 7,
                        layer_count: 2,
                        layer_index: 1,
                        emit_test_markers: false,
                        name: "Ink".into(),
                        visible: true,
                        locked: false,
                        selected: true,
                        has_selection: true,
                        opacity: 255,
                        blend: paint_document::Blend::Normal,
                    },
                    &mut actions,
                    &mut focus_rename,
                );
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("the layer actions remain exposed to AccessKit");
        let nodes = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            nodes.contains("Copy selected pixels to new layer"),
            "selection copy is missing from the keyboard/screen-reader action list: {nodes}"
        );
        assert!(
            nodes.contains("role: Button"),
            "selection copy must be a semantic button: {nodes}"
        );
        assert!(
            actions.is_empty(),
            "rendering the menu must not mutate artwork"
        );
    }

    #[test]
    fn quick_brush_preset_switch_updates_real_settings_and_keeps_the_color() {
        let mut brush = Brush::preset(BrushKind::Round);
        brush.color = [102, 44, 212];
        activate_brush_preset(&mut brush, BrushKind::Ink);
        assert_eq!(brush.kind, BrushKind::Ink);
        assert_eq!(brush.size, 8.0);
        assert_eq!(brush.hardness, 1.0);
        assert_eq!(brush.color, [102, 44, 212]);
        assert!(brush.validate());

        activate_brush_preset(&mut brush, BrushKind::Eraser);
        assert_eq!(brush.kind, BrushKind::Eraser);
        assert_eq!(brush.color, [102, 44, 212]);
        assert!(brush.validate());
    }

    #[test]
    fn lasso_capacity_does_not_reject_subthreshold_motion() {
        let mut points = vec![[0.0, 0.0]; paint_document::editing::MAX_SELECTION_POINTS];
        assert_eq!(
            append_lasso_point(&mut points, [1.0, 0.0]),
            LassoAppend::Unchanged
        );
        assert_eq!(points.len(), paint_document::editing::MAX_SELECTION_POINTS);
    }

    #[test]
    fn native_save_path_is_not_confused_by_image_export_path() {
        assert_eq!(
            native_project_path(Path::new("artwork.png")),
            PathBuf::from("artwork.rpl")
        );
        assert_eq!(
            native_project_path(Path::new("artwork")),
            PathBuf::from("artwork.rpl")
        );
    }

    #[test]
    fn recent_artwork_search_matches_filenames_case_insensitively_and_trims_query() {
        let path = Path::new("/work/Neon Alley.rpl");
        assert!(recent_project_matches(path, ""));
        assert!(recent_project_matches(path, " neon "));
        assert!(recent_project_matches(path, "RPL"));
        assert!(!recent_project_matches(path, "portrait"));
    }

    #[test]
    fn recent_artwork_gallery_accounts_for_card_insets_at_each_breakpoint() {
        for (available, item_count, expected_columns) in
            [(420.0, 1, 1), (800.0, 2, 2), (1280.0, 3, 3), (1920.0, 4, 4)]
        {
            let (columns, card_width) = recent_gallery_layout(available, item_count);
            assert_eq!(columns, expected_columns, "viewport width {available}");
            assert!((180.0..=476.0).contains(&card_width));
            let total = columns as f32 * (card_width + 24.0) + 14.0 * (columns - 1) as f32;
            assert!(total <= available + 0.1, "cards overflow {available}px");
        }
    }

    #[test]
    fn home_gallery_adds_a_real_open_action_only_to_an_unfiltered_partial_row() {
        assert!(home_open_card_needed(3, 4, true));
        assert!(!home_open_card_needed(4, 4, true));
        assert!(!home_open_card_needed(3, 4, false));
        assert!(!home_open_card_needed(0, 4, true));
        assert_eq!(recent_gallery_layout(1920.0, 3).0, 3);
        assert_eq!(recent_gallery_layout(1920.0, 4).0, 4);
    }

    #[test]
    fn document_format_grid_reflows_without_overflow_from_compact_to_wide_dialogs() {
        for (available, expected_columns) in [(260.0, 1), (480.0, 2), (680.0, 3), (848.0, 4)] {
            let (columns, card_width) = document_format_grid(available);
            assert_eq!(columns, expected_columns, "dialog width {available}");
            assert!((108.0..=190.0).contains(&card_width));
            let total = columns as f32 * card_width + 12.0 * (columns - 1) as f32;
            assert!(total <= available + 0.1, "cards overflow {available}px");
        }
    }

    #[test]
    fn document_format_cards_compact_for_short_viewports() {
        assert_eq!(document_format_card_height(480.0), 112.0);
        assert_eq!(document_format_card_height(560.0), 112.0);
        assert_eq!(document_format_card_height(561.0), 132.0);
        assert_eq!(document_format_card_height(800.0), 132.0);
        assert_eq!(document_format_card_height(f32::NAN), 132.0);

        // The narrow dialog's three rows now fit within its initial viewport,
        // including both row gaps and the short heading/subtitle block.
        let grid_height = 3.0 * document_format_card_height(480.0) + 2.0 * 7.0;
        assert!(grid_height + 88.0 <= 448.0);
    }

    #[test]
    fn format_cards_use_distinct_visual_cues_and_true_page_aspects() {
        let labels = [
            "Portrait",
            "Landscape",
            "Square",
            "A4 Print",
            "Comic Page",
            "Full HD",
            "Custom",
        ];
        let kinds = labels.map(format_illustration);
        for left in 0..kinds.len() {
            for right in left + 1..kinds.len() {
                assert_ne!(
                    kinds[left], kinds[right],
                    "{} and {} need distinct art",
                    labels[left], labels[right]
                );
            }
        }
        let box_size = egui::vec2(72.0, 62.0);
        let portrait = fit_paper_to_box(box_size, 1800.0, 2400.0);
        let landscape = fit_paper_to_box(box_size, 2400.0, 1800.0);
        let square = fit_paper_to_box(box_size, 2048.0, 2048.0);
        assert!(portrait.x < portrait.y);
        assert!(landscape.x > landscape.y);
        assert!((square.x - square.y).abs() < 0.01);
        assert_eq!(
            fit_paper_to_box(box_size, f32::NAN, 0.0),
            egui::vec2(62.0, 62.0)
        );
        assert_eq!(kinds[0], FormatIllustration::Portrait);
    }

    #[test]
    fn recovery_labels_hide_internal_numeric_identifiers() {
        assert_eq!(
            recovery_checkpoint_label(Path::new("recovery-visual-checkpoint.rpl"), 0),
            "Visual checkpoint"
        );
        assert_eq!(
            recovery_checkpoint_label(Path::new("recovery-1792972309-473412.rpl"), 0),
            "Artwork checkpoint · Most recent"
        );
        assert_eq!(
            recovery_checkpoint_label(Path::new("recovery-1792972309-473412.rpl"), 1),
            "Artwork checkpoint · 2"
        );
    }

    #[test]
    fn recovery_manager_identifies_which_durable_source_can_be_restored() {
        assert_eq!(
            recovery_source_label(true, true, true),
            "Snapshot available"
        );
        assert_eq!(
            recovery_source_label(false, true, false),
            "Journal checkpoint · validated on recovery"
        );
        assert_eq!(
            recovery_source_label(false, false, true),
            "Backup checkpoint · validated on recovery"
        );
        assert_eq!(
            recovery_source_label(false, false, false),
            "Recovery data unavailable"
        );
    }

    #[test]
    fn home_stacks_secondary_panel_on_narrow_and_wide_displays() {
        assert!(home_sections_stacked(640.0));
        assert!(!home_sections_stacked(1280.0));
        assert!(home_sections_stacked(1920.0));
    }

    #[test]
    fn home_hero_reserves_more_space_for_real_art_and_adapts_to_narrow_windows() {
        assert_eq!(home_hero_height(640.0), 280.0);
        assert_eq!(home_hero_height(1280.0), 340.0);
    }

    #[test]
    fn recent_artwork_previews_leave_room_for_project_details() {
        assert_eq!(recent_artwork_preview_height(180.0), 128.0);
        assert!((recent_artwork_preview_height(360.0) - 172.8).abs() < 0.01);
        assert_eq!(recent_artwork_preview_height(500.0), 176.0);
    }

    #[test]
    fn quick_document_cards_fit_narrow_normal_and_wide_home_layouts() {
        for (width, expected) in [
            (360.0, 1),
            (640.0, 2),
            (1024.0, 3),
            (1280.0, 5),
            (1840.0, 5),
        ] {
            let (columns, card_width) = quick_document_card_layout(width);
            assert_eq!(columns, expected);
            assert!((172.0..=410.0).contains(&card_width));
            assert!(columns as f32 * card_width + 12.0 * (columns - 1) as f32 <= width + 0.1);
        }
    }
}
