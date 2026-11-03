use canvas_demo::scene::CanvasMapping;
use canvas_demo::{navigation::Navigation, navigation::TouchNavigation, navigation::TouchPhase};
use canvas_demo::{navigation::TouchTarget, navigation::TouchTargetRouter};
use paint_input::{
    Buttons, Phase, PointerSample, SourceCounter, Tool,
    lifecycle::{Change, Processor, Reason},
};
use paint_stroke::{Router, StrokeCommand, coordinates::Viewport};

fn sample(
    id: u64,
    timestamp_ms: u64,
    tool: Tool,
    phase: Phase,
    position_px: [f64; 2],
) -> PointerSample {
    PointerSample {
        id,
        timestamp_ms,
        source_counter: None,
        tool,
        phase,
        position_px,
        pressure: None,
        tilt_degrees: [None; 2],
        buttons: Buttons::default(),
        in_contact: matches!(phase, Phase::Down | Phase::Move),
        in_range: true,
    }
}

fn feed(
    processor: &mut Processor,
    router: &mut Router,
    sample: PointerSample,
) -> Vec<StrokeCommand> {
    processor
        .process(sample)
        .iter()
        .filter_map(|output| router.route(output))
        .collect()
}

#[test]
fn pen_hover_is_nonpainting_and_contact_metadata_and_history_order_are_preserved() {
    let mut processor = Processor::default();
    let mut router = Router::new(Viewport::default());

    let hover = sample(4, 9, Tool::Pen, Phase::Hover, [10.0, 20.0]);
    let hover_output = processor.process(hover);
    assert!(matches!(hover_output[0].change, Change::Hovered(_)));
    assert_eq!(router.route(&hover_output[0]), None);

    let mut down = sample(4, 10, Tool::Pen, Phase::Down, [10.25, 20.5]);
    down.source_counter = SourceCounter::new(100, 1_000);
    down.pressure = Some(0.125);
    down.tilt_degrees = [Some(-37.5), Some(12.25)];
    let commands = feed(&mut processor, &mut router, down);
    assert_eq!(commands.len(), 1);
    let stroke_id = match &commands[0] {
        StrokeCommand::BeginStroke { stroke_id, sample } => {
            assert_eq!(sample.pressure, Some(0.125));
            assert_eq!(sample.tilt_degrees, [Some(-37.5), Some(12.25)]);
            assert_eq!(sample.source_counter, down.source_counter);
            *stroke_id
        }
        other => panic!("expected BeginStroke, got {other:?}"),
    };

    let mut history_a = sample(4, 11, Tool::Pen, Phase::Move, [11.0, 21.0]);
    history_a.source_counter = SourceCounter::new(200, 1_000);
    history_a.pressure = Some(0.333);
    history_a.tilt_degrees = [Some(5.0), None];
    let mut history_b = sample(4, 11, Tool::Pen, Phase::Move, [12.0, 22.0]);
    history_b.source_counter = SourceCounter::new(201, 1_000);
    history_b.pressure = Some(0.444);
    history_b.tilt_degrees = [None, Some(-18.0)];
    let first = feed(&mut processor, &mut router, history_a);
    let second = feed(&mut processor, &mut router, history_b);
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    for (command, expected_counter, pressure, tilt) in [
        (
            &first[0],
            history_a.source_counter,
            Some(0.333),
            [Some(5.0), None],
        ),
        (
            &second[0],
            history_b.source_counter,
            Some(0.444),
            [None, Some(-18.0)],
        ),
    ] {
        match command {
            StrokeCommand::UpdateStroke {
                stroke_id: actual,
                sample,
            } => {
                assert_eq!(*actual, stroke_id);
                assert_eq!(sample.source_counter, expected_counter);
                assert_eq!(sample.pressure, pressure);
                assert_eq!(sample.tilt_degrees, tilt);
            }
            other => panic!("expected UpdateStroke, got {other:?}"),
        }
    }

    let mut up = sample(4, 12, Tool::Pen, Phase::Up, [13.0, 23.0]);
    up.pressure = Some(0.875);
    up.tilt_degrees = [Some(1.0), Some(2.0)];
    let commands = feed(&mut processor, &mut router, up);
    assert!(
        matches!(commands.as_slice(), [StrokeCommand::CommitStroke { stroke_id: id, sample }] if *id == stroke_id && sample.pressure == Some(0.875) && sample.tilt_degrees == [Some(1.0), Some(2.0)])
    );
}

