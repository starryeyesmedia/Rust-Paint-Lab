use canvas_demo::{
    interaction::{LiveMouse, MouseEvent},
    native_history::{
        HistoryDeduplicator, MAX_NATIVE_HISTORY_SAMPLES, MAX_QUEUED_PEN_EVENTS,
        NativeHistoryFailure, NativeHistoryFailures, NativePenQueue,
        validate_coordinate_conversion, validate_history_count_query, validate_history_read,
    },
};
use paint_input::{Buttons, Phase, PointerSample, Tool};

fn stamp(counter: u64) -> canvas_demo::native_history::NativeStamp {
    canvas_demo::native_history::NativeStamp {
        counter,
        timestamp_ms: 1,
        position_himetric: [100, 200],
        pointer_flags: 1,
        pen_flags: 0,
        pressure: 512,
        tilt: [0, 0],
        mask: 7,
    }
}

fn sample(id: u64, time: u64, phase: Phase) -> PointerSample {
    PointerSample {
        source_counter: None,
        id,
        timestamp_ms: time,
        tool: Tool::Pen,
        phase,
        position_px: [116.5 + time as f64, 116.25],
        pressure: Some(0.2 + time as f32 / 10.0),
        tilt_degrees: [Some(-25.0), Some(30.0)],
        buttons: Buttons {
            barrel: true,
            ..Buttons::default()
        },
        in_contact: matches!(phase, Phase::Down | Phase::Move),
        in_range: true,
    }
}

#[test]
fn overlapped_history_drops_exact_duplicates_and_stale_samples() {
    let mut d = HistoryDeduplicator::default();
    assert!(d.accept(1, stamp(5)));
    let newest_first = [stamp(7), stamp(6), stamp(5), stamp(4)];
    let accepted: Vec<_> = newest_first
        .into_iter()
        .rev()
        .filter(|s| d.accept(1, *s))
        .map(|s| s.counter)
        .collect();
    assert_eq!(accepted, [6, 7]);
}

#[test]
fn same_time_pressure_tilt_button_and_capability_changes_are_preserved() {
    let mut d = HistoryDeduplicator::default();
    let mut s = stamp(10);
    assert!(d.accept(1, s));
    s.pressure += 1;
    assert!(d.accept(1, s));
    s.tilt[1] = 30;
    assert!(d.accept(1, s));
    s.pen_flags = 8;
    assert!(d.accept(1, s));
    s.mask = 0;
    assert!(d.accept(1, s));
    assert!(!d.accept(1, s));
}

#[test]
fn pointer_id_reuse_and_zero_counter_do_not_confuse_other_pointers() {
    let mut d = HistoryDeduplicator::default();
    assert!(d.accept(1, stamp(100)));
    assert!(d.accept(2, stamp(1)));
    assert!(d.accept(1, stamp(101)));
    let mut s = stamp(0);
    s.timestamp_ms = u32::MAX;
    assert!(d.accept(3, s));
    s.timestamp_ms = 0;
    assert!(d.accept(3, s));
    assert!(!d.accept(3, s));
    assert_eq!(d.len(), 3);
}

#[test]
fn long_native_stream_retention_is_bounded() {
    let mut d = HistoryDeduplicator::default();
    assert!(d.is_empty());
    for id in 0..100000 {
        assert!(d.accept(id, stamp(u64::from(id) + 1)));
        assert!(d.len() <= 128);
    }
    assert_eq!(d.len(), 128);
}

#[test]
fn native_history_api_failures_and_invalid_counts_have_distinct_categories() {
    assert_eq!(
        validate_history_count_query(false, 0),
        Err(NativeHistoryFailure::HistoryCountQuery)
    );
    for count in [0, MAX_NATIVE_HISTORY_SAMPLES + 1, u32::MAX] {
        assert_eq!(
            validate_history_count_query(true, count),
            Err(NativeHistoryFailure::InvalidHistoryCount)
        );
    }
    assert_eq!(
        validate_history_count_query(true, MAX_NATIVE_HISTORY_SAMPLES),
        Ok(MAX_NATIVE_HISTORY_SAMPLES as usize)
    );

    assert_eq!(
        validate_history_read(false, 8, 1),
        Err(NativeHistoryFailure::HistoryRead)
    );
    assert_eq!(
        validate_history_read(true, 8, 0),
        Err(NativeHistoryFailure::InvalidHistoryCount)
    );
    assert_eq!(
        validate_history_read(true, 8, 9),
        Err(NativeHistoryFailure::InvalidHistoryCount)
    );
    assert_eq!(validate_history_read(true, 8, 3), Ok(3));

    assert_eq!(
        validate_coordinate_conversion(false),
        Err(NativeHistoryFailure::CoordinateConversion)
    );
    assert_eq!(validate_coordinate_conversion(true), Ok(()));
}

