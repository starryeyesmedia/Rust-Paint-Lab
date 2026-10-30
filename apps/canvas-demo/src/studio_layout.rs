//! Small, deterministic helpers for placing floating studio panels.

use crate::workspace::PaletteDock;

/// Minimum canvas viewport kept beside the desktop Inspector. Smaller windows
/// use an overlay so opening tools does not squeeze the document to a sliver.
pub const MIN_CANVAS_WIDTH: f32 = 320.0;
pub const MIN_CANVAS_HEIGHT: f32 = 240.0;
pub const MIN_INSPECTOR_DOCK_WIDTH: f32 = 280.0;
pub const STANDARD_INSPECTOR_DOCK_WIDTH: f32 = 360.0;
pub const MAX_INSPECTOR_DOCK_WIDTH: f32 = 400.0;
pub const DESKTOP_DOCK_MIN_WIDTH: f32 = 760.0;
pub const DESKTOP_DOCK_MIN_HEIGHT: f32 = 500.0;
const COMPACT_SHEET_MAX_WIDTH: f32 = 480.0;
const COMPACT_SHEET_MAX_HEIGHT: f32 = 240.0;
const COMPACT_MARGIN: f32 = 8.0;
const COMPACT_INSPECTOR_MIN_WIDTH: f32 = 180.0;
/// Leave a small additional area-constraint allowance for egui's resizable
/// Window response, whose outer drag region extends beyond the painted frame.
const COMPACT_WINDOW_AREA_SLACK: f32 = 14.0;
const COMPACT_WINDOW_EDGE_SLACK: f32 = 7.0;
/// Compact Inspectors float over the canvas, so they may overlap part of the
/// document viewport while keeping a useful drawing strip beside the palette.
/// At 640 points this reserves enough room for the approved 256-point first-run
/// drawer while accounting separately for the palette, window margins, and the
/// Inspector's resize gutter. This is intentionally smaller than the desktop
/// dock's 320-point canvas-width minimum.
const COMPACT_CANVAS_MIN_WIDTH: f32 = 256.0;
/// egui's resizable Window reserves a 12pt interaction gutter beside the
/// painted frame. Account for it when protecting the canvas strip.
const COMPACT_RESIZE_HANDLE_GUTTER: f32 = 12.0;

/// Let the Inspector's invisible resize interaction region approach the
/// window edge by seven points on the side opposite the toolbar. This keeps
/// the painted drawer inside the window while avoiding wasted canvas width.
pub fn compact_inspector_constraint(
    viewport: egui::Rect,
    palette: egui::Rect,
    force_edge_reflow: bool,
) -> egui::Rect {
    let mut safe = viewport.shrink(COMPACT_MARGIN);
    if force_edge_reflow && palette.center().x < viewport.center().x {
        safe.max.x += COMPACT_WINDOW_EDGE_SLACK;
    } else if force_edge_reflow && palette.center().x > viewport.center().x {
        safe.min.x -= COMPACT_WINDOW_EDGE_SLACK;
    }
    safe.intersect(viewport)
}

/// Place the selected reference as a small canvas-only picture-in-picture.
/// The requested image footprint is 192×144 points and remains capped at
/// 24% of the safe canvas region; tiny viewports intentionally return `None`
/// instead of obscuring most of the artwork.
pub fn reference_picture_in_picture(
    safe_canvas: egui::Rect,
    preferred_position: [f32; 2],
) -> Option<egui::Rect> {
    if !safe_canvas.is_positive() || !preferred_position.iter().all(|value| value.is_finite()) {
        return None;
    }
    const EDGE: f32 = 8.0;
    let max_width = (safe_canvas.width() * 0.24).min(320.0);
    let max_height = (safe_canvas.height() * 0.24).min(240.0);
    let width = 192.0_f32.min(max_width);
    let height = 144.0_f32.min(max_height);
    if width < 160.0 || height < 120.0 {
        return None;
    }
    let size = egui::vec2(width, height);
    let max_position = safe_canvas.max - size - egui::vec2(EDGE, EDGE);
    let position = egui::pos2(
        preferred_position[0].clamp(safe_canvas.left() + EDGE, max_position.x),
        preferred_position[1].clamp(safe_canvas.top() + EDGE, max_position.y),
    );
    Some(egui::Rect::from_min_size(position, size))
}

