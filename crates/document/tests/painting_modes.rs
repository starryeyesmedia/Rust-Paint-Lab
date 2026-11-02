use paint_document::{
    Blend, Engine, Layer, State, StrokeMode, TILE, TILE_BYTES,
    brush::{Brush, BrushKind},
    editing::{Selection, SelectionKind},
    storage::Store,
};
use paint_input::{Buttons, Tool};
use paint_stroke::{StrokeCommand as C, StrokeId, StrokeSample, coordinates::DocumentPoint};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

const WIDTH: u32 = 128;
const HEIGHT: u32 = 64;
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const HALF_MAGENTA_PM: [u8; 4] = [80, 0, 80, 128];
const SECOND_LAYER_PIXEL: [u8; 4] = [0, 180, 0, 180];
const FOREGROUND: [u8; 3] = [255, 255, 0];
const START: [f64; 2] = [12.5, 32.5];
const END: [f64; 2] = [60.5, 32.5];

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug)]
enum TestMode {
    Brush,
    Smudge,
    Mixer { strength: f64 },
}

impl TestMode {
    fn stroke_mode(self) -> StrokeMode {
        match self {
            Self::Brush => StrokeMode::Paint,
            Self::Smudge => StrokeMode::Smudge,
            Self::Mixer { strength } => StrokeMode::ColorMix { strength },
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Brush => "brush",
            Self::Smudge => "smudge",
            Self::Mixer { .. } => "mixer",
        }
    }
}

fn sample(x: f64, y: f64, pressure: Option<f32>, time_ms: u64) -> StrokeSample {
    StrokeSample {
        source_counter: None,
        pointer_id: 7,
        tool: Tool::Pen,
        position_client_px: [x, y],
        position_document: DocumentPoint([x, y]),
        pressure,
        tilt_degrees: [Some(0.0), None],
        buttons: Buttons::default(),
        source_time_ms: time_ms,
        time_ms,
    }
}

fn pixel(tile: &mut [u8], x: u32, y: u32, rgba_pm: [u8; 4]) {
    let offset = ((y % TILE * TILE + x % TILE) * 4) as usize;
    tile[offset..offset + 4].copy_from_slice(&rgba_pm);
}

fn set_pixel(state: &mut State, layer_index: usize, x: u32, y: u32, rgba_pm: [u8; 4]) {
    let key = (x / TILE, y / TILE);
    let tile = state.layers[layer_index]
        .tiles
        .entry(key)
        .or_insert_with(|| Arc::new(vec![0; TILE_BYTES]));
    pixel(Arc::make_mut(tile).as_mut_slice(), x, y, rgba_pm);
}

/// Same sparse RGBA state for all modes: adjacent opaque red/blue, a
/// premultiplied half-alpha magenta patch, transparent canvas, and a second
/// populated layer well outside the stroke.
fn fixture() -> State {
    let mut state = State::new(WIDTH, HEIGHT, 300).unwrap();
    state.background = None;
    state.brush = Brush {
        kind: BrushKind::Round,
        size: 10.0,
        opacity: 0.75,
        flow: 0.5,
        hardness: 1.0,
        spacing: 0.2,
        pressure_size: false,
        pressure_opacity: true,
        tilt: false,
        angle: 0.0,
        smoothing: 0.0,
        color: FOREGROUND,
    };

    state.layers.push(Layer {
        id: 2,
        name: "Untouched green layer".into(),
        visible: true,
        locked: false,
        opacity: 255,
        blend: Blend::Normal,
        mask: None,
        vector_paths: Vec::new(),
        tiles: Default::default(),
    });
    for y in 24..40 {
        for x in 8..24 {
            set_pixel(&mut state, 0, x, y, RED);
        }
        for x in 24..40 {
            set_pixel(&mut state, 0, x, y, BLUE);
        }
        for x in 40..48 {
            set_pixel(&mut state, 0, x, y, HALF_MAGENTA_PM);
        }
        for x in 96..112 {
            set_pixel(&mut state, 1, x, y, SECOND_LAYER_PIXEL);
        }
    }
    state.validate().unwrap();
    state
}

fn engine_from(state: &State) -> Engine {
    Engine::from_state(state.clone()).unwrap()
}

