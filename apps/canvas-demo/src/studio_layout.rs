//! Small, deterministic helpers for placing floating studio panels.

use crate::workspace::PaletteDock;

/// Convert a physical client-pixel position to egui points and test whether it
/// belongs to a panel rect. The window scale is explicit so input routing stays
/// correct on high-DPI displays.
pub fn client_point_in_rect(rect: egui::Rect, client_px: [f64; 2], pixels_per_point: f32) -> bool {
    if !pixels_per_point.is_finite()
        || pixels_per_point <= 0.0
        || !client_px[0].is_finite()
        || !client_px[1].is_finite()
    {
        return false;
    }
    let point = egui::pos2(
        (client_px[0] / f64::from(pixels_per_point)) as f32,
        (client_px[1] / f64::from(pixels_per_point)) as f32,
    );
    point.x.is_finite() && point.y.is_finite() && rect.contains(point)
}

/// Position a docked tool palette inside the available canvas viewport. The
/// caller excludes application menu and contextual-option bars from this
/// viewport, so top and side docks can sit close to the canvas edge. Top and
/// bottom docks are centered and use a horizontal row.
pub fn palette_anchor(viewport: egui::Rect, dock: PaletteDock, floating: egui::Pos2) -> egui::Pos2 {
    let margin = 8.0;
    let right_anchor = (viewport.right() - 72.0).max(viewport.left() + margin);
    let horizontal_anchor = (viewport.center().x - 320.0).clamp(
        viewport.left() + margin,
        (viewport.right() - 656.0).max(viewport.left() + margin),
    );
    match dock {
        PaletteDock::Left => egui::pos2(viewport.left() + margin, viewport.top() + margin),
        PaletteDock::Right => egui::pos2(right_anchor, viewport.top() + margin),
        PaletteDock::Top => egui::pos2(horizontal_anchor, viewport.top() + margin),
        PaletteDock::Bottom => egui::pos2(horizontal_anchor, viewport.bottom() - 72.0),
        PaletteDock::Floating => floating,
    }
}

/// Snap a dragged palette to its nearest edge when released within the hit
/// margin. Positions outside that margin remain floating and viewport-clamped.
pub fn snap_palette(viewport: egui::Rect, rect: egui::Rect) -> PaletteDock {
    let edges = [
        (PaletteDock::Left, (rect.left() - viewport.left()).abs()),
        (PaletteDock::Right, (viewport.right() - rect.right()).abs()),
        (PaletteDock::Top, (rect.top() - viewport.top()).abs()),
        (
            PaletteDock::Bottom,
            (viewport.bottom() - rect.bottom()).abs(),
        ),
    ];
    edges
        .into_iter()
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .filter(|(_, distance)| *distance <= 24.0)
        .map_or(PaletteDock::Floating, |(dock, _)| dock)
}

/// Select a panel position that fits the viewport and avoids the tool palette
/// when there is room. Candidate order follows the palette edge, then falls
/// back to the location with the least overlap after clamping.
pub fn flyout_position(
    viewport: egui::Rect,
    palette: egui::Rect,
    panel_size: egui::Vec2,
    preferred: egui::Pos2,
) -> egui::Pos2 {
    let margin = 8.0;
    let max_size = egui::vec2(
        (viewport.width() - margin * 2.0).max(1.0),
        (viewport.height() - margin * 2.0).max(1.0),
    );
    let size = egui::vec2(panel_size.x.min(max_size.x), panel_size.y.min(max_size.y));
    let right = egui::pos2(palette.right() + margin, palette.top());
    let left = egui::pos2(palette.left() - size.x - margin, palette.top());
    let below = egui::pos2(palette.left(), palette.bottom() + margin);
    let above = egui::pos2(palette.left(), palette.top() - size.y - margin);
    let candidates = [preferred, right, left, below, above];
    let mut best = None;
    for candidate in candidates {
        let pos = clamp_position(viewport, size, candidate, margin);
        let rect = egui::Rect::from_min_size(pos, size);
        let overlap = rect.intersect(palette).area();
        if overlap <= 0.5 {
            return pos;
        }
        let visible = rect.intersect(viewport).area();
        let score = visible - overlap * 4.0;
        if best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((pos, score));
        }
    }
    best.map_or(
        egui::pos2(viewport.left() + margin, viewport.top() + margin),
        |(p, _)| p,
    )
}