/// Return a bounded dock width only when the remaining canvas can retain its
/// minimum usable width. The caller uses a compact overlay otherwise.
pub fn inspector_dock_width(viewport: egui::Rect, preferred: f32) -> Option<f32> {
    if !preferred.is_finite()
        || viewport.width() < DESKTOP_DOCK_MIN_WIDTH
        || viewport.height() < DESKTOP_DOCK_MIN_HEIGHT
        || viewport.width() < MIN_CANVAS_WIDTH + MIN_INSPECTOR_DOCK_WIDTH
        || viewport.height() < MIN_CANVAS_HEIGHT
    {
        return None;
    }
    let maximum = inspector_max_dock_width(viewport);
    Some(preferred.clamp(MIN_INSPECTOR_DOCK_WIDTH, maximum))
}

/// Match the maximum user-resizable Inspector width to the current desktop
/// breakpoint. A saved width may remain larger, but it is only adapted for the
/// current viewport and never squeezes the canvas past its minimum.
pub fn inspector_max_dock_width(viewport: egui::Rect) -> f32 {
    let viewport_maximum = if viewport.width() >= 1600.0 {
        MAX_INSPECTOR_DOCK_WIDTH
    } else {
        STANDARD_INSPECTOR_DOCK_WIDTH
    };
    (viewport.width() - MIN_CANVAS_WIDTH).min(viewport_maximum)
}

/// Compact windows below the effective point-size threshold use a full-sheet
/// Inspector. The persistent Canvas return action restores the unobstructed
/// drawing surface without changing document or tool state.
pub fn inspector_uses_full_sheet(viewport: egui::Rect) -> bool {
    viewport.width() < COMPACT_SHEET_MAX_WIDTH || viewport.height() < COMPACT_SHEET_MAX_HEIGHT
}

/// Keep floating Inspector overlays clear of the application menu, contextual
/// tool row, and bottom status controls. The caller supplies the protected
/// top edge in screen coordinates and the bottom chrome height. When the
/// window is extremely short, this leaves the largest possible usable region
/// while keeping the overlay inside the window.
pub fn inspector_safe_viewport(
    viewport: egui::Rect,
    protected_top: f32,
    protected_bottom: f32,
) -> egui::Rect {
    let top = protected_top
        .max(viewport.top())
        .min(viewport.top() + viewport.height() * 0.45);
    let bottom = (viewport.bottom() - protected_bottom.max(0.0))
        .max(top + 1.0)
        .min(viewport.bottom());
    egui::Rect::from_min_max(
        egui::pos2(viewport.left(), top),
        egui::pos2(viewport.right(), bottom),
    )
}

/// Bound the compact Inspector to its safe viewport. A full sheet uses the
/// whole safe region; a drawer uses the saved preferred size where it fits.
pub fn inspector_compact_size(viewport: egui::Rect, preferred: egui::Vec2) -> egui::Vec2 {
    let safe_size = viewport.shrink(COMPACT_MARGIN).size().max(egui::Vec2::ZERO);
    if inspector_uses_full_sheet(viewport) {
        return safe_size;
    }
    egui::vec2(
        preferred.x.clamp(220.0, 800.0).min(safe_size.x),
        preferred.y.clamp(180.0, 1400.0).min(safe_size.y),
    )
}

