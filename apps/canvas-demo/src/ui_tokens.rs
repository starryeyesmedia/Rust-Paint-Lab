//! Shared interaction dimensions for the desktop studio.

pub const TOUCH_TARGET: f32 = 44.0;
pub const COMPACT_TARGET: f32 = 36.0;
pub const TOOL_TARGET: f32 = 48.0;
pub const BUTTON_RADIUS: f32 = 8.0;
pub const PANEL_RADIUS: f32 = 12.0;

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
}
