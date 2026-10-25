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
        for label in ["Layers", "New", "Duplicate", "Delete", "Visible", "Locked"] {
            assert!(
                labels.contains(label),
                "{label} missing from tree: {labels}"
            );
        }
    }
}
