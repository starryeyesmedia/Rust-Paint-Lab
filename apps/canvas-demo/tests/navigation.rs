use canvas_demo::{
    navigation::{Navigation, TouchNavigation, TouchPhase as P},
    scene::CanvasMapping,
};
#[test]
fn pinch_preserves_anchor_and_limits_zoom_pan() {
    let mut n = Navigation::default();
    assert!(n.pinch([100.0; 2], [150.0, 100.0], [160.0, 100.0], 2.0));
    assert_eq!(n.zoom, 2.0);
    assert_eq!(n.pan, [-40.0, 0.0]);
    assert!(!n.pinch([0.0; 2], [0.0; 2], [0.0; 2], f64::NAN));
    n.pinch([0.0; 2], [0.0; 2], [100000.0; 2], 100.0);
    assert_eq!(n.zoom, 8.0);
    assert_eq!(n.pan, [32768.0; 2]);
}
#[test]
fn touches_require_two_contacts_and_clean_up_duplicates_orphans_and_id_reuse() {
    let mut t = TouchNavigation::default();
    assert!(t.event(1, P::Move, [10.0; 2]).is_none());
    assert!(t.event(1, P::Start, [10.0; 2]).is_none());
    assert!(t.event(2, P::Start, [30.0, 10.0]).is_none());
    let p = t.event(2, P::Move, [50.0, 10.0]).unwrap();
    assert_eq!(p.ratio, 2.0);
    assert_eq!(p.from, [20.0, 10.0]);
    assert_eq!(p.to, [30.0, 10.0]);
    assert!(t.event(2, P::Start, [50.0, 10.0]).is_none());
    t.event(2, P::Cancel, [50.0, 10.0]);
    assert_eq!(t.active_count(), 1);
    assert!(t.event(2, P::Start, [50.0, 10.0]).is_none());
    t.clear();
    assert_eq!(t.active_count(), 0);
}
#[test]
fn touch_state_is_bounded_and_invalid_positions_cancel() {
    let mut t = TouchNavigation::default();
    for id in 0..10000 {
        t.event(id, P::Start, [10.0; 2]);
        assert!(t.active_count() <= 16);
    }
    t.event(1, P::Move, [f64::INFINITY, 0.0]);
    assert_eq!(t.active_count(), 0);
}
#[test]
fn navigation_mapping_roundtrips_and_clips_to_workspace_after_zoom_and_pan() {
    let m = CanvasMapping::for_document([1000, 700], [800.0, 500.0], [0.0, 100.0, 700.0, 650.0])
        .unwrap()
        .navigated(4.0, [120.0, -50.0])
        .unwrap();
    let p = [400.0, 250.0];
    let clip = m.to_clip(p).unwrap();
    let client = [
        (f64::from(clip[0]) + 1.0) * 500.0,
        (1.0 - f64::from(clip[1])) * 350.0,
    ];
    let result = m.routing_viewport().to_document(client).unwrap().0;
    for i in 0..2 {
        assert!((p[i] - result[i]).abs() < 0.001);
    }
    assert!(!m.contains_client([800.0, 300.0]));
    let [x, y, w, h] = m.scissor();
    assert!(x + w <= 700 && y >= 100 && y + h <= 650);
    assert!(m.navigated(f64::NAN, [0.0; 2]).is_err());
    assert!(m.navigated(1.0, [f64::INFINITY, 0.0]).is_err());
}

#[test]
fn pointer_segments_clip_to_document_after_zoom_and_pan() {
    let mapping =
        CanvasMapping::for_document([1000, 700], [800.0, 500.0], [20.0, 40.0, 980.0, 680.0])
            .unwrap()
            .navigated(2.5, [35.0, -20.0])
            .unwrap();
    let vp = mapping.routing_viewport();
    let client = |doc: [f64; 2]| mapping.document_to_client(doc).unwrap();
    let clipped = mapping
        .clip_segment_client(client([-100.0, 120.0]), client([900.0, 120.0]))
        .unwrap()
        .unwrap();
    assert!((clipped.start_t - 0.1).abs() < 1e-9);
    assert!((clipped.end_t - 0.9).abs() < 1e-9);
    let start = vp.to_document(clipped.start_client).unwrap().0;
    let end = vp.to_document(clipped.end_client).unwrap().0;
    assert!((start[0]).abs() < 1e-9 && (start[1] - 120.0).abs() < 1e-9);
    assert!((end[0] - 800.0).abs() < 1e-9 && (end[1] - 120.0).abs() < 1e-9);

    assert!(
        mapping
            .clip_segment_client(client([-40.0, -20.0]), client([-20.0, -10.0]))
            .unwrap()
            .is_none()
    );
    assert!(
        mapping
            .clip_segment_client([f64::NAN, 0.0], [10.0, 10.0])
            .is_err()
    );
}
