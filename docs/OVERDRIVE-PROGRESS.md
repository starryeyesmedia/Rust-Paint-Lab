# Overdrive painting alpha — durable checkpoint

Baseline: e29a98c, Phase 5; GitHub Actions run 40047220819 succeeded on all three desktop platforms. Branch: feature/overdrive-painting-alpha. No release or merge authorized.

## Completed before Overdrive
67 tests, shared lifecycle/replay/router, mouse canvas, GPU pixel checks, Linux real-event window tests, standalone native pen diagnostic, Windows portable artifacts.

## Validated checkpoints

- Phase 5 baseline e29a98c; Actions 40047220819, Windows/Linux/macOS success.
- Native pen checkpoint d97c3c4; Actions 40048248605, all three platforms success;
  70 Rust tests plus required software GPU runner. Physical integration tests pending.
- Raster/document/UI checkpoint: local validation below; matrix pending push.

## Architecture decisions

Preserve input/router and the standalone diagnostic. Native APIs stay in the
canvas binary; shared raster/document logic has no OS/GUI/GPU dependency. Use one
useful `paint-document` crate with brush/storage/recovery modules. No empty planned
crates. Egui controls consume the existing command pipeline, not an alternative
painting path. No telemetry, accounts, cloud service, signing or release. Original
project distribution license remains undecided.

## Checkpoint: raster/document/workspace vertical slice
Implemented and locally exercised: sparse 64×64 CPU tiles; antialiased deterministic
round/ink/pencil/marker/airbrush/soft/textured/calligraphy/eraser presets plus an
explicitly experimental wash; configurable size/opacity/flow/hardness/spacing/
pressure response/tilt angle/smoothing/rotation; independent scratch transactions;
editable layers with rename/duplicate/order/visibility/lock/opacity and Normal/
Multiply/Screen/Overlay; bounded COW snapshot undo/redo; `.rpl` editable save/open,
checksum validation, previous-file backup, locking/conflict detection and fault
injection; background snapshot recovery and close/destructive-action safeguards;
PNG import/export; functional egui brush/color/layer/file controls, dark/light,
keyboard history/save/export and fullscreen. Dirty tiles are composited/uploaded,
not the whole document per small stroke. The original mouse/replay path remains
available using `--compat-mouse` / `--exit-after-demo` for regressions.

GUI decision: egui/egui-winit/egui-wgpu 0.33.3 requires wgpu 27 (Rust 1.88), whereas
newer egui requires Rust 1.92. Use wgpu 27.0.1 with existing Rust 1.90; rerun GPU
regressions. No custom/forked GUI renderer. Font and dependency notices are bundled.

Validation: 17 document-core tests plus earlier 70 tests (87 total); formatting,
workspace Clippy/build and Windows GNU cross-target lint succeeded. GPU runner
passed raster preview/cancel/commit/undo/redo pixel checks. real X11 alpha
workflow passed painting -> save checksum -> undo/redo -> backup -> PNG -> reopen
-> close on Mesa software Vulkan. This is not physical S Pen or Windows file-system
validation. Latest fully CI-validated checkpoint remains d97c3c4 until this slice
passes the complete matrix. 

## Measured performance — cloud CPU only

Rust 1.90 release, AMD EPYC 9V74 cloud host, single run, synthetic 1000 horizontal
200px strokes, size12 pressure0.7, one layer. `cargo run --release --locked -p
paint-document --example metrics` reproduces the workload. 2048²: 799ms raster,
17ms encode, 16ms decode, 1000 tiles, 16,400,424-byte native file, 8 retained history
steps. 4096²: 827ms raster, 66ms encode, 65ms decode, 3615 tiles, 59,286,424-byte file,
2 retained history steps. Encode/decode are memory operations, not disk-save latency.
Undo/redo loops measured below 1ms at integer-ms precision. These are observations,
not hardware FPS/pen latency/GPU timings or a robust statistical benchmark.

Known issue: conservative snapshot history accounting counts shared references,
so large documents retain very few steps. Improve unique-tile accounting/history
patches after preserving correctness. No physical frame-time/memory numbers claimed.

Partial/missing P0 details: native timing is still ms in shared samples; no
prediction; recovery is bounded snapshots, not a complete operation journal; no
automatic recovery retention cleanup or recent-documents list yet; Save As uses
path entry and refuses unrelated overwrite. Advanced accessibility semantics,
touch navigation, selections/transforms, groups/masks, editable vectors, timelapse,
OpenRaster/other formats, Android, and video export remain unimplemented. Wash is
not physical watercolor simulation. UI editing/painting hardware tests are pending.


## Exact continuation

1. Push the raster/document/UI checkpoint, verify Windows/Linux/macOS Actions and
   download both canvas and diagnostic artifacts. Fix matrix failures before expansion.
2. Test the native pen canvas and project durability using the user guide on Galaxy
   Book2 Pro 360; synthetic/CI success does not establish this hardware behavior.
3. Improve bounded history accounting and recovery retention/recent documents;
   add randomized malformed native-file tests and disk/PNG overwrite regressions.
4. Extend usable editing next: selections/fill/transform with undo, then groups and
   OpenRaster. Touch navigation/timelapse follow stable P0 workflows.
5. Editable vectors, JPEG/WebP/TIFF/PSD investigation, Android functional APK and
   video remain later P2 work. Do not count them as delivered.
