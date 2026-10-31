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
- Latest three-platform Actions run for code checkpoint `e3665de`:
  [`40059871822`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40059871822),
  successful on Windows, Linux, and macOS, including the separate Windows
  diagnostic and canvas artifact builds.
- At resume, local HEAD was `069802c` (documentation only) and origin was
  `47996f6`; both were retained and pushed with recovery checkpoint `299506e`.
- Actions run [`40104914603`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40104914603)
  for code checkpoint `6001265` completed successfully on Windows, Linux, and macOS.
  Its Windows artifacts `rust-paint-lab-windows-diagnostic` and
  `rust-paint-lab-canvas-demo` are available and unexpired. The matrix includes
  bounded compaction, writer-exclusion tests, checked sequence overflow, and clearer
  recovery fallback messaging. Earlier runs are superseded checkpoints.
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
The real alpha window workflow still passes. The user has since verified on a
Samsung Galaxy Book2 Pro 360 that leaving the artboard no longer makes a live
stroke disappear. This confirms the reported artboard-exit behavior on that
device only; capture theft, focus loss, pen pressure/tilt interpolation, DPI edge
cases, and storage failure behavior remain unverified on physical Windows hardware.

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
  locally. Run `40059871822` completed successfully on all three platforms. Its
  Windows leg built both portable applications and uploaded both artifacts. The
  Windows artifact is CI output, not physical hardware validation.
- Physical Windows stylus/capture and modest-hardware performance: not tested here.
- The user separately verified on a Galaxy Book2 Pro 360 that artboard exit no
  longer makes a live stroke disappear. This confirms only that reported boundary
  behavior on that device; no other pen/capture/storage behavior is inferred.
- Recovery journal checkpoint local validation: formatting, Clippy, all workspace
  tests (141), build, required Vulkan GPU pixels, forced-crash alpha workflow, Linux
  live mouse X11 smoke, and Windows GNU target check/Clippy passed for `6001265`.
  GitHub Actions `40104914603` then passed all three desktop platforms, including
  software GPU and Linux app workflows plus both portable Windows package builds.
- Phase 7B continuation adds deterministic failures after torn-tail truncation,
  partial append, and before atomic compaction replacement. The real alpha/X11
  integration smoke now waits until the app's durable journal record matches the
  committed recovery checkpoint, kills the process, removes only the paired
  recovery snapshot, and invokes the app's recovery workflow from the journal. It
  verifies exact recovered bytes and confirms an independently saved `.rpl` remains
  byte-identical. Existing `.rpl`/`.bak` fallback and v1 reader are unchanged.
- Journal benchmark (Linux container, release build, 4096² single-layer fixture;
  Linux `/proc` RSS sampling): sparse one-tile payload 16,824 bytes, each record
  grows the journal by about 16.9 KB, writes 98/104 µs, RSS 1,468→1,780 KB. Dense
  4096-tile payload 67,174,824 bytes, each record grows the journal by 67.2 MB,
  writes 162/381 ms, RSS 67,388→199,328 KB, peak RSS 264,888 KB. Dense timing and
  memory include full snapshot encoding, temporary state clone, checksum, journal
  read/validation, and durable write; these are environment measurements, not
  target-device guarantees. Dense checkpoints can transiently require multiple
  project-sized buffers, so low-memory stress testing remains outstanding.
- Linux smoke passed with real application recovery after the forced process kill;
  it also starts with a persisted external PNG reference and checks that the board
  remains workspace metadata while PNG export stays in document coordinates. This
  is app workflow coverage, not Windows power-loss validation.

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
  snapshot recovery, and fault-injection exist. New work adds a bounded append-only
  journal of complete checksummed native snapshots. It is integrated into recovery:
  checkpoints are journaled before replacing the `.rpl` snapshot, so an interrupted
  replacement remains recoverable. Identical state records are idempotent; truncated
  or corrupted tails return the previous complete record; journal-only recovery is
  discoverable; the existing snapshot and `.bak` remain fallbacks. The journal is
  capped at two maximum project payloads and compacts atomically to the newest full
  state when needed. Injected partial-append, tail-truncation, and pre-replace
  compaction failures preserve a previous valid record. Linux process-kill
  integration now recovers from journal-only state and proves an existing artwork
  file is untouched. This is not an operation WAL, does not retain undo history, and
  still has a checkpoint interval in which a hard crash can lose recent work.
  Append calls sync journal file data, but physical Windows filesystem behavior,
  directory-entry durability, sudden power removal, disk controller caches, and
  full-disk behavior remain unverified. Legacy snapshots and `.bak` remain enabled.
  Do not claim comprehensive crash/power-loss protection.
- **7C Workspace (first slice):** light/dark theme and brush-panel width persist in
  `workspace.json`; the resizable brush panel remains egui-native. A reference board
  accepts up to six external PNG/JPEG file paths, opens independently draggable and
  resizable windows, persists their paths/placement/size/opacity, and excludes these
  images from document/export state. Decode/file/texture bounds are enforced.
  Automated Linux startup/export coverage and preference schema tests pass. Still
  missing: native file picker, dock/snap controls, panel layout modes, low-memory
  aggregation testing, accessibility audit, and physical touch interaction.
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
- **7N Distribution:** latest CI builds the separate portable Windows diagnostic,
  canvas demo, and Linux/macOS alpha packages. Android is not a functional app.
  Physical Windows and Mac hardware are untested.

## Known issues and exact next steps

1. Run full workspace, GPU, and target checks, commit and push this verified slice;
   check GitHub Actions and preserve the Windows portable canvas artifact.
2. Add pixel-level noninclusion verification to the Linux reference-board smoke and
   manually exercise multi-window drag/resize/opacity persistence on Windows.
3. Continue 7C with dock/snap affordances and a native image picker; then improve the
   brush/layer workspace. Keep the reference board outside document/export state.
4. Test recovery under Windows file replacement and power-loss simulations where
   safely available. Preserve `.rpl`/`.bak` fallback until that evidence exists.

Do not describe hardware behavior, Windows runtime behavior, journal durability, or
low-end performance as verified until tested on the corresponding hardware and
failure modes.

### Current continuation validation (working tree)

Formatting, workspace Clippy with warnings denied, all **148 Rust tests**, workspace
build, Windows GNU target check and target-specific Clippy passed after the 7B/7C
changes. The required Vulkan offscreen GPU pixel suite passed on Mesa llvmpipe. Linux
X11 alpha recovery smoke passed journal-only forced-kill recovery, persisted reference
startup, independent artwork preservation, and normal save/export workflows. The
journal Windows unit-test paths now combine wall-clock entropy with a process-local
atomic suffix after the hosted runner exposed a temporary-path collision. Commit
`26b3173` passed GitHub Actions run
[`40112079927`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40112079927)
on Windows, Linux, and macOS. CI uploaded `rust-paint-lab-windows-diagnostic`,
`rust-paint-lab-canvas-demo`, `rust-paint-lab-Linux-X64-alpha`, and
`rust-paint-lab-macOS-ARM64-alpha`; artifacts expire after 30 days. The Windows
canvas package now contains both a concise `README.txt` and full `USER-GUIDE.md`.
No Windows hardware or sudden-power-loss test was available. CI packaging does not
establish physical device compatibility or power-loss durability.