#[test]
fn touch_and_unknown_are_not_painted_and_mouse_does_not_inherit_pen_capabilities() {
    let mut processor = Processor::default();
    let mut router = Router::new(Viewport::default());

    let mut mouse = sample(1, 1, Tool::Mouse, Phase::Down, [30.0, 40.0]);
    mouse.buttons.primary = true;
    mouse.pressure = Some(0.7);
    mouse.tilt_degrees = [Some(44.0), Some(-20.0)];
    let commands = feed(&mut processor, &mut router, mouse);
    assert!(
        matches!(commands.as_slice(), [StrokeCommand::BeginStroke { sample, .. }] if sample.tool == Tool::Mouse && sample.pressure.is_none() && sample.tilt_degrees == [None, None])
    );

    for (index, (id, tool)) in [(2, Tool::Touch), (3, Tool::Unknown)]
        .into_iter()
        .enumerate()
    {
        let start = 2 + (index as u64 * 3);
        let down = sample(id, start, tool, Phase::Down, [50.0, 60.0]);
        assert!(feed(&mut processor, &mut router, down).is_empty());
        let moved = sample(id, start + 1, tool, Phase::Move, [55.0, 65.0]);
        assert!(feed(&mut processor, &mut router, moved).is_empty());
        let up = sample(id, start + 2, tool, Phase::Up, [55.0, 65.0]);
        assert!(feed(&mut processor, &mut router, up).is_empty());
    }
    assert_eq!(
        router.active_count(),
        1,
        "the mouse stroke stays independent"
    );
    assert!(matches!(
        feed(
            &mut processor,
            &mut router,
            sample(1, 8, Tool::Mouse, Phase::Up, [30.0, 40.0])
        )
        .as_slice(),
        [StrokeCommand::CommitStroke { .. }]
    ));
    assert_eq!(router.active_count(), 0);
}

#[test]
fn unavailable_pen_capabilities_remain_unavailable_per_axis() {
    let mut processor = Processor::default();
    let mut router = Router::new(Viewport::default());
    let mut down = sample(6, 1, Tool::Pen, Phase::Down, [4.0, 5.0]);
    down.tilt_degrees = [None, Some(16.0)];
    let commands = feed(&mut processor, &mut router, down);
    assert!(
        matches!(commands.as_slice(), [StrokeCommand::BeginStroke { sample, .. }] if sample.pressure.is_none() && sample.tilt_degrees == [None, Some(16.0)])
    );

    let moved = sample(6, 2, Tool::Pen, Phase::Move, [5.0, 6.0]);
    let commands = feed(&mut processor, &mut router, moved);
    assert!(
        matches!(commands.as_slice(), [StrokeCommand::UpdateStroke { sample, .. }] if sample.pressure.is_none() && sample.tilt_degrees == [None, None])
    );
    let commands = feed(
        &mut processor,
        &mut router,
        sample(6, 3, Tool::Pen, Phase::Up, [5.0, 6.0]),
    );
    assert!(matches!(
        commands.as_slice(),
        [StrokeCommand::CommitStroke { .. }]
    ));
}

