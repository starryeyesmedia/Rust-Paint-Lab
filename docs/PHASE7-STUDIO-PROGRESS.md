# Phase 7 Studio Overdrive progress

This document is the durable continuation point for the Phase 7 program. It
records implemented behavior separately from future goals; a successful build is
not evidence that a workflow is complete.

## Starting point and branch

- Repository: `starryeyesmedia/Rust-Paint-Lab`.
- Working branch: `feature/studio-overdrive-phase-7`.
- Branch base: documentation checkpoint `fc1f0f8`, which follows validated code
  checkpoint `bdea96f` on `feature/overdrive-painting-alpha`.
- First Phase 7 implementation checkpoint: `0fce7fa` (artboard clipping and
  transaction continuity).
- Duplicate pen Downs outside paper are ignored without cancelling an existing
  same-ID contact in follow-up commit `43009eb`.
- The Phase 5 X11 smoke had an obsolete paper-exit cancellation expectation; its
  updated artboard-reentry and focus-cancel checks are in `51cfcef`.
- Latest inherited three-platform Actions run: `40056494303`, successful on
  Windows, Linux, and macOS. The Phase 7 branch has not yet run CI.
- No merge, release, or public exposure is authorized.

## Completed checkpoint 7A: artboard crossing and stroke continuity

The app now distinguishes leaving the paper from losing the actual input stream.
Pointer-down outside the artboard is ignored. For an active pen or mouse contact,
each client-space segment is clipped against the transformed document bounds before
it enters the shared Processor/Router pipeline. The contact and `StrokeId` remain
active through exit and re-entry. The new `BreakStrokeSegment` command starts a
disconnected subpath without committing or cancelling the transaction. The demo
scene preserves the subpath boundary; the raster brush resets interpolation and
dab spacing there. Off-document pixels are clipped. Focus loss, capture loss,
invalid input, resize/mode change, and leaving the application window still cancel.

Pen pressure and each supported tilt axis are interpolated at generated boundary
samples only when both actual endpoints contain that capability. If either endpoint
is unsupported the clipped sample stays unsupported; zero remains a valid value.
Generated intermediate samples carry interpolated normalized milliseconds but no
`SourceCounter`, since they are not exact native clock readings. Mouse pressure/tilt
remain `None`. Actual native pen samples and their source counters are otherwise
unchanged. No additional event loop or capture behavior was introduced.

Automated coverage includes clipping after zoom/pan, re-entry under the same stroke
identity, true cancellation, document pixels across disconnected segments, demo
geometry, native pen travel around an off-paper margin with disconnected geometry
and exact-counter preservation, and an offscreen GPU pixel assertion for both
sides of an unpainted gap.
The real alpha window workflow still passes. No physical Windows boundary, capture,
pressure interpolation, DPI, or Iris Xe test has been performed in this checkpoint.

### Validation on this checkpoint

- Full local validation of `e3665de`: formatting, Clippy, all **136 tests**, build,
  required Vulkan GPU runner, Windows GNU target check/Clippy, and updated Linux
  X11 live-window smoke all passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: passed, **136 tests**.
- `cargo build --workspace --locked`: passed.
- Required Vulkan offscreen GPU runner on Mesa llvmpipe: passed; it verified the
  artboard exit/re-entry gap remains unpainted.
- Linux/Xvfb `scripts/alpha-workflow-smoke.py`: passed (native save/reopen, edit,
  PNG/OpenRaster paths and forced-process recovery workflow).
- Windows GNU target workspace check and Clippy: passed. This is cross-checking,
  not Windows linking or execution.
- `40059380023` exposed the outdated Linux X11 paper-exit cancellation expectation;
  the smoke now checks paper re-entry plus separate focus cancellation and passed
  locally. CI run `40059871822` for latest code `e3665de` is in progress on Windows,
  Linux, and macOS. The last successful API status query showed macOS completed
  successfully and Windows/Linux in their workspace test steps. Subsequent GitHub
  API calls returned HTTP 401 because the environment's `GH_TOKEN` was rejected,
  so the final Windows/Linux status could not be read here. Recheck run
  `40059871822` before relying on CI. Earlier runs were stopped after being
  superseded; only `40059871822` is the final matrix run for this code.