#[test]
fn failure_categories_are_fixed_and_counted_without_raw_pointer_data() {
    let mut failures = NativeHistoryFailures::default();
    assert!(failures.is_empty());
    let expected_labels = [
        "history_count_query",
        "invalid_history_count",
        "history_read",
        "coordinate_conversion",
        "pen_event_queue_overflow",
    ];
    for (category, expected_label) in NativeHistoryFailure::ALL.into_iter().zip(expected_labels) {
        failures.record(category);
        assert_eq!(category.label(), expected_label);
    }
    let mut additional = NativeHistoryFailures::default();
    additional.record(NativeHistoryFailure::HistoryRead);
    failures.merge(additional);

    assert_eq!(NativeHistoryFailure::ALL.len(), 5);
    assert_eq!(failures.count(NativeHistoryFailure::HistoryCountQuery), 1);
    assert_eq!(failures.count(NativeHistoryFailure::InvalidHistoryCount), 1);
    assert_eq!(failures.count(NativeHistoryFailure::HistoryRead), 2);
    assert_eq!(failures.count(NativeHistoryFailure::CoordinateConversion), 1);
    assert_eq!(failures.count(NativeHistoryFailure::PenEventQueueOverflow), 1);
}

#[test]
fn queue_overflow_fails_closed_and_accepts_a_fresh_batch_after_cancellation() {
    let mut queue = NativePenQueue::default();
    queue
        .push_batch(vec![sample(1, 1, Phase::Down), sample(1, 2, Phase::Move)])
        .unwrap();
    assert_eq!(queue.len(), 2);

    let overflow = queue
        .push_batch(vec![sample(1, 5, Phase::Move); MAX_QUEUED_PEN_EVENTS])
        .unwrap_err();
    assert_eq!(overflow, NativeHistoryFailure::PenEventQueueOverflow);
    assert!(queue.is_empty());
    assert!(queue.take_lost());
    assert!(!queue.take_lost());

    let fresh = vec![sample(1, 3, Phase::Down), sample(1, 4, Phase::Up)];
    queue.push_batch(fresh.clone()).unwrap();
    assert_eq!(queue.drain(), fresh);

    let mut capture_cancel_queue = NativePenQueue::default();
    capture_cancel_queue
        .push_batch(vec![sample(2, 1, Phase::Move); MAX_QUEUED_PEN_EVENTS])
        .unwrap();
    assert_eq!(
        capture_cancel_queue.push(sample(2, 2, Phase::Cancel)),
        Err(NativeHistoryFailure::PenEventQueueOverflow)
    );
    assert!(capture_cancel_queue.is_empty());
    assert!(capture_cancel_queue.take_lost());
}

#[test]
fn each_history_failure_cancels_a_provisional_stroke_and_fresh_input_commits() {
    for failure in NativeHistoryFailure::ALL {
        let mut live = LiveMouse::new([832, 532]);
        live.pen(sample(7, 1, Phase::Down));
        live.pen(sample(7, 2, Phase::Move));
        assert_eq!(live.scene().provisional().len(), 1, "{failure:?}");

        let mut queue = NativePenQueue::<PointerSample>::default();
        queue.push_batch(vec![sample(7, 3, Phase::Move)]).unwrap();
        assert_eq!(queue.fail_closed(failure), failure);
        assert!(queue.is_empty(), "{failure:?}");
        assert!(queue.take_lost(), "{failure:?}");

        // This is the consumer action used by main.rs when take_lost() is set.
        live.handle(MouseEvent::CaptureLost, 3);
        assert!(live.scene().provisional().is_empty(), "{failure:?}");
        assert!(live.scene().committed().is_empty(), "{failure:?}");

        live.pen(sample(7, 4, Phase::Down));
        live.pen(sample(7, 5, Phase::Move));
        live.pen(sample(7, 6, Phase::Up));
        assert!(!live.active(), "{failure:?}");
        assert!(live.scene().provisional().is_empty(), "{failure:?}");
        assert_eq!(live.scene().committed().len(), 1, "{failure:?}");
    }
}

#[test]
fn diagnostics_leave_successful_event_order_pressure_and_tilt_unchanged() {
    let expected = vec![
        sample(4, 1, Phase::Down),
        sample(4, 2, Phase::Move),
        sample(4, 3, Phase::Up),
    ];
    let mut queue = NativePenQueue::default();
    queue.push_batch(expected.clone()).unwrap();

    let mut failures = NativeHistoryFailures::default();
    failures.record(NativeHistoryFailure::HistoryRead);
    assert_eq!(failures.count(NativeHistoryFailure::HistoryRead), 1);

    let actual = queue.drain();
    assert_eq!(actual, expected);
    assert_eq!(actual[0].pressure, Some(0.3));
    assert_eq!(actual[1].pressure, Some(0.4));
    assert_eq!(actual[2].pressure, Some(0.5));
    assert!(actual.iter().all(|event| event.tilt_degrees == [Some(-25.0), Some(30.0)]));
}
