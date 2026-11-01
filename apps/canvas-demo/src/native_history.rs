//! Bounded native pointer-history policy shared by the Windows adapter and tests.
//! No OS handles or APIs live here.
use std::collections::{BTreeMap, VecDeque};

pub const MAX_NATIVE_HISTORY_SAMPLES: u32 = 256;
pub const MAX_QUEUED_PEN_EVENTS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum NativeHistoryFailure {
    HistoryCountQuery = 0,
    InvalidHistoryCount = 1,
    HistoryRead = 2,
    CoordinateConversion = 3,
    PenEventQueueOverflow = 4,
}

impl NativeHistoryFailure {
    pub const ALL: [Self; 5] = [
        Self::HistoryCountQuery,
        Self::InvalidHistoryCount,
        Self::HistoryRead,
        Self::CoordinateConversion,
        Self::PenEventQueueOverflow,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::HistoryCountQuery => "history_count_query",
            Self::InvalidHistoryCount => "invalid_history_count",
            Self::HistoryRead => "history_read",
            Self::CoordinateConversion => "coordinate_conversion",
            Self::PenEventQueueOverflow => "pen_event_queue_overflow",
        }
    }
}

/// Fixed-size category counts suitable for a bounded diagnostic message.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NativeHistoryFailures {
    counts: [u64; NativeHistoryFailure::ALL.len()],
}

impl NativeHistoryFailures {
    pub fn record(&mut self, failure: NativeHistoryFailure) {
        let slot = &mut self.counts[failure as usize];
        *slot = slot.saturating_add(1);
    }

    pub fn merge(&mut self, other: Self) {
        for (slot, count) in self.counts.iter_mut().zip(other.counts) {
            *slot = slot.saturating_add(count);
        }
    }

    pub fn count(self, failure: NativeHistoryFailure) -> u64 {
        self.counts[failure as usize]
    }

    pub fn is_empty(self) -> bool {
        self.counts.iter().all(|count| *count == 0)
    }

    pub fn from_counts(counts: [u64; NativeHistoryFailure::ALL.len()]) -> Self {
        Self { counts }
    }
}

pub fn validate_history_count_query(
    succeeded: bool,
    count: u32,
) -> Result<usize, NativeHistoryFailure> {
    if !succeeded {
        return Err(NativeHistoryFailure::HistoryCountQuery);
    }
    if !(1..=MAX_NATIVE_HISTORY_SAMPLES).contains(&count) {
        return Err(NativeHistoryFailure::InvalidHistoryCount);
    }
    Ok(count as usize)
}

pub fn validate_history_read(
    succeeded: bool,
    capacity: usize,
    count: u32,
) -> Result<usize, NativeHistoryFailure> {
    if !succeeded {
        return Err(NativeHistoryFailure::HistoryRead);
    }
    if count == 0 || count as usize > capacity {
        return Err(NativeHistoryFailure::InvalidHistoryCount);
    }
    Ok(count as usize)
}

pub fn validate_coordinate_conversion(succeeded: bool) -> Result<(), NativeHistoryFailure> {
    succeeded
        .then_some(())
        .ok_or(NativeHistoryFailure::CoordinateConversion)
}

/// A bounded queue that drops the incomplete batch and asks the consumer to
/// cancel any provisional stroke after native history becomes unreliable.
pub struct NativePenQueue<T> {
    events: VecDeque<T>,
    lost: bool,
}

impl<T> Default for NativePenQueue<T> {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            lost: false,
        }
    }
}

impl<T> NativePenQueue<T> {
    pub fn push_batch(&mut self, events: Vec<T>) -> Result<(), NativeHistoryFailure> {
        if self.events.len().saturating_add(events.len()) > MAX_QUEUED_PEN_EVENTS {
            return Err(self.fail_closed(NativeHistoryFailure::PenEventQueueOverflow));
        }
        self.events.extend(events);
        Ok(())
    }

    pub fn push(&mut self, event: T) -> Result<(), NativeHistoryFailure> {
        if self.events.len() >= MAX_QUEUED_PEN_EVENTS {
            return Err(self.fail_closed(NativeHistoryFailure::PenEventQueueOverflow));
        }
        self.events.push_back(event);
        Ok(())
    }

    pub fn fail_closed(&mut self, failure: NativeHistoryFailure) -> NativeHistoryFailure {
        self.events.clear();
        self.lost = true;
        failure
    }

    /// Preserve the existing capture notification behavior while making the
    /// generic lost-capture signal share the same consumer API.
    pub fn mark_lost(&mut self) {
        self.lost = true;
    }

    pub fn take_lost(&mut self) -> bool {
        std::mem::take(&mut self.lost)
    }

    pub fn drain(&mut self) -> Vec<T> {
        self.events.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Keep attribute changes even when a driver supplies identical time/position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeStamp {
    pub counter: u64,
    pub timestamp_ms: u32,
    pub position_himetric: [i32; 2],
    pub pointer_flags: u32,
    pub pen_flags: u32,
    pub pressure: u32,
    pub tilt: [i32; 2],
    pub mask: u32,
}

#[derive(Default)]
pub struct HistoryDeduplicator {
    last: BTreeMap<u32, NativeStamp>,
}

impl HistoryDeduplicator {
    pub fn len(&self) -> usize {
        self.last.len()
    }

    pub fn is_empty(&self) -> bool {
        self.last.is_empty()
    }

    pub fn accept(&mut self, id: u32, stamp: NativeStamp) -> bool {
        if self.last.get(&id).is_some_and(|old| {
            *old == stamp || (old.counter != 0 && stamp.counter != 0 && stamp.counter < old.counter)
        }) {
            return false;
        }
        if self.last.len() >= 128 && !self.last.contains_key(&id) {
            self.last.pop_first();
        }
        self.last.insert(id, stamp);
        true
    }
}
