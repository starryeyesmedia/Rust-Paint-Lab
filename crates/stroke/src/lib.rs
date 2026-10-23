//! Pure routing from validated input lifecycles to provisional stroke transactions.
//! No rendering, brush algorithms, document mutation, or automatic recording.
#![forbid(unsafe_code)]
pub mod coordinates;

use coordinates::{DocumentPoint, Viewport};
use paint_input::{
    Buttons, Phase, PointerSample, Tool,
    lifecycle::{Change, MAX_CONTACTS, Output, Reason, valid_sample},
};
use std::collections::BTreeMap;

/// Unique within one Router session. Do not persist or merge across sessions
/// without namespacing. Reused pointer IDs receive a fresh stroke ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StrokeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeSample {
    pub pointer_id: u64,
    pub tool: Tool,
    pub position_client_px: [f64; 2],
    pub position_document: DocumentPoint,
    pub pressure: Option<f32>,
    pub tilt_degrees: [Option<f32>; 2],
    pub buttons: Buttons,
    pub source_time_ms: u64,
    pub time_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    Input(Reason),
    InvalidInput,
    InvalidCoordinates,
    MousePrimaryReleased,
    ToolChanged,
    RouterInterrupted,
}

/// Begin/Update are provisional; Commit is the only successful completion.
/// Cancel discards the transaction and carries its last accepted point, not an
/// invented end sample. Consumers must key state by StrokeId, not just pointer ID.
#[derive(Debug, Clone, PartialEq)]
pub enum StrokeCommand {
    BeginStroke {
        stroke_id: StrokeId,
        sample: StrokeSample,
    },
    UpdateStroke {
        stroke_id: StrokeId,
        sample: StrokeSample,
    },
    CommitStroke {
        stroke_id: StrokeId,
        sample: StrokeSample,
    },
    CancelStroke {
        stroke_id: StrokeId,
        last: StrokeSample,
        time_ms: u64,
        reason: CancelReason,
    },
}

#[derive(Debug)]
struct ActiveStroke {
    id: StrokeId,
    viewport: Viewport,
    last: StrokeSample,
    raw: PointerSample,
}

/// One router per ordered Processor stream. At most MAX_CONTACTS active strokes,
/// no retained stroke history. Touch/unknown tools are reserved, never painted.
/// A viewport is captured at Begin and stays fixed for that transaction.
#[derive(Debug, Default)]
pub struct Router {
    viewport: Viewport,
    active: BTreeMap<u64, ActiveStroke>,
    next_id: u64,
    time_ms: u64,
}

