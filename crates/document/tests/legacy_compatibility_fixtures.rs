use paint_document::{
    Engine, State,
    brush::{Brush, BrushKind},
    journal::Journal,
    recovery,
    storage::{self, Store},
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const V1: &[u8] =
    include_bytes!("fixtures/bp_compat/historical-serializer-generated/project-v1.rpl");
const V2: &[u8] =
    include_bytes!("fixtures/bp_compat/historical-serializer-generated/project-v2.rpl");
const V3: &[u8] =
    include_bytes!("fixtures/bp_compat/historical-serializer-generated/project-v3.rpl");
const V4: &[u8] =
    include_bytes!("fixtures/bp_compat/historical-serializer-generated/project-v4.rpl");
const BRUSH_V1: &[u8] =
    include_bytes!("fixtures/bp_compat/historical-serializer-generated/brush-preset-v1.rplbrush");

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn temporary_directory(label: &str) -> PathBuf {
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "bp-legacy-fixture-{}-{id}-{label}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn raw_pixel(state: &State, layer_index: usize, x: u32, y: u32) -> [u8; 4] {
    let tile = &state.layers[layer_index].tiles[&(x / 64, y / 64)];
    let offset = (((y % 64) * 64 + x % 64) * 4) as usize;
    tile[offset..offset + 4].try_into().unwrap()
}

fn composite_pixel(state: &State, x: u32, y: u32) -> [u8; 4] {
    let rgba = Engine::from_state(state.clone()).unwrap().rgba();
    let offset = ((y * state.width + x) * 4) as usize;
    rgba[offset..offset + 4].try_into().unwrap()
}

fn decode_project(bytes: &[u8], magic: &[u8; 8], expected_sha: &str) -> State {
    assert_eq!(&bytes[..8], magic);
    assert_eq!(sha256(bytes), expected_sha);
    let state = storage::decode(bytes).expect("historical serializer output decodes");
    assert_eq!([state.width, state.height, state.dpi], [64, 64, 300]);
    assert_eq!(state.layers.len(), 2);
    assert_eq!(state.layers[0].name, "Raster base");
    assert_eq!(state.layers[1].name, "Raster overlay");
    assert_eq!(state.background, None);
    assert_eq!(raw_pixel(&state, 0, 8, 12), [255, 0, 0, 255]);
    assert_eq!(raw_pixel(&state, 0, 20, 12), [0, 0, 255, 255]);
    assert_eq!(raw_pixel(&state, 0, 32, 12), [64, 0, 64, 128]);
    assert_eq!(raw_pixel(&state, 0, 40, 12), [0, 0, 0, 0]);
    assert_eq!(raw_pixel(&state, 1, 48, 12), [0, 160, 0, 160]);
    assert!(state.validate().is_ok());
    state
}

#[test]
fn historical_serializer_samples_reopen_with_their_versioned_artwork_features() {
    let v1 = decode_project(
        V1,
        b"RPLALP01",
        "c6f22b7e149953cf68f112d7eb95ad184f886a266c6b427125bb5ba01a85a3de",
    );
    assert!(v1.layers.iter().all(|layer| layer.mask.is_none()));
    assert!(v1.layers.iter().all(|layer| layer.vector_paths.is_empty()));
    assert_eq!(composite_pixel(&v1, 8, 12), [255, 0, 0, 255]);
    assert_eq!(composite_pixel(&v1, 20, 12), [0, 0, 255, 255]);
    assert_eq!(composite_pixel(&v1, 32, 12), [128, 0, 128, 128]);
    assert_eq!(composite_pixel(&v1, 40, 12), [0, 0, 0, 0]);
    assert_eq!(composite_pixel(&v1, 48, 12), [0, 255, 0, 160]);

    let v2 = decode_project(
        V2,
        b"RPLALP02",
        "c7d5aac9582a38d713478162acfa903a66cb51dc37202c3b02e82944b2339053",
    );
    let mask = v2.layers[0].mask.as_ref().unwrap();
    assert!(mask.enabled);
    assert_eq!(mask.tiles[&(0, 0)][12 * 64 + 8], 128);
    assert_eq!(mask.tiles[&(0, 0)][12 * 64 + 32], 0);
    assert!(v2.layers.iter().all(|layer| layer.vector_paths.is_empty()));
    assert_eq!(composite_pixel(&v2, 8, 12), [255, 0, 0, 128]);
    assert_eq!(composite_pixel(&v2, 32, 12), [0, 0, 0, 0]);
    assert_eq!(composite_pixel(&v2, 20, 12), [0, 0, 255, 255]);

    let v3 = decode_project(
        V3,
        b"RPLALP03",
        "c5e34f45c1df2dc9945e33c95b282de374cce588ed336cfaf225340379ce28ff",
    );
    assert!(v3.layers[0].mask.is_some());
    assert_eq!(v3.layers[1].vector_paths.len(), 1);
    assert_eq!(v3.layers[1].vector_paths[0].start, [16.0, 50.0]);
    assert_eq!(v3.layers[1].vector_paths[0].fill, None);
    assert_eq!(composite_pixel(&v3, 32, 50), [35, 65, 180, 255]);

    let v4 = decode_project(
        V4,
        b"RPLALP04",
        "bf1abaa5f02a468acb22a017e1c92f9ed399000005d058510317be479ae03ecd",
    );
    assert!(v4.layers[0].mask.is_some());
    assert_eq!(v4.layers[1].vector_paths.len(), 1);
    assert_eq!(v4.layers[1].vector_paths[0].fill, Some([220, 40, 145, 255]));
    assert_eq!(composite_pixel(&v4, 32, 23), [220, 40, 145, 255]);
    assert_eq!(composite_pixel(&v4, 2, 23), [0, 0, 0, 0]);

    for state in [v1, v2, v3, v4] {
        let reopened = Engine::from_state(state.clone()).unwrap();
        assert_eq!(reopened.state(), &state);
        assert_eq!(reopened.rgba().len(), (64 * 64 * 4) as usize);
    }
}

#[test]
fn historical_rpl_files_save_in_place_keep_original_backup_and_recover() {
    let projects = [
        (V1, b"RPLALP01" as &[u8; 8]),
        (V2, b"RPLALP02" as &[u8; 8]),
        (V3, b"RPLALP03" as &[u8; 8]),
        (V4, b"RPLALP04" as &[u8; 8]),
    ];
    for (index, (original, magic)) in projects.into_iter().enumerate() {
        let directory = temporary_directory(&format!("in-place-{index}"));
        let path = directory.join("legacy-artwork.rpl");
        fs::write(&path, original).unwrap();

        let (mut store, original_state) = Store::open(path.clone()).unwrap();
        let mut edited = original_state.clone();
        edited.layers[0].name = "Edited in place".into();
        store.save(&edited).unwrap();

        let backup = path.with_extension("rpl.bak");
        assert_eq!(fs::read(&backup).unwrap(), original);
        let reopened = Store::open(path.clone()).unwrap().1;
        assert_eq!(&fs::read(&path).unwrap()[..8], magic);
        assert_eq!(reopened, edited);

        // Exercise verified backup fallback using the genuine historical-
        // serializer bytes as the backup payload. Recovery must not rewrite
        // either the damaged primary or the backup source.
        let damaged = b"damaged primary snapshot";
        fs::write(&path, damaged).unwrap();
        let (recovered, used_backup) = recovery::restore(&path).unwrap();
        assert!(used_backup);
        assert_eq!(recovered, original_state);
        assert_eq!(fs::read(&path).unwrap(), damaged);
        assert_eq!(fs::read(&backup).unwrap(), original);

        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn historical_project_states_are_checkpoints_in_the_current_recovery_journal() {
    for (index, bytes) in [V1, V2, V3, V4].into_iter().enumerate() {
        let state = storage::decode(bytes).unwrap();
        let directory = temporary_directory(&format!("journal-{index}"));
        let journal_path = directory.join("legacy-project.rpl.journal");
        let mut journal = Journal::new(journal_path.clone());
        journal.append(&state).unwrap();
        assert_eq!(&fs::read(&journal_path).unwrap()[..8], b"RPLJRN01");
        let (restored, fell_back) = journal.restore().unwrap();
        assert!(!fell_back);
        assert_eq!(restored, state);

        // Recovery's current writer records the decoded state in both its
        // journal and snapshot. This is a runtime derivative, not a claimed
        // historical recovery file.
        let recovery_dir = directory.join("recovery");
        let recovery_writer = recovery::Recovery::start(&recovery_dir).unwrap();
        recovery_writer.flush(state.clone()).unwrap();
        let snapshot = recovery_writer.path.clone();
        drop(recovery_writer);
        let (recovered, used_backup) = recovery::restore(&snapshot).unwrap();
        assert!(!used_backup);
        assert_eq!(recovered, state);

        fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn historical_serializer_brush_preset_loads_without_modifying_its_bytes() {
    assert_eq!(
        sha256(BRUSH_V1),
        "83f3e4f23e8d8dfd99cc3671c1566a3af1db929274f944d6087ba7b798860bd2"
    );
    let text = std::str::from_utf8(BRUSH_V1).unwrap();
    let decoded = Brush::decode(text).unwrap();
    assert_eq!(decoded.kind, BrushKind::Ink);
    assert_eq!(decoded.size, 9.0);
    assert_eq!(decoded.opacity, 0.65);
    assert_eq!(decoded.flow, 0.4);
    assert_eq!(decoded.color, [9, 101, 233]);
    assert!(!decoded.pressure_size);
    assert!(decoded.pressure_opacity);

    let directory = temporary_directory("brush");
    let path = directory.join("historical-preset.rplbrush");
    fs::write(&path, BRUSH_V1).unwrap();
    assert_eq!(storage::load_brush_preset(&path).unwrap(), decoded);
    assert_eq!(fs::read(&path).unwrap(), BRUSH_V1);
    fs::remove_dir_all(directory).unwrap();
}
