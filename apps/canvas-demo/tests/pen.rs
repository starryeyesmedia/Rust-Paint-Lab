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
fn pen_is_excluded_from_replay_and_clips_artboard_crossings() {
    let mut session = CanvasSession::new([832, 532]);
    session.select_replay(Scenario::Completed, 0);
    session.pen(sample(1, 1, Phase::Down));
    assert!(!session.live().active());
    session.set_mode(Mode::Live, 2);
    session.pen(sample(1, 3, Phase::Down));
    let mut outside = sample(1, 4, Phase::Move);
    outside.position_px = [0.0, 116.0];
    session.pen(outside);
    assert!(session.live().active());
    let mut reentered = sample(1, 5, Phase::Move);
    reentered.position_px = [831.0, 116.0];
    reentered.pressure = Some(0.9);
    reentered.source_counter = paint_input::SourceCounter::new(300, 1_000);
    session.pen(reentered);
    let mut up = sample(1, 6, Phase::Up);
    up.position_px = [831.0, 116.0];
    session.pen(up);
    assert!(!session.live().active());
    let points = session.scene().committed().values().next().unwrap();
    assert!(
        points
            .iter()
            .all(|p| (0.0..=800.0).contains(&p.position_document.0[0]))
    );
    assert!(
        points
            .iter()
            .any(|p| p.pressure.is_some_and(|pressure| pressure > 0.85))
    );
    assert!(points.iter().all(|p| p.source_counter.is_none()));
}

#[test]
fn native_pen_cancellation_still_discards_after_paper_crossing() {
    let mut live = LiveMouse::new([832, 532]);
    live.pen(sample(1, 1, Phase::Down));
    let mut outside = sample(1, 2, Phase::Move);
    outside.position_px = [0.0, 116.0];
    live.pen(outside);
    live.pen(sample(1, 3, Phase::Cancel));
    assert!(!live.active());
    assert!(live.scene().provisional().is_empty());
    assert!(live.scene().committed().is_empty());
}

#[test]
fn pathological_pen_down_sequence_keeps_tracking_bounded_and_duplicates_do_not_replace() {
    let mut live = LiveMouse::new([832, 532]);
    let first = sample(1, 1, Phase::Down);
    live.pen(first);
    let mut duplicate = sample(1, 2, Phase::Down);
    duplicate.position_px = [300.0, 300.0];
    live.pen(duplicate);
    for id in 2..10_000 {
        let mut down = sample(id, id, Phase::Down);
        down.position_px = [116.0, 116.0];
        live.pen(down);
    }
    assert!(live.active());
    assert_eq!(
        live.scene().provisional().len(),
        paint_input::lifecycle::MAX_CONTACTS
    );
    let original = live.scene().provisional().values().next().unwrap()[0];
    assert_eq!(original.position_client_px, [116.5, 116.25]);
}
