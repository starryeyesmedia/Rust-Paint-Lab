use paint_document::{
    Engine, StrokeMode, storage,
    vector::{VectorHandle, VectorSegment},
};
use paint_input::Tool;
use paint_stroke::{StrokeCommand, StrokeId, StrokeSample, coordinates::DocumentPoint};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::PathBuf};

fn blank() -> Engine {
    let mut engine = Engine::new(96, 64, 300).unwrap();
    engine
        .edit(|state| {
            state.background = None;
            Ok(())
        })
        .unwrap();
    engine.brush.color = [220, 40, 145];
    engine.brush.size = 3.0;
    engine.brush.opacity = 1.0;
    engine
}

fn add_path(engine: &mut Engine) -> u64 {
    engine
        .add_vector_path(&[[8.0, 12.0], [24.0, 12.0], [35.0, 20.0]])
        .unwrap()
}

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

fn pixel(engine: &Engine, x: usize, y: usize) -> [u8; 4] {
    engine.rgba()[(y * engine.state().width as usize + x) * 4..][..4]
        .try_into()
        .unwrap()
}

fn temp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("rpl-vector-{}-{name}", std::process::id()))
}

fn reseal(bytes: &mut [u8]) {
    let checksum = Sha256::digest(&bytes[..bytes.len() - 32]);
    let end = bytes.len();
    bytes[end - 32..].copy_from_slice(&checksum);
}

#[test]
fn editable_path_renders_without_raster_mutation_and_roundtrips_in_v3() {
    let mut engine = blank();
    let layer_id = engine.state().selected;
    let history_before = engine.undo_count();
    let path_id = add_path(&mut engine);

    let layer = engine
        .state()
        .layers
        .iter()
        .find(|layer| layer.id == layer_id)
        .unwrap();
    assert!(
        layer.tiles.is_empty(),
        "vectors must not be baked into raster tiles"
    );
    assert_eq!(layer.vector_paths.len(), 1);
    assert_eq!(layer.vector_paths[0].id, path_id);
    assert_eq!(layer.vector_paths[0].segments.len(), 2);
    assert_eq!(pixel(&engine, 18, 12), [220, 40, 145, 255]);
    assert_eq!(engine.undo_count(), history_before + 1);
    let (_, _, thumbnail) = engine.thumbnail_rgba(48, 32).unwrap();
    assert!(
        thumbnail
            .chunks_exact(4)
            .any(|pixel| pixel[0] > pixel[1] && pixel[2] > pixel[1] && pixel[3] > 0)
    );

    let encoded = storage::encode(engine.state()).unwrap();
    assert_eq!(&encoded[..8], b"RPLALP03");
    let reopened = Engine::from_state(storage::decode(&encoded).unwrap()).unwrap();
    assert_eq!(reopened.state().layers[0].vector_paths, layer.vector_paths);
    assert_eq!(reopened.rgba(), engine.rgba());

    assert!(engine.undo());
    assert!(engine.state().layers[0].vector_paths.is_empty());
    assert_eq!(pixel(&engine, 18, 12), [0; 4]);
    assert!(engine.redo());
    assert_eq!(pixel(&engine, 18, 12), [220, 40, 145, 255]);
}

#[test]
fn dense_editable_paths_keep_document_and_layer_previews_tile_bounded() {
    let mut engine = Engine::new(2048, 2048, 300).unwrap();
    engine.brush.size = 1.0;
    for path_index in 0..8 {
        let points = (0..=1_024)
            .map(|index| {
                [
                    100.0 + index as f64 / 1_024.0,
                    100.0 + f64::from(index % 2) * 0.05 + f64::from(path_index) * 0.1,
                ]
            })
            .collect::<Vec<_>>();
        engine.add_vector_path(&points).unwrap();
    }
    assert_eq!(
        engine.state().layers[0]
            .vector_paths
            .iter()
            .map(|path| path.segments.len())
            .sum::<usize>(),
        paint_document::vector::MAX_VECTOR_SEGMENTS
    );

    let (_, _, home_preview) = engine.thumbnail_rgba(128, 128).unwrap();
    assert!(
        home_preview
            .chunks_exact(4)
            .any(|pixel| pixel[0] > 0 && pixel[3] > 0)
    );
}

#[test]
fn existing_v1_and_v2_versions_are_selected_without_vector_content() {
    let engine = blank();
    assert_eq!(&storage::encode(engine.state()).unwrap()[..8], b"RPLALP01");

    let mut masked = blank();
    let layer_id = masked.state().selected;
    masked.add_layer_mask(layer_id).unwrap();
    assert_eq!(&storage::encode(masked.state()).unwrap()[..8], b"RPLALP02");
    let reopened =
        Engine::from_state(storage::decode(&storage::encode(masked.state()).unwrap()).unwrap())
            .unwrap();
    assert!(reopened.state().layers[0].mask.is_some());
    assert!(reopened.state().layers[0].vector_paths.is_empty());
}

#[test]
fn vector_and_editable_mask_share_a_layer_and_v3_recovery_snapshot() {
    let mut engine = blank();
    let layer_id = engine.state().selected;
    let path_id = add_path(&mut engine);
    engine.add_layer_mask(layer_id).unwrap();
    assert_eq!(pixel(&engine, 18, 12), [220, 40, 145, 255]);
    engine.invert_layer_mask(layer_id).unwrap();
    assert_eq!(pixel(&engine, 18, 12), [0; 4]);

    let data = storage::encode(engine.state()).unwrap();
    assert_eq!(&data[..8], b"RPLALP03");
    let decoded = storage::decode(&data).unwrap();
    let layer = decoded
        .layers
        .iter()
        .find(|layer| layer.id == layer_id)
        .unwrap();
    assert!(layer.mask.as_ref().unwrap().inverted);
    assert_eq!(layer.vector_paths[0].id, path_id);
    assert_eq!(layer.tiles.len(), 0);

    assert!(engine.undo());
    assert_eq!(pixel(&engine, 18, 12), [220, 40, 145, 255]);
    assert!(engine.redo());
    assert_eq!(pixel(&engine, 18, 12), [0; 4]);
}

