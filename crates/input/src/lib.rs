//! Platform-independent pointer samples. No windowing, rendering, or OS dependencies.
#![forbid(unsafe_code)]

/// Device category, kept distinct so touch is never mistaken for a paint stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Touch,
    Mouse,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Down,
    Move,
    Up,
    Hover,
    Cancel,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Buttons {
    pub primary: bool,
    pub secondary: bool,
    pub barrel: bool,
    pub eraser: bool,
    pub inverted: bool,
}

/// Coordinates are physical pixels relative to the window's client origin.
/// IDs are adapter-local and may be reused after Up/Cancel. Timestamps are
/// adapter-relative milliseconds, not wall time; adapters document wraparound.
/// Missing capabilities remain None (in particular, missing pressure is not 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerSample {
    pub id: u64,
    pub timestamp_ms: u64,
    pub tool: Tool,
    pub phase: Phase,
    pub position_px: [f64; 2],
    pub pressure: Option<f32>,
    pub tilt_degrees: [Option<f32>; 2],
    pub buttons: Buttons,
    pub in_contact: bool,
    pub in_range: bool,
}

/// Normalize a reported pressure range; reject invalid readings/ranges and
/// clamp finite outliers. Call only when the platform capability mask is set.
pub fn normalize_pressure(value: f32, maximum: f32) -> Option<f32> {
    if !value.is_finite() || !maximum.is_finite() || maximum <= 0.0 {
        return None;
    }
    Some((value / maximum).clamp(0.0, 1.0))
}

/// Preserve each axis's availability independently; reject non-finite readings.
pub fn normalize_tilt(degrees: f32) -> Option<f32> {
    degrees.is_finite().then(|| degrees.clamp(-90.0, 90.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_endpoints_and_midpoint() {
        assert_eq!(normalize_pressure(0.0, 1024.0), Some(0.0));
        assert_eq!(normalize_pressure(512.0, 1024.0), Some(0.5));
        assert_eq!(normalize_pressure(1024.0, 1024.0), Some(1.0));
    }

    #[test]
    fn pressure_clamps_finite_outliers() {
        assert_eq!(normalize_pressure(-1.0, 1024.0), Some(0.0));
        assert_eq!(normalize_pressure(2048.0, 1024.0), Some(1.0));
    }

    #[test]
    fn pressure_rejects_invalid_range() {
        for max in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(normalize_pressure(1.0, max), None);
        }
    }

    #[test]
    fn pressure_rejects_nonfinite_readings() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(normalize_pressure(value, 1024.0), None);
        }
    }

    #[test]
    fn pressure_uses_the_adapter_range_instead_of_assuming_windows_units() {
        assert_eq!(normalize_pressure(0.75, 1.0), Some(0.75));
        assert_eq!(normalize_pressure(3072.0, 4096.0), Some(0.75));
    }

    #[test]
    fn tilt_preserves_signed_angles_and_clamps_outliers() {
        for (input, expected) in [(-120.0, -90.0), (-45.0, -45.0), (0.0, 0.0), (120.0, 90.0)] {
            assert_eq!(normalize_tilt(input), Some(expected));
        }
    }

    #[test]
    fn tilt_rejects_nonfinite_readings() {
        assert_eq!(normalize_tilt(f32::NAN), None);
        assert_eq!(normalize_tilt(f32::INFINITY), None);
        assert_eq!(normalize_tilt(f32::NEG_INFINITY), None);
    }
}
