//! Hardware-independent live mouse adapter and mode/session ownership.
//! No window handles, capture APIs, wall clock, or automatic persistence.
use crate::{
    playback::{Playback, Scenario},
    scene::{CanvasMapping, Scene, SceneError},
};
use paint_input::{
    Buttons, Phase, PointerSample, Tool,
    lifecycle::{Processor, Reason},
};
use paint_stroke::Router;

#[derive(Debug, Clone, Copy)]
pub enum MouseEvent {
    Moved([f64; 2]),
    Button { primary: bool, pressed: bool },
    Focus(bool),
    Entered,
    Left,
    CaptureLost,
    Interrupted,
    Resized([u32; 2]),
}

#[derive(Debug, Clone, Copy)]
struct PenTrack {
    raw_client: [f64; 2],
    last_paper_client: [f64; 2],
    raw_sample: PointerSample,
}

/// One logical mouse stream. Reused pointer ID 1 is normalized by Processor;
/// Router's monotonically increasing stroke IDs are never reset by clear/mode switches.
pub struct LiveMouse {
    scene: Scene,
    project: paint_document::Engine,
    document_mode: bool,
    processor: Processor,
    router: Router,
    mapping: Option<CanvasMapping>,
    cursor: Option<[f64; 2]>,
    primary_down: bool,
    last_mouse_paper: Option<[f64; 2]>,
    pen_tracks: std::collections::BTreeMap<u64, PenTrack>,
    focused: bool,
    inside: bool,
    time_ms: u64,
    notice: Option<SceneError>,
    document_error: Option<String>,
    last_pen: Option<PointerSample>,
    stroke_mode: paint_document::StrokeMode,
}
impl LiveMouse {
    pub fn new(size: [u32; 2]) -> Self {
        Self {
            scene: Scene::default(),
            project: paint_document::Engine::new(800, 500, 300).expect("valid default document"),
            document_mode: false,
            processor: Processor::default(),
            router: Router::default(),
            mapping: CanvasMapping::new(size).ok(),
            cursor: None,
            primary_down: false,
            last_mouse_paper: None,
            pen_tracks: Default::default(),
            focused: true,
            inside: false,
            time_ms: 0,
            notice: None,
            document_error: None,
            last_pen: None,
            stroke_mode: paint_document::StrokeMode::Paint,
        }
    }
    pub fn project(&self) -> &paint_document::Engine {
        &self.project
    }
    pub fn project_mut(&mut self) -> &mut paint_document::Engine {
        &mut self.project
    }
    pub fn set_stroke_mode(&mut self, mode: paint_document::StrokeMode) {
        if self.stroke_mode != mode {
            self.interrupt(Reason::Interrupted);
            self.stroke_mode = mode;
        }
    }
    pub fn enable_document(&mut self) {
        self.interrupt(Reason::Interrupted);
        self.document_mode = true;
        self.scene.clear();
    }
    pub fn set_mapping(&mut self, mapping: CanvasMapping) {
        if self.mapping != Some(mapping) {
            self.interrupt(Reason::Interrupted);
            self.mapping = Some(mapping);
        }
    }
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
    pub fn active(&self) -> bool {
        !self.processor.active().is_empty()
    }
    pub fn last_pen(&self) -> Option<PointerSample> {
        self.last_pen
    }
    pub fn document_error(&self) -> Option<&str> {
        self.document_error.as_deref()
    }
    pub fn notice(&self) -> Option<SceneError> {
        self.notice
    }

    fn interrupt(&mut self, reason: Reason) {
        self.last_pen = None;
        for output in self.processor.interrupt(self.time_ms, reason) {
            if let Some(command) = self.router.route(&output) {
                let _ = self.scene.apply(&command);
                let _ = self.project.apply_with_mode(&command, self.stroke_mode);
            }
        }
        self.primary_down = false;
        self.last_mouse_paper = None;
        self.pen_tracks.clear();
        self.cursor = None;
    }

    /// Feed only the portion of a raw pointer segment inside the document. A
    /// re-entry begins a new subpath under the same routed StrokeId.
    fn clipped_move(
        &mut self,
        pointer_id: u64,
        from: [f64; 2],
        to: [f64; 2],
        previous: PointerSample,
        mut sample: PointerSample,
    ) -> Option<[f64; 2]> {
        let clipped = self.mapping?.clip_segment_client(from, to).ok()??;
        let endpoint_sample = sample;
        sample.phase = Phase::Move;
        sample.in_contact = true;
        if clipped.start_t > 1e-10 || clipped.start_t == clipped.end_t {
            self.router.break_before_next_move(pointer_id);
            sample.position_px = clipped.start_client;
            interpolate_attributes(&mut sample, previous, clipped.start_t);
            self.consume(sample);
        }
        if clipped.end_t - clipped.start_t > 1e-10 {
            sample = endpoint_sample;
            sample.position_px = clipped.end_client;
            if clipped.end_t < 1.0 - 1e-10 {
                interpolate_attributes(&mut sample, previous, clipped.end_t);
            }
            self.consume(sample);
        }
        if clipped.end_t < 1.0 - 1e-10 {
            self.router.break_before_next_move(pointer_id);
        }
        Some(clipped.end_client)
    }

