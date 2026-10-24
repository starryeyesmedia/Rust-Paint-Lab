//! Time-based scheduling for nonintrusive recovery checkpoints.
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(700);
pub const RETRY_DELAY: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
pub struct AutosaveDebounce {
    observed_revision: Option<u64>,
    due_at: Option<Instant>,
}

impl AutosaveDebounce {
    /// Restart the quiet period when a new dirty revision is observed. Clean
    /// revisions cancel pending work and become the new baseline.
    pub fn observe(&mut self, revision: u64, dirty: bool, now: Instant) {
        if !dirty {
            self.observed_revision = Some(revision);
            self.due_at = None;
        } else if self.observed_revision != Some(revision) {
            self.observed_revision = Some(revision);
            self.due_at = Some(now + DEBOUNCE);
        }
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.due_at
    }

    pub fn ready(&self, now: Instant) -> bool {
        self.due_at.is_some_and(|due| now >= due)
    }

    pub fn submitted(&mut self) {
        self.due_at = None;
    }

    pub fn retry_later(&mut self, now: Instant) {
        self.due_at = Some(now + RETRY_DELAY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successive_changes_coalesce_until_a_quiet_period() {
        let start = Instant::now();
        let mut schedule = AutosaveDebounce::default();
        schedule.observe(1, true, start);
        assert_eq!(schedule.deadline(), Some(start + DEBOUNCE));
        schedule.observe(1, true, start + Duration::from_millis(300));
        assert_eq!(schedule.deadline(), Some(start + DEBOUNCE));
        schedule.observe(2, true, start + Duration::from_millis(500));
        assert_eq!(
            schedule.deadline(),
            Some(start + Duration::from_millis(1200))
        );
        assert!(!schedule.ready(start + Duration::from_millis(1199)));
        assert!(schedule.ready(start + Duration::from_millis(1200)));
    }

    #[test]
    fn clean_state_cancels_pending_autosave_and_failures_back_off() {
        let start = Instant::now();
        let mut schedule = AutosaveDebounce::default();
        schedule.observe(4, true, start);
        schedule.observe(4, false, start + Duration::from_millis(100));
        assert_eq!(schedule.deadline(), None);
        schedule.observe(5, true, start + Duration::from_millis(200));
        schedule.retry_later(start + Duration::from_millis(1000));
        assert_eq!(
            schedule.deadline(),
            Some(start + Duration::from_millis(3000))
        );
        assert!(!schedule.ready(start + Duration::from_millis(2999)));
        schedule.submitted();
        assert_eq!(schedule.deadline(), None);
    }
}