fn apply(
    engine: &mut Engine,
    mode: TestMode,
    id: u64,
    start: [f64; 2],
    end: [f64; 2],
    pressure: Option<f32>,
) {
    let stroke_mode = mode.stroke_mode();
    engine
        .apply_with_mode(
            &C::BeginStroke {
                stroke_id: StrokeId(id),
                sample: sample(start[0], start[1], pressure, 0),
            },
            stroke_mode,
        )
        .unwrap();
    engine
        .apply_with_mode(
            &C::CommitStroke {
                stroke_id: StrokeId(id),
                sample: sample(end[0], end[1], pressure, 1),
            },
            stroke_mode,
        )
        .unwrap();
}

fn apply_dot(
    state: &State,
    mode: TestMode,
    opacity: f64,
    flow: f64,
    pressure: Option<f32>,
) -> Engine {
    apply_dot_at(state, mode, opacity, flow, pressure, [20.5, 32.5])
}

fn apply_dot_at(
    state: &State,
    mode: TestMode,
    opacity: f64,
    flow: f64,
    pressure: Option<f32>,
    position: [f64; 2],
) -> Engine {
    let mut engine = engine_from(state);
    engine.brush.opacity = opacity;
    engine.brush.flow = flow;
    apply(&mut engine, mode, 10, position, position, pressure);
    engine
}

fn rgba_at(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * WIDTH + x) * 4) as usize;
    rgba[offset..offset + 4].try_into().unwrap()
}

fn raw_at(state: &State, layer_index: usize, x: u32, y: u32) -> [u8; 4] {
    let key = (x / TILE, y / TILE);
    let offset = ((y % TILE * TILE + x % TILE) * 4) as usize;
    state.layers[layer_index]
        .tiles
        .get(&key)
        .map_or([0; 4], |tile| tile[offset..offset + 4].try_into().unwrap())
}

fn assert_premultiplied(state: &State) {
    for layer in &state.layers {
        for tile in layer.tiles.values() {
            for px in tile.chunks_exact(4) {
                assert!(
                    px[0] <= px[3] && px[1] <= px[3] && px[2] <= px[3],
                    "stored tile pixel must be premultiplied RGBA, got {px:?}"
                );
                if px[3] == 0 {
                    assert_eq!(px, [0, 0, 0, 0], "transparent pixels have zero pigment");
                }
            }
        }
    }
}

#[test]
fn identical_fixture_produces_differential_brush_smudge_and_mixer_pixels() {
    let initial = fixture();
    let initial_rgba = engine_from(&initial).rgba();
    assert_eq!(rgba_at(&initial_rgba, 16, 32), RED);
    assert_eq!(rgba_at(&initial_rgba, 31, 32), BLUE);
    assert_eq!(rgba_at(&initial_rgba, 44, 32), [159, 0, 159, 128]);
    assert_eq!(rgba_at(&initial_rgba, 72, 32), [0, 0, 0, 0]);

    let mut brush = engine_from(&initial);
    let mut smudge = engine_from(&initial);
    let mut mixer = engine_from(&initial);
    let modes = [
        TestMode::Brush,
        TestMode::Smudge,
        TestMode::Mixer { strength: 0.5 },
    ];
    for (engine, mode) in [
        (&mut brush, modes[0]),
        (&mut smudge, modes[1]),
        (&mut mixer, modes[2]),
    ] {
        apply(engine, mode, 1, START, END, Some(1.0));
        assert_premultiplied(engine.state());
    }

    let brush_pixels = brush.rgba();
    let smudge_pixels = smudge.rgba();
    let mixer_pixels = mixer.rgba();
    assert_ne!(
        brush_pixels, smudge_pixels,
        "Smudge must redistribute fixture pigment, not reproduce Brush output"
    );
    assert_ne!(
        brush_pixels, mixer_pixels,
        "Color Mixer must blend sampled pigment, not reproduce Brush output"
    );
    assert_ne!(
        smudge_pixels, mixer_pixels,
        "Color Mixer and Smudge must produce different RGBA output"
    );

    assert_eq!(rgba_at(&brush_pixels, 20, 32), [253, 182, 0, 255]);
    assert_eq!(rgba_at(&smudge_pixels, 58, 32), [99, 0, 191, 44]);
    assert_eq!(raw_at(smudge.state(), 0, 58, 32), [17, 0, 33, 44]);
    assert_eq!(rgba_at(&mixer_pixels, 20, 32), [255, 161, 0, 255]);
    assert_eq!(rgba_at(&mixer_pixels, 31, 32), [141, 141, 106, 255]);
    assert_eq!(
        rgba_at(&smudge_pixels, 72, 32),
        [0, 0, 0, 0],
        "the stroke must stop before untouched transparent pixels"
    );
    let picked_up = rgba_at(&smudge_pixels, 58, 32);
    assert!(
        picked_up[3] > 0,
        "Smudge should carry source pigment into transparent pixels"
    );
    assert!(
        picked_up[0].max(picked_up[2]) > picked_up[1],
        "Smudge carries source pigments instead of the yellow foreground, got {picked_up:?}"
    );
    assert_ne!(
        rgba_at(&mixer_pixels, 20, 32),
        rgba_at(&brush_pixels, 20, 32)
    );
    assert_ne!(
        rgba_at(&mixer_pixels, 31, 32),
        rgba_at(&brush_pixels, 31, 32)
    );
    assert_eq!(raw_at(brush.state(), 1, 100, 32), SECOND_LAYER_PIXEL);
    assert_eq!(raw_at(smudge.state(), 1, 100, 32), SECOND_LAYER_PIXEL);
    assert_eq!(raw_at(mixer.state(), 1, 100, 32), SECOND_LAYER_PIXEL);
}