    pub fn pen(&mut self, mut sample: PointerSample) {
        self.last_pen = if self.focused && paint_input::lifecycle::valid_sample(&sample) {
            Some(sample)
        } else {
            None
        };
        let native_id = sample.id;
        sample.id |= 1 << 63; // Disjoint from the logical mouse stream.
        self.time_ms = self.time_ms.max(sample.timestamp_ms);
        sample.timestamp_ms = self.time_ms;
        if sample.phase == Phase::Cancel {
            self.pen_tracks.remove(&native_id);
            self.consume(sample);
            return;
        }
        let Some(mapping) = self.mapping else {
            return;
        };
        if !self.focused || !mapping.contains_window_client(sample.position_px) {
            self.pen_tracks.remove(&native_id);
            for output in self.processor.process(PointerSample {
                phase: Phase::Cancel,
                in_contact: false,
                ..sample
            }) {
                if let Some(command) = self.router.route(&output) {
                    let _ = self.scene.apply(&command);
                    let _ = self.project.apply_with_mode(&command, self.stroke_mode);
                }
            }
            return;
        }

        if sample.tool != Tool::Pen {
            if matches!(sample.phase, Phase::Up | Phase::Cancel) {
                self.pen_tracks.remove(&native_id);
            }
            self.consume(sample);
            return;
        }

        match sample.phase {
            Phase::Hover => self.consume(sample),
            Phase::Down => {
                if self.processor.active().contains_key(&sample.id) {
                    return;
                }
                if !mapping.contains_client(sample.position_px) {
                    self.pen_tracks.remove(&native_id);
                    self.consume(PointerSample {
                        phase: Phase::Cancel,
                        in_contact: false,
                        ..sample
                    });
                    return;
                }
                if self.processor.active().len() >= paint_input::lifecycle::MAX_CONTACTS
                    || self.pen_tracks.len() >= paint_input::lifecycle::MAX_CONTACTS
                {
                    return;
                }
                self.router.set_viewport(mapping.routing_viewport());
                self.pen_tracks.insert(
                    native_id,
                    PenTrack {
                        raw_client: sample.position_px,
                        last_paper_client: sample.position_px,
                        raw_sample: sample,
                    },
                );
                self.consume(sample);
            }
            Phase::Move => {
                let Some(track) = self.pen_tracks.get(&native_id).copied() else {
                    self.consume(sample);
                    return;
                };
                if let Some(last) = self.clipped_move(
                    sample.id,
                    track.raw_client,
                    sample.position_px,
                    track.raw_sample,
                    sample,
                ) {
                    self.pen_tracks.insert(
                        native_id,
                        PenTrack {
                            raw_client: sample.position_px,
                            last_paper_client: last,
                            raw_sample: sample,
                        },
                    );
                } else if let Some(track) = self.pen_tracks.get_mut(&native_id) {
                    track.raw_client = sample.position_px;
                    track.raw_sample = sample;
                }
            }
            Phase::Up => {
                let Some(track) = self.pen_tracks.get(&native_id).copied() else {
                    self.consume(sample);
                    return;
                };
                let last_paper = self
                    .clipped_move(
                        sample.id,
                        track.raw_client,
                        sample.position_px,
                        track.raw_sample,
                        sample,
                    )
                    .unwrap_or(track.last_paper_client);
                sample.position_px = if mapping.contains_client(sample.position_px) {
                    sample.position_px
                } else {
                    last_paper
                };
                self.pen_tracks.remove(&native_id);
                self.consume(sample);
            }
            Phase::Cancel => unreachable!("handled above"),
        }
    }
    fn consume(&mut self, sample: PointerSample) {
        for output in self.processor.process(sample) {
            if let Some(command) = self.router.route(&output) {
                let result = if self.document_mode {
                    self.project
                        .apply_with_mode(&command, self.stroke_mode)
                        .map_err(|error| {
                            self.document_error = Some(error.to_string());
                            SceneError::Capacity
                        })
                } else {
                    self.scene.apply(&command)
                };
                if let Err(error) = result {
                    self.notice = Some(error);
                    self.interrupt(Reason::Interrupted);
                    break;
                }
            }
        }
    }
    fn sample(&mut self, phase: Phase, position: [f64; 2]) {
        self.consume(self.mouse_sample(phase, position));
    }
    fn mouse_sample(&self, phase: Phase, position: [f64; 2]) -> PointerSample {
        let contact = matches!(phase, Phase::Down | Phase::Move);
        PointerSample {
            source_counter: None,
            id: 1,
            timestamp_ms: self.time_ms,
            tool: Tool::Mouse,
            phase,
            position_px: position,
            pressure: None,
            tilt_degrees: [None; 2],
            buttons: Buttons {
                primary: contact,
                ..Buttons::default()
            },
            in_contact: contact,
            in_range: true,
        }
    }
    pub fn clear(&mut self, time_ms: u64) {
        self.time_ms = self.time_ms.max(time_ms);
        let cursor = self.cursor;
        self.interrupt(Reason::Interrupted);
        self.cursor = cursor;
        self.scene.clear();
        if self.document_mode {
            let _ = self.project.clear_layer();
        }
        self.notice = None;
        self.document_error = None;
    }
    /// Winit positions and sizes must both be physical client pixels. No second
    /// scale-factor multiplication. Cancellation resets the cursor, requiring a
    /// fresh position before another Down; orphan Moves never restart a stroke.
    pub fn handle(&mut self, event: MouseEvent, time_ms: u64) {
        if matches!(event, MouseEvent::Moved(_) | MouseEvent::Button { .. }) {
            self.last_pen = None;
        }
        self.time_ms = self.time_ms.max(time_ms);
        match event {
            MouseEvent::Focus(focused) => {
                self.focused = focused;
                if !focused {
                    self.interrupt(Reason::Interrupted);
                }
            }
            MouseEvent::Left => {
                self.inside = false;
                self.interrupt(Reason::Interrupted);
            }
            MouseEvent::Entered => self.inside = true,
            MouseEvent::CaptureLost => self.interrupt(Reason::CaptureLost),
            MouseEvent::Interrupted => self.interrupt(Reason::Interrupted),
            MouseEvent::Resized(size) => {
                self.interrupt(Reason::Interrupted);
                self.mapping = CanvasMapping::new(size).ok();
            }
            MouseEvent::Moved(position) => {
                self.inside = true;
                if !position.iter().all(|value| value.is_finite())
                    || self
                        .mapping
                        .is_some_and(|mapping| !mapping.contains_window_client(position))
                {
                    self.interrupt(Reason::Interrupted);
                    return;
                }
                let previous = self.cursor;
                self.cursor = Some(position);
                if self.focused
                    && self.primary_down
                    && self.active()
                    && let Some(previous) = previous
                    && let Some(last) = self.clipped_move(
                        1,
                        previous,
                        position,
                        self.mouse_sample(Phase::Move, previous),
                        self.mouse_sample(Phase::Move, position),
                    )
                {
                    self.last_mouse_paper = Some(last);
                }
            }
            MouseEvent::Button {
                primary: true,
                pressed,
            } => {
                if pressed {
                    if self.primary_down || !self.focused || !self.inside {
                        return;
                    }
                    let Some(position) = self.cursor else {
                        return;
                    };
                    let Some(mapping) = self.mapping else {
                        return;
                    };
                    if !mapping.contains_client(position) {
                        return;
                    }
                    self.router.set_viewport(mapping.routing_viewport());
                    self.primary_down = true;
                    self.last_mouse_paper = Some(position);
                    self.notice = None;
                    self.document_error = None;
                    self.sample(Phase::Down, position);
                } else {
                    self.primary_down = false;
                    if self.active() {
                        if let Some(position) = self.cursor {
                            let final_position = if self
                                .mapping
                                .is_some_and(|mapping| mapping.contains_client(position))
                            {
                                Some(position)
                            } else {
                                self.last_mouse_paper
                            };
                            if let Some(position) = final_position {
                                self.sample(Phase::Up, position);
                            } else {
                                self.interrupt(Reason::Interrupted);
                            }
                        } else {
                            self.interrupt(Reason::Interrupted);
                        }
                    }
                    self.last_mouse_paper = None;
                }
            }
            MouseEvent::Button { primary: false, .. } => {} // Right/middle never paint.
        }
    }
}

