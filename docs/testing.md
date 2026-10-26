# Automated input validation

Use the pinned Rust toolchain, from the workspace root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo run --locked -p paint-input --example replay -- crates/input/tests/fixtures/mixed.replay
```

The input crate contains seven normalization unit tests plus fourteen processing
integration tests. They exercise exact output sequences/data; simultaneous touch
and pen; hover/contact/lift; varying and zero/unsupported pressure/tilt; cancellation,
capture loss, interruptions and replay EOF; reused IDs and duplicate/unexpected
sequences; stale terminal cleanup; equal/stale timestamps and 32-bit Windows
rollover; invalid input and parser/version/resource limits; and bounded state over
50,000 generated events. Replay output is compared across repeated executions.
Tests do not sleep, read wall time, use network, or require hardware. Fixture
limits are tested at the parser boundary; long-stream tests do not retain outputs.
The native diagnostic binary itself has no automated hardware test suite.

`paint-stroke` adds nineteen routing/coordinate integration tests, for **40 total
workspace tests**. Existing fixtures exercise exact Begin/Update/Commit/Cancel
sequences, pressure changes, missing versus zero data, hover and barrel buttons,
pen/touch mixing, ID reuse, and interruptions/capture loss. A synthetic mouse
fixture covers primary-button gating and unavailable mouse pressure/tilt. Direct
lifecycle boundary tests cover duplicates, orphans, stale outputs, malformed
values, and independent transactions. Coordinate tests cover translation, zoom,
viewport changes during a stroke, invalid values, and arithmetic overflow.
Additional tests verify dispatch/source timing, bounded state over 10,000 strokes,
explicit router teardown, and identical command streams on repeated replay.
These checks do not implement or validate a renderer, brush engine, or document
consumer. Cancellation is asserted as a distinct command, never a Commit.

For focused development:

```sh
cargo test --locked -p paint-input --test processing
cargo test --locked -p paint-stroke --test routing
cargo run --locked -p paint-stroke --example stroke_commands
```

On Linux/macOS also check the Windows adapter without linking/executing:

```sh
rustup target add x86_64-pc-windows-gnu
cargo check --workspace --locked --target x86_64-pc-windows-gnu
cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings
```

Existing CI checks format/lint/tests/native builds on Windows, Linux, and macOS.
The Windows leg additionally builds and packages the release diagnostic; it does
not exercise a physical pen. The downloadable package includes an updated short
README, with hardware tests in [the full checklist](hardware-testing.md).
Do not infer actual Windows execution from cross-compilation, or pen behavior
from either native builds or synthetic replay. The Galaxy Book checklist is still
pending and is independent of completing the automated processing milestone.

## Phase 4 canvas checks

The canvas adds twelve hardware-independent tests, bringing the Rust suite to
**52 tests**. They cover the actual routed A/B/C fixtures, geometry and document
mapping, final samples and retained pressure, preview/commit/cancel transitions,
independent IDs/touch exclusion, nonfinite/range errors, fixed width/zero-length
segments, duplicates, restart determinism, and explicit sample/stroke/vertex limits.

```sh
cargo test --locked -p canvas-demo --test geometry
cargo test --locked -p canvas-demo --test gpu_smoke
cargo run --locked -p canvas-demo
```

The offscreen smoke runner is an additional custom harness, outside Rust's unit
passed-test count. It prints **GPU_SMOKE: PASSED** after real pixel checks or
**GPU_SMOKE: SKIPPED** if no adapter exists. After finding an adapter, device,
shader, rendering, readback, or pixel failures fail the command. Set
`RPL_REQUIRE_GPU=1` to make absence fail too; Linux CI does so with Mesa installed.
The runner records its actual adapter/backend and appends outcome to the Actions
step summary. Do not describe a skip as passing GPU validation.

Linux CI additionally runs the actual window under a virtual display:

```sh
xvfb-run -a cargo run --locked -p canvas-demo -- --exit-after-demo
```

The normal demo stays open; this testing flag exits only after replay completion
and presentation of the final frame. Use an external timeout in automation (CI
limits this step to two minutes). Software-rendered/offscreen/virtual-window tests
are not physical RTX 3060, Iris Xe, or stylus validation. The downloadable canvas
and stylus diagnostic remain distinct artifacts. See the [manual checklist](canvas-demo.md).

## Phase 5 live mouse validation

Fifteen mouse/session integration tests bring the hardware-independent Rust suite
to **67 tests**. They run the real sample/lifecycle/router/scene pipeline and cover
primary/secondary/hover/orphan input, final samples and unavailable pressure/tilt,
focus/capture/window exit, nonfinite/outside points, duplicate events, ID reuse,
resize and physical-pixel mapping, mode isolation, replay/clear semantics, and
nonfatal bounded-state cleanup. The GPU runner additionally checks live preview,
commit/cancel pixel regions and scissor clipping at the paper edge.

```sh
cargo test --locked -p canvas-demo --test mouse
cargo test --locked -p canvas-demo --test gpu_smoke
xvfb-run -a python3 scripts/live-mouse-window-smoke.py
```

The Linux window test needs libXtst (`libxtst6` on Ubuntu) in addition to the
existing display prerequisites. It injects real X11 button/motion/focus/key events
into the normal winit executable, checks transaction/mode feedback, and exits
cleanly. It does not use an application test-input shortcut or record user data.
Both virtual-window tests and required software-GPU validation remain in CI.

The isolated Windows capture subclass observes actual WM_CAPTURECHANGED/CANCELMODE,
while suppressing expected release during normal WM_LBUTTONUP. It is cross-checked/
linted for Windows and built in native CI; theft/release behavior still needs
manual Windows validation. Linux/macOS use winit focus/leave notifications; capture
stealing without either notification is not independently observed there.
See [Galaxy Book interactive checklist](live-mouse.md#manual-galaxy-book-checklist).


## Overdrive alpha validation

`cargo test -p paint-document --locked` tests brush determinism, pressure semantics,
opacity versus flow, endpoint smoothing, scratch cancellation/concurrency, layers,
bounded history, blend invariants, corrupted/truncated native files, injected save
failures, external-change conflicts, recovery ownership/flush, journal tail recovery,
idempotent checkpoint append, and PNG round trips.
The existing offscreen runner now also checks real raster shader pixels across
preview, cancellation, commit, undo and redo. A missing adapter remains SKIPPED;
Linux CI requires software GPU execution. Existing 70 Rust tests are preserved.

```
cargo run --release --locked -p paint-document --example metrics
xvfb-run -a python3 scripts/alpha-workflow-smoke.py
```

The latter injects real OS mouse/key events into the default alpha, verifies a
checksummed editable save, undo/redo tile results, backup, PNG dimensions and reopen.
It uses a temporary working/recovery directory. The older mouse window script uses
`--compat-mouse` to preserve its original regression contract. These tests are not
physical pen, Windows filesystem, Iris Xe/RTX frame-rate or tablet UX certification.

History hardening adds unique COW allocation accounting and 4 storage/history tests
(91 total Rust tests). Run the same workspace suite and target-specific license
notice generator when packaging additional desktop platforms.

Selection/editing adds seven portable tests (98 total). The real alpha window
workflow additionally selects a rectangle, fills, saves, undoes and verifies the
original native file is restored byte-for-byte. Physical pen editing remains untested.

Navigation and noncanonical-file regressions bring the suite to 103 Rust tests,
plus the separate GPU pixel runner. Touch gesture math is synthetic; physical
Windows touch/pen arbitration is still untested.

OpenRaster adds seven portable tests (113 Rust tests total) and independent Python
ZIP validation in the real window workflow. See openraster.md for deliberately
unsupported input and allocation protections. External app roundtrips are pending.

Independent X/Y tilt azimuth regression brings the current suite to 114 Rust tests.
The metrics example now measures actual durable saves, flushed recovery and PNG,
not only in-memory encode/decode. Cloud numbers are in OVERDRIVE-PROGRESS.md.

Precise counter/replay v2 adds four tests (120 current Rust tests total). V1 fixtures
remain unchanged except the unknown-version negative test now uses version 3.
Native QPC attachment is Windows cross-linted/built; precise timestamp behavior on
physical S Pen remains a manual check. No auto-recording of device input is added.

Four native raw-history identity tests bring the suite to 124 Rust tests. They are
portable adapter-helper tests, not mocked claims that Windows delivered hardware
history. Physical coalescing/capture/promotion behavior remains on the manual list.

## Current Overdrive workspace (2026-10-24)

Before the journal continuation, the suite contained **129 Rust tests** across input normalization and
lifecycle, stroke routing, canvas, native-history deduplication, raster brush/document,
editing, recovery preferences, OpenRaster, and brush preset IO. Three canvas unit tests
cover bounded lasso cancellation and native save-path selection; two document tests
exercise `.rplbrush` roundtrip, validation, size limits and no-clobber behavior. The
Linux/Xvfb alpha integration workflow also uses actual keyboard events to save a
preset, load it, save the project and compare brush metadata. This complements (not
replaces) the portable unit tests.

## Phase 7 artboard-crossing regression

Artboard clipping adds seven portable regressions (136 Rust tests total): client-space
segment clipping after zoom/pan, same-StrokeId subpath breaks, mouse and native-pen
exit/re-entry, cancellation remaining distinct, and document raster continuity.
The offscreen GPU runner now paints outside the artboard path and verifies the
outside gap remains unpainted while both in-paper segments render. Linux/Xvfb alpha
workflow remains the full save/edit/recovery integration check; physical Windows
pen pressure, capture behavior, display scaling, and Iris Xe rendering still require
manual testing.

The original recovery-journal checkpoint covered five tests (141 Rust tests at that checkpoint):
latest-state selection and idempotent duplicates, truncated/checksum-corrupt tail
fallback, discovery/restoration when its paired snapshot replacement is absent,
writer exclusion, and bounded compaction. The actual alpha smoke exercises forced
process termination with the integrated recovery worker; Windows power-loss behavior
and filesystem durability remain untested.

```sh
cargo test --locked -p paint-stroke --test routing
cargo test --locked -p canvas-demo --test mouse --test pen --test navigation
cargo test --locked -p paint-document --test document
RPL_REQUIRE_GPU=1 WGPU_BACKEND=vulkan cargo test --locked -p canvas-demo --test gpu_smoke
```

## Phase 7B journal and first 7C workspace slice

`paint-document` journal tests inject partial append, post-tail-truncation, and
pre-compaction-replacement failures. They require the last complete record to remain
restorable, and a corrupted journal never changes an independently saved native
project. The Linux/Xvfb alpha workflow waits for a complete journal record after a
real app edit, kills the process, removes the paired snapshot, starts the application
again, invokes Recover, and checks exact project bytes plus source-file preservation.
This tests process termination after the journal checkpoint. It does not simulate
Windows kernel/filesystem behavior or sudden power loss.

The metrics example has repeatable sparse/dense journal modes:

```sh
cargo run --release --locked -p paint-document --example metrics -- --journal-sparse
cargo run --release --locked -p paint-document --example metrics -- --journal-dense
```

RSS is sampled from Linux `/proc`; other hosts report it unavailable. Results in
`docs/PHASE7-STUDIO-PROGRESS.md` are for this container only. The alpha UI smoke
starts with a persisted external reference image and verifies the image remains a
workspace preference outside the exported document. `canvas-demo` unit tests cover
schema/version checks, geometry sanitization and six-image limits. Manual Windows
checks are still required for reference-window dragging, resizing, opacity, theme
and panel-width persistence, and integrated GPU memory.


The 7B/7C continuation adds three journal/storage regressions and four workspace
tests, for **148 Rust tests**. The new workspace tests cover preference JSON versioning,
geometry/count/file-size limits, and a real PNG-to-egui texture decode. The journal
smoke validates journal-only recovery through the application after a forced process
termination. The reference-board paths/layout are persisted separately from artwork.

## Phase 8.2 workspace and reference navigation

The Home recent list reads no full project into memory. It reads at most the
bounded native-format metadata header for dimensions/DPI and stores last-opened
UTC seconds in the separate, backward-compatible `recent.json`. Missing files are
shown as unavailable and cannot be opened from the stale card. The header peek is
display-only; complete project opening still verifies total size, schema, tile
structure and checksum. Tests cover old recent-list migration, timestamp/path
ordering, malformed header rejection, navigation preference bounds, reference
viewport clamping and focal-point zoom. `TouchTargetRouter` tests ensure contacts
stay on one surface and clean up before IDs can be reused.

Run the targeted tests with:

```sh
cargo test --locked -p paint-document recent_
cargo test --locked -p canvas-demo --lib reference_
cargo test --locked -p canvas-demo --test navigation
```

The tests exercise navigation math and surface ownership, not Windows Precision
Touchpad delivery, touchscreen gesture latency, palm rejection, DPI transitions,
or live egui hit testing on the Galaxy Book. Home now uses a bounded thumbnail
cache for real saved projects. The Linux Xvfb workflow checks that reopening a
saved project renders its cached thumbnail, captures that recent-artwork state at
1920 × 1080, and verifies visible blue stroke pixels from the smoke artwork in the
captured thumbnail. It also retains an intentional first-run empty state. These
checks exercise the virtual Linux display and fixture artwork, not a user library
or Windows DPI behavior.

## Phase 8 startup, document setup, and native file selection

`paint-document::document_setup` has hardware-free tests for pixel/inch/millimeter/
centimeter conversion, DPI behavior, finite/range validation, every included preset,
safe document names, exact created dimensions/background/transparency, and creating
an empty sparse state without allocating tiles. Run them with:

```sh
cargo test --locked -p canvas-demo document_setup::tests
```

The real alpha workflow starts from the welcome surface, creates a document through
Ctrl+N, then exercises drawing, save/open, presets, import/export, and recovery.
When running under Xvfb only, `RPL_TEST_PATH_DIALOG=1` enables a deterministic
path-entry harness because native desktop pickers cannot be assumed in a headless
session. Normal users use rfd native dialogs; this test harness is not an alternate
painting or document path. Physical Windows/macOS picker behavior still requires
manual checking.

The local draft manager tests verify that an unnamed document is stored as a normal
`.rpl`, can be reopened with matching dimensions, is discoverable after a restart,
and refuses to exceed the eight-draft bound. It ignores unrelated files. If the
bound or a write error prevents automatic close-save, the application remains open
and offers retry, Save As, or Cancel. Named-document external-change conflicts use
the existing `Store` content-hash guard and also keep the window open.

## Phase 8.3 accessibility foundation

`canvas-demo` tests high-contrast text and control-edge contrast in light/dark
palettes, persists bounded interface scale and high-contrast preferences, and
checks the shared 44-point egui interaction baseline. A generated AccessKit tree
test checks representative toolbar/document labels and winit routes AccessKit
actions through the existing application event loop. These are style, preference
and integration tests, not a complete WCAG audit or screen-reader pass. A UI-token
test verifies that the persisted reduced-motion setting sets egui transition time
to zero. See
`docs/accessibility-assessment.md` for untested Narrator/NVDA behavior.

The thumbnail generator test checks deterministic bounded RGBA output, alpha,
aspect ratio, invalid output limits, and refusal to snapshot provisional strokes.
It composites only the sampled pixels from sparse tiles rather than flattening a
full-size image. The Home cache test checks bounded PNG
decode, cache-key invalidation and pruning to eight files. Run:

```sh
cargo test --locked -p paint-document --test document home_thumbnail
cargo test --locked -p canvas-demo --lib home_thumbnails
```

The artwork recovery manager exposes validated inactive snapshots from Home and
File, shows bounded header metadata and size, and routes Recover through the
existing replacement/preservation gate. Removal remains confirmed and uses the
recovery module's ownership/path guards. A selected checkpoint can be previewed
on demand after the normal native loader validates it; pixel sampling is capped at
180 × 120 and the one retained texture is workspace-only. A unit test covers
valid and corrupted files. AccessKit tree tests also render the actual floating
Layers-panel contents and assert its layer actions and state labels. Its
position/size, Brush flyout position and Color flyout position preferences cover
migration defaults, finite checks, persistence and bounds.

The Linux alpha-window workflow drives Home, New Artwork, workspace, reference
board, Brush library, Color controls, Layers panel and Recovery manager, then
restarts the app to capture recent artwork and dark/high-contrast appearances.
It exercises the top-docked horizontal palette as well as the default left dock,
and resizes the live window for narrow, normal and 1920 × 1080 wide captures.
The dark top-dock case reports the actual palette and contextual-options bounds
and fails if the palette overlaps the brush-size/opacity bar.
The wide Home capture exercises the four-column gallery layout. It verifies that
real layers and a valid recovery checkpoint are present, and rejects
blank/undersampled images. The current matrix captures 22 named states, including
normal/narrow/wide Home, populated recent artwork, normal/narrow New Artwork,
populated light/dark Layers and Recovery, and light/dark Reference, Brush and
Color panels. The first-run Home capture waits for its rendered quick-start
format actions inside the hero. The recent-artwork capture requires visible blue
pixels from the real painted smoke-project thumbnail. Reference captures require
pixels from a uniquely colored region of the imported reference image. The
rendered windows are uploaded as `rust-paint-lab-studio-ui-smoke`. These
screenshots validate the virtual Linux window, not Windows DPI, Narrator/NVDA or
physical hardware. It explicitly checks that Escape dismisses the Brush flyout
without closing the app before continuing to Layers. The test environment logs which
flyout Escape dismissed, derives the palette click target from the rendered
button bounds, and exercises Ctrl+B as a keyboard alternative. The companion
`rust-paint-lab-linux-ui-diagnostics`
artifact retains the input/action log on failures.

The Home capture at 1920 × 1080 now also asserts that its full viewport covers the
GPU canvas beneath it. For the populated recent-artwork capture, the harness reads
the actual responsive-grid bounds and selects the largest complete project card
visible in the viewport, scrolling if no card is complete. It checks visible versus
total layout height, readable card content, and blue pixels from the real cached
thumbnail; another card can remain clipped at the bottom edge. Recovery screenshots
exercise the actual Preview button using an app-produced `.rpl` artwork fixture in
the isolated test recovery directory. The captured preview region must contain
multiple rendered colors. That visual fixture is removed before the independent
forced-termination/journal-recovery check runs. Recovery labels conceal internal
numeric snapshot names, identify snapshot/journal/backup sources and pass a rendered
4.5:1 text-contrast assertion in both Light and Dark themes. These are Linux/Xvfb
interaction checks, not manual Windows assistive-technology validation.

The Brush Library also generates deterministic preset samples by running each
built-in brush through the raster engine on a bounded temporary document. The
active sample reflects the current brush settings; preset cards use a stable
violet sample. The eraser preview cuts into an under-stroke to make erase
behavior visible. These preview pixels are UI cache data and never enter the
artist document. Tests compare deterministic output, distinguish representative
brush kinds, validate eraser transparency, reject invalid settings before tile
allocation, and inspect the active preview's AccessKit label. The Linux window
smoke requires all ten preset samples and a non-trivial active preview region.

```sh
cargo test --locked -p canvas-demo brush_preview::tests
xvfb-run -a env PYTHONDONTWRITEBYTECODE=1 python3 scripts/alpha-workflow-smoke.py
```

The screenshot archive from Actions run `40250590747` is produced on Linux, but
its artifact blob host returned HTTP 403 in the current environment. The local
X11 interaction run passed without screenshot capture; no current brush-gallery
screenshots have been visually inspected. Screenshot appearance acceptance is
therefore still pending. Actions run `40250590747` validates pushed base commit
`c81f3d8`, not later local changes.
