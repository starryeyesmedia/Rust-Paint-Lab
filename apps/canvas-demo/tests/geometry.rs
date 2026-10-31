use canvas_demo::{
    playback::{Playback, Scenario, commands},
    scene::{
        COMMITTED, CanvasMapping, MAX_POINTS_PER_STROKE, MAX_SAMPLES, MAX_STROKES, MAX_VERTICES,
        PREVIEW, Scene, SceneError, WIDTH,
    },
};
use paint_stroke::{StrokeCommand as C, StrokeId};

#[test]
fn completed_replay_preserves_final_sample_and_pressure_without_width_changes() {
    let mut p = Playback::new(Scenario::Completed).unwrap();
    p.advance_to(800).unwrap();
    assert_eq!(p.scene.provisional().len(), 1);
    assert!(p.scene.committed().is_empty());
    let preview = p.scene.mesh([832, 532]).unwrap();
    assert!(preview.iter().skip(6).all(|v| v.color == PREVIEW));
    p.advance_to(1800).unwrap();
    assert!(p.finished());
    assert!(p.scene.provisional().is_empty());
    let points = &p.scene.committed()[&StrokeId(0)];
    assert_eq!(points.len(), 4);
    assert_eq!(points.last().unwrap().position_document.0, [700.0, 140.0]);
    assert_eq!(
        points.iter().map(|s| s.pressure).collect::<Vec<_>>(),
        vec![Some(0.0), Some(0.3), Some(0.8), Some(0.0)]
    );
    let mesh = p.scene.mesh([832, 532]).unwrap();
    assert!(mesh.iter().skip(6).all(|v| v.color == COMMITTED));
    // Final cap is present, not just the last Update segment.
    let end = CanvasMapping::new([832, 532])
        .unwrap()
        .to_clip([700.0 + WIDTH / 2.0, 140.0 + WIDTH / 2.0])
        .unwrap();
    assert!(
        mesh.iter()
            .any(|v| v.position == end && v.color == COMMITTED)
    );
}

#[test]
fn cancellation_removes_every_preview_vertex_and_never_commits() {
    let mut p = Playback::new(Scenario::Cancelled).unwrap();
    p.advance_to(1500).unwrap();
    assert_eq!(p.scene.sample_count(), 3);
    assert!(p.scene.mesh([832, 532]).unwrap().len() > 6);
    p.advance_to(2600).unwrap();
    assert!(p.finished());
    assert_eq!(p.scene.sample_count(), 0);
    assert_eq!(p.scene.mesh([832, 532]).unwrap().len(), 6); // Paper only.
    assert!(p.scene.committed().is_empty());
}

#[test]
fn concurrent_stroke_ids_stay_separate_and_touch_creates_no_geometry() {
    let mut p = Playback::new(Scenario::Concurrent).unwrap();
    p.advance_to(1600).unwrap();
    assert_eq!(
        p.scene.provisional().keys().copied().collect::<Vec<_>>(),
        vec![StrokeId(0), StrokeId(1)]
    );
    assert!(
        p.scene
            .provisional()
            .values()
            .flatten()
            .all(|s| s.pointer_id != 9)
    );
    p.advance_to(1800).unwrap();
    assert_eq!(p.scene.committed().len(), 1);
    assert_eq!(p.scene.provisional().len(), 1);
    p.advance_to(u64::MAX).unwrap();
    assert_eq!(p.scene.committed().len(), 2);
    assert_eq!(p.scene.sample_count(), 7);
    assert!(p.scene.provisional().is_empty());
}

#[test]
fn explicit_clock_is_monotonic_and_restart_releases_state() {
    let mut p = Playback::new(Scenario::Completed).unwrap();
    p.advance_to(299).unwrap();
    assert_eq!(p.scene.sample_count(), 0);
    p.advance_to(300).unwrap();
    assert_eq!(p.scene.sample_count(), 1);
    p.advance_to(0).unwrap();
    assert_eq!(p.scene.sample_count(), 1);
    p.advance_to(u64::MAX).unwrap();
    p.restart();
    assert!(!p.finished());
    assert_eq!(p.scene.sample_count(), 0);
    p.advance_to(300).unwrap();
    assert_eq!(p.scene.sample_count(), 1);
}

