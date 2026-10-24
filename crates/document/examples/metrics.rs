//! Reproducible CPU benchmark; not a display latency or hardware GPU benchmark.
use paint_document::{Engine, storage};
use paint_input::{Buttons, Tool};
use paint_stroke::{StrokeCommand as C, StrokeId, StrokeSample, coordinates::DocumentPoint};
use std::time::Instant;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for size in [2048, 4096] {
        let mut e = Engine::new(size, size, 300)?;
        e.brush.size = 12.0;
        e.take_damage();
        let start = Instant::now();
        for id in 0..1000 {
            let x = f64::from(20 + (id as u32 * 37) % (size - 240));
            let y = f64::from(20 + (id as u32 * 71) % (size - 40));
            let s = StrokeSample {
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
    Ok(())
}
