//! Minimal Android touch adapter for the shared pen/contact pipeline.
use paint_input::{Buttons, Phase, PointerSample, Tool};

/// Convert one ordered winit touch event to the shared pointer contract.
/// Winit does not expose Android's MotionEvent tool type here, so these events
/// remain explicitly `Tool::Touch`. Pressure, tilt, buttons, and source-clock
/// metadata stay missing; this adapter does not claim S Pen support.
pub fn touch_sample(
    id: u64,
    timestamp_ms: u64,
    phase: Phase,
    position_px: [f64; 2],
) -> PointerSample {
    PointerSample {
        id,
        timestamp_ms,
        source_counter: None,
        tool: Tool::Touch,
        phase,
        position_px,
        pressure: None,
        tilt_degrees: [None, None],
        buttons: Buttons::default(),
        in_contact: matches!(phase, Phase::Down | Phase::Move),
        in_range: !matches!(phase, Phase::Cancel),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_keeps_unknown_capabilities_missing_and_maps_contact_phases() {
        let down = touch_sample(7, 11, Phase::Down, [12.0, 34.0]);
        assert_eq!(down.tool, Tool::Touch);
        assert_eq!(down.pressure, None);
        assert_eq!(down.tilt_degrees, [None, None]);
        assert_eq!(down.source_counter, None);
        assert!(down.in_contact && down.in_range);

        let up = touch_sample(7, 12, Phase::Up, [15.0, 37.0]);
        assert!(!up.in_contact && up.in_range);
        let cancel = touch_sample(7, 13, Phase::Cancel, [15.0, 37.0]);
        assert!(!cancel.in_contact && !cancel.in_range);
    }
}
