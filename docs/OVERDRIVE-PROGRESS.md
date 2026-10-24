# Overdrive painting alpha — durable checkpoint

Baseline: e29a98c, Phase 5; GitHub Actions run 40047220819 succeeded on all three desktop platforms. Branch: feature/overdrive-painting-alpha. No release or merge authorized.

## Completed before Overdrive
67 tests, shared lifecycle/replay/router, mouse canvas, GPU pixel checks, Linux real-event window tests, standalone native pen diagnostic, Windows portable artifacts.

## Validated checkpoints

- Phase 5 baseline e29a98c; Actions 40047220819, Windows/Linux/macOS success.
- Native pen checkpoint d97c3c4; Actions 40048248605, all three platforms success;
  70 Rust tests plus required software GPU runner. Physical integration tests pending.
- Raster/document/UI checkpoint d322c55: 87 Rust tests plus raster GPU checks;
  Actions 40051472347 Linux/macOS succeeded, Windows still running at this update.

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


## History and storage hardening checkpoint

Unique Arc tile allocations now count once per history queue. Both benchmark
workloads retain 32 steps instead of 8/2. This trades bookkeeping CPU time for
usable history: same release workload measured 1171/1649ms raster at 2048²/4096²,
18/70ms encode, 18/65ms decode, 13/59ms for 32 undo plus 32 redo operations.
No statistical/hardware latency claim. Future optimization: incremental reference
accounting or patches, after profiling. Snapshot queue remains capped at 128 MiB
unique tile payload; metadata adds bounded overhead (32 states × 32 layers).

Four additional tests cover shared allocation accounting, checksum-valid malformed
metadata plus deterministic mutation corpus, cooperative writer locking, and PNG
no-overwrite. Workspace now has 91 Rust tests plus separate required GPU runner.
Drawing failures now surface a document-specific status, not only a legacy capacity
notice. New documents reset the default save path; recovery keeps distinct per-
document lineages. Artifact workflow additionally produces native Linux/macOS tar
packages with architecture/build ID, system import list and target-specific notices;
these still require system graphics/desktop libraries and are unsigned/unnotarized.

Pending P0 refinements: recent documents, recovery retention UI, full operation
journal and asynchronous explicit save. Current snapshot recovery is usable and
fault-tested but these refinements are not claimed. Do not silently delete recovery.

Windows CI correction: d322c55 Linux/macOS succeeded, Windows failed in tests
(after GPU pixel success). Full logs are blocked by the environment's GitHub logs
host; check annotations expose only the exit code. Code review identified read-only
handles passed to sync_all for backup/PNG files, which Windows FlushFileBuffers
requires write access for. Changed those to write handles; native matrix rerun is
required to confirm. Added bounded failed-test tail annotations so exact errors
remain accessible through the GitHub API. Never count this failed run as success.

## Selection/editing checkpoint

Implemented rectangle/ellipse/lasso selection gestures and outlines, brush selection
clipping, fill/delete, composite eyedropper, selected-layer translation/uniform
scale/rotation/flips and all-layer crop. These are real document operations with
undo/redo, locks, nonfinite/singular transform rejection and bounded masks. Crop uses
selection bounds, transforms use nearest neighbor and clip outside-canvas content.
No feathering, transform handles or clipboard cut/paste. Selection state participates
in session history but is deliberately not persisted by native v1.

Seven core tests assert boundaries, lasso mask agreement, pixel edits/history,
transforms, crop, eyedropper/locks and brush clipping/cancellation. Full suite now
98 Rust tests plus GPU runner; local formatting/Clippy/build and Windows cross-lint
pass. Real alpha OS workflow passed rectangle-select -> fill -> save -> undo -> save,
restoring byte-for-byte original project data. Hardware UX still pending. Recent
files, recovery cleanup/journal, groups/masks, navigation, interchange and P2 remain
incomplete. Verify the next CI run before treating the checkpoint as matrix-validated.

## Navigation/safety checkpoint

Added bounded physical-pixel zoom/pan mapping with workspace clipping, cursor-
anchored wheel zoom, middle-button pan, fit control and two-contact touch deltas.
Touch navigation is suspended during active painting; single/third contacts do not
paint. Four synthetic tests cover anchor math, zoom/pan limits, two-contact state,
orphans/duplicates/reuse, long streams, nonfinite data and inverse/clip mapping.
Physical touch behavior is NOT TESTED. Navigation is not a full palm classifier.
One additional storage test checks valid noncanonical JSON opens/saves correctly;
conflict detection now hashes actual file bytes, not a reserialization. PNG publish
uses an atomic no-clobber hard link so concurrent exports cannot replace artwork;
filesystems without hard links will report an error, not overwrite a target.

