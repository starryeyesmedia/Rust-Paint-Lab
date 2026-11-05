//! Direct tool catalog and deterministic focus navigation.

use crate::{studio::EditTool, studio_icons::Icon};

/// User-facing activation intent for a direct toolbar control.
///
/// Brush and Eraser are separate controls, while both use `EditTool::Brush`
/// underneath and use the app's existing brush preset behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolActivation {
    /// Select the ordinary editing tool recorded in the catalog entry.
    Tool,
    /// Select `EditTool::Brush` and return from Eraser to the last brush preset.
    Brush,
    /// Select `EditTool::Brush` with the existing Eraser preset.
    Eraser,
}

/// Metadata for one individually accessible toolbar action.
///
/// The label is suitable for visible text and the control's accessible name.
/// A shortcut is absent when the current app does not assign one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToolCatalogEntry {
    pub(crate) tool: EditTool,
    pub(crate) activation: ToolActivation,
    pub(crate) icon: Icon,
    pub(crate) label: &'static str,
    pub(crate) shortcut: Option<&'static str>,
}

#[allow(dead_code)]
impl ToolCatalogEntry {
    /// Return the spoken control name, including an assigned keyboard shortcut.
    pub(crate) fn accessible_name(self) -> String {
        self.shortcut.map_or_else(
            || self.label.to_owned(),
            |shortcut| format!("{} ({shortcut})", self.label),
        )
    }
}

const fn catalog_entry(
    tool: EditTool,
    activation: ToolActivation,
    icon: Icon,
    label: &'static str,
    shortcut: Option<&'static str>,
) -> ToolCatalogEntry {
    ToolCatalogEntry {
        tool,
        activation,
        icon,
        label,
        shortcut,
    }
}

/// All implemented tools in stable top-to-bottom and keyboard-focus order.
///
/// `Create path` remains named as the current path editor; it is not presented
/// as a finished freehand vector brush.
pub(crate) const TOOL_CATALOG: [ToolCatalogEntry; 15] = [
    catalog_entry(
        EditTool::Brush,
        ToolActivation::Brush,
        Icon::Brush,
        "Brush",
        Some("B"),
    ),
    catalog_entry(
        EditTool::Brush,
        ToolActivation::Eraser,
        Icon::Eraser,
        "Eraser",
        None,
    ),
    catalog_entry(
        EditTool::Eyedropper,
        ToolActivation::Tool,
        Icon::Eyedropper,
        "Eyedropper",
        Some("I"),
    ),
    catalog_entry(
        EditTool::Smudge,
        ToolActivation::Tool,
        Icon::Smudge,
        "Smudge",
        Some("S"),
    ),
    catalog_entry(
        EditTool::ColorMix,
        ToolActivation::Tool,
        Icon::ColorMixer,
        "Color Mixer",
        Some("X"),
    ),
    catalog_entry(
        EditTool::Fill,
        ToolActivation::Tool,
        Icon::Fill,
        "Fill",
        Some("F"),
    ),
    catalog_entry(
        EditTool::Gradient,
        ToolActivation::Tool,
        Icon::Gradient,
        "Gradient",
        Some("G"),
    ),
    catalog_entry(
        EditTool::QuickShape,
        ToolActivation::Tool,
        Icon::QuickShape,
        "QuickShape",
        Some("Q"),
    ),
    catalog_entry(
        EditTool::ShapeLine,
        ToolActivation::Tool,
        Icon::Line,
        "Line",
        Some("L"),
    ),
    catalog_entry(
        EditTool::ShapeRectangle,
        ToolActivation::Tool,
        Icon::Rectangle,
        "Rectangle",
        Some("R"),
    ),
    catalog_entry(
        EditTool::ShapeEllipse,
        ToolActivation::Tool,
        Icon::Ellipse,
        "Ellipse",
        Some("O"),
    ),
    catalog_entry(
        EditTool::Rectangle,
        ToolActivation::Tool,
        Icon::RectangleSelection,
        "Rectangle selection",
        Some("M"),
    ),
    catalog_entry(
        EditTool::Ellipse,
        ToolActivation::Tool,
        Icon::EllipseSelection,
        "Ellipse selection",
        Some("E"),
    ),
    catalog_entry(
        EditTool::Lasso,
        ToolActivation::Tool,
        Icon::Lasso,
        "Lasso selection",
        Some("K"),
    ),
    catalog_entry(
        EditTool::VectorPath,
        ToolActivation::Tool,
        Icon::Path,
        "Create path",
        Some("V"),
    ),
];

