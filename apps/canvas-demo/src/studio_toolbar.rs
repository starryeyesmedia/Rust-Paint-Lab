//! Direct tool catalog plus temporary grouped-rail compatibility and deterministic navigation.

use crate::{studio::EditTool, studio_icons::Icon};

/// Compatibility taxonomy used by the current `studio.rs` grouped rail. It is
/// retained until the separately leased integration replaces those callers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ToolFamily {
    PaintEffects,
    ColorApplication,
    Shapes,
    Selection,
    Vector,
}

impl ToolFamily {
    pub(crate) const ORDERED: [Self; 5] = [
        Self::PaintEffects,
        Self::ColorApplication,
        Self::Shapes,
        Self::Selection,
        Self::Vector,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::PaintEffects => "Paint Effects",
            Self::ColorApplication => "Color Application",
            Self::Shapes => "Shapes",
            Self::Selection => "Selection",
            Self::Vector => "Vector",
        }
    }

    pub(crate) const fn representative(self) -> Icon {
        match self {
            Self::PaintEffects => Icon::Smudge,
            Self::ColorApplication => Icon::Fill,
            Self::Shapes => Icon::QuickShape,
            Self::Selection => Icon::RectangleSelection,
            Self::Vector => Icon::Path,
        }
    }

    pub(crate) const fn members(self) -> &'static [ToolEntry] {
        match self {
            Self::PaintEffects => &PAINT_EFFECTS,
            Self::ColorApplication => &COLOR_APPLICATION,
            Self::Shapes => &SHAPES,
            Self::Selection => &SELECTION,
            Self::Vector => &VECTOR,
        }
    }

    pub(crate) fn active(self, tool: EditTool) -> Option<&'static ToolEntry> {
        self.members().iter().find(|entry| entry.tool == tool)
    }

    pub(crate) fn containing(tool: EditTool) -> Option<Self> {
        Self::ORDERED
            .into_iter()
            .find(|family| family.active(tool).is_some())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToolEntry {
    pub(crate) tool: EditTool,
    pub(crate) icon: Icon,
    pub(crate) label: &'static str,
    pub(crate) shortcut: &'static str,
}

/// User-facing activation intent for a direct toolbar control.
///
/// Brush and Eraser are separate controls, while both use `EditTool::Brush`
/// underneath. The integration applies the existing brush preset behavior.
// The renderer will consume this metadata once PR #9 releases studio.rs.
#[allow(dead_code)]
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
// The renderer will consume this metadata once PR #9 releases studio.rs.
#[allow(dead_code)]
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
#[allow(dead_code)]
///
/// `Create path` remains named as the current path editor; it is not presented
/// as a finished freehand vector brush. This catalog is groundwork only; the
/// current `studio.rs` rail still uses its compatibility grouped APIs.
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
        Some("Shift+E"),
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

/// Interim direct-tool subset consumed by the current `studio.rs` rail.
/// Preserve it until that file's grouped-rail integration is replaced.
pub(crate) const DIRECT_TOOLS: [ToolEntry; 3] = [
    ToolEntry {
        tool: EditTool::Brush,
        icon: Icon::Brush,
        label: "Brush",
        shortcut: "B",
    },
    ToolEntry {
        tool: EditTool::Brush,
        icon: Icon::Eraser,
        label: "Eraser",
        shortcut: "",
    },
    ToolEntry {
        tool: EditTool::Eyedropper,
        icon: Icon::Eyedropper,
        label: "Eyedropper",
        shortcut: "I",
    },
];

const PAINT_EFFECTS: [ToolEntry; 2] = [
    ToolEntry {
        tool: EditTool::Smudge,
        icon: Icon::Smudge,
        label: "Smudge",
        shortcut: "S",
    },
    ToolEntry {
        tool: EditTool::ColorMix,
        icon: Icon::ColorMixer,
        label: "Color mixer",
        shortcut: "X",
    },
];

const COLOR_APPLICATION: [ToolEntry; 2] = [
    ToolEntry {
        tool: EditTool::Fill,
        icon: Icon::Fill,
        label: "Fill",
        shortcut: "F",
    },
    ToolEntry {
        tool: EditTool::Gradient,
        icon: Icon::Gradient,
        label: "Gradient",
        shortcut: "G",
    },
];

const SHAPES: [ToolEntry; 4] = [
    ToolEntry {
        tool: EditTool::QuickShape,
        icon: Icon::QuickShape,
        label: "QuickShape",
        shortcut: "Q",
    },
    ToolEntry {
        tool: EditTool::ShapeLine,
        icon: Icon::Line,
        label: "Line",
        shortcut: "L",
    },
    ToolEntry {
        tool: EditTool::ShapeRectangle,
        icon: Icon::Rectangle,
        label: "Rectangle",
        shortcut: "R",
    },
    ToolEntry {
        tool: EditTool::ShapeEllipse,
        icon: Icon::Ellipse,
        label: "Ellipse",
        shortcut: "O",
    },
];

const SELECTION: [ToolEntry; 3] = [
    ToolEntry {
        tool: EditTool::Rectangle,
        icon: Icon::RectangleSelection,
        label: "Rectangle selection",
        shortcut: "M",
    },
    ToolEntry {
        tool: EditTool::Ellipse,
        icon: Icon::EllipseSelection,
        label: "Ellipse selection",
        shortcut: "Shift+E",
    },
    ToolEntry {
        tool: EditTool::Lasso,
        icon: Icon::Lasso,
        label: "Lasso selection",
        shortcut: "K",
    },
];

const VECTOR: [ToolEntry; 1] = [ToolEntry {
    tool: EditTool::VectorPath,
    icon: Icon::Path,
    label: "Create path",
    shortcut: "V",
}];

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

/// Compatibility navigation for the current grouped rail; remove after the
/// `studio.rs` integration consumes `TOOL_CATALOG`.
pub(crate) fn adjacent_family(index: usize, forward: bool) -> Option<ToolFamily> {
    let count = ToolFamily::ORDERED.len();
    if count == 0 {
        return None;
    }
    let index = if forward {
        index.saturating_add(1) % count
    } else {
        index.checked_sub(1).unwrap_or(count - 1)
    };
    Some(ToolFamily::ORDERED[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_grouped_compatibility_entries_keep_the_existing_members() {
        assert_eq!(
            ToolFamily::ORDERED.map(ToolFamily::label),
            [
                "Paint Effects",
                "Color Application",
                "Shapes",
                "Selection",
                "Vector"
            ]
        );
        assert_eq!(
            ToolFamily::PaintEffects
                .members()
                .iter()
                .map(|entry| entry.label)
                .collect::<Vec<_>>(),
            ["Smudge", "Color mixer"]
        );
        assert_eq!(
            ToolFamily::ColorApplication
                .members()
                .iter()
                .map(|entry| entry.label)
                .collect::<Vec<_>>(),
            ["Fill", "Gradient"]
        );
        assert_eq!(
            ToolFamily::Shapes
                .members()
                .iter()
                .map(|entry| entry.label)
                .collect::<Vec<_>>(),
            ["QuickShape", "Line", "Rectangle", "Ellipse"]
        );
        assert_eq!(
            ToolFamily::Selection
                .members()
                .iter()
                .map(|entry| entry.label)
                .collect::<Vec<_>>(),
            [
                "Rectangle selection",
                "Ellipse selection",
                "Lasso selection"
            ]
        );
        assert_eq!(
            ToolFamily::Vector
                .members()
                .iter()
                .map(|entry| entry.label)
                .collect::<Vec<_>>(),
            ["Create path"]
        );
    }

    #[test]
    fn direct_tools_and_smudge_symbol_keep_the_approved_mapping() {
        assert_eq!(
            DIRECT_TOOLS.map(|entry| entry.label),
            ["Brush", "Eraser", "Eyedropper"]
        );
        assert_eq!(PAINT_EFFECTS[0].icon, Icon::Smudge);
        assert_eq!(Icon::Smudge.asset_name(), "touch_app");
        assert_eq!(
            ToolFamily::containing(EditTool::ColorMix),
            Some(ToolFamily::PaintEffects)
        );
        assert_eq!(ToolFamily::containing(EditTool::Brush), None);
        assert_eq!(
            DIRECT_TOOLS.map(|entry| (entry.label, entry.icon.asset_name())),
            [
                ("Brush", "brush"),
                ("Eraser", "ink_eraser"),
                ("Eyedropper", "colorize")
            ]
        );
        assert_eq!(
            ToolFamily::PaintEffects
                .members()
                .iter()
                .map(|entry| (entry.label, entry.icon.asset_name()))
                .collect::<Vec<_>>(),
            [("Smudge", "touch_app"), ("Color mixer", "blender")]
        );
        assert_eq!(
            ToolFamily::ColorApplication
                .members()
                .iter()
                .map(|entry| (entry.label, entry.icon.asset_name()))
                .collect::<Vec<_>>(),
            [("Fill", "format_color_fill"), ("Gradient", "linear_scale")]
        );
        assert_eq!(
            ToolFamily::Shapes
                .members()
                .iter()
                .map(|entry| (entry.label, entry.icon.asset_name()))
                .collect::<Vec<_>>(),
            [
                ("QuickShape", "shapes"),
                ("Line", "diagonal_line"),
                ("Rectangle", "rectangle"),
                ("Ellipse", "circle")
            ]
        );
        assert_eq!(
            ToolFamily::Selection
                .members()
                .iter()
                .map(|entry| (entry.label, entry.icon.asset_name()))
                .collect::<Vec<_>>(),
            [
                ("Rectangle selection", "select_all"),
                ("Ellipse selection", "radio_button_unchecked"),
                ("Lasso selection", "lasso_select")
            ]
        );
        assert_eq!(
            ToolFamily::Vector.members()[0].icon.asset_name(),
            "polyline"
        );
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
                Some("Shift+E"),
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
            "Ellipse selection (Shift+E)",
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

    #[test]
    fn legacy_group_navigation_keeps_existing_order_until_integration() {
        assert_eq!(adjacent_family(0, true), Some(ToolFamily::ColorApplication));
        assert_eq!(adjacent_family(3, true), Some(ToolFamily::Vector));
        assert_eq!(adjacent_family(4, true), Some(ToolFamily::PaintEffects));
        assert_eq!(adjacent_family(0, false), Some(ToolFamily::Vector));
        assert_eq!(adjacent_family(4, false), Some(ToolFamily::Selection));
    }
}