103 total Rust tests plus software GPU runner pass locally. Real alpha workflow
also exercised wheel zoom/fit without changing saved artwork. Windows write-handle
fix run 40051986737 passed native tests and reached release canvas compilation;
Linux/macOS jobs succeeded and published native development artifacts. Verify final
matrix/artifacts after all changes; do not treat pending compilation as success.

## Recent documents and recovery maintenance

Added bounded local recent-project preferences with real UI open actions, plus
explicit confirmed recovery/backup deletion. Active owners and ordinary artwork
are protected; discovery retains only 20 paths in memory even in large directories
and filters active owners before selecting recent candidates. No automatic deletion.
Three additional tests cover preferences bounds/roundtrip/corruption, active-owner
cleanup refusal and bounded discovery with inactive older files. Current total:
106 Rust tests plus GPU runner. UI code is split into widget construction and frame
execution for readable formatting and clearer action ownership.

P0 gaps remaining: snapshot recovery rather than operation journal, asynchronous
explicit save, advanced stabilizer/high-resolution stored native timing, and physical
Windows painting/storage certification. Not a finished production replacement.

Matrix confirmation: 276b76c / Actions 40051986737 succeeded on Windows, Linux and
macOS, including portable Windows canvas/diagnostic packaging and native Unix
packages. The write-handle correction resolved the Windows test failure. Newer
selection/navigation/preferences commits still require their own final matrix check.


## Restricted OpenRaster checkpoint

Added real layered OpenRaster export/import with UI and shortcuts. Preserve order,
names, visibility/opacity, implemented blends and PNG pixels; native background
becomes a layer. Warn about omitted locks/presets/history; reject unsupported groups,
masks, nonzero offsets and blend modes. ZIP directory is preflighted before parser
allocation; XML/PNG/decompression/allocation limits, CRC, no extraction and no-clobber
publication apply. Maintained zip9.0.5 and roxmltree0.21.1 added after Rust1.90/MSRV
investigation. PNG alpha conversion now rounds premultiplication/unpremultiplication
so representable premultiplied channels survive interchange. Export refuses active
previews. Seven tests cover round trips, background, unsupported features, corrupted/
oversized directories, DTD/malformed input, alpha preservation and invalid XML names.

Local suite: 114 Rust tests plus GPU runner, formatting/Clippy/build, Windows cross-
lint; real window workflow now exports ORA, validates it with Python's independent
ZIP reader, imports it and saves a new editable native project. Full matrix pending
for this slice. Earlier selection/navigation runs 40052433677 / 40052797512 succeeded
on all three platforms. No external painting-app roundtrip/hardware claim.

Independent X/Y tilt now influences calligraphy azimuth when both axes are available;
single-axis/unsupported fallbacks remain explicit and raw samples remain unchanged.
One synthetic regression asserts different X/Y outputs. Final release benchmark on
the cloud EPYC host (one run, ordinary background load): 2048² 1474ms / 4096² 2274ms
for 1000 strokes; encode22/68ms, decode22/65ms, 32 undo+redo24/88ms, three durable
native saves202/686ms, flushed recovery34/119ms, PNG126/499ms. Combined process peak
RSS312848KiB via Linux resource.getrusage (not GPU/driver memory). Both retain32 undo
steps; history payload18,694,144 /60,702,720 bytes. Disk results are the cloud local
filesystem, not the user's Windows disk. Workload and measurements are reproducible
with the metrics example; no 60FPS/hardware latency assertion.

## Crash-recovery acceptance checkpoint

Added checksum-verified backup fallback with explicit lost-newer-edits warning and
recovery of orphaned `.bak` checkpoints when the primary is missing. Recovery opens
a new unsaved document, never rewrites the source. Ctrl+Shift+R exposes latest
recovery. Two more tests verify corrupt-both refusal, fallback preservation and
orphan discovery/cleanup (116 Rust tests total). The real Linux window test now
force-kills the app after a durable committed snapshot, restarts it, restores via
the actual UI shortcut, saves and asserts byte-for-byte checkpoint equality. This
is genuine process interruption testing, not a hardware/power-loss claim.

Dependency audit: cargo-audit0.22.2, RustSec DB84d943a6deeb8cc6c6f632531aaf4f1c62275239
(updated2026-10-23),315 packages: no known vulnerability advisories. Two informational
unmaintained warnings remain: paste1.0.15 (RUSTSEC-2024-0436, graphics macro chain),
ttf-parser0.25.1 (RUSTSEC-2026-0192, egui fonts). Do not claim a warning-free audit;
upstream GUI/font updates under pinned Rust1.90 need evaluation. No custom fonts
are imported by this app. Audit tool is external dev tooling, not a product dependency.

