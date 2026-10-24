use canvas_demo::{
    interaction::{CanvasSession, LiveMouse, Mode, MouseEvent as E},
    playback::Scenario,
    scene::{CanvasMapping, MAX_POINTS_PER_STROKE, MAX_STROKES, SceneError},
};
use paint_input::Tool;
use paint_stroke::StrokeId;

fn button(pressed: bool) -> E {
    E::Button {
        primary: true,
        pressed,
    }
}
fn begin(mouse: &mut LiveMouse, time: u64) {
    mouse.handle(E::Moved([116.0, 116.0]), time);
    mouse.handle(button(true), time);
}
fn finish(mouse: &mut LiveMouse, time: u64) {
    mouse.handle(E::Moved([316.0, 216.0]), time);
    mouse.handle(button(false), time);
}
fn stroke(mouse: &mut LiveMouse, time: u64) {
    begin(mouse, time);
    finish(mouse, time + 1);
}

#[test]
fn complete_mouse_sequence_uses_shared_pipeline_and_keeps_final_sample() {
    let mut m = LiveMouse::new([832, 532]);
    begin(&mut m, 0);
    assert!(m.active());
    assert_eq!(m.scene().provisional().len(), 1);
    finish(&mut m, 1);
    assert!(!m.active());
    assert!(m.scene().provisional().is_empty());
    let s = &m.scene().committed()[&StrokeId(0)];
    assert_eq!(s.len(), 3);
    assert_eq!(s[0].position_client_px, [116.0, 116.0]);
    assert_eq!(s[0].position_document.0, [100.0, 100.0]);
    assert_eq!(s.last().unwrap().position_document.0, [300.0, 200.0]);
    assert!(!s.last().unwrap().buttons.primary);
    assert!(
        s.iter()
            .all(|p| p.tool == Tool::Mouse && p.pressure.is_none() && p.tilt_degrees == [None; 2])
    );
}

#[test]
fn hover_secondary_and_orphan_release_do_not_paint() {
    let mut m = LiveMouse::new([832, 532]);
    m.handle(E::Moved([116.0, 116.0]), 0);
    m.handle(
        E::Button {
            primary: false,
            pressed: true,
        },
        1,
    );
    m.handle(E::Moved([216.0, 116.0]), 2);
    m.handle(
        E::Button {
            primary: false,
            pressed: false,
        },
        3,
    );
    m.handle(button(false), 4);
    assert_eq!(m.scene().sample_count(), 0);
    assert!(!m.active());
}

#[test]
fn focus_loss_cancels_only_active_stroke_and_orphan_moves_cannot_resume() {
    let mut m = LiveMouse::new([832, 532]);
    stroke(&mut m, 0);
    begin(&mut m, 2);
    m.handle(E::Focus(false), 3);
    assert!(!m.active());
    assert!(m.scene().provisional().is_empty());
    assert_eq!(m.scene().committed().len(), 1);
    m.handle(E::Moved([200.0, 200.0]), 4);
    m.handle(button(true), 4);
    assert!(!m.active());
    m.handle(E::Focus(true), 5);
    m.handle(E::Moved([200.0, 200.0]), 5);
    m.handle(button(false), 5);
    assert_eq!(m.scene().committed().len(), 1);
    stroke(&mut m, 6);
    assert_eq!(m.scene().committed().len(), 2);
}

#[test]
fn capture_loss_and_window_exit_cancel_then_require_a_new_down() {
    for event in [E::CaptureLost, E::Left, E::Interrupted] {
        let mut m = LiveMouse::new([832, 532]);
        begin(&mut m, 0);
        m.handle(event, 1);
        assert!(!m.active());
        assert_eq!(m.scene().sample_count(), 0);
        m.handle(E::Entered, 2);
        m.handle(E::Moved([116.0, 116.0]), 2);
        m.handle(button(false), 3);
        assert_eq!(m.scene().sample_count(), 0);
        stroke(&mut m, 4);
        assert_eq!(m.scene().committed().len(), 1);
    }
}