/// Place a contextual panel without covering the movable palette or any
/// caller-supplied floating surfaces (for example the reference board). The
/// base placement preserves existing palette-aware behavior; additional
/// candidates move the panel around blockers before accepting overlap.
pub fn flyout_position_avoiding(
    viewport: egui::Rect,
    palette: egui::Rect,
    panel_size: egui::Vec2,
    preferred: egui::Pos2,
    obstacles: &[egui::Rect],
) -> egui::Pos2 {
    const MARGIN: f32 = 8.0;
    let size = egui::vec2(
        panel_size.x.min((viewport.width() - MARGIN * 2.0).max(1.0)),
        panel_size
            .y
            .min((viewport.height() - MARGIN * 2.0).max(1.0)),
    );
    let mut candidates = vec![
        flyout_position(viewport, palette, size, preferred),
        egui::pos2(viewport.right() - size.x - MARGIN, viewport.top() + MARGIN),
        egui::pos2(
            viewport.right() - size.x - MARGIN,
            viewport.bottom() - size.y - MARGIN,
        ),
        egui::pos2(viewport.left() + MARGIN, viewport.top() + MARGIN),
        egui::pos2(
            viewport.left() + MARGIN,
            viewport.bottom() - size.y - MARGIN,
        ),
    ];
    for obstacle in obstacles {
        candidates.extend([
            egui::pos2(obstacle.right() + MARGIN, obstacle.top()),
            egui::pos2(obstacle.left() - size.x - MARGIN, obstacle.top()),
            egui::pos2(obstacle.left(), obstacle.bottom() + MARGIN),
            egui::pos2(obstacle.left(), obstacle.top() - size.y - MARGIN),
        ]);
    }

    let mut best = None;
    for candidate in candidates {
        let position = clamp_position(viewport, size, candidate, MARGIN);
        let rect = egui::Rect::from_min_size(position, size);
        let overlap = rect.intersect(palette).area()
            + obstacles
                .iter()
                .map(|obstacle| rect.intersect(*obstacle).area())
                .sum::<f32>();
        if overlap <= 0.5 {
            return position;
        }
        let score = rect.intersect(viewport).area() - overlap * 5.0;
        if best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((position, score));
        }
    }
    best.map_or(
        egui::pos2(viewport.left() + MARGIN, viewport.top() + MARGIN),
        |(position, _)| position,
    )
}

/// Limit a floating panel to the free space beside a vertical palette. On a
/// narrow window the toolbar is made compact by the workspace, leaving a
/// usable strip for the panel instead of letting it open behind the tools.
pub fn flyout_size(viewport: egui::Rect, palette: egui::Rect, preferred: egui::Vec2) -> egui::Vec2 {
    let margin = 8.0;
    let max_width = (viewport.width() - margin * 2.0).max(1.0);
    // Keep room for the app's bottom canvas controls and status line.
    let max_height = (viewport.height() - margin * 2.0 - 64.0).max(120.0);
    let horizontal_palette = palette.width() > viewport.width() * 0.45;
    // `egui::Window` adds a title/frame around its content size; leave a small
    // allowance so the rendered outer edge also stays clear of the palette.
    let free_width = if horizontal_palette {
        (max_width - 32.0).max(1.0)
    } else {
        let right = viewport.right() - palette.right() - margin * 2.0;
        let left = palette.left() - viewport.left() - margin * 2.0;
        (right.max(left) - 32.0).clamp(1.0, max_width)
    };
    let free_height = if horizontal_palette {
        let below = viewport.bottom() - palette.bottom() - margin * 2.0;
        let above = palette.top() - viewport.top() - margin * 2.0;
        below.max(above).clamp(120.0, max_height)
    } else {
        max_height
    };
    egui::vec2(preferred.x.min(free_width), preferred.y.min(free_height))
}