## Precise source-time checkpoint

Closed the earlier 6A timing-data gap: optional exact SourceCounter ticks/nonzero
frequency now survive PointerSample -> Processor -> Router -> StrokeSample commands,
including cancellation. Windows queries QPC frequency and attaches actual native
PerformanceCount. Legacy mouse and fixture v1 retain None. Synthetic fixture v2
supports explicit ticks/frequency or unsupported pairs without changing v1 behavior.
Four new tests cover sub-ms/zero/unsupported values, overflow-safe differences,
invalid fixture pairs, deterministic lifecycle and routing/cancellation preservation.
Current suite: 120 Rust tests plus required GPU runner, formatting/Clippy/build and
Windows cross-target lint pass locally. Diagnostic changed only to initialize the
new optional shared field (None); its native inspection behavior is unchanged.
No raw input is automatically persisted. Future velocity/stabilization can consume
precise times; the brush remains spatially spaced and native v1 stores raster art.
Native pen UX also exposes a bounded latest-sample status (including independently
unavailable tilt axes) and clipped hover diameter ring. Focus/interruption clears
feedback. Status occupies a reserved line so changing pen text cannot move the
canvas mid-stroke. Existing synthetic pen checks exercise metadata/cleanup; visual
native pen behavior remains NOT TESTED on physical hardware.
The native adapter's bounded history deduplicator is now independently testable
without OS APIs. Four tests cover overlap/stale packets, attribute changes at the
same counter, ID reuse/zero-counter rollover, and 100000 IDs with 128 retained
stamps maximum. Callback uses the tested helper; no capture/event-loop policy was
changed. Current complete suite is 124 Rust tests plus GPU runner. Native history
FFI delivery/order still requires physical pen validation; pure packet tests do not
substitute for it. OpenRaster checkpoint31965b8 / Actions40054016435 passed all3OS.

## Editing and save-path safety checkpoint (in progress)

Fixed three concrete alpha-shell hazards. Document/path/recovery/close/cleanup prompts
are now blocking egui modals so the workspace behind a destructive confirmation
cannot receive clicks. A lasso that exceeds the 1024-vertex budget is cancelled in
full and reports why; it can no longer silently commit a clipped outline. Saving an
untitled document after exporting PNG selects the corresponding `.rpl` path instead
of writing native project bytes with a `.png` suffix. Added three focused regressions
for lasso spacing/cap and native project path selection. README was corrected to
describe the actual painting alpha and its current limitations; user guide updated.

Local validation on Rust 1.90: fmt, workspace Clippy, workspace build, all **127 Rust
tests**, required Vulkan software GPU pixel runner (Mesa llvmpipe; passed), and the
interactive X11 alpha workflow passed. The latter verifies real window input, native
project roundtrip, edits, PNG/OpenRaster, and process-kill recovery. Windows GNU
cross-target check and Clippy passed; this is not Windows execution. Previous source
checkpoint `ed8b88a` Actions run `40055386097` completed successfully on Windows,
Linux, and macOS, with its existing artifacts. The changes in this checkpoint have
not yet been pushed or run through CI.

Known remaining limits are unchanged: native Windows pen path and physical file
durability are not tested here; recovery is snapshot-based; no full operation journal,
async explicit export/save, groups/masks, vectors, timelapse, Android app or full ICC
workflow. Exact continuation: commit and push this fix, verify the three-platform
Actions build/artifact uploads, then continue P0 reliability work after assessing
Windows results. Do not claim the new modal flow or path behavior was hardware-tested.

## Custom brush preset workflow (implemented; CI pending)

Connected the existing versioned Brush JSON schema to actual storage and workspace
controls. A user can save/load `.rplbrush` presets by typed path or Ctrl+Shift+B /
Ctrl+Alt+B. Storage validates the extension and settings, caps reads at 16 KiB,
validates schema version on load, and publishes via same-directory atomic no-clobber
hard links. Existing presets are preserved; choose a new filename to save another.
The alpha smoke workflow now drives both shortcuts through the real egui dialog,
loads the preset, saves the project, and compares the embedded brush settings. Two
storage tests cover roundtrip, existing target, size, extension, and invalid version.
No dependency was added.

At this point local format, Clippy, workspace build and all **129 Rust tests** pass;
the required Mesa llvmpipe Vulkan pixel runner passes. The updated interactive alpha
workflow passes with brush preset roundtrip included. Windows GNU cross-target check
and Clippy pass. Actions for the preceding f6ab0a1 checkpoint are still in progress;
this brush change has not yet been committed or covered by CI.
