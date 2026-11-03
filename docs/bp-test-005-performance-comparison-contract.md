# BP-TEST-005 large-brush performance contract

This harness measures document-engine work independently from windowing, GPU
presentation, and physical input. It makes no universal frame-time assertion.

## Reproduce the algorithmic run

From the repository root, use the same machine, power mode, Rust toolchain, and
release build for every comparison:

```sh
rustc -Vv
BP_BENCH_SAMPLES=15 cargo test -p paint-document --test large_brush_performance --release -- --ignored --nocapture
```

The harness includes two warmups and 15 measured strokes for each of two
2048×512 fixtures, four tools, and four brush sizes (16, 64, 128, and 256 px).
It prints CSV-like records to standard output. Retain the full output with the
source SHA, OS and version, CPU model, RAM, Rust version, power mode, and command.
Repeat the full command three times. Compare each row's per-run median, P95, and
maximum; report the median of the three run medians and retain each run's P95 and
maximum. Do not pool rows from different machines or use the worst observation
as a portable CI threshold.

The sparse fixture contains opaque red and blue regions, a premultiplied
half-alpha magenta region, transparent space, and a second green layer below
part of that transparent space. The dense fixture fills those same colored
regions through the full canvas height and keeps the same transparent regions
and second layer. Each identical starting state is cloned for every mode. The
straight 1536 px path crosses each color area and uses fixed pressure 0.75,
yellow foreground, Round brush, opacity 0.8, flow 0.7, hardness 0.8, and spacing
0.5. Eraser is selected through the engine's eraser input button. Color Mixer
uses strength 0.5.

## Read the measurements

- `stroke_*_ns` covers public `BeginStroke`, the fixed sequence of `UpdateStroke`
  calls, and `CommitStroke`, including transaction and history work.
- `update_dab_*_ns` covers one public update whose segment length is exactly one
  spacing interval. It is a useful per-dab input proxy, not a private brush-kernel
  timer. Begin and commit each do additional dab work and are included in total
  stroke timing.
- Allocation operations and requested bytes are deltas during the stroke call
  sequence. Live and peak-live byte deltas are process allocator observations
  around that same sequence. The integration-test process contains only this
  harness, but allocator instrumentation still adds small measurement overhead.
- `document_tile_bytes` and `history_undo_bytes` are engine-visible retained
  document/history sizes after commit. `damaged_tiles` reports the engine damage
  set available to a presenter; it is not GPU upload or presentation latency.
- The output hash checks that every measured stroke changed the fixture. It is a
  compact run-integrity check, not a pixel correctness oracle.

The command is opt-in and ignored by the ordinary test suite. Its assertions
check only that a stroke committed and changed the controlled fixture. Existing
pixel behavior tests remain the source of semantic correctness assertions.
Keep the harness in release mode for comparisons. A debug-mode run may be useful
for diagnosis but must be labeled separately.

## Next-alpha comparison and interactive evidence

For a candidate alpha, run the same command at both exact source SHAs on the
same host and record unedited stdout. Compare each tool/size/profile row; identify
regressions from measured baseline distributions and profile CPU/allocation
changes before proposing any threshold. Backend may change data structures or
implementation as long as the observable contract and fixture remain fixed.
Changes to fixture semantics, public API coverage, or the number of dabs per
update must be called out because they affect comparability.

This harness cannot measure the egui event loop, canvas compositing, tile upload,
GPU queueing, vsync, OS scheduling, or input-to-photon latency. A separate
interactive run on the target Galaxy Book2 Pro 360 must record physical display
refresh, Windows build, GPU driver, app SHA, actual 16/64/128/256 px pen strokes,
and CPU/GPU/memory telemetry. Measure pointer contact to presented canvas
response with a repeatable external capture or a validated app presentation
trace; report distributions and visible stutter observations. Mouse or CI
results do not establish stylus comfort or hardware acceptance.
