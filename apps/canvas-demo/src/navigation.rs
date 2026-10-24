//! Hardware-independent, bounded two-contact navigation. All positions are physical
//! client pixels; the shell decides whether UI or active painting owns input.
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Navigation {
    pub zoom: f64,
    pub pan: [f64; 2],
}
impl Default for Navigation {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: [0.0; 2],
        }
    }
}
impl Navigation {
    pub fn pinch(&mut self, center: [f64; 2], from: [f64; 2], to: [f64; 2], ratio: f64) -> bool {
        if !self.zoom.is_finite()
            || !(0.25..=8.0).contains(&self.zoom)
            || !self.pan.iter().all(|n| n.is_finite())
        {
            return false;
        }
        if !center
            .into_iter()
            .chain(from)
            .chain(to)
            .chain([ratio])
            .all(f64::is_finite)
            || ratio <= 0.0
        {
            return false;
        }
        let zoom = (self.zoom * ratio).clamp(0.25, 8.0);
        let ratio = zoom / self.zoom;
        let pan = std::array::from_fn(|i| {
            (to[i] - center[i] - ratio * (from[i] - center[i] - self.pan[i]))
                .clamp(-32768.0, 32768.0)
        });
        let changed = self.zoom != zoom || self.pan != pan;
        self.zoom = zoom;
        self.pan = pan;
        changed
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Start,
    Move,
    End,
    Cancel,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pinch {
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub ratio: f64,
}
#[derive(Default)]
pub struct TouchNavigation {
    contacts: BTreeMap<u64, [f64; 2]>,
    previous: Option<([u64; 2], [f64; 2], f64)>,
}
impl TouchNavigation {
    pub fn clear(&mut self) {
        self.contacts.clear();
        self.previous = None;
    }
    pub fn active_count(&self) -> usize {
        self.contacts.len()
    }
    pub fn event(&mut self, id: u64, phase: TouchPhase, p: [f64; 2]) -> Option<Pinch> {
        if !p.iter().all(|n| n.is_finite() && n.abs() <= 32768.0) {
            self.clear();
            return None;
        }
        match phase {
            TouchPhase::Start => {
                if self.contacts.contains_key(&id) {
                    self.previous = None;
                } else if self.contacts.len() >= 16 {
                    self.clear();
                    return None;
                }
                self.contacts.insert(id, p);
            }
            TouchPhase::Move => {
                *self.contacts.get_mut(&id)? = p;
            }
            TouchPhase::End | TouchPhase::Cancel => {
                self.contacts.remove(&id);
                self.previous = None;
                return None;
            }
        }
        if self.contacts.len() != 2 {
            self.previous = None;
            return None;
        }
        let pair: Vec<_> = self.contacts.iter().collect();
        let ids = [*pair[0].0, *pair[1].0];
        let a = pair[0].1;
        let b = pair[1].1;
        let center = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let distance = (a[0] - b[0]).hypot(a[1] - b[1]);
        let previous = self.previous.replace((ids, center, distance));
        let (old_ids, from, old_distance) = previous?;
        if old_ids != ids || old_distance < 4.0 || distance < 4.0 {
            return None;
        }
        Some(Pinch {
            from,
            to: center,
            ratio: distance / old_distance,
        })
    }
}
