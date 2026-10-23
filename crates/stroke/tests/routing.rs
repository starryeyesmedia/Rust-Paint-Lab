use paint_input::{
    Buttons, Phase, PointerSample, Tool,
    lifecycle::{Change, MAX_CONTACTS, Output, Processor, Reason},
    replay::Replay,
};
use paint_stroke::{
    CancelReason, Router, StrokeCommand as C, StrokeId, StrokeSample,
    coordinates::{DocumentPoint, TransformError, Viewport},
};

fn raw(id: u64, time: u64, phase: Phase, tool: Tool) -> PointerSample {
    PointerSample {
        id,
        timestamp_ms: time,
        tool,
        phase,
        position_px: [10.0, 20.0],
        pressure: Some(0.5),
        tilt_degrees: [None, Some(0.0)],
        buttons: Buttons {
            primary: matches!(phase, Phase::Down | Phase::Move),
            ..Buttons::default()
        },
        in_contact: matches!(phase, Phase::Down | Phase::Move),
        in_range: true,
    }
}
fn output(s: PointerSample) -> Output {
    Output {
        time_ms: s.timestamp_ms,
        change: match s.phase {
            Phase::Down => Change::Started(s),
            Phase::Move => Change::Moved(s),
            Phase::Up => Change::Ended(s),
            Phase::Hover => Change::Hovered(s),
            Phase::Cancel => Change::Cancelled {
                last: s,
                reason: Reason::Cancelled,
            },
        },
    }
}
// Expected identity-coordinate samples, deliberately not using the router's mapper.
fn point(s: PointerSample) -> StrokeSample {
    StrokeSample {
        pointer_id: s.id,
        tool: s.tool,
        position_client_px: s.position_px,
        position_document: DocumentPoint(s.position_px),
        pressure: s.pressure,
        tilt_degrees: s.tilt_degrees,
        buttons: s.buttons,
        source_time_ms: s.timestamp_ms,
        time_ms: s.timestamp_ms,
    }
}
fn replay(text: &str) -> Vec<C> {
    let mut router = Router::default();
    let result = text
        .parse::<Replay>()
        .unwrap()
        .run()
        .iter()
        .filter_map(|o| router.route(o))
        .collect();
    assert_eq!(
        router.active_count(),
        0,
        "Replay EOF must close every painting transaction"
    );
    result
}
fn samples(r: &Replay) -> Vec<PointerSample> {
    r.records()
        .iter()
        .filter_map(|r| match r {
            paint_input::replay::Record::Sample(s) => Some(*s),
            _ => None,
        })
        .collect()
}

#[test]
fn existing_pen_replay_has_exact_commands_pressure_tilt_buttons_and_no_hover_stroke() {
    let text = include_str!("../../input/tests/fixtures/pen.replay");
    let s = samples(&text.parse().unwrap());
    let commands = replay(text);
    assert_eq!(
        commands,
        vec![
            C::BeginStroke {
                stroke_id: StrokeId(0),
                sample: point(s[1])
            },
            C::UpdateStroke {
                stroke_id: StrokeId(0),
                sample: point(s[2])
            },
            C::UpdateStroke {
                stroke_id: StrokeId(0),
                sample: point(s[3])
            },
            C::CommitStroke {
                stroke_id: StrokeId(0),
                sample: point(s[4])
            },
        ]
    );
    assert_eq!(s[1].pressure, Some(0.0));
    assert_eq!(s[2].pressure, Some(0.25));
    assert!(s[2].buttons.barrel);
    assert_eq!(s[3].pressure, Some(1.0));
    assert_eq!(s[3].tilt_degrees, [Some(0.0), None]);
}

#[test]
fn unsupported_pressure_and_zero_are_distinct_in_commands() {
    let text = "paint-input-replay 1\nsample 0 1 pen down 0 0 - - - 1 1 1\nsample 1 1 pen move 1 1 0 - - 1 1 1\nsample 2 1 pen up 2 2 0 - - 0 0 1\n";
    let s = samples(&text.parse().unwrap());
    assert_eq!(
        replay(text),
        vec![
            C::BeginStroke {
                stroke_id: StrokeId(0),
                sample: point(s[0])
            },
            C::UpdateStroke {
                stroke_id: StrokeId(0),
                sample: point(s[1])
            },
            C::CommitStroke {
                stroke_id: StrokeId(0),
                sample: point(s[2])
            }
        ]
    );
    assert_eq!(s[0].pressure, None);
    assert_eq!(s[1].pressure, Some(0.0));
}