/// Choose a useful brush-library width before the general viewport clamp.
/// Narrow windows prioritize keeping the palette and canvas discoverable; the
/// preset list remains scrollable inside that compact width.
pub fn brush_flyout_preferred_size(viewport_width: f32) -> egui::Vec2 {
    let width = if viewport_width < 760.0 {
        416.0
    } else if viewport_width >= 1600.0 {
        560.0
    } else {
        520.0
    };
    egui::vec2(width, 640.0)
}

/// Detect a saved flyout rectangle that exceeds its current viewport-derived
/// size. Egui retains window geometry for the life of a context, so changing
/// the desired size alone does not shrink an already-open panel.
pub fn flyout_size_exceeds(current: egui::Vec2, desired: egui::Vec2) -> bool {
    const FRAME_ALLOWANCE: f32 = 24.0;
    current.x > desired.x + FRAME_ALLOWANCE || current.y > desired.y + FRAME_ALLOWANCE
}

/// Constrained windows use a temporary compact layout for the currently open
/// Layers and Reference surfaces. The returned rectangles are deliberately
/// transient: callers use separate egui area IDs and never copy these smaller
/// rectangles into the saved large-window preferences.
pub fn compact_workspace_panel_rects(
    viewport: egui::Rect,
    palette: egui::Rect,
    preferred_sizes: &[egui::Vec2],
) -> Option<Vec<egui::Rect>> {
    let panel_count = preferred_sizes.len();
    if panel_count == 0 || !compact_workspace_mode(viewport) {
        return None;
    }

    const EDGE: f32 = 8.0;
    const GAP: f32 = 8.0;
    const RESERVED_BOTTOM: f32 = 40.0;
    let bounds = viewport.shrink(EDGE);
    let compact_multi_row = panel_count > 1
        && palette.left() <= viewport.left() + 120.0
        && palette.top() <= bounds.top() + 120.0
        && palette.bottom() <= bounds.top() + 160.0;
    let (left, top, right, bottom) = if compact_multi_row {
        // A short, left-docked palette occupies only the top strip in compact
        // mode. Use the full width beneath it so two useful companion panels
        // retain their real minimum widths instead of spilling into each
        // other in the narrow strip beside the toolbar.
        (
            bounds.left(),
            (palette.bottom() + GAP).min(bounds.bottom() - 1.0),
            bounds.right(),
            (bounds.bottom() - RESERVED_BOTTOM)
                .max(palette.bottom() + GAP + 1.0)
                .min(bounds.bottom()),
        )
    } else if palette.left() <= viewport.left() + 120.0 {
        (
            (palette.right() + GAP).min(bounds.right() - 1.0),
            (palette.top() - 8.0).max(bounds.top()),
            bounds.right(),
            (bounds.bottom() - RESERVED_BOTTOM).max(palette.top().max(bounds.top()) + 1.0),
        )
    } else if palette.right() >= viewport.right() - 120.0 {
        (
            bounds.left(),
            (palette.top() - 8.0).max(bounds.top()),
            (palette.left() - GAP).max(bounds.left() + 1.0),
            (bounds.bottom() - RESERVED_BOTTOM).max(palette.top().max(bounds.top()) + 1.0),
        )
    } else if palette.top() <= viewport.top() + 160.0 {
        (
            bounds.left(),
            (palette.bottom() + GAP).min(bounds.bottom() - 1.0),
            bounds.right(),
            bounds.bottom(),
        )
    } else if palette.bottom() >= viewport.bottom() - 120.0 {
        (
            bounds.left(),
            bounds.top(),
            bounds.right(),
            (palette.top() - GAP).max(bounds.top() + 1.0),
        )
    } else {
        (bounds.left(), bounds.top(), bounds.right(), bounds.bottom())
    };
    let work = egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom));
    if !work.is_positive() {
        return None;
    }

    let columns = panel_count.min(3);
    let rows = panel_count.div_ceil(columns);
    let cell_width = ((work.width() - GAP * (columns - 1) as f32) / columns as f32).max(1.0);
    let cell_height = ((work.height() - GAP * (rows - 1) as f32) / rows as f32).max(1.0);
    Some(
        (0..panel_count)
            .map(|index| {
                let column = index % columns;
                let row = index / columns;
                // Egui adds a frame around fixed content sizes. Leave room for
                // that outer extent so actual window rectangles do not collide
                // even when a panel reaches its minimum content width.
                let width_allowance = if panel_count == 1 {
                    0.0
                } else if cell_width >= 240.0 {
                    32.0
                } else {
                    18.0
                };
                let position = egui::pos2(
                    work.left() + column as f32 * (cell_width + GAP) + width_allowance * 0.5,
                    work.top() + row as f32 * (cell_height + GAP),
                );
                let size = if panel_count == 1 {
                    egui::vec2(
                        preferred_sizes[index].x.min(cell_width),
                        preferred_sizes[index].y.min(cell_height),
                    )
                } else {
                    egui::vec2((cell_width - width_allowance).max(1.0), cell_height)
                };
                egui::Rect::from_min_size(position, size)
            })
            .collect(),
    )
}

