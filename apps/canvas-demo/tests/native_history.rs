use canvas_demo::native_history::{HistoryDeduplicator, NativeStamp};
fn stamp(counter: u64) -> NativeStamp {
    NativeStamp {
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