/// Narrow the compact drawer only as much as needed to preserve the requested
/// unobstructed canvas strip beside the movable tool rail. If even the minimum
/// usable drawer cannot fit, the caller should show a full-height sheet.
pub fn inspector_drawer_width(
    viewport: egui::Rect,
    palette_width: f32,
    preferred_width: f32,
) -> Option<f32> {
    if !palette_width.is_finite() || palette_width < 0.0 || !preferred_width.is_finite() {
        return None;
    }
    let available = viewport.width()
        - palette_width
        - COMPACT_MARGIN * 2.0
        - COMPACT_CANVAS_MIN_WIDTH
        - COMPACT_RESIZE_HANDLE_GUTTER
        - COMPACT_WINDOW_AREA_SLACK;
    (available >= COMPACT_INSPECTOR_MIN_WIDTH).then_some(
        preferred_width
            .clamp(COMPACT_INSPECTOR_MIN_WIDTH, 800.0)
            .min(available),
    )
}

/// A compact drawer may need a temporary viewport-sized width even when its
/// previously saved egui window is wider. Only persist a resized drawer when
/// the current viewport can actually retain the requested preference.
pub fn compact_drawer_can_persist_size(
    viewport: egui::Rect,
    palette_width: f32,
    preferred_width: f32,
) -> bool {
    inspector_drawer_width(viewport, palette_width, preferred_width)
        .is_some_and(|available| available + 0.5 >= preferred_width)
}

/// egui's Window keeps an existing Area size when only `max_size` changes.
/// Detect that stale geometry so the caller can apply one temporary fixed-size
/// frame and then return the window to its normal resizable behavior.
pub fn compact_window_exceeds_target(current: egui::Vec2, target: egui::Vec2) -> bool {
    current.x > target.x + 1.0 || current.y > target.y + 1.0
}

/// Place a compact Inspector sheet inside the window and away from the
/// movable toolbar where possible. It overlays the existing canvas viewport;
/// it never persists this temporary size back to the roomy dock preference.
pub fn inspector_overlay_rect(
    viewport: egui::Rect,
    palette: egui::Rect,
    preferred_size: egui::Vec2,
) -> egui::Rect {
    const MARGIN: f32 = COMPACT_MARGIN;
    let size = egui::vec2(
        preferred_size
            .x
            .min((viewport.width() - MARGIN * 2.0).max(1.0)),
        preferred_size
            .y
            .min((viewport.height() - MARGIN * 2.0).max(1.0)),
    );
    let positions = [
        egui::pos2(viewport.right() - size.x - MARGIN, viewport.top() + MARGIN),
        egui::pos2(viewport.left() + MARGIN, viewport.top() + MARGIN),
        egui::pos2(
            viewport.right() - size.x - MARGIN,
            viewport.bottom() - size.y - MARGIN,
        ),
        egui::pos2(
            viewport.left() + MARGIN,
            viewport.bottom() - size.y - MARGIN,
        ),
    ];
    let mut best = None;
    for position in positions {
        let rect =
            egui::Rect::from_min_size(clamp_position(viewport, size, position, MARGIN), size);
        let overlap = rect.intersect(palette).area();
        if overlap <= 0.5 {
            return rect;
        }
        let score = rect.intersect(viewport).area() - overlap * 5.0;
        if best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((rect, score));
        }
    }
    best.map_or_else(
        || egui::Rect::from_min_size(viewport.min + egui::vec2(MARGIN, MARGIN), size),
        |(rect, _)| rect,
    )
}

/// Preserve a valid compact Inspector placement unless a temporary responsive
/// size reduction or collision requires edge reflow. In that case, choose a
/// clear position that maximizes the remaining canvas beside the palette.
pub fn compact_inspector_rect(
    viewport: egui::Rect,
    palette: egui::Rect,
    size: egui::Vec2,
    saved_position: egui::Pos2,
    force_edge_reflow: bool,
) -> egui::Rect {
    let saved = egui::Rect::from_min_size(saved_position, size);
    if !force_edge_reflow
        && viewport.shrink(COMPACT_MARGIN).contains_rect(saved)
        && !saved.intersects(palette)
    {
        saved
    } else {
        inspector_overlay_rect(viewport, palette, size)
    }
}