pub fn compact_workspace_mode(viewport: egui::Rect) -> bool {
    viewport.width() < 760.0 || viewport.height() < 560.0
}

/// Bound the recovery manager to the visible viewport and reserve room for
/// its title, explanation, and footer before sizing the scrollable checkpoint
/// list. This prevents populated recovery dialogs from extending beyond small
/// laptop windows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecoveryModalLimits {
    pub min_width: f32,
    pub max_size: egui::Vec2,
    pub list_height: f32,
}

pub fn recovery_modal_limits(viewport: egui::Rect) -> RecoveryModalLimits {
    let available = viewport.shrink(24.0).size();
    let max_size = egui::vec2(available.x.max(1.0), available.y.max(1.0));
    RecoveryModalLimits {
        min_width: max_size.x.min(520.0),
        max_size: egui::vec2(max_size.x.min(760.0), max_size.y),
        list_height: (max_size.y - 160.0).clamp(1.0, 360.0),
    }
}

/// Keep the saved dock preference intact while changing its temporary
/// presentation on narrow windows. Horizontal palettes leave too little room
/// for contextual controls and companion panels at laptop-like 640px widths.
pub fn effective_palette_dock(
    preferred: crate::workspace::PaletteDock,
    viewport: egui::Rect,
    dragging: bool,
) -> crate::workspace::PaletteDock {
    use crate::workspace::PaletteDock;
    if viewport.width() >= 760.0 || dragging {
        return preferred;
    }
    match preferred {
        PaletteDock::Right => PaletteDock::Right,
        PaletteDock::Floating => PaletteDock::Floating,
        PaletteDock::Left | PaletteDock::Top | PaletteDock::Bottom => PaletteDock::Left,
    }
}