#[test]
fn existing_mixed_replay_reserves_touch_and_cancels_pen_capture_and_unfinished_stroke() {
    let text = include_str!("../../input/tests/fixtures/mixed.replay");
    let s = samples(&text.parse().unwrap());
    assert_eq!(
        replay(text),
        vec![
            C::BeginStroke {
                stroke_id: StrokeId(0),
                sample: point(s[2])
            },
            C::UpdateStroke {
                stroke_id: StrokeId(0),
                sample: point(s[4])
            },
            C::CancelStroke {
                stroke_id: StrokeId(0),
                last: point(s[4]),
                time_ms: 13,
                reason: CancelReason::Input(Reason::CaptureLost)
            },
            C::BeginStroke {
                stroke_id: StrokeId(1),
                sample: point(s[6])
            },
            C::CancelStroke {
                stroke_id: StrokeId(1),
                last: point(s[6]),
                time_ms: 15,
                reason: CancelReason::Input(Reason::EndOfReplay)
            },
        ]
    );
}

#[test]
fn existing_interrupted_replay_never_commits_aborted_transactions() {
    let text = include_str!("../../input/tests/fixtures/interrupted.replay");
    let s = samples(&text.parse().unwrap());
    assert_eq!(
        replay(text),
        vec![
            C::BeginStroke {
                stroke_id: StrokeId(0),
                sample: point(s[0])
            },
            C::CancelStroke {
                stroke_id: StrokeId(0),
                last: point(s[0]),
                time_ms: 2,
                reason: CancelReason::Input(Reason::Cancelled)
            },
            C::BeginStroke {
                stroke_id: StrokeId(1),
                sample: point(s[3])
            },
            C::CancelStroke {
                stroke_id: StrokeId(1),
                last: point(s[3]),
                time_ms: 4,
                reason: CancelReason::Input(Reason::Replaced)
            },
            C::BeginStroke {
                stroke_id: StrokeId(2),
                sample: point(s[4])
            },
            C::CancelStroke {
                stroke_id: StrokeId(2),
                last: point(s[4]),
                time_ms: 4,
                reason: CancelReason::Input(Reason::OutOfOrder)
            },
        ]
    );
}

#[test]
fn mouse_primary_paints_without_pressure_or_tilt_and_button_loss_aborts() {
    let text = include_str!("fixtures/mouse.replay");
    let s = samples(&text.parse().unwrap());
    let mouse_point = |s| {
        let mut p = point(s);
        p.pressure = None;
        p.tilt_degrees = [None; 2];
        p
    };
    assert_eq!(
        replay(text),
        vec![
            C::BeginStroke {
                stroke_id: StrokeId(0),
                sample: mouse_point(s[2])
            },
            C::UpdateStroke {
                stroke_id: StrokeId(0),
                sample: mouse_point(s[3])
            },
            C::CommitStroke {
                stroke_id: StrokeId(0),
                sample: mouse_point(s[4])
            },
            C::BeginStroke {
                stroke_id: StrokeId(1),
                sample: mouse_point(s[5])
            },
            C::CancelStroke {
                stroke_id: StrokeId(1),
                last: mouse_point(s[5]),
                time_ms: 6,
                reason: CancelReason::MousePrimaryReleased
            },
        ]
    );
}

