//! Opt-in, document-engine-only latency and allocation harness.
//!
//! Run in release mode with:
//! `BP_BENCH_SAMPLES=15 cargo test -p paint-document --test large_brush_performance --release -- --ignored --nocapture`
//! This reports observations and has no hardware-specific timing threshold.

use paint_document::{Blend, Engine, Layer, State, StrokeMode, TILE, TILE_BYTES};
use paint_input::{Buttons, Tool};
use paint_stroke::{StrokeCommand, StrokeId, StrokeSample, coordinates::DocumentPoint};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const WIDTH: u32 = 2048;
const HEIGHT: u32 = 512;
const START_X: f64 = 256.0;
const END_X: f64 = 1792.0;
const STROKE_Y: f64 = 256.0;
const SIZES: [u16; 4] = [16, 64, 128, 256];
const PROFILES: [Profile; 2] = [Profile::Sparse, Profile::Dense];
const MODES: [Mode; 4] = [Mode::Brush, Mode::Eraser, Mode::Smudge, Mode::Mixer];
const WARMUPS: usize = 2;

struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REQUESTED_BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// SAFETY: each operation delegates to `System` with the same pointer and
// layout contract. The counters are atomic and do not allocate.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: `layout` is supplied by Rust's global allocator caller.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            record_allocation(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller provides the pointer and matching layout returned
        // by this allocator, and `System` owns the original allocation.
        unsafe { System.dealloc(ptr, layout) };
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller provides an allocation from this allocator and its
        // original layout; `System` performs the requested reallocation.
        let replacement = unsafe { System.realloc(ptr, layout, new_size) };
        if !replacement.is_null() {
            if ACTIVE.load(Ordering::Relaxed) {
                ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
                REQUESTED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
            }
            let current = LIVE_BYTES.load(Ordering::Relaxed);
            let updated = current
                .saturating_sub(layout.size())
                .saturating_add(new_size);
            LIVE_BYTES.store(updated, Ordering::Relaxed);
            update_peak(updated);
        }
        replacement
    }
}

fn record_allocation(size: usize) {
    if ACTIVE.load(Ordering::Relaxed) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        REQUESTED_BYTES.fetch_add(size as u64, Ordering::Relaxed);
    }
    let live = LIVE_BYTES
        .fetch_add(size, Ordering::Relaxed)
        .saturating_add(size);
    update_peak(live);
}

