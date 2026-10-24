use paint_input::{
    SourceCounter,
    lifecycle::Change,
    replay::{Record, Replay},
};
#[test]
fn counter_zero_unavailable_overflow_and_different_clock_rates_remain_distinct() {
    let zero = SourceCounter::new(0, 10_000_000).unwrap();
    let later = SourceCounter::new(123, 10_000_000).unwrap();
    assert_eq!(later.elapsed_since(zero).unwrap().as_nanos(), 12300);
    assert!(SourceCounter::new(0, 0).is_none());
    assert!(zero.elapsed_since(later).is_none());
    assert!(
        later
            .elapsed_since(SourceCounter::new(0, 1000).unwrap())
            .is_none()
    );
    assert_eq!(
        SourceCounter::new(u64::MAX, 1)
            .unwrap()
            .elapsed_since(SourceCounter::new(0, 1).unwrap())
            .unwrap()
            .as_secs(),
        u64::MAX
    );
}
#[test]
fn version_two_replay_preserves_precise_metadata_through_lifecycle() {
    let replay: Replay = include_str!("fixtures/precise.replay").parse().unwrap();
    let outputs = replay.run();
    assert_eq!(outputs, replay.run());
    assert!(matches!(outputs[0].change, Change::Started(_)));
    assert!(matches!(outputs[2].change, Change::Ended(_)));
    let Change::Moved(sample) = outputs[1].change else {
        panic!()
    };
    assert_eq!(sample.source_counter.unwrap().ticks, 1000123);
    let zero: Replay = "paint-input-replay 2\nsample 0 1 pen down 1 1 0 - - 1 1 1 0 1000"
        .parse()
        .unwrap();
    let Record::Sample(sample) = zero.records()[0] else {
        panic!()
    };
    assert_eq!(sample.source_counter.unwrap().ticks, 0);
}
#[test]
fn legacy_fixtures_remain_unsupported_for_precise_data_and_invalid_pairs_are_rejected() {
    let legacy: Replay = "paint-input-replay 1\nsample 0 1 pen down 1 1 1 - - 1 1 1"
        .parse()
        .unwrap();
    let Record::Sample(s) = legacy.records()[0] else {
        panic!()
    };
    assert!(s.source_counter.is_none());
    for pair in ["1 0", "- 1000", "1 -", "-", "1 1000 extra"] {
        assert!(
            format!("paint-input-replay 2\nsample 0 1 pen down 1 1 1 - - 1 1 1 {pair}")
                .parse::<Replay>()
                .is_err()
        );
    }
}
