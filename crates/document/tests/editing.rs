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
fn layer_pixel(engine: &Engine, layer_id: u64, x: usize, y: usize) -> [u8; 4] {
    let layer = engine
        .state()
        .layers
        .iter()
        .find(|layer| layer.id == layer_id)
        .unwrap();
    let Some(tile) = layer.tiles.get(&((x as u32) / 64, (y as u32) / 64)) else {
        return [0; 4];
    };
    let offset = ((((y as u32) % 64) * 64 + (x as u32) % 64) * 4) as usize;
    tile[offset..offset + 4].try_into().unwrap()
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
fn copy_selection_to_new_layer_preserves_source_and_copies_pixel_center_masks() {
    for (case_index, selection) in [
        rect(K::Rectangle),
        rect(K::Ellipse),
        Selection::new(
            K::Lasso,
            vec![[2.0, 2.0], [28.0, 3.0], [18.0, 28.0], [4.0, 20.0]],
        )
        .unwrap(),
    ]
    .into_iter()
    .enumerate()
    {
        let mut engine = empty();
        engine.brush.color = [90, 30, 210];
        engine.fill_selection(false).unwrap();
        let source_id = engine.state().selected;
        let source_tiles = engine.state().layers[0].tiles.clone();
        let original_composite = engine.rgba();
        engine.set_selection(Some(selection.clone())).unwrap();

        let undo_before = engine.undo_count();
        let copied_id = engine.copy_selection_to_new_layer().unwrap();
        assert_eq!(engine.undo_count(), undo_before + 1);
        assert_eq!(engine.state().selected, copied_id);
        assert_eq!(engine.state().layers.len(), 2);
        assert_eq!(engine.state().layers[0].id, source_id);
        assert_eq!(engine.state().layers[1].id, copied_id);
        assert_eq!(engine.state().layers[1].name, "Paint 1 copy");
        assert_eq!(engine.state().selection.as_ref(), Some(&selection));
        assert_eq!(engine.state().layers[0].tiles, source_tiles);

        for y in 0..32 {
            for x in 0..32 {
                let expected = if selection.contains([x as f64 + 0.5, y as f64 + 0.5]) {
                    layer_pixel(&engine, source_id, x, y)
                } else {
                    [0; 4]
                };
                assert_eq!(layer_pixel(&engine, copied_id, x, y), expected);
            }
        }
        assert_eq!(engine.rgba(), original_composite);

        assert!(engine.undo());
        assert_eq!(engine.state().layers.len(), 1);
        assert_eq!(engine.state().selected, source_id);
        assert_eq!(engine.state().selection.as_ref(), Some(&selection));
        assert_eq!(engine.rgba(), original_composite);
        assert!(engine.redo());
        assert_eq!(engine.state().selected, copied_id);
        assert_eq!(engine.rgba(), original_composite);

        let root = std::env::temp_dir().join(format!(
            "rpl-copy-selection-{}-{case_index}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let native = root.join("copy.rpl");
        {
            let mut store = paint_document::storage::Store::new(native.clone());
            store.save(engine.state()).unwrap();
            let (_store, reopened_state) = paint_document::storage::Store::open(native).unwrap();
            let reopened = Engine::from_state(reopened_state).unwrap();
            assert_eq!(reopened.state().selected, copied_id);
            assert_eq!(reopened.rgba(), original_composite);
            assert_eq!(
                reopened.state().layers[1].tiles,
                engine.state().layers[1].tiles
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn copy_selection_rejects_missing_empty_hidden_or_locked_sources_atomically() {
    let mut engine = empty();
    let before = engine.rgba();
    let undo_before = engine.undo_count();
    assert!(engine.copy_selection_to_new_layer().is_err());
    assert_eq!(engine.rgba(), before);
    assert_eq!(engine.state().layers.len(), 1);
    assert_eq!(engine.undo_count(), undo_before);

    engine.set_selection(Some(rect(K::Rectangle))).unwrap();
    assert!(engine.copy_selection_to_new_layer().is_err());
    assert_eq!(engine.rgba(), before);
    assert_eq!(engine.state().layers.len(), 1);
    assert_eq!(engine.undo_count(), undo_before);

    engine.fill_selection(false).unwrap();
    engine
        .edit(|state| {
            state.layers[0].locked = true;
            Ok(())
        })
        .unwrap();
    let locked_pixels = engine.rgba();
    let locked_history = engine.undo_count();
    assert!(matches!(
        engine.copy_selection_to_new_layer(),
        Err(paint_document::PaintError::Locked)
    ));
    assert_eq!(engine.rgba(), locked_pixels);
    assert_eq!(engine.state().layers.len(), 1);
    assert_eq!(engine.undo_count(), locked_history);

    engine
        .edit(|state| {
            state.layers[0].locked = false;
            state.layers[0].visible = false;
            Ok(())
        })
        .unwrap();
    let hidden_history = engine.undo_count();
    assert!(matches!(
        engine.copy_selection_to_new_layer(),
        Err(paint_document::PaintError::Locked)
    ));
    assert_eq!(engine.state().layers.len(), 1);
    assert_eq!(engine.undo_count(), hidden_history);
}

#[test]
fn copy_selection_obeys_layer_and_tile_budgets_without_losing_source_art() {
    use std::{collections::BTreeMap, sync::Arc};

    let mut full = empty();
    full.fill_selection(false).unwrap();
    full.set_selection(Some(rect(K::Rectangle))).unwrap();
    for _ in 1..paint_document::MAX_LAYERS {
        full.add_layer().unwrap();
    }
    let layer_count = full.state().layers.len();
    let selected = full.state().selected;
    let history = full.undo_count();
    assert!(matches!(
        full.copy_selection_to_new_layer(),
        Err(paint_document::PaintError::Limit)
    ));
    assert_eq!(full.state().layers.len(), layer_count);
    assert_eq!(full.state().selected, selected);
    assert_eq!(full.undo_count(), history);

    let shared = Arc::new({
        let mut tile = vec![0; paint_document::TILE_BYTES];
        tile[..4].copy_from_slice(&[10, 20, 30, 255]);
        tile
    });
    let mut state = paint_document::State::new(4096, 4096, 300).unwrap();
    state.background = None;
    state.selection = Some(Selection::new(K::Rectangle, vec![[0.0, 0.0], [1.0, 1.0]]).unwrap());
    for layer_index in 0..paint_document::MAX_LAYERS {
        let mut tiles = BTreeMap::new();
        for tile_y in 0..4 {
            for tile_x in 0..64 {
                tiles.insert((tile_x, tile_y), shared.clone());
            }
        }
        if layer_index == 0 {
            state.layers[0].tiles = tiles;
        } else {
            state.layers.push(paint_document::Layer {
                id: (layer_index + 1) as u64,
                name: format!("Paint {}", layer_index + 1),
                visible: true,
                locked: false,
                opacity: 255,
                blend: paint_document::Blend::Normal,
                tiles,
            });
        }
    }
    state.selected = paint_document::MAX_LAYERS as u64;
    let mut full_tiles = Engine::from_state(state).unwrap();
    let original_tile_count = full_tiles.state().tile_count();
    let original_layers = full_tiles.state().layers.len();
    let original_selection = full_tiles.state().selection.clone();
    assert_eq!(original_tile_count, paint_document::MAX_TILES);
    assert!(matches!(
        full_tiles.copy_selection_to_new_layer(),
        Err(paint_document::PaintError::Limit)
    ));
    assert_eq!(full_tiles.state().tile_count(), original_tile_count);
    assert_eq!(full_tiles.state().layers.len(), original_layers);
    assert_eq!(full_tiles.state().selection, original_selection);
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
