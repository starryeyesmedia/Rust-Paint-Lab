use canvas_demo::{
    interaction::{CanvasSession, LiveMouse, Mode, MouseEvent},
    playback::Scenario,
};
use paint_input::{Buttons, Phase, PointerSample, Tool};
fn sample(id: u64, time: u64, phase: Phase) -> PointerSample {
    PointerSample {
        source_counter: None,
        id,
        timestamp_ms: time,
        tool: Tool::Pen,
        phase,
        position_px: [116.5, 116.25],
        pressure: Some(0.25),
        tilt_degrees: [Some(-25.0), None],
        buttons: Buttons {
            barrel: true,
            ..Default::default()
        },
        in_contact: matches!(phase, Phase::Down | Phase::Move),
        in_range: true,
    }
}
#[test]
fn pen_pipeline_preserves_pressure_subpixels_tilt_buttons_and_final_sample() {
    let mut live = LiveMouse::new([832, 532]);
    live.pen(sample(7, 0, Phase::Hover));
    assert_eq!(live.scene().sample_count(), 0);
    assert_eq!(live.last_pen().unwrap().pressure, Some(0.25));
    assert!(live.last_pen().unwrap().buttons.barrel);
    live.pen(sample(7, 1, Phase::Down));
    let mut s = sample(7, 2, Phase::Move);
    s.pressure = Some(0.8);
    s.position_px = [216.75, 216.5];
    live.pen(s);
    live.pen(sample(7, 3, Phase::Up));
    let points = live.scene().committed().values().next().unwrap();
    assert_eq!(points.len(), 3);
    assert_eq!(points[0].position_document.0, [100.5, 100.25]);
    assert_eq!(points[1].pressure, Some(0.8));
    assert_eq!(points[0].tilt_degrees, [Some(-25.0), None]);
    assert!(points[0].buttons.barrel);
    live.pen(sample(7, 4, Phase::Cancel));
    assert_eq!(live.scene().committed().len(), 1);
}
#[test]
fn pen_ids_mouse_and_cancellation_are_isolated() {
    let mut live = LiveMouse::new([832, 532]);
    live.pen(sample(1, 1, Phase::Down));
    live.pen(sample(2, 1, Phase::Down));
    live.handle(MouseEvent::Moved([200.0, 200.0]), 2);
    live.handle(
        MouseEvent::Button {
            primary: true,
            pressed: true,
        },
        2,
    );
    assert_eq!(live.scene().provisional().len(), 3);
    live.pen(sample(1, 3, Phase::Cancel));
    assert_eq!(live.scene().provisional().len(), 2);
    live.handle(MouseEvent::Focus(false), 4);
    assert!(!live.active());
    assert!(live.last_pen().is_none());
    assert!(live.scene().provisional().is_empty());
    assert!(live.scene().committed().is_empty());
}
#[test]
fn pen_is_excluded_from_replay_and_interrupted_at_bounds() {
    let mut session = CanvasSession::new([832, 532]);
    session.select_replay(Scenario::Completed, 0);
    session.pen(sample(1, 1, Phase::Down));
    assert!(!session.live().active());
    session.set_mode(Mode::Live, 2);
    session.pen(sample(1, 3, Phase::Down));
    let mut outside = sample(1, 4, Phase::Move);
    outside.position_px = [0.0, 0.0];
    session.pen(outside);
    session.pen(sample(1, 5, Phase::Up));
    assert_eq!(session.scene().sample_count(), 0);
}
