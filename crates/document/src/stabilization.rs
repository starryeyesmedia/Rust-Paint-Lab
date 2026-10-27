//! Deterministic One Euro position filtering for active raster strokes.
//!
//! The brush's `smoothing` value controls the minimum cutoff. Slow motion is
//! filtered strongly while faster motion raises the cutoff to retain corners
//! and quick direction changes. Pressure and tilt stay on the original sample.
//! Each stroke owns one small filter; cancelling it drops all filter state.

#[derive(Debug, Clone, Copy)]
struct Axis {
    raw: f64,
    filtered: f64,
    derivative: f64,
}

/// Small constant-memory, time-aware position filter for one stroke.
#[derive(Debug, Clone, Copy)]
pub struct StrokeStabilizer {
    axes: [Axis; 2],
    last_time_ms: u64,
}

impl StrokeStabilizer {
    pub fn new(position: [f64; 2], time_ms: u64) -> Self {
        Self {
            axes: position.map(|value| Axis {
                raw: value,
                filtered: value,
                derivative: 0.0,
            }),
            last_time_ms: time_ms,
        }
    }

    /// Begin a disconnected segment without carrying velocity across a clipped
    /// document edge. This preserves a clean re-entry while retaining the
    /// surrounding stroke transaction.
    pub fn reset(&mut self, position: [f64; 2], time_ms: u64) {
        *self = Self::new(position, time_ms);
    }

    pub fn update(&mut self, position: [f64; 2], time_ms: u64, amount: f64) -> [f64; 2] {
        if amount <= 0.0 {
            self.reset(position, time_ms.max(self.last_time_ms));
            return position;
        }

        let amount = amount.clamp(0.0, 1.0);
        let elapsed_ms = time_ms.saturating_sub(self.last_time_ms);
        // Pointer timestamps are millisecond-resolution on supported adapters.
        // Repeated timestamps get a stable 120 Hz interval instead of an
        // unbounded derivative; long pauses are capped so the next sample does
        // not jump backwards toward an obsolete filter state.
        let dt = if elapsed_ms == 0 {
            1.0 / 120.0
        } else {
            (elapsed_ms as f64 / 1000.0).clamp(0.001, 0.1)
        };
        let min_cutoff = 12.0 * (1.0 - amount).powi(2) + 0.6;
        let beta = 0.015 + 0.035 * amount;
        let alpha = |cutoff: f64| {
            let tau = 1.0 / (std::f64::consts::TAU * cutoff);
            1.0 / (1.0 + tau / dt)
        };

        let filtered = std::array::from_fn(|index| {
            let axis = &mut self.axes[index];
            let raw_derivative = (position[index] - axis.raw) / dt;
            axis.derivative += alpha(1.0) * (raw_derivative - axis.derivative);
            let cutoff = min_cutoff + beta * axis.derivative.abs();
            axis.filtered += alpha(cutoff) * (position[index] - axis.filtered);
            axis.raw = position[index];
            axis.filtered
        });
        self.last_time_ms = self.last_time_ms.max(time_ms);
        filtered
    }
}

#[cfg(test)]
mod tests {
    use super::StrokeStabilizer;

    #[test]
    fn disabled_filter_is_exact_identity_and_resets_velocity() {
        let mut filter = StrokeStabilizer::new([10.0, 10.0], 5);
        assert_eq!(filter.update([20.0, 30.0], 20, 0.0), [20.0, 30.0]);
        assert_eq!(filter.update([21.0, 31.0], 30, 0.0), [21.0, 31.0]);
    }

    #[test]
    fn smoothing_reduces_slow_jitter_and_preserves_fast_motion() {
        let mut filter = StrokeStabilizer::new([0.0, 40.0], 0);
        let mut raw_error = 0.0;
        let mut smooth_error = 0.0;
        for index in 1..=120 {
            let x = index as f64 * 2.0;
            let jitter = if index % 2 == 0 { 1.4 } else { -1.4 };
            let raw = [x, 40.0 + jitter];
            let stabilized = filter.update(raw, index * 8, 0.85);
            raw_error += (raw[1] - 40.0).powi(2);
            smooth_error += (stabilized[1] - 40.0).powi(2);
        }
        assert!(
            smooth_error < raw_error * 0.55,
            "{smooth_error} >= {raw_error}"
        );

        let before = filter.update([242.0, 40.0], 968, 0.85);
        let after = filter.update([442.0, 40.0], 976, 0.85);
        assert!(
            after[0] - before[0] > 150.0,
            "fast path lagged: {before:?} {after:?}"
        );
    }

    #[test]
    fn repeated_and_out_of_order_times_remain_finite_and_bounded() {
        let mut filter = StrokeStabilizer::new([2.0, 3.0], 10);
        for (time, x) in [(10, 4.0), (9, 6.0), (10, 8.0), (10_000, 10.0)] {
            let point = filter.update([x, 3.0], time, 1.0);
            assert!(point.iter().all(|value| value.is_finite()));
            assert!(point[0] >= 2.0 && point[0] <= 10.0);
        }
    }
}
