//! Accessibility tree behavior tests for the real egui integration.

#[cfg(test)]
mod tests {
    use crate::studio_icons::{self, Icon};

    #[test]
    fn accesskit_tree_contains_labeled_toolbar_and_document_controls() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                studio_icons::button(ui, Icon::Brush, "Brush", Some("B"), true, [48.0, 48.0]);
                ui.add(egui::Button::new("New document"));
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("egui emits a tree when AccessKit generation is enabled");
        let labels = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            labels.contains("Brush"),
            "toolbar label missing from tree: {labels}"
        );
        assert!(
            labels.contains("New document"),
            "document action missing from tree: {labels}"
        );
    }

    #[test]
    fn accesskit_tree_exposes_actions_in_the_real_floating_layers_panel() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut project = paint_document::Engine::new(32, 32, 300).unwrap();
        project.add_layer().unwrap();
        project.add_layer().unwrap();
        let mut actions = Vec::new();
        let output = context.run(Default::default(), |ctx| {
            egui::Window::new("Layers").show(ctx, |ui| {
                crate::studio::layer_flyout_contents(ui, &mut project, &mut actions);
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("layer flyout emits its semantic nodes");
        let labels = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        for label in [
            "Layers",
            "New",
            "Duplicate",
            "Delete",
            "Raise",
            "Lower",
            "Visible",
            "Locked",
        ] {
            assert!(
                labels.contains(label),
                "{label} missing from tree: {labels}"
            );
        }
        for label in ["Paint 1", "Paint 2", "Paint 3"] {
            assert!(
                labels.contains(label),
                "{label} missing from the populated layer tree: {labels}"
            );
        }
    }

    #[test]
    fn color_field_has_a_semantic_name_and_keyboard_adjustable_components() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::studio::saturation_value_field(ui, 0.78, 0.5, 0.75);
                let mut hue = 0.78;
                let mut saturation = 0.5;
                let mut brightness = 0.75;
                ui.add(egui::Slider::new(&mut hue, 0.0..=1.0).text("Hue"));
                ui.add(egui::Slider::new(&mut saturation, 0.0..=1.0).text("Saturation"));
                ui.add(egui::Slider::new(&mut brightness, 0.0..=1.0).text("Brightness"));
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("color controls emit AccessKit semantic nodes");
        let labels = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        for name in [
            "Saturation and brightness color field",
            "Hue",
            "Saturation",
            "Brightness",
        ] {
            assert!(labels.contains(name), "{name} missing from tree: {labels}");
        }
    }

    #[test]
    fn new_artwork_format_cards_have_semantic_names_in_the_accessibility_tree() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for (name, width, height) in [
                    ("Portrait", 1800.0, 2400.0),
                    ("Landscape", 2400.0, 1800.0),
                    ("Square", 2048.0, 2048.0),
                ] {
                    crate::studio::document_format_card(
                        ui,
                        name,
                        width,
                        height,
                        name == "Square",
                        120.0,
                    );
                }
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("format cards emit AccessKit nodes");
        let labels = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        for label in ["Portrait", "Landscape", "Square"] {
            assert!(
                labels.contains(label),
                "{label} format card is missing from the accessibility tree: {labels}"
            );
        }
        assert!(labels.contains("role: RadioButton"));
        assert!(
            labels.contains("toggled: True"),
            "selected format is exposed to assistive technology: {labels}"
        );
        assert!(
            labels.contains("toggled: False"),
            "unselected formats are exposed to assistive technology: {labels}"
        );
    }

    #[test]
    fn brush_preset_cards_keep_artist_facing_names_in_the_accessibility_tree() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::studio::brush_preset_card(
                    ui,
                    paint_document::brush::BrushKind::Round,
                    true,
                    [100, 40, 190],
                );
                crate::studio::brush_preset_card(
                    ui,
                    paint_document::brush::BrushKind::Ink,
                    false,
                    [100, 40, 190],
                );
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("brush cards emit AccessKit nodes");
        let labels = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        for label in ["Hard round", "Ink pen"] {
            assert!(
                labels.contains(label),
                "{label} missing from the Brush library accessibility tree: {labels}"
            );
        }
        assert!(
            labels.contains("toggled: True"),
            "active brush preset is marked selected: {labels}"
        );
        assert!(
            labels.contains("toggled: False"),
            "inactive brush presets are marked unselected: {labels}"
        );
    }

    #[test]
    fn focusable_custom_cards_activate_with_enter_and_space() {
        let context = egui::Context::default();
        let mut id = None;
        let _ = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                id = Some(
                    ui.allocate_response(egui::vec2(120.0, 80.0), egui::Sense::click())
                        .id,
                );
            });
        });
        context.memory_mut(|memory| memory.request_focus(id.expect("card id")));

        for key in [egui::Key::Enter, egui::Key::Space] {
            let input = egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            };
            let mut activated = false;
            let _ = context.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let response =
                        ui.allocate_response(egui::vec2(120.0, 80.0), egui::Sense::click());
                    activated = crate::studio::activate_focusable_card(ui, &response);
                });
            });
            assert!(activated, "focused card did not activate with {key:?}");
        }
    }
}
