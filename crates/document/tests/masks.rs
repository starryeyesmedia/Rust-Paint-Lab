use paint_document::editing::{Selection, SelectionKind};
use paint_document::{Engine, LayerMask, MASK_TILE_BYTES, StrokeMode, TILE_BYTES, storage};
use paint_input::Tool;
use paint_stroke::{
    CancelReason, StrokeCommand, StrokeId, StrokeSample, coordinates::DocumentPoint,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

fn sample(x: f64, y: f64, time_ms: u64) -> StrokeSample {
    StrokeSample {
        source_counter: None,
        pointer_id: 1,
        tool: Tool::Pen,
        position_client_px: [x, y],
        position_document: DocumentPoint([x, y]),
        pressure: Some(1.0),
        tilt_degrees: [None, None],
        buttons: Default::default(),
        source_time_ms: time_ms,
        time_ms,
    }
}

fn empty_canvas() -> Engine {
    let mut engine = Engine::new(32, 32, 300).unwrap();
    engine
        .edit(|state| {
            state.background = None;
            Ok(())
        })
        .unwrap();
    engine.brush.color = [240, 20, 10];
    engine.brush.size = 8.0;
    engine.brush.hardness = 1.0;
    engine.brush.pressure_size = false;
    engine.brush.pressure_opacity = false;
    engine
}

fn stroke(engine: &mut Engine, id: u64, mode: StrokeMode) {
    engine
        .apply_with_mode(
            &StrokeCommand::BeginStroke {
                stroke_id: StrokeId(id),
                sample: sample(16.0, 16.0, id * 2),
            },
            mode,
        )
        .unwrap();
    engine
        .apply_with_mode(
            &StrokeCommand::CommitStroke {
                stroke_id: StrokeId(id),
                sample: sample(16.0, 16.0, id * 2 + 1),
            },
            mode,
        )
        .unwrap();
}

fn pixel(engine: &Engine, x: usize, y: usize) -> [u8; 4] {
    let rgba = engine.rgba();
    rgba[(y * engine.state().width as usize + x) * 4..][..4]
        .try_into()
        .unwrap()
}

fn refresh_checksum(bytes: &mut [u8]) {
    let digest = Sha256::digest(&bytes[..bytes.len() - 32]);
    let split = bytes.len() - 32;
    bytes[split..].copy_from_slice(&digest);
}

#[test]
fn mask_paint_preview_commit_cancel_undo_redo_and_eraser_are_transactional() {
    let mut engine = empty_canvas();
    stroke(&mut engine, 1, StrokeMode::Paint);
    assert_eq!(pixel(&engine, 16, 16), [240, 20, 10, 255]);
    engine.add_layer_mask(1).unwrap();

    engine.brush.color = [0, 0, 0];
    let before = engine.rgba();
    engine
        .apply_with_mode(
            &StrokeCommand::BeginStroke {
                stroke_id: StrokeId(2),
                sample: sample(16.0, 16.0, 4),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();
    assert_eq!(pixel(&engine, 16, 16), [0, 0, 0, 0]);
    assert!(
        engine.state().layers[0]
            .mask
            .as_ref()
            .unwrap()
            .tiles
            .is_empty()
    );
    engine
        .apply(&StrokeCommand::CancelStroke {
            stroke_id: StrokeId(2),
            last: sample(16.0, 16.0, 5),
            time_ms: 5,
            reason: CancelReason::RouterInterrupted,
        })
        .unwrap();
    assert_eq!(engine.rgba(), before);
    assert!(
        engine.state().layers[0]
            .mask
            .as_ref()
            .unwrap()
            .tiles
            .is_empty()
    );

    stroke(&mut engine, 3, StrokeMode::MaskPaint);
    assert_eq!(pixel(&engine, 16, 16), [0, 0, 0, 0]);
    let mask = engine.state().layers[0].mask.as_ref().unwrap();
    assert_eq!(mask.tiles[&(0, 0)][16 * 64 + 16], 0);
    assert!(engine.undo());
    assert_eq!(pixel(&engine, 16, 16), [240, 20, 10, 255]);
    assert!(engine.redo());
    assert_eq!(pixel(&engine, 16, 16), [0, 0, 0, 0]);

    engine.brush.kind = paint_document::brush::BrushKind::Eraser;
    stroke(&mut engine, 4, StrokeMode::MaskPaint);
    assert_eq!(pixel(&engine, 16, 16), [240, 20, 10, 255]);
    assert_eq!(engine.undo_count(), 5);
}

#[test]
fn mask_controls_are_undoable_and_affect_the_composite_without_touching_raster() {
    let mut engine = empty_canvas();
    stroke(&mut engine, 1, StrokeMode::Paint);
    let raster = engine.state().layers[0].tiles.clone();
    engine.add_layer_mask(1).unwrap();
    engine.brush.color = [0, 0, 0];
    stroke(&mut engine, 2, StrokeMode::MaskPaint);
    assert_eq!(pixel(&engine, 16, 16)[3], 0);

    engine.set_layer_mask_enabled(1, false).unwrap();
    assert_eq!(pixel(&engine, 16, 16)[3], 255);
    engine.undo();
    assert_eq!(pixel(&engine, 16, 16)[3], 0);
    engine.invert_layer_mask(1).unwrap();
    assert_eq!(pixel(&engine, 16, 16)[3], 255);
    engine.undo();
    assert_eq!(pixel(&engine, 16, 16)[3], 0);
    engine.clear_layer_mask(1).unwrap();
    assert_eq!(pixel(&engine, 16, 16)[3], 255);
    engine.undo();
    assert_eq!(pixel(&engine, 16, 16)[3], 0);
    assert_eq!(engine.state().layers[0].tiles, raster);
    engine.remove_layer_mask(1).unwrap();
    assert!(engine.state().layers[0].mask.is_none());
    engine.undo();
    assert!(engine.state().layers[0].mask.is_some());
    assert_eq!(pixel(&engine, 16, 16)[3], 0);
}

#[test]
fn mask_paint_obeys_selection_and_composites_without_mutating_raster_tiles() {
    let mut engine = empty_canvas();
    engine.brush.size = 20.0;
    stroke(&mut engine, 1, StrokeMode::Paint);
    let raster_tiles = engine.state().layers[0].tiles.clone();
    engine.add_layer_mask(1).unwrap();
    engine
        .set_selection(Some(
            Selection::new(SelectionKind::Rectangle, vec![[12.0, 12.0], [20.0, 20.0]]).unwrap(),
        ))
        .unwrap();
    engine.brush.color = [0, 0, 0];
    engine.brush.size = 8.0;
    engine
        .apply_with_mode(
            &StrokeCommand::BeginStroke {
                stroke_id: StrokeId(2),
                sample: sample(16.0, 16.0, 4),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();
    engine
        .apply_with_mode(
            &StrokeCommand::CommitStroke {
                stroke_id: StrokeId(2),
                sample: sample(16.0, 16.0, 5),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();

    let mask = engine.state().layers[0].mask.as_ref().unwrap();
    assert_eq!(mask.tiles[&(0, 0)][16 * 64 + 16], 0);
    assert_eq!(mask.tiles[&(0, 0)][16 * 64 + 10], 255);
    assert_eq!(pixel(&engine, 16, 16), [0, 0, 0, 0]);
    assert_eq!(pixel(&engine, 10, 16), [240, 20, 10, 255]);
    assert_eq!(engine.state().layers[0].tiles, raster_tiles);
}

#[test]
fn mask_paint_clips_a_crossing_stroke_to_selection_and_rejects_outside_strokes() {
    let mut engine = empty_canvas();
    stroke(&mut engine, 1, StrokeMode::Paint);
    let raster_tiles = engine.state().layers[0].tiles.clone();
    engine.add_layer_mask(1).unwrap();
    engine
        .set_selection(Some(
            Selection::new(SelectionKind::Rectangle, vec![[12.0, 12.0], [20.0, 20.0]]).unwrap(),
        ))
        .unwrap();
    engine.brush.color = [0, 0, 0];
    engine.brush.size = 8.0;

    // The real transaction starts beyond the left edge and crosses through
    // the selected region. Per-pixel selection clipping must prevent the
    // brush radius and interpolated dabs from changing the surrounding mask.
    engine
        .apply_with_mode(
            &StrokeCommand::BeginStroke {
                stroke_id: StrokeId(2),
                sample: sample(4.0, 16.0, 4),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();
    engine
        .apply_with_mode(
            &StrokeCommand::UpdateStroke {
                stroke_id: StrokeId(2),
                sample: sample(16.0, 16.0, 5),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();
    engine
        .apply_with_mode(
            &StrokeCommand::CommitStroke {
                stroke_id: StrokeId(2),
                sample: sample(28.0, 16.0, 6),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();

    let crossing_tiles = engine.state().layers[0]
        .mask
        .as_ref()
        .unwrap()
        .tiles
        .clone();
    let crossing = crossing_tiles
        .get(&(0, 0))
        .expect("selected pixels changed");
    let mut changed_pixels = 0;
    for y in 0..32 {
        for x in 0..32 {
            let value = crossing[(y * 64 + x) as usize];
            if value < 255 {
                changed_pixels += 1;
                assert!(
                    (12..20).contains(&x) && (12..20).contains(&y),
                    "mask changed outside selected pixel centers at ({x}, {y}): {value}"
                );
            }
        }
    }
    assert!(
        changed_pixels > 0,
        "the crossing stroke must paint its selected portion"
    );
    assert_eq!(crossing[(16 * 64 + 8) as usize], 255);
    assert_eq!(crossing[(16 * 64 + 12) as usize], 0);
    assert_eq!(crossing[(16 * 64 + 19) as usize], 0);
    assert_eq!(crossing[(16 * 64 + 20) as usize], 255);
    assert_eq!(engine.state().layers[0].tiles, raster_tiles);

    assert!(engine.undo());
    assert!(
        engine.state().layers[0]
            .mask
            .as_ref()
            .unwrap()
            .tiles
            .is_empty()
    );
    assert!(engine.redo());
    assert_eq!(
        engine.state().layers[0].mask.as_ref().unwrap().tiles,
        crossing_tiles
    );

    // A separate stroke entirely outside the selection is a committed no-op:
    // it creates neither mask tiles nor an undo entry.
    let history_before_outside = engine.undo_count();
    engine
        .apply_with_mode(
            &StrokeCommand::BeginStroke {
                stroke_id: StrokeId(3),
                sample: sample(4.0, 4.0, 7),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();
    engine
        .apply_with_mode(
            &StrokeCommand::UpdateStroke {
                stroke_id: StrokeId(3),
                sample: sample(16.0, 4.0, 8),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();
    engine
        .apply_with_mode(
            &StrokeCommand::CommitStroke {
                stroke_id: StrokeId(3),
                sample: sample(28.0, 4.0, 9),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();
    assert_eq!(engine.undo_count(), history_before_outside);
    assert_eq!(
        engine.state().layers[0].mask.as_ref().unwrap().tiles,
        crossing_tiles
    );
    assert_eq!(engine.state().layers[0].tiles, raster_tiles);
}

#[test]
fn mask_mode_requires_a_mask_and_rejects_locked_or_hidden_layers() {
    let mut engine = empty_canvas();
    let command = StrokeCommand::BeginStroke {
        stroke_id: StrokeId(1),
        sample: sample(16.0, 16.0, 1),
    };
    assert!(
        engine
            .apply_with_mode(&command, StrokeMode::MaskPaint)
            .is_err()
    );
    assert_eq!(engine.active_count(), 0);
    engine.add_layer_mask(1).unwrap();
    engine
        .edit(|state| {
            state.layers[0].locked = true;
            Ok(())
        })
        .unwrap();
    assert!(
        engine
            .apply_with_mode(&command, StrokeMode::MaskPaint)
            .is_err()
    );
    engine
        .edit(|state| {
            state.layers[0].locked = false;
            state.layers[0].visible = false;
            Ok(())
        })
        .unwrap();
    assert!(
        engine
            .apply_with_mode(&command, StrokeMode::MaskPaint)
            .is_err()
    );
    assert_eq!(engine.active_count(), 0);
}

#[test]
fn native_v1_remains_readable_and_masked_projects_roundtrip_as_v2() {
    let mut engine = empty_canvas();
    stroke(&mut engine, 1, StrokeMode::Paint);
    let v1 = storage::encode(engine.state()).unwrap();
    assert_eq!(&v1[..8], b"RPLALP01");
    let legacy = storage::decode(&v1).unwrap();
    assert!(legacy.layers[0].mask.is_none());
    assert_eq!(Engine::from_state(legacy).unwrap().rgba(), engine.rgba());

    engine.add_layer_mask(1).unwrap();
    engine.brush.color = [0, 0, 0];
    stroke(&mut engine, 2, StrokeMode::MaskPaint);
    let v2 = storage::encode(engine.state()).unwrap();
    assert_eq!(&v2[..8], b"RPLALP02");
    let reopened = Engine::from_state(storage::decode(&v2).unwrap()).unwrap();
    assert_eq!(reopened.rgba(), engine.rgba());
    assert_eq!(
        reopened.state().layers[0].mask,
        engine.state().layers[0].mask
    );
}

#[test]
fn malformed_v2_mask_records_are_rejected_after_checksum_validation() {
    let mut engine = empty_canvas();
    stroke(&mut engine, 1, StrokeMode::Paint);
    engine.add_layer_mask(1).unwrap();
    engine.brush.color = [0, 0, 0];
    stroke(&mut engine, 2, StrokeMode::MaskPaint);
    let valid = storage::encode(engine.state()).unwrap();
    let metadata_length = u32::from_le_bytes(valid[8..12].try_into().unwrap()) as usize;
    let raster_count_at = 12 + metadata_length;
    let raster_count = u32::from_le_bytes(
        valid[raster_count_at..raster_count_at + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let mask_count_at = raster_count_at + 4 + raster_count * (16 + TILE_BYTES);
    assert_eq!(
        u32::from_le_bytes(valid[mask_count_at..mask_count_at + 4].try_into().unwrap()),
        1
    );
    let mut unknown_layer = valid.clone();
    unknown_layer[mask_count_at + 4..mask_count_at + 12].copy_from_slice(&99u64.to_le_bytes());
    refresh_checksum(&mut unknown_layer);
    assert!(storage::decode(&unknown_layer).is_err());

    let mut truncated = valid.clone();
    truncated.truncate(truncated.len() - 40);
    let digest = Sha256::digest(&truncated);
    truncated.extend(digest);
    assert!(storage::decode(&truncated).is_err());
}

#[test]
fn mask_tiles_share_document_tile_limit_and_state_validation_checks_geometry() {
    let mut engine = empty_canvas();
    engine
        .edit(|state| {
            state.layers[0].mask = Some(LayerMask {
                enabled: true,
                inverted: false,
                tiles: std::collections::BTreeMap::from([(
                    (0, 0),
                    Arc::new(vec![255; MASK_TILE_BYTES]),
                )]),
            });
            Ok(())
        })
        .unwrap();
    assert_eq!(engine.state().tile_count(), 1);
    engine.state().validate().unwrap();
    let invalid = engine.edit(|state| {
        state.layers[0]
            .mask
            .as_mut()
            .unwrap()
            .tiles
            .insert((1, 0), Arc::new(vec![255; MASK_TILE_BYTES]));
        Ok(())
    });
    assert!(invalid.is_err());
}

#[test]
fn thumbnails_apply_masks_and_selection_transforms_keep_masks_aligned_and_undoable() {
    let mut engine = Engine::new(4, 4, 300).unwrap();
    engine
        .edit(|state| {
            state.background = None;
            let mut pixels = vec![0; TILE_BYTES];
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[120, 0, 0, 255]);
            }
            let mut mask = vec![255; MASK_TILE_BYTES];
            mask[0] = 0;
            state.layers[0].tiles.insert((0, 0), Arc::new(pixels));
            state.layers[0].mask = Some(LayerMask {
                enabled: true,
                inverted: false,
                tiles: [((0, 0), Arc::new(mask))].into_iter().collect(),
            });
            Ok(())
        })
        .unwrap();
    let (_, _, thumbnail) = engine.thumbnail_rgba(4, 4).unwrap();
    assert_eq!(&thumbnail[..4], &[0, 0, 0, 0]);
    assert_eq!(&thumbnail[4..8], &[120, 0, 0, 255]);

    let mut moving = Engine::new(32, 32, 300).unwrap();
    moving
        .edit(|state| {
            state.background = None;
            let mut pixels = vec![0; TILE_BYTES];
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[120, 0, 0, 255]);
            }
            let mut mask = vec![255; MASK_TILE_BYTES];
            mask[16 * 64 + 16] = 0;
            state.layers[0].tiles.insert((0, 0), Arc::new(pixels));
            state.layers[0].mask = Some(LayerMask {
                enabled: true,
                inverted: false,
                tiles: [((0, 0), Arc::new(mask))].into_iter().collect(),
            });
            Ok(())
        })
        .unwrap();
    moving
        .set_selection(Some(
            paint_document::editing::Selection::new(
                paint_document::editing::SelectionKind::Rectangle,
                vec![[8.0, 8.0], [24.0, 24.0]],
            )
            .unwrap(),
        ))
        .unwrap();
    let before_transform = moving.rgba();
    moving
        .transform_selection([1.0, 0.0, 0.0, 1.0, 8.0, 0.0])
        .unwrap();
    assert_eq!(pixel(&moving, 16, 16)[3], 255);
    assert_eq!(pixel(&moving, 24, 16)[3], 0);
    let moved_mask = &moving.state().layers[0].mask.as_ref().unwrap().tiles[&(0, 0)];
    assert_eq!(moved_mask[16 * 64 + 16], 255);
    assert_eq!(moved_mask[16 * 64 + 24], 0);
    assert!(moving.undo());
    assert_eq!(moving.rgba(), before_transform);
    let reopened =
        Engine::from_state(storage::decode(&storage::encode(moving.state()).unwrap()).unwrap())
            .unwrap();
    assert_eq!(
        reopened.state().layers[0].mask,
        moving.state().layers[0].mask
    );
}
