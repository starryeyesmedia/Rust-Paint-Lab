//! Shared interaction dimensions for the desktop studio.

pub const TOUCH_TARGET: f32 = 44.0;
pub const COMPACT_TARGET: f32 = 36.0;
pub const TOOL_TARGET: f32 = 48.0;
pub const BUTTON_RADIUS: f32 = 8.0;
pub const PANEL_RADIUS: f32 = 12.0;

pub fn brand_mark(ui: &mut egui::Ui, size: f32) {
    let (bounds, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    paint_brand_mark(ui.painter(), bounds, ui.visuals().window_fill);
}

fn paint_brand_mark(painter: &egui::Painter, bounds: egui::Rect, surface: egui::Color32) {
    let dark = surface.r() < 128;
    let well = if dark {
        egui::Color32::from_rgb(52, 48, 70)
    } else {
        egui::Color32::from_rgb(238, 232, 248)
    };
    painter.rect_filled(bounds, bounds.width() * 0.27, well);
    let cell = bounds.width() * 0.115;
    let gap = cell * 0.29;
    let grid = [
        [false, true, true, false],
        [true, false, false, true],
        [true, true, true, false],
        [true, false, false, true],
        [false, true, true, false],
    ];
    let width = cell * 4.0 + gap * 3.0;
    let height = cell * 5.0 + gap * 4.0;
    let origin = bounds.center() - egui::vec2(width, height) * 0.5;
    for (row, cells) in grid.iter().enumerate() {
        for (column, active) in cells.iter().enumerate() {
            if !active {
                continue;
            }
            let color = match (row + column) % 3 {
                0 => egui::Color32::from_rgb(190, 157, 255),
                1 => egui::Color32::from_rgb(255, 132, 173),
                _ => egui::Color32::from_rgb(112, 218, 208),
            };
            let minimum =
                origin + egui::vec2(column as f32 * (cell + gap), row as f32 * (cell + gap));
            painter.rect_filled(
                egui::Rect::from_min_size(minimum, egui::vec2(cell, cell)),
                cell * 0.22,
                color,
            );
        }
    }
}

pub fn brand_wordmark(ui: &mut egui::Ui, size: f32) {
    let accent = crate::appearance::readable_text_accent(
        egui::Color32::from_rgb(190, 157, 255),
        ui.visuals().window_fill,
    );
    ui.horizontal(|ui| {
        brand_mark(ui, size * 1.18);
        ui.add_space(2.0);
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(egui::RichText::new("Bitch").size(size).strong());
        ui.label(
            egui::RichText::new("Pixel")
                .size(size)
                .strong()
                .color(accent),
        );
    });
}

pub fn install_motion_preference(context: &egui::Context, reduced_motion: bool) {
    context.style_mut(|style| {
        style.animation_time = if reduced_motion { 0.0 } else { 0.1 };
    });
}

pub fn install(context: &egui::Context) {
    context.style_mut(|style| {
        style.spacing.interact_size = egui::vec2(TOUCH_TARGET, TOUCH_TARGET);
        style.spacing.button_padding = egui::vec2(10.0, 8.0);
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(26.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(14.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(14.0, egui::FontFamily::Proportional),
        );
        for widget in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.corner_radius = BUTTON_RADIUS.into();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_style_uses_touch_sized_default_controls_and_focusable_rounding() {
        let context = egui::Context::default();
        install(&context);
        let style = context.style();
        assert_eq!(
            style.spacing.interact_size,
            egui::vec2(TOUCH_TARGET, TOUCH_TARGET)
        );
        assert_eq!(
            style.visuals.widgets.inactive.corner_radius,
            BUTTON_RADIUS.into()
        );
    }

    #[test]
    fn reduced_motion_disables_interface_transition_animation() {
        let context = egui::Context::default();
        install_motion_preference(&context, true);
        assert_eq!(context.style().animation_time, 0.0);
        install_motion_preference(&context, false);
        assert_eq!(context.style().animation_time, 0.1);
    }

    #[test]
    fn brand_mark_paints_a_distinctive_accessible_geometry() {
        let context = egui::Context::default();
        let output = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| brand_wordmark(ui, 22.0));
        });
        assert!(output.shapes.len() >= 12);
    }
}
