//! Configurable native pen barrel-button routing, independent of WM_POINTER.
//! The Windows adapter remains responsible for capturing the real button bit.

use paint_input::{Phase, PointerSample, Tool};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
const MAX_TRACKED_POINTERS: usize = paint_input::lifecycle::MAX_CONTACTS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BarrelAction {
    /// Hold to erase, then resume the selected painting tool.
    #[default]
    Eraser,
    /// Hold to sample the flattened canvas color under the pen.
    Eyedropper,
    /// Hold to pan the canvas with pen movement.
    Pan,
    /// Preserve the input button state without assigning an action.
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BarrelRoute {
    Forward,
    Eraser,
    SwitchToEraser,
    RestoreTool,
    PickColor,
    Pan(Option<[f64; 2]>),
    Suppress,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BarrelEffect {
    None,
    ColorPicked([u8; 3]),
    ColorUnavailable,
    Pan([f64; 2]),
}

#[derive(Debug, Default)]
pub struct BarrelRouter {
    contacts: BTreeSet<u64>,
    held: BTreeSet<u64>,
    suppressed: BTreeSet<u64>,
    pan_positions: BTreeMap<u64, [f64; 2]>,
}

impl BarrelRouter {
    pub fn route(&mut self, action: BarrelAction, sample: &PointerSample) -> BarrelRoute {
        if sample.tool != Tool::Pen {
            return BarrelRoute::Forward;
        }
        let id = sample.id;
        if sample.phase == Phase::Down {
            // A reused native pointer ID starts with no stale button/contact
            // state, even if an earlier device sequence ended unexpectedly.
            self.remove(id);
            let tracked: BTreeSet<_> = self
                .contacts
                .iter()
                .chain(self.held.iter())
                .chain(self.suppressed.iter())
                .copied()
                .collect();
            if tracked.len() >= MAX_TRACKED_POINTERS {
                return BarrelRoute::Suppress;
            }
        }
        if !paint_input::lifecycle::valid_sample(sample) {
            self.remove(id);
            return BarrelRoute::Forward;
        }
        if sample.phase == Phase::Cancel {
            self.remove(id);
            return BarrelRoute::Forward;
        }
        let contact_was_active = self.contacts.contains(&id);
        let button_was_held = self.held.contains(&id);
        let was_suppressed = self.suppressed.contains(&id);
        let terminal = sample.phase == Phase::Up;

        let route = if was_suppressed && !sample.buttons.barrel {
            self.held.remove(&id);
            self.pan_positions.remove(&id);
            if terminal {
                self.remove(id);
                BarrelRoute::Forward
            } else {
                BarrelRoute::Suppress
            }
        } else {
            match action {
                BarrelAction::Disabled => {
                    self.held.remove(&id);
                    self.pan_positions.remove(&id);
                    BarrelRoute::Forward
                }
                BarrelAction::Eraser if sample.buttons.barrel => {
                    self.held.insert(id);
                    if button_was_held {
                        BarrelRoute::Eraser
                    } else if contact_was_active {
                        BarrelRoute::SwitchToEraser
                    } else {
                        BarrelRoute::Eraser
                    }
                }
                BarrelAction::Eraser if button_was_held => {
                    self.held.remove(&id);
                    if contact_was_active && !terminal {
                        BarrelRoute::RestoreTool
                    } else {
                        BarrelRoute::Forward
                    }
                }
                BarrelAction::Eyedropper if sample.buttons.barrel => {
                    self.held.insert(id);
                    self.contacts.remove(&id);
                    if sample.in_contact || contact_was_active {
                        self.suppressed.insert(id);
                    }
                    BarrelRoute::PickColor
                }
                BarrelAction::Eyedropper if button_was_held => {
                    self.held.remove(&id);
                    self.pan_positions.remove(&id);
                    self.contacts.remove(&id);
                    if sample.in_contact || contact_was_active {
                        self.suppressed.insert(id);
                    }
                    if terminal {
                        BarrelRoute::Forward
                    } else {
                        BarrelRoute::Suppress
                    }
                }
                BarrelAction::Pan if sample.buttons.barrel => {
                    self.held.insert(id);
                    self.contacts.remove(&id);
                    if sample.in_contact || contact_was_active {
                        self.suppressed.insert(id);
                    }
                    let current = sample.position_px;
                    if !current
                        .iter()
                        .all(|value| value.is_finite() && value.abs() <= 32768.0)
                    {
                        self.pan_positions.remove(&id);
                        BarrelRoute::Pan(None)
                    } else {
                        let previous = self.pan_positions.insert(id, current);
                        BarrelRoute::Pan(previous.map(|previous| {
                            [
                                (current[0] - previous[0]).clamp(-4096.0, 4096.0),
                                (current[1] - previous[1]).clamp(-4096.0, 4096.0),
                            ]
                        }))
                    }
                }
                BarrelAction::Pan if button_was_held => {
                    self.held.remove(&id);
                    self.pan_positions.remove(&id);
                    self.contacts.remove(&id);
                    if sample.in_contact || contact_was_active {
                        self.suppressed.insert(id);
                    }
                    if terminal {
                        BarrelRoute::Forward
                    } else {
                        BarrelRoute::Suppress
                    }
                }
                _ => BarrelRoute::Forward,
            }
        };

        // A temporary Eyedropper/Pan action owns this pointer while held.
        // Its down/up samples are deliberately converted to hover by Studio,
        // so these contacts are not eligible to restart a painting stroke.
        if action == BarrelAction::Eraser && !self.suppressed.contains(&id) {
            match sample.phase {
                Phase::Down => {
                    self.contacts.insert(id);
                }
                Phase::Move if sample.in_contact => {
                    self.contacts.insert(id);
                }
                Phase::Up | Phase::Cancel | Phase::Hover | Phase::Move => {
                    if terminal || !sample.in_contact {
                        self.contacts.remove(&id);
                    }
                }
            }
        }
        if terminal {
            self.remove(id);
        }
        route
    }

    pub fn cancel_all(&mut self) {
        self.contacts.clear();
        self.held.clear();
        self.suppressed.clear();
        self.pan_positions.clear();
    }

    fn remove(&mut self, id: u64) {
        self.contacts.remove(&id);
        self.held.remove(&id);
        self.suppressed.remove(&id);
        self.pan_positions.remove(&id);
    }

    pub fn cancel_pointer(&mut self, id: u64) {
        self.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{interaction::CanvasSession, scene::CanvasMapping};

    fn sample(id: u64, phase: Phase, barrel: bool, position: [f64; 2]) -> PointerSample {
        PointerSample {
            source_counter: None,
            id,
            timestamp_ms: 1,
            tool: Tool::Pen,
            phase,
            position_px: position,
            pressure: Some(0.5),
            tilt_degrees: [Some(3.0), Some(-4.0)],
            buttons: paint_input::Buttons {
                barrel,
                ..Default::default()
            },
            in_contact: matches!(phase, Phase::Down | Phase::Move),
            in_range: true,
        }
    }

    fn selected_layer_pixel(session: &CanvasSession, x: u32, y: u32) -> [u8; 4] {
        let state = session.live().project().state();
        let key = (x / paint_document::TILE, y / paint_document::TILE);
        let offset = (((y % paint_document::TILE) * paint_document::TILE
            + x % paint_document::TILE)
            * 4) as usize;
        state
            .layers
            .iter()
            .find(|layer| layer.id == state.selected)
            .and_then(|layer| layer.tiles.get(&key))
            .map_or([0; 4], |tile| {
                tile[offset..offset + 4]
                    .try_into()
                    .expect("RGBA pixel has four channels")
            })
    }

    #[test]
    fn default_barrel_eraser_switches_and_restores_a_contact_without_losing_segments() {
        let mut router = BarrelRouter::default();
        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(1, Phase::Down, false, [10.0, 20.0])
            ),
            BarrelRoute::Forward
        );
        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(1, Phase::Move, true, [12.0, 23.0])
            ),
            BarrelRoute::SwitchToEraser
        );
        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(1, Phase::Move, true, [18.0, 30.0])
            ),
            BarrelRoute::Eraser
        );
        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(1, Phase::Move, false, [20.0, 32.0])
            ),
            BarrelRoute::RestoreTool
        );
        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(1, Phase::Up, false, [24.0, 36.0])
            ),
            BarrelRoute::Forward
        );
        assert_eq!(router.contacts.len(), 0);
        assert_eq!(router.held.len(), 0);
    }

    #[test]
    fn eyedropper_and_pan_suppress_painting_and_pan_has_bounded_independent_deltas() {
        let mut router = BarrelRouter::default();
        assert_eq!(
            router.route(
                BarrelAction::Eyedropper,
                &sample(2, Phase::Down, true, [30.0, 40.0])
            ),
            BarrelRoute::PickColor
        );
        assert_eq!(
            router.route(
                BarrelAction::Eyedropper,
                &sample(2, Phase::Move, true, [34.0, 42.0])
            ),
            BarrelRoute::PickColor
        );
        assert_eq!(
            router.route(
                BarrelAction::Eyedropper,
                &sample(2, Phase::Up, false, [34.0, 42.0])
            ),
            BarrelRoute::Forward
        );
        assert_eq!(
            router.route(
                BarrelAction::Pan,
                &sample(3, Phase::Down, true, [10.0, 10.0])
            ),
            BarrelRoute::Pan(None)
        );
        assert_eq!(
            router.route(
                BarrelAction::Pan,
                &sample(3, Phase::Move, true, [15.0, 7.0])
            ),
            BarrelRoute::Pan(Some([5.0, -3.0]))
        );
        assert_eq!(
            router.route(
                BarrelAction::Pan,
                &sample(4, Phase::Move, true, [5000.0, 5000.0])
            ),
            BarrelRoute::Pan(None),
            "pointer IDs have independent pan origins"
        );
        assert_eq!(
            router.route(
                BarrelAction::Pan,
                &sample(4, Phase::Move, false, [5000.0, 5000.0])
            ),
            BarrelRoute::Suppress
        );
        assert_eq!(
            router.route(
                BarrelAction::Pan,
                &sample(3, Phase::Move, true, [9000.0, 8.0])
            ),
            BarrelRoute::Pan(Some([4096.0, 1.0]))
        );
        assert_eq!(
            router.route(
                BarrelAction::Pan,
                &sample(3, Phase::Move, false, [9000.0, 8.0])
            ),
            BarrelRoute::Suppress
        );
        assert!(router.held.is_empty());
        assert!(router.pan_positions.is_empty());
    }

    #[test]
    fn cancellation_focus_cleanup_and_reused_ids_release_temporary_assignments() {
        let mut router = BarrelRouter::default();
        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(9, Phase::Down, true, [0.0, 0.0])
            ),
            BarrelRoute::Eraser
        );
        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(9, Phase::Cancel, true, [1.0, 1.0])
            ),
            BarrelRoute::Forward
        );
        assert!(router.held.is_empty());
        assert_eq!(
            router.route(BarrelAction::Pan, &sample(9, Phase::Down, true, [5.0, 5.0])),
            BarrelRoute::Pan(None)
        );
        router.cancel_all();
        assert_eq!(
            router.route(
                BarrelAction::Disabled,
                &sample(9, Phase::Down, false, [9.0, 9.0])
            ),
            BarrelRoute::Forward
        );
        assert!(router.held.is_empty());
        assert!(router.pan_positions.is_empty());

        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(9, Phase::Down, true, [12.0, 12.0])
            ),
            BarrelRoute::Eraser,
            "a reused ID starts a fresh barrel-button contact"
        );
    }

    #[test]
    fn temporary_contact_stays_suppressed_until_real_pen_lift() {
        let mut router = BarrelRouter::default();
        assert_eq!(
            router.route(
                BarrelAction::Eyedropper,
                &sample(12, Phase::Down, false, [2.0, 3.0])
            ),
            BarrelRoute::Forward
        );
        assert_eq!(
            router.route(
                BarrelAction::Eyedropper,
                &sample(12, Phase::Move, true, [4.0, 5.0])
            ),
            BarrelRoute::PickColor
        );
        assert_eq!(
            router.route(
                BarrelAction::Eyedropper,
                &sample(12, Phase::Move, false, [6.0, 7.0])
            ),
            BarrelRoute::Suppress
        );
        assert_eq!(router.suppressed.len(), 1);
        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(12, Phase::Move, false, [8.0, 9.0])
            ),
            BarrelRoute::Suppress,
            "changing assignments while a temporary contact is down cannot restart painting"
        );
        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(12, Phase::Up, false, [8.0, 9.0])
            ),
            BarrelRoute::Forward
        );
        assert!(router.held.is_empty());
        assert!(router.suppressed.is_empty());
        assert!(router.contacts.is_empty());
    }

    #[test]
    fn terminal_sample_cleans_a_still_held_barrel_button() {
        let mut router = BarrelRouter::default();
        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(13, Phase::Down, true, [1.0, 1.0])
            ),
            BarrelRoute::Eraser
        );
        assert_eq!(
            router.route(
                BarrelAction::Eraser,
                &sample(13, Phase::Up, true, [1.0, 1.0])
            ),
            BarrelRoute::Eraser
        );
        assert!(router.held.is_empty());
        assert!(router.contacts.is_empty());
    }

    #[test]
    fn barrel_eraser_commits_and_release_resumes_the_selected_paint_tool() {
        let mut session = CanvasSession::new([800, 500]);
        session.live_mut().enable_document();
        let project = session.live_mut().project_mut();
        project.brush.color = [255, 0, 0];
        project.brush.size = 8.0;
        project.brush.hardness = 1.0;
        project.brush.flow = 1.0;
        project.brush.opacity = 1.0;
        project.brush.pressure_size = false;
        project.brush.pressure_opacity = false;
        let mapping = CanvasMapping::new([800, 500]).unwrap();
        let client = |x| mapping.document_to_client([x, 250.0]).unwrap();
        let mut router = BarrelRouter::default();

        session.pen_with_barrel(
            &mut router,
            BarrelAction::Eraser,
            sample(21, Phase::Down, false, client(20.0)),
        );
        session.pen_with_barrel(
            &mut router,
            BarrelAction::Eraser,
            sample(21, Phase::Move, false, client(140.0)),
        );
        session.pen_with_barrel(
            &mut router,
            BarrelAction::Eraser,
            sample(21, Phase::Move, true, client(148.0)),
        );
        session.pen_with_barrel(
            &mut router,
            BarrelAction::Eraser,
            sample(21, Phase::Move, true, client(168.0)),
        );
        session.pen_with_barrel(
            &mut router,
            BarrelAction::Eraser,
            sample(21, Phase::Move, false, client(172.0)),
        );
        session.pen_with_barrel(
            &mut router,
            BarrelAction::Eraser,
            sample(21, Phase::Up, false, client(172.0)),
        );

        assert!(!session.live().active());
        assert_eq!(session.live().project().undo_count(), 3);
        assert_eq!(
            selected_layer_pixel(&session, 150, 250),
            [0; 4],
            "the held side button erases the selected layer's real pixels"
        );

        assert!(session.live_mut().project_mut().undo());
        assert!(session.live_mut().project_mut().undo());
        assert_eq!(selected_layer_pixel(&session, 150, 250), [255, 0, 0, 255]);
        assert!(session.live_mut().project_mut().undo());
        let state = session.live().project().state();
        assert!(
            state
                .layers
                .iter()
                .find(|layer| layer.id == state.selected)
                .is_some_and(|layer| layer.tiles.is_empty())
        );
    }

    #[test]
    fn barrel_eyedropper_samples_visible_pixels_without_starting_a_stroke() {
        let mut session = CanvasSession::new([800, 500]);
        session.live_mut().enable_document();
        session.live_mut().project_mut().brush.color = [255, 0, 0];
        session.live_mut().project_mut().brush.size = 12.0;
        session.live_mut().project_mut().brush.pressure_size = false;
        session.live_mut().project_mut().brush.pressure_opacity = false;
        let mapping = CanvasMapping::new([800, 500]).unwrap();
        let point = mapping.document_to_client([64.0, 250.0]).unwrap();
        session.pen(sample(22, Phase::Down, false, point));
        session.pen(sample(22, Phase::Up, false, point));
        session.live_mut().project_mut().brush.color = [0, 0, 255];

        let mut router = BarrelRouter::default();
        assert_eq!(
            session.pen_with_barrel(
                &mut router,
                BarrelAction::Eyedropper,
                sample(23, Phase::Down, true, point),
            ),
            BarrelEffect::ColorPicked([255, 0, 0])
        );
        assert_eq!(session.live().project().brush.color, [255, 0, 0]);
        assert!(!session.live().active());
        assert_eq!(session.live().project().undo_count(), 1);
        assert_eq!(
            session.pen_with_barrel(
                &mut router,
                BarrelAction::Eyedropper,
                sample(23, Phase::Move, false, point),
            ),
            BarrelEffect::None
        );
        assert!(!session.live().active());
        assert_eq!(
            session.pen_with_barrel(
                &mut router,
                BarrelAction::Eyedropper,
                sample(23, Phase::Up, false, point),
            ),
            BarrelEffect::None
        );
        assert_eq!(session.live().project().undo_count(), 1);
    }

    #[test]
    fn barrel_pan_returns_bounded_deltas_and_keeps_contact_nonpainting() {
        let mut session = CanvasSession::new([800, 500]);
        session.live_mut().enable_document();
        let mut router = BarrelRouter::default();
        assert_eq!(
            session.pen_with_barrel(
                &mut router,
                BarrelAction::Pan,
                sample(24, Phase::Down, true, [30.0, 40.0]),
            ),
            BarrelEffect::None
        );
        assert_eq!(
            session.pen_with_barrel(
                &mut router,
                BarrelAction::Pan,
                sample(24, Phase::Move, true, [35.0, 33.0]),
            ),
            BarrelEffect::Pan([5.0, -7.0])
        );
        assert!(!session.live().active());
        assert_eq!(session.live().project().undo_count(), 0);
    }
}
