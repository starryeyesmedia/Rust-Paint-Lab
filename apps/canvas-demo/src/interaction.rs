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

/// One logical mouse stream. Reused pointer ID 1 is normalized by Processor;
/// Router's monotonically increasing stroke IDs are never reset by clear/mode switches.
pub struct LiveMouse {
    scene: Scene,
    processor: Processor,
    router: Router,
    mapping: Option<CanvasMapping>,
    cursor: Option<[f64; 2]>,
    primary_down: bool,
    focused: bool,
    inside: bool,
    time_ms: u64,
    notice: Option<SceneError>,
}
impl LiveMouse {
    pub fn new(size: [u32; 2]) -> Self {
        Self {
            scene: Scene::default(),
            processor: Processor::default(),
            router: Router::default(),
            mapping: CanvasMapping::new(size).ok(),
            cursor: None,
            primary_down: false,
            focused: true,
            inside: false,
            time_ms: 0,
            notice: None,
        }
    }
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
    pub fn active(&self) -> bool {
        !self.processor.active().is_empty()
    }
    pub fn notice(&self) -> Option<SceneError> {
        self.notice
    }

    fn interrupt(&mut self, reason: Reason) {
        for output in self.processor.interrupt(self.time_ms, reason) {
            if let Some(command) = self.router.route(&output) {
                let _ = self.scene.apply(&command);
            }
        }
        self.primary_down = false;
        self.cursor = None;
    }
    pub fn pen(&mut self, mut sample: PointerSample) {
        sample.id |= 1 << 63; // Disjoint from the logical mouse stream.
        self.time_ms = self.time_ms.max(sample.timestamp_ms);
        sample.timestamp_ms = self.time_ms;
        if sample.phase == Phase::Cancel {
            self.consume(sample);
            return;
        }
        if !self.focused
            || !self
                .mapping
                .is_some_and(|m| m.contains_client(sample.position_px))
        {
            for output in self.processor.process(PointerSample {
                phase: Phase::Cancel,
                in_contact: false,
                ..sample
            }) {
                if let Some(command) = self.router.route(&output) {
                    let _ = self.scene.apply(&command);
                }
            }
            return;
        }
        if sample.phase == Phase::Down {
            self.router
                .set_viewport(self.mapping.expect("checked mapping").routing_viewport());
        }
        self.consume(sample);
    }
    fn consume(&mut self, sample: PointerSample) {
        for output in self.processor.process(sample) {
            if let Some(command) = self.router.route(&output)
                && let Err(error) = self.scene.apply(&command)
            {
                self.notice = Some(error);
                self.interrupt(Reason::Interrupted);
                break;
            }
        }
    }
    fn sample(&mut self, phase: Phase, position: [f64; 2]) {
        let contact = matches!(phase, Phase::Down | Phase::Move);
        let sample = PointerSample {
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
        };
        self.consume(sample);
    }
    pub fn clear(&mut self, time_ms: u64) {
        self.time_ms = self.time_ms.max(time_ms);
        let cursor = self.cursor;
        self.interrupt(Reason::Interrupted);
        self.cursor = cursor;
        self.scene.clear();
        self.notice = None;
    }
    /// Winit positions and sizes must both be physical client pixels. No second
    /// scale-factor multiplication. Cancellation resets the cursor, requiring a
    /// fresh position before another Down; orphan Moves never restart a stroke.
    pub fn handle(&mut self, event: MouseEvent, time_ms: u64) {
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
                if !self.mapping.is_some_and(|m| m.contains_client(position)) {
                    self.interrupt(Reason::Interrupted);
                    return;
                }
                self.cursor = Some(position);
                if self.focused && self.primary_down && self.active() {
                    self.sample(Phase::Move, position);
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
                    self.notice = None;
                    self.sample(Phase::Down, position);
                } else {
                    self.primary_down = false;
                    if self.active() {
                        if let Some(position) = self.cursor {
                            self.sample(Phase::Up, position);
                        } else {
                            self.interrupt(Reason::Interrupted);
                        }
                    }
                }
            }
            MouseEvent::Button { primary: false, .. } => {} // Right/middle never paint.
        }
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