fn interpolate_attributes(sample: &mut PointerSample, from: PointerSample, t: f64) {
    fn lerp_option(a: Option<f32>, b: Option<f32>, t: f64) -> Option<f32> {
        match (a, b) {
            (Some(a), Some(b)) => Some(a + (b - a) * t as f32),
            _ => None,
        }
    }
    sample.pressure = lerp_option(from.pressure, sample.pressure, t);
    for axis in 0..2 {
        sample.tilt_degrees[axis] =
            lerp_option(from.tilt_degrees[axis], sample.tilt_degrees[axis], t);
    }
    if t < 1.0 - 1e-10 {
        sample.source_counter = None;
    }
    let fraction = t.clamp(0.0, 1.0);
    let elapsed = sample.timestamp_ms.saturating_sub(from.timestamp_ms);
    sample.timestamp_ms = from
        .timestamp_ms
        .saturating_add((elapsed as f64 * fraction).round() as u64);
}

#[cfg(test)]
mod smudge_tests {
    use super::{LiveMouse, MouseEvent};
    use crate::scene::CanvasMapping;
    use paint_document::{StrokeMode, brush::BrushKind};
    use paint_stroke::{StrokeCommand, StrokeId, StrokeSample, coordinates::DocumentPoint};

    fn seed_red_patch(live: &mut LiveMouse) {
        let engine = live.project_mut();
        engine.brush.kind = BrushKind::Round;
        engine.brush.color = [255, 0, 0];
        engine.brush.size = 8.0;
        engine.brush.pressure_size = false;
        engine.brush.pressure_opacity = false;
        engine.brush.hardness = 1.0;
        let sample = |x| StrokeSample {
            source_counter: None,
            pointer_id: 2,
            tool: paint_input::Tool::Pen,
            position_client_px: [x, 250.0],
            position_document: DocumentPoint([x, 250.0]),
            pressure: Some(1.0),
            tilt_degrees: [None; 2],
            buttons: Default::default(),
            source_time_ms: 0,
            time_ms: 0,
        };
        engine
            .apply(&StrokeCommand::BeginStroke {
                stroke_id: StrokeId(22),
                sample: sample(12.0),
            })
            .unwrap();
        engine
            .apply(&StrokeCommand::CommitStroke {
                stroke_id: StrokeId(22),
                sample: sample(26.0),
            })
            .unwrap();
    }

