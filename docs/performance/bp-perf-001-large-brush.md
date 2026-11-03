# BP-PERF-001 — Large-Brush Raster Profile

Status: measured engine correction in progress on `perf/large-brush-raster`.
The unmodified comparison baseline is `1559122943e8b6e9123b653a555f72e7a894d3e6`.
This document records container measurements, not a physical-device acceptance.

## Reproduce

Use the pinned Rust 1.90 toolchain on a Linux host. The example runs five trials
after one warm-up and prints 40 deterministic scenarios. It profiles 16, 64,
128, and 256 px brushes, mouse and pen-like samples, Paint, Eraser, Smudge,
Color Mixer, Round/Pencil/Textured/Calligraphy/Airbrush presets, 4096 × 4096
documents, sparse four-layer and populated two-layer documents, plus selection,
mask, and symmetry cases.

```sh
rustc --version
lscpu
BP_BENCH_CPU='describe the host CPU and core limit' \
  cargo +1.90.0 run --release --locked -p paint-document --example brush_latency
```

The harness reports nearest-rank median, P95, and maximum across 485 events per
scenario (five 97-event strokes). It times `Engine::apply_with_mode` separately
from CPU `Engine::composite_tile`, and reports their sum as estimated CPU service
time. Stroke commit, undo+redo, and five native encodes of a populated 4096²
two-layer document are timed separately. Linux RSS/HWM comes from `/proc/self/status`.
An FNV digest covers the rendered dirty-tile pixels after each completed
transaction; identical repeated trials must have identical digests.

For a constrained single-core run on Linux, pin the same command to one allowed
CPU, for example `taskset -c 0 env BP_BENCH_CPU='same host, pinned to one vCPU'
cargo +1.90.0 run --release --locked -p paint-document --example brush_latency`.
Pin the benchmark to the same CPU when comparing source revisions. This is a
CPU-contention proxy, not a model of an Intel i7 or mobile processor.

To compare with the baseline, create a clean worktree at the baseline SHA and
copy this example plus `tests/large_brush_identity.rs` from the candidate commit
into it. Run the same command and host settings in both worktrees. Do not change
the fixture, event timestamps, pressure, brush spacing, or document state
between runs.

## Findings and correction

The profile identifies two repeated costs in the targeted paths:

- Dab evaluation looked up transaction tiles in `BTreeMap`s and performed
  `Arc::make_mut` checks for covered pixels. Smudge also resolved the selected
  and transaction tile maps while sampling every pixel.
- `composite_tile` cloned a full 16 KiB source tile into temporary storage for
  ordinary raster layers, then blended every pixel. Empty sparse layers also
  paid for a zero tile and a no-op blend.

The correction walks the affected pixel bounds tile-by-tile and retains one
mutable transaction tile for each touched tile in Paint, mask paint, Color
Mixer, and Smudge. Smudge sampling resolves the source tile once per tile. CPU
composition now blends immutable stored raster tiles directly into the output
and skips layers that have no raster tile, active raster preview, or vector
content. A transparent-destination Normal blend fast path avoids divisions;
the opaque-source shortcut was rejected because integer rounding differed by
one in some channels.

The pixel math and within-tile pixel order are unchanged. The regression suite
locks 20 complete rendered outputs from the baseline across four sizes and five
operations, including pressure, tilt, selection, symmetry, masks, and commit.
The full 40-scenario benchmark also reports **40/40 matching digests** between
the baseline and optimized source.

## Results

Host: AMD EPYC 9V74, five vCPUs exposed to the Linux container, Rust 1.90.0,
release profile. The table uses pen-like pressure/tilt input with a 4 px sample
step at 240 Hz, Round brush except the Eraser preset, and the workload described
above. Each cell is baseline → optimized, in microseconds. Service median is
often near zero for 64–256 px because 0.15 spacing means many 4 px events do not
place a dab; P95 and maximum include dab-bearing events. This is why the engine
and composite columns are shown separately.