#[test]
fn simultaneous_pointers_complete_independently_and_reuse_gets_new_transaction_id() {
    let text = "paint-input-replay 1\nsample 0 1 pen down 1 1 - - - 1 1 1\nsample 0 2 pen down 2 2 0.5 - - 1 1 1\nsample 1 1 pen up 1 1 0 - - 0 0 1\nsample 2 1 pen down 3 3 1 - - 1 1 1\nsample 3 2 pen up 2 2 0 - - 0 0 1\nsample 4 1 pen up 3 3 0 - - 0 0 1\n";
    let s = samples(&text.parse().unwrap());
    assert_eq!(
        replay(text),
        vec![
            C::BeginStroke {
                stroke_id: StrokeId(0),
                sample: point(s[0])
            },
            C::BeginStroke {
                stroke_id: StrokeId(1),
                sample: point(s[1])
            },
            C::CommitStroke {
                stroke_id: StrokeId(0),
                sample: point(s[2])
            },
            C::BeginStroke {
                stroke_id: StrokeId(2),
                sample: point(s[3])
            },
            C::CommitStroke {
                stroke_id: StrokeId(1),
                sample: point(s[4])
            },
            C::CommitStroke {
                stroke_id: StrokeId(2),
                sample: point(s[5])
            },
        ]
    );
}

#[test]
fn duplicate_and_orphan_lifecycle_outputs_do_not_duplicate_commands() {
    let mut r = Router::default();
    let start = output(raw(1, 1, Phase::Down, Tool::Pen));
    let movement = output(raw(1, 2, Phase::Move, Tool::Pen));
    let end = output(raw(1, 3, Phase::Up, Tool::Pen));
    assert!(r.route(&movement).is_none());
    assert!(r.route(&start).is_none());
    // Start before the stream watermark is stale. Begin a fresh valid stream.
    let mut r = Router::default();
    assert!(matches!(r.route(&start), Some(C::BeginStroke { .. })));
    assert_eq!(r.route(&start), None);
    assert!(matches!(r.route(&movement), Some(C::UpdateStroke { .. })));
    assert_eq!(r.route(&movement), None);
    assert!(matches!(r.route(&end), Some(C::CommitStroke { .. })));
    assert_eq!(r.route(&end), None);
    assert_eq!(r.active_count(), 0);
}

#[test]
fn old_cancellation_and_end_cannot_abort_a_reused_pointer_transaction() {
    let mut r = Router::default();
    let first = raw(1, 1, Phase::Down, Tool::Pen);
    r.route(&output(first));
    let cancel = Output {
        time_ms: 2,
        change: Change::Cancelled {
            last: first,
            reason: Reason::CaptureLost,
        },
    };
    assert!(matches!(r.route(&cancel), Some(C::CancelStroke { .. })));
    assert!(r.route(&cancel).is_none());
    r.route(&output(raw(1, 3, Phase::Down, Tool::Pen)));
    assert!(r.route(&cancel).is_none());
    assert!(r.route(&output(raw(1, 2, Phase::Up, Tool::Pen))).is_none());
    assert_eq!(r.active_count(), 1);
    assert!(matches!(
        r.route(&output(raw(1, 4, Phase::Up, Tool::Pen))),
        Some(C::CommitStroke {
            stroke_id: StrokeId(1),
            ..
        })
    ));
}

#[test]
fn app_interruption_cancels_all_active_strokes_in_stable_pointer_order() {
    let mut p = Processor::default();
    let mut r = Router::default();
    for id in [20, 10] {
        for o in p.process(raw(id, 0, Phase::Down, Tool::Pen)) {
            r.route(&o);
        }
    }
    let commands: Vec<_> = p
        .interrupt(5, Reason::Interrupted)
        .iter()
        .filter_map(|o| r.route(o))
        .collect();
    assert_eq!(
        commands,
        vec![
            C::CancelStroke {
                stroke_id: StrokeId(1),
                last: point(raw(10, 0, Phase::Down, Tool::Pen)),
                time_ms: 5,
                reason: CancelReason::Input(Reason::Interrupted)
            },
            C::CancelStroke {
                stroke_id: StrokeId(0),
                last: point(raw(20, 0, Phase::Down, Tool::Pen)),
                time_ms: 5,
                reason: CancelReason::Input(Reason::Interrupted)
            },
        ]
    );
    assert_eq!(r.active_count(), 0);
}

