//! Bounded, synchronous lifecycle processing; no clocks, threads, or OS calls.
use crate::{Phase, PointerSample, Tool};
use std::collections::BTreeMap;

pub const MAX_CONTACTS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Cancelled,
    CaptureLost,
    Interrupted,
    Replaced,
    InvalidData,
    OutOfOrder,
    MissingDown,
    Capacity,
    EndOfReplay,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Started(PointerSample),
    Moved(PointerSample),
    Ended(PointerSample),
    Hovered(PointerSample),
    Cancelled { last: PointerSample, reason: Reason },
    Ignored { id: u64, reason: Reason },
}

/// `time_ms` is nondecreasing dispatch time. Samples retain original timestamps;
/// cancellation carries the last accepted contact sample, not invented pen data.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub time_ms: u64,
    pub change: Change,
}

#[derive(Debug, Default)]
pub struct Processor {
    active: BTreeMap<u64, PointerSample>,
    time_ms: u64,
}

impl Processor {
    pub fn active(&self) -> &BTreeMap<u64, PointerSample> {
        &self.active
    }
    pub fn time_ms(&self) -> u64 {
        self.time_ms
    }

    fn output(&self, change: Change) -> Output {
        Output {
            time_ms: self.time_ms,
            change,
        }
    }

    /// Loss/cancellation is honored even when its timestamp is stale.
    pub fn cancel(&mut self, id: u64, time_ms: u64, reason: Reason) -> Output {
        self.time_ms = self.time_ms.max(time_ms);
        let change = match self.active.remove(&id) {
            Some(last) => Change::Cancelled { last, reason },
            None => Change::Ignored {
                id,
                reason: Reason::MissingDown,
            },
        };
        self.output(change)
    }

    /// Stable ID order makes global cleanup deterministic. No history is retained.
    pub fn interrupt(&mut self, time_ms: u64, reason: Reason) -> Vec<Output> {
        self.time_ms = self.time_ms.max(time_ms);
        std::mem::take(&mut self.active)
            .into_values()
            .map(|last| self.output(Change::Cancelled { last, reason }))
            .collect()
    }

    pub fn process(&mut self, sample: PointerSample) -> Vec<Output> {
        let id = sample.id;
        if sample.phase == Phase::Cancel {
            return vec![self.cancel(id, sample.timestamp_ms, Reason::Cancelled)];
        }
        if sample.timestamp_ms < self.time_ms {
            // A stale terminal must still clean up. Other stale events cannot
            // mutate a newer contact (including reused IDs).
            return vec![if sample.phase == Phase::Up {
                self.cancel(id, sample.timestamp_ms, Reason::OutOfOrder)
            } else {
                self.output(Change::Ignored {
                    id,
                    reason: Reason::OutOfOrder,
                })
            }];
        }
        self.time_ms = sample.timestamp_ms;
        if !valid_sample(&sample) {
            return vec![if self.active.contains_key(&id) {
                self.cancel(id, sample.timestamp_ms, Reason::InvalidData)
            } else {
                self.output(Change::Ignored {
                    id,
                    reason: Reason::InvalidData,
                })
            }];
        }
        let mut outputs = Vec::with_capacity(2);
        if let Some(last) = self.active.get(&id)
            && (sample.phase == Phase::Down
                || last.tool != sample.tool
                || sample.phase == Phase::Hover)
        {
            outputs.push(self.cancel(id, sample.timestamp_ms, Reason::Replaced));
        }
        let change = match sample.phase {
            Phase::Down if self.active.len() < MAX_CONTACTS => {
                self.active.insert(id, sample);
                Change::Started(sample)
            }
            Phase::Down => Change::Ignored {
                id,
                reason: Reason::Capacity,
            },
            Phase::Move if self.active.contains_key(&id) => {
                self.active.insert(id, sample);
                Change::Moved(sample)
            }
            Phase::Up if self.active.remove(&id).is_some() => Change::Ended(sample),
            Phase::Hover => Change::Hovered(sample),
            _ => Change::Ignored {
                id,
                reason: Reason::MissingDown,
            },
        };
        outputs.push(self.output(change));
        outputs
    }
}

/// Input adapters normalize units first. Reject malformed values instead of
/// allowing NaN, unsupported phase/contact combinations, or invalid ranges downstream.
pub fn valid_sample(s: &PointerSample) -> bool {
    s.position_px.iter().all(|x| x.is_finite())
        && s.pressure
            .is_none_or(|p| p.is_finite() && (0.0..=1.0).contains(&p))
        && s.tilt_degrees
            .iter()
            .all(|v| v.is_none_or(|t| t.is_finite() && (-90.0..=90.0).contains(&t)))
        && match s.phase {
            Phase::Down | Phase::Move => s.in_contact && s.in_range,
            Phase::Up | Phase::Cancel => !s.in_contact,
            Phase::Hover => {
                !s.in_contact && s.in_range && matches!(s.tool, Tool::Pen | Tool::Mouse)
            }
        }
}

/// Extend a wrapping 32-bit millisecond stream. Deltas below half the modulus
/// are forward (including rollover); larger deltas are stale and clamp to the
/// last time without moving the raw anchor. Requires observations < 2^31 ms apart.
#[derive(Debug, Default)]
pub struct MillisecondClock32 {
    last: Option<u32>,
    extended: u64,
}

impl MillisecondClock32 {
    pub fn extend(&mut self, raw: u32) -> u64 {
        match self.last {
            None => {
                self.last = Some(raw);
                self.extended = u64::from(raw);
            }
            Some(last) => {
                let delta = raw.wrapping_sub(last);
                if delta < (1 << 31) {
                    self.extended = self.extended.saturating_add(u64::from(delta));
                    self.last = Some(raw);
                }
            }
        }
        self.extended
    }
}