#[test]
fn mixed_raster_mask_and_edited_vector_survive_store_journal_and_exports() {
    let mut engine = blank();
    let layer_id = engine.state().selected;
    engine.brush.size = 10.0;
    engine.brush.hardness = 1.0;
    engine.brush.pressure_size = false;
    engine.brush.pressure_opacity = false;
    engine
        .apply_with_mode(
            &StrokeCommand::BeginStroke {
                stroke_id: StrokeId(1),
                sample: sample(28.0, 38.0, 1),
            },
            StrokeMode::Paint,
        )
        .unwrap();
    engine
        .apply_with_mode(
            &StrokeCommand::CommitStroke {
                stroke_id: StrokeId(1),
                sample: sample(28.0, 38.0, 2),
            },
            StrokeMode::Paint,
        )
        .unwrap();
    let raster_tiles = engine.state().layers[0].tiles.clone();
    assert!(!raster_tiles.is_empty());

    engine.add_layer_mask(layer_id).unwrap();
    engine.brush.color = [0, 0, 0];
    engine
        .apply_with_mode(
            &StrokeCommand::BeginStroke {
                stroke_id: StrokeId(2),
                sample: sample(24.0, 38.0, 3),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();
    engine
        .apply_with_mode(
            &StrokeCommand::UpdateStroke {
                stroke_id: StrokeId(2),
                sample: sample(40.0, 38.0, 4),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();
    engine
        .apply_with_mode(
            &StrokeCommand::CommitStroke {
                stroke_id: StrokeId(2),
                sample: sample(48.0, 38.0, 5),
            },
            StrokeMode::MaskPaint,
        )
        .unwrap();
    let mask_tiles = engine.state().layers[0]
        .mask
        .as_ref()
        .unwrap()
        .tiles
        .clone();
    assert!(!mask_tiles.is_empty());
    assert!(
        mask_tiles
            .values()
            .any(|tile| tile.iter().any(|value| *value < 255))
    );
    assert_eq!(engine.state().layers[0].tiles, raster_tiles);

    let path_id = engine
        .add_vector_path(&[[8.0, 12.0], [50.0, 12.0], [70.0, 28.0]])
        .unwrap();
    engine
        .convert_vector_path_segment_to_cubic(path_id, 0)
        .unwrap();
    let converted = storage::encode(engine.state()).unwrap();
    let before_handle = engine.state().layers[0].vector_paths[0].clone();
    assert!(matches!(
        before_handle.segments[0],
        VectorSegment::CubicTo { .. }
    ));
    engine
        .move_vector_path_handle(path_id, 0, VectorHandle::Outgoing, [22.0, 7.0])
        .unwrap();
    let edited = storage::encode(engine.state()).unwrap();
    assert_eq!(&edited[..8], b"RPLALP03");
    assert_ne!(edited, converted);
    assert_eq!(engine.state().layers[0].tiles, raster_tiles);
    assert_eq!(
        engine.state().layers[0].mask.as_ref().unwrap().tiles,
        mask_tiles
    );

    assert!(engine.undo());
    assert_eq!(storage::encode(engine.state()).unwrap(), converted);
    assert!(engine.redo());
    assert_eq!(storage::encode(engine.state()).unwrap(), edited);

    let root = temp("mixed-raster-mask-vector");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let project_path = root.join("mixed.rpl");
    let mut store = storage::Store::new(project_path.clone());
    store.save(engine.state()).unwrap();
    drop(store);
    let (store, opened_state) = storage::Store::open(project_path.clone()).unwrap();
    let opened = Engine::from_state(opened_state).unwrap();
    assert_eq!(storage::encode(opened.state()).unwrap(), edited);
    assert_eq!(opened.state().layers[0].tiles, raster_tiles);
    assert_eq!(
        opened.state().layers[0].mask.as_ref().unwrap().tiles,
        mask_tiles
    );
    assert_eq!(
        opened.state().layers[0].vector_paths[0],
        engine.state().layers[0].vector_paths[0]
    );
    assert_eq!(opened.rgba(), engine.rgba());
    drop(store);

    let journal_path = root.join("mixed.journal");
    let mut journal = paint_document::journal::Journal::new(journal_path.clone());
    journal.append(engine.state()).unwrap();
    let (recovered_state, damaged_tail) = journal.restore().unwrap();
    assert!(!damaged_tail);
    let recovered = Engine::from_state(recovered_state).unwrap();
    assert_eq!(storage::encode(recovered.state()).unwrap(), edited);
    assert_eq!(recovered.rgba(), engine.rgba());
    drop(journal);
    fs::remove_file(journal_path.with_extension("journal.lock")).unwrap();

    let png_path = root.join("mixed.png");
    storage::export_png(&png_path, &engine).unwrap();
    let mut png_reader =
        png::Decoder::new(std::io::BufReader::new(fs::File::open(&png_path).unwrap()))
            .read_info()
            .unwrap();
    let mut png_pixels = vec![0; png_reader.output_buffer_size().unwrap()];
    let png_info = png_reader.next_frame(&mut png_pixels).unwrap();
    assert_eq!([png_info.width, png_info.height], [96, 64]);
    assert_eq!(&png_pixels[..png_info.buffer_size()], engine.rgba());

    let ora_path = root.join("mixed.ora");
    paint_document::openraster::export(&ora_path, &engine).unwrap();
    let mut archive = zip::ZipArchive::new(fs::File::open(&ora_path).unwrap()).unwrap();
    let mut mimetype = Vec::new();
    archive
        .by_name("mimetype")
        .unwrap()
        .read_to_end(&mut mimetype)
        .unwrap();
    assert_eq!(mimetype, b"image/openraster");
    assert!(archive.by_name("stack.xml").is_ok());
    assert!(archive.by_name("data/layer0.png").is_ok());
    assert_eq!(storage::encode(engine.state()).unwrap(), edited);

    drop(archive);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn paths_can_be_hit_tested_moved_deleted_and_undone_as_transactions() {
    let mut engine = blank();
    let id = add_path(&mut engine);
    assert_eq!(engine.hit_test_vector_path([18.0, 12.0], 1.0), Some(id));
    assert_eq!(engine.hit_test_vector_path([18.0, 30.0], 1.0), None);
    assert_eq!(engine.hit_test_vector_path([f64::NAN, 12.0], 10.0), None);

    let before = engine.rgba();
    engine.translate_vector_path(id, [5.0, 8.0]).unwrap();
    assert_ne!(engine.rgba(), before);
    assert_eq!(engine.hit_test_vector_path([23.0, 20.0], 1.0), Some(id));
    assert!(engine.undo());
    assert_eq!(engine.rgba(), before);
    assert!(engine.redo());
    assert_ne!(engine.rgba(), before);

    engine.remove_vector_path(id).unwrap();
    assert!(engine.state().layers[0].vector_paths.is_empty());
    assert!(engine.undo());
    assert_eq!(engine.state().layers[0].vector_paths[0].id, id);
}

#[test]
fn vector_stroke_style_edits_preserve_geometry_and_roundtrip_through_history() {
    let mut engine = blank();
    let id = add_path(&mut engine);
    let before = storage::encode(engine.state()).unwrap();
    let before_pixels = engine.rgba();
    let before_history = engine.undo_count();

    engine
        .set_vector_path_style(id, [24, 110, 236, 128], 8.0)
        .unwrap();
    let after = storage::encode(engine.state()).unwrap();
    let path = &engine.state().layers[0].vector_paths[0];
    assert_eq!(path.id, id);
    assert_eq!(path.stroke, [24, 110, 236, 128]);
    assert_eq!(path.width, 8.0);
    assert_eq!(path.start, [8.0, 12.0]);
    assert_eq!(path.segments.len(), 2);
    assert_ne!(engine.rgba(), before_pixels);
    assert_eq!(engine.undo_count(), before_history + 1);

    let reopened = Engine::from_state(storage::decode(&after).unwrap()).unwrap();
    assert_eq!(reopened.state().layers[0].vector_paths[0], *path);
    assert_eq!(reopened.rgba(), engine.rgba());

    engine
        .set_vector_path_style(id, [24, 110, 236, 128], 8.0)
        .unwrap();
    assert_eq!(
        engine.undo_count(),
        before_history + 1,
        "no-op style edits add no history"
    );
    assert!(engine.undo());
    assert_eq!(storage::encode(engine.state()).unwrap(), before);
    assert_eq!(engine.rgba(), before_pixels);
    assert!(engine.redo());
    assert_eq!(storage::encode(engine.state()).unwrap(), after);
    assert_eq!(engine.rgba(), reopened.rgba());
}

#[test]
fn vector_style_edit_rejects_invalid_widths_and_locked_or_hidden_owner_layers() {
    let mut engine = blank();
    let id = add_path(&mut engine);
    let before = storage::encode(engine.state()).unwrap();
    let history = engine.undo_count();
    for width in [f64::NAN, f64::INFINITY, 0.0, 512.1] {
        assert!(
            engine
                .set_vector_path_style(id, [0, 0, 0, 255], width)
                .is_err()
        );
        assert_eq!(storage::encode(engine.state()).unwrap(), before);
    }
    assert!(
        engine
            .set_vector_path_style(id + 1, [0, 0, 0, 255], 4.0)
            .is_err()
    );
    assert_eq!(engine.undo_count(), history);

    engine
        .edit(|state| {
            state.layers[0].locked = true;
            Ok(())
        })
        .unwrap();
    let locked_history = engine.undo_count();
    assert!(
        engine
            .set_vector_path_style(id, [0, 0, 0, 255], 4.0)
            .is_err()
    );
    assert_eq!(engine.undo_count(), locked_history);
}

#[test]
fn vector_anchor_edits_preserve_cubic_handle_offsets_and_roundtrip_as_one_transaction() {
    let mut engine = blank();
    let id = add_path(&mut engine);
    engine
        .edit(|state| {
            state.layers[0].vector_paths[0].segments = vec![
                VectorSegment::CubicTo {
                    control1: [16.0, 12.0],
                    control2: [24.0, 20.0],
                    point: [32.0, 24.0],
                },
                VectorSegment::CubicTo {
                    control1: [40.0, 28.0],
                    control2: [48.0, 36.0],
                    point: [56.0, 40.0],
                },
            ];
            Ok(())
        })
        .unwrap();
    let before = storage::encode(engine.state()).unwrap();
    let pixels_before = engine.rgba();
    let history_before = engine.undo_count();

    engine.move_vector_path_anchor(id, 1, [36.0, 30.0]).unwrap();
    let path = &engine.state().layers[0].vector_paths[0];
    assert_eq!(
        path.anchor_points(),
        vec![[8.0, 12.0], [36.0, 30.0], [56.0, 40.0]]
    );
    assert_eq!(
        path.segments,
        vec![
            VectorSegment::CubicTo {
                control1: [16.0, 12.0],
                control2: [28.0, 26.0],
                point: [36.0, 30.0],
            },
            VectorSegment::CubicTo {
                control1: [44.0, 34.0],
                control2: [48.0, 36.0],
                point: [56.0, 40.0],
            },
        ]
    );
    assert_ne!(engine.rgba(), pixels_before);
    assert_eq!(engine.undo_count(), history_before + 1);
    let after = storage::encode(engine.state()).unwrap();
    let reopened = Engine::from_state(storage::decode(&after).unwrap()).unwrap();
    assert_eq!(
        reopened.state().layers[0].vector_paths[0],
        engine.state().layers[0].vector_paths[0]
    );

    assert!(engine.undo());
    assert_eq!(storage::encode(engine.state()).unwrap(), before);
    assert_eq!(engine.rgba(), pixels_before);
    assert!(engine.redo());
    assert_eq!(storage::encode(engine.state()).unwrap(), after);
    assert_eq!(engine.rgba(), reopened.rgba());
}

#[test]
fn vector_anchor_edits_reject_invalid_indices_points_and_locked_owners_atomically() {
    let mut engine = blank();
    let id = add_path(&mut engine);
    let before = storage::encode(engine.state()).unwrap();
    let history = engine.undo_count();

    for (index, point) in [
        (usize::MAX, [20.0, 20.0]),
        (1, [f64::NAN, 20.0]),
        (1, [16_385.0, 20.0]),
    ] {
        assert!(engine.move_vector_path_anchor(id, index, point).is_err());
        assert_eq!(storage::encode(engine.state()).unwrap(), before);
        assert_eq!(engine.undo_count(), history);
    }

    engine
        .edit(|state| {
            state.layers[0].locked = true;
            Ok(())
        })
        .unwrap();
    let locked_state = storage::encode(engine.state()).unwrap();
    assert!(engine.move_vector_path_anchor(id, 1, [20.0, 20.0]).is_err());
    assert_eq!(storage::encode(engine.state()).unwrap(), locked_state);
}

#[test]
fn cubic_handle_edits_preserve_other_geometry_and_roundtrip_as_individual_transactions() {
    let mut engine = blank();
    let id = engine
        .add_vector_path(&[[8.0, 12.0], [40.0, 18.0]])
        .unwrap();
    engine
        .edit(|state| {
            state.layers[0].vector_paths[0].segments = vec![VectorSegment::CubicTo {
                control1: [12.0, 30.0],
                control2: [34.0, -2.0],
                point: [40.0, 18.0],
            }];
            Ok(())
        })
        .unwrap();
    let before = storage::encode(engine.state()).unwrap();
    let pixels_before = engine.rgba();
    let history_before = engine.undo_count();
    assert_eq!(
        engine.state().layers[0].vector_paths[0].control_handle(0, VectorHandle::Outgoing),
        Some([12.0, 30.0])
    );
    assert_eq!(
        engine.state().layers[0].vector_paths[0].control_handle(1, VectorHandle::Incoming),
        Some([34.0, -2.0])
    );
    assert_eq!(
        engine.state().layers[0].vector_paths[0].control_handle(0, VectorHandle::Incoming),
        None
    );

    engine
        .move_vector_path_handle(id, 0, VectorHandle::Outgoing, [18.0, 36.0])
        .unwrap();
    let outgoing = engine.state().layers[0].vector_paths[0].clone();
    assert_eq!(outgoing.start, [8.0, 12.0]);
    assert_eq!(outgoing.anchor_points(), vec![[8.0, 12.0], [40.0, 18.0]]);
    assert_eq!(
        outgoing.segments[0],
        VectorSegment::CubicTo {
            control1: [18.0, 36.0],
            control2: [34.0, -2.0],
            point: [40.0, 18.0],
        }
    );
    assert_ne!(engine.rgba(), pixels_before);
    assert!(engine.state().layers[0].tiles.is_empty());
    assert_eq!(engine.undo_count(), history_before + 1);
    let outgoing_bytes = storage::encode(engine.state()).unwrap();
    let reopened = Engine::from_state(storage::decode(&outgoing_bytes).unwrap()).unwrap();
    assert_eq!(reopened.state().layers[0].vector_paths[0], outgoing);
    assert_eq!(reopened.rgba(), engine.rgba());

    engine
        .move_vector_path_handle(id, 1, VectorHandle::Incoming, [32.0, 4.0])
        .unwrap();
    let after = storage::encode(engine.state()).unwrap();
    assert_eq!(
        engine.state().layers[0].vector_paths[0].control_handle(1, VectorHandle::Incoming),
        Some([32.0, 4.0])
    );
    assert_eq!(engine.undo_count(), history_before + 2);
    assert!(engine.undo());
    assert_eq!(storage::encode(engine.state()).unwrap(), outgoing_bytes);
    assert!(engine.undo());
    assert_eq!(storage::encode(engine.state()).unwrap(), before);
    assert!(engine.redo());
    assert!(engine.redo());
    assert_eq!(storage::encode(engine.state()).unwrap(), after);
}

#[test]
fn cubic_handle_edits_reject_missing_segments_invalid_points_and_locked_layers() {
    let mut engine = blank();
    let id = engine
        .add_vector_path(&[[8.0, 12.0], [40.0, 18.0]])
        .unwrap();
    engine
        .edit(|state| {
            state.layers[0].vector_paths[0].segments[0] = VectorSegment::CubicTo {
                control1: [12.0, 30.0],
                control2: [34.0, -2.0],
                point: [40.0, 18.0],
            };
            Ok(())
        })
        .unwrap();
    let before = storage::encode(engine.state()).unwrap();
    let history = engine.undo_count();
    for (anchor, handle, point) in [
        (usize::MAX, VectorHandle::Incoming, [20.0, 20.0]),
        (0, VectorHandle::Incoming, [20.0, 20.0]),
        (2, VectorHandle::Outgoing, [20.0, 20.0]),
        (0, VectorHandle::Outgoing, [f64::NAN, 20.0]),
        (0, VectorHandle::Outgoing, [16_385.0, 20.0]),
    ] {
        assert!(
            engine
                .move_vector_path_handle(id, anchor, handle, point)
                .is_err()
        );
        assert_eq!(storage::encode(engine.state()).unwrap(), before);
        assert_eq!(engine.undo_count(), history);
    }
    engine
        .edit(|state| {
            state.layers[0].locked = true;
            Ok(())
        })
        .unwrap();
    let locked = storage::encode(engine.state()).unwrap();
    assert!(
        engine
            .move_vector_path_handle(id, 0, VectorHandle::Outgoing, [20.0, 20.0])
            .is_err()
    );
    assert_eq!(storage::encode(engine.state()).unwrap(), locked);
}

#[test]
fn line_segment_can_become_an_editable_cubic_without_changing_its_line() {
    let mut engine = blank();
    let id = engine
        .add_vector_path(&[[8.25, 12.5], [73.75, 42.125], [88.0, 16.0]])
        .unwrap();
    let before = storage::encode(engine.state()).unwrap();
    let pixels_before = engine.rgba();
    let history_before = engine.undo_count();

    engine.convert_vector_path_segment_to_cubic(id, 0).unwrap();
    let path = &engine.state().layers[0].vector_paths[0];
    let VectorSegment::CubicTo {
        control1,
        control2,
        point,
    } = path.segments[0]
    else {
        panic!("the selected line segment should now be cubic")
    };
    assert_eq!(
        control1,
        [8.25 + (73.75 - 8.25) / 3.0, 12.5 + (42.125 - 12.5) / 3.0]
    );
    assert_eq!(
        control2,
        [
            8.25 + (73.75 - 8.25) * (2.0 / 3.0),
            12.5 + (42.125 - 12.5) * (2.0 / 3.0)
        ]
    );
    assert_eq!(point, [73.75, 42.125]);
    assert!(path.control_handle(0, VectorHandle::Outgoing).is_some());
    assert!(path.control_handle(1, VectorHandle::Incoming).is_some());
    assert_eq!(
        engine.rgba(),
        pixels_before,
        "collinear handles preserve the rendered line"
    );
    assert_eq!(engine.undo_count(), history_before + 1);
    let curved = storage::encode(engine.state()).unwrap();
    let reopened = Engine::from_state(storage::decode(&curved).unwrap()).unwrap();
    assert_eq!(reopened.state().layers[0].vector_paths[0], *path);

    assert!(engine.undo());
    assert_eq!(storage::encode(engine.state()).unwrap(), before);
    assert!(engine.redo());
    assert_eq!(storage::encode(engine.state()).unwrap(), curved);
}

#[test]
fn cubic_conversion_refuses_non_lines_bad_indices_and_locked_or_hidden_layers_atomically() {
    let mut engine = blank();
    let id = engine
        .add_vector_path(&[[8.0, 12.0], [40.0, 18.0], [80.0, 30.0]])
        .unwrap();
    engine.convert_vector_path_segment_to_cubic(id, 0).unwrap();
    let before = storage::encode(engine.state()).unwrap();
    let history = engine.undo_count();
    for index in [usize::MAX, 0, 2] {
        assert!(
            engine
                .convert_vector_path_segment_to_cubic(id, index)
                .is_err()
        );
        assert_eq!(storage::encode(engine.state()).unwrap(), before);
        assert_eq!(engine.undo_count(), history);
    }
    engine
        .edit(|state| {
            state.layers[0].locked = true;
            Ok(())
        })
        .unwrap();
    let locked = storage::encode(engine.state()).unwrap();
    assert!(engine.convert_vector_path_segment_to_cubic(id, 1).is_err());
    assert_eq!(storage::encode(engine.state()).unwrap(), locked);

    engine
        .edit(|state| {
            state.layers[0].locked = false;
            state.layers[0].visible = false;
            Ok(())
        })
        .unwrap();
    let hidden = storage::encode(engine.state()).unwrap();
    assert!(engine.convert_vector_path_segment_to_cubic(id, 1).is_err());
    assert_eq!(storage::encode(engine.state()).unwrap(), hidden);
}

#[test]
fn repeated_native_roundtrips_preserve_cubic_f64_coordinates_exactly() {
    let mut engine = blank();
    let id = engine
        .add_vector_path(&[
            [574.7176368375326, 1870.3735881841878],
            [900.3792811643632, 1876.5793504734806],
        ])
        .unwrap();
    engine
        .edit(|state| {
            let path = state.layers[0]
                .vector_paths
                .iter_mut()
                .find(|path| path.id == id)
                .ok_or(paint_document::PaintError::Invalid)?;
            path.segments = vec![
                VectorSegment::CubicTo {
                    control1: [683.1450912250218, 1761.9461337966986],
                    control2: [659.518836690486, 2009.1995970890173],
                    point: [692.4970810777455, 1959.7322305081277],
                },
                VectorSegment::LineTo {
                    point: [900.3792811643632, 1876.5793504734806],
                },
            ];
            Ok(())
        })
        .unwrap();
    let expected = engine.state().layers[0].vector_paths[0].clone();
    let mut encoded = storage::encode(engine.state()).unwrap();

    for cycle in 0..8 {
        let decoded = storage::decode(&encoded).unwrap();
        let restored = decoded.layers[0]
            .vector_paths
            .iter()
            .find(|path| path.id == id)
            .unwrap();
        assert_eq!(
            restored, &expected,
            "vector geometry drifted in cycle {cycle}"
        );
        let next = storage::encode(&decoded).unwrap();
        assert_eq!(next, encoded, "native bytes drifted in cycle {cycle}");
        encoded = next;
    }
}

#[test]
fn line_anchor_insert_delete_and_history_restore_exact_geometry_and_pixels() {
    let mut engine = blank();
    let id = add_path(&mut engine);
    let before = storage::encode(engine.state()).unwrap();
    let pixels_before = engine.rgba();
    let history_before = engine.undo_count();

    let selected = engine.insert_vector_path_anchor(id, 0).unwrap();
    assert_eq!(selected, 1);
    let inserted_path = engine.state().layers[0].vector_paths[0].clone();
    assert_eq!(
        inserted_path.anchor_points(),
        vec![[8.0, 12.0], [16.0, 12.0], [24.0, 12.0], [35.0, 20.0]]
    );
    assert_eq!(engine.rgba(), pixels_before);
    assert_eq!(engine.undo_count(), history_before + 1);
    let inserted = storage::encode(engine.state()).unwrap();
    let reopened = Engine::from_state(storage::decode(&inserted).unwrap()).unwrap();
    assert_eq!(reopened.state().layers[0].vector_paths[0], inserted_path);
    assert_eq!(reopened.rgba(), pixels_before);

    assert!(engine.undo());
    assert_eq!(storage::encode(engine.state()).unwrap(), before);
    assert!(engine.redo());
    assert_eq!(storage::encode(engine.state()).unwrap(), inserted);

    let selected = engine.remove_vector_path_anchor(id, 1).unwrap();
    assert_eq!(selected, 0);
    assert_eq!(storage::encode(engine.state()).unwrap(), before);
    assert_eq!(engine.rgba(), pixels_before);
    assert_eq!(engine.undo_count(), history_before + 2);
    assert!(engine.undo());
    assert_eq!(storage::encode(engine.state()).unwrap(), inserted);
    assert!(engine.redo());
    assert_eq!(storage::encode(engine.state()).unwrap(), before);
}

#[test]
fn cubic_anchor_insertion_splits_the_same_curve_and_curve_interior_delete_is_rejected() {
    use paint_document::vector::VectorPath;

    let mut path =
        VectorPath::from_polyline(1, &[[8.0, 12.0], [56.0, 40.0]], [1, 2, 3, 255], 3.0).unwrap();
    path.segments = vec![VectorSegment::CubicTo {
        control1: [16.0, 60.0],
        control2: [48.0, -8.0],
        point: [56.0, 40.0],
    }];
    let original = path.clone();
    assert_eq!(path.insert_anchor_after(0).unwrap(), 1);
    assert_eq!(path.segments.len(), 2);
    assert!(
        path.segments
            .iter()
            .all(|segment| matches!(segment, VectorSegment::CubicTo { .. }))
    );
    for step in 0..=64 {
        let t = f64::from(step) / 64.0;
        let u = 1.0 - t;
        let point = [
            u.powi(3) * original.start[0]
                + 3.0 * u.powi(2) * t * 16.0
                + 3.0 * u * t.powi(2) * 48.0
                + t.powi(3) * 56.0,
            u.powi(3) * original.start[1]
                + 3.0 * u.powi(2) * t * 60.0
                + 3.0 * u * t.powi(2) * -8.0
                + t.powi(3) * 40.0,
        ];
        assert!(
            paint_document::vector::path_distance(&path, point) <= 0.25,
            "split curve deviates at t={t}: {point:?}"
        );
    }
    let after_split = path.clone();
    assert!(!path.can_remove_anchor(1));
    assert!(path.remove_anchor(1).is_err());
    assert_eq!(path, after_split);
}

#[test]
fn anchor_insert_delete_reject_invalid_indices_minimal_paths_and_locks_atomically() {
    use paint_document::vector::VectorPath;

    let mut path =
        VectorPath::from_polyline(1, &[[8.0, 12.0], [24.0, 12.0]], [1, 2, 3, 255], 3.0).unwrap();
    let before = path.clone();
    assert!(path.insert_anchor_after(usize::MAX).is_err());
    assert!(!path.can_remove_anchor(0));
    assert!(path.remove_anchor(0).is_err());
    assert_eq!(path, before);

    let points: Vec<_> = (0..=paint_document::vector::MAX_VECTOR_SEGMENTS_PER_PATH)
        .map(|index| [index as f64, 4.0])
        .collect();
    let mut capped = VectorPath::from_polyline(2, &points, [1, 2, 3, 255], 3.0).unwrap();
    let capped_before = capped.clone();
    assert!(capped.insert_anchor_after(0).is_err());
    assert_eq!(capped, capped_before);

    let mut engine = blank();
    let id = add_path(&mut engine);
    let before = storage::encode(engine.state()).unwrap();
    let history = engine.undo_count();
    assert!(engine.insert_vector_path_anchor(id, usize::MAX).is_err());
    assert!(engine.remove_vector_path_anchor(id, usize::MAX).is_err());
    assert_eq!(storage::encode(engine.state()).unwrap(), before);
    assert_eq!(engine.undo_count(), history);
    engine
        .edit(|state| {
            state.layers[0].locked = true;
            Ok(())
        })
        .unwrap();
    let locked = storage::encode(engine.state()).unwrap();
    assert!(engine.insert_vector_path_anchor(id, 0).is_err());
    assert!(engine.remove_vector_path_anchor(id, 1).is_err());
    assert_eq!(storage::encode(engine.state()).unwrap(), locked);
}

#[test]
fn uniform_path_transform_moves_cubic_geometry_and_handles_together() {
    use paint_document::vector::VectorPath;

    let mut path =
        VectorPath::from_polyline(1, &[[20.0, 20.0], [40.0, 20.0]], [40, 90, 220, 255], 3.0)
            .unwrap();
    path.segments = vec![VectorSegment::CubicTo {
        control1: [25.0, 25.0],
        control2: [35.0, 15.0],
        point: [40.0, 20.0],
    }];

    path.transform_uniform(2.0, 90.0).unwrap();

    assert_eq!(path.start, [30.0, 0.0]);
    assert_eq!(
        path.segments,
        vec![VectorSegment::CubicTo {
            control1: [20.0, 10.0],
            control2: [40.0, 30.0],
            point: [30.0, 40.0],
        }]
    );
    assert_eq!(path.width, 6.0);
    assert!(path.validate().is_ok());
}

#[test]
fn vector_transform_and_duplicate_are_bounded_native_undoable_transactions() {
    let mut engine = blank();
    let layer_id = engine.state().selected;
    engine.brush.size = 8.0;
    engine.brush.hardness = 1.0;
    engine.brush.pressure_size = false;
    engine.brush.pressure_opacity = false;
    engine
        .apply_with_mode(
            &StrokeCommand::BeginStroke {
                stroke_id: StrokeId(80),
                sample: sample(45.0, 48.0, 1),
            },
            StrokeMode::Paint,
        )
        .unwrap();
    engine
        .apply_with_mode(
            &StrokeCommand::CommitStroke {
                stroke_id: StrokeId(80),
                sample: sample(45.0, 48.0, 2),
            },
            StrokeMode::Paint,
        )
        .unwrap();
    engine.add_layer_mask(layer_id).unwrap();
    let path_id = engine
        .add_vector_path(&[[24.0, 20.0], [48.0, 24.0], [68.0, 38.0]])
        .unwrap();
    let raster_before = engine.state().layers[0].tiles.clone();
    let mask_before = engine.state().layers[0].mask.clone();
    let before_transform = storage::encode(engine.state()).unwrap();
    let history_before = engine.undo_count();

    engine.transform_vector_path(path_id, 1.1, 15.0).unwrap();
    let transformed = storage::encode(engine.state()).unwrap();
    let transformed_path = engine.state().layers[0].vector_paths[0].clone();
    assert_ne!(transformed_path.start, [24.0, 20.0]);
    assert!((transformed_path.width - 8.8).abs() < 1.0e-9);
    assert_eq!(engine.state().layers[0].tiles, raster_before);
    assert_eq!(engine.state().layers[0].mask, mask_before);
    assert_eq!(engine.undo_count(), history_before + 1);

    let reopened = Engine::from_state(storage::decode(&transformed).unwrap()).unwrap();
    assert_eq!(reopened.state().layers[0].vector_paths[0], transformed_path);
    assert_eq!(reopened.rgba(), engine.rgba());
    assert_eq!(reopened.state().layers[0].tiles, raster_before);
    assert_eq!(reopened.state().layers[0].mask, mask_before);

    assert!(engine.undo());
    assert_eq!(storage::encode(engine.state()).unwrap(), before_transform);
    assert!(engine.redo());
    assert_eq!(storage::encode(engine.state()).unwrap(), transformed);

    let before_duplicate = storage::encode(engine.state()).unwrap();
    let before_duplicate_count = engine.undo_count();
    let duplicate_id = engine.duplicate_vector_path(path_id).unwrap();
    let duplicated = storage::encode(engine.state()).unwrap();
    let paths = &engine.state().layers[0].vector_paths;
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0].id, path_id);
    assert_eq!(paths[1].id, duplicate_id);
    assert_ne!(paths[0].start, paths[1].start);
    assert_eq!(engine.state().layers[0].tiles, raster_before);
    assert_eq!(engine.state().layers[0].mask, mask_before);
    assert_eq!(engine.undo_count(), before_duplicate_count + 1);
    let reopened = Engine::from_state(storage::decode(&duplicated).unwrap()).unwrap();
    assert_eq!(reopened.state().layers[0].vector_paths, *paths);

    assert!(engine.undo());
    assert_eq!(storage::encode(engine.state()).unwrap(), before_duplicate);
    assert!(engine.redo());
    assert_eq!(storage::encode(engine.state()).unwrap(), duplicated);
}

#[test]
fn vector_transforms_reject_invalid_bounds_and_locked_layers_without_history() {
    let mut engine = blank();
    let id = engine.add_vector_path(&[[1.0, 1.0], [30.0, 1.0]]).unwrap();
    let before = storage::encode(engine.state()).unwrap();
    let history = engine.undo_count();
    for (scale, rotation) in [
        (f64::NAN, 0.0),
        (0.0, 0.0),
        (1.0, f64::INFINITY),
        (1.0, 361.0),
        (1.5, 0.0),
        (10.0, 0.0),
    ] {
        assert!(engine.transform_vector_path(id, scale, rotation).is_err());
        assert_eq!(storage::encode(engine.state()).unwrap(), before);
        assert_eq!(engine.undo_count(), history);
    }
    engine
        .edit(|state| {
            state.layers[0].locked = true;
            Ok(())
        })
        .unwrap();
    let locked = storage::encode(engine.state()).unwrap();
    let locked_history = engine.undo_count();
    assert!(engine.transform_vector_path(id, 1.1, 15.0).is_err());
    assert!(engine.duplicate_vector_path(id).is_err());
    assert_eq!(storage::encode(engine.state()).unwrap(), locked);
    assert_eq!(engine.undo_count(), locked_history);
}

#[test]
fn vector_duplication_respects_the_document_object_cap_atomically() {
    use paint_document::vector::{MAX_VECTOR_PATHS, VectorPath};

    let mut engine = blank();
    let template =
        VectorPath::from_polyline(1, &[[20.0, 20.0], [40.0, 24.0]], [220, 40, 145, 255], 3.0)
            .unwrap();
    engine
        .edit(|state| {
            state.layers[0].vector_paths = (1..=MAX_VECTOR_PATHS as u64)
                .map(|id| {
                    let mut path = template.clone();
                    path.id = id;
                    path
                })
                .collect();
            state.next_vector_id = MAX_VECTOR_PATHS as u64 + 1;
            Ok(())
        })
        .unwrap();
    let before = storage::encode(engine.state()).unwrap();
    let history = engine.undo_count();

    assert!(matches!(
        engine.duplicate_vector_path(1),
        Err(paint_document::PaintError::Limit)
    ));
    assert_eq!(storage::encode(engine.state()).unwrap(), before);
    assert_eq!(engine.undo_count(), history);
}

#[test]
fn layer_duplication_remaps_vector_ids_and_preserves_editable_order() {
    let mut engine = blank();
    let first_id = add_path(&mut engine);
    let before = storage::encode(engine.state()).unwrap();

    engine.duplicate_layer().unwrap();

    let state = engine.state();
    assert_eq!(state.layers.len(), 2);
    let original_id = state.layers[0].vector_paths[0].id;
    let copy_id = state.layers[1].vector_paths[0].id;
    assert_eq!(original_id, first_id);
    assert_ne!(copy_id, original_id);
    assert_eq!(state.next_vector_id, copy_id + 1);
    assert_eq!(
        engine.hit_test_vector_path([18.0, 12.0], 1.0),
        Some(copy_id)
    );
    let after = storage::encode(state).unwrap();
    assert_ne!(before, after);

    assert!(engine.undo());
    assert_eq!(storage::encode(engine.state()).unwrap(), before);
    assert!(engine.redo());
    assert_eq!(storage::encode(engine.state()).unwrap(), after);
}

#[test]
fn vector_paths_export_to_png_and_openraster_as_pixels_but_remain_native_objects() {
    let mut engine = blank();
    add_path(&mut engine);
    let root = temp("exports");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let png_path = root.join("path.png");
    storage::export_png(&png_path, &engine).unwrap();
    let mut png_reader =
        png::Decoder::new(std::io::BufReader::new(fs::File::open(&png_path).unwrap()))
            .read_info()
            .unwrap();
    let mut png_pixels = vec![0; png_reader.output_buffer_size().unwrap()];
    let png_info = png_reader.next_frame(&mut png_pixels).unwrap();
    let png_stride = png_info.width as usize * 4;
    assert_eq!(
        &png_pixels[12 * png_stride + 18 * 4..][..4],
        &[220, 40, 145, 255]
    );

    let ora_path = root.join("path.ora");
    paint_document::openraster::export(&ora_path, &engine).unwrap();
    let mut archive = zip::ZipArchive::new(fs::File::open(&ora_path).unwrap()).unwrap();
    let mut layer_png = Vec::new();
    archive
        .by_name("data/layer0.png")
        .unwrap()
        .read_to_end(&mut layer_png)
        .unwrap();
    let mut ora_reader = png::Decoder::new(std::io::Cursor::new(layer_png))
        .read_info()
        .unwrap();
    let mut ora_pixels = vec![0; ora_reader.output_buffer_size().unwrap()];
    let ora_info = ora_reader.next_frame(&mut ora_pixels).unwrap();
    assert_eq!(ora_info.width, 96);
    assert_eq!(
        &ora_pixels[12 * png_stride + 18 * 4..][..4],
        &[220, 40, 145, 255]
    );
    assert_eq!(engine.state().layers[0].vector_paths.len(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn vectors_reject_downgraded_or_corrupt_native_records() {
    let mut engine = blank();
    add_path(&mut engine);
    let valid = storage::encode(engine.state()).unwrap();

    let mut downgraded = valid.clone();
    downgraded[..8].copy_from_slice(b"RPLALP02");
    reseal(&mut downgraded);
    assert!(storage::decode(&downgraded).is_err());

    let mut corrupt = valid;
    corrupt[20] ^= 0x01;
    assert!(storage::decode(&corrupt).is_err());
}

#[test]
fn cubic_metadata_and_path_limits_are_validated() {
    let mut engine = blank();
    let id = add_path(&mut engine);
    engine
        .edit(|state| {
            state.layers[0].vector_paths[0].segments[0] = VectorSegment::CubicTo {
                control1: [14.0, 4.0],
                control2: [18.0, 24.0],
                point: [24.0, 12.0],
            };
            Ok(())
        })
        .unwrap();
    let reopened =
        Engine::from_state(storage::decode(&storage::encode(engine.state()).unwrap()).unwrap())
            .unwrap();
    assert!(matches!(
        reopened.state().layers[0].vector_paths[0].segments[0],
        VectorSegment::CubicTo { .. }
    ));
    assert_eq!(reopened.hit_test_vector_path([16.0, 11.0], 2.0), Some(id));

    let mut invalid = reopened.state().clone();
    invalid.layers[0].vector_paths[0].width = f64::INFINITY;
    assert!(invalid.validate().is_err());
    invalid.layers[0].vector_paths[0].width = 3.0;
    invalid.layers[0].vector_paths[0].start[0] = f64::NAN;
    assert!(invalid.validate().is_err());
}

#[test]
fn native_vector_store_save_and_reopen_preserves_editable_paths() {
    let mut engine = blank();
    add_path(&mut engine);
    let root = temp("store");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let path = root.join("editable.rpl");
    let mut store = storage::Store::new(path.clone());
    store.save(engine.state()).unwrap();
    let (_opened_store, state) = storage::Store::open(path.clone()).unwrap();
    let reopened = Engine::from_state(state).unwrap();
    assert_eq!(
        reopened.state().layers[0].vector_paths,
        engine.state().layers[0].vector_paths
    );
    assert_eq!(reopened.rgba(), engine.rgba());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn journal_recovery_preserves_editable_paths_and_monotonic_ids() {
    let mut engine = blank();
    let path_id = add_path(&mut engine);
    let journal_path = temp("recovery.journal");
    let _ = fs::remove_file(&journal_path);
    let _ = fs::remove_file(journal_path.with_extension("journal.lock"));

    let mut journal = paint_document::journal::Journal::new(journal_path.clone());
    journal.append(engine.state()).unwrap();
    let (restored, damaged_tail) = journal.restore().unwrap();

    assert!(!damaged_tail);
    assert_eq!(restored.next_vector_id, path_id + 1);
    assert_eq!(restored.layers[0].vector_paths[0].id, path_id);
    let reopened = Engine::from_state(restored).unwrap();
    assert_eq!(pixel(&reopened, 18, 12), [220, 40, 145, 255]);

    drop(journal);
    fs::remove_file(&journal_path).unwrap();
    fs::remove_file(journal_path.with_extension("journal.lock")).unwrap();
}

#[test]
fn vector_ids_are_monotonic_across_delete_save_and_reopen() {
    let mut engine = blank();
    let first = add_path(&mut engine);
    engine.remove_vector_path(first).unwrap();
    let encoded = storage::encode(engine.state()).unwrap();
    assert_eq!(&encoded[..8], b"RPLALP03");
    let mut reopened = Engine::from_state(storage::decode(&encoded).unwrap()).unwrap();
    let second = add_path(&mut reopened);
    assert!(second > first);
}
