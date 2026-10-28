//! Shared interaction dimensions for the desktop studio.

pub const TOUCH_TARGET: f32 = 44.0;
pub const COMPACT_TARGET: f32 = 36.0;
pub const TOOL_TARGET: f32 = 48.0;
pub const BUTTON_RADIUS: f32 = 8.0;
pub const PANEL_RADIUS: f32 = 12.0;

const UI_REGULAR: &str = "BitchPixel Manrope 400";
const UI_SEMIBOLD: &str = "BitchPixel Manrope 600";
const UI_SEMIBOLD_TABULAR: &str = "BitchPixel Manrope 600 Tabular";
const UI_BOLD: &str = "BitchPixel Manrope 700";
const DISPLAY_BOLD: &str = "BitchPixel Outfit 700";
const DISPLAY_EXTRABOLD: &str = "BitchPixel Outfit 800";
const DISPLAY_BLACK: &str = "BitchPixel Outfit 900";

/// Named roles in the shared typographic hierarchy. Components choose a role
/// instead of inventing a point size for every label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeRole {
    Brand,
    Display,
    PageTitle,
    Slogan,
    SectionTitle,
    PanelTitle,
    CardTitle,
    Body,
    Button,
    InputLabel,
    ToolName,
    LayerName,
    Caption,
    Metadata,
    Numeric,
    Tooltip,
    Status,
}

impl TypeRole {
    fn size(self) -> f32 {
        match self {
            Self::Brand => 32.0,
            Self::Display => 48.0,
            Self::PageTitle => 30.0,
            Self::Slogan => 18.0,
            Self::SectionTitle => 20.0,
            Self::PanelTitle => 17.0,
            Self::CardTitle => 16.0,
            Self::Body => 15.5,
            Self::Button => 14.0,
            Self::InputLabel | Self::ToolName | Self::Metadata | Self::Tooltip | Self::Status => {
                13.0
            }
            Self::LayerName | Self::Numeric => 14.0,
            Self::Caption => 12.5,
        }
    }

    fn family(self) -> egui::FontFamily {
        let name = match self {
            Self::Brand | Self::Display | Self::Slogan => DISPLAY_BLACK,
            Self::PageTitle | Self::SectionTitle => DISPLAY_EXTRABOLD,
            Self::PanelTitle | Self::CardTitle => DISPLAY_BOLD,
            Self::Button | Self::InputLabel | Self::ToolName | Self::LayerName => UI_SEMIBOLD,
            Self::Numeric => UI_SEMIBOLD_TABULAR,
            Self::Body | Self::Caption | Self::Metadata | Self::Tooltip | Self::Status => {
                UI_REGULAR
            }
        };
        egui::FontFamily::Name(name.into())
    }
}

pub fn font_id(role: TypeRole) -> egui::FontId {
    font_id_at(role, role.size())
}

pub fn font_id_at(role: TypeRole, size: f32) -> egui::FontId {
    egui::FontId::new(size.max(1.0), role.family())
}

pub fn type_text(text: impl Into<String>, role: TypeRole, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(text).font(font_id(role)).color(color)
}

/// Builds a locally rendered, offline gradient text run using the same named
/// type roles as the rest of the interface. Callers must check contrast of the
/// gradient against its actual surface before using it.
pub fn gradient_type_job(
    text: &str,
    role: TypeRole,
    colors: [egui::Color32; 2],
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let count = text.chars().count();
    let denominator = count.saturating_sub(1).max(1) as f32;
    for (index, character) in text.chars().enumerate() {
        let amount = index as f32 / denominator;
        let channel = |start: u8, end: u8| {
            (f32::from(start) + (f32::from(end) - f32::from(start)) * amount).round() as u8
        };
        let color = egui::Color32::from_rgb(
            channel(colors[0].r(), colors[1].r()),
            channel(colors[0].g(), colors[1].g()),
            channel(colors[0].b(), colors[1].b()),
        );
        job.append(
            &character.to_string(),
            0.0,
            egui::TextFormat {
                font_id: font_id(role),
                color,
                ..Default::default()
            },
        );
    }
    job
}

fn mix_color(a: egui::Color32, b: egui::Color32, amount: f32) -> egui::Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |left: u8, right: u8| {
        (f32::from(left) + (f32::from(right) - f32::from(left)) * amount).round() as u8
    };
    egui::Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// Install the locally bundled OFL type system before creating any widgets.