#[test]
fn outside_paper_and_invalid_positions_do_not_start_or_leave_orphan_contacts() {
    for point in [
        [0.0, 0.0],
        [831.0, 531.0],
        [-1.0, 30.0],
        [f64::NAN, 100.0],
        [f64::INFINITY, 0.0],
    ] {
        let mut m = LiveMouse::new([832, 532]);
        m.handle(E::Moved(point), 0);
        m.handle(button(true), 0);
        assert!(!m.active());
        assert_eq!(m.scene().sample_count(), 0);
        begin(&mut m, 1);
        m.handle(E::Moved(point), 2);
        m.handle(button(false), 3);
        assert!(!m.active());
        assert_eq!(m.scene().sample_count(), 0);
    }
}

#[test]
fn duplicate_down_and_up_do_not_replace_or_duplicate_strokes() {
    let mut m = LiveMouse::new([832, 532]);
    begin(&mut m, 0);
    m.handle(button(true), 1);
    assert_eq!(
        m.scene().provisional().keys().copied().collect::<Vec<_>>(),
        vec![StrokeId(0)]
    );
    assert_eq!(m.scene().sample_count(), 1);
    finish(&mut m, 2);
    m.handle(button(false), 3);
    m.handle(button(false), 3);
    assert_eq!(m.scene().committed().len(), 1);
    stroke(&mut m, 4);
    assert!(m.scene().committed().contains_key(&StrokeId(1)));
}

#[test]
fn resize_cancels_active_mapping_and_next_stroke_uses_new_physical_viewport() {
    let mut m = LiveMouse::new([832, 532]);
    stroke(&mut m, 0);
    begin(&mut m, 2);
    m.handle(E::Resized([1632, 1032]), 3);
    assert!(!m.active());
    assert_eq!(m.scene().committed().len(), 1);
    m.handle(E::Moved([216.0, 216.0]), 4);
    m.handle(button(true), 4);
    m.handle(button(false), 5);
    let s = m.scene().committed()[&StrokeId(2)][0];
    assert_eq!(s.position_document.0, [100.0, 100.0]);
    // Existing document geometry does not change on resize.
    assert_eq!(
        m.scene().committed()[&StrokeId(0)][0].position_document.0,
        [100.0, 100.0]
    );
}

#[test]
fn physical_pixel_mapping_roundtrips_without_double_scaling() {
    for size in [[832, 532], [1632, 1032], [2000, 1400]] {
        let mapping = CanvasMapping::new(size).unwrap();
        let center = [f64::from(size[0]) / 2.0, f64::from(size[1]) / 2.0];
        assert_eq!(
            mapping.routing_viewport().to_document(center).unwrap().0,
            [400.0, 250.0]
        );
        assert!(mapping.contains_client(center));
        let p = mapping.routing_viewport().to_document(center).unwrap();
        assert_eq!(mapping.to_clip(p.0).unwrap(), [0.0, 0.0]);
        let [x, y, w, h] = mapping.scissor();
        assert!(w > 0 && h > 0 && x + w <= size[0] && y + h <= size[1]);
    }
}

#[test]
fn zero_sized_or_invalid_viewport_disables_paint_until_valid_resize() {
    let mut m = LiveMouse::new([0, 0]);
    begin(&mut m, 0);
    assert!(!m.active());
    m.handle(E::Resized([832, 532]), 1);
    stroke(&mut m, 2);
    assert_eq!(m.scene().committed().len(), 1);
    m.handle(E::Resized([9000, 9000]), 4);
    begin(&mut m, 5);
    assert!(!m.active());
}

#[test]
fn clear_cancels_inflight_work_preserves_cursor_and_never_reuses_transaction_ids() {
    let mut m = LiveMouse::new([832, 532]);
    stroke(&mut m, 0);
    begin(&mut m, 2);
    m.clear(3);
    assert_eq!(m.scene().sample_count(), 0);
    assert!(!m.active());
    m.handle(button(false), 4);
    assert_eq!(m.scene().sample_count(), 0);
    // A fresh click at the same known position should work without moving first.
    m.handle(button(true), 5);
    m.handle(button(false), 6);
    assert!(m.scene().committed().contains_key(&StrokeId(2)));
}