#[test]
fn router_teardown_is_cancel_only_idempotent_and_monotonic() {
    let mut r = Router::default();
    r.route(&output(raw(1, 10, Phase::Down, Tool::Pen)));
    assert_eq!(
        r.interrupt(0),
        vec![C::CancelStroke {
            stroke_id: StrokeId(0),
            last: point(raw(1, 10, Phase::Down, Tool::Pen)),
            time_ms: 10,
            reason: CancelReason::RouterInterrupted
        }]
    );
    assert!(r.interrupt(20).is_empty());
    assert_eq!(r.active_count(), 0);
}

#[test]
fn touch_unknown_hover_and_ignored_are_not_painting() {
    let mut r = Router::default();
    for tool in [Tool::Touch, Tool::Unknown] {
        assert!(r.route(&output(raw(1, 0, Phase::Down, tool))).is_none());
        assert!(r.route(&output(raw(1, 1, Phase::Move, tool))).is_none());
        assert!(r.route(&output(raw(1, 2, Phase::Up, tool))).is_none());
    }
    assert!(
        r.route(&output(raw(2, 3, Phase::Hover, Tool::Pen)))
            .is_none()
    );
    assert!(
        r.route(&Output {
            time_ms: 4,
            change: Change::Ignored {
                id: 2,
                reason: Reason::MissingDown
            }
        })
        .is_none()
    );
    assert_eq!(r.active_count(), 0);
}

#[test]
fn viewport_identity_translation_and_zoom_have_exact_document_units() {
    assert_eq!(
        Viewport::default().to_document([-10.0, 20.0]),
        Ok(DocumentPoint([-10.0, 20.0]))
    );
    assert_eq!(
        Viewport::new([10.0, -20.0], 1.0)
            .unwrap()
            .to_document([14.0, -12.0]),
        Ok(DocumentPoint([4.0, 8.0]))
    );
    assert_eq!(
        Viewport::new([10.0, -20.0], 2.0)
            .unwrap()
            .to_document([14.0, -12.0]),
        Ok(DocumentPoint([2.0, 4.0]))
    );
    assert_eq!(
        Viewport::new([10.0, -20.0], 0.5)
            .unwrap()
            .to_document([14.0, -12.0]),
        Ok(DocumentPoint([8.0, 16.0]))
    );
}

#[test]
fn invalid_viewports_positions_and_arithmetic_overflow_are_rejected() {
    for zoom in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            Viewport::new([0.0; 2], zoom),
            Err(TransformError::InvalidZoom)
        );
    }
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            Viewport::new([bad, 0.0], 1.0),
            Err(TransformError::InvalidOrigin)
        );
        assert_eq!(
            Viewport::default().to_document([0.0, bad]),
            Err(TransformError::InvalidPosition)
        );
    }
    assert_eq!(
        Viewport::new([-f64::MAX, 0.0], 1.0)
            .unwrap()
            .to_document([f64::MAX, 0.0]),
        Err(TransformError::NonFiniteResult)
    );
    assert_eq!(
        Viewport::new([0.0; 2], f64::MIN_POSITIVE)
            .unwrap()
            .to_document([f64::MAX, 0.0]),
        Err(TransformError::NonFiniteResult)
    );
}

#[test]
fn router_captures_viewport_per_stroke_and_preserves_source_attributes() {
    let mut r = Router::new(Viewport::new([2.0, 4.0], 2.0).unwrap());
    let first = raw(1, 0, Phase::Down, Tool::Pen);
    let mut expected = point(first);
    expected.position_document = DocumentPoint([4.0, 8.0]);
    assert_eq!(
        r.route(&output(first)),
        Some(C::BeginStroke {
            stroke_id: StrokeId(0),
            sample: expected
        })
    );
    r.set_viewport(Viewport::new([0.0; 2], 0.5).unwrap());
    let movement = raw(1, 1, Phase::Move, Tool::Pen);
    let mut expected = point(movement);
    expected.position_document = DocumentPoint([4.0, 8.0]);
    assert_eq!(
        r.route(&output(movement)),
        Some(C::UpdateStroke {
            stroke_id: StrokeId(0),
            sample: expected
        })
    );
    let second = raw(2, 1, Phase::Down, Tool::Pen);
    let mut expected = point(second);
    expected.position_document = DocumentPoint([20.0, 40.0]);
    assert_eq!(
        r.route(&output(second)),
        Some(C::BeginStroke {
            stroke_id: StrokeId(1),
            sample: expected
        })
    );
}

