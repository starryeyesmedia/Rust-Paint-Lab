//! Application-only palettes for the studio shell. Document and raster colors
//! are rendered by a separate pipeline and are not changed by these settings.

use crate::workspace::{AccentScheme, AppearanceMode};

fn rgb(color: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(color[0], color[1], color[2])
}

fn color(scheme: AccentScheme, custom: Option<[u8; 3]>, dark: bool) -> egui::Color32 {
    let value = match scheme {
        AccentScheme::Violet => [0xA7, 0x8B, 0xFA],
        AccentScheme::Blue => [0x60, 0xA5, 0xFA],
        AccentScheme::Teal => [0x2D, 0xD4, 0xBF],
        AccentScheme::Coral => [0xFB, 0x71, 0x85],
        AccentScheme::Green => [0x4A, 0xDE, 0x80],
        AccentScheme::Neutral if dark => [0xC7, 0xC9, 0xD3],
        AccentScheme::Neutral => [0x59, 0x5B, 0x68],
        AccentScheme::Custom => custom.unwrap_or([0xA7, 0x8B, 0xFA]),
    };
    rgb(value)
}

fn linear_channel(value: u8) -> f32 {
    let value = f32::from(value) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub fn contrast_ratio(a: egui::Color32, b: egui::Color32) -> f32 {
    let luminance = |color: egui::Color32| {
        0.2126 * linear_channel(color.r())
            + 0.7152 * linear_channel(color.g())
            + 0.0722 * linear_channel(color.b())
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn mix(a: egui::Color32, b: egui::Color32, amount: f32) -> egui::Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    egui::Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

fn ensure_control_contrast(accent: egui::Color32, surface: egui::Color32) -> egui::Color32 {
    if contrast_ratio(accent, surface) >= 3.0 {
        return accent;
    }
    let black = egui::Color32::BLACK;
    let white = egui::Color32::WHITE;
    let target = if contrast_ratio(black, surface) > contrast_ratio(white, surface) {
        black
    } else {
        white
    };
    (1..=20)
        .map(|step| mix(accent, target, step as f32 / 20.0))
        .find(|candidate| contrast_ratio(*candidate, surface) >= 3.0)
        .unwrap_or(target)
}

fn readable_foreground(background: egui::Color32) -> egui::Color32 {
    let black = egui::Color32::BLACK;
    let white = egui::Color32::WHITE;
    if contrast_ratio(black, background) >= contrast_ratio(white, background) {
        black
    } else {
        white
    }
}

pub fn visuals(
    mode: AppearanceMode,
    system_dark: bool,
    scheme: AccentScheme,
    custom_accent: Option<[u8; 3]>,
) -> egui::Visuals {
    let dark = mode.is_dark(system_dark);
    let (background, surface, elevated, border, text, weak_text) = if dark {
        (
            rgb([0x24, 0x26, 0x32]), // Midnight Violet canvas surround
            rgb([0x30, 0x33, 0x41]),
            rgb([0x3E, 0x40, 0x52]),
            rgb([0x41, 0x42, 0x54]),
            rgb([0xF7, 0xF4, 0xFA]),
            rgb([0xB8, 0xB7, 0xC8]),
        )
    } else {
        (
            rgb([0xF7, 0xF6, 0xFA]),
            egui::Color32::WHITE,
            rgb([0xEF, 0xEC, 0xF4]),
            rgb([0xD7, 0xD4, 0xDF]),
            rgb([0x2C, 0x2A, 0x36]),
            rgb([0x64, 0x62, 0x70]),
        )
    };
    let accent = ensure_control_contrast(color(scheme, custom_accent, dark), background);
    let text_on_accent = readable_foreground(accent);
    let hover = mix(surface, accent, 0.18);
    let selected = mix(surface, accent, 0.28);

    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.dark_mode = dark;
    visuals.override_text_color = None;
    visuals.weak_text_color = Some(weak_text);
    visuals.panel_fill = background;
    visuals.window_fill = surface;
    visuals.window_stroke = egui::Stroke::new(1.0, border);
    visuals.faint_bg_color = surface;
    visuals.extreme_bg_color = if dark {
        background
    } else {
        egui::Color32::WHITE
    };
    visuals.text_edit_bg_color = Some(if dark {
        background
    } else {
        egui::Color32::WHITE
    });
    visuals.code_bg_color = elevated;
    visuals.hyperlink_color = accent;
    visuals.warn_fg_color = if dark {
        rgb([0xFF, 0xD1, 0x74])
    } else {
        rgb([0x8A, 0x56, 0x00])
    };
    visuals.error_fg_color = if dark {
        rgb([0xFF, 0x85, 0x8A])
    } else {
        rgb([0xB2, 0x1D, 0x28])
    };
    visuals.selection.bg_fill = selected;
    visuals.selection.stroke = egui::Stroke::new(1.0, accent);

    let set_widget = |widget: &mut egui::style::WidgetVisuals,
                      fill: egui::Color32,
                      weak_fill: egui::Color32,
                      edge: egui::Color32,
                      foreground: egui::Color32| {
        widget.bg_fill = fill;
        widget.weak_bg_fill = weak_fill;
        widget.bg_stroke = egui::Stroke::new(1.0, edge);
        widget.fg_stroke = egui::Stroke::new(1.5, foreground);
        widget.corner_radius = 7.0.into();
        widget.expansion = 0.0;
    };
    set_widget(
        &mut visuals.widgets.noninteractive,
        surface,
        surface,
        border,
        text,
    );
    set_widget(
        &mut visuals.widgets.inactive,
        surface,
        surface,
        border,
        text,
    );
    set_widget(&mut visuals.widgets.hovered, hover, hover, accent, text);
    set_widget(
        &mut visuals.widgets.active,
        accent,
        accent,
        accent,
        text_on_accent,
    );
    set_widget(&mut visuals.widgets.open, selected, selected, accent, text);
    visuals
}

/// Strengthen text, control boundaries, and selection contrast without changing
/// document/canvas color. This is an optional application accessibility mode.
pub fn high_contrast(mut visuals: egui::Visuals, dark: bool) -> egui::Visuals {
    let background = if dark {
        egui::Color32::BLACK
    } else {
        egui::Color32::WHITE
    };
    let foreground = if dark {
        egui::Color32::WHITE
    } else {
        egui::Color32::BLACK
    };
    let edge = if dark {
        egui::Color32::from_rgb(255, 230, 96)
    } else {
        egui::Color32::from_rgb(24, 48, 132)
    };
    visuals.panel_fill = background;
    visuals.window_fill = background;
    visuals.faint_bg_color = background;
    visuals.extreme_bg_color = background;
    visuals.override_text_color = Some(foreground);
    visuals.weak_text_color = Some(foreground);
    visuals.window_stroke = egui::Stroke::new(2.0, edge);
    visuals.selection.stroke = egui::Stroke::new(2.0, edge);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.fg_stroke = egui::Stroke::new(2.0, foreground);
        widget.bg_stroke = egui::Stroke::new(2.0, edge);
        widget.weak_bg_fill = background;
        if widget.bg_fill == egui::Color32::TRANSPARENT {
            widget.bg_fill = background;
        }
    }
    visuals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midnight_violet_palette_keeps_text_and_accent_controls_contrasting() {
        for (scheme, custom) in [
            (AccentScheme::Violet, None),
            (AccentScheme::Blue, None),
            (AccentScheme::Teal, None),
            (AccentScheme::Coral, None),
            (AccentScheme::Green, None),
            (AccentScheme::Neutral, None),
            (AccentScheme::Custom, Some([120, 30, 190])),
        ] {
            for mode in [AppearanceMode::Dark, AppearanceMode::Light] {
                let visuals = visuals(mode, false, scheme, custom);
                assert!(
                    contrast_ratio(
                        visuals.widgets.noninteractive.fg_stroke.color,
                        visuals.panel_fill
                    ) >= 4.5
                );
                assert!(contrast_ratio(visuals.widgets.active.bg_fill, visuals.panel_fill) >= 3.0);
                assert!(
                    contrast_ratio(
                        visuals.widgets.active.fg_stroke.color,
                        visuals.widgets.active.bg_fill
                    ) >= 4.5
                );
            }
        }
    }

    #[test]
    fn follow_system_appearance_uses_the_supplied_platform_theme() {
        assert!(
            visuals(
                AppearanceMode::FollowSystem,
                true,
                AccentScheme::Violet,
                None
            )
            .dark_mode
        );
        assert!(
            !visuals(
                AppearanceMode::FollowSystem,
                false,
                AccentScheme::Violet,
                None
            )
            .dark_mode
        );
    }

    #[test]
    fn high_contrast_mode_keeps_text_legible_and_control_edges_distinct() {
        for dark in [true, false] {
            let base = visuals(
                if dark {
                    AppearanceMode::Dark
                } else {
                    AppearanceMode::Light
                },
                dark,
                AccentScheme::Violet,
                None,
            );
            let high = high_contrast(base, dark);
            let text = high.override_text_color.unwrap();
            assert!(contrast_ratio(text, high.panel_fill) >= 7.0);
            assert!(contrast_ratio(high.widgets.inactive.bg_stroke.color, high.panel_fill) >= 3.0);
            assert!(high.widgets.inactive.bg_stroke.width >= 2.0);
        }
    }
}
