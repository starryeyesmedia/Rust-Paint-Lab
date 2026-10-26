//! Functional alpha controls. No widget is presented without an implemented action.
use crate::{
    gpu::{Failure, Renderer},
    interaction::{CanvasSession, MouseEvent},
    scene::CanvasMapping,
};
use paint_document::{
    Blend, Engine,
    brush::{Brush, BrushKind},
    editing::{Selection, SelectionKind},
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

type AppearanceKey = (
    crate::workspace::AppearanceMode,
    crate::workspace::AccentScheme,
    Option<[u8; 3]>,
    bool,
    bool,
    bool,
    u32,
);

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

fn recent_gallery_layout(available_width: f32) -> (usize, f32) {
    let columns = match available_width {
        width if width >= 1500.0 => 4,
        width if width >= 1000.0 => 3,
        width if width >= 600.0 => 2,
        _ => 1,
    };
    let gaps = 14.0 * (columns - 1) as f32;
    let card_width = ((available_width - gaps) / columns as f32).clamp(180.0, 500.0);
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

pub(crate) fn home_document_card(
    ui: &mut egui::Ui,
    name: &str,
    dimensions: [u32; 2],
    card_width: f32,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(card_width, 124.0), egui::Sense::click());
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
    let aspect = dimensions[0] as f32 / dimensions[1] as f32;
    let paper_size = if aspect >= 1.0 {
        egui::vec2(max_paper.x, max_paper.x / aspect)
    } else {
        egui::vec2(max_paper.y * aspect, max_paper.y)
    };
    let paper =
        egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 37.0), paper_size);
    ui.painter()
        .rect_filled(paper, 3.0, visuals.extreme_bg_color);
    ui.painter().rect_stroke(
        paper,
        3.0,
        egui::Stroke::new(1.5, visuals.text_color()),
        egui::StrokeKind::Inside,
    );
    for row in 0..3 {
        let y = paper.top() + 9.0 + row as f32 * 7.0;
        if y < paper.bottom() - 4.0 {
            ui.painter().line_segment(
                [
                    egui::pos2(paper.left() + 5.0, y),
                    egui::pos2(paper.right() - 5.0, y),
                ],
                egui::Stroke::new(1.0, visuals.window_stroke.color),
            );
        }
    }
    let fg = crate::appearance::readable_text_color(visuals.text_color(), fill);
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 26.0),
        egui::Align2::CENTER_CENTER,
        name,
        egui::FontId::proportional(15.0),
        fg,
    );
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 10.0),
        egui::Align2::CENTER_CENTER,
        format!("{} × {} px", dimensions[0], dimensions[1]),
        egui::FontId::proportional(11.0),
        crate::appearance::readable_text_color(visuals.weak_text_color(), fill),
    );
    response.clicked()
        || (response.has_focus()
            && ui.input(|input| {
                input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::Space)
            }))
}

pub(crate) fn home_custom_document_card(ui: &mut egui::Ui, card_width: f32) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(card_width, 124.0), egui::Sense::click());
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
    ui.painter().text(
        egui::pos2(rect.center().x, rect.top() + 37.0),
        egui::Align2::CENTER_CENTER,
        "+",
        egui::FontId::proportional(38.0),
        visuals.hyperlink_color,
    );
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 23.0),
        egui::Align2::CENTER_CENTER,
        "Custom size",
        egui::FontId::proportional(15.0),
        visuals.text_color(),
    );
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 8.0),
        egui::Align2::CENTER_CENTER,
        "Set exact dimensions",
        egui::FontId::proportional(11.0),
        crate::appearance::readable_text_color(visuals.weak_text_color(), fill),
    );
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
        egui::FontId::proportional(17.0),
        egui::Color32::WHITE,
    );
    ui.painter().text(
        egui::pos2(caption.left() + 16.0, caption.top() + 40.0),
        egui::Align2::LEFT_CENTER,
        "Continue creating",
        egui::FontId::proportional(12.0),
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

pub(crate) fn document_format_card(
    ui: &mut egui::Ui,
    label: &str,
    width: f32,
    height: f32,
    dpi: u32,
    selected: bool,
    card_width: f32,
) -> bool {
    let size = egui::vec2(card_width, 116.0);
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
    let ratio = width / height;
    let paper_max = egui::vec2((card_width - 24.0).min(64.0), 58.0);
    let paper_size = if ratio >= 1.0 {
        egui::vec2(paper_max.x, paper_max.x / ratio)
    } else {
        egui::vec2(paper_max.y * ratio, paper_max.y)
    };
    let paper =
        egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 34.0), paper_size);
    painter.rect_filled(paper, 3.0, visuals.extreme_bg_color);
    painter.rect_stroke(
        paper,
        3.0,
        egui::Stroke::new(1.5, visuals.text_color()),
        egui::StrokeKind::Inside,
    );
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 35.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(14.0),
        crate::appearance::readable_text_color(visuals.text_color(), fill),
    );
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 20.0),
        egui::Align2::CENTER_CENTER,
        format!("{}×{} px", width as u32, height as u32),
        egui::FontId::proportional(10.0),
        crate::appearance::readable_text_color(visuals.weak_text_color(), fill),
    );
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 7.0),
        egui::Align2::CENTER_CENTER,
        format!("{dpi} DPI"),
        egui::FontId::proportional(10.0),
        crate::appearance::readable_text_color(visuals.weak_text_color(), fill),
    );
    activate_focusable_card(ui, &response)
}

pub(crate) fn brush_preset_card(
    ui: &mut egui::Ui,
    kind: BrushKind,
    selected: bool,
    color: [u8; 3],
    card_width: f32,
) -> bool {
    let name = brush_display_name(kind);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(card_width.max(120.0), 82.0),
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
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 13.0),
        egui::Align2::CENTER_CENTER,
        name,
        egui::FontId::proportional(13.0),
        visuals.text_color(),
    );
    if selected {
        painter.text(
            egui::pos2(rect.right() - 8.0, rect.top() + 9.0),
            egui::Align2::RIGHT_CENTER,
            "ACTIVE",
            egui::FontId::proportional(8.0),
            visuals.text_color(),
        );
    }
    activate_focusable_card(ui, &response)
}

fn color_swatch(ui: &mut egui::Ui, color: [u8; 3], label: &str, selected: bool, size: f32) -> bool {
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
    response.on_hover_text(label).clicked()
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
    Clear,
    Select(u64),
    Move(i32),
    Layer(u64, String, bool, bool, u8, Blend),
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

pub(crate) fn layer_flyout_contents(
    ui: &mut egui::Ui,
    project: &mut Engine,
    actions: &mut Vec<Action>,
) {
    let selected = project.state().selected;
    let selected_index = project
        .state()
        .layers
        .iter()
        .position(|layer| layer.id == selected)
        .unwrap_or(0);
    let layer_count = project.state().layers.len();
    let canvas_size = [project.state().width, project.state().height];
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
                layer_thumbnail_samples(layer, canvas_size),
            )
        })
        .collect();
    ui.horizontal(|ui| {
        if ui.button("New").clicked() {
            actions.push(Action::Add);
        }
        if ui.button("Duplicate").clicked() {
            actions.push(Action::Duplicate);
        }
        if ui
            .add_enabled(layers.len() > 1, egui::Button::new("Delete"))
            .clicked()
        {
            actions.push(Action::Delete);
        }
    });
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
    for (id, mut name, mut visible, mut locked, thumbnail) in layers {
        egui::Frame::new()
            .fill(if id == selected {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().faint_bg_color
            })
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    layer_thumbnail(ui, &thumbnail, id == selected);
                    if ui.selectable_label(id == selected, &name).clicked() {
                        actions.push(Action::Select(id));
                    }
                });
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut visible, "Visible").changed()
                        && let Some(layer) =
                            project.state().layers.iter().find(|layer| layer.id == id)
                    {
                        actions.push(Action::Layer(
                            id,
                            layer.name.clone(),
                            visible,
                            layer.locked,
                            layer.opacity,
                            layer.blend,
                        ));
                    }
                    if ui.checkbox(&mut locked, "Locked").changed()
                        && let Some(layer) =
                            project.state().layers.iter().find(|layer| layer.id == id)
                    {
                        actions.push(Action::Layer(
                            id,
                            layer.name.clone(),
                            layer.visible,
                            locked,
                            layer.opacity,
                            layer.blend,
                        ));
                    }
                });
                if id == selected {
                    let selected_layer = project.state().layers.iter().find(|layer| layer.id == id);
                    let (mut opacity, mut blend) = selected_layer
                        .map(|layer| (layer.opacity, layer.blend))
                        .unwrap_or((255, Blend::Normal));
                    let mut changed = ui
                        .add(egui::TextEdit::singleline(&mut name).hint_text("Layer name"))
                        .changed();
                    let mut percent = f32::from(opacity) * (100.0 / 255.0);
                    if ui
                        .add(
                            egui::Slider::new(&mut percent, 0.0..=100.0)
                                .suffix("%")
                                .text("Opacity"),
                        )
                        .changed()
                    {
                        opacity = (percent * 2.55).round() as u8;
                        changed = true;
                    }
                    let previous_blend = blend;
                    egui::ComboBox::from_id_salt(("flyout-blend", id))
                        .selected_text(format!("{blend:?}"))
                        .show_ui(ui, |ui| {
                            for mode in [
                                Blend::Normal,
                                Blend::Multiply,
                                Blend::Screen,
                                Blend::Overlay,
                            ] {
                                ui.selectable_value(&mut blend, mode, format!("{mode:?}"));
                            }
                        });
                    changed |= blend != previous_blend;
                    if changed {
                        actions.push(Action::Layer(
                            id,
                            name.clone(),
                            visible,
                            locked,
                            opacity,
                            blend,
                        ));
                    }
                }
            });
    }
}

