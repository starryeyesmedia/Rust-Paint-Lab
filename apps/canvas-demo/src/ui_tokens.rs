//! Shared interaction dimensions for the desktop studio.

pub const TOUCH_TARGET: f32 = 44.0;
pub const COMPACT_TARGET: f32 = 36.0;
pub const TOOL_TARGET: f32 = 48.0;
pub const BUTTON_RADIUS: f32 = 8.0;
pub const PANEL_RADIUS: f32 = 12.0;

const UI_FONT: &str = "BitchPixel Manrope";
const DISPLAY_FONT: &str = "BitchPixel Syne";

fn mix_color(a: egui::Color32, b: egui::Color32, amount: f32) -> egui::Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |left: u8, right: u8| {
        (f32::from(left) + (f32::from(right) - f32::from(left)) * amount).round() as u8
    };
    egui::Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

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

pub fn brand_wordmark(ui: &mut egui::Ui, size: f32) {
    let surface = ui.visuals().window_fill;
    let foreground = crate::appearance::readable_text_color(ui.visuals().text_color(), surface);
    let accent =
        crate::appearance::readable_text_accent(egui::Color32::from_rgb(197, 92, 255), surface);
    let display = egui::FontFamily::Name(DISPLAY_FONT.into());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(
            egui::RichText::new("Bitch")
                .font(egui::FontId::new(size, display.clone()))
                .strong()
                .color(foreground),
        );
        ui.label(
            egui::RichText::new("Pixel")
                .font(egui::FontId::new(size, display))
                .strong()
                .color(accent),
        );
    });
}

/// A display face for the brand headline and intentionally prominent actions.
/// The bundled family is followed by the UI family for broad glyph coverage.
pub fn display_text(text: &str, size: f32, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(text)
        .font(egui::FontId::new(
            size,
            egui::FontFamily::Name(DISPLAY_FONT.into()),
        ))
        .strong()
        .color(color)
}

/// Theme-aware Home surface with a restrained brand wash. The wash is a
/// surface choice, not artwork, and the high-contrast mode can opt out.
pub fn hero_frame(visuals: &egui::Visuals) -> egui::Frame {
    let fill = mix_color(
        visuals.window_fill,
        visuals.hyperlink_color,
        if visuals.dark_mode { 0.075 } else { 0.035 },
    );
    egui::Frame::new()
        .fill(fill)
        .stroke(visuals.window_stroke)
        .corner_radius(22.0)
        .inner_margin(egui::Margin::same(24))
}

/// Consistent raised surface for floating creative panels.
pub fn floating_panel_frame(visuals: &egui::Visuals) -> egui::Frame {
    egui::Frame::new()
        .fill(visuals.window_fill)
        .stroke(visuals.window_stroke)
        .corner_radius(PANEL_RADIUS)
        .inner_margin(egui::Margin::same(14))
}

/// Card surface shared by Home, New Artwork and contextual tool panels.
pub fn card_frame(visuals: &egui::Visuals, selected: bool) -> egui::Frame {
    let stroke = if selected {
        visuals.selection.stroke
    } else {
        visuals.window_stroke
    };
    egui::Frame::new()
        .fill(if selected {
            visuals.selection.bg_fill
        } else {
            visuals.faint_bg_color
        })
        .stroke(stroke)
        .corner_radius(BUTTON_RADIUS)
        .inner_margin(egui::Margin::same(12))
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
            egui::FontId::new(28.0, egui::FontFamily::Name(DISPLAY_FONT.into())),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(15.0, egui::FontFamily::Proportional),
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
    fn temporary_identity_is_text_only_and_uses_display_typography() {
        let context = egui::Context::default();
        install(&context);
        let output = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| brand_wordmark(ui, 22.0));
        });
        assert!(!output.shapes.is_empty());
        assert!(output.shapes.len() < 12);
        assert!(
            font_definitions()
                .families
                .contains_key(&egui::FontFamily::Name(DISPLAY_FONT.into()))
        );
    }
}