#[test]
fn interruption_cancels_without_commit_and_reused_id_can_start_a_fresh_stroke() {
    let mut processor = Processor::default();
    let mut router = Router::new(Viewport::default());
    let down = sample(8, 1, Tool::Pen, Phase::Down, [20.0, 30.0]);
    let begin = feed(&mut processor, &mut router, down);
    let first_id = match begin[0] {
        StrokeCommand::BeginStroke { stroke_id, .. } => stroke_id,
        _ => panic!("expected initial stroke"),
    };

    let outputs = processor.interrupt(2, Reason::CaptureLost);
    assert_eq!(outputs.len(), 1);
    let cancelled = outputs
        .iter()
        .filter_map(|output| router.route(output))
        .collect::<Vec<_>>();
    assert!(
        matches!(cancelled.as_slice(), [StrokeCommand::CancelStroke { stroke_id, reason: paint_stroke::CancelReason::Input(Reason::CaptureLost), .. }] if *stroke_id == first_id)
    );
    assert!(
        feed(
            &mut processor,
            &mut router,
            sample(8, 3, Tool::Pen, Phase::Move, [40.0, 50.0])
        )
        .is_empty()
    );

    let restarted = feed(
        &mut processor,
        &mut router,
        sample(8, 4, Tool::Pen, Phase::Down, [21.0, 31.0]),
    );
    let second_id = match restarted.as_slice() {
        [StrokeCommand::BeginStroke { stroke_id, .. }] => *stroke_id,
        other => panic!("expected fresh BeginStroke, got {other:?}"),
    };
    assert_ne!(first_id, second_id);
    assert!(
        matches!(feed(&mut processor, &mut router, sample(8, 5, Tool::Pen, Phase::Up, [22.0, 32.0])).as_slice(), [StrokeCommand::CommitStroke { stroke_id, .. }] if *stroke_id == second_id)
    );
}

#[test]
fn two_touch_navigation_is_target_captured_and_never_paints() {
    let mut processor = Processor::default();
    let mut router = Router::new(Viewport::default());
    let mut owner = TouchTargetRouter::default();
    let mut touches = TouchNavigation::default();
    let mut navigation = Navigation::default();

    for (id, point) in [(10, [100.0, 100.0]), (11, [200.0, 100.0])] {
        assert_eq!(owner.start(id, TouchTarget::Canvas), TouchTarget::Canvas);
        assert!(
            feed(
                &mut processor,
                &mut router,
                sample(id, id, Tool::Touch, Phase::Down, point)
            )
            .is_empty()
        );
        assert!(touches.event(id, TouchPhase::Start, point).is_none());
    }
    assert_eq!(
        owner.start(12, TouchTarget::Canvas),
        TouchTarget::Suppressed
    );

    let moved = [220.0, 100.0];
    assert!(
        feed(
            &mut processor,
            &mut router,
            sample(11, 12, Tool::Touch, Phase::Move, moved)
        )
        .is_empty()
    );
    let pinch = touches.event(11, TouchPhase::Move, moved).unwrap();
    assert!(navigation.pinch(pinch.from, pinch.from, pinch.to, pinch.ratio));
    assert_eq!(navigation.zoom, 1.2);
    assert_eq!(owner.target(11), Some(TouchTarget::Canvas));
    assert_eq!(router.active_count(), 0);

    for (id, point, time) in [(10, [100.0, 100.0], 13), (11, moved, 14)] {
        assert!(
            feed(
                &mut processor,
                &mut router,
                sample(id, time, Tool::Touch, Phase::Up, point)
            )
            .is_empty()
        );
        touches.event(id, TouchPhase::End, point);
        owner.finish(id);
    }
    owner.finish(12);
    assert_eq!(owner.active_target(), None);
}

#[test]
fn physical_client_mapping_roundtrips_at_two_scales_and_both_orientations() {
    let cases = [
        ([1200, 800], "100% landscape"),
        ([1800, 1200], "150% landscape"),
        ([800, 1200], "portrait after orientation change"),
    ];
    let target_document = [240.0, 160.0];
    for (size, _label) in cases {
        let mapping = CanvasMapping::for_document(
            size,
            [800.0, 500.0],
            [0.0, 0.0, f64::from(size[0]), f64::from(size[1])],
        )
        .unwrap();
        let client_px = mapping.document_to_client(target_document).unwrap();
        let mut processor = Processor::default();
        let mut router = Router::new(mapping.routing_viewport());
        let commands = feed(
            &mut processor,
            &mut router,
            sample(20, 1, Tool::Pen, Phase::Down, client_px),
        );
        match commands.as_slice() {
            [StrokeCommand::BeginStroke { sample, .. }] => {
                for (axis, expected) in target_document.iter().enumerate() {
                    assert!(
                        (sample.position_document.0[axis] - expected).abs() < 1e-9,
                        "{_label} axis {axis}"
                    );
                }
            }
            other => panic!("expected mapped stroke begin, got {other:?}"),
        }
    }
}
