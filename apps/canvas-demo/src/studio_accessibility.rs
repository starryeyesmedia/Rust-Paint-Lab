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
}
