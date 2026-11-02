use canvas_demo::{
    interaction::{LiveMouse, MouseEvent},
    scene::CanvasMapping,
};
use paint_document::{StrokeMode, brush::BrushKind};
use paint_input::{Buttons, Phase, PointerSample, Tool};
use paint_stroke::{StrokeCommand, StrokeId, StrokeSample, coordinates::DocumentPoint};

fn sample(
    id: u64,
    timestamp_ms: u64,
    phase: Phase,
    position_px: [f64; 2],
    pressure: Option<f32>,
    tilt_degrees: [Option<f32>; 2],
    tool: Tool,
) -> PointerSample {
    PointerSample {
        source_counter: None,
        id,
        timestamp_ms,
        tool,
        phase,
        position_px,
        pressure,
        tilt_degrees,
        buttons: Buttons::default(),
        in_contact: matches!(phase, Phase::Down | Phase::Move),
        in_range: true,
    }
}

fn pen(id: u64, timestamp_ms: u64, phase: Phase, position_px: [f64; 2]) -> PointerSample {
    sample(
        id,
        timestamp_ms,
        phase,
        position_px,
        Some(0.5),
        [Some(0.0), Some(0.0)],
        Tool::Pen,
    )
}

#[test]
fn pen_attributes_missing_capabilities_and_immediate_lift_restart_are_preserved() {
    let mut live = LiveMouse::new([832, 532]);
    live.pen(sample(
        7,
        1,
        Phase::Down,
        [116.5, 116.25],
        Some(0.125),
        [Some(-31.0), Some(17.0)],
        Tool::Pen,
    ));
    live.pen(sample(
        7,
        2,
        Phase::Move,
        [216.75, 216.5],
        Some(0.875),
        [Some(22.0), None],
        Tool::Pen,
    ));
    live.pen(sample(
        7,
        3,
        Phase::Up,
        [216.75, 216.5],
        Some(0.625),
        [None, Some(-12.0)],
        Tool::Pen,
    ));

    assert_eq!(live.scene().committed().len(), 1);
    let first = live.scene().committed().values().next().unwrap();
    assert_eq!(first.len(), 3);
    assert_eq!(first[0].pressure, Some(0.125));
    assert_eq!(first[1].pressure, Some(0.875));
    assert_eq!(first[2].pressure, Some(0.625));
    assert_eq!(first[0].tilt_degrees, [Some(-31.0), Some(17.0)]);
    assert_eq!(first[1].tilt_degrees, [Some(22.0), None]);
    assert_eq!(first[2].tilt_degrees, [None, Some(-12.0)]);

    // A new contact from the same pointer immediately after lift is independent.
    live.pen(sample(
        7,
        4,
        Phase::Down,
        [316.0, 116.25],
        None,
        [None, None],
        Tool::Pen,
    ));
    live.pen(sample(
        7,
        5,
        Phase::Move,
        [416.0, 116.25],
        None,
        [None, None],
        Tool::Pen,
    ));
    live.pen(sample(
        7,
        6,
        Phase::Up,
        [416.0, 116.25],
        None,
        [None, None],
        Tool::Pen,
    ));

    assert!(!live.active());
    assert!(live.scene().provisional().is_empty());
    assert_eq!(live.scene().committed().len(), 2);
    let second = live
        .scene()
        .committed()
        .values()
        .find(|points| points[0].position_client_px == [316.0, 116.25])
        .unwrap();
    assert!(second.iter().all(|point| point.pressure.is_none()));
    assert!(
        second
            .iter()
            .all(|point| point.tilt_degrees == [None, None])
    );
}

#[test]
fn touch_is_reserved_while_pen_owns_the_painting_contact() {
    let mut live = LiveMouse::new([832, 532]);
    live.pen(pen(1, 1, Phase::Down, [116.0, 116.0]));
    assert_eq!(live.scene().provisional().len(), 1);

    live.pen(sample(
        2,
        2,
        Phase::Down,
        [116.0, 116.0],
        None,
        [None, None],
        Tool::Touch,
    ));
    live.pen(sample(
        2,
        3,
        Phase::Move,
        [200.0, 180.0],
        None,
        [None, None],
        Tool::Touch,
    ));
    live.pen(sample(
        2,
        4,
        Phase::Up,
        [200.0, 180.0],
        None,
        [None, None],
        Tool::Touch,
    ));

    assert_eq!(live.scene().provisional().len(), 1);
    live.pen(pen(1, 5, Phase::Up, [216.0, 116.0]));
    assert!(live.scene().provisional().is_empty());
    assert_eq!(live.scene().committed().len(), 1);
    assert_eq!(live.scene().committed().values().next().unwrap().len(), 2);
}