fn update_peak(live: usize) {
    let mut previous = PEAK_LIVE_BYTES.load(Ordering::Relaxed);
    while live > previous {
        match PEAK_LIVE_BYTES.compare_exchange_weak(
            previous,
            live,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(actual) => previous = actual,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Profile {
    Sparse,
    Dense,
}

impl Profile {
    fn name(self) -> &'static str {
        match self {
            Self::Sparse => "sparse",
            Self::Dense => "dense",
        }
    }

    fn active_rows(self) -> std::ops::Range<u32> {
        match self {
            Self::Sparse => 224..288,
            Self::Dense => 0..HEIGHT,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Brush,
    Eraser,
    Smudge,
    Mixer,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Brush => "brush",
            Self::Eraser => "eraser",
            Self::Smudge => "smudge",
            Self::Mixer => "color_mixer_0.5",
        }
    }

    fn engine_mode(self) -> StrokeMode {
        match self {
            Self::Brush | Self::Eraser => StrokeMode::Paint,
            Self::Smudge => StrokeMode::Smudge,
            Self::Mixer => StrokeMode::ColorMix { strength: 0.5 },
        }
    }

    fn eraser_button(self) -> bool {
        matches!(self, Self::Eraser)
    }
}

#[derive(Clone, Copy)]
struct AllocSnapshot {
    allocations: u64,
    requested_bytes: u64,
    live_bytes: usize,
    peak_live_bytes: usize,
}

fn alloc_snapshot() -> AllocSnapshot {
    AllocSnapshot {
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        requested_bytes: REQUESTED_BYTES.load(Ordering::Relaxed),
        live_bytes: LIVE_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
    }
}

fn set_pixel(state: &mut State, layer: usize, x: u32, y: u32, rgba_pm: [u8; 4]) {
    let key = (x / TILE, y / TILE);
    let tile = state.layers[layer]
        .tiles
        .entry(key)
        .or_insert_with(|| std::sync::Arc::new(vec![0; TILE_BYTES]));
    let offset = ((y % TILE * TILE + x % TILE) * 4) as usize;
    std::sync::Arc::make_mut(tile)[offset..offset + 4].copy_from_slice(&rgba_pm);
}

fn fixture(profile: Profile) -> State {
    let mut state = State::new(WIDTH, HEIGHT, 300).expect("valid benchmark canvas");
    state.background = None;
    state.brush.kind = paint_document::brush::BrushKind::Round;
    state.brush.size = 16.0;
    state.brush.opacity = 0.8;
    state.brush.flow = 0.7;
    state.brush.hardness = 0.8;
    state.brush.spacing = 0.5;
    state.brush.pressure_size = false;
    state.brush.pressure_opacity = false;
    state.brush.color = [255, 220, 16];

    state.layers.push(Layer {
        id: 2,
        name: "Second green layer under transparent region".into(),
        visible: true,
        locked: false,
        opacity: 255,
        blend: Blend::Normal,
        mask: None,
        vector_paths: Vec::new(),
        tiles: Default::default(),
    });

    for y in profile.active_rows() {
        for x in 0..512 {
            set_pixel(&mut state, 0, x, y, [220, 24, 20, 255]);
        }
        for x in 512..1024 {
            set_pixel(&mut state, 0, x, y, [20, 32, 220, 255]);
        }
        for x in 1024..1280 {
            set_pixel(&mut state, 0, x, y, [72, 0, 72, 128]);
        }
        for x in 1280..1792 {
            set_pixel(&mut state, 1, x, y, [12, 150, 48, 255]);
        }
    }

    state.validate().expect("valid premultiplied fixture");
    state
}

fn sample(x: f64, time_ms: u64, erase: bool) -> StrokeSample {
    StrokeSample {
        source_counter: None,
        pointer_id: 5,
        tool: Tool::Pen,
        position_client_px: [x, STROKE_Y],
        position_document: DocumentPoint([x, STROKE_Y]),
        pressure: Some(0.75),
        tilt_degrees: [Some(0.0), None],
        buttons: Buttons {
            eraser: erase,
            ..Buttons::default()
        },
        source_time_ms: time_ms,
        time_ms,
    }
}

fn stroke_positions(size: u16) -> Vec<f64> {
    let step = f64::from(size) * 0.5;
    let count = ((END_X - START_X) / step).round() as usize;
    (1..=count)
        .map(|index| START_X + step * index as f64)
        .collect()
}

fn run_stroke(
    engine: &mut Engine,
    mode: Mode,
    positions: &[f64],
    update_timings: &mut Vec<Duration>,
    stroke_id: u64,
) -> Duration {
    let stroke_mode = mode.engine_mode();
    let start = sample(START_X, 0, mode.eraser_button());
    let whole_start = Instant::now();
    engine
        .apply_with_mode(
            &StrokeCommand::BeginStroke {
                stroke_id: StrokeId(stroke_id),
                sample: start,
            },
            stroke_mode,
        )
        .expect("begin benchmark stroke");

    for (index, x) in positions.iter().copied().enumerate() {
        let input = StrokeCommand::UpdateStroke {
            stroke_id: StrokeId(stroke_id),
            sample: sample(x, index as u64 + 1, mode.eraser_button()),
        };
        let dab_start = Instant::now();
        engine
            .apply_with_mode(&input, stroke_mode)
            .expect("update benchmark stroke");
        update_timings.push(dab_start.elapsed());
    }

    engine
        .apply_with_mode(
            &StrokeCommand::CommitStroke {
                stroke_id: StrokeId(stroke_id),
                sample: sample(END_X, positions.len() as u64 + 1, mode.eraser_button()),
            },
            stroke_mode,
        )
        .expect("commit benchmark stroke");
    whole_start.elapsed()
}

fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    let rank = sorted.len().saturating_mul(percentile).div_ceil(100).max(1);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn summary_ns(samples: &[Duration]) -> (u128, u128, u128) {
    let mut values: Vec<u128> = samples.iter().map(Duration::as_nanos).collect();
    values.sort_unstable();
    let median = if values.len().is_multiple_of(2) {
        (values[values.len() / 2 - 1] + values[values.len() / 2]) / 2
    } else {
        values[values.len() / 2]
    };
    (median, percentile(&values, 95), *values.last().unwrap())
}

fn state_bytes(engine: &Engine) -> usize {
    engine
        .state()
        .layers
        .iter()
        .map(|layer| layer.tiles.len() * TILE_BYTES)
        .sum()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[test]
#[ignore = "opt-in performance harness; run in release mode, no universal timing threshold"]
fn large_brush_modes_report_latency_and_memory_observations() {
    let samples = std::env::var("BP_BENCH_SAMPLES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(15)
        .clamp(3, 200);

    println!(
        "metadata,os={},arch={},parallelism={:?},profile=release",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism()
    );
    println!(
        "method,canvas=2048x512,dpi=300,brush_sizes=16/64/128/256,spacing=0.5,path_px=1536,pressure=0.75,foreground=#ffdc10,warmups={WARMUPS},samples={samples}"
    );
    println!(
        "profile,mode,size_px,updates_per_stroke,stroke_median_ns,stroke_p95_ns,stroke_max_ns,update_dab_median_ns,update_dab_p95_ns,update_dab_max_ns,alloc_ops,requested_alloc_bytes,live_delta_bytes,peak_live_delta_bytes,document_tile_bytes,history_undo_bytes,damaged_tiles,output_hash_fnv1a"
    );

    for profile in PROFILES {
        let initial = fixture(profile);
        let baseline_hash = fnv1a(&Engine::from_state(initial.clone()).unwrap().rgba());
        for mode in MODES {
            for size in SIZES {
                let positions = stroke_positions(size);
                let mut stroke_timings = Vec::with_capacity(samples);
                let mut dab_timings = Vec::with_capacity(positions.len() * samples);
                let mut allocation_delta = (0_u64, 0_u64, 0_usize, 0_usize);
                let mut output_hash = 0;
                let mut document_bytes = 0;
                let mut history_bytes = 0;
                let mut damaged_tiles = 0;

                for warmup in 0..WARMUPS {
                    let mut engine = Engine::from_state(initial.clone()).unwrap();
                    engine.brush.size = f64::from(size);
                    engine.sync_brush_settings();
                    engine.take_damage();
                    let mut timings = Vec::with_capacity(positions.len());
                    run_stroke(
                        &mut engine,
                        mode,
                        &positions,
                        &mut timings,
                        warmup as u64 + 1,
                    );
                    assert_eq!(engine.active_count(), 0);
                    assert_eq!(engine.undo_count(), 1);
                }

                for iteration in 0..samples {
                    let mut engine = Engine::from_state(initial.clone()).unwrap();
                    engine.brush.size = f64::from(size);
                    engine.sync_brush_settings();
                    engine.take_damage();
                    let mut timings = Vec::with_capacity(positions.len());
                    PEAK_LIVE_BYTES.store(LIVE_BYTES.load(Ordering::Relaxed), Ordering::Relaxed);
                    let before = alloc_snapshot();
                    ACTIVE.store(true, Ordering::Relaxed);
                    let total = run_stroke(
                        &mut engine,
                        mode,
                        &positions,
                        &mut timings,
                        iteration as u64 + 100,
                    );
                    ACTIVE.store(false, Ordering::Relaxed);
                    let after = alloc_snapshot();

                    assert_eq!(engine.active_count(), 0, "stroke transaction must finish");
                    assert_eq!(
                        engine.undo_count(),
                        1,
                        "stroke must create one history item"
                    );
                    let result_hash = fnv1a(&engine.rgba());
                    assert_ne!(result_hash, baseline_hash, "mode must alter fixture pixels");

                    stroke_timings.push(total);
                    dab_timings.extend(timings);
                    allocation_delta.0 += after.allocations - before.allocations;
                    allocation_delta.1 += after.requested_bytes - before.requested_bytes;
                    allocation_delta.2 = allocation_delta
                        .2
                        .saturating_add(after.live_bytes.saturating_sub(before.live_bytes));
                    allocation_delta.3 = allocation_delta
                        .3
                        .max(after.peak_live_bytes.saturating_sub(before.live_bytes));
                    output_hash = result_hash;
                    document_bytes = state_bytes(&engine);
                    history_bytes = engine.history_bytes()[0];
                    damaged_tiles = engine.take_damage().len();
                }

                let stroke_summary = summary_ns(&stroke_timings);
                let dab_summary = summary_ns(&dab_timings);
                println!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    profile.name(),
                    mode.name(),
                    size,
                    positions.len(),
                    stroke_summary.0,
                    stroke_summary.1,
                    stroke_summary.2,
                    dab_summary.0,
                    dab_summary.1,
                    dab_summary.2,
                    allocation_delta.0 / samples as u64,
                    allocation_delta.1 / samples as u64,
                    allocation_delta.2 / samples,
                    allocation_delta.3,
                    document_bytes,
                    history_bytes,
                    damaged_tiles,
                    output_hash
                );
            }
        }
    }
}
