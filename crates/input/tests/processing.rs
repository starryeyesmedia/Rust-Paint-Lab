use paint_input::{
    Buttons, Phase, PointerSample, Tool,
    lifecycle::{Change, MAX_CONTACTS, MillisecondClock32, Processor, Reason},
    replay::{Record, Replay},
};

fn sample(id: u64, time: u64, phase: Phase, tool: Tool) -> PointerSample {
    PointerSample {
        id,
        timestamp_ms: time,
        phase,
        tool,
        position_px: [1.0, 2.0],
        pressure: Some(0.5),
        tilt_degrees: [None, Some(0.0)],
        buttons: Buttons::default(),
        in_contact: matches!(phase, Phase::Down | Phase::Move),
        in_range: true,
    }
}
fn fixture(text: &str) -> Replay {
    text.parse().unwrap()
}
fn changes(p: &mut Processor, s: PointerSample) -> Vec<Change> {
    p.process(s).into_iter().map(|o| o.change).collect()
}

#[test]
fn pen_fixture_preserves_all_samples_and_cleans_lift() {
    let r = fixture(include_str!("fixtures/pen.replay"));
    let samples: Vec<_> = r
        .records()
        .iter()
        .map(|r| match r {
            Record::Sample(s) => *s,
            _ => panic!(),
        })
        .collect();
    let out = r.run();
    assert_eq!(
        out.iter().map(|o| o.change.clone()).collect::<Vec<_>>(),
        vec![
            Change::Hovered(samples[0]),
            Change::Started(samples[1]),
            Change::Moved(samples[2]),
            Change::Moved(samples[3]),
            Change::Ended(samples[4]),
        ]
    );
    assert_eq!(samples[0].pressure, None);
    assert_eq!(samples[1].pressure, Some(0.0));
    assert_eq!(samples[2].pressure, Some(0.25));
    assert!(samples[2].buttons.barrel);
    assert_eq!(samples[3].tilt_degrees, [Some(0.0), None]);
    assert_eq!(
        out.iter().map(|o| o.time_ms).collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    let mut p = Processor::default();
    for s in samples {
        p.process(s);
    }
    assert!(p.active().is_empty());
}

#[test]
fn mixed_contacts_capture_loss_interrupt_and_eof_are_repeatable() {
    let r = fixture(include_str!("fixtures/mixed.replay"));
    let out = r.run();
    for _ in 0..10 {
        assert_eq!(out, r.run());
    }
    assert_eq!(out.len(), 10);
    assert!(matches!(out[5].change, Change::Ended(s) if s.id == 10));
    assert!(
        matches!(out[6].change, Change::Cancelled { last, reason: Reason::CaptureLost } if last.id == 30 && last.pressure == Some(0.75))
    );
    assert!(
        matches!(out[7].change, Change::Cancelled { last, reason: Reason::Interrupted } if last.id == 20)
    );
    assert!(matches!(out[8].change, Change::Started(s) if s.id == 20 && s.tool == Tool::Pen));
    assert!(matches!(
        out[9].change,
        Change::Cancelled {
            reason: Reason::EndOfReplay,
            ..
        }
    ));
}

#[test]
fn unexpected_sequence_has_explicit_deterministic_outcomes() {
    let out = fixture(include_str!("fixtures/interrupted.replay")).run();
    assert_eq!(out.len(), 10);
    assert!(matches!(
        out[1].change,
        Change::Cancelled {
            reason: Reason::Cancelled,
            ..
        }
    ));
    assert!(matches!(
        out[2].change,
        Change::Ignored {
            reason: Reason::MissingDown,
            ..
        }
    ));
    assert!(matches!(
        out[4].change,
        Change::Cancelled {
            reason: Reason::Replaced,
            ..
        }
    ));
    assert!(matches!(out[5].change, Change::Started(_)));
    assert!(matches!(
        out[6].change,
        Change::Ignored {
            reason: Reason::OutOfOrder,
            ..
        }
    ));
    assert!(matches!(
        out[7].change,
        Change::Cancelled {
            reason: Reason::OutOfOrder,
            ..
        }
    ));
    assert!(matches!(
        out[8].change,
        Change::Ignored {
            reason: Reason::MissingDown,
            ..
        }
    ));
    assert!(matches!(out[9].change, Change::Ignored { id: 99, .. }));
    assert!(
        out.windows(2)
            .all(|pair| pair[0].time_ms <= pair[1].time_ms)
    );
}

#[test]
fn simultaneous_contacts_are_independent_and_cleanup_is_sorted() {
    let mut p = Processor::default();
    for (id, tool) in [(30, Tool::Pen), (20, Tool::Touch), (10, Tool::Touch)] {
        p.process(sample(id, 1, Phase::Down, tool));
    }
    assert_eq!(p.active().len(), 3);
    p.process(sample(20, 2, Phase::Up, Tool::Touch));
    assert_eq!(p.active().keys().copied().collect::<Vec<_>>(), vec![10, 30]);
    let out = p.interrupt(3, Reason::Interrupted);
    assert!(matches!(out[0].change, Change::Cancelled { last, .. } if last.id == 10));
    assert!(matches!(out[1].change, Change::Cancelled { last, .. } if last.id == 30));
    assert!(p.active().is_empty());
    assert!(p.interrupt(4, Reason::Interrupted).is_empty());
}

#[test]
fn reused_id_requires_new_down_and_retains_no_old_data() {
    let mut p = Processor::default();
    p.process(sample(1, 1, Phase::Down, Tool::Pen));
    p.process(sample(1, 2, Phase::Up, Tool::Pen));
    let mut next = sample(1, 3, Phase::Down, Tool::Touch);
    next.pressure = None;
    assert_eq!(changes(&mut p, next), vec![Change::Started(next)]);
    assert_eq!(p.active()[&1], next);
    p.cancel(1, 4, Reason::CaptureLost);
    assert!(p.active().is_empty());
}

#[test]
fn hover_never_retains_contacts_and_cleans_missing_up() {
    let mut p = Processor::default();
    let hover = sample(1, 0, Phase::Hover, Tool::Pen);
    assert_eq!(changes(&mut p, hover), vec![Change::Hovered(hover)]);
    assert!(p.active().is_empty());
    p.process(sample(1, 1, Phase::Down, Tool::Pen));
    let out = p.process(sample(1, 2, Phase::Hover, Tool::Pen));
    assert!(matches!(
        out[0].change,
        Change::Cancelled {
            reason: Reason::Replaced,
            ..
        }
    ));
    assert!(matches!(out[1].change, Change::Hovered(_)));
    assert!(p.active().is_empty());
}

#[test]
fn tool_change_without_down_cancels_but_does_not_invent_stroke() {
    let mut p = Processor::default();
    p.process(sample(1, 1, Phase::Down, Tool::Pen));
    let out = p.process(sample(1, 2, Phase::Move, Tool::Touch));
    assert_eq!(out.len(), 2);
    assert!(matches!(
        out[0].change,
        Change::Cancelled {
            reason: Reason::Replaced,
            ..
        }
    ));
    assert!(matches!(
        out[1].change,
        Change::Ignored {
            reason: Reason::MissingDown,
            ..
        }
    ));
    assert!(p.active().is_empty());
}

#[test]
fn invalid_data_and_contact_flags_cannot_leave_orphan_contacts() {
    let mut invalids = Vec::new();
    let base = sample(1, 2, Phase::Move, Tool::Pen);
    let mut s = base;
    s.position_px[0] = f64::NAN;
    invalids.push(s);
    let mut s = base;
    s.pressure = Some(1.1);
    invalids.push(s);
    let mut s = base;
    s.tilt_degrees[0] = Some(f32::INFINITY);
    invalids.push(s);
    let mut s = base;
    s.in_contact = false;
    invalids.push(s);
    let mut s = base;
    s.in_range = false;
    invalids.push(s);
    for s in invalids {
        let mut p = Processor::default();
        p.process(sample(1, 1, Phase::Down, Tool::Pen));
        assert!(matches!(
            p.process(s)[0].change,
            Change::Cancelled {
                reason: Reason::InvalidData,
                ..
            }
        ));
        assert!(p.active().is_empty());
    }
}

#[test]
fn stale_start_cannot_replace_newer_contact_but_stale_cancel_cleans_up() {
    let mut p = Processor::default();
    let original = sample(1, 10, Phase::Down, Tool::Pen);
    p.process(original);
    assert!(matches!(
        p.process(sample(1, 9, Phase::Down, Tool::Touch))[0].change,
        Change::Ignored {
            reason: Reason::OutOfOrder,
            ..
        }
    ));
    assert_eq!(p.active()[&1], original);
    let out = p.cancel(1, 0, Reason::CaptureLost);
    assert_eq!(out.time_ms, 10);
    assert!(p.active().is_empty());
}

#[test]
fn windows_timestamp_rollover_duplicates_and_late_packets() {
    let mut clock = MillisecondClock32::default();
    let inputs = [u32::MAX - 1, u32::MAX, 0, 1, u32::MAX, 1, 2];
    let expected = [
        4294967294, 4294967295, 4294967296, 4294967297, 4294967297, 4294967297, 4294967298,
    ];
    for (raw, expected) in inputs.into_iter().zip(expected) {
        assert_eq!(clock.extend(raw), expected);
    }
    let mut p = Processor::default();
    let mut clock = MillisecondClock32::default();
    for (time, phase) in [(u32::MAX, Phase::Down), (0, Phase::Move), (1, Phase::Up)] {
        p.process(sample(1, clock.extend(time), phase, Tool::Pen));
    }
    assert!(p.active().is_empty());
    assert_eq!(p.time_ms(), 4294967297);
}

#[test]
fn clock_small_regression_and_half_range_are_clamped_without_anchor_change() {
    let mut clock = MillisecondClock32::default();
    assert_eq!(clock.extend(100), 100);
    assert_eq!(clock.extend(99), 100);
    assert_eq!(clock.extend(100 + (1 << 31)), 100);
    assert_eq!(clock.extend(101), 101);
}

#[test]
fn bounded_capacity_and_long_stream_cleanup() {
    let mut p = Processor::default();
    for id in 0..MAX_CONTACTS as u64 {
        p.process(sample(id, 0, Phase::Down, Tool::Touch));
    }
    assert!(matches!(
        p.process(sample(1000, 0, Phase::Down, Tool::Pen))[0].change,
        Change::Ignored {
            reason: Reason::Capacity,
            ..
        }
    ));
    assert_eq!(p.active().len(), MAX_CONTACTS);
    // Replacing an existing ID works even at capacity.
    assert_eq!(p.process(sample(0, 1, Phase::Down, Tool::Pen)).len(), 2);
    assert_eq!(p.interrupt(2, Reason::Interrupted).len(), MAX_CONTACTS);
    for time in 3..50_003 {
        let phase = if time % 2 == 1 {
            Phase::Down
        } else {
            Phase::Up
        };
        p.process(sample((time - 1) / 2, time, phase, Tool::Pen));
        assert!(p.active().len() <= 1);
    }
    assert!(p.active().is_empty());
}

#[test]
fn replay_rejects_unknown_versions_fields_nonfinite_and_malformed_values() {
    for text in [
        "",
        "paint-input-replay 2",
        "paint-input-replay 1\ninterrupt nope",
        "paint-input-replay 1\ninterrupt 1 extra",
        "paint-input-replay 1\nwat 1",
        "paint-input-replay 1\nsample 1 1 pen down NaN 0 0 - - 1 1 1",
        "paint-input-replay 1\nsample 1 1 pen down 0 0 2 - - 1 1 1",
        "paint-input-replay 1\nsample 1 1 pen down 0 0 0 - - 32 1 1",
        "paint-input-replay 1\nsample 1 1 touch hover 0 0 - - - 0 0 1",
        "paint-input-replay 1\nsample 1 1 pen down 0 0 - - - 0 2 1",
    ] {
        assert!(text.parse::<Replay>().is_err(), "accepted {text}");
    }
    let error = "# fixture\npaint-input-replay 1\ninterrupt bad"
        .parse::<Replay>()
        .unwrap_err();
    assert_eq!(error.line, 3);
    assert!(error.to_string().contains("invalid number"));
}

#[test]
fn replay_has_explicit_resource_limits_and_empty_replay_is_valid() {
    assert!(fixture("paint-input-replay 1\n").run().is_empty());
    assert!(
        " ".repeat(paint_input::replay::MAX_FIXTURE_BYTES + 1)
            .parse::<Replay>()
            .is_err()
    );
    let text = format!(
        "paint-input-replay 1\n{}",
        "interrupt 0\n".repeat(paint_input::replay::MAX_RECORDS + 1)
    );
    assert!(text.parse::<Replay>().is_err());
}