#[test]
fn failed_mapping_does_not_begin_or_commit_corrupt_strokes() {
    let viewport = Viewport::new([0.0; 2], f64::MIN_POSITIVE).unwrap();
    let mut r = Router::new(viewport);
    assert!(
        r.route(&output(raw(1, 0, Phase::Down, Tool::Pen)))
            .is_none()
    );
    let mut small = raw(1, 1, Phase::Down, Tool::Pen);
    small.position_px = [0.0; 2];
    let mut expected = point(small);
    expected.position_document = DocumentPoint([0.0; 2]);
    assert_eq!(
        r.route(&output(small)),
        Some(C::BeginStroke {
            stroke_id: StrokeId(0),
            sample: expected
        })
    );
    assert_eq!(
        r.route(&output(raw(1, 2, Phase::Up, Tool::Pen))),
        Some(C::CancelStroke {
            stroke_id: StrokeId(0),
            last: expected,
            time_ms: 2,
            reason: CancelReason::InvalidCoordinates
        })
    );
    assert_eq!(r.active_count(), 0);
}

#[test]
fn malformed_or_tool_mismatched_outputs_never_commit() {
    for kind in 0..4 {
        let mut r = Router::default();
        let start = raw(1, 0, Phase::Down, Tool::Pen);
        r.route(&output(start));
        let mut s = raw(1, 1, Phase::Up, Tool::Pen);
        let mut event = output(s);
        match kind {
            0 => {
                s.position_px[0] = f64::NAN;
                event.change = Change::Ended(s);
            }
            1 => {
                s.phase = Phase::Move;
                event.change = Change::Ended(s);
            }
            2 => {
                s.timestamp_ms = 10;
                event.change = Change::Ended(s);
            }
            _ => {
                s.tool = Tool::Mouse;
                event.change = Change::Ended(s);
            }
        }
        assert!(matches!(r.route(&event), Some(C::CancelStroke { .. })));
        assert_eq!(r.active_count(), 0);
    }
}

#[test]
fn command_time_and_source_time_remain_distinct() {
    let mut r = Router::default();
    let s = raw(1, 1, Phase::Down, Tool::Pen);
    let mut o = output(s);
    o.time_ms = 5;
    let mut expected = point(s);
    expected.time_ms = 5;
    assert_eq!(
        r.route(&o),
        Some(C::BeginStroke {
            stroke_id: StrokeId(0),
            sample: expected
        })
    );
}

#[test]
fn bounded_state_and_long_sequences_release_every_transaction() {
    let mut r = Router::default();
    for id in 0..MAX_CONTACTS as u64 {
        r.route(&output(raw(id, 0, Phase::Down, Tool::Pen)));
    }
    assert_eq!(r.active_count(), MAX_CONTACTS);
    assert!(
        r.route(&output(raw(1000, 0, Phase::Down, Tool::Pen)))
            .is_none()
    );
    assert_eq!(r.interrupt(0).len(), MAX_CONTACTS);
    for id in 0..10_000 {
        r.route(&output(raw(id, id * 2 + 1, Phase::Down, Tool::Pen)));
        assert_eq!(r.active_count(), 1);
        r.route(&output(raw(id, id * 2 + 2, Phase::Up, Tool::Pen)));
        assert_eq!(r.active_count(), 0);
    }
}

#[test]
fn replay_command_output_is_identical_across_runs() {
    for text in [
        include_str!("../../input/tests/fixtures/pen.replay"),
        include_str!("../../input/tests/fixtures/mixed.replay"),
        include_str!("../../input/tests/fixtures/interrupted.replay"),
        include_str!("fixtures/mouse.replay"),
    ] {
        let expected = replay(text);
        for _ in 0..10 {
            assert_eq!(replay(text), expected);
        }
    }
}