| Mode | Size | Engine apply P95 | CPU composite P95 | CPU service median / P95 / max |
|---|---:|---:|---:|---:|
| Paint | 16 | 19 → 21 | 1090 → 132 | 524.9 / 1104.6 / 2088.3 → 69.4 / 153.6 / 370.9 |
| Paint | 64 | 126 → 135 | 1022 → 138 | 0.6 / 1143.5 / 3018.4 → 0.4 / 261.6 / 507.1 |
| Paint | 128 | 424 → 444 | 1068 → 183 | 0.3 / 1573.6 / 2176.7 → 0.3 / 633.7 / 1041.9 |
| Paint | 256 | 1124 → 1259 | 3007 → 411 | 0.3 / 4137.1 / 7090.8 → 0.3 / 1670.1 / 3630.2 |
| Eraser | 16 | 19 → 21 | 1008 → 151 | 480.7 / 1027.8 / 2173.5 → 78.8 / 167.4 / 457.5 |
| Eraser | 64 | 135 → 135 | 952 → 165 | 0.7 / 1069.9 / 2583.4 → 0.5 / 292.0 / 450.3 |
| Eraser | 128 | 410 → 422 | 943 → 202 | 0.3 / 1405.6 / 1859.4 → 0.3 / 642.8 / 1035.4 |
| Eraser | 256 | 1265 → 1141 | 2852 → 557 | 0.3 / 3999.6 / 8927.5 → 0.3 / 1684.8 / 3850.6 |
| Smudge | 16 | 34 → 23 | 839 → 66 | 418.4 / 865.5 / 5003.7 → 39.8 / 87.8 / 204.5 |
| Smudge | 64 | 220 → 144 | 982 → 65 | 0.9 / 1151.4 / 2229.8 → 0.4 / 196.5 / 437.8 |
| Smudge | 128 | 598 → 435 | 857 → 66 | 0.3 / 1570.5 / 2074.7 → 0.3 / 502.4 / 909.7 |
| Smudge | 256 | 1753 → 1141 | 2363 → 174 | 0.3 / 4184.7 / 8172.9 → 0.3 / 1346.6 / 4117.1 |
| Mixer | 16 | 26 → 19 | 869 → 81 | 415.8 / 885.0 / 1328.9 → 37.5 / 99.2 / 284.0 |
| Mixer | 64 | 173 → 125 | 866 → 63 | 0.7 / 1000.7 / 1399.8 → 0.4 / 182.3 / 428.2 |
| Mixer | 128 | 537 → 451 | 852 → 71 | 0.3 / 1475.5 / 1913.3 → 0.3 / 523.5 / 869.7 |
| Mixer | 256 | 1631 → 996 | 2588 → 209 | 0.3 / 4560.0 / 8940.8 → 0.3 / 1197.0 / 2969.5 |

The 256 px representative pen case reduced P95 estimated CPU service by about
60% for Paint, 58% for Eraser, 68% for Smudge, and 74% for Color Mixer in this
run. Under one-vCPU pinning, the same 256 px scenarios measured service P95 of
4.11 → 1.44 ms (Paint), 4.13 → 1.66 ms (Eraser), 4.09 → 1.19 ms (Smudge), and
4.00 → 1.26 ms (Mixer). The one-core run still showed a synthetic backlog in
some larger preset/mask/symmetry cases; across all 40 scenarios the maximum
modeled backlog fell from about 14.0 ms to 6.9 ms. All samples are processed by
the harness; this model does not prove how a real OS input queue behaves.

For the populated 4096² two-layer fixture, interaction RSS/HWM was about 85 MB
for both revisions. Sparse 256 px scenarios were about 11–12 MB. Five repeated
native encodes produced the same 83,968,507-byte document. In one paired run,
encode median/P95/max was 100.888/102.249/102.249 ms baseline and
98.747/100.535/100.535 ms optimized. A later optimized repeat measured
104.708/181.928/181.928 ms. With only five encode samples and this run-to-run
spread, no save-time improvement is claimed; encoding was not optimized here.
The process HWM reached about 249 MB in both revisions, while RSS after encoding
was about 167 MB.

The benchmark's `upload_bytes` is a full-tile byte estimate from dirty tile
notifications, not a GPU timing. For the representative 256 px paint scenario
it was about 10 MB across the replay, unchanged by this correction. The existing
offscreen GPU smoke passed using Mesa llvmpipe (CPU software renderer). It tests
pixel regions, not large-brush speed.

## What this profile does not establish

- The container has no display server or `xvfb-run`; it did not profile actual
  window frames, swap/presentation latency, real GPU texture transfers, or the
  input event queue. The app's rendered-frame timing was not observed.
- One-CPU pinning on an EPYC host is not a substitute for Iris Xe, Windows, or
  Android hardware. No physical device or dedicated-GPU benchmark was run.
- RSS is process-wide and does not count per-dab allocations or GPU memory
  separately. Tile-allocation reduction is inferred from the code path, while
  RSS remained bounded and comparable in these fixtures.
- The harness does not separately instrument individual blend arithmetic,
  allocation counts, or tile-clone counts. Engine apply time includes dab
  evaluation and pixel writes; composition is timed separately.
- The synthetic queue estimate adds synchronous engine plus CPU composite
  duration against 240 Hz arrivals. It does not claim actual event drops or
  prove physical stylus latency.

Windows S Pen and Iris Xe, Android, and real presentation/frame-time validation
remain outstanding. PR #9's save-before-close and artwork-preservation work is
unrelated and remains an independent draft pending owner acceptance.