    #[test]
    fn live_mouse_router_smudge_previews_cancels_and_commits_real_document_pixels() {
        let mut live = LiveMouse::new([800, 500]);
        live.enable_document();
        seed_red_patch(&mut live);
        let mapping = CanvasMapping::new([800, 500]).unwrap();
        let client = |x| mapping.document_to_client([x, 250.0]).unwrap();
        live.project_mut().brush.size = 8.0;
        live.project_mut().brush.opacity = 1.0;
        live.project_mut().brush.flow = 0.0;
        let before = live.project().rgba();
        let destination = (250 * 800 + 55) * 4;

        live.set_stroke_mode(StrokeMode::Smudge);
        live.handle(MouseEvent::Entered, 1);
        live.handle(MouseEvent::Moved(client(16.0)), 2);
        live.handle(
            MouseEvent::Button {
                primary: true,
                pressed: true,
            },
            3,
        );
        live.handle(MouseEvent::Moved(client(60.0)), 4);
        let preview = live.project().rgba();
        assert_ne!(
            &preview[destination..destination + 4],
            &before[destination..destination + 4],
            "input pipeline displays preview"
        );
        assert_eq!(
            live.project().state().tile_count(),
            1,
            "preview is scratch-only"
        );
        live.handle(MouseEvent::Focus(false), 5);
        assert!(!live.active());
        assert_eq!(
            live.project().rgba(),
            before,
            "focus loss cancels the smudge"
        );

        live.handle(MouseEvent::Focus(true), 6);
        live.handle(MouseEvent::Entered, 7);
        live.handle(MouseEvent::Moved(client(16.0)), 8);
        live.handle(
            MouseEvent::Button {
                primary: true,
                pressed: true,
            },
            9,
        );
        live.handle(MouseEvent::Moved(client(60.0)), 10);
        live.handle(
            MouseEvent::Button {
                primary: true,
                pressed: false,
            },
            11,
        );
        let after = live.project().rgba();
        assert!(
            after[destination + 3] > 0,
            "router smudges pigment to the endpoint"
        );
        assert!(live.project().dirty());
        assert!(live.project_mut().undo());
        assert_eq!(live.project().rgba(), before);
    }