const LAYER_PREVIEW_COLUMNS: usize = 10;
const LAYER_PREVIEW_ROWS: usize = 7;

fn layer_thumbnail_samples(
    layer: &paint_document::Layer,
    document_size: [u32; 2],
) -> Vec<egui::Color32> {
    let mut samples = Vec::with_capacity(LAYER_PREVIEW_COLUMNS * LAYER_PREVIEW_ROWS);
    let (width, height) = (document_size[0].max(1), document_size[1].max(1));
    for row in 0..LAYER_PREVIEW_ROWS {
        for column in 0..LAYER_PREVIEW_COLUMNS {
            let x0 = column as u64 * u64::from(width) / LAYER_PREVIEW_COLUMNS as u64;
            let x1 =
                ((column + 1) as u64 * u64::from(width) / LAYER_PREVIEW_COLUMNS as u64).max(x0 + 1);
            let y0 = row as u64 * u64::from(height) / LAYER_PREVIEW_ROWS as u64;
            let y1 = ((row + 1) as u64 * u64::from(height) / LAYER_PREVIEW_ROWS as u64).max(y0 + 1);
            let mut rgba = [0u32; 4];
            let mut count = 0u32;
            for sample_y in 0..4u64 {
                for sample_x in 0..4u64 {
                    let x = (x0 + (x1 - x0 - 1) * sample_x / 3).min(u64::from(width - 1)) as u32;
                    let y = (y0 + (y1 - y0 - 1) * sample_y / 3).min(u64::from(height - 1)) as u32;
                    count += 1;
                    let tile_x = x / paint_document::TILE;
                    let tile_y = y / paint_document::TILE;
                    let Some(tile) = layer.tiles.get(&(tile_x, tile_y)) else {
                        continue;
                    };
                    let pixel = ((y % paint_document::TILE * paint_document::TILE
                        + x % paint_document::TILE)
                        * 4) as usize;
                    for channel in 0..4 {
                        rgba[channel] += u32::from(tile[pixel + channel]);
                    }
                }
            }
            samples.push(if count == 0 {
                egui::Color32::TRANSPARENT
            } else {
                egui::Color32::from_rgba_premultiplied(
                    (rgba[0] / count) as u8,
                    (rgba[1] / count) as u8,
                    (rgba[2] / count) as u8,
                    (rgba[3] / count) as u8,
                )
            });
        }
    }
    samples
}

