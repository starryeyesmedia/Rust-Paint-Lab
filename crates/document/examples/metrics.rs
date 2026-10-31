//! Reproducible CPU benchmark; not a display latency or hardware GPU benchmark.
use paint_document::{Engine, State, TILE, TILE_BYTES, journal::Journal, storage};
use paint_input::{Buttons, Tool};
use paint_stroke::{StrokeCommand as C, StrokeId, StrokeSample, coordinates::DocumentPoint};
use std::time::Instant;

fn memory_sample() -> String {
    #[cfg(target_os = "linux")]
    {
        let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
            return "unavailable".into();
        };
        let rss = status
            .lines()
            .find(|line| line.starts_with("VmRSS:"))
            .unwrap_or("VmRSS: unavailable");
        let high = status
            .lines()
            .find(|line| line.starts_with("VmHWM:"))
            .unwrap_or("VmHWM: unavailable");
        format!("{rss}; {high}")
    }
    #[cfg(not(target_os = "linux"))]
    {
        "unavailable (this example samples Linux /proc only)".into()
    }
}

fn journal_metrics(label: &str, state: &State) -> Result<(), Box<dyn std::error::Error>> {
    let temp = std::env::temp_dir().join(format!(
        "rpl-journal-metrics-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&temp)?;
    let path = temp.join("measured.rpl.journal");
    let mut journal = Journal::new(path.clone());
    let memory_before = memory_sample();
    let start = Instant::now();
    journal.append(state)?;
    let first_write = start.elapsed();
    let first_size = std::fs::metadata(&path)?.len();
    let mut second = state.clone();
    second.layers[0].name.push_str(" checkpoint");
    let start = Instant::now();
    journal.append(&second)?;
    let second_write = start.elapsed();
    let final_size = std::fs::metadata(&path)?.len();
    println!(
        "journal {label}: project_bytes={} first_write_us={} second_write_us={} journal_bytes_after_1={} journal_bytes_after_2={} growth_bytes={} memory_before=[{}] memory_after=[{}]",
        storage::encode(state)?.len(),
        first_write.as_micros(),
        second_write.as_micros(),
        first_size,
        final_size,
        final_size.saturating_sub(first_size),
        memory_before,
        memory_sample(),
    );
    drop(journal);
    std::fs::remove_dir_all(temp)?;
    Ok(())
}

fn journal_fixtures() -> Result<(), Box<dyn std::error::Error>> {
    journal_metrics("4096² sparse (1 tile)", &sparse_fixture()?)?;

    journal_metrics("4096² dense (all 4096 tiles)", &dense_fixture()?)?;
    Ok(())
}

fn sparse_fixture() -> Result<State, Box<dyn std::error::Error>> {
    let mut sparse = State::new(4096, 4096, 300)?;
    let mut tile = vec![0u8; TILE_BYTES];
    for pixel in tile.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[48, 24, 12, 128]);
    }
    sparse.layers[0]
        .tiles
        .insert((0, 0), std::sync::Arc::new(tile));
    sparse.validate()?;
    Ok(sparse)
}

fn dense_fixture() -> Result<State, Box<dyn std::error::Error>> {
    let mut dense = State::new(4096, 4096, 300)?;
    for y in 0..4096 / TILE {
        for x in 0..4096 / TILE {
            let mut tile = vec![0u8; TILE_BYTES];
            let red = ((x * 17 + y * 13) & 0xff) as u8;
            let green = ((x * 7 + y * 23) & 0xff) as u8;
            for pixel in tile.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[red, green, 32, 255]);
            }
            dense.layers[0]
                .tiles
                .insert((x, y), std::sync::Arc::new(tile));
        }
    }
    dense.validate()?;
    Ok(dense)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::args().nth(1).as_deref() {
        Some("--journal-sparse") => {
            return journal_metrics("4096² sparse (1 tile)", &sparse_fixture()?);
        }
        Some("--journal-dense") => {
            return journal_metrics("4096² dense (all 4096 tiles)", &dense_fixture()?);
        }
        Some(other) => return Err(format!("unknown metrics mode: {other}").into()),
        None => {}
    }
    for size in [2048, 4096] {
        let mut e = Engine::new(size, size, 300)?;
        e.brush.size = 12.0;
        e.take_damage();
        let start = Instant::now();
        for id in 0..1000 {
            let x = f64::from(20 + (id as u32 * 37) % (size - 240));
            let y = f64::from(20 + (id as u32 * 71) % (size - 40));
            let s = StrokeSample {
                source_counter: None,
                pointer_id: 1,
                tool: Tool::Pen,
                position_client_px: [x, y],
                position_document: DocumentPoint([x, y]),
                pressure: Some(0.7),
                tilt_degrees: [None, None],
                buttons: Buttons::default(),
                source_time_ms: id * 10,
                time_ms: id * 10,
            };
            e.apply(&C::BeginStroke {
                stroke_id: StrokeId(id),
                sample: s,
            })?;
            let mut end = s;
            end.position_document.0[0] += 200.0;
            e.apply(&C::CommitStroke {
                stroke_id: StrokeId(id),
                sample: end,
            })?;
        }
        let paint = start.elapsed();
        let start = Instant::now();
        let bytes = storage::encode(e.state())?;
        let encode = start.elapsed();
        let start = Instant::now();
        let restored = storage::decode(&bytes)?;
        let decode = start.elapsed();
        assert_eq!(restored.tile_count(), e.state().tile_count());
        let start = Instant::now();
        let steps = e.undo_count().min(32);
        for _ in 0..steps {
            e.undo();
        }
        for _ in 0..steps {
            e.redo();
        }
        println!(
            "{size}x{size}: 1000 strokes={}ms encode={}ms decode={}ms undo+redo({steps} steps each)={}ms tiles={} retained_undo={} file_bytes={} dirty_tiles={}",
            paint.as_millis(),
            encode.as_millis(),
            decode.as_millis(),
            start.elapsed().as_millis(),
            e.state().tile_count(),
            e.undo_count(),
            bytes.len(),
            e.take_damage().len()
        );
        let temp = std::env::temp_dir().join(format!(
            "rpl-metrics-{}-{size}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir(&temp)?;
        let mut store = storage::Store::new(temp.join("measured.rpl"));
        let start = Instant::now();
        for _ in 0..3 {
            store.save(e.state())?;
        }
        let save = start.elapsed();
        let start = Instant::now();
        let recovery = paint_document::recovery::Recovery::start(&temp)?;
        recovery.flush(e.state().clone())?;
        let checkpoint = start.elapsed();
        drop(recovery);
        let start = Instant::now();
        storage::export_png(&temp.join("image.png"), &e)?;
        let png = start.elapsed();
        println!(
            "{size}x{size}: 3 durable saves={}ms recovery checkpoint={}ms PNG={}ms history_payload={:?}",
            save.as_millis(),
            checkpoint.as_millis(),
            png.as_millis(),
            e.history_bytes()
        );
        std::fs::remove_dir_all(temp)?;
    }
    journal_fixtures()?;
    Ok(())
}
