//! Small, deterministic helpers for placing floating studio panels.

use crate::workspace::PaletteDock;

/// Position a docked tool palette with comfortable clearance from the app menu
/// and status bars. Top and bottom docks are centered and use a horizontal row.
pub fn palette_anchor(viewport: egui::Rect, dock: PaletteDock, floating: egui::Pos2) -> egui::Pos2 {
    let margin = 8.0;
    let right_anchor = (viewport.right() - 72.0).max(viewport.left() + margin);
    let horizontal_anchor = (viewport.center().x - 320.0).clamp(
        viewport.left() + margin,
        (viewport.right() - 656.0).max(viewport.left() + margin),
    );
    match dock {
        PaletteDock::Left => egui::pos2(viewport.left() + margin, viewport.top() + 64.0),
        PaletteDock::Right => egui::pos2(right_anchor, viewport.top() + 64.0),
        PaletteDock::Top => egui::pos2(horizontal_anchor, viewport.top() + 64.0),
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
        (PaletteDock::Top, (rect.top() - viewport.top() - 56.0).abs()),
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
            (rect(400.0, 60.0, 480.0, 72.0), PaletteDock::Top),
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
            egui::pos2(320.0, 64.0)
        );
        assert_eq!(
            palette_anchor(viewport, PaletteDock::Bottom, egui::Pos2::ZERO),
            egui::pos2(320.0, 728.0)
        );
        assert_eq!(
            palette_anchor(viewport, PaletteDock::Left, egui::Pos2::ZERO),
            egui::pos2(8.0, 64.0)
        );
        assert_eq!(
            palette_anchor(viewport, PaletteDock::Right, egui::Pos2::ZERO),
            egui::pos2(1208.0, 64.0)
        );
        let narrow = rect(0.0, 0.0, 320.0, 240.0);
        assert_eq!(
            palette_anchor(narrow, PaletteDock::Top, egui::Pos2::ZERO),
            egui::pos2(8.0, 64.0)
        );
        assert_eq!(
            palette_anchor(narrow, PaletteDock::Bottom, egui::Pos2::ZERO),
            egui::pos2(8.0, 168.0)
        );
    }
}