#[test]
fn identical_replay_times_produce_identical_scenes_and_meshes() {
    for scenario in [
        Scenario::Completed,
        Scenario::Cancelled,
        Scenario::Concurrent,
    ] {
        assert_eq!(
            commands(scenario.fixture()).unwrap(),
            commands(scenario.fixture()).unwrap()
        );
        let mut a = Playback::new(scenario).unwrap();
        let mut b = Playback::new(scenario).unwrap();
        for t in [0, 300, 800, 1500, 1900, 2700] {
            a.advance_to(t).unwrap();
            b.advance_to(t).unwrap();
            assert_eq!(a.scene, b.scene);
            assert_eq!(
                a.scene.mesh([1000, 700]).unwrap(),
                b.scene.mesh([1000, 700]).unwrap()
            );
        }
    }
}

#[test]
fn coordinate_mapping_preserves_aspect_margin_y_direction_and_resize() {
    let m = CanvasMapping::new([832, 532]).unwrap();
    assert_eq!(m.to_clip([400.0, 250.0]).unwrap(), [0.0, 0.0]);
    let a = m.to_clip([0.0, 0.0]).unwrap();
    let b = m.to_clip([800.0, 500.0]).unwrap();
    assert!(a[0] < 0.0 && a[1] > 0.0 && b[0] > 0.0 && b[1] < 0.0);
    assert!((a[0] + 800.0 / 832.0).abs() < 0.00001);
    assert!((a[1] - 500.0 / 532.0).abs() < 0.00001);
    assert_eq!(
        CanvasMapping::new([1600, 1000])
            .unwrap()
            .to_clip([400.0, 250.0])
            .unwrap(),
        [0.0, 0.0]
    );
}

#[test]
fn invalid_nonfinite_and_out_of_range_coordinates_never_reach_gpu() {
    for size in [[0, 500], [800, 0], [8193, 10]] {
        assert!(matches!(
            CanvasMapping::new(size),
            Err(SceneError::InvalidViewport)
        ));
    }
    let mapping = CanvasMapping::new([832, 532]).unwrap();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
        assert_eq!(
            mapping.to_clip([bad, 0.0]),
            Err(SceneError::InvalidCoordinates)
        );
    }
    let mut cs = commands(Scenario::Completed.fixture()).unwrap();
    if let C::UpdateStroke { sample, .. } = &mut cs[1] {
        sample.position_document.0[0] = f64::NAN;
    }
    let mut s = Scene::default();
    s.apply(&cs[0]).unwrap();
    assert_eq!(s.apply(&cs[1]), Err(SceneError::InvalidCoordinates));
    assert_eq!(s.sample_count(), 0);
    s.apply(cs.last().unwrap()).unwrap(); // Orphan Commit cannot reintroduce it.
    assert!(s.committed().is_empty());
}

#[test]
fn duplicate_terminal_or_begin_and_orphan_updates_are_safe() {
    let cs = commands(Scenario::Completed.fixture()).unwrap();
    let mut s = Scene::default();
    s.apply(&cs[1]).unwrap();
    assert_eq!(s.sample_count(), 0);
    s.apply(&cs[0]).unwrap();
    s.apply(&cs[0]).unwrap();
    assert_eq!(s.sample_count(), 1);
    for c in &cs[1..] {
        s.apply(c).unwrap();
    }
    let n = s.sample_count();
    s.apply(cs.last().unwrap()).unwrap();
    s.apply(&cs[0]).unwrap();
    assert_eq!(s.sample_count(), n);
}