#[test]
fn mixer_strength_loading_and_pressure_have_defined_pixel_effects() {
    let initial = fixture();
    let mix = |strength, flow, pressure| {
        apply_dot(&initial, TestMode::Mixer { strength }, 1.0, flow, pressure)
    };

    let quarter = mix(0.25, 1.0, Some(1.0));
    let half = mix(0.5, 1.0, Some(1.0));
    let three_quarters = mix(0.75, 1.0, Some(1.0));
    assert_eq!(rgba_at(&quarter.rgba(), 20, 32), [255, 112, 0, 255]);
    assert_eq!(rgba_at(&half.rgba(), 20, 32), [255, 192, 0, 255]);
    assert_eq!(rgba_at(&three_quarters.rgba(), 20, 32), [255, 239, 0, 255]);
    let mixed_partial = apply_dot_at(
        &initial,
        TestMode::Mixer { strength: 0.5 },
        1.0,
        1.0,
        Some(1.0),
        [44.5, 32.5],
    );
    let mixed_transparent = apply_dot_at(
        &initial,
        TestMode::Mixer { strength: 0.5 },
        1.0,
        1.0,
        Some(1.0),
        [60.5, 32.5],
    );
    assert_eq!(rgba_at(&mixed_partial.rgba(), 44, 32), [231, 191, 40, 255]);
    assert_eq!(
        rgba_at(&mixed_transparent.rgba(), 60, 32),
        [255, 255, 0, 255]
    );
    assert_ne!(quarter.rgba(), half.rgba());
    assert_ne!(half.rgba(), three_quarters.rgba());

    let no_load = mix(0.5, 0.0, Some(1.0));
    let half_load = mix(0.5, 0.5, Some(1.0));
    let full_load = mix(0.5, 1.0, Some(1.0));
    assert_eq!(no_load.rgba(), engine_from(&initial).rgba());
    assert!(rgba_at(&half_load.rgba(), 20, 32)[1] > rgba_at(&no_load.rgba(), 20, 32)[1]);
    assert!(rgba_at(&full_load.rgba(), 20, 32)[1] > rgba_at(&half_load.rgba(), 20, 32)[1]);

    let soft_pressure = mix(1.0, 1.0, Some(0.25));
    let medium_pressure = mix(1.0, 1.0, Some(0.5));
    let firm_pressure = mix(1.0, 1.0, Some(1.0));
    let unavailable_pressure = mix(1.0, 1.0, None);
    assert_eq!(
        firm_pressure.rgba(),
        unavailable_pressure.rgba(),
        "unavailable pressure uses the engine's defined full-pressure fallback"
    );
    assert!(
        rgba_at(&medium_pressure.rgba(), 20, 32)[1] > rgba_at(&soft_pressure.rgba(), 20, 32)[1]
    );
    assert!(
        rgba_at(&firm_pressure.rgba(), 20, 32)[1] > rgba_at(&medium_pressure.rgba(), 20, 32)[1]
    );
}