    #[test]
    fn live_mouse_routes_color_mixing_through_processor_and_router() {
        let mut live = LiveMouse::new([800, 500]);
        live.enable_document();
        seed_red_patch(&mut live);
        live.project_mut().brush.size = 8.0;
        live.project_mut().brush.color = [0, 0, 255];
        live.project_mut().brush.flow = 1.0;
        live.project_mut().brush.opacity = 1.0;
        live.project_mut().brush.pressure_size = false;
        live.project_mut().brush.pressure_opacity = false;
        let before = live.project().rgba();
        let mapping = CanvasMapping::new([800, 500]).unwrap();
        let client = |x| mapping.document_to_client([x, 250.0]).unwrap();

        live.set_stroke_mode(paint_document::StrokeMode::ColorMix { strength: 1.0 });
        live.handle(MouseEvent::Entered, 1);
        live.handle(MouseEvent::Moved(client(16.0)), 2);
        live.handle(
            MouseEvent::Button {
                primary: true,
                pressed: true,
            },
            3,
        );
        live.handle(MouseEvent::Moved(client(60.0)), 4);
        assert_ne!(
            live.project().rgba(),
            before,
            "routed input previews mixed pixels"
        );
        live.handle(MouseEvent::Focus(false), 5);
        assert_eq!(
            live.project().rgba(),
            before,
            "focus loss cancels the mixture"
        );

        live.handle(MouseEvent::Focus(true), 6);
        live.handle(MouseEvent::Entered, 7);
        live.handle(MouseEvent::Moved(client(16.0)), 8);
        live.handle(
            MouseEvent::Button {
                primary: true,
                pressed: true,
            },
            9,
        );
        live.handle(MouseEvent::Moved(client(60.0)), 10);
        live.handle(
            MouseEvent::Button {
                primary: true,
                pressed: false,
            },
            11,
        );
        assert_ne!(live.project().rgba(), before);
        assert_eq!(live.project().undo_count(), 2, "the mix is one transaction");
        assert!(live.project_mut().undo());
        assert_eq!(live.project().rgba(), before);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Live,
    Replay,
}

/// Distinct scene owners provide a session namespace for live versus replay IDs.
/// Switching preserves committed live work but discards all interrupted previews.
pub struct CanvasSession {
    live: LiveMouse,
    playback: Playback,
    mode: Mode,
    scenario: Scenario,
}
impl CanvasSession {
    pub fn new(size: [u32; 2]) -> Self {
        Self {
            live: LiveMouse::new(size),
            playback: Playback::new(Scenario::Completed).expect("built-in valid replay"),
            mode: Mode::Live,
            scenario: Scenario::Completed,
        }
    }
    pub fn pen(&mut self, sample: PointerSample) {
        if self.mode == Mode::Live {
            self.live.pen(sample);
        }
    }
    pub fn live_mut(&mut self) -> &mut LiveMouse {
        &mut self.live
    }
    pub fn live(&self) -> &LiveMouse {
        &self.live
    }
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn scenario(&self) -> Scenario {
        self.scenario
    }
    pub fn scene(&self) -> &Scene {
        match self.mode {
            Mode::Live => self.live.scene(),
            Mode::Replay => &self.playback.scene,
        }
    }
    pub fn replay_finished(&self) -> bool {
        self.playback.finished()
    }
    pub fn mouse(&mut self, event: MouseEvent, time_ms: u64) {
        // Geometry changes, focus, and cancellation must invalidate the hidden live
        // adapter too. Pointer motion/buttons in replay must not seed live state.
        if self.mode == Mode::Live
            || !matches!(
                event,
                MouseEvent::Moved(_) | MouseEvent::Button { .. } | MouseEvent::Entered
            )
        {
            self.live.handle(event, time_ms);
        }
    }
    pub fn set_mode(&mut self, mode: Mode, time_ms: u64) {
        if self.mode == mode {
            return;
        }
        self.live.handle(MouseEvent::Interrupted, time_ms);
        self.playback.restart();
        self.mode = mode;
    }
    pub fn select_replay(&mut self, scenario: Scenario, time_ms: u64) {
        self.live.handle(MouseEvent::Interrupted, time_ms);
        self.playback = Playback::new(scenario).expect("built-in valid replay");
        self.scenario = scenario;
        self.mode = Mode::Replay;
    }
    pub fn restart_replay(&mut self) {
        if self.mode == Mode::Replay {
            self.playback.restart();
        }
    }
    pub fn clear_live(&mut self, time_ms: u64) {
        if self.mode == Mode::Live {
            self.live.clear(time_ms);
        }
    }
    pub fn advance_replay(&mut self, elapsed: u64) -> Result<(), SceneError> {
        if self.mode == Mode::Replay {
            self.playback.advance_to(elapsed)?;
        }
        Ok(())
    }
}
