//! Small original vector icons drawn as egui geometry. This avoids glyph-font
//! substitutions on Windows and keeps icons sharp at display scale factors.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Brush,
    Eraser,
    Rectangle,
    Ellipse,
    Lasso,
    Eyedropper,
    Deselect,
    Fill,
    Undo,
    Redo,
    Open,
    Save,
    Layers,
    Color,
    Settings,
}

fn paint(painter: &egui::Painter, bounds: egui::Rect, icon: Icon, color: egui::Color32) {
    use egui::{Shape, Stroke, StrokeKind, Vec2};

    let scale = (bounds.width().min(bounds.height()) / 24.0).max(0.5);
    let center = bounds.center();
    let at = |x: f32, y: f32| center + Vec2::new(x * scale, y * scale);
    let stroke = Stroke::new(1.8 * scale, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
    };
    let path = |points: &[(f32, f32)]| {
        painter.add(Shape::line(
            points.iter().map(|&(x, y)| at(x, y)).collect(),
            stroke,
        ));
    };
    let rect = |left: f32, top: f32, right: f32, bottom: f32| {
        egui::Rect::from_min_max(at(left, top), at(right, bottom))
    };

    match icon {
        Icon::Brush => {
            path(&[(-7.0, 7.0), (-4.0, 3.0), (4.0, -5.0), (7.0, -7.0)]);
            painter.add(Shape::closed_line(
                [(-8.0, 8.0), (-7.0, 4.0), (-4.0, 7.0)]
                    .into_iter()
                    .map(|(x, y)| at(x, y))
                    .collect(),
                stroke,
            ));
            line((4.0, -5.0), (6.0, -3.0));
        }
        Icon::Eraser => {
            let points = [(-7.0, 2.0), (1.0, -7.0), (8.0, -1.0), (0.0, 7.0)];
            painter.add(Shape::convex_polygon(
                points.into_iter().map(|(x, y)| at(x, y)).collect(),
                color.linear_multiply(0.16),
                stroke,
            ));
            line((-2.0, 4.0), (5.0, -3.0));
        }
        Icon::Rectangle => {
            painter.rect_stroke(
                rect(-7.0, -6.0, 7.0, 6.0),
                2.0 * scale,
                stroke,
                StrokeKind::Inside,
            );
        }
        Icon::Ellipse => {
            painter.circle_stroke(center, 7.0 * scale, stroke);
        }
        Icon::Lasso => {
            painter.add(Shape::closed_line(
                [
                    (-6.0, -3.0),
                    (-2.0, -7.0),
                    (5.0, -5.0),
                    (7.0, 1.0),
                    (3.0, 6.0),
                    (-4.0, 5.0),
                ]
                .into_iter()
                .map(|(x, y)| at(x, y))
                .collect(),
                stroke,
            ));
            line((3.0, 6.0), (6.0, 9.0));
        }
        Icon::Eyedropper => {
            line((-6.0, 6.0), (5.0, -5.0));
            line((-8.0, 8.0), (-5.0, 5.0));
            painter.circle_stroke(at(5.5, -5.5), 3.0 * scale, stroke);
            painter.circle_filled(at(-6.5, 6.5), 1.3 * scale, color);
        }
        Icon::Deselect => {
            line((-7.0, -4.0), (-7.0, -7.0));
            line((-7.0, -7.0), (-4.0, -7.0));
            line((4.0, -7.0), (7.0, -7.0));
            line((7.0, -7.0), (7.0, -4.0));
            line((7.0, 4.0), (7.0, 7.0));
            line((7.0, 7.0), (4.0, 7.0));
            line((-4.0, 7.0), (-7.0, 7.0));
            line((-7.0, 7.0), (-7.0, 4.0));
        }
        Icon::Fill => {
            painter.add(Shape::closed_line(
                [
                    (-7.0, -4.0),
                    (-2.0, -8.0),
                    (6.0, -1.0),
                    (1.0, 5.0),
                    (-5.0, 0.0),
                ]
                .into_iter()
                .map(|(x, y)| at(x, y))
                .collect(),
                stroke,
            ));
            line((-4.0, 2.0), (-7.0, 5.0));
            line((-7.0, 5.0), (1.0, 5.0));
            painter.circle_filled(at(6.0, 7.0), 1.2 * scale, color);
        }
        Icon::Undo | Icon::Redo => {
            let sign = if icon == Icon::Undo { 1.0 } else { -1.0 };
            let points = (0..=12)
                .map(|step| {
                    let angle = std::f32::consts::PI * (0.15 + step as f32 * 0.7 / 12.0);
                    (sign * angle.cos() * 6.0, angle.sin() * 5.5)
                })
                .collect::<Vec<_>>();
            path(&points);
            let (x, y) = points[0];
            let tip = (x, y);
            line(tip, (tip.0 + sign * 4.0, tip.1 + 1.0));
            line(tip, (tip.0 + sign * 1.0, tip.1 + 4.0));
        }
        Icon::Open => {
            path(&[
                (-8.0, -4.0),
                (-3.0, -4.0),
                (-1.0, -1.0),
                (8.0, -1.0),
                (6.0, 6.0),
                (-8.0, 6.0),
                (-8.0, -4.0),
            ]);
            line((-7.0, 1.0), (7.0, 1.0));
        }
        Icon::Save => {
            painter.rect_stroke(
                rect(-7.0, -8.0, 7.0, 8.0),
                1.5 * scale,
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                rect(-4.0, -7.0, 4.0, -2.0),
                scale,
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(rect(-4.0, 2.0, 4.0, 7.0), scale, stroke, StrokeKind::Inside);
        }
        Icon::Layers => {
            for offset in [-3.0, 0.0, 3.0] {
                painter.rect_stroke(
                    rect(-6.0 + offset, -5.0 - offset, 6.0 + offset, 1.0 - offset),
                    1.0 * scale,
                    stroke,
                    StrokeKind::Inside,
                );
            }
        }
        Icon::Color => {
            painter.circle_stroke(center, 7.0 * scale, stroke);
            for (x, y) in [(-2.5, -2.0), (3.0, -2.5), (-2.0, 3.0), (3.0, 3.0)] {
                painter.circle_filled(at(x, y), 1.0 * scale, color);
            }
        }
        Icon::Settings => {
            painter.circle_stroke(center, 5.5 * scale, stroke);
            painter.circle_stroke(center, 2.2 * scale, stroke);
            for index in 0..8 {
                let angle = index as f32 * std::f32::consts::TAU / 8.0;
                let (sin, cos) = angle.sin_cos();
                line((sin * 6.0, cos * 6.0), (sin * 8.0, cos * 8.0));
            }
        }
    }
}

/// Draw an icon-only button while retaining its human-readable button label for
/// hover, focus, and accessibility information.
pub fn button(
    ui: &mut egui::Ui,
    icon: Icon,
    name: &str,
    shortcut: Option<&str>,
    selected: bool,
    size: [f32; 2],
) -> egui::Response {
    let hidden_label = egui::RichText::new(name)
        .size(1.0)
        .color(egui::Color32::TRANSPARENT);
    let response = ui.add_sized(size, egui::Button::new(hidden_label).selected(selected));
    let color = if selected {
        ui.visuals().selection.stroke.color
    } else if response.hovered() {
        ui.visuals().widgets.hovered.fg_stroke.color
    } else {
        ui.visuals().text_color()
    };
    paint(ui.painter(), response.rect.shrink(9.0), icon, color);
    let tooltip = shortcut.map_or_else(|| name.to_string(), |key| format!("{name} ({key})"));
    response.on_hover_text(tooltip)
}

/// Add a vector mark to a normal text button. The text remains the accessible
/// button label and the complete row remains clickable.
pub fn menu_button(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    menu_button_enabled(ui, icon, label, true)
}

pub fn menu_button_enabled(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
) -> egui::Response {
    let response = ui.add_enabled(enabled, egui::Button::new(format!("    {label}")));
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(response.rect.left() + 14.0, response.rect.center().y),
        egui::vec2(17.0, 17.0),
    );
    let color = if response.hovered() {
        ui.visuals().widgets.hovered.fg_stroke.color
    } else {
        ui.visuals().text_color()
    };
    paint(ui.painter(), icon_rect, icon, color);
    response
}