#[test]
fn zero_length_segments_are_finite_and_fixed_width_independent_of_pressure() {
    let mut cs = commands(Scenario::Completed.fixture()).unwrap();
    let start = match cs[0] {
        C::BeginStroke { sample, .. } => sample,
        _ => panic!(),
    };
    if let C::UpdateStroke { sample, .. } = &mut cs[1] {
        sample.position_document = start.position_document;
        sample.pressure = None;
    }
    let mut s = Scene::default();
    s.apply(&cs[0]).unwrap();
    s.apply(&cs[1]).unwrap();
    let mesh = s.mesh([832, 532]).unwrap();
    assert_eq!(mesh.len(), 18); // Paper + two square caps, no degenerate segment.
    assert!(mesh.iter().flat_map(|v| v.position).all(f32::is_finite));
    assert_eq!(&mesh[6..12], &mesh[12..18]);
}

#[test]
fn stroke_and_point_limits_abort_preview_without_unbounded_growth() {
    let cs = commands(Scenario::Completed.fixture()).unwrap();
    let mut s = Scene::default();
    let start = match cs[0] {
        C::BeginStroke { sample, .. } => sample,
        _ => panic!(),
    };
    for id in 0..MAX_STROKES as u64 {
        s.apply(&C::BeginStroke {
            stroke_id: StrokeId(id),
            sample: start,
        })
        .unwrap();
    }
    assert_eq!(
        s.apply(&C::BeginStroke {
            stroke_id: StrokeId(999),
            sample: start
        }),
        Err(SceneError::Capacity)
    );
    assert_eq!(s.stroke_count(), MAX_STROKES);
    s.clear();
    s.apply(&cs[0]).unwrap();
    for i in 1..MAX_POINTS_PER_STROKE {
        let mut sample = start;
        sample.time_ms = i as u64;
        s.apply(&C::UpdateStroke {
            stroke_id: StrokeId(0),
            sample,
        })
        .unwrap();
    }
    let mut sample = start;
    sample.time_ms = 9999;
    assert_eq!(
        s.apply(&C::UpdateStroke {
            stroke_id: StrokeId(0),
            sample
        }),
        Err(SceneError::Capacity)
    );
    assert_eq!(s.sample_count(), 0);
}

#[test]
fn global_sample_and_vertex_budgets_hold_and_reset_does_not_accumulate() {
    let cs = commands(Scenario::Completed.fixture()).unwrap();
    let mut s = Scene::default();
    let start = match cs[0] {
        C::BeginStroke { sample, .. } => sample,
        _ => panic!(),
    };
    for id in 0..8 {
        s.apply(&C::BeginStroke {
            stroke_id: StrokeId(id),
            sample: start,
        })
        .unwrap();
        for i in 1..MAX_POINTS_PER_STROKE {
            let mut sample = start;
            sample.time_ms = i as u64;
            s.apply(&C::UpdateStroke {
                stroke_id: StrokeId(id),
                sample,
            })
            .unwrap();
        }
    }
    assert_eq!(s.sample_count(), MAX_SAMPLES);
    assert!(s.mesh([832, 532]).unwrap().len() <= MAX_VERTICES);
    assert_eq!(
        s.apply(&C::BeginStroke {
            stroke_id: StrokeId(100),
            sample: start
        }),
        Err(SceneError::Capacity)
    );
    for _ in 0..100 {
        s.clear();
        for c in &cs {
            s.apply(c).unwrap();
        }
        assert_eq!(s.sample_count(), 4);
    }
}

#[test]
fn cancellation_isolated_from_other_committed_transaction() {
    let mut p = Playback::new(Scenario::Concurrent).unwrap();
    p.advance_to(1800).unwrap();
    let last = *p.scene.provisional()[&StrokeId(1)].last().unwrap();
    p.scene
        .apply(&C::CancelStroke {
            stroke_id: StrokeId(1),
            last,
            time_ms: 1900,
            reason: paint_stroke::CancelReason::Input(paint_input::lifecycle::Reason::CaptureLost),
        })
        .unwrap();
    assert!(p.scene.provisional().is_empty());
    assert_eq!(p.scene.committed().len(), 1);
    assert_eq!(
        p.scene.committed()[&StrokeId(0)]
            .last()
            .unwrap()
            .position_document
            .0,
        [700.0, 100.0]
    );
}