#[test]
fn brush_pressure_missing_data_and_smudge_strength_and_pickup_are_deterministic() {
    let initial = fixture();
    let brush_soft = apply_dot(&initial, TestMode::Brush, 1.0, 1.0, Some(0.25));
    let brush_medium = apply_dot(&initial, TestMode::Brush, 1.0, 1.0, Some(0.5));
    let brush_firm = apply_dot(&initial, TestMode::Brush, 1.0, 1.0, Some(1.0));
    let brush_unavailable = apply_dot(&initial, TestMode::Brush, 1.0, 1.0, None);
    let brush_no_load = apply_dot(&initial, TestMode::Brush, 1.0, 0.0, Some(1.0));
    let brush_half_load = apply_dot(&initial, TestMode::Brush, 1.0, 0.5, Some(1.0));
    let brush_full_load = apply_dot(&initial, TestMode::Brush, 1.0, 1.0, Some(1.0));
    let brush_zero_opacity = apply_dot(&initial, TestMode::Brush, 0.0, 1.0, Some(1.0));
    assert_eq!(brush_firm.rgba(), brush_unavailable.rgba());
    assert_eq!(rgba_at(&brush_firm.rgba(), 20, 32), [255, 255, 0, 255]);
    assert_eq!(brush_no_load.rgba(), engine_from(&initial).rgba());
    assert_eq!(brush_zero_opacity.rgba(), engine_from(&initial).rgba());
    assert!(rgba_at(&brush_half_load.rgba(), 20, 32)[1] > 0);
    assert!(
        rgba_at(&brush_full_load.rgba(), 20, 32)[1] > rgba_at(&brush_half_load.rgba(), 20, 32)[1]
    );
    assert!(rgba_at(&brush_soft.rgba(), 20, 32)[1] < rgba_at(&brush_medium.rgba(), 20, 32)[1]);
    assert!(rgba_at(&brush_medium.rgba(), 20, 32)[1] < rgba_at(&brush_firm.rgba(), 20, 32)[1]);

    let smudge = |opacity, flow, pressure| {
        let mut engine = engine_from(&initial);
        engine.brush.opacity = opacity;
        engine.brush.flow = flow;
        apply(&mut engine, TestMode::Smudge, 3, START, END, pressure);
        engine
    };
    let weak = smudge(0.25, 0.0, Some(1.0));
    let strong = smudge(1.0, 0.0, Some(1.0));
    let low_pickup = smudge(1.0, 0.0, Some(1.0));
    let high_pickup = smudge(1.0, 1.0, Some(1.0));
    let soft_pressure = smudge(1.0, 0.0, Some(0.25));
    let firm_pressure = smudge(1.0, 0.0, Some(1.0));
    let unavailable_pressure = smudge(1.0, 0.0, None);
    assert_ne!(
        weak.rgba(),
        strong.rgba(),
        "Smudge strength changes redistribution"
    );
    assert_ne!(
        low_pickup.rgba(),
        high_pickup.rgba(),
        "Smudge flow controls pickup from later sampled pixels"
    );
    assert_ne!(
        soft_pressure.rgba(),
        firm_pressure.rgba(),
        "Smudge pressure affects deposit strength"
    );
    assert_eq!(firm_pressure.rgba(), unavailable_pressure.rgba());
    let low_pixel = rgba_at(&weak.rgba(), 58, 32);
    let high_pixel = rgba_at(&strong.rgba(), 58, 32);
    assert!(
        high_pixel[3] > low_pixel[3],
        "stronger Smudge deposits more alpha at the transparent edge"
    );
}

