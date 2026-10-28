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
blank/undersampled images. The current matrix captures 23 named states, including
normal/narrow/wide Home, populated recent artwork, normal/narrow New Artwork,
populated light/dark Layers and Recovery, light/dark Reference, Brush and Color
panels, and the populated floating Workspace controls. The earlier suite required
29 visual states and produced 30 captures; the expanded workflow now requires 31
states and 32 captures, with the added multi-reference light views pending Actions.
The first-run Home capture waits for its rendered quick-start format actions
inside the hero. The recent-artwork capture requires visible blue pixels from the
real painted smoke-project thumbnail. In test mode, each synthetic project card
now also has an accessible **DEVELOPMENT TEST FIXTURE** pill, and the screenshot
checks its purple pixels so generated art cannot be mistaken for an owner's
project. Reference captures require
pixels from a uniquely colored region of the imported reference image. The
rendered windows are uploaded as `rust-paint-lab-studio-ui-smoke`. These
screenshots validate the virtual Linux window, not Windows DPI, Narrator/NVDA or
physical hardware. It explicitly checks that Escape dismisses the Brush flyout
without closing the app before continuing to Layers. The test environment logs which
flyout Escape dismissed, derives the palette click target from the rendered
button bounds, and exercises Ctrl+B as a keyboard alternative. The companion
`rust-paint-lab-linux-ui-diagnostics`
artifact retains the input/action log on failures.

The same Linux workflow also toggles the movable Workspace controls window with
F6, captures it in a populated state, switches its Layers/Brush tabs, and checks
that these window controls receive a click while geometrically over the document
canvas. It exercises the saved Color palette's add, reuse, duplicate rejection,
and removal against persisted workspace preferences. The official brand slogan
is checked for at least 4.5:1 contrast in both light and dark themes. Workspace
control geometry and size use bounded, DPI-aware workspace state; this test does
not replace native screen-reader verification.

For the populated Layers state, X11/XTest opens the per-row **Actions** menu and
checks layer-targeted Duplicate, Send backward, and Delete operations against
the actual document state. Ctrl+Z must restore each layer count/order. A separate
secondary-click on the row name must open the native context menu. The semantic
AccessKit tree asserts that every populated row exposes the text-labeled
**Actions** control; this does not stand in for Narrator/NVDA validation.
XTest also holds the dedicated pointer drag grip, moves a layer over the
backmost row and releases it; the test compares the full visible ID order before
and after Ctrl+Z. The grip is explicitly described as a pointer action, while
Bring forward/Send backward remain the keyboard path.

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

Reference-board captures report the current image rectangle after each resize.
Pixel checks are restricted to that rectangle and require both the fixture's
unique green marker and varied scene colors (at least 12). This prevents toolbar,
canvas, or unrelated UI colors from satisfying the reference-image assertion.
On the latest local run the light normal/narrow captures each showed 483 marker
pixels and 176 image colors; dark normal showed 126 marker pixels and 295 image
colors. The compact reference header/Settings row leaves at least 85% of its
window width for visible artwork (91.1% light and 96.3% dark in normal and narrow
states). The initial reference fixture is both panned and zoomed. X11/XTest opens
the actual Settings menu, chooses Center image, and verifies pan resets while
110% zoom persists to workspace preferences. Representative normal and narrow
captures were opened and visually reviewed locally. The final script also
rejects a visible image rectangle wider than its containing window. This
execution image has no Xvfb, so the latest complete local run used a dedicated
dummy Xorg server with XTEST; the exact final matrix passed there.

