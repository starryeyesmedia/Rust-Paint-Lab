//! Hardware-independent, bounded two-contact navigation. All positions are physical
//! client pixels; the shell decides whether UI or active painting owns input.
use std::collections::BTreeMap;
use std::path::PathBuf;
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

/// Captures each direct-touch contact to one navigation surface for its full
/// lifetime. A gesture cannot start on the canvas and finish on a reference
/// window, or combine contacts from different surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TouchTarget {
    Canvas,
    Reference(PathBuf),
    Suppressed,
}

#[derive(Default)]
pub struct TouchTargetRouter {
    owners: BTreeMap<u64, TouchTarget>,
    active: Option<TouchTarget>,
    active_contacts: usize,
}

impl TouchTargetRouter {
    pub fn start(&mut self, id: u64, requested: TouchTarget) -> TouchTarget {
        if let Some(existing) = self.owners.get(&id) {
            return existing.clone();
        }
        let target = if self.active_contacts == 0
            || (self.active.as_ref() == Some(&requested) && self.active_contacts < 2)
        {
            requested
        } else {
            TouchTarget::Suppressed
        };
        if target != TouchTarget::Suppressed {
            self.active = Some(target.clone());
            self.active_contacts += 1;
        }
        self.owners.insert(id, target.clone());
        target
    }

    pub fn target(&self, id: u64) -> Option<TouchTarget> {
        self.owners.get(&id).cloned()
    }

    pub fn finish(&mut self, id: u64) {
        if let Some(target) = self.owners.remove(&id)
            && target != TouchTarget::Suppressed
        {
            self.active_contacts = self.active_contacts.saturating_sub(1);
            if self.active_contacts == 0 {
                self.active = None;
            }
        }
    }

    pub fn active_target(&self) -> Option<&TouchTarget> {
        self.active.as_ref()
    }

    pub fn clear(&mut self) {
        self.owners.clear();
        self.active = None;
        self.active_contacts = 0;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_gesture_keeps_one_surface_owner_until_all_contacts_end() {
        let reference = TouchTarget::Reference(PathBuf::from("moodboard.jpg"));
        let mut router = TouchTargetRouter::default();
        assert_eq!(router.start(1, reference.clone()), reference);
        assert_eq!(router.start(2, reference.clone()), reference);
        assert_eq!(router.start(3, reference.clone()), TouchTarget::Suppressed);
        assert_eq!(
            router.start(4, TouchTarget::Canvas),
            TouchTarget::Suppressed
        );
        router.finish(1);
        assert_eq!(router.active_target(), Some(&reference));
        router.finish(2);
        router.finish(3);
        router.finish(4);
        assert_eq!(router.active_target(), None);
        assert_eq!(router.start(4, TouchTarget::Canvas), TouchTarget::Canvas);
        router.clear();
        assert_eq!(router.target(4), None);
    }
}