impl Router {
    pub fn new(viewport: Viewport) -> Self {
        Self {
            viewport,
            ..Self::default()
        }
    }
    /// Affects future strokes only. Active strokes keep their original mapping.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    fn cancel(&mut self, pointer: u64, reason: CancelReason) -> Option<StrokeCommand> {
        self.active
            .remove(&pointer)
            .map(|active| StrokeCommand::CancelStroke {
                stroke_id: active.id,
                last: active.last,
                time_ms: self.time_ms,
                reason,
            })
    }

    /// Shell teardown safeguard; normal cleanup arrives through lifecycle Cancelled.
    /// Stable pointer-ID order, no synthetic Commit, and nondecreasing command time.
    pub fn interrupt(&mut self, time_ms: u64) -> Vec<StrokeCommand> {
        self.time_ms = self.time_ms.max(time_ms);
        let pointers: Vec<_> = self.active.keys().copied().collect();
        pointers
            .into_iter()
            .filter_map(|id| self.cancel(id, CancelReason::RouterInterrupted))
            .collect()
    }

    /// Consume each ordered lifecycle Output exactly once. Invalid/orphan/duplicate
    /// starts/updates cannot invent a stroke. Terminal errors cancel rather than
    /// committing partial or corrupt data. No phase or timing is synthesized.
    pub fn route(&mut self, input: &Output) -> Option<StrokeCommand> {
        let stale = input.time_ms < self.time_ms;
        self.time_ms = self.time_ms.max(input.time_ms);
        let (sample, expected) = match input.change {
            Change::Cancelled { last, reason } => {
                // A duplicate cancellation from an earlier use of the same ID
                // must not abort a newer transaction. Processor supplies the
                // exact last accepted sample for a real cancellation.
                if self.active.get(&last.id).is_none_or(|a| a.raw != last) {
                    return None;
                }
                return self.cancel(last.id, CancelReason::Input(reason));
            }
            Change::Started(s) => (s, Phase::Down),
            Change::Moved(s) => (s, Phase::Move),
            Change::Ended(s) => (s, Phase::Up),
            Change::Hovered(_) | Change::Ignored { .. } => return None,
        };
        if stale {
            return None;
        }
        // Processor is the primary validator. Defend the public boundary against
        // forged lifecycle values without forwarding nonfinite data to an engine.
        if sample.phase != expected || !valid_sample(&sample) || sample.timestamp_ms > input.time_ms
        {
            return self.cancel(sample.id, CancelReason::InvalidInput);
        }
        if expected == Phase::Down {
            if self.active.contains_key(&sample.id) || self.active.len() == MAX_CONTACTS {
                return None;
            }
            if sample.tool != Tool::Pen && !(sample.tool == Tool::Mouse && sample.buttons.primary) {
                return None;
            }
            let next = self.next_id.checked_add(1)?; // Never wrap transaction identity.
            let point = map_sample(sample, self.viewport, input.time_ms).ok()?;
            let id = StrokeId(self.next_id);
            self.next_id = next;
            self.active.insert(
                sample.id,
                ActiveStroke {
                    id,
                    viewport: self.viewport,
                    last: point,
                    raw: sample,
                },
            );
            return Some(StrokeCommand::BeginStroke {
                stroke_id: id,
                sample: point,
            });
        }
        let active = self.active.get(&sample.id)?;
        if active.raw.tool != sample.tool {
            return self.cancel(sample.id, CancelReason::ToolChanged);
        }
        if sample.timestamp_ms < active.raw.timestamp_ms {
            return None;
        }
        if expected == Phase::Move && sample == active.raw && input.time_ms == active.last.time_ms {
            return None;
        }
        if expected == Phase::Move && sample.tool == Tool::Mouse && !sample.buttons.primary {
            return self.cancel(sample.id, CancelReason::MousePrimaryReleased);
        }
        let point = match map_sample(sample, active.viewport, input.time_ms) {
            Ok(point) => point,
            Err(_) => return self.cancel(sample.id, CancelReason::InvalidCoordinates),
        };
        let id = active.id;
        if expected == Phase::Up {
            self.active.remove(&sample.id);
            Some(StrokeCommand::CommitStroke {
                stroke_id: id,
                sample: point,
            })
        } else {
            let active = self.active.get_mut(&sample.id)?;
            active.last = point;
            active.raw = sample;
            Some(StrokeCommand::UpdateStroke {
                stroke_id: id,
                sample: point,
            })
        }
    }
}

fn map_sample(
    s: PointerSample,
    viewport: Viewport,
    time_ms: u64,
) -> Result<StrokeSample, coordinates::TransformError> {
    Ok(StrokeSample {
        pointer_id: s.id,
        tool: s.tool,
        position_client_px: s.position_px,
        position_document: viewport.to_document(s.position_px)?,
        // Mouse painting is supported as mouse input, never as pressure/tilt pen data.
        pressure: if s.tool == Tool::Mouse {
            None
        } else {
            s.pressure
        },
        tilt_degrees: if s.tool == Tool::Mouse {
            [None; 2]
        } else {
            s.tilt_degrees
        },
        buttons: s.buttons,
        source_time_ms: s.timestamp_ms,
        time_ms,
    })
}