pub fn color_swatch(ui: &mut egui::Ui, color: egui::Color32, label: &str) -> egui::Response {
    let semantic_label = egui::RichText::new(label)
        .size(1.0)
        .color(egui::Color32::TRANSPARENT);
    let response = ui.add_sized([32.0, 32.0], egui::Button::new(semantic_label).fill(color));
    let border = if response.hovered() || response.has_focus() {
        ui.visuals().selection.stroke
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    ui.painter().rect_stroke(
        response.rect.shrink(1.0),
        5.0,
        border,
        egui::StrokeKind::Inside,
    );
    response.on_hover_text(label)
}

pub fn label(ui: &mut egui::Ui, icon: Icon, name: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
    paint(
        ui.painter(),
        rect.shrink(2.0),
        icon,
        ui.visuals().text_color(),
    );
    ui.label(name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_current_studio_icon_emits_vector_geometry_in_egui() {
        let icons = [
            Icon::Brush,
            Icon::Eraser,
            Icon::Rectangle,
            Icon::Ellipse,
            Icon::Lasso,
            Icon::Eyedropper,
            Icon::Deselect,
            Icon::Fill,
            Icon::Undo,
            Icon::Redo,
            Icon::Open,
            Icon::Save,
            Icon::Layers,
            Icon::Color,
            Icon::Settings,
        ];
        let context = egui::Context::default();
        let output = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                for icon in icons {
                    button(ui, icon, "Tool name", Some("Shortcut"), false, [36.0, 36.0]);
                }
            });
        });
        assert!(output.shapes.len() >= icons.len() * 2);
    }

    #[test]
    fn recent_color_swatch_has_a_touch_sized_named_button() {
        let context = egui::Context::default();
        let mut width = 0.0;
        let output = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                let response = color_swatch(
                    ui,
                    egui::Color32::from_rgb(160, 80, 210),
                    "Brush color #A050D2",
                );
                width = response.rect.width();
                assert!(response.enabled());
            });
        });
        assert!(width >= 32.0);
        assert!(!output.shapes.is_empty());
    }
}