fn clamp_position(
    viewport: egui::Rect,
    size: egui::Vec2,
    pos: egui::Pos2,
    margin: f32,
) -> egui::Pos2 {
    let max_x = (viewport.right() - margin - size.x).max(viewport.left() + margin);
    let max_y = (viewport.bottom() - margin - size.y).max(viewport.top() + margin);
    egui::pos2(
        pos.x.clamp(viewport.left() + margin, max_x),
        pos.y.clamp(viewport.top() + margin, max_y),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height))
    }

    #[test]
    fn default_brush_flyout_moves_away_from_palette() {
        let viewport = rect(0.0, 0.0, 1280.0, 800.0);
        let palette = rect(8.0, 64.0, 90.0, 420.0);
        let pos = flyout_position(
            viewport,
            palette,
            egui::vec2(360.0, 500.0),
            egui::pos2(104.0, 72.0),
        );
        let flyout = rect(pos.x, pos.y, 360.0, 500.0);
        assert!(!flyout.intersects(palette));
        assert!(viewport.contains_rect(flyout));
    }

    #[test]
    fn color_flyout_moves_clear_of_reference_board() {
        let viewport = rect(0.0, 0.0, 1280.0, 800.0);
        let palette = rect(8.0, 64.0, 90.0, 420.0);
        let reference = rect(47.0, 309.0, 360.0, 245.0);
        let size = egui::vec2(360.0, 620.0);
        let position = flyout_position_avoiding(
            viewport,
            palette,
            size,
            egui::pos2(104.0, 300.0),
            &[reference],
        );
        let panel = rect(position.x, position.y, size.x, size.y);
        assert!(viewport.contains_rect(panel));
        assert!(!panel.intersects(palette));
        assert!(!panel.intersects(reference));
    }

    #[test]
    fn oversized_saved_flyout_is_reset_but_small_manual_size_is_preserved() {
        assert!(flyout_size_exceeds(
            egui::vec2(646.0, 560.0),
            egui::vec2(520.0, 560.0)
        ));
        assert!(!flyout_size_exceeds(
            egui::vec2(500.0, 550.0),
            egui::vec2(520.0, 560.0)
        ));
    }

    #[test]
    fn brush_library_uses_compact_width_only_for_narrow_viewports() {
        assert_eq!(brush_flyout_preferred_size(640.0), egui::vec2(416.0, 640.0));
        assert_eq!(
            brush_flyout_preferred_size(1280.0),
            egui::vec2(520.0, 640.0)
        );
        assert_eq!(
            brush_flyout_preferred_size(1920.0),
            egui::vec2(560.0, 640.0)
        );
    }

    #[test]
    fn brush_flyout_keeps_a_reference_board_visible_when_both_fit() {
        let viewport = rect(0.0, 0.0, 1280.0, 800.0);
        let palette = rect(8.0, 132.0, 72.0, 500.0);
        let reference = rect(306.0, 100.0, 333.0, 330.0);
        let size = egui::vec2(520.0, 560.0);
        let position = flyout_position_avoiding(
            viewport,
            palette,
            size,
            egui::pos2(752.0, 12.0),
            &[reference],
        );
        let panel = rect(position.x, position.y, size.x, size.y);
        assert!(viewport.contains_rect(panel));
        assert!(!panel.intersects(palette));
        assert!(!panel.intersects(reference));
    }

    #[test]
    fn floating_panel_hit_testing_converts_physical_pixels_at_display_scale() {
        let panel = rect(100.0, 80.0, 240.0, 160.0);
        assert!(client_point_in_rect(panel, [400.0, 320.0], 2.0));
        assert!(!client_point_in_rect(panel, [190.0, 160.0], 2.0));
        assert!(!client_point_in_rect(panel, [400.0, 320.0], 0.0));
        assert!(!client_point_in_rect(panel, [f64::NAN, 320.0], 2.0));
        assert!(!client_point_in_rect(panel, [f64::MAX, 320.0], 0.5));
    }

    #[test]
    fn narrow_viewports_clamp_size_and_keep_panel_reachable() {
        for viewport in [rect(0.0, 0.0, 620.0, 460.0), rect(0.0, 0.0, 320.0, 240.0)] {
            let palette = rect(8.0, 64.0, 76.0, 150.0);
            let position = flyout_position(
                viewport,
                palette,
                egui::vec2(360.0, 500.0),
                egui::pos2(104.0, 72.0),
            );
            assert!(viewport.contains(position));
            let width = 360.0_f32.min(viewport.width() - 16.0);
            let height = 500.0_f32.min(viewport.height() - 16.0);
            assert!(viewport.contains_rect(rect(position.x, position.y, width, height)));
        }
    }

    #[test]
    fn compact_viewport_gives_brush_flyout_its_own_reachable_column() {
        let viewport = rect(0.0, 0.0, 640.0, 480.0);
        let palette = rect(8.0, 64.0, 58.0, 620.0);
        let size = flyout_size(viewport, palette, egui::vec2(620.0, 560.0));
        let position = flyout_position(
            viewport,
            palette,
            size,
            egui::pos2(palette.right() + 8.0, palette.top()),
        );
        let panel = rect(position.x, position.y, size.x, size.y);
        assert!(viewport.contains_rect(panel));
        assert!(!panel.intersects(palette));
        assert!(size.x >= 500.0);
        assert!(size.y <= 400.0);
    }

    #[test]
    fn horizontal_palette_reserves_the_open_canvas_area_for_flyouts() {
        let viewport = rect(0.0, 0.0, 1280.0, 800.0);
        let palette = rect(320.0, 64.0, 654.0, 114.0);
        let size = flyout_size(viewport, palette, egui::vec2(620.0, 560.0));
        let position = flyout_position(viewport, palette, size, egui::pos2(222.0, 64.0));
        let panel = rect(position.x, position.y, size.x, size.y);
        assert!(viewport.contains_rect(panel));
        assert!(!panel.intersects(palette));
        assert!(position.y >= palette.bottom());
        assert!(size.x >= 580.0);
    }

    #[test]
    fn compact_layers_and_reference_tiles_fit_without_destroying_saved_geometry() {
        let viewport = rect(0.0, 0.0, 640.0, 480.0);
        let palette = rect(8.0, 128.0, 58.0, 280.0);
        let panels = compact_workspace_panel_rects(
            viewport,
            palette,
            &[egui::vec2(420.0, 560.0), egui::vec2(360.0, 300.0)],
        )
        .unwrap();

        assert_eq!(panels.len(), 2);
        for panel in &panels {
            assert!(viewport.shrink(8.0).contains_rect(*panel));
            assert!(!panel.intersects(palette));
            assert!(panel.width() >= 240.0);
            assert!(panel.height() >= 200.0);
        }
        assert!(!panels[0].intersects(panels[1]));
        assert!(
            compact_workspace_panel_rects(
                rect(0.0, 0.0, 1280.0, 800.0),
                palette,
                &[egui::vec2(420.0, 560.0), egui::vec2(360.0, 300.0)],
            )
            .is_none()
        );
    }

    #[test]
    fn recovery_modal_limits_keep_the_checkpoint_list_inside_small_viewports() {
        let narrow = recovery_modal_limits(rect(0.0, 0.0, 640.0, 480.0));
        assert_eq!(narrow.max_size.x, 592.0);
        assert_eq!(narrow.max_size.y, 432.0);
        assert_eq!(narrow.list_height, 272.0);
        assert!(narrow.min_width <= narrow.max_size.x);

        let wide = recovery_modal_limits(rect(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(wide.max_size, egui::vec2(760.0, 1032.0));
        assert_eq!(wide.list_height, 360.0);

        let tiny = recovery_modal_limits(rect(0.0, 0.0, 320.0, 180.0));
        assert!(tiny.min_width <= tiny.max_size.x);
        assert!(tiny.list_height <= tiny.max_size.y);
    }

    #[test]
    fn compact_layout_handles_docked_palettes_and_multiple_panels() {
        let viewport = rect(0.0, 0.0, 640.0, 480.0);
        let right_palette = rect(574.0, 128.0, 58.0, 280.0);
        let panels = compact_workspace_panel_rects(
            viewport,
            right_palette,
            &[
                egui::vec2(420.0, 560.0),
                egui::vec2(360.0, 300.0),
                egui::vec2(360.0, 300.0),
            ],
        )
        .unwrap();
        assert_eq!(panels.len(), 3);
        assert!(panels.iter().all(|panel| viewport.contains_rect(*panel)));
        assert!(panels.iter().all(|panel| !panel.intersects(right_palette)));
        for (index, panel) in panels.iter().enumerate() {
            for other in panels.iter().skip(index + 1) {
                assert!(!panel.intersects(*other));
            }
        }
    }

    #[test]
    fn narrow_viewport_adapts_horizontal_docks_without_rewriting_preferences() {
        use crate::workspace::PaletteDock;
        let narrow = rect(0.0, 0.0, 640.0, 480.0);
        assert_eq!(
            effective_palette_dock(PaletteDock::Top, narrow, false),
            PaletteDock::Left
        );
        assert_eq!(
            effective_palette_dock(PaletteDock::Bottom, narrow, false),
            PaletteDock::Left
        );
        assert_eq!(
            effective_palette_dock(PaletteDock::Right, narrow, false),
            PaletteDock::Right
        );
        assert_eq!(
            effective_palette_dock(PaletteDock::Top, narrow, true),
            PaletteDock::Top
        );
        assert_eq!(
            effective_palette_dock(PaletteDock::Top, rect(0.0, 0.0, 1280.0, 800.0), false),
            PaletteDock::Top
        );
    }

    #[test]
    fn user_placement_survives_when_it_is_valid_and_clear() {
        let viewport = rect(0.0, 0.0, 1200.0, 800.0);
        let palette = rect(8.0, 64.0, 90.0, 300.0);
        let pos = flyout_position(
            viewport,
            palette,
            egui::vec2(340.0, 420.0),
            egui::pos2(700.0, 160.0),
        );
        assert_eq!(pos, egui::pos2(700.0, 160.0));
    }

    #[test]
    fn palette_docks_to_all_four_edges_and_floats_between_them() {
        let viewport = rect(0.0, 0.0, 1280.0, 800.0);
        for (bounds, expected) in [
            (rect(4.0, 120.0, 72.0, 400.0), PaletteDock::Left),
            (rect(1204.0, 120.0, 72.0, 400.0), PaletteDock::Right),
            (rect(400.0, 4.0, 480.0, 72.0), PaletteDock::Top),
            (rect(400.0, 720.0, 480.0, 72.0), PaletteDock::Bottom),
            (rect(400.0, 300.0, 480.0, 72.0), PaletteDock::Floating),
        ] {
            assert_eq!(snap_palette(viewport, bounds), expected);
        }
    }

    #[test]
    fn edge_anchors_center_horizontal_palettes_and_honor_handed_sides() {
        let viewport = rect(0.0, 0.0, 1280.0, 800.0);
        assert_eq!(
            palette_anchor(viewport, PaletteDock::Top, egui::Pos2::ZERO),
            egui::pos2(320.0, 8.0)
        );
        assert_eq!(
            palette_anchor(viewport, PaletteDock::Bottom, egui::Pos2::ZERO),
            egui::pos2(320.0, 728.0)
        );
        assert_eq!(
            palette_anchor(viewport, PaletteDock::Left, egui::Pos2::ZERO),
            egui::pos2(8.0, 8.0)
        );
        assert_eq!(
            palette_anchor(viewport, PaletteDock::Right, egui::Pos2::ZERO),
            egui::pos2(1208.0, 8.0)
        );
        let narrow = rect(0.0, 0.0, 320.0, 240.0);
        assert_eq!(
            palette_anchor(narrow, PaletteDock::Top, egui::Pos2::ZERO),
            egui::pos2(8.0, 8.0)
        );
        assert_eq!(
            palette_anchor(narrow, PaletteDock::Bottom, egui::Pos2::ZERO),
            egui::pos2(8.0, 168.0)
        );
    }

    #[test]
    fn docked_palette_uses_the_canvas_edge_below_app_chrome() {
        let canvas = rect(0.0, 126.0, 1280.0, 800.0);
        assert_eq!(
            palette_anchor(canvas, PaletteDock::Top, egui::Pos2::ZERO),
            egui::pos2(320.0, 134.0)
        );
        assert_eq!(
            palette_anchor(canvas, PaletteDock::Left, egui::Pos2::ZERO),
            egui::pos2(8.0, 134.0)
        );
        assert_eq!(
            snap_palette(canvas, rect(320.0, 130.0, 640.0, 56.0)),
            PaletteDock::Top
        );
    }
}