/// Move within a keyboard-navigable sequence without wrapping at either end.
pub(crate) fn adjacent_focus(index: usize, count: usize, forward: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let index = index.min(count - 1);
    Some(if forward {
        index.saturating_add(1).min(count - 1)
    } else {
        index.saturating_sub(1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smudge_keeps_the_touch_app_symbol_mapping() {
        let smudge = TOOL_CATALOG
            .iter()
            .find(|entry| entry.label == "Smudge")
            .unwrap();

        assert_eq!(smudge.icon, Icon::Smudge);
        assert_eq!(smudge.icon.asset_name(), "touch_app");
        assert_eq!(Icon::EditAnchors.asset_name(), "shape_line");
    }

    #[test]
    fn direct_catalog_lists_every_implemented_tool_in_stable_order() {
        assert_eq!(
            TOOL_CATALOG.map(|entry| entry.label),
            [
                "Brush",
                "Eraser",
                "Eyedropper",
                "Smudge",
                "Color Mixer",
                "Fill",
                "Gradient",
                "QuickShape",
                "Line",
                "Rectangle",
                "Ellipse",
                "Rectangle selection",
                "Ellipse selection",
                "Lasso selection",
                "Create path",
            ]
        );
        assert_eq!(TOOL_CATALOG.len(), 15);

        let implemented_tools = [
            EditTool::Brush,
            EditTool::Smudge,
            EditTool::ColorMix,
            EditTool::Fill,
            EditTool::Rectangle,
            EditTool::Ellipse,
            EditTool::Lasso,
            EditTool::Eyedropper,
            EditTool::ShapeLine,
            EditTool::ShapeRectangle,
            EditTool::ShapeEllipse,
            EditTool::QuickShape,
            EditTool::VectorPath,
            EditTool::Gradient,
        ];
        for tool in implemented_tools {
            let direct_controls = TOOL_CATALOG
                .iter()
                .filter(|entry| entry.tool == tool && entry.activation != ToolActivation::Eraser)
                .count();
            assert_eq!(
                direct_controls, 1,
                "{tool:?} needs one direct catalog control"
            );
        }
    }

    #[test]
    fn catalog_shortcuts_and_material_symbol_mappings_are_stable() {
        assert_eq!(
            TOOL_CATALOG.map(|entry| entry.shortcut),
            [
                Some("B"),
                None,
                Some("I"),
                Some("S"),
                Some("X"),
                Some("F"),
                Some("G"),
                Some("Q"),
                Some("L"),
                Some("R"),
                Some("O"),
                Some("M"),
                Some("E"),
                Some("K"),
                Some("V"),
            ]
        );
        assert_eq!(
            TOOL_CATALOG.map(|entry| (entry.label, entry.icon.asset_name())),
            [
                ("Brush", "brush"),
                ("Eraser", "ink_eraser"),
                ("Eyedropper", "colorize"),
                ("Smudge", "touch_app"),
                ("Color Mixer", "blender"),
                ("Fill", "format_color_fill"),
                ("Gradient", "linear_scale"),
                ("QuickShape", "shapes"),
                ("Line", "diagonal_line"),
                ("Rectangle", "rectangle"),
                ("Ellipse", "circle"),
                ("Rectangle selection", "select_all"),
                ("Ellipse selection", "radio_button_unchecked"),
                ("Lasso selection", "lasso_select"),
                ("Create path", "polyline"),
            ]
        );
    }

    #[test]
    fn brush_and_eraser_are_distinct_controls_over_the_brush_tool() {
        let brush = TOOL_CATALOG[0];
        let eraser = TOOL_CATALOG[1];
        assert_eq!(brush.tool, EditTool::Brush);
        assert_eq!(eraser.tool, EditTool::Brush);
        assert_eq!(brush.activation, ToolActivation::Brush);
        assert_eq!(eraser.activation, ToolActivation::Eraser);
        assert_eq!(brush.label, "Brush");
        assert_eq!(eraser.label, "Eraser");
        assert_eq!(brush.icon, Icon::Brush);
        assert_eq!(eraser.icon, Icon::Eraser);
        assert_eq!(brush.shortcut, Some("B"));
        assert_eq!(eraser.shortcut, None);
    }

    #[test]
    fn ellipse_selection_hint_matches_the_runtime_shortcut() {
        let entry = TOOL_CATALOG
            .iter()
            .find(|entry| entry.tool == EditTool::Ellipse)
            .unwrap();

        assert_eq!(entry.shortcut, Some("E"));
        assert_eq!(entry.accessible_name(), "Ellipse selection (E)");
    }

    #[test]
    fn accessible_names_include_only_assigned_shortcuts() {
        let expected = [
            "Brush (B)",
            "Eraser",
            "Eyedropper (I)",
            "Smudge (S)",
            "Color Mixer (X)",
            "Fill (F)",
            "Gradient (G)",
            "QuickShape (Q)",
            "Line (L)",
            "Rectangle (R)",
            "Ellipse (O)",
            "Rectangle selection (M)",
            "Ellipse selection (E)",
            "Lasso selection (K)",
            "Create path (V)",
        ];
        for (entry, expected) in TOOL_CATALOG.iter().zip(expected) {
            assert_eq!(entry.accessible_name().as_str(), expected);
        }
    }

    #[test]
    fn keyboard_focus_moves_through_the_full_catalog_without_wrapping() {
        let count = TOOL_CATALOG.len();
        for index in 0..count {
            assert_eq!(
                adjacent_focus(index, count, true),
                Some((index + 1).min(count - 1))
            );
            assert_eq!(
                adjacent_focus(index, count, false),
                Some(index.saturating_sub(1))
            );
        }
        assert_eq!(adjacent_focus(usize::MAX, count, true), Some(count - 1));
        assert_eq!(adjacent_focus(usize::MAX, count, false), Some(count - 2));
        assert_eq!(adjacent_focus(0, 0, true), None);
    }
}