/// Keep pinned reference windows within the unobstructed part of the live
/// canvas. The returned region is temporary layout geometry; callers should
/// not write its clamped coordinates back to saved reference preferences.
pub fn pinned_reference_viewport(
    canvas: egui::Rect,
    inspector: Option<egui::Rect>,
    palette: Option<egui::Rect>,
) -> egui::Rect {
    const GAP: f32 = 8.0;
    let mut safe = canvas;
    if let Some(inspector) = inspector.filter(|panel| safe.intersects(*panel)) {
        if inspector.center().x >= safe.center().x {
            safe.max.x = safe.max.x.min(inspector.left() - GAP);
        } else {
            safe.min.x = safe.min.x.max(inspector.right() + GAP);
        }
    }
    if let Some(palette) = palette.filter(|panel| safe.intersects(*panel)) {
        if palette.center().x <= safe.center().x {
            safe.min.x = safe.min.x.max(palette.right() + GAP);
        } else {
            safe.max.x = safe.max.x.min(palette.left() - GAP);
        }
    }
    if safe.is_positive() { safe } else { canvas }
}

/// Whether a reference window's current position is an artist preference or
/// merely a temporary responsive/pinned presentation. A pin-state transition
/// can move an egui window in the same frame as the pointer activates the pin
/// control; that geometry must never be mistaken for a deliberate drag.
pub fn should_persist_reference_position(
    presentation_transition: bool,
    pointer_dragged: bool,
    explicitly_repositioned: bool,
) -> bool {
    explicitly_repositioned || (!presentation_transition && pointer_dragged)
}

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

