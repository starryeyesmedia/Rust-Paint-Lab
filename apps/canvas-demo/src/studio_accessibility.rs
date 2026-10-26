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
                        300,
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
    fn home_quick_start_cards_are_named_buttons_with_exact_formats() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for (name, dimensions) in [
                    ("Portrait", [1800, 2400]),
                    ("Square", [2048, 2048]),
                    ("Landscape", [2400, 1800]),
                    ("A4 Print", [2480, 3508]),
                ] {
                    crate::studio::home_document_card(ui, name, dimensions, 180.0);
                }
                crate::studio::home_custom_document_card(ui, 180.0);
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("Home quick-start choices emit AccessKit nodes");
        let nodes = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        for label in [
            "New Portrait artwork, 1800 by 2400 pixels",
            "New Square artwork, 2048 by 2048 pixels",
            "New Landscape artwork, 2400 by 1800 pixels",
            "New A4 Print artwork, 2480 by 3508 pixels",
            "Create artwork with a custom canvas size",
        ] {
            assert!(nodes.contains(label), "{label} missing from tree: {nodes}");
        }
        assert!(nodes.matches("role: Button").count() >= 5);
    }

    #[test]
    fn recent_artwork_search_has_a_named_keyboard_editable_control() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut query = String::new();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::studio::recent_search_field(ui, &mut query);
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("recent search emits an AccessKit semantic node");
        let labels = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            labels.contains("Find recent artwork by filename"),
            "search field label missing from accessibility tree: {labels}"
        );
        assert!(labels.contains("role: TextInput"));
    }

    #[test]
    fn brush_preset_cards_keep_artist_facing_names_in_the_accessibility_tree() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::studio::brush_preset_card_with_preview(
                    ui,
                    paint_document::brush::BrushKind::Round,
                    true,
                    [100, 40, 190],
                    148.0,
                    None,
                );
                crate::studio::brush_preset_card_with_preview(
                    ui,
                    paint_document::brush::BrushKind::Ink,
                    false,
                    [100, 40, 190],
                    148.0,
                    None,
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

    #[test]
    fn saved_color_swatches_have_a_named_accessible_action() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::studio::color_swatch_response(
                    ui,
                    [245, 88, 206],
                    "Use saved color #F558CE",
                    true,
                    38.0,
                );
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("saved swatch emits an AccessKit node");
        let labels = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            labels.contains("Use saved color #F558CE"),
            "saved color action is unnamed: {labels}"
        );
        assert!(labels.contains("role: Button"));
    }
}
