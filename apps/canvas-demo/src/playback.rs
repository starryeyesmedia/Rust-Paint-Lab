//! Command generation is deterministic; elapsed demo time is supplied explicitly.
use crate::scene::{Scene, SceneError};
use paint_input::replay::{ParseError, Replay};
use paint_stroke::{Router, StrokeCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    Completed,
    Cancelled,
    Concurrent,
}
impl Scenario {
    pub fn name(self) -> &'static str {
        match self {
            Self::Completed => "A: completed",
            Self::Cancelled => "B: cancelled",
            Self::Concurrent => "C: concurrent pens + touch",
        }
    }
    pub fn fixture(self) -> &'static str {
        match self {
            Self::Completed => include_str!("../fixtures/completed.replay"),
            Self::Cancelled => include_str!("../fixtures/cancelled.replay"),
            Self::Concurrent => include_str!("../fixtures/concurrent.replay"),
        }
    }
}
pub fn commands(text: &str) -> Result<Vec<StrokeCommand>, ParseError> {
    let replay: Replay = text.parse()?;
    let mut router = Router::default();
    Ok(replay
        .run()
        .iter()
        .filter_map(|o| router.route(o))
        .collect())
}
fn time(c: &StrokeCommand) -> u64 {
    match c {
        StrokeCommand::BeginStroke { sample, .. }
        | StrokeCommand::UpdateStroke { sample, .. }
        | StrokeCommand::CommitStroke { sample, .. } => sample.time_ms,
        StrokeCommand::CancelStroke { time_ms, .. } => *time_ms,
    }
}
pub struct Playback {
    pub scene: Scene,
    commands: Vec<StrokeCommand>,
    cursor: usize,
    elapsed: u64,
}
impl Playback {
    pub fn new(scenario: Scenario) -> Result<Self, ParseError> {
        Self::from_fixture(scenario.fixture())
    }
    pub fn from_fixture(text: &str) -> Result<Self, ParseError> {
        Ok(Self {
            scene: Scene::default(),
            commands: commands(text)?,
            cursor: 0,
            elapsed: 0,
        })
    }
    pub fn finished(&self) -> bool {
        self.cursor == self.commands.len()
    }
    pub fn restart(&mut self) {
        self.scene.clear();
        self.cursor = 0;
        self.elapsed = 0;
    }
    pub fn advance_to(&mut self, elapsed_ms: u64) -> Result<(), SceneError> {
        self.elapsed = self.elapsed.max(elapsed_ms);
        while let Some(c) = self.commands.get(self.cursor) {
            if time(c) > self.elapsed {
                break;
            }
            self.scene.apply(c)?;
            self.cursor += 1;
        }
        Ok(())
    }
}
