//! Accessibility tree behavior tests for the real egui integration.

#[cfg(test)]
mod tests {
    use crate::studio_icons::{self, Icon};
    use paint_document::vector::{VectorHandle, VectorPath, VectorSegment};

    fn app_context() -> egui::Context {
        let context = egui::Context::default();
        crate::ui_tokens::install(&context);
        context
    }

    #[test]
    fn accesskit_tree_contains_labeled_toolbar_and_document_controls() {
        let context = app_context();
        context.enable_accesskit();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                studio_icons::button(ui, Icon::Brush, "Brush", Some("B"), true, [48.0, 48.0]);
                studio_icons::button(
                    ui,
                    Icon::ColorMixer,
                    "Color Mixer",
                    Some("X"),
                    false,
                    [48.0, 48.0],
                );
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
            labels.contains("Color Mixer"),
            "new paint tool is missing its accessible toolbar name: {labels}"
        );
        assert!(
            labels.contains("New document"),
            "document action missing from tree: {labels}"
        );
    }

    #[test]
    fn unified_inspector_destinations_are_named_selected_radio_controls() {
        let context = app_context();
        context.enable_accesskit();
        let mut selected = crate::workspace::InspectorTab::Layers;
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (crate::workspace::InspectorTab::Layers, "Layers"),
                        (crate::workspace::InspectorTab::Brush, "Brush"),
                        (crate::workspace::InspectorTab::Edit, "Edit"),
                    ] {
                        crate::studio::inspector_tab_button_named(
                            ui,
                            &mut selected,
                            tab,
                            label,
                            label,
                            64.0,
                        );
                    }
                });
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (crate::workspace::InspectorTab::Color, "Color"),
                        (crate::workspace::InspectorTab::References, "References"),
                    ] {
                        crate::studio::inspector_tab_button_named(
                            ui,
                            &mut selected,
                            tab,
                            label,
                            label,
                            96.0,
                        );
                    }
                });
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("the unified Inspector destinations are present in the tree");
        for (label, selected_state) in [
            ("Layers", true),
            ("Brush", false),
            ("Edit", false),
            ("Color", false),
            ("References", false),
        ] {
            let node = tree
                .nodes
                .iter()
                .map(|(_, node)| node)
                .find(|node| node.label() == Some(label))
                .unwrap_or_else(|| panic!("{label} is missing from the Inspector tree"));
            assert_eq!(format!("{:?}", node.role()), "RadioButton", "{label}");
            assert_eq!(
                node.toggled().map(|value| format!("{value:?}")),
                Some(if selected_state { "True" } else { "False" }.to_string()),
                "{label}: {node:?}"
            );
        }
    }

    #[test]
    fn cubic_handle_editor_exposes_named_keyboard_adjustable_controls() {
        let context = app_context();
        context.enable_accesskit();
        let mut path =
            VectorPath::from_polyline(7, &[[8.0, 12.0], [48.0, 32.0]], [140, 70, 220, 255], 3.0)
                .unwrap();
        path.segments[0] = VectorSegment::CubicTo {
            control1: [14.0, 32.0],
            control2: [40.0, 4.0],
            point: [48.0, 32.0],
        };
        let mut selected = Some((path.id, 0, VectorHandle::Outgoing));
        let mut actions = Vec::new();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::studio::vector_handle_editor_contents(
                    ui,
                    &path,
                    0,
                    &mut selected,
                    &mut actions,
                    false,
                );
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("Bezier handle controls are represented in the AccessKit tree");
        let nodes = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        for label in [
            "Incoming handle",
            "Outgoing handle",
            "Vector Outgoing handle X position",
            "Vector Outgoing handle Y position",
            "Apply handle position",
        ] {
            assert!(nodes.contains(label), "{label} missing from tree: {nodes}");
        }
        assert!(
            nodes.contains("role: Button") && nodes.contains("role: SpinButton"),
            "handle selection and coordinate editing need keyboard control semantics: {nodes}"
        );
        assert!(
            actions.is_empty(),
            "rendering controls must not edit the path"
        );
    }

    #[test]
    fn accesskit_tree_exposes_actions_in_the_real_floating_layers_panel() {
        let context = app_context();
        context.enable_accesskit();
        let mut project = paint_document::Engine::new(32, 32, 300).unwrap();
        project.add_layer().unwrap();
        project.add_layer().unwrap();
        let mut actions = Vec::new();
        let mut thumbnail_cache = crate::layer_thumbnails::LayerThumbnailCache::default();
        let mut thumbnail_context = crate::studio::LayerThumbnailContext {
            cache: &mut thumbnail_cache,
            document_epoch: 0,
        };
        let mut drag_handles = Vec::new();
        let mut drop_targets = Vec::new();
        let mut drag_ui = crate::studio::LayerDragUi {
            handles: &mut drag_handles,
            drop_targets: &mut drop_targets,
            source: None,
            target: None,
        };
        let output = context.run(Default::default(), |ctx| {
            egui::Window::new("Layers").show(ctx, |ui| {
                crate::studio::layer_flyout_contents(
                    ui,
                    &mut project,
                    &mut thumbnail_context,
                    &mut actions,
                    crate::studio::LayerFlyoutOptions::default(),
                    &mut drag_ui,
                );
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
            "Actions",
            "Drag Paint 3 with a pointer to reorder; use Actions for keyboard reordering",
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
    fn compact_layer_icon_toggles_keep_checkbox_names_and_values() {
        let context = app_context();
        context.enable_accesskit();
        let mut project = paint_document::Engine::new(32, 32, 300).unwrap();
        project.add_layer().unwrap();
        let mut actions = Vec::new();
        let mut thumbnail_cache = crate::layer_thumbnails::LayerThumbnailCache::default();
        let mut thumbnail_context = crate::studio::LayerThumbnailContext {
            cache: &mut thumbnail_cache,
            document_epoch: 0,
        };
        let mut drag_handles = Vec::new();
        let mut drop_targets = Vec::new();
        let mut drag_ui = crate::studio::LayerDragUi {
            handles: &mut drag_handles,
            drop_targets: &mut drop_targets,
            source: None,
            target: None,
        };
        let output = context.run(Default::default(), |ctx| {
            egui::Window::new("Layers").show(ctx, |ui| {
                crate::studio::layer_flyout_contents(
                    ui,
                    &mut project,
                    &mut thumbnail_context,
                    &mut actions,
                    crate::studio::LayerFlyoutOptions {
                        compact: true,
                        ..Default::default()
                    },
                    &mut drag_ui,
                );
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("compact layer controls remain in the accessible tree");
        let nodes = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(nodes.contains("label: \"Visible\""), "{nodes}");
        assert!(nodes.contains("label: \"Locked\""), "{nodes}");
        assert!(
            nodes.contains("role: CheckBox"),
            "compact toggles are not checkbox semantics: {nodes}"
        );
        assert!(
            nodes.contains("label: \"Visible\", toggled: True"),
            "visible layer state is missing: {nodes}"
        );
        assert!(
            nodes.contains("label: \"Locked\", toggled: False"),
            "unlocked layer state is missing: {nodes}"
        );
    }

    #[test]
    fn drawing_symmetry_controls_are_named_and_keyboard_exposed() {
        let context = app_context();
        context.enable_accesskit();
        let mut symmetry = paint_document::Symmetry {
            mode: paint_document::SymmetryMode::Radial,
            axes: 8,
            center: [0.4, 0.6],
        };
        let mut origin_armed = false;
        let output = context.run(Default::default(), |ctx| {
            egui::Window::new("Drawing Symmetry").show(ctx, |ui| {
                crate::studio::symmetry_panel_contents(ui, &mut symmetry, &mut origin_armed, false);
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("drawing symmetry emits its semantic controls");
        let nodes = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        for label in [
            "Drawing Symmetry",
            "Off",
            "Vertical",
            "Horizontal",
            "Both axes",
            "Radial",
            "Radial axes",
            "Origin X",
            "Origin Y",
            "Place origin on canvas",
            "Center",
            "Turn symmetry off",
        ] {
            assert!(nodes.contains(label), "{label} missing from tree: {nodes}");
        }
        assert!(
            nodes.contains("role: Slider"),
            "radial axis count must be keyboard adjustable: {nodes}"
        );
        assert!(
            nodes.contains("role: RadioButton") || nodes.contains("role: Button"),
            "symmetry modes must have keyboard-activatable semantics: {nodes}"
        );
    }

    #[test]
    fn color_field_has_a_semantic_name_and_keyboard_adjustable_components() {
        let context = app_context();
        context.enable_accesskit();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::studio::saturation_value_field(ui, 0.78, 0.5, 0.75);
                crate::studio::hue_selection_wheel(ui, 0.78, 0.5, 0.75);
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
            "Hue selection wheel",
            "Hue",
            "Saturation",
            "Brightness",
        ] {
            assert!(labels.contains(name), "{name} missing from tree: {labels}");
        }
    }

    #[test]
    fn new_artwork_format_cards_have_semantic_names_in_the_accessibility_tree() {
        let context = app_context();
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
        let context = app_context();
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
        let context = app_context();
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
    fn home_gallery_open_action_has_a_descriptive_keyboard_button_name() {
        let context = app_context();
        context.enable_accesskit();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::studio::home_open_project_card(ui, 420.0, false);
            });
        });
        let tree = output
            .platform_output
            .accesskit_update
            .expect("the open-project tile emits its AccessKit semantics");
        let nodes = tree
            .nodes
            .iter()
            .map(|(_, node)| format!("{node:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            nodes.contains("Open another project from this device"),
            "the open-project action is described in the accessibility tree: {nodes}"
        );
        assert!(nodes.contains("role: Button"));
    }

    #[test]
    fn brush_preset_cards_keep_artist_facing_names_in_the_accessibility_tree() {
        let context = app_context();
        context.enable_accesskit();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::studio::brush_preset_card_with_preview(
                    ui,
                    paint_document::brush::BrushKind::Round,
                    true,
                    [100, 40, 190],
                    148.0,
                    82.0,
                    None,
                );
                crate::studio::brush_preset_card_with_preview(
                    ui,
                    paint_document::brush::BrushKind::Ink,
                    false,
                    [100, 40, 190],
                    148.0,
                    82.0,
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
        let context = app_context();
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
        let context = app_context();
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