#[test]
fn focus_and_capture_interruption_cancel_pen_previews_and_require_a_new_down() {
    for focus_loss in [false, true] {
        let mut live = LiveMouse::new([832, 532]);
        live.pen(pen(9, 1, Phase::Down, [116.0, 116.0]));
        live.pen(pen(9, 2, Phase::Move, [216.0, 116.0]));
        assert_eq!(live.scene().provisional().len(), 1);

        if focus_loss {
            live.handle(MouseEvent::Focus(false), 3);
            live.handle(MouseEvent::Focus(true), 4);
        } else {
            live.handle(MouseEvent::CaptureLost, 3);
        }
        assert!(!live.active());
        assert!(live.scene().provisional().is_empty());
        assert!(live.scene().committed().is_empty());

        live.pen(pen(9, 5, Phase::Move, [316.0, 116.0]));
        assert!(
            live.scene().provisional().is_empty(),
            "orphan Move restarted a stroke"
        );
        live.pen(pen(9, 6, Phase::Down, [316.0, 116.0]));
        live.pen(pen(9, 7, Phase::Up, [416.0, 116.0]));
        assert!(!live.active());
        assert!(live.scene().provisional().is_empty());
        assert_eq!(live.scene().committed().len(), 1);
    }
}

#[test]
fn physical_pixel_mapping_round_trips_at_100_and_150_percent_scale() {
    for (size, expected_center) in [([832, 532], [416.0, 266.0]), ([1248, 798], [624.0, 399.0])] {
        let mapping = CanvasMapping::new(size).unwrap();
        let center = mapping.document_to_client([400.0, 250.0]).unwrap();
        assert_eq!(center, expected_center);

        let mut live = LiveMouse::new(size);
        live.pen(pen(1, 1, Phase::Down, center));
        live.pen(pen(1, 2, Phase::Up, center));
        let points = live.scene().committed().values().next().unwrap();
        assert_eq!(points[0].position_client_px, center);
        assert_eq!(points[0].position_document.0, [400.0, 250.0]);
        assert_eq!(points[1].position_document.0, [400.0, 250.0]);
    }
}

fn seed_red_patch(live: &mut LiveMouse) {
    let engine = live.project_mut();
    engine.brush.kind = BrushKind::Round;
    engine.brush.color = [255, 0, 0];
    engine.brush.size = 8.0;
    engine.brush.pressure_size = false;
    engine.brush.pressure_opacity = false;
    engine.brush.hardness = 1.0;
    let make_sample = |x| StrokeSample {
        source_counter: None,
        pointer_id: 2,
        tool: Tool::Pen,
        position_client_px: [x, 250.0],
        position_document: DocumentPoint([x, 250.0]),
        pressure: Some(1.0),
        tilt_degrees: [None; 2],
        buttons: Buttons::default(),
        source_time_ms: 0,
        time_ms: 0,
    };
    engine
        .apply(&StrokeCommand::BeginStroke {
            stroke_id: StrokeId(22),
            sample: make_sample(12.0),
        })
        .unwrap();
    engine
        .apply(&StrokeCommand::CommitStroke {
            stroke_id: StrokeId(22),
            sample: make_sample(26.0),
        })
        .unwrap();
}

fn document_after_pen_stroke(mode: StrokeMode, flow: f64) -> (Vec<u8>, Vec<u8>) {
    let mut live = LiveMouse::new([832, 532]);
    live.enable_document();
    seed_red_patch(&mut live);
    let before = live.project().rgba();
    live.project_mut().brush.size = 8.0;
    live.project_mut().brush.opacity = 1.0;
    live.project_mut().brush.flow = flow;
    live.project_mut().brush.color = [0, 0, 255];
    live.project_mut().brush.pressure_size = false;
    live.project_mut().brush.pressure_opacity = false;
    live.set_stroke_mode(mode);

    live.pen(pen(1, 1, Phase::Down, [32.0, 266.0]));
    live.pen(pen(1, 2, Phase::Move, [76.0, 266.0]));
    live.pen(pen(1, 3, Phase::Up, [76.0, 266.0]));
    assert!(!live.active());
    (before, live.project().rgba())
}

#[test]
fn pen_samples_reach_the_selected_smudge_mode_in_document_routing() {
    let (before, smudged) = document_after_pen_stroke(StrokeMode::Smudge, 0.0);
    let (paint_before, painted) = document_after_pen_stroke(StrokeMode::Paint, 0.0);
    assert_eq!(paint_before, before);
    assert_eq!(
        painted, before,
        "Paint mode with zero flow should add no pigment"
    );
    assert_ne!(
        smudged, before,
        "selected Smudge mode should move existing pigment"
    );
}

#[test]
fn pen_samples_reach_the_selected_color_mixer_mode_in_document_routing() {
    let (before, mixed) = document_after_pen_stroke(StrokeMode::ColorMix { strength: 1.0 }, 1.0);
    let (paint_before, painted) = document_after_pen_stroke(StrokeMode::Paint, 1.0);
    assert_eq!(paint_before, before);
    assert_ne!(mixed, before);
    assert_ne!(
        mixed, painted,
        "Color Mixer must not fall through to ordinary Paint mode"
    );
}