/// Named weight families keep body, action, and display hierarchy consistent;
/// every family retains egui's normal fallback fonts for unsupported glyphs.
fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    let assets = [
        (
            UI_REGULAR,
            include_bytes!("../../../assets/fonts/Manrope-Regular.ttf").as_slice(),
        ),
        (
            UI_SEMIBOLD,
            include_bytes!("../../../assets/fonts/Manrope-SemiBold.ttf").as_slice(),
        ),
        (
            UI_SEMIBOLD_TABULAR,
            include_bytes!("../../../assets/fonts/Manrope-SemiBold-Tabular.ttf").as_slice(),
        ),
        (
            UI_BOLD,
            include_bytes!("../../../assets/fonts/Manrope-Bold.ttf").as_slice(),
        ),
        (
            DISPLAY_BOLD,
            include_bytes!("../../../assets/fonts/Outfit-Bold.ttf").as_slice(),
        ),
        (
            DISPLAY_EXTRABOLD,
            include_bytes!("../../../assets/fonts/Outfit-ExtraBold.ttf").as_slice(),
        ),
        (
            DISPLAY_BLACK,
            include_bytes!("../../../assets/fonts/Outfit-Black.ttf").as_slice(),
        ),
    ];
    for (name, bytes) in assets {
        fonts.font_data.insert(
            name.to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    let proportional = fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .expect("egui provides a proportional font family");
    proportional.insert(0, UI_REGULAR.to_owned());
    let fallback = proportional.clone();
    for (name, weight) in [
        (UI_REGULAR, vec![UI_REGULAR]),
        (UI_SEMIBOLD, vec![UI_SEMIBOLD, UI_REGULAR]),
        (
            UI_SEMIBOLD_TABULAR,
            vec![UI_SEMIBOLD_TABULAR, UI_SEMIBOLD, UI_REGULAR],
        ),
        (UI_BOLD, vec![UI_BOLD, UI_SEMIBOLD, UI_REGULAR]),
        (
            DISPLAY_BOLD,
            vec![DISPLAY_BOLD, UI_BOLD, UI_SEMIBOLD, UI_REGULAR],
        ),
        (
            DISPLAY_EXTRABOLD,
            vec![DISPLAY_EXTRABOLD, DISPLAY_BOLD, UI_BOLD, UI_REGULAR],
        ),
        (
            DISPLAY_BLACK,
            vec![
                DISPLAY_BLACK,
                DISPLAY_EXTRABOLD,
                DISPLAY_BOLD,
                UI_BOLD,
                UI_REGULAR,
            ],
        ),
    ] {
        let mut candidates = weight.into_iter().map(str::to_owned).collect::<Vec<_>>();
        for font in &fallback {
            if !candidates.contains(font) {
                candidates.push(font.clone());
            }
        }
        fonts
            .families
            .insert(egui::FontFamily::Name(name.into()), candidates);
    }
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
    let display = TypeRole::Brand.family();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(
            egui::RichText::new("Bitch")
                .font(egui::FontId::new(size, display.clone()))
                .color(foreground),
        );
        ui.label(
            egui::RichText::new("Pixel")
                .font(egui::FontId::new(size, display))
                .color(accent),
        );
    });
}

/// A display face for the brand headline and intentionally prominent actions.
pub fn display_text(text: &str, size: f32, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(text)
        .font(egui::FontId::new(size, TypeRole::Display.family()))
        .color(color)
}

/// Theme-aware Home surface with a restrained brand wash. The wash is a
/// surface choice, not artwork, and the high-contrast mode can opt out.
pub fn hero_frame(visuals: &egui::Visuals) -> egui::Frame {
    egui::Frame::new()
        .fill(hero_surface(visuals))
        .stroke(visuals.window_stroke)
        .corner_radius(22.0)
        .inner_margin(egui::Margin::same(24))
}

/// Return the exact Home hero fill so prominent text can be checked against
/// the surface it is actually drawn on rather than the surrounding window.
pub fn hero_surface(visuals: &egui::Visuals) -> egui::Color32 {
    if visuals.override_text_color.is_some() {
        return visuals.window_fill;
    }
    mix_color(
        visuals.window_fill,
        visuals.hyperlink_color,
        if visuals.dark_mode { 0.075 } else { 0.035 },
    )
}