- Physical Windows stylus/capture and modest-hardware performance: not tested here.

## Architecture decisions

- Artboard clipping is in the platform-facing canvas adapter: it owns current view
  transforms and knows whether a contact is inside the document. Shared input
  lifecycle and stroke routing remain OS/window independent.
- `BreakStrokeSegment` preserves one transaction while explicitly preventing
  interpolation across off-paper movement. It is not a successful completion and
  cannot enter history by itself.
- `paint-document` continues to own provisional scratch tiles until Commit; Cancel
  discards them. The renderer displays valid provisional content and never bakes it
  irreversibly into committed pixels.
- The app continues to use one winit event loop. Existing Windows pointer and
  capture adapters remain isolated.
- All creative functionality remains free. No cloud, account, telemetry, tracking,
  ads, paywall, or required network service is permitted.

## Partial and unimplemented checkpoints

- **7B Artwork preservation:** native save, verified backup, bounded per-document
  snapshot recovery, and fault-injection exist. A durable operation journal with
  replay of completed edits is not implemented. Explicit saving/export still has
  synchronous UI paths; do not replace snapshot recovery until a journal has passed
  interrupted-write, duplicate-operation, corruption, and real-workflow tests.
- **7C Workspace:** current alpha has functional controls, but the egui panel layout
  remains developer-oriented; no Draw/Illustrate/Animate workspace system, docking,
  native file picker, or system-theme following.
- **7D Document v2:** v1 raster document/layers/history are implemented. No editable
  vector/live-media/animation content variants or migration to a new format version.
- **7E Vector:** no editable vector engine or SVG import/export.
- **7F Brushes:** ten deterministic presets and custom `.rplbrush` IO exist; no
  textured tips, scatter/jitter, pressure response curves, searchable libraries, or
  thumbnail workflow. Latency has not been measured on target hardware.
- **7G Editing:** raster selection/transform, layers and undo/redo exist. Groups,
  masks, clipping masks, feathering, clipboard, and transform handles are absent.
- **7H Watercolor:** the Wash preset is not a live-media simulation. No wetness,
  diffusion, drying, or saved simulation state.
- **7I/7J Animation:** no frame timeline, cels, onion skins, or keyframes.
- **7K Interoperability:** PNG and OpenRaster are supported. JPEG, WebP, TIFF,
  animation exports, and PSD remain absent/unverified.
- **7L Timelapse:** no automatic or opt-in edit-event timelapse system exists.
- **7M Performance:** sparse tiles and bounded history/cache exist, but 4/8/16 GB
  profiles and target-machine frame/input latency are not benchmarked.
- **7N Distribution:** desktop CI and portable Windows artifacts exist on Phase 6;
  Phase 7 artifact workflow must be confirmed after push. Android is not a functional
  app. Physical Windows and Mac hardware are untested.

## Known issues and exact next steps

1. Push the verified 7A checkpoint and confirm the new Windows/Linux/macOS Actions
   matrix; inspect the Windows canvas and separate stylus diagnostic artifacts.
2. Perform the boundary acceptance exercise on Windows: start inside, exit each
   artboard edge while remaining in the app, travel around an outside margin,
   re-enter, release; confirm no disappearing preview or bridge. Repeat with native
   S Pen and inspect pressure/tilt at the clipping edge. Separately test focus,
   actual capture theft, and leaving the window still cancel.
3. Continue 7B only after reviewing `paint-document` storage/recovery and the current
   app close/save path. Prototype an append-only, versioned operation journal with
   bounded record sizes and checksums. First support a small set of deterministic
   committed operations and replay into a scratch document; keep snapshots as the
   recovery fallback until crash injection proves correct recovery and idempotence.
4. Add tests for journal truncation/corruption, repeated checkpoint replay, operation
   ordering, writer conflicts, cleanup, and failure preserving the previous valid
   project. Extend `scripts/alpha-workflow-smoke.py` to force-kill after a durable
   journal append and verify recovery through the actual UI.
5. Only then evaluate background saving/progress and continue toward 7C/7D. Keep the
   v1 reader compatible and never rewrite a user's source project during migration.

Do not describe hardware behavior, Windows runtime behavior, journal durability, or
low-end performance as verified until tested on the corresponding hardware and
failure modes.
