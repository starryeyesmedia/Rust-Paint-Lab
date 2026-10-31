//! Approved tool taxonomy and deterministic toolbar navigation.

use crate::{studio::EditTool, studio_icons::Icon};

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
    fn toolbar_groups_match_the_approved_order_and_members() {
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
    fn keyboard_focus_moves_without_wrapping() {
        assert_eq!(adjacent_focus(0, 8, false), Some(0));
        assert_eq!(adjacent_focus(0, 8, true), Some(1));
        assert_eq!(adjacent_focus(7, 8, true), Some(7));
        assert_eq!(adjacent_focus(7, 8, false), Some(6));
        assert_eq!(adjacent_focus(0, 0, true), None);
    }
}