/// Consistent raised surface for floating creative panels.
pub fn floating_panel_frame(visuals: &egui::Visuals) -> egui::Frame {
    egui::Frame::new()
        .fill(visuals.window_fill)
        .stroke(visuals.window_stroke)
        .corner_radius(PANEL_RADIUS)
        .inner_margin(egui::Margin::same(14))
}

/// The primary tool rail uses the same raised surface as flyouts, with a
/// restrained violet edge so it reads as one coherent workspace component.
pub fn tool_palette_frame(visuals: &egui::Visuals) -> egui::Frame {
    let high_contrast = visuals.override_text_color.is_some();
    let accent = if high_contrast {
        visuals.window_stroke.color
    } else {
        visuals.hyperlink_color
    };
    egui::Frame::new()
        .fill(if high_contrast {
            visuals.window_fill
        } else {
            mix_color(
                visuals.window_fill,
                accent,
                if visuals.dark_mode { 0.08 } else { 0.035 },
            )
        })
        .stroke(if high_contrast {
            visuals.window_stroke
        } else {
            egui::Stroke::new(1.25, mix_color(visuals.window_stroke.color, accent, 0.35))
        })
        .corner_radius(16.0)
        .inner_margin(egui::Margin::same(5))
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

/// A consistent editorial heading for major Home and panel sections.
pub fn section_heading(ui: &mut egui::Ui, title: &str, detail: Option<&str>) {
    let visuals = ui.visuals();
    let foreground =
        crate::appearance::readable_text_color(visuals.text_color(), visuals.window_fill);
    let marker_color = if visuals.override_text_color.is_some() {
        visuals.window_stroke.color
    } else {
        visuals.hyperlink_color
    };
    ui.horizontal(|ui| {
        let (marker, _) = ui.allocate_exact_size(egui::vec2(4.0, 22.0), egui::Sense::hover());
        ui.painter().rect_filled(marker, 2.0, marker_color);
        ui.label(type_text(title, TypeRole::SectionTitle, foreground));
        if let Some(detail) = detail {
            let detail_width = (ui.available_width() - 8.0).max(20.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let metadata = crate::appearance::readable_text_color(
                    ui.visuals().weak_text_color(),
                    ui.visuals().window_fill,
                );
                ui.add_sized(
                    egui::vec2(detail_width, TypeRole::Metadata.size() * 1.6),
                    egui::Label::new(type_text(detail, TypeRole::Metadata, metadata)).truncate(),
                );
            });
        }
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
            egui::FontId::new(25.0, TypeRole::PageTitle.family()),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(TypeRole::Body.size(), egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(TypeRole::Button.size(), TypeRole::Button.family()),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(TypeRole::Caption.size(), egui::FontFamily::Proportional),
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
            TypeRole::PageTitle.family()
        );
        assert_eq!(
            style.text_styles[&egui::TextStyle::Button].family,
            egui::FontFamily::Name(UI_SEMIBOLD.into())
        );
        let fonts = font_definitions();
        for name in [
            UI_REGULAR,
            UI_SEMIBOLD,
            UI_SEMIBOLD_TABULAR,
            UI_BOLD,
            DISPLAY_BOLD,
            DISPLAY_EXTRABOLD,
            DISPLAY_BLACK,
        ] {
            assert!(
                fonts.font_data.contains_key(name),
                "missing local font {name}"
            );
        }
        for role in [
            TypeRole::Brand,
            TypeRole::Display,
            TypeRole::PageTitle,
            TypeRole::Slogan,
            TypeRole::SectionTitle,
            TypeRole::PanelTitle,
            TypeRole::CardTitle,
            TypeRole::Body,
            TypeRole::Button,
            TypeRole::InputLabel,
            TypeRole::ToolName,
            TypeRole::LayerName,
            TypeRole::Caption,
            TypeRole::Metadata,
            TypeRole::Numeric,
            TypeRole::Tooltip,
            TypeRole::Status,
        ] {
            assert!(role.size() >= 12.5 && role.size() <= 48.0);
            assert!(fonts.families.contains_key(&role.family()));
        }
        let fallback = &fonts.families[&egui::FontFamily::Proportional];
        for family in [
            TypeRole::Body.family(),
            TypeRole::Button.family(),
            TypeRole::PageTitle.family(),
        ] {
            let members = &fonts.families[&family];
            assert!(members.iter().any(|font| fallback.contains(font)));
        }
        assert_eq!(
            TypeRole::Brand.family(),
            egui::FontFamily::Name(DISPLAY_BLACK.into())
        );
        assert_eq!(
            TypeRole::PageTitle.family(),
            egui::FontFamily::Name(DISPLAY_EXTRABOLD.into())
        );
        assert_eq!(
            TypeRole::PanelTitle.family(),
            egui::FontFamily::Name(DISPLAY_BOLD.into())
        );
        assert_eq!(
            TypeRole::Numeric.family(),
            egui::FontFamily::Name(UI_SEMIBOLD_TABULAR.into())
        );
    }

    #[test]
    fn numeric_role_uses_tabular_manrope_figures() {
        let context = egui::Context::default();
        install(&context);
        let mut advances = Vec::new();
        let _ = context.run(egui::RawInput::default(), |context| {
            advances = context.fonts_mut(|fonts| {
                (0..=9)
                    .map(|digit| {
                        fonts
                            .layout_no_wrap(
                                digit.to_string(),
                                font_id(TypeRole::Numeric),
                                egui::Color32::WHITE,
                            )
                            .size()
                            .x
                    })
                    .collect::<Vec<_>>()
            });
        });
        assert!(
            advances
                .iter()
                .all(|advance| (*advance - advances[0]).abs() < 0.01)
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
                .contains_key(&TypeRole::Brand.family())
        );
    }

    #[test]
    fn gradient_type_job_preserves_text_role_and_both_color_endpoints() {
        let start = egui::Color32::from_rgb(156, 108, 255);
        let end = egui::Color32::from_rgb(234, 111, 214);
        let job = gradient_type_job("No limits. No bullshit.", TypeRole::Slogan, [start, end]);
        assert_eq!(job.text, "No limits. No bullshit.");
        assert_eq!(job.sections.first().unwrap().format.color, start);
        assert_eq!(job.sections.last().unwrap().format.color, end);
        assert!(
            job.sections
                .iter()
                .all(|section| { section.format.font_id == font_id(TypeRole::Slogan) })
        );
    }

    #[test]
    fn every_typographic_role_shapes_with_local_fonts_and_fallback_glyphs() {
        let context = egui::Context::default();
        install(&context);
        let output = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                for role in [
                    TypeRole::Brand,
                    TypeRole::Display,
                    TypeRole::PageTitle,
                    TypeRole::Slogan,
                    TypeRole::SectionTitle,
                    TypeRole::PanelTitle,
                    TypeRole::CardTitle,
                    TypeRole::Body,
                    TypeRole::Button,
                    TypeRole::InputLabel,
                    TypeRole::ToolName,
                    TypeRole::LayerName,
                    TypeRole::Caption,
                    TypeRole::Metadata,
                    TypeRole::Numeric,
                    TypeRole::Tooltip,
                    TypeRole::Status,
                ] {
                    ui.label(type_text(
                        "BitchPixel 2048 × 2048 · Ångström — 艺术",
                        role,
                        ui.visuals().text_color(),
                    ));
                }
            });
        });
        assert!(
            output.shapes.len() >= 17,
            "all type roles emitted text shapes"
        );
    }

    #[test]
    fn hero_text_meets_contrast_on_the_surface_used_by_the_hero_frame() {
        for mode in [
            crate::workspace::AppearanceMode::Dark,
            crate::workspace::AppearanceMode::Light,
        ] {
            for high_contrast_enabled in [false, true] {
                let mut visuals = crate::appearance::visuals(
                    mode,
                    mode.is_dark(false),
                    crate::workspace::AccentScheme::Violet,
                    None,
                );
                if high_contrast_enabled {
                    visuals = crate::appearance::high_contrast(visuals, mode.is_dark(false));
                }
                let surface = hero_surface(&visuals);
                let headline =
                    crate::appearance::readable_text_color(visuals.text_color(), surface);
                let slogan =
                    crate::appearance::readable_text_accent(visuals.hyperlink_color, surface);
                assert!(crate::appearance::contrast_ratio(headline, surface) >= 4.5);
                assert!(crate::appearance::contrast_ratio(slogan, surface) >= 4.5);
            }
        }
    }
}