/// Hit-test the only floating reference surface that is still interactive:
/// the explicitly pinned picture-in-picture. egui retains Area geometry after
/// an Area is no longer shown, so callers must also pass its current visibility
/// before using that stale-safe rectangle for canvas input routing.
pub fn pinned_reference_hit(
    pinned: bool,
    visible: bool,
    area: Option<egui::Rect>,
    client_px: [f64; 2],
    pixels_per_point: f32,
) -> bool {
    pinned
        && visible
        && area.is_some_and(|rect| client_point_in_rect(rect, client_px, pixels_per_point))
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
    fn inspector_dock_preserves_minimum_canvas_space_and_clamps_width() {
        let wide = rect(0.0, 0.0, 1280.0, 800.0);
        assert_eq!(inspector_dock_width(wide, 336.0), Some(336.0));
        assert_eq!(inspector_dock_width(wide, 480.0), Some(360.0));
        let large = rect(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(inspector_dock_width(large, 480.0), Some(400.0));
        assert_eq!(inspector_max_dock_width(large), 400.0);
        let constrained = rect(0.0, 0.0, 800.0, 600.0);
        let width = inspector_dock_width(constrained, 480.0).unwrap();
        assert_eq!(width, 360.0);
        assert!(constrained.width() - width >= MIN_CANVAS_WIDTH);
        assert_eq!(
            inspector_dock_width(rect(0.0, 0.0, 599.0, 480.0), 336.0),
            None
        );
        assert_eq!(
            inspector_dock_width(rect(0.0, 0.0, 640.0, 480.0), 336.0),
            None
        );
        assert_eq!(
            inspector_dock_width(rect(0.0, 0.0, 1280.0, 499.0), 336.0),
            None
        );
        assert_eq!(
            inspector_dock_width(rect(0.0, 0.0, 900.0, 200.0), 336.0),
            None
        );
        assert_eq!(inspector_dock_width(wide, f32::NAN), None);
    }

    #[test]
    fn compact_inspector_switches_to_a_full_sheet_only_when_effective_space_requires_it() {
        let narrow_100 = rect(0.0, 0.0, 640.0, 480.0);
        assert!(!inspector_uses_full_sheet(narrow_100));
        assert_eq!(
            inspector_compact_size(narrow_100, egui::vec2(256.0, 420.0)),
            egui::vec2(256.0, 420.0)
        );

        let narrow_150 = rect(0.0, 0.0, 426.0, 320.0);
        assert!(inspector_uses_full_sheet(narrow_150));
        assert_eq!(
            inspector_compact_size(narrow_150, egui::vec2(256.0, 420.0)),
            egui::vec2(410.0, 304.0)
        );

        let laptop_150 = rect(0.0, 0.0, 853.0, 533.0);
        assert!(!inspector_uses_full_sheet(laptop_150));

        // The 100% 640×480 workspace's usable region is shorter than the
        // nominal window once menus and status bars are allocated, but still
        // has room for a compact drawer and a useful canvas strip.
        assert!(!inspector_uses_full_sheet(rect(0.0, 108.0, 640.0, 324.0)));
        assert_eq!(
            inspector_drawer_width(rect(0.0, 108.0, 640.0, 324.0), 70.5, 256.0,),
            Some(256.0)
        );
        assert_eq!(
            inspector_drawer_width(rect(0.0, 0.0, 548.0, 400.0), 70.5, 256.0),
            None
        );
        // At 150% the effective canvas is too small even after app chrome.
        assert!(inspector_uses_full_sheet(rect(0.0, 72.0, 426.0, 216.0)));
    }

    #[test]
    fn compact_inspector_sheet_stays_in_bounds_and_moves_around_toolbar() {
        let viewport = rect(0.0, 0.0, 640.0, 480.0);
        let left_palette = rect(8.0, 80.0, 56.0, 320.0);
        let size = egui::vec2(300.0, 420.0);
        let sheet = inspector_overlay_rect(viewport, left_palette, size);
        assert!(viewport.shrink(8.0).contains_rect(sheet));
        assert!(!sheet.intersects(left_palette));
        assert_eq!(sheet.size(), size);

        let right_palette = rect(576.0, 80.0, 56.0, 320.0);
        let opposite = inspector_overlay_rect(viewport, right_palette, size);
        assert!(viewport.shrink(8.0).contains_rect(opposite));
        assert!(!opposite.intersects(right_palette));
        assert!(opposite.left() < viewport.center().x);

        let tiny = inspector_overlay_rect(
            rect(0.0, 0.0, 320.0, 180.0),
            rect(8.0, 60.0, 48.0, 90.0),
            egui::vec2(400.0, 600.0),
        );
        assert!(rect(0.0, 0.0, 320.0, 180.0).contains_rect(tiny));
    }

    #[test]
    fn temporarily_narrowed_inspector_reflows_to_preserve_the_canvas_strip() {
        let viewport = rect(0.0, 124.0, 640.0, 276.0);
        let palette = rect(8.0, 124.0, 70.5, 276.0);
        let size = egui::vec2(221.5, 248.0);
        let saved_position = egui::pos2(382.0, 124.0);
        let compact = compact_inspector_rect(viewport, palette, size, saved_position, true);

        assert!(viewport.shrink(COMPACT_MARGIN).contains_rect(compact));
        assert!(!compact.intersects(palette));
        assert!(compact.left() > saved_position.x);
        let painted_left = compact.left() + COMPACT_RESIZE_HANDLE_GUTTER;
        assert!(painted_left - palette.right() >= COMPACT_CANVAS_MIN_WIDTH - 1.0);

        let restored = compact_inspector_rect(viewport, palette, size, compact.min, false);
        assert_eq!(restored, compact);

        let constraint = compact_inspector_constraint(viewport, palette, true);
        assert_eq!(constraint.right(), viewport.right() - 1.0);
        assert!(viewport.contains_rect(constraint));
        assert_eq!(
            compact_inspector_constraint(viewport, palette, false).right(),
            viewport.right() - COMPACT_MARGIN
        );
    }

    #[test]
    fn compact_drawer_adapts_temporarily_without_replacing_roomy_preferences() {
        let usable = rect(0.0, 108.0, 640.0, 324.0);
        assert_eq!(inspector_drawer_width(usable, 70.5, 360.0), Some(271.5));
        assert!(!compact_drawer_can_persist_size(usable, 70.5, 360.0));
        assert!(compact_drawer_can_persist_size(
            rect(0.0, 108.0, 1280.0, 620.0),
            70.5,
            360.0
        ));
        assert!(compact_window_exceeds_target(
            egui::vec2(305.0, 239.0),
            egui::vec2(221.5, 240.0)
        ));
        assert!(!compact_window_exceeds_target(
            egui::vec2(221.5, 239.0),
            egui::vec2(221.5, 240.0)
        ));
    }

    #[test]
    fn compact_inspector_avoids_tool_chrome_and_status_controls() {
        let screen = rect(0.0, 0.0, 640.0, 480.0);
        let toolbar = rect(8.0, 132.0, 70.0, 270.0);
        let safe = inspector_safe_viewport(screen, 124.0, 96.0);
        let size = inspector_compact_size(safe, egui::vec2(256.0, 420.0));
        let overlay = inspector_overlay_rect(safe, toolbar, size);

        assert!(safe.top() >= 124.0);
        assert!(safe.bottom() <= 384.0);
        assert!(safe.shrink(8.0).contains_rect(overlay));
        assert!(!overlay.intersects(toolbar));
        assert!(overlay.top() >= 124.0);
        assert!(overlay.bottom() <= 384.0);
        assert!(size.y < 300.0, "saved height should adapt temporarily");
    }

    #[test]
    fn hidden_or_unpinned_reference_areas_never_block_canvas_input() {
        let stale_area = rect(600.0, 100.0, 320.0, 260.0);
        let point = [640.0, 200.0];

        assert!(!pinned_reference_hit(
            false,
            false,
            Some(stale_area),
            point,
            1.0
        ));
        assert!(!pinned_reference_hit(
            true,
            false,
            Some(stale_area),
            point,
            1.0
        ));
        assert!(!pinned_reference_hit(true, true, None, point, 1.0));
        assert!(pinned_reference_hit(
            true,
            true,
            Some(stale_area),
            point,
            1.0
        ));
        assert!(!pinned_reference_hit(
            true,
            true,
            Some(stale_area),
            [500.0, 200.0],
            1.0
        ));
    }

    #[test]
    fn pinned_reference_viewport_excludes_inspector_and_tool_palette() {
        let canvas = rect(70.0, 108.0, 930.0, 619.0);
        let inspector = rect(700.0, 108.0, 300.0, 619.0);
        let palette = rect(70.0, 160.0, 64.0, 360.0);
        let safe = pinned_reference_viewport(canvas, Some(inspector), Some(palette));
        assert!(canvas.contains_rect(safe));
        assert!(!safe.intersects(inspector));
        assert!(!safe.intersects(palette));
        assert!((safe.left() - 142.0).abs() < f32::EPSILON);
        assert!((safe.right() - 692.0).abs() < f32::EPSILON);

        let right_palette = rect(640.0, 160.0, 64.0, 360.0);
        let right_safe = pinned_reference_viewport(canvas, None, Some(right_palette));
        assert!(!right_safe.intersects(right_palette));
        assert!((right_safe.right() - 632.0).abs() < f32::EPSILON);
    }

    #[test]
    fn reference_picture_in_picture_is_bounded_to_safe_canvas_and_screen_edges() {
        let safe = rect(100.0, 40.0, 1000.0, 700.0);
        let picture = reference_picture_in_picture(safe, [900.0, 700.0]).unwrap();
        assert!(safe.contains_rect(picture));
        assert_eq!(picture.size(), egui::vec2(192.0, 144.0));
        assert!(picture.width() <= safe.width() * 0.24 + 0.1);
        assert!(picture.height() <= safe.height() * 0.24 + 0.1);

        let narrow = rect(0.0, 0.0, 640.0, 480.0);
        assert!(reference_picture_in_picture(narrow, [0.0, 0.0]).is_none());
        let invalid = rect(0.0, 0.0, 900.0, 600.0);
        assert!(reference_picture_in_picture(invalid, [f32::NAN, 4.0]).is_none());
    }

    #[test]
    fn reference_positions_ignore_pin_transition_but_keep_real_moves() {
        assert!(!should_persist_reference_position(true, true, false,));
        assert!(should_persist_reference_position(false, true, false,));
        assert!(should_persist_reference_position(true, false, true,));
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
