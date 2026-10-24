//! Bounded raw packet identity for the platform adapter. No OS handles or APIs.
//! Keep attribute changes even when a driver supplies identical time/position.
use std::collections::BTreeMap;
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
