//! Deterministic raster brush latency benchmark.
//!
//! Run with `cargo run --release --locked -p paint-document --example
//! brush_latency`. This measures document processing and CPU tile compositing,
//! not native event arrival, GPU execution, or display presentation.
use paint_document::{
    Blend, Engine, Layer, LayerMask, State, StrokeMode, Symmetry, SymmetryMode, TILE, TILE_BYTES,
    brush::{Brush, BrushKind},
    editing::{Selection, SelectionKind},
    storage,
};
use paint_input::{Buttons, Tool};
use paint_stroke::{StrokeCommand as C, StrokeId, StrokeSample, coordinates::DocumentPoint};
use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};

const DOC: u32 = 4096;
const EVENTS: usize = 96;
const TRIALS: usize = 5;
const MOVE_PER_EVENT: f64 = 4.0;
const ARRIVAL_NS_240_HZ: u128 = 1_000_000_000 / 240;

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputKind {
    Mouse,
    Pen,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Paint,
    Eraser,
    Smudge,
    Mixer,
    Mask,
}

impl Operation {
    fn name(self) -> &'static str {
        match self {
            Self::Paint => "paint",
            Self::Eraser => "eraser",
            Self::Smudge => "smudge",
            Self::Mixer => "mixer",
            Self::Mask => "mask",
        }
    }

    fn mode(self) -> StrokeMode {
        match self {
            Self::Paint | Self::Eraser => StrokeMode::Paint,
            Self::Smudge => StrokeMode::Smudge,
            Self::Mixer => StrokeMode::ColorMix { strength: 0.55 },
            Self::Mask => StrokeMode::MaskPaint,
        }
    }
}

struct Case {
    operation: Operation,
    size: f64,
    kind: BrushKind,
    input: InputKind,
    populated: bool,
    dense_two_layer: bool,
    selection: bool,
    symmetry: bool,
}

#[derive(Default)]
struct Samples {
    apply: Vec<Duration>,
    composite: Vec<Duration>,
    service: Vec<Duration>,
    commit: Vec<Duration>,
    history: Vec<Duration>,
    damaged_tiles: usize,
    upload_bytes: usize,
    touched_tiles: BTreeSet<(u32, u32)>,
    output_digests: Vec<u64>,
    queued_events_240: usize,
    backlog_240_ns: u128,
}

fn pixel_tile(color: [u8; 4], patterned: bool, tile_x: u32, tile_y: u32) -> Vec<u8> {
    let mut bytes = vec![0; TILE_BYTES];
    for y in 0..TILE {
        for x in 0..TILE {
            let index = ((y * TILE + x) * 4) as usize;
            let rgba = if patterned {
                let document_x = tile_x * TILE + x;
                let document_y = tile_y * TILE + y;
                [
                    ((document_x * 11 + document_y * 3 + 32) % 224 + 31) as u8,
                    ((document_x * 5 + document_y * 13 + 48) % 224 + 31) as u8,
                    ((document_x * 7 + document_y * 17 + 64) % 224 + 31) as u8,
                    255,
                ]
            } else {
                color
            };
            bytes[index..index + 4].copy_from_slice(&rgba);
        }
    }
    bytes
}

fn fixture(case: &Case) -> Result<State, Box<dyn std::error::Error>> {
    let mut state = State::new(DOC, DOC, 300)?;
    state.background = None;
    if case.selection {
        state.selection = Some(Selection::new(
            SelectionKind::Rectangle,
            vec![[1200.0, 1500.0], [2900.0, 2600.0]],
        )?);
    }
    let layer_count = if case.dense_two_layer { 2 } else { 4 };
    for id in 2..=layer_count {
        state.layers.push(Layer {
            id: id as u64,
            name: format!("Background {id}"),
            visible: true,
            locked: false,
            opacity: 255,
            blend: Blend::Normal,
            mask: None,
            vector_paths: Vec::new(),
            tiles: Default::default(),
        });
    }
    if case.operation == Operation::Mask {
        state.layers[0].mask = Some(LayerMask::default());
    }

    if case.populated || case.dense_two_layer {
        let first_x = if case.dense_two_layer { 0 } else { 18 };
        let last_x = if case.dense_two_layer { DOC / TILE } else { 46 };
        let first_y = if case.dense_two_layer { 0 } else { 22 };
        let last_y = if case.dense_two_layer { DOC / TILE } else { 42 };
        for (layer_index, layer) in state.layers.iter_mut().enumerate() {
            if !case.dense_two_layer && layer_index > 0 {
                // Extra visible layers exercise composition without making the
                // selected layer's pixel source ambiguous for these timings.
                let tx = 16 + layer_index as u32;
                let ty = 16 + layer_index as u32;
                layer.tiles.insert(
                    (tx, ty),
                    Arc::new(pixel_tile([24, 80, 160, 255], false, tx, ty)),
                );
                continue;
            }
            let layer_last_x = if case.dense_two_layer && layer_index > 0 {
                DOC / TILE / 2
            } else {
                last_x
            };
            let layer_last_y = if case.dense_two_layer && layer_index > 0 {
                DOC / TILE / 2
            } else {
                last_y
            };
            for ty in first_y..layer_last_y {
                for tx in first_x..layer_last_x {
                    let color = if layer_index == 0 {
                        [180, 32, 96, 255]
                    } else {
                        [32, 96, 180, 255]
                    };
                    layer.tiles.insert(
                        (tx, ty),
                        Arc::new(pixel_tile(color, layer_index == 0, tx, ty)),
                    );
                }
            }
        }
    } else {
        // A visible lower layer beneath a transparent active layer separates
        // layer-local brush behavior from composite presentation cost.
        let lower = &mut state.layers[1];
        lower.tiles.insert(
            (32, 32),
            Arc::new(pixel_tile([220, 48, 24, 255], false, 32, 32)),
        );
    }
    state.validate()?;
    Ok(state)
}