fn layer_thumbnail(ui: &mut egui::Ui, samples: &[egui::Color32], selected: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 44.0), egui::Sense::hover());
    let painter = ui.painter();
    let cell = egui::vec2(
        rect.width() / LAYER_PREVIEW_COLUMNS as f32,
        rect.height() / LAYER_PREVIEW_ROWS as f32,
    );
    for row in 0..LAYER_PREVIEW_ROWS {
        for column in 0..LAYER_PREVIEW_COLUMNS {
            let index = row * LAYER_PREVIEW_COLUMNS + column;
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
            if let Some(color) = samples.get(index).filter(|color| color.a() > 0) {
                painter.rect_filled(cell_rect, 0.0, *color);
            }
        }
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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EditTool {
    Brush,
    Rectangle,
    Ellipse,
    Lasso,
    Eyedropper,
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
    favorite_brushes: Vec<BrushKind>,
    brush_search: String,
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
    test_brush_rect_logged: bool,
    test_brush_layout_logged: bool,
    test_home_preview_logged: bool,
    test_home_quick_start_logged: bool,
    test_recovery_label_logged: bool,
    test_recovery_preview_hover_logged: bool,
    test_recovery_preview_target_logged: bool,
    test_recovery_preview_logged: bool,
    test_reference_preview_logged: bool,
    test_new_artwork_logged: bool,
    test_home_dark_contrast_logged: bool,
    test_home_light_contrast_logged: bool,
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
    palette_collapsed: bool,
    handedness: crate::workspace::Handedness,
    palette_position: [f32; 2],
    layers_flyout_position: [f32; 2],
    layers_flyout_size: [f32; 2],
    brush_flyout_position: [f32; 2],
    color_flyout_position: [f32; 2],
    brush_flyout_open: bool,
    layers_flyout_open: bool,
    color_flyout_open: bool,
    navigation: crate::navigation::Navigation,
    touches: crate::navigation::TouchNavigation,
    touch_routes: crate::navigation::TouchTargetRouter,
    control_held: bool,
    workspace_needs_save: bool,
    pan_cursor: Option<[f64; 2]>,
    workspace_center: [f64; 2],
    tool: EditTool,
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
        let favorite_brushes = preferences.favorite_brushes.clone();
        let references: Vec<ReferenceImage> = preferences
            .references
            .iter()
            .cloned()
            .filter_map(|layout| load_reference_texture(&context, layout).ok())
            .collect();
        if test_path_dialog {
            eprintln!("UI_TEST_REFERENCE_COUNT: {}", references.len());
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
            favorite_brushes,
            brush_search: String::new(),
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
            test_brush_rect_logged: false,
            test_brush_layout_logged: false,
            test_home_preview_logged: false,
            test_home_quick_start_logged: false,
            test_recovery_label_logged: false,
            test_recovery_preview_hover_logged: false,
            test_recovery_preview_target_logged: false,
            test_recovery_preview_logged: false,
            test_reference_preview_logged: false,
            test_new_artwork_logged: false,
            test_home_dark_contrast_logged: false,
            test_home_light_contrast_logged: false,
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
            palette_collapsed: preferences.palette_collapsed,
            handedness: preferences.handedness,
            palette_position: preferences.palette_position,
            layers_flyout_position: preferences.layers_flyout_position,
            layers_flyout_size: preferences.layers_flyout_size,
            brush_flyout_position: preferences.brush_flyout_position,
            color_flyout_position: preferences.color_flyout_position,
            brush_flyout_open: false,
            layers_flyout_open: false,
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
    fn reference_under_client(&self, client_px: [f64; 2]) -> bool {
        self.reference_index_under_client(client_px).is_some()
    }
    fn reference_index_under_client(&self, client_px: [f64; 2]) -> Option<usize> {
        let pixels_per_point = self.context.pixels_per_point();
        self.references
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, reference)| {
                reference_contains_client(&reference.layout, client_px, pixels_per_point)
                    .then_some(index)
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
            eprintln!("TEST_PATH_DIALOG: Enter received at winit boundary");
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
            self.clear_touch_routes();
            self.pan_cursor = None;
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
                let viewport = reference.viewport.unwrap();
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
    fn editing_pointer(
        &mut self,
        session: &mut CanvasSession,
        id: u64,
        phase: paint_input::Phase,
        client: [f64; 2],
    ) -> bool {
        use paint_input::Phase;
        if self.tool == EditTool::Brush
            || session.mode() != crate::interaction::Mode::Live
            || self.blocks_canvas()
        {
            return false;
        }
        let on_canvas = self.mapping.is_some_and(|m| m.contains_client(client))
            && !self.reference_under_client(client);
        let captured = self.edit_pointer == Some(id);
        if !on_canvas && !captured {
            return false;
        }
        if phase == Phase::Cancel && !captured {
            return on_canvas;
        }
        if phase == Phase::Cancel || !on_canvas {
            self.edit_pointer = None;
            self.edit_points.clear();
            return true;
        }
        let Some(position) = self
            .mapping
            .and_then(|m| m.routing_viewport().to_document(client).ok())
            .map(|p| p.0)
        else {
            return true;
        };
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
                if self.tool == EditTool::Lasso {
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
                if phase == Phase::Up {
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
                }
            }
            _ => {}
        }
        true
    }
    pub fn tool_shortcut(&mut self, key: char, session: &mut CanvasSession) {
        session.mouse(MouseEvent::Interrupted, 0);
        self.edit_pointer = None;
        self.edit_points.clear();
        if key == 'c' {
            self.color_flyout_open = !self.color_flyout_open;
            self.brush_flyout_open = false;
            if self.test_path_dialog && self.color_flyout_open {
                eprintln!("UI_TEST_COLOR_FLYOUT_OPENED: shortcut");
            }
            return;
        }
        if key == '0' {
            self.navigation = Default::default();
            self.workspace_needs_save = true;
            return;
        }
        self.tool = match key {
            'm' => EditTool::Rectangle,
            'e' => EditTool::Ellipse,
            'k' => EditTool::Lasso,
            'i' => EditTool::Eyedropper,
            _ => EditTool::Brush,
        };
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
                eprintln!("UI_TEST_BRUSH_FLYOUT_OPENED: shortcut");
            }
            return;
        }
        if key == 'l' {
            self.layers_flyout_open = !self.layers_flyout_open;
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
                eprintln!(
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
        let dismissed = if self.brush_flyout_open {
            self.brush_flyout_open = false;
            "brush"
        } else if self.color_flyout_open {
            self.color_flyout_open = false;
            "color"
        } else if self.layers_flyout_open {
            self.layers_flyout_open = false;
            "layers"
        } else {
            "none"
        };
        if self.test_path_dialog {
            eprintln!("UI_TEST_ESCAPE_DISMISSED: {dismissed}");
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
            favorite_brushes: self.favorite_brushes.clone(),
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
            ..Default::default()
        };
        if self.test_path_dialog {
            eprintln!(
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
                                .checkbox(&mut self.show_controls, "Show workspace panel")
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
                            ui.label("B Brush · C Color · M Rectangle · E Ellipse");
                            ui.label("K Lasso · I Eyedropper");
                            ui.label("Ctrl+N New · Ctrl+O Open · Ctrl+S Save");
                            ui.label("Ctrl+L Floating Layers panel");
                            ui.label("Ctrl+Shift+N New layer");
                            ui.label("Ctrl+Shift+M Artwork recovery manager");
                            ui.label("Ctrl+Z / Ctrl+Y Undo / Redo · Esc Close");
                        });
                    });
                });
            });

            egui::TopBottomPanel::top("contextual tool options")
                .resizable(false)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(ui.visuals().panel_fill)
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if self.tool == EditTool::Brush {
                                    let project = session.live_mut().project_mut();
                                    ui.strong(brush_display_name(project.brush.kind));
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
                                    if ui.button("Brushes · B").clicked() {
                                        self.brush_flyout_open = !self.brush_flyout_open;
                                        self.color_flyout_open = false;
                                    }
                                } else {
                                    let has_selection =
                                        session.live().project().state().selection.is_some();
                                    ui.strong(match self.tool {
                                        EditTool::Rectangle => "Rectangle selection",
                                        EditTool::Ellipse => "Ellipse selection",
                                        EditTool::Lasso => "Freehand selection",
                                        EditTool::Eyedropper => "Eyedropper",
                                        EditTool::Brush => "Brush",
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
                                ui.separator();
                                let project = session.live().project();
                                if ui
                                    .add_enabled(
                                        project.undo_count() > 0,
                                        egui::Button::new("Undo · Ctrl+Z"),
                                    )
                                    .clicked()
                                {
                                    actions.push(Action::Undo);
                                }
                                if ui
                                    .add_enabled(
                                        project.redo_count() > 0,
                                        egui::Button::new("Redo · Ctrl+Y"),
                                    )
                                    .clicked()
                                {
                                    actions.push(Action::Redo);
                                }
                            });
                        });
                });

            let old_tool = self.tool;
            let screen = ctx.content_rect();
            let floating_position = egui::pos2(self.palette_position[0], self.palette_position[1]);
            let dock_position =
                crate::studio_layout::palette_anchor(screen, self.palette_dock, floating_position);
            let horizontal_palette = matches!(
                self.palette_dock,
                crate::workspace::PaletteDock::Top | crate::workspace::PaletteDock::Bottom
            );
            let compact_palette =
                self.palette_collapsed || horizontal_palette || screen.width() < 760.0;
            let area = egui::Area::new(egui::Id::new("tool palette"))
                .movable(true)
                .constrain_to(screen.shrink(4.0))
                .order(egui::Order::Foreground);
            let area = if self.palette_dock != crate::workspace::PaletteDock::Floating
                && !self.palette_dragging
            {
                area.current_pos(dock_position)
            } else {
                area.default_pos(dock_position)
            };
            let palette_response = area.show(ctx, |ui| {
                egui::Frame::new()
                    .fill(ui.visuals().window_fill)
                    .stroke(ui.visuals().window_stroke)
                    .corner_radius(12.0)
                    .inner_margin(6.0)
                    .show(ui, |ui| {
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
                            ui.label(egui::RichText::new("⠿").size(18.0).strong())
                                .on_hover_text(
                                    "Drag this palette to move it; release near an edge to dock",
                                )
                                .on_disabled_hover_text("Tool palette");
                            if !compact_palette
                                && ui
                                    .small_button(if self.palette_collapsed {
                                        "Expand tools"
                                    } else {
                                        "Collapse tools"
                                    })
                                    .clicked()
                            {
                                self.palette_collapsed = !self.palette_collapsed;
                                workspace_changed = true;
                            }
                            for (tool, icon, name, shortcut) in [
                                (
                                    EditTool::Brush,
                                    crate::studio_icons::Icon::Brush,
                                    "Brush",
                                    "B",
                                ),
                                (
                                    EditTool::Rectangle,
                                    crate::studio_icons::Icon::Rectangle,
                                    "Rectangle selection",
                                    "M",
                                ),
                                (
                                    EditTool::Ellipse,
                                    crate::studio_icons::Icon::Ellipse,
                                    "Ellipse selection",
                                    "E",
                                ),
                                (
                                    EditTool::Lasso,
                                    crate::studio_icons::Icon::Lasso,
                                    "Freehand lasso",
                                    "K",
                                ),
                                (
                                    EditTool::Eyedropper,
                                    crate::studio_icons::Icon::Eyedropper,
                                    "Eyedropper",
                                    "I",
                                ),
                            ] {
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
                                    eprintln!(
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
                                    if tool == EditTool::Brush {
                                        self.show_controls = false;
                                        self.color_flyout_open = false;
                                        self.brush_flyout_open = true;
                                        if self.test_path_dialog {
                                            eprintln!("UI_TEST_BRUSH_FLYOUT_OPENED: palette");
                                        }
                                        workspace_changed = true;
                                    }
                                }
                            }
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
                            }
                            if !compact_palette {
                                let layers = ui
                                    .button("Layers")
                                    .on_hover_text("Open floating layer panel");
                                if layers.clicked() {
                                    self.layers_flyout_open = !self.layers_flyout_open;
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
                        } else if screen.height() < 620.0 {
                            egui::ScrollArea::vertical()
                                .max_height((screen.height() - 160.0).max(120.0))
                                .show(ui, |ui| ui.vertical_centered(&mut draw_tools));
                        } else {
                            ui.vertical_centered(&mut draw_tools);
                        }
                    });
            });
            let rect = palette_response.response.rect;
            let clamped = clamp_reference_position(
                [rect.min.x, rect.min.y],
                [rect.width(), rect.height()],
                screen,
            );
            self.palette_position = [clamped.x, clamped.y];
            if palette_response.response.drag_started() {
                self.palette_dragging = true;
            }
            if palette_response.response.drag_stopped() {
                self.palette_dock = crate::studio_layout::snap_palette(screen, rect);
                self.palette_dragging = false;
                workspace_changed = true;
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
            });

            if self.show_controls {
                let side_panel = egui::SidePanel::right("painting controls")
                    .default_width(self.brush_panel_width)
                    .resizable(true)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (tab, label) in [
                                    (crate::workspace::InspectorTab::Layers, "Layers"),
                                    (crate::workspace::InspectorTab::Brush, "Brush"),
                                    (crate::workspace::InspectorTab::Edit, "Edit"),
                                ] {
                                    if ui
                                        .selectable_label(self.inspector_tab == tab, label)
                                        .clicked()
                                    {
                                        self.inspector_tab = tab;
                                        workspace_changed = true;
                                    }
                                }
                            });
                            ui.separator();
                            let project = session.live_mut().project_mut();

                            if self.inspector_tab == crate::workspace::InspectorTab::Brush {
                                ui.heading("Brush presets and color");
                                egui::ComboBox::from_label("Preset")
                                    .selected_text(brush_display_name(project.brush.kind))
                                    .show_ui(ui, |ui| {
                                        for k in [
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
                                            if ui
                                                .selectable_label(
                                                    project.brush.kind == k,
                                                    brush_display_name(k),
                                                )
                                                .clicked()
                                            {
                                                let color = project.brush.color;
                                                project.brush = Brush::preset(k);
                                                project.brush.color = color;
                                            }
                                        }
                                    });

                                ui.horizontal(|ui| {
                                    crate::studio_icons::label(
                                        ui,
                                        crate::studio_icons::Icon::Color,
                                        "Color",
                                    );
                                    ui.color_edit_button_srgb(&mut project.brush.color);
                                    let mut eraser = project.brush.kind == BrushKind::Eraser;
                                    if ui.checkbox(&mut eraser, "Eraser mode").changed() {
                                        project.brush.kind = if eraser {
                                            BrushKind::Eraser
                                        } else {
                                            BrushKind::Round
                                        };
                                    }
                                });
                                if self.hex_color_for != Some(project.brush.color) {
                                    self.hex_color =
                                        crate::color_controls::format_hex_rgb(project.brush.color);
                                    self.hex_color_for = Some(project.brush.color);
                                }
                                ui.collapsing("Color values (sRGB)", |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label("HEX");
                                        let response = ui.add(
                                            egui::TextEdit::singleline(&mut self.hex_color)
                                                .char_limit(7)
                                                .desired_width(88.0),
                                        );
                                        let apply = ui.button("Apply").clicked()
                                            || (response.lost_focus()
                                                && ui.input(|input| {
                                                    input.key_pressed(egui::Key::Enter)
                                                }));
                                        if apply {
                                            if let Some(color) =
                                                crate::color_controls::parse_hex_rgb(
                                                    &self.hex_color,
                                                )
                                            {
                                                project.brush.color = color;
                                                self.hex_color =
                                                    crate::color_controls::format_hex_rgb(color);
                                                self.hex_color_for = Some(color);
                                                self.status = "Brush color updated".into();
                                            } else {
                                                self.status =
                                                    "Enter a six-digit color such as #4B86D1"
                                                        .into();
                                            }
                                        }
                                    });
                                    let mut rgb = project.brush.color;
                                    ui.horizontal(|ui| {
                                        ui.label("RGB");
                                        for component in &mut rgb {
                                            ui.add(
                                                egui::DragValue::new(component)
                                                    .range(0..=255)
                                                    .speed(1),
                                            );
                                        }
                                    });
                                    if rgb != project.brush.color {
                                        project.brush.color = rgb;
                                        self.hex_color = crate::color_controls::format_hex_rgb(rgb);
                                        self.hex_color_for = Some(rgb);
                                    }
                                });
                                ui.collapsing("Recent brush colors", |ui| {
                                    if self.recent_brush_colors.is_empty() {
                                        ui.small("Colors you choose will appear here.");
                                    } else {
                                        ui.horizontal_wrapped(|ui| {
                                            for color in self.recent_brush_colors.clone() {
                                                let hex =
                                                    crate::color_controls::format_hex_rgb(color);
                                                if crate::studio_icons::color_swatch(
                                                    ui,
                                                    egui::Color32::from_rgb(
                                                        color[0], color[1], color[2],
                                                    ),
                                                    &format!("Brush color {hex}"),
                                                )
                                                .clicked()
                                                {
                                                    project.brush.color = color;
                                                    self.hex_color = hex;
                                                    self.hex_color_for = Some(color);
                                                }
                                            }
                                        });
                                    }
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Size");
                                    ui.add(
                                        egui::DragValue::new(&mut project.brush.size)
                                            .range(1.0..=256.0)
                                            .speed(0.25)
                                            .suffix(" px"),
                                    );
                                });
                                ui.add(
                                    egui::Slider::new(&mut project.brush.size, 1.0..=256.0)
                                        .show_value(false)
                                        .text("Size"),
                                );
                                ui.horizontal(|ui| {
                                    ui.label("Opacity");
                                    ui.add(
                                        egui::DragValue::new(&mut project.brush.opacity)
                                            .range(0.0..=1.0)
                                            .speed(0.01),
                                    );
                                });
                                ui.add(
                                    egui::Slider::new(&mut project.brush.opacity, 0.0..=1.0)
                                        .show_value(false)
                                        .text("Opacity"),
                                );
                                ui.collapsing("More brush settings", |ui| {
                                    ui.add(
                                        egui::Slider::new(&mut project.brush.flow, 0.0..=1.0)
                                            .text("Flow"),
                                    );
                                    ui.add(
                                        egui::Slider::new(&mut project.brush.hardness, 0.0..=1.0)
                                            .text("Hardness"),
                                    );
                                    ui.add(
                                        egui::Slider::new(&mut project.brush.spacing, 0.05..=1.0)
                                            .text("Spacing"),
                                    );
                                    ui.add(
                                        egui::Slider::new(&mut project.brush.smoothing, 0.0..=1.0)
                                            .text("Smoothing"),
                                    );
                                    ui.add(
                                        egui::Slider::new(
                                            &mut project.brush.angle,
                                            -std::f64::consts::PI..=std::f64::consts::PI,
                                        )
                                        .text("Angle"),
                                    );
                                    ui.checkbox(&mut project.brush.pressure_size, "Pressure size");
                                    ui.checkbox(
                                        &mut project.brush.pressure_opacity,
                                        "Pressure opacity",
                                    );
                                    ui.checkbox(&mut project.brush.tilt, "Tilt angle");
                                });
                            }

                            if self.inspector_tab == crate::workspace::InspectorTab::Edit {
                                ui.heading("Canvas and selection");
                                let mut transparent = project.state().background.is_none();

                                if ui
                                    .checkbox(&mut transparent, "Transparent background")
                                    .changed()
                                    && let Err(e) = project.edit(|s| {
                                        s.background =
                                            if transparent { None } else { Some([255; 3]) };
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
                                        egui::Slider::new(
                                            &mut self.transform_angle,
                                            -180.0..=180.0,
                                        )
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
                                let selected_index = project
                                    .state()
                                    .layers
                                    .iter()
                                    .position(|layer| layer.id == selected)
                                    .unwrap_or(0);

                                ui.separator();
                                ui.heading("Layers");
                                ui.small(format!(
                                    "{layer_count} layers · top of list is frontmost"
                                ));
                                ui.horizontal(|ui| {
                                    if ui.button("New layer").clicked() {
                                        actions.push(Action::Add);
                                    }
                                    if ui.button("Duplicate").clicked() {
                                        actions.push(Action::Duplicate);
                                    }
                                    if ui
                                        .add_enabled(layer_count > 1, egui::Button::new("Delete"))
                                        .clicked()
                                    {
                                        actions.push(Action::Delete);
                                    }
                                });

                                ui.horizontal(|ui| {
                                    if ui
                                        .add_enabled(
                                            selected_index + 1 < layer_count,
                                            egui::Button::new("Raise"),
                                        )
                                        .clicked()
                                    {
                                        actions.push(Action::Move(1));
                                    }
                                    if ui
                                        .add_enabled(selected_index > 0, egui::Button::new("Lower"))
                                        .clicked()
                                    {
                                        actions.push(Action::Move(-1));
                                    }
                                    if ui.button("Clear layer").clicked() {
                                        actions.push(Action::Clear);
                                    }
                                });

                                let layers: Vec<_> = project
                                    .state()
                                    .layers
                                    .iter()
                                    .rev()
                                    .map(|l| {
                                        (
                                            l.id,
                                            l.name.clone(),
                                            l.visible,
                                            l.locked,
                                            l.opacity,
                                            l.blend,
                                        )
                                    })
                                    .collect();

                                for (
                                    id,
                                    mut name,
                                    mut visible,
                                    mut locked,
                                    mut opacity,
                                    mut blend,
                                ) in layers
                                {
                                    ui.group(|ui| {
                                        let mut changed = false;
                                        ui.horizontal(|ui| {
                                            if ui.selectable_label(id == selected, &name).clicked()
                                            {
                                                actions.push(Action::Select(id));
                                            }
                                            changed |=
                                                ui.checkbox(&mut visible, "Visible").changed();
                                            changed |= ui.checkbox(&mut locked, "Locked").changed();
                                        });
                                        egui::CollapsingHeader::new("Layer settings")
                                            .id_salt(("layer-settings", id))
                                            .show(ui, |ui| {
                                                changed |= ui
                                                    .add(
                                                        egui::TextEdit::singleline(&mut name)
                                                            .char_limit(64)
                                                            .hint_text("Layer name"),
                                                    )
                                                    .changed();
                                                let mut opacity_percent =
                                                    f32::from(opacity) * 100.0 / 255.0;
                                                if ui
                                                    .add(
                                                        egui::Slider::new(
                                                            &mut opacity_percent,
                                                            0.0..=100.0,
                                                        )
                                                        .suffix("%")
                                                        .text("Opacity"),
                                                    )
                                                    .changed()
                                                {
                                                    opacity =
                                                        (opacity_percent * 2.55).round() as u8;
                                                    changed = true;
                                                }
                                                let old = blend;
                                                egui::ComboBox::from_id_salt(("blend", id))
                                                    .selected_text(format!("{blend:?}"))
                                                    .show_ui(ui, |ui| {
                                                        for b in [
                                                            Blend::Normal,
                                                            Blend::Multiply,
                                                            Blend::Screen,
                                                            Blend::Overlay,
                                                        ] {
                                                            ui.selectable_value(
                                                                &mut blend,
                                                                b,
                                                                format!("{b:?}"),
                                                            );
                                                        }
                                                    });
                                                changed |= old != blend;
                                            });
                                        if changed {
                                            actions.push(Action::Layer(
                                                id, name, visible, locked, opacity, blend,
                                            ));
                                        }
                                    });
                                }

                                ui.separator();
                                ui.label("Ctrl-Z / Ctrl-Y: history");
                            }
                        });
                    });
                let panel_width = side_panel.response.rect.width();
                if (panel_width - self.brush_panel_width).abs() > 0.5 {
                    self.brush_panel_width = panel_width.clamp(180.0, 640.0);
                    workspace_changed = true;
                }
            }

            if self.layers_flyout_open {
                let mut open = self.layers_flyout_open;
                let layers_position = crate::studio_layout::flyout_position(
                    screen,
                    palette_response.response.rect,
                    egui::vec2(self.layers_flyout_size[0], self.layers_flyout_size[1]),
                    egui::pos2(
                        self.layers_flyout_position[0],
                        self.layers_flyout_position[1],
                    ),
                );
                let panel = egui::Window::new("Layers")
                    .frame(crate::ui_tokens::floating_panel_frame(&ctx.style().visuals))
                    .open(&mut open)
                    .resizable(true)
                    .default_size(egui::vec2(
                        self.layers_flyout_size[0],
                        self.layers_flyout_size[1],
                    ))
                    .max_size(egui::vec2(
                        (screen.width() - 16.0).max(1.0),
                        (screen.height() - 16.0).max(1.0),
                    ))
                    .default_pos(layers_position)
                    .constrain_to(screen.shrink(8.0))
                    .show(ctx, |ui| {
                        layer_flyout_contents(ui, session.live_mut().project_mut(), actions);
                    });
                if let Some(panel) = panel {
                    let rect = panel.response.rect;
                    let position = [rect.left(), rect.top()];
                    let size = [rect.width(), rect.height()];
                    if position != self.layers_flyout_position || size != self.layers_flyout_size {
                        self.layers_flyout_position = position;
                        self.layers_flyout_size = size;
                        workspace_changed = true;
                    }
                }
                self.layers_flyout_open = open;
            }

            if self.brush_flyout_open {
                let mut open = self.brush_flyout_open;
                let brush_size = crate::studio_layout::flyout_size(
                    screen,
                    palette_response.response.rect,
                    egui::vec2(620.0, 560.0),
                );
                let brush_position = crate::studio_layout::flyout_position(
                    screen,
                    palette_response.response.rect,
                    brush_size,
                    egui::pos2(self.brush_flyout_position[0], self.brush_flyout_position[1]),
                );
                let brush_area = egui::AreaState::load(ctx, egui::Id::new("Brush library"));
                let brush_previous_rect = brush_area.map(|state| state.rect());
                let brush_needs_reanchor = brush_area.is_some_and(|state| {
                    let current_rect = state.rect();
                    !screen.contains_rect(current_rect)
                        || current_rect.intersects(palette_response.response.rect)
                });
                let panel = egui::Window::new("Brush library")
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
                    panel.current_pos(brush_position)
                } else {
                    panel
                }
                    .constrain_to(screen.shrink(8.0))
                    .show(ctx, |ui| {
                        ui.set_width((brush_size.x - 24.0).max(160.0));
                        egui::ScrollArea::vertical()
                            .auto_shrink([true, false])
                            // Account for the floating window title and frame;
                            // an unconstrained content viewport can make egui
                            // exceed max_size and slide the panel over the tools.
                            .max_height((brush_size.y - 112.0).max(80.0))
                            .show(ui, |ui| {
                        let project = session.live_mut().project_mut();
                        ui.horizontal(|ui| {
                            ui.heading("Brushes");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.weak(format!("{} presets", 10));
                            });
                        });
                        ui.weak("Choose a tool. Pressure is preserved; previews show each preset’s character.");
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
                            ui.horizontal(|ui| {
                                let color = crate::appearance::readable_text_color(
                                    ui.visuals().text_color(),
                                    ui.visuals().window_fill(),
                                );
                                ui.colored_label(color, egui::RichText::new(group).strong());
                                ui.separator();
                            });
                            if screen.width() < 760.0 {
                                for kind in filtered {
                                    ui.horizontal(|ui| {
                                        let selected = project.brush.kind == kind;
                                        let card_width = (ui.available_width() - 44.0).clamp(120.0, 440.0);
                                        if brush_preset_card(
                                            ui,
                                            kind,
                                            selected,
                                            project.brush.color,
                                            card_width,
                                        ) {
                                            let color = project.brush.color;
                                            project.brush = Brush::preset(kind);
                                            project.brush.color = color;
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
                                ui.horizontal_wrapped(|ui| {
                                    for kind in filtered {
                                        ui.vertical(|ui| {
                                            let selected = project.brush.kind == kind;
                                            if brush_preset_card(ui, kind, selected, project.brush.color, 148.0) {
                                                let color = project.brush.color;
                                                project.brush = Brush::preset(kind);
                                                project.brush.color = color;
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
                        if visible_count == 0 {
                            ui.add_space(8.0);
                            ui.weak(if self.favorites_only {
                                "No matching favorites yet. Add a brush with the star button."
                            } else {
                                "No brushes match this search."
                            });
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
                        ui.add(egui::Slider::new(&mut project.brush.size, 1.0..=256.0).text("Size"));
                        ui.add(egui::Slider::new(&mut project.brush.opacity, 0.0..=1.0).text("Opacity"));
                        egui::CollapsingHeader::new("Flow and dynamics").show(ui, |ui| {
                            ui.add(egui::Slider::new(&mut project.brush.flow, 0.01..=1.0).text("Flow"));
                            ui.add(egui::Slider::new(&mut project.brush.hardness, 0.0..=1.0).text("Hardness"));
                            ui.add(egui::Slider::new(&mut project.brush.spacing, 0.02..=1.0).text("Spacing"));
                            ui.add(egui::Slider::new(&mut project.brush.smoothing, 0.0..=0.95).text("Smoothing"));
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
                    if self.test_path_dialog
                        && screen.width() < 700.0
                        && !self.test_brush_layout_logged
                    {
                        self.test_brush_layout_logged = true;
                        eprintln!(
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
            if self.color_flyout_open {
                let mut open = self.color_flyout_open;
                let color_position = crate::studio_layout::flyout_position(
                    screen,
                    palette_response.response.rect,
                    egui::vec2(280.0, 340.0),
                    egui::pos2(self.color_flyout_position[0], self.color_flyout_position[1]),
                );
                let panel = egui::Window::new("Color controls")
                    .frame(crate::ui_tokens::floating_panel_frame(&ctx.style().visuals))
                    .open(&mut open)
                    .resizable(true)
                    .default_size(egui::vec2(320.0, 390.0))
                    .max_size(egui::vec2(
                        (screen.width() - 16.0).max(1.0),
                        (screen.height() - 16.0).max(1.0),
                    ))
                    .default_pos(color_position)
                    .constrain_to(screen.shrink(8.0))
                    .show(ctx, |ui| {
                        let project = session.live_mut().project_mut();
                        ui.horizontal(|ui| {
                            ui.heading("Color");
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
                                let _ =
                                    color_swatch(ui, previous, "Previous brush color", false, 42.0);
                                ui.weak("Previous");
                            }
                        });
                        ui.color_edit_button_srgb(&mut project.brush.color);
                        let mut hsv = crate::color_controls::rgb_to_hsv(project.brush.color);
                        if hsv[1] > 0.01 && hsv[2] > 0.01 {
                            self.color_hue = hsv[0];
                        }
                        let mut hue_degrees = self.color_hue * 360.0;
                        ui.label("Pick a shade");
                        let hue_changed = ui
                            .add(
                                egui::Slider::new(&mut hue_degrees, 0.0..=360.0)
                                    .suffix("°")
                                    .text("Hue"),
                            )
                            .changed();
                        let hue = (hue_degrees / 360.0).rem_euclid(1.0);
                        if let Some([saturation, value]) =
                            saturation_value_field(ui, hue, hsv[1], hsv[2])
                        {
                            hsv[1] = saturation;
                            hsv[2] = value;
                            self.color_hue = hue;
                            project.brush.color = crate::color_controls::hsv_to_rgb(hsv);
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
                                    self.status = "Enter a six-digit color such as #C55CFF".into();
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
                });

            let screen_viewport = ctx.content_rect();
            for (index, reference) in self.references.iter_mut().enumerate() {
                let title = reference
                    .layout
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Reference");
                let reference_id = reference.layout.path.clone();
                let id = egui::Id::new(("reference-board", reference_id));
                let mut open = true;
                let mut remove_requested = false;
                let mut center_requested = false;
                let mut reset_position_requested = false;
                let before = reference.layout.clone();
                let window_size = [
                    before.size[0].min(screen_viewport.width().max(1.0)),
                    before.size[1].min(screen_viewport.height().max(1.0)),
                ];
                let desired_pos =
                    clamp_reference_position(before.position, window_size, screen_viewport);
                let area = egui::AreaState::load(ctx, id);
                let current_pos = area.map(|state| state.left_top_pos());
                let correction = current_pos.and_then(|current| {
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
                });
                let mut window = egui::Window::new(title)
                    .id(id)
                    .open(&mut open)
                    .movable(!before.position_locked)
                    .resizable(true)
                    .max_size(screen_viewport.size())
                    .default_size(egui::vec2(window_size[0], window_size[1]));
                if let Some(pos) = correction {
                    window = window.current_pos(pos);
                } else {
                    window = window.default_pos(desired_pos);
                }
                let response = window.show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let zoom_out = ui
                            .add_sized([44.0, 40.0], egui::Button::new("−"))
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
                            .add_sized([44.0, 40.0], egui::Button::new("+"))
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
                        ui.menu_button("Reference controls", |ui| {
                            if ui.button("Fit image to window").clicked() {
                                reference.layout.view_zoom = 1.0;
                                reference.layout.view_pan = [0.0, 0.0];
                                ui.close();
                            }
                            if ui.button("Reset image view").clicked() {
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
                    });
                    let available = ui.available_size().max(egui::vec2(48.0, 48.0));
                    let source_size = reference.texture.size_vec2();
                    let (viewport, image_response) =
                        ui.allocate_exact_size(available, egui::Sense::drag());
                    image_response.clone().on_hover_text(format!(
                        "Original image: {} × {} pixels",
                        reference.source_size[0], reference.source_size[1]
                    ));
                    reference.viewport = Some(viewport);
                    let reference_visible_workspace =
                        !self.welcome && !self.new_dialog && !self.recovery_manager;
                    if self.test_path_dialog
                        && reference_visible_workspace
                        && !self.test_reference_preview_logged
                    {
                        eprintln!(
                            "UI_TEST_REFERENCE_IMAGE_VIEWPORT: {:.1},{:.1} {:.1}x{:.1} screen {:.1}x{:.1}",
                            viewport.left(), viewport.top(), viewport.width(), viewport.height(),
                            screen_viewport.width(), screen_viewport.height()
                        );
                    }
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
                    if let Some(image_rect) = reference_image_rect(
                        viewport,
                        source_size,
                        reference.layout.view_zoom,
                        reference.layout.view_pan,
                    ) {
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
                            eprintln!(
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
                    reference.layout.position = [next_position.x, next_position.y];
                    reference.layout.size = [rect.width(), rect.height()];
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
                    // Home is a full-workspace destination. Paint the complete
                    // viewport so the GPU canvas cannot show through beneath a
                    // short or scrolled Home layout after a large resize.
                    ui.painter().rect_filled(
                        screen,
                        0.0,
                        ctx.style().visuals.panel_fill,
                    );
                    egui::Frame::new()
                        .fill(ctx.style().visuals.panel_fill)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                ui.add_space(if screen.height() < 620.0 { 10.0 } else { 18.0 });
                                let hero_margin = if screen.width() < 720.0 { 14 } else { 30 };
                                egui::Frame::new()
                                    .fill(ui.visuals().window_fill)
                                    .stroke(ui.visuals().window_stroke)
                                    .corner_radius(22.0)
                                    .inner_margin(egui::Margin::same(hero_margin))
                                    .show(ui, |ui| {
                                        ui.set_max_height(home_hero_height(screen.width()));
                                        ui.set_min_width((screen.width() - side_pad * 2.0).max(240.0));
                                        ui.horizontal(|ui| {
                                            crate::ui_tokens::brand_wordmark(ui, 31.0);
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                ui.weak("PAINT  ·  DRAW  ·  MAKE IT YOURS");
                                            });
                                        });
                                        ui.add_space(if screen.width() < 760.0 { 8.0 } else { 18.0 });
                                        let compact = screen.width() < 760.0;
                                        ui.with_layout(
                                            if compact { egui::Layout::top_down(egui::Align::Min) } else { egui::Layout::left_to_right(egui::Align::Center) },
                                            |ui| {
                                                ui.vertical(|ui| {
                                                    let surface = ui.visuals().window_fill;
                                                    let foreground = crate::appearance::readable_text_color(
                                                        ui.visuals().text_color(), surface,
                                                    );
                                                    let muted = crate::appearance::readable_text_color(
                                                        ui.visuals().weak_text_color(), surface,
                                                    );
                                                    let dark_mode = ui.visuals().dark_mode;
                                                    let contrast = crate::appearance::contrast_ratio(foreground, surface);
                                                    if self.test_path_dialog && contrast >= 4.5 {
                                                        let logged = if dark_mode {
                                                            &mut self.test_home_dark_contrast_logged
                                                        } else {
                                                            &mut self.test_home_light_contrast_logged
                                                        };
                                                        if !*logged {
                                                            *logged = true;
                                                            eprintln!("UI_TEST_HOME_HEADLINE_CONTRAST: {} {:.2}", if dark_mode { "dark" } else { "light" }, contrast);
                                                        }
                                                    }
                                                    let accent = crate::appearance::readable_text_accent(ui.visuals().hyperlink_color, surface);
                                                    ui.label(egui::RichText::new("No limits. No bullshit.")
                                                        .size(16.0).strong().color(accent));
                                                    ui.add_space(4.0);
                                                    ui.label(egui::RichText::new("Create without permission.")
                                                        .size((screen.width() * 0.042).clamp(32.0, 64.0))
                                                        .strong().color(foreground));
                                                    ui.add_space(5.0);
                                                    ui.label(egui::RichText::new("A painting studio for ideas that do not fit inside the lines.")
                                                        .size(16.0).color(muted));
                                                    ui.add_space(if compact { 8.0 } else { 12.0 });
                                                    ui.horizontal_wrapped(|ui| {
                                                        let action_width = (screen.width() * 0.42).clamp(168.0, 232.0);
                                                        let action_height = if compact { 48.0 } else { 54.0 };
                                                        let accent_fill = ui.visuals().selection.bg_fill;
                                                        let action_text = crate::appearance::readable_text_color(
                                                            ui.visuals().text_color(),
                                                            accent_fill,
                                                        );
                                                        let primary = egui::Button::new(egui::RichText::new("New artwork").size(16.0).strong().color(action_text))
                                                            .fill(accent_fill)
                                                            .min_size(egui::vec2(action_width, action_height));
                                                        if ui.add(primary).clicked() {
                                                            self.picker_from_welcome = false;
                                                            if !self.test_path_dialog { self.new_name = "Untitled".into(); }
                                                            self.new_from_welcome = true;
                                                            self.new_dialog = true;
                                                            self.welcome = false;
                                                        }
                                                        if ui.add_sized([action_width, action_height], egui::Button::new("Open a project…")).clicked() {
                                                            self.picker_from_welcome = true;
                                                            self.picker = Some(FileRequest::OpenProject);
                                                            self.welcome = false;
                                                        }
                                                    });
                                                    if !compact {
                                                        ui.add_space(5.0);
                                                        ui.label(egui::RichText::new("Your work stays on this device.")
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
                                                        eprintln!("UI_TEST_HOME_RECENT_PREVIEW_VISIBLE");
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
                                                                if home_document_card(
                                                                    ui,
                                                                    name,
                                                                    [width, height],
                                                                    card_width,
                                                                ) {
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
                                                            eprintln!("UI_TEST_HOME_QUICK_START_VISIBLE");
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
                                        ui.heading("Start with a canvas");
                                        ui.add_space(8.0);
                                        ui.weak("Choose a format, then fine-tune it.");
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
                                            if home_document_card(
                                                ui,
                                                name,
                                                [width, height],
                                                card_width,
                                            ) {
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
                                        ui.heading("Recent artwork");
                                        ui.label("Projects are ordered by most recently opened.");
                                        ui.add_space(8.0);
                                        if self.recent.paths.is_empty() {
                                            crate::ui_tokens::card_frame(ui.visuals(), false).inner_margin(egui::Margin::same(22)).show(ui, |ui| {
                                                ui.set_min_width((screen.width() - 100.0).max(300.0));
                                                ui.label(egui::RichText::new("Nothing recent yet").size(19.0).strong());
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
                                            let (columns, card_width) =
                                                recent_gallery_layout(ui.available_width());
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
                                                                egui::Frame::new().fill(ui.visuals().extreme_bg_color).corner_radius(8.0).inner_margin(4.0).show(ui, |ui| {
                                                                    ui.set_min_size(egui::vec2(card_width - 36.0, preview_height));
                                                                    ui.centered_and_justified(|ui| {
                                                                        ui.add(egui::Image::new((texture.id(), image_size)).fit_to_exact_size(image_size));
                                                                    });
                                                                });
                                                            } else {
                                                                egui::Frame::new().fill(ui.visuals().extreme_bg_color).corner_radius(6.0).show(ui, |ui| {
                                                                    ui.set_min_size(egui::vec2(card_width - 30.0, (card_width * 0.62).clamp(140.0, 220.0)));
                                                                    ui.centered_and_justified(|ui| { ui.weak("Artwork preview"); });
                                                                });
                                                            }
                                                            ui.add_space(4.0);
                                                            ui.label(egui::RichText::new(name).strong().size(17.0));
                                                            ui.label(egui::RichText::new(details).size(13.0).color(ui.visuals().weak_text_color()));
                                                            if !available { ui.colored_label(ui.visuals().error_fg_color, "Missing project"); }
                                                        });
                                                    });
                                                    let visible_card = card.response.rect.intersect(ui.clip_rect());
                                                    if self.test_path_dialog
                                                        && screen.width() >= 1800.0
                                                    {
                                                        eprintln!(
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
                                                    if (index + 1) % columns == 0 || index == visible_recent.len().min(8) - 1 { ui.end_row(); }
                                                }
                                            });
                                            }
                                        }
                                    });
                                    ui.add_space(24.0);
                                    ui.vertical(|ui| {
                                        ui.heading("Continue safely");
                                        ui.label("Recovery and unnamed work stay separate from exported artwork.");
                                        if !self.recovery_files.is_empty() {
                                            ui.add_space(8.0);
                                            egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(10.0).inner_margin(12.0).show(ui, |ui| {
                                                ui.label(egui::RichText::new("Recoverable work").strong());
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
                self.new_from_welcome = true;
                self.new_dialog = true;
                self.welcome = false;
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
            egui::Modal::new(egui::Id::new("recovery manager")).show(ctx, |ui| {
                ui.set_min_width(520.0);
                ui.heading("Artwork recovery manager");
                ui.label("These local checkpoints are separate from saved projects. Recover opens a copy as a new unsaved document.");
                if self.recovery_files.is_empty() {
                    ui.label("No inactive recovery checkpoints are available.");
                } else {
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        for (index, path) in self.recovery_files.clone().into_iter().enumerate() {
                            let name = recovery_checkpoint_label(&path, index);
                            let recovery_surface = ui.visuals().faint_bg_color;
                            let recovery_label_color = crate::appearance::readable_text_color(
                                ui.visuals().text_color(),
                                recovery_surface,
                            );
                            if self.test_path_dialog && !self.test_recovery_label_logged {
                                self.test_recovery_label_logged = true;
                                eprintln!("UI_TEST_RECOVERY_LABEL: {name}");
                                if index == 0 {
                                let contrast = crate::appearance::contrast_ratio(
                                    recovery_label_color,
                                    recovery_surface,
                                );
                                eprintln!(
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
                                                eprintln!("UI_TEST_RECOVERY_PREVIEW_VISIBLE");
                                                eprintln!(
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
                                            if self.test_path_dialog
                                                && index == 0
                                                && !self.test_recovery_preview_target_logged
                                            {
                                                self.test_recovery_preview_target_logged = true;
                                                eprintln!(
                                                    "UI_TEST_RECOVERY_PREVIEW_TARGET: {:.1} {:.1} {:.1} {:.1} {:.2} {:.1}",
                                                    response.rect.left(),
                                                    response.rect.top(),
                                                    response.rect.width(),
                                                    response.rect.height(),
                                                    ctx.pixels_per_point(),
                                                    ctx.content_rect().min.y,
                                                );
                                            }
                                            if self.test_path_dialog
                                                && index == 0
                                                && response.hovered()
                                                && !self.test_recovery_preview_hover_logged
                                            {
                                                self.test_recovery_preview_hover_logged = true;
                                                eprintln!("UI_TEST_RECOVERY_PREVIEW_HOVERED");
                                            }
                                            if response.clicked() {
                                                let texture =
                                                    crate::recovery_previews::load(&path, ctx);
                                                self.recovery_preview = Some((path.clone(), texture));
                                            }
                                        }
                                        ui.vertical(|ui| {
                                            ui.label(
                                                egui::RichText::new(name)
                                                    .strong()
                                                    .color(recovery_label_color),
                                            );
                                            ui.label(source_label);
                                            ui.label(dimensions);
                                            ui.label(updated);
                                            ui.label(bytes.map(|size| format!("{} KiB", size.div_ceil(1024)))
                                                .unwrap_or_else(|| "Recovery data unavailable".into()));
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
            });
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
                eprintln!("UI_TEST_NEW_ARTWORK_VISIBLE");
            }
            egui::Modal::new(egui::Id::new("new document")).show(ctx, |ui| {
                let dialog_width = (ctx.content_rect().width() - 32.0).clamp(280.0, 880.0);
                ui.set_min_width(dialog_width.min(440.0));
                ui.set_max_width(dialog_width);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .min_scrolled_height((ctx.content_rect().height() - 64.0).clamp(180.0, 560.0))
                    .max_height((ctx.content_rect().height() - 32.0).max(120.0))
                    .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading("Create a new canvas");
                    let can_create = self
                        .new_unit
                        .to_pixels(self.new_width, self.new_dpi)
                        .is_some()
                        && self
                            .new_unit
                            .to_pixels(self.new_height, self.new_dpi)
                            .is_some();
                    let button_text = crate::appearance::readable_text_color(
                        ui.visuals().text_color(),
                        ui.visuals().selection.bg_fill,
                    );
                    let button = egui::Button::new(
                        egui::RichText::new("Create canvas")
                            .strong()
                            .color(button_text),
                    )
                        .fill(ui.visuals().selection.bg_fill)
                        .min_size(egui::vec2(156.0, 44.0));
                    if ui.add_enabled(can_create, button).clicked() {
                        actions.push(Action::New);
                        self.new_dialog = false;
                        self.welcome = false;
                    }
                });
                ui.label("Start with a paper shape, then fine-tune the document size.");
                ui.add_space(8.0);
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
                        if document_format_card(
                            ui,
                            label,
                            width as f32,
                            height as f32,
                            dpi,
                            selected,
                            card_width,
                        ) {
                            self.new_unit = crate::document_setup::DocumentUnit::Pixels;
                            self.new_width = f64::from(width);
                            self.new_height = f64::from(height);
                            self.new_dpi = dpi;
                            self.new_custom_size_selected = false;
                        }
                    }
                    let custom_width = current_width_px.unwrap_or(1080);
                    let custom_height = current_height_px.unwrap_or(1080);
                    if document_format_card(
                        ui,
                        "Custom",
                        custom_width as f32,
                        custom_height as f32,
                        self.new_dpi,
                        self.new_custom_size_selected || !current_matches_preset,
                        card_width,
                    ) {
                        self.new_custom_size_selected = true;
                    }
                });
                let preview_width: f32 = 128.0;
                let aspect = (self.new_height / self.new_width).clamp(0.25, 4.0) as f32;
                let preview_size = [preview_width.min(128.0 / aspect), (preview_width * aspect).min(128.0)];
                ui.horizontal_wrapped(|ui| {
                    let (preview, _) = ui.allocate_exact_size(
                        egui::vec2(preview_size[0], preview_size[1]),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter();
                    if self.new_transparent {
                        let cell = 8.0;
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
                    ui.vertical(|ui| {
                        let preview_heading = crate::appearance::readable_text_color(
                            ui.visuals().text_color(),
                            ui.visuals().window_fill,
                        );
                        ui.label(egui::RichText::new("Canvas preview").strong().color(preview_heading));
                        if let (Some(width), Some(height)) = (
                            self.new_unit.to_pixels(self.new_width, self.new_dpi),
                            self.new_unit.to_pixels(self.new_height, self.new_dpi),
                        ) {
                            ui.label(format!("{width} × {height} px"));
                        }
                        ui.label(if self.new_transparent { "Transparent background" } else { "Solid background" });
                        ui.label(format!("{} DPI", self.new_dpi));
                    });
                });
                ui.add_sized(
                    [(dialog_width - 40.0).clamp(190.0, 380.0), 40.0],
                    egui::TextEdit::singleline(&mut self.new_name).hint_text("Document name"),
                );
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
                if ui.add_enabled(can_create, create_button).clicked()
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
                    if ui.button("Cancel").clicked() {
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
        let raw = self.input.take_egui_input(window);
        let ctx = self.context.clone();
        let mut actions = Vec::new();
        if self.test_path_dialog && self.manual_file_dialog && self.manual_file_submit {
            self.manual_file_submit = false;
            self.manual_file_dialog = false;
            if let Some(action) = self.manual_file_action.take() {
                eprintln!("TEST_PATH_DIALOG: queued action for deterministic submission");
                actions.push(action);
            }
        }
        let mut central = egui::Rect::NOTHING;
        let mut approved = false;
        let mut output = ctx.run(raw, |ctx| {
            self.widgets(ctx, session, &mut actions, &mut central, &mut approved)
        });

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
                match add_reference_path(&ctx, &mut self.references, path) {
                    Ok(()) => reference_imported += 1,
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
                for (width, color) in [(3.0, egui::Color32::BLACK), (1.0, egui::Color32::WHITE)] {
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
        if self.tool == EditTool::Brush
            && session.mode() == crate::interaction::Mode::Live
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
                    Ok(()) => self.status = "Recovery checkpoint complete".into(),
                    Err(e) => {
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
                } else {
                    self.autosave_schedule.retry_later(now);
                }
            }
        }
        self.input
            .handle_platform_output(window, output.platform_output.clone());
        output
    }
    fn execute(&mut self, a: Action, session: &mut CanvasSession) -> Result<(), Failure> {
        let project = session.live_mut().project_mut();
        match a {
            Action::ImportOra => {
                let state = paint_document::openraster::load(std::path::Path::new(&self.path))?;
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
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
            }
            Action::Redo => {
                project.redo();
            }
            Action::Add => project.add_layer()?,
            Action::Duplicate => project.duplicate_layer()?,
            Action::Delete => project.delete_layer()?,
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
            Action::Layer(id, name, visible, locked, opacity, blend) => project.edit(|s| {
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
            })?,
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
                *project = Engine::from_state(state)?;
                self.store = None;
                self.path = format!(
                    "{}.rpl",
                    crate::document_setup::sanitized_document_stem(&self.new_name)
                );
                self.last_submitted = u64::MAX;
                self.welcome = false;
                self.status = format!("Created {width} × {height} px at {} DPI", self.new_dpi);
                if let Err(error) = self.persist_workspace() {
                    self.status =
                        format!("Document created; setup preference save failed: {error}");
                }
            }
            Action::Recover(path) => {
                let (state, fallback) = paint_document::recovery::restore(&path)?;
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
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
        LassoAppend, append_lasso_point, document_format_grid, home_hero_height,
        home_sections_stacked, layer_thumbnail_samples, native_project_path,
        quick_document_card_layout, recent_artwork_preview_height, recent_gallery_layout,
        recent_project_matches, recovery_checkpoint_label, recovery_source_label,
    };
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
    fn recent_artwork_gallery_scales_from_single_column_to_four_columns() {
        for (available, expected_columns) in [(420.0, 1), (800.0, 2), (1280.0, 3), (1920.0, 4)] {
            let (columns, card_width) = recent_gallery_layout(available);
            assert_eq!(columns, expected_columns, "viewport width {available}");
            assert!((180.0..=500.0).contains(&card_width));
            let total = columns as f32 * card_width + 14.0 * (columns - 1) as f32;
            assert!(total <= available + 0.1, "cards overflow {available}px");
        }
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

    #[test]
    fn layer_previews_are_bounded_and_show_real_transparent_layer_pixels() {
        let mut tile = vec![0; paint_document::TILE_BYTES];
        for y in 16..48 {
            for x in 16..48 {
                let pixel = ((y * paint_document::TILE + x) * 4) as usize;
                tile[pixel..pixel + 4].copy_from_slice(&[120, 30, 60, 128]);
            }
        }
        let layer = paint_document::Layer {
            id: 1,
            name: "Paint 1".into(),
            visible: true,
            locked: false,
            opacity: 255,
            blend: paint_document::Blend::Normal,
            tiles: std::collections::BTreeMap::from([((0, 0), std::sync::Arc::new(tile))]),
        };
        let preview = layer_thumbnail_samples(&layer, [64, 64]);
        assert_eq!(preview.len(), 70);
        assert!(preview.iter().any(|color| color.a() > 0));
        assert!(preview.iter().any(|color| color.a() == 0));

        let empty = paint_document::Layer {
            tiles: std::collections::BTreeMap::new(),
            ..layer
        };
        assert!(
            layer_thumbnail_samples(&empty, [4096, 4096])
                .iter()
                .all(|color| color.a() == 0)
        );
    }
}
