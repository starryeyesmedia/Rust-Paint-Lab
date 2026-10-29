use paint_document::{Engine, storage, vector::VectorSegment};
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
