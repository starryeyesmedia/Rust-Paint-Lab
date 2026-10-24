use paint_document::{
    Engine,
    editing::{Selection, SelectionKind as K},
};
fn rect(kind: K) -> Selection {
    Selection::new(kind, vec![[8.0, 8.0], [24.0, 24.0]]).unwrap()
}
fn empty() -> Engine {
    let mut e = Engine::new(32, 32, 300).unwrap();
    e.edit(|s| {
        s.background = None;
        Ok(())
    })
    .unwrap();
    e.brush.color = [255, 0, 0];
    e
}
fn pixel(e: &Engine, x: usize, y: usize) -> [u8; 4] {
    e.rgba()[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4]
        .try_into()
        .unwrap()
}
#[test]
fn selection_boundaries_and_nonfinite_rejection() {
    for k in [K::Rectangle, K::Ellipse] {
        let s = rect(k);
        assert!(s.contains([16.5, 16.5]));
        assert!(!s.contains([24.0, 16.5]));
        assert!(!s.contains([7.9, 16.0]));
    }
    assert!(!rect(K::Ellipse).contains([8.5, 8.5]));
    assert!(Selection::new(K::Rectangle, vec![[0.0, 0.0], [f64::NAN, 10.0]]).is_err());
    assert!(Selection::new(K::Lasso, vec![[0.0, 0.0]; 1025]).is_err());
}
#[test]
fn fill_delete_selection_and_history_are_exact() {
    let mut e = empty();
    e.set_selection(Some(rect(K::Rectangle))).unwrap();
    e.fill_selection(false).unwrap();
    assert_eq!(pixel(&e, 16, 16), [255, 0, 0, 255]);
    assert_eq!(pixel(&e, 7, 16), [0; 4]);
    let before = e.rgba();
    e.fill_selection(true).unwrap();
    assert_eq!(pixel(&e, 16, 16), [0; 4]);
    assert!(e.undo());
    assert_eq!(e.rgba(), before);
    assert!(e.undo());
    assert_eq!(pixel(&e, 16, 16), [0; 4]);
    assert!(e.redo());
    assert_eq!(e.rgba(), before);
}
#[test]
fn ellipse_and_lasso_fill_match_pixel_center_geometry() {
    for selection in [
        rect(K::Ellipse),
        Selection::new(K::Lasso, vec![[2.0, 2.0], [28.0, 2.0], [16.0, 28.0]]).unwrap(),
    ] {
        let mut e = empty();
        e.set_selection(Some(selection.clone())).unwrap();
        e.fill_selection(false).unwrap();
        for y in 0..32 {
            for x in 0..32 {
                assert_eq!(
                    pixel(&e, x, y)[3] > 0,
                    selection.contains([x as f64 + 0.5, y as f64 + 0.5])
                );
            }
        }
    }
}
#[test]
fn transforms_move_flip_rotate_scale_and_undo() {
    for m in [
        [1.0, 0.0, 0.0, 1.0, 4.0, 0.0],
        [-1.0, 0.0, 0.0, 1.0, 32.0, 0.0],
        [0.0, 1.0, -1.0, 0.0, 32.0, 0.0],
        [0.5, 0.0, 0.0, 0.5, 8.0, 8.0],
    ] {
        let mut e = empty();
        e.set_selection(Some(rect(K::Rectangle))).unwrap();
        e.fill_selection(false).unwrap();
        let before = e.rgba();
        e.transform_selection(m).unwrap();
        assert!(e.state().selection.is_none());
        assert!(e.rgba().iter().any(|p| *p > 0));
        e.undo();
        assert_eq!(e.rgba(), before);
        assert!(e.state().selection.is_some());
    }
    let mut e = empty();
    let before = e.rgba();
    assert!(e.transform_selection([0.0; 6]).is_err());
    assert!(e.transform_selection([f64::NAN; 6]).is_err());
    assert_eq!(e.rgba(), before);
}
#[test]
fn crop_keeps_all_editable_layers_and_is_undoable() {
    let mut e = empty();
    e.set_selection(Some(rect(K::Rectangle))).unwrap();
    e.fill_selection(false).unwrap();
    e.duplicate_layer().unwrap();
    e.crop_selection().unwrap();
    assert_eq!((e.state().width, e.state().height), (16, 16));
    assert_eq!(e.state().layers.len(), 2);
    assert_eq!(e.rgba(), [255, 0, 0, 255].repeat(256));
    e.undo();
    assert_eq!(e.state().width, 32);
    assert!(e.state().selection.is_some());
}
#[test]
fn eyedropper_and_lock_guards_preserve_artwork() {
    let mut e = empty();
    e.fill_selection(false).unwrap();
    assert_eq!(e.eyedropper([2.0, 2.0]).unwrap(), [255, 0, 0]);
    assert!(e.eyedropper([f64::INFINITY, 0.0]).is_err());
    e.edit(|s| {
        s.layers[0].locked = true;
        Ok(())
    })
    .unwrap();
    let before = e.rgba();
    assert!(e.fill_selection(true).is_err());
    assert!(
        e.transform_selection([1.0, 0.0, 0.0, 1.0, 1.0, 0.0])
            .is_err()
    );
    assert_eq!(e.rgba(), before);
}

#[test]
fn raster_brush_respects_selection_and_cancel_stays_out_of_history() {
    use paint_input::{Buttons, Tool};
    use paint_stroke::{
        CancelReason, StrokeCommand as C, StrokeId, StrokeSample, coordinates::DocumentPoint,
    };
    let mut e = empty();
    e.brush.size = 32.0;
    e.brush.pressure_size = false;
    e.set_selection(Some(rect(K::Rectangle))).unwrap();
    let sample = StrokeSample {
        source_counter: None,
        pointer_id: 1,
        tool: Tool::Mouse,
        position_client_px: [16.0, 16.0],
        position_document: DocumentPoint([16.0, 16.0]),
        pressure: None,
        tilt_degrees: [None, None],
        buttons: Buttons::default(),
        source_time_ms: 0,
        time_ms: 0,
    };
    let count = e.undo_count();
    e.apply(&C::BeginStroke {
        stroke_id: StrokeId(1),
        sample,
    })
    .unwrap();
    assert!(pixel(&e, 16, 16)[3] > 0);
    assert_eq!(pixel(&e, 7, 16), [0; 4]);
    e.apply(&C::CancelStroke {
        stroke_id: StrokeId(1),
        last: sample,
        time_ms: 1,
        reason: CancelReason::RouterInterrupted,
    })
    .unwrap();
    assert_eq!(pixel(&e, 16, 16), [0; 4]);
    assert_eq!(e.undo_count(), count);
}