The expanded workflow accepts `RPL_TEST_REFERENCE_IMPORT_PATH` only when the
existing test-only path harness is enabled. After centering the first board it
imports a second image through the normal bounded loader and asserts that the
saved window remains on-screen and clear of the movable palette at 1280 × 800 and
640 × 480. It captures the populated multi-reference state at both sizes, using
stable geometry markers for the topmost board. These UI checks passed locally on
the Xorg/XTEST display. They do not replace testing the native Windows
multi-select dialog.

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
cargo build --locked -p canvas-demo
xvfb-run -a env PYTHONDONTWRITEBYTECODE=1 python3 scripts/alpha-workflow-smoke.py
```

The alpha script launches `target/debug/canvas-demo` directly. Rebuild that
executable after source edits; `cargo test` can leave it stale. On 2026-10-26,
the updated 29-state matrix completed and its 30 rendered images were inspected
locally through the supported image viewer. The narrow Brush capture now asserts
that the viewport contains at least two fully visible preset cards and that the
window clears the palette. The screenshot workflow uses a bounded wgpu offscreen
copy, so it does not depend on reading the X11 window surface with ImageMagick.
This is actual local visual review, while overall visual acceptance remains
pending for the broader Phase 8.3 redesign and Windows-specific layout behavior.

Actions run [`40265944815`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40265944815)
passed Windows, Linux, and macOS for commit `2b615f9`. The hue-wheel change below
is a later local commit and must receive a fresh Actions result after push.
Captures and pixel-region checks do not replace Windows DPI, Narrator/NVDA, touch,
or physical-device testing.

The Color flyout's hue ring uses normalized hue with red at the top and clockwise
progression. A center sample displays the current color; clicks/drags are accepted
on the annulus, while its center and exterior are ignored. The numeric Hue slider
remains available for keyboard and precise adjustment. The AccessKit tree names
the wheel and saturation/brightness field. The Linux integration test clicks the
actual ring, verifies that hue changes while saturation and brightness stay fixed,
and restores the previous color from the saved palette. The Color screenshot test
checks that the rendered ring region contains at least 32 distinct chromatic
colors. After editing this UI, rebuild `canvas-demo`, run the full workspace
checks, and repeat the X11/XTest workflow above. The current workspace suite has
249 passing Rust tests.

Local Windows-GNU target checking is compile/lint validation only; the native
Windows CI job remains the executable validation for Windows builds.

### Type system and brush-size hotkeys

`ui_tokens::tests` checks that every type role resolves to a locally embedded
family with the original proportional fallback chain, and that mixed Latin,
symbol and CJK sample text can be shaped. The font assets are version-pinned
static instances; their weights, source revision, hashes, and OFL notices are
recorded in `assets/fonts/README.md`. The X11/XTest alpha workflow exercises `]`
and `[` in the live canvas and verifies a ten-percent increase/decrease sequence
against the active raster brush. Shift-modified one-percent adjustments, bounds,
and invalid starting values have deterministic unit coverage. Text fields retain
keyboard ownership, and replay mode does not route these brush controls.

```sh
cargo test --locked -p canvas-demo ui_tokens::tests --lib
cargo test --locked -p canvas-demo studio::tests::brush_size_shortcuts_are_predictable_and_bounded --lib
cargo build --locked -p canvas-demo
xvfb-run -a env PYTHONDONTWRITEBYTECODE=1 python3 scripts/alpha-workflow-smoke.py
```

The latest local checkpoint's Linux Xorg/XTEST run captures 37 populated and
empty UI states across light, dark and high-contrast themes at 640 × 480,
1280 × 800, and 1920 × 1080. It exercises the live bracket brush-size shortcut
and checks the full mouse-paint/save/export/recovery path. Recovery dialog
bounds are read from the live egui layout and asserted after narrow resizing;
the populated light and dark dialogs remain inside 640 × 480. Screenshots are
inspected manually after capture; automated pixel checks establish content
presence and layout bounds, not subjective design acceptance.

### 2026-10-26 Home and New Artwork integration

This environment has no Xvfb binary. The same real winit/egui application
workflow was run locally on a dedicated dummy Xorg display with XTEST and Mesa
software rendering (`DISPLAY=:200`), using the repository's supported wgpu
offscreen window capture. The complete 31-state screenshot matrix and artwork
workflow passed, including live drawing, layer actions/undo, save/reopen,
PNG/OpenRaster exports, reference isolation, and process-kill journal recovery.
The final Home tile was clicked with XTEST; it opened the deterministic Open
Project picker and the actual Cancel action returned to Home.

New Artwork screenshots use dynamically reported egui bounds for all seven
paper illustrations, not hard-coded approximate coordinates. Each preview was
checked for colored paper pixels (Portrait 318 colors, Landscape 281, Square
262, A4 Print 174, Comic Page 230, Full HD 272, Custom 196). The live document
preview shows the selected aspect ratio and configured solid or transparent
background. The recent-artwork screenshot requires a real cached project
thumbnail, a harness-only DEVELOPMENT TEST FIXTURE badge, and the accessible
Open another project action card. Home gallery sizing includes card insets and
only allocates the columns that its current project/action count can fill.

The reference fixture's unique green marker now sits in the visible center of
the image rather than at a panned/cropped corner. The latest dark normal/narrow
screens showed 915/913 marker pixels and 325/398 scene colors; light and dark
image-width shares were 91.1% and 96.3%. I visually inspected Home, populated
Home, New Artwork in Light and Dark, and the dark multi-reference canvas captures
locally. Screenshots are review evidence, not design acceptance for Phase 8.3.
The latest pushed commit `76a77b0` still has a Linux alpha workflow failure in
Actions run `40270241090` (Windows and macOS passed); its run-log/artifact receiver
returned HTTP 403. The current source has not yet received a fresh Actions run.

### Outfit and compact workspace regression

The current Linux Xorg/XTest continuation adds compact panel checks at 640 ×
480. The harness opens a real three-layer stack and two imported references,
verifies both compact windows stay inside the application and above the status
strip, checks that the large saved panel dimensions survive resizing, navigates
the compact reference board, and toggles/restores layer visibility and locking
through the accessible Actions menu. It uses the actual layer-name bounds for
the native context menu after the row hierarchy changes. Escape closes an open
egui popup before the panel or application; application shutdown tests allow
that two-step behavior. Existing workflows continue through targeted layer
duplicate/reorder/delete and undo, reference-isolated PNG/OpenRaster export,
save/reopen, and journal-durable process-termination recovery.

The screenshot matrix uses real app captures at 640 × 480, 1280 × 800 and
1920 × 1080 in light/dark/high-contrast modes; populated layer/reference and
recovery states are included. It asserts actual Home/New Artwork text and
format-card regions, reference-image prominence, panel bounds, and screenshot
coverage, then requires manual inspection of generated captures. Fonts are
embedded Outfit 700/800/900 and Manrope 400/600/700; `ui_tokens` tests exercise
the shared roles and fallback shaping. See
[`assets/fonts/README.md`](../assets/fonts/README.md) for licensing and source.

The populated Recovery modal also reports its current content bounds. The XTest
workflow asserts those bounds stay inside the window after resizing to 640 ×
480, checks the preview button with a fresh pointer hover, and captures the
visible title, entries, actions and footer in both themes.

The 2026-10-27 code checkpoint `9c227bc` passed
[GitHub Actions run `40452947352`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40452947352)
on Windows, Linux and macOS. Windows visible-window startup and macOS first-frame
startup passed; Linux passed the real UI/artwork workflow. Windows uploaded
`rust-paint-lab-canvas-demo` and `rust-paint-lab-windows-diagnostic` separately.

Run against the local Xorg display (or a suitable Xvfb desktop) with a software
wgpu backend:

```sh
DISPLAY=:203 XDG_RUNTIME_DIR=/tmp/bitchpixel-runtime \
  XDG_CACHE_HOME=/tmp/bitchpixel-cache WGPU_BACKEND=gl \
  RPL_UI_SCREENSHOT_DIR=/tmp/bitchpixel-ui \
  python3 scripts/alpha-workflow-smoke.py
```

This uses Mesa software rendering and Linux XTest, not physical display/GPU
validation. Windows GUI startup, Windows DPI, screen readers and physical pen
behavior remain platform-specific checks.

## Engine-first painting continuation

The current local checkpoint replaces the document engine's per-sample smoothing
with a time-aware, constant-memory One Euro position filter. The amount-zero
path is exact identity. Tests compare real raster output with smoothing disabled
and enabled, repeat identical timestamped sequences, check the terminal release
pixel, prove cancelled tiles do not enter document state, and retain the existing
document-edge clipping/re-entry coverage. Pressure/tilt remain sample data and
are not synthesized by the filter.

Document tests also exercise raster line, rectangle and ellipse operations,
selection clipping, locked layers, native encode/decode, PNG export/import, and
single-operation undo/redo. The Linux application workflow uses XTest to draw
and cancel a live shape preview, then commits each shape, saves and checks a
pixel in the selected native layer, and verifies exact project bytes on undo and
redo. Screenshot-region checks ensure the active preview actually rendered with
contrast before the test cancels it. The full workflow additionally repeats
painting, layer, native project, PNG/OpenRaster and journal-recovery paths.

At this code checkpoint, local validation passed: **262 workspace tests**,
`cargo fmt --all -- --check`, strict workspace Clippy, workspace build, Windows
GNU target check/Clippy, llvmpipe OpenGL offscreen GPU smoke, and the full Xorg/XTest
workflow. The exact Xvfb invocation used was:

```sh
export CARGO_HOME=/workspace/.cargo RUSTUP_HOME=/workspace/.rustup
export PATH=/workspace/.canvas-runtime/usr/bin:/workspace/.cargo/bin:$PATH
export LD_LIBRARY_PATH=/workspace/.canvas-runtime/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
export XDG_RUNTIME_DIR=/tmp/rpl-runtime XDG_CACHE_HOME=/tmp/rpl-cache
export VK_ICD_FILENAMES=/workspace/.canvas-runtime/lvp.json WGPU_BACKEND=vulkan
export RPL_UI_SCREENSHOT_DIR=/tmp/bitchpixel-phase83-ui
xvfb-run -a -s '-screen 0 2200x1400x24' python3 scripts/alpha-workflow-smoke.py
```

This verifies a software-rendered Linux window and real application input under
XTest, not physical pen latency or Windows/macOS GUI behavior. Basic S Pen
drawing was physically observed on the owner’s device in an earlier build;
advanced pen behavior and native Windows accessibility testing remain pending.

The pushed implementation commit `6d85c36` passed all three desktop jobs in
[GitHub Actions run `40529268958`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40529268958).
Windows visible-window startup and both portable Windows application artifacts
passed; Linux's GPU/window, mouse, and full artwork/XTest workflow passed; macOS
first-frame startup and unsigned alpha packaging passed. The run provides the
[`rust-paint-lab-canvas-demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40529268958/artifacts/12744953710),
retained [`rust-paint-lab-windows-diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40529268958/artifacts/12746885390),
and Linux screenshot archive
[`rust-paint-lab-studio-ui-smoke`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40529268958/artifacts/12745936098).
These are workflow artifacts, not a release or physical-hardware validation.

## Linear and radial raster gradients

The document tests verify clamped linear projection and radial distance,
linear-light sRGB stop interpolation, pixel-center rectangle/ellipse/lasso
clipping, hidden/locked layer rejection, brush opacity, no-op idempotence,
tile bounds, one-step undo/redo, native `.rpl` encode/decode and PNG import/export.
The XTest workflow exercises `G`, selects Radial through its real contextual
control, checks the transient live preview in a captured wgpu frame, cancels with
Escape and verifies byte-identical artwork, then commits Linear and Radial,
checks selected-layer pixels, saves, and compares exact project bytes through
undo/redo. Linear/radial gradient screenshots were reviewed to confirm reference
windows remain above the preview and visible.

The latest local run passes **266 workspace tests**, formatting, strict Clippy,
workspace build, Windows GNU target check/Clippy, llvmpipe OpenGL GPU smoke, and
the complete Linux/Xvfb/XTest artwork workflow. The existing 640 × 480, 1280 ×
800, 1920 × 1080, theme, populated Layers/Recovery, save/reopen, PNG/OpenRaster
and recovery coverage remains active. This does not validate Windows hardware,
physical color output, pen pressure/tilt behavior, or Narrator/NVDA. The owner's
known successful `b50e7ec` Windows launch remains the hardware baseline.

GitHub Actions run
[`40546299074`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074)
passed Windows, Linux and macOS. Windows performed the visible-window startup
smoke and uploaded the separate
[`rust-paint-lab-canvas-demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074/artifacts/12752669319)
and [`rust-paint-lab-windows-diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074/artifacts/12752474102)
artifacts. Linux passed the complete artwork/XTest workflow and uploaded
[`rust-paint-lab-studio-ui-smoke`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074/artifacts/12752754019).
macOS passed first-frame startup and packaged an unsigned development build.
These CI results do not replace physical device, display color or advanced
stylus testing.

## Coordinated workspace and raster-layer thumbnails

The bounded thumbnail cache is covered by
`cargo test --locked -p canvas-demo layer_thumbnails::tests --lib`. It verifies
the 96 × 64 size cap, layer opacity, actual brush/shape/gradient content, Undo and
Redo, selective per-layer invalidation, and a save/open round trip using the
native `.rpl` Store. The document test
`clearing_a_targeted_layer_preserves_selection_and_undo_restores_its_pixels`
checks ID-targeted clearing, selection retention, lock rejection, and pixel
restoration.

The real-window Xorg/XTest workflow opens the real Layers and Brush panels from
Workspace controls, paints and selects a populated layer through normal input,
then clears and undoes it. A document regression also checks that the Brush
Library's calligraphy-angle setting rotates the actual rasterized mark. Its
screenshot assertions check content pixels inside the thumbnail rather than
counting the selected border. It captures populated
Layers/References at 640 × 480, 1280 × 800, and 1920 × 1080, and fails if the
actual window rectangles overlap or leave the viewport. The latest local run
passed this workflow and all 270 workspace tests. Screenshot PNGs are written to
the configured `RPL_UI_SCREENSHOT_DIR`; generated files are not committed.

The recovered workspace/layer implementation was committed as
[`8985754`](https://github.com/starryeyesmedia/Rust-Paint-Lab/commit/8985754f78723fed3722bf69860e463be5fe10e1)
and passed GitHub Actions run
[`40574152036`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036)
on Windows, Linux and macOS. Windows passed the visible-window startup smoke and
uploaded the separate [`canvas demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036/artifacts/12767816064)
and [`stylus diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036/artifacts/12766097424)
artifacts. Linux passed the complete Xorg/XTest artwork workflow and uploaded
the [`UI screenshot archive`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036/artifacts/12767007921).
macOS passed its first-frame startup smoke and unsigned packaging. These CI
checks do not constitute physical Windows, display-scaling, advanced pen, or
native screen-reader validation.

### Layer copy and compositing controls

`Engine::copy_selection_to_new_layer` copies nontransparent premultiplied pixels
whose centers lie inside the active rectangle, ellipse, or lasso selection. It
requires the selected source to be visible and unlocked, preserves that source
and the selection, inserts one normal layer above it, and selects the result.
Document dimensions, the layer cap, and tile budget bound the operation. It is
one undoable document transaction and uses the existing `.rpl` representation.
Tests verify exact tile pixels, selection masks, unchanged source/composite,
undo/redo, native save/reopen, empty/hidden/locked rejection, and both layer and
tile limits.

The selected-layer panel now lays out its opacity slider across the available
settings width and labels the accessible value. Opacity and Normal/Multiply/
Screen/Overlay controls continue to update the selected document layer. Pixel
regressions verify each compositing mode through native project and PNG
roundtrips. A COW-snapshot check in `Engine::edit` skips unchanged transactions,
so repeated unchanged widget output does not dirty the document or fill undo
history; changed metadata and pixels still enter bounded history.

The real XTest flow selects existing artwork, invokes Copy selected pixels from
the actual Layers action menu, saves and decodes the `.rpl`, compares source
tiles byte-for-byte, confirms the new layer contains exact selected-source
pixels, then checks exact undo/redo project bytes. It drags the real opacity
slider, verifies saved partial opacity and Ctrl+Z/Ctrl+Y while that control had
focus, and chooses Multiply from the actual blend menu with native save and
history verification. Existing shape, gradient, PNG/OpenRaster, reference
isolation, and forced-crash journal recovery steps still run in that same
application workflow.

On 2026-10-27 the local run passed **275 workspace tests**, formatting, strict
Clippy, workspace build, Windows GNU workspace check/Clippy, the required
llvmpipe/OpenGL offscreen GPU smoke, and the full Linux/Xorg/XTest artwork flow.
Screenshots of populated layer copy, light and dark Layers/References, recovery,
Home, and high contrast were generated; I inspected the layer and high-contrast
captures. The high-contrast Home capture shows the current scrolled content and
cropped bottom-row cards, so it is not evidence of a fully fitted Home page.
The full workflow uses an Xvfb screen at least 1920 × 1080 so pointer events can
reach controls in wide-window screenshots:

```sh
xvfb-run -a -s '-screen 0 2200x1400x24' python3 scripts/alpha-workflow-smoke.py
```

GitHub Actions run
[`40590882859`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40590882859)
passed on Windows, Linux, and macOS for commit `87588a4`. The hosted Windows
visible-window startup smoke and macOS first-frame smoke passed; Linux passed
the full Xorg/XTest artwork workflow and screenshot capture. The run published
the separate Windows
[`rust-paint-lab-canvas-demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40590882859/artifacts/12774436753)
and [`rust-paint-lab-windows-diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40590882859/artifacts/12774361545)
artifacts and Linux
[`rust-paint-lab-studio-ui-smoke`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40590882859/artifacts/12775130275)
archive. No physical Windows hardware was retested here.

## Phase 9 transient Smudge mode

`paint-document` tests cover premultiplied-color pickup into transparent pixels,
strength and pressure response, preview-only tile copies, cancellation, exact
undo/redo, native `.rpl` and PNG roundtrips, selection clipping, hidden/locked
layers, device eraser precedence, and safe concurrent-contact rejection. A
`LiveMouse` test drives samples through Processor, Router and the document engine,
then checks preview, focus-loss cancellation, commit and undo on actual pixels.
Smudge mode is transient and does not change native v1 storage.

The Linux artwork workflow also selects **S Smudge**, draws through the real
winit/XTest path, saves, compares selected-layer tile bytes, and checks exact
undo/redo file restoration. It captures `smudge-workspace.png`. The 2026-10-28
local run passed against Xorg with llvmpipe; Actions will refresh the screenshot
archive after this implementation is pushed.

Color Mixer regression coverage exercises real pixel blending and no-op loads,
pressure and opacity/flow behavior, transparent pixels, selection clipping,
locked layers, eraser precedence, concurrent-contact safety, cancel/commit,
undo/redo, native-v1 and PNG roundtrips, and the routed `LiveMouse` lifecycle.
The Linux Xorg/XTest artwork workflow also selects **X Color Mixer**, paints
through the normal input processor/router, verifies changed selected-layer tile
bytes in the saved project, then checks exact undo/redo persistence. This
software-rendered test validates the application workflow, not physical stylus
hardware or color-managed mixing.

The platform-independent barrel router tests cover default Eraser switch/release,
terminal and cancellation cleanup, reused IDs, multiple pointer identities,
bounded Pan deltas, temporary-action suppression until pen lift, and malformed
or unexpected lifecycle events. `CanvasSession` integration tests drive synthetic
native-style samples through Processor, Stroke Router and the document engine:
they verify held Eraser pixels plus separate undo transactions, release back to
the selected paint tool, visible-composite Eyedropper sampling without painting,
and Pan deltas without painting. Workspace tests verify that Eraser is the
backward-compatible default and that assignments persist. This does not replace
physical S Pen validation of the Windows WM_POINTER button bit.