fn sample(input: InputKind, x: f64, n: usize) -> StrokeSample {
    let (tool, pressure, tilt) = match input {
        InputKind::Mouse => (Tool::Mouse, None, [None, None]),
        InputKind::Pen => {
            let pressure = 0.2 + ((n % 7) as f32) * 0.12;
            (
                Tool::Pen,
                Some(pressure.min(1.0)),
                [Some(18.0), Some(-12.0)],
            )
        }
    };
    StrokeSample {
        source_counter: None,
        pointer_id: 1,
        tool,
        position_client_px: [x, 2048.0],
        position_document: DocumentPoint([x, 2048.0]),
        pressure,
        tilt_degrees: tilt,
        buttons: Buttons::default(),
        source_time_ms: ((n as u128 * 1000) / 240) as u64,
        time_ms: ((n as u128 * 1000) / 240) as u64,
    }
}

fn percentile(values: &[Duration], p: f64) -> Duration {
    if values.is_empty() {
        return Duration::ZERO;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() as f64 * p).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[index]
}

fn memory_sample() -> String {
    #[cfg(target_os = "linux")]
    {
        let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
            return "unavailable".into();
        };
        let field = |prefix: &str| {
            status
                .lines()
                .find(|line| line.starts_with(prefix))
                .unwrap_or("unavailable")
        };
        format!("{}; {}", field("VmRSS:"), field("VmHWM:"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        "unavailable (RSS sampler uses Linux /proc)".into()
    }
}

fn composite_damage(engine: &mut Engine, samples: &mut Samples) -> Duration {
    let damage = engine.take_damage();
    samples.damaged_tiles += damage.len();
    samples.upload_bytes += damage.len() * TILE_BYTES;
    samples.touched_tiles.extend(damage.iter().copied());
    let start = Instant::now();
    let mut checksum = 0_u64;
    for key in damage {
        let tile = engine.composite_tile(key);
        checksum = checksum.wrapping_add(u64::from(tile[0]));
    }
    std::hint::black_box(checksum);
    start.elapsed()
}

fn rendered_digest(engine: &Engine, keys: &BTreeSet<(u32, u32)>) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for key in keys {
        for byte in key
            .0
            .to_le_bytes()
            .into_iter()
            .chain(key.1.to_le_bytes())
            .chain(engine.composite_tile(*key))
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn run_trial(
    case: &Case,
    state: &State,
    samples: &mut Samples,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = Engine::from_state(state.clone())?;
    let mut brush = Brush::preset(case.kind);
    brush.size = case.size;
    brush.spacing = 0.15;
    brush.pressure_size = case.input == InputKind::Pen;
    brush.pressure_opacity = true;
    brush.color = [248, 220, 24];
    engine.brush = brush;
    if case.symmetry {
        engine.set_symmetry(Symmetry {
            mode: SymmetryMode::VerticalAndHorizontal,
            axes: 8,
            center: [0.5, 0.5],
        })?;
    }
    engine.take_damage();
    let mode = case.operation.mode();
    let id = StrokeId(1);
    let first = sample(case.input, 1664.0, 0);
    let started = Instant::now();
    engine.apply_with_mode(
        &C::BeginStroke {
            stroke_id: id,
            sample: first,
        },
        mode,
    )?;
    let begin = started.elapsed();
    samples.apply.push(begin);
    let composite = composite_damage(&mut engine, samples);
    samples.composite.push(composite);
    let service = begin + composite;
    samples.service.push(service);
    let mut backlog = 0_u128;
    if service.as_nanos() > ARRIVAL_NS_240_HZ {
        backlog = service.as_nanos() - ARRIVAL_NS_240_HZ;
        samples.queued_events_240 += 1;
    }
    samples.backlog_240_ns = samples.backlog_240_ns.max(backlog);

    for index in 1..=EVENTS {
        let current = sample(
            case.input,
            first.position_document.0[0] + MOVE_PER_EVENT * index as f64,
            index,
        );
        let start = Instant::now();
        engine.apply_with_mode(
            &C::UpdateStroke {
                stroke_id: id,
                sample: current,
            },
            mode,
        )?;
        let apply = start.elapsed();
        let composite = composite_damage(&mut engine, samples);
        let service = apply + composite;
        samples.apply.push(apply);
        samples.composite.push(composite);
        samples.service.push(service);
        backlog = backlog
            .saturating_add(service.as_nanos())
            .saturating_sub(ARRIVAL_NS_240_HZ);
        samples.backlog_240_ns = samples.backlog_240_ns.max(backlog);
        if service.as_nanos() > ARRIVAL_NS_240_HZ {
            samples.queued_events_240 += 1;
        }
    }

    let endpoint = sample(
        case.input,
        first.position_document.0[0] + MOVE_PER_EVENT * (EVENTS + 1) as f64,
        EVENTS + 1,
    );
    let start = Instant::now();
    engine.apply_with_mode(
        &C::CommitStroke {
            stroke_id: id,
            sample: endpoint,
        },
        mode,
    )?;
    samples.commit.push(start.elapsed());
    composite_damage(&mut engine, samples);
    let start = Instant::now();
    let _ = engine.undo();
    let _ = engine.redo();
    samples.history.push(start.elapsed());
    samples
        .output_digests
        .push(rendered_digest(&engine, &samples.touched_tiles));
    std::hint::black_box(engine.state());
    Ok(())
}

fn run(case: Case) -> Result<(), Box<dyn std::error::Error>> {
    let state = fixture(&case)?;
    // One warm-up stroke is excluded from latency distributions.
    let mut warmup = Samples::default();
    run_trial(&case, &state, &mut warmup)?;
    let mut samples = Samples::default();
    let memory_before = memory_sample();
    for _ in 0..TRIALS {
        run_trial(&case, &state, &mut samples)?;
    }
    let apply_median = percentile(&samples.apply, 0.50).as_nanos();
    let apply_p95 = percentile(&samples.apply, 0.95).as_nanos();
    let apply_max = samples
        .apply
        .iter()
        .max()
        .copied()
        .unwrap_or_default()
        .as_nanos();
    let service_median = percentile(&samples.service, 0.50).as_nanos();
    let service_p95 = percentile(&samples.service, 0.95).as_nanos();
    let service_max = samples
        .service
        .iter()
        .max()
        .copied()
        .unwrap_or_default()
        .as_nanos();
    let composite_p95 = percentile(&samples.composite, 0.95).as_nanos();
    let composite_max = samples
        .composite
        .iter()
        .max()
        .copied()
        .unwrap_or_default()
        .as_nanos();
    let commit_median = percentile(&samples.commit, 0.50).as_nanos();
    let commit_p95 = percentile(&samples.commit, 0.95).as_nanos();
    let commit_max = samples
        .commit
        .iter()
        .max()
        .copied()
        .unwrap_or_default()
        .as_nanos();
    let history_median = percentile(&samples.history, 0.50).as_nanos();
    let history_p95 = percentile(&samples.history, 0.95).as_nanos();
    let history_max = samples
        .history
        .iter()
        .max()
        .copied()
        .unwrap_or_default()
        .as_nanos();
    let backlog_ms = samples.backlog_240_ns as f64 / 1_000_000.0;
    let output_digest = samples.output_digests.first().copied().unwrap_or_default();
    if samples
        .output_digests
        .iter()
        .any(|digest| *digest != output_digest)
    {
        return Err("identical replay produced different rendered pixels".into());
    }
    println!(
        "brush_latency op={} size={} kind={:?} input={} populated={} dense2={} selection={} symmetry={} layers={} events={} apply_us_med/p95/max={:.3}/{:.3}/{:.3} service_us_med/p95/max={:.3}/{:.3}/{:.3} composite_us_med/p95/max={:.3}/{:.3}/{:.3} commit_us_med/p95/max={:.3}/{:.3}/{:.3} undo_redo_us_med/p95/max={:.3}/{:.3}/{:.3} simulated_240hz_overruns={} max_backlog_ms={:.3} damage_tiles={} touched_tiles={} upload_bytes={} pixel_digest={:016x} rss_before=[{}] rss_after=[{}]",
        case.operation.name(),
        case.size,
        case.kind,
        match case.input {
            InputKind::Mouse => "mouse",
            InputKind::Pen => "pen-like",
        },
        case.populated,
        case.dense_two_layer,
        case.selection,
        case.symmetry,
        state.layers.len(),
        samples.apply.len(),
        apply_median as f64 / 1000.0,
        apply_p95 as f64 / 1000.0,
        apply_max as f64 / 1000.0,
        service_median as f64 / 1000.0,
        service_p95 as f64 / 1000.0,
        service_max as f64 / 1000.0,
        percentile(&samples.composite, 0.50).as_nanos() as f64 / 1000.0,
        composite_p95 as f64 / 1000.0,
        composite_max as f64 / 1000.0,
        commit_median as f64 / 1000.0,
        commit_p95 as f64 / 1000.0,
        commit_max as f64 / 1000.0,
        history_median as f64 / 1000.0,
        history_p95 as f64 / 1000.0,
        history_max as f64 / 1000.0,
        samples.queued_events_240,
        backlog_ms,
        samples.damaged_tiles,
        samples.touched_tiles.len(),
        samples.upload_bytes,
        output_digest,
        memory_before,
        memory_sample(),
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rust_version = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "record rustc --version separately".into());
    println!(
        "brush_latency host_cpu={} rust={} hz=240 move_px={} events_per_stroke={} trials={} rss=[VmRSS,VmHWM]",
        std::env::var("BP_BENCH_CPU").unwrap_or_else(|_| "record-with-lscpu".into()),
        rust_version,
        MOVE_PER_EVENT,
        EVENTS,
        TRIALS,
    );
    for size in [16.0, 64.0, 128.0, 256.0] {
        for input in [InputKind::Mouse, InputKind::Pen] {
            for (operation, kind, populated) in [
                (Operation::Paint, BrushKind::Round, false),
                (Operation::Eraser, BrushKind::Eraser, true),
                (Operation::Smudge, BrushKind::Round, true),
                (Operation::Mixer, BrushKind::Round, true),
            ] {
                run(Case {
                    operation,
                    size,
                    kind,
                    input,
                    populated,
                    dense_two_layer: false,
                    selection: false,
                    symmetry: false,
                })?;
            }
        }
    }
    for kind in [
        BrushKind::Pencil,
        BrushKind::Textured,
        BrushKind::Calligraphy,
        BrushKind::Airbrush,
    ] {
        run(Case {
            operation: Operation::Paint,
            size: 256.0,
            kind,
            input: InputKind::Pen,
            populated: true,
            dense_two_layer: false,
            selection: false,
            symmetry: false,
        })?;
    }
    for operation in [Operation::Paint, Operation::Smudge, Operation::Mixer] {
        run(Case {
            operation,
            size: 256.0,
            kind: BrushKind::Round,
            input: InputKind::Pen,
            populated: true,
            dense_two_layer: true,
            selection: true,
            symmetry: false,
        })?;
    }
    run(Case {
        operation: Operation::Mask,
        size: 256.0,
        kind: BrushKind::Round,
        input: InputKind::Pen,
        populated: true,
        dense_two_layer: false,
        selection: true,
        symmetry: true,
    })?;

    // Persistence is timed separately from event latency; it is not charged to
    // the 240 Hz input-service estimate.
    let state = fixture(&Case {
        operation: Operation::Paint,
        size: 256.0,
        kind: BrushKind::Round,
        input: InputKind::Pen,
        populated: true,
        dense_two_layer: true,
        selection: false,
        symmetry: false,
    })?;
    let mut encode_times = Vec::with_capacity(TRIALS);
    let mut encoded = Vec::new();
    for _ in 0..TRIALS {
        let start = Instant::now();
        encoded = storage::encode(&state)?;
        encode_times.push(start.elapsed());
    }
    println!(
        "brush_latency persistence dense2_4096 encode_us_med/p95/max={}/{}/{} bytes={} rss=[{}]",
        percentile(&encode_times, 0.50).as_micros(),
        percentile(&encode_times, 0.95).as_micros(),
        encode_times
            .iter()
            .max()
            .copied()
            .unwrap_or_default()
            .as_micros(),
        encoded.len(),
        memory_sample(),
    );
    Ok(())
}
