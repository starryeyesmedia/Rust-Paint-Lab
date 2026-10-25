//! Shared interaction dimensions for the desktop studio.

pub const TOUCH_TARGET: f32 = 44.0;
pub const COMPACT_TARGET: f32 = 36.0;
pub const TOOL_TARGET: f32 = 48.0;
pub const BUTTON_RADIUS: f32 = 8.0;
pub const PANEL_RADIUS: f32 = 12.0;

const UI_FONT: &str = "BitchPixel Manrope";
const DISPLAY_FONT: &str = "BitchPixel Syne";

/// Install the locally bundled OFL type system before creating any widgets.
/// The normal proportional family keeps broad glyph fallbacks; a named Syne
/// family is reserved for display headings and the product wordmark.
fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        UI_FONT.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/Manrope-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        DISPLAY_FONT.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/Syne-Bold.ttf"
        ))),
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .expect("egui provides a proportional font family")
        .insert(0, UI_FONT.to_owned());
    fonts.families.insert(
        egui::FontFamily::Name(DISPLAY_FONT.into()),
        vec![DISPLAY_FONT.to_owned(), UI_FONT.to_owned()],
    );
    fonts
}

pub fn install_fonts(context: &egui::Context) {
    context.set_fonts(font_definitions());
}

pub fn brand_mark(ui: &mut egui::Ui, size: f32) {
    let (bounds, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    paint_brand_mark(ui.painter(), bounds, ui.visuals().window_fill);
}

fn paint_brand_mark(painter: &egui::Painter, bounds: egui::Rect, surface: egui::Color32) {
    let dark = surface.r() < 128;
    let well = if dark {
        egui::Color32::from_rgb(44, 30, 58)
    } else {
        egui::Color32::from_rgb(240, 228, 250)
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
                0 => egui::Color32::from_rgb(197, 92, 255),
                1 => egui::Color32::from_rgb(245, 142, 255),
                _ => egui::Color32::from_rgb(226, 190, 255),
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
        egui::Color32::from_rgb(197, 92, 255),
        ui.visuals().window_fill,
    );
    let display = egui::FontFamily::Name(DISPLAY_FONT.into());
    ui.horizontal(|ui| {
        brand_mark(ui, size * 1.18);
        ui.add_space(2.0);
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(
            egui::RichText::new("Bitch")
                .font(egui::FontId::new(size, display.clone()))
                .strong(),
        );
        ui.label(
            egui::RichText::new("Pixel")
                .font(egui::FontId::new(size, display))
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
    install_fonts(context);
    context.style_mut(|style| {
        style.spacing.interact_size = egui::vec2(TOUCH_TARGET, TOUCH_TARGET);
        style.spacing.button_padding = egui::vec2(10.0, 8.0);
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(26.0, egui::FontFamily::Name(DISPLAY_FONT.into())),
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
        assert_eq!(
            style.text_styles[&egui::TextStyle::Heading].family,
            egui::FontFamily::Name(DISPLAY_FONT.into())
        );
        let fonts = font_definitions();
        assert!(fonts.font_data.contains_key(UI_FONT));
        assert!(fonts.font_data.contains_key(DISPLAY_FONT));
        assert!(
            fonts
                .families
                .contains_key(&egui::FontFamily::Name(DISPLAY_FONT.into()))
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
        install(&context);
        let output = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| brand_wordmark(ui, 22.0));
        });
        assert!(output.shapes.len() >= 12);
    }
}