#[test]
fn each_mode_preview_cancel_commit_undo_redo_and_rpl_reopen_match_pixels() {
    let initial = fixture();
    let initial_rgba = engine_from(&initial).rgba();
    let original_undo_count = 0;
    let modes = [
        TestMode::Brush,
        TestMode::Smudge,
        TestMode::Mixer { strength: 0.5 },
    ];

    for (index, mode) in modes.into_iter().enumerate() {
        let mut preview = engine_from(&initial);
        let id = StrokeId(20 + index as u64);
        let stroke_mode = mode.stroke_mode();
        preview
            .apply_with_mode(
                &C::BeginStroke {
                    stroke_id: id,
                    sample: sample(START[0], START[1], Some(1.0), 0),
                },
                stroke_mode,
            )
            .unwrap();
        preview
            .apply_with_mode(
                &C::UpdateStroke {
                    stroke_id: id,
                    sample: sample(END[0], END[1], Some(1.0), 1),
                },
                stroke_mode,
            )
            .unwrap();
        assert_ne!(
            preview.rgba(),
            initial_rgba,
            "{} preview should render provisional pixels",
            mode.name()
        );
        preview.cancel_all();
        assert_eq!(
            preview.rgba(),
            initial_rgba,
            "{} cancellation restores the exact initial canvas",
            mode.name()
        );
        assert_eq!(
            preview.state(),
            &initial,
            "{} cancellation must not commit layer tiles",
            mode.name()
        );
        assert_eq!(preview.undo_count(), original_undo_count);

        let mut committed = engine_from(&initial);
        apply(
            &mut committed,
            mode,
            40 + index as u64,
            START,
            END,
            Some(1.0),
        );
        let expected_pixels = committed.rgba();
        let expected_state = committed.state().clone();
        assert_ne!(
            expected_state,
            initial,
            "{} commit changes authoritative state",
            mode.name()
        );
        assert_premultiplied(&expected_state);

        let file = TempFile::new(mode.name());
        let mut store = Store::new(file.path.clone());
        store.save(&expected_state).unwrap();
        let (_opened_store, reopened_state) = Store::open(file.path.clone()).unwrap();
        let reopened = Engine::from_state(reopened_state.clone()).unwrap();
        assert_eq!(
            reopened.rgba(),
            expected_pixels,
            "{} .rpl reopen keeps exact composite RGBA",
            mode.name()
        );
        assert_eq!(
            reopened_state,
            expected_state,
            "{} .rpl reopen keeps the exact committed document state",
            mode.name()
        );

        assert!(committed.undo(), "{} Undo should succeed", mode.name());
        assert_eq!(
            committed.state(),
            &initial,
            "{} Undo restores the exact initial document",
            mode.name()
        );
        assert_eq!(committed.rgba(), initial_rgba);
        assert!(committed.redo(), "{} Redo should succeed", mode.name());
        assert_eq!(
            committed.state(),
            &expected_state,
            "{} Redo restores the exact committed document",
            mode.name()
        );
        assert_eq!(committed.rgba(), expected_pixels);
    }
}

#[test]
fn all_modes_respect_selection_bounds_and_the_selected_layer() {
    let mut initial = fixture();
    initial.selection =
        Some(Selection::new(SelectionKind::Rectangle, vec![[8.0, 24.0], [48.0, 40.0]]).unwrap());
    let before_second_layer = initial.layers[1].tiles.clone();
    let before_outside = raw_at(&initial, 0, 58, 32);
    let before_inside = raw_at(&initial, 0, 30, 32);

    for mode in [
        TestMode::Brush,
        TestMode::Smudge,
        TestMode::Mixer { strength: 0.5 },
    ] {
        let mut engine = engine_from(&initial);
        apply(&mut engine, mode, 80, START, END, Some(1.0));
        assert_ne!(
            raw_at(engine.state(), 0, 30, 32),
            before_inside,
            "{} changes eligible pixels",
            mode.name()
        );
        assert_eq!(
            raw_at(engine.state(), 0, 58, 32),
            before_outside,
            "{} cannot paint outside selection",
            mode.name()
        );
        assert_eq!(
            engine.state().layers[1].tiles,
            before_second_layer,
            "{} cannot mutate another layer",
            mode.name()
        );
        assert_premultiplied(engine.state());
    }
}

#[test]
fn smudge_on_an_empty_selected_layer_does_not_stamp_the_foreground_color() {
    let mut initial = fixture();
    initial.selected = 2;
    let mut engine = engine_from(&initial);
    let before = engine.rgba();
    let before_layers = engine.state().layers.clone();

    apply(
        &mut engine,
        TestMode::Smudge,
        91,
        [60.5, 32.5],
        [88.5, 32.5],
        Some(1.0),
    );

    assert_eq!(engine.rgba(), before, "empty-source Smudge is a no-op");
    assert_eq!(
        engine.state().layers,
        before_layers,
        "an empty source layer gains no foreground deposit"
    );
    assert_eq!(raw_at(engine.state(), 1, 72, 32), [0; 4]);
}

struct TempFile {
    directory: PathBuf,
    path: PathBuf,
}

impl TempFile {
    fn new(label: &str) -> Self {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "bp-paint-modes-{}-{id}-{label}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("result.rpl");
        Self { directory, path }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