#[test]
fn live_and_replay_scene_namespaces_are_separate_and_switch_discards_previews() {
    let mut s = CanvasSession::new([832, 532]);
    for (e, t) in [
        (E::Moved([116.0, 116.0]), 0),
        (button(true), 1),
        (button(false), 2),
        (button(true), 3),
    ] {
        s.mouse(e, t);
    }
    assert_eq!(s.live().scene().committed().len(), 1);
    assert!(s.live().active());
    s.select_replay(Scenario::Completed, 4);
    assert!(!s.live().active());
    assert!(s.live().scene().provisional().is_empty());
    s.advance_replay(800).unwrap();
    assert_eq!(s.scene().provisional().len(), 1);
    s.mouse(E::Moved([500.0, 300.0]), 5);
    s.mouse(button(true), 5); // No input leaks from replay.
    s.set_mode(Mode::Live, 6);
    assert_eq!(s.scene().committed().len(), 1);
    assert!(s.scene().provisional().is_empty());
    s.mouse(E::Moved([216.0, 216.0]), 7);
    s.mouse(button(true), 7);
    s.mouse(button(false), 8);
    assert!(s.scene().committed().contains_key(&StrokeId(2)));
    s.set_mode(Mode::Replay, 9);
    assert_eq!(s.scene().sample_count(), 0);
    s.advance_replay(1800).unwrap();
    assert_eq!(s.scene().committed().len(), 1); // Replay ID 0 cannot collide with live ID 0.
    s.set_mode(Mode::Live, 10);
    assert_eq!(s.scene().committed().len(), 2);
}

#[test]
fn replay_restart_and_clear_controls_do_not_clear_hidden_live_work() {
    let mut s = CanvasSession::new([832, 532]);
    s.mouse(E::Moved([116.0, 116.0]), 0);
    s.mouse(button(true), 0);
    s.mouse(button(false), 1);
    s.select_replay(Scenario::Concurrent, 2);
    s.advance_replay(1600).unwrap();
    assert_eq!(s.scene().provisional().len(), 2);
    s.clear_live(3);
    assert_eq!(s.live().scene().committed().len(), 1);
    s.restart_replay();
    assert_eq!(s.scene().sample_count(), 0);
    s.advance_replay(2700).unwrap();
    assert_eq!(s.scene().committed().len(), 2);
    s.set_mode(Mode::Live, 4);
    s.clear_live(5);
    assert_eq!(s.scene().sample_count(), 0);
}

#[test]
fn repeated_mode_switches_and_clears_leave_no_active_contacts_or_retained_previews() {
    let mut s = CanvasSession::new([832, 532]);
    for time in 0..1000 {
        s.mouse(E::Moved([116.0, 116.0]), time);
        s.mouse(button(true), time);
        s.set_mode(Mode::Replay, time);
        s.advance_replay(1000).unwrap();
        s.set_mode(Mode::Live, time);
        s.clear_live(time);
        assert_eq!(s.scene().sample_count(), 0);
        assert!(!s.live().active());
    }
}

#[test]
fn canvas_capacity_is_nonfatal_and_clear_restores_drawing() {
    let mut m = LiveMouse::new([832, 532]);
    for i in 0..MAX_STROKES {
        stroke(&mut m, i as u64 * 2);
    }
    assert_eq!(m.scene().committed().len(), MAX_STROKES);
    begin(&mut m, 1000);
    assert!(!m.active());
    assert_eq!(m.notice(), Some(SceneError::Capacity));
    assert_eq!(m.scene().committed().len(), MAX_STROKES);
    m.clear(1001);
    stroke(&mut m, 1002);
    assert_eq!(m.scene().committed().len(), 1);
    assert_eq!(m.notice(), None);
}

#[test]
fn overlong_stroke_is_cancelled_without_erasing_previous_commits() {
    let mut m = LiveMouse::new([832, 532]);
    stroke(&mut m, 0);
    begin(&mut m, 2);
    for i in 0..MAX_POINTS_PER_STROKE {
        m.handle(E::Moved([116.0 + (i % 200) as f64, 116.0]), i as u64 + 3);
    }
    assert!(!m.active());
    assert!(m.scene().provisional().is_empty());
    assert_eq!(m.scene().committed().len(), 1);
    assert_eq!(m.notice(), Some(SceneError::Capacity));
    m.handle(button(false), 2000);
    assert_eq!(m.scene().committed().len(), 1);
}
