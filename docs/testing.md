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

## Canvas-first Inspector responsive layout and reference safety

`studio_layout::tests` covers the approved effective logical sizes for first-run
Inspector docks (1920×1080→360 pt, 1280×720→336 pt, 1280×800→320 pt,
853×533→288 pt), the 640×480 256-point drawer, and the full-sheet 640×480 at
150% case. Workspace preference tests cover migration of legacy custom widths;
temporary geometry is checked separately from retained artist preferences.
Geometry tests cover all-edge panel/palette avoidance, Zoom/Fit protection,
24-point snapping, clamping, and invalid viewports.
The compact layer-row regression test ensures egui's expanded clip margin cannot
inflate the Inspector and that rows respect the narrower viewport while keeping
the 44-point minimum for essential controls.

The real pinned-reference pointer drag and no-paint assertion runs as part of
the complete Linux workflow:

```sh
xvfb-run -a -s "-screen 0 2200x1400x24" python3 scripts/alpha-workflow-smoke.py
```

The app reports the effective Inspector and reference-safe rectangles through
test markers. CI screenshots are uploaded with the
`rust-paint-lab-studio-ui-smoke` artifact for inspection. A virtual X11 scale setting verifies logical
layout calculations, but does not establish native Windows/macOS display
scaling, Narrator/NVDA, or physical-device behavior. This checkout's current
environment has no Xvfb executable; run the UI command in CI or install the
workflow's Ubuntu display prerequisites before claiming the real pointer case
passed locally.

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

`Engine::move_selection_to_new_layer` is tested separately from Copy for all four
supported blend modes. It preserves the composite and source opacity/blend metadata,
clears only selected source pixels, retains unselected pixels/selection, and supports
one-step undo/redo plus native save/reopen. Empty and locked source failures are
atomic. The real Layers Actions workflow moves the pixels copied earlier in the
same session, verifies exact premultiplied tile bytes and the empty original copy
layer in the saved `.rpl`, then undoes to byte-identical prior artwork. CI screenshot
coverage includes the populated moved-layer state; Actions verification is pending.

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

### Editable layer masks — real XTest regression

The mask workflow uses the selected layer's expandable settings rather than a
nested Actions submenu. The Linux artwork smoke opens the rendered controls by
their stable UI markers, clicks Add white mask, saves `.rpl` v2, and reopens that
file through the application picker. Entering mask mode opens Brush Library;
Escape dismisses that floating panel, and the test confirms that mask mode
remains active before it paints with XTest. It verifies saved mask tiles changed
while every source raster tile remained byte-identical, then checks exact
Undo/Redo. It clicks the Enabled checkbox both ways, Invert, Clear to white, and
Remove. Each change is validated against parsed saved state and its own exact
history round trip; removing the last mask writes readable v1 metadata with the
optional `mask` field omitted, and undo returns to the exact v2 state. Rust bool
markers are compared using lowercase `true`/`false`. The later OpenRaster UI
step must still report flattened masks. Document tests cover selection clipping,
effective alpha compositing, source tile preservation, malformed v2 records,
recovery and v1/v2 round trips. The full Linux/Xorg/XTest flow has passed locally
on the existing display; a hosted run is still required to verify the CI runner.
`cargo test` does not substitute for the Linux window workflow.

On 2026-10-29, the final local validation passed all 336 workspace tests,
formatting, strict Clippy, workspace build, Windows GNU target check/Clippy,
`scripts/check-tests.py`, and the required llvmpipe GPU smoke. The complete
Linux window workflow passed on `DISPLAY=:102` with Xorg/XTest and OpenGL
software rendering, producing 59 PNGs. The mask-editing screenshot shows the
contextual Brush Library open; the test dismisses it with Escape and confirms
mask editing remains active before sending the canvas stroke. Screenshots were
reviewed for mask editing, painted-mask state, and populated recovery. Hosted
cross-platform CI remains pending for the corrected harness.

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

The document suite also covers transformed image placement: pixel/alpha output,
quarter-turn rotation, document-edge clipping, invalid-transform atomicity, one
undo/redo transaction, and native encode/decode. The bounded PNG/JPEG decoder tests
corrupt and over-dimension images. `scripts/place-image-smoke.py` starts the actual
canvas under Xvfb, inspects colored regions inside the preview's actual rendered
bounds, clicks Cancel and verifies the project is unchanged, then changes scale and
rotation, drags the preview, captures the transformed GPU state, and uses Place. It
saves a native `.rpl`, checks layer metadata, tile count and integrity checksum, then
uses application Undo/Redo to verify placement is one reversible operation. CI
uploads the cancelled, placed and transformed preview screenshots with the other
Linux studio captures. The Xorg smoke also verifies that egui's floating transform
panel remains above the temporary canvas preview and can be manipulated by real
mouse events.
The smoke tests do not validate Windows pen behavior; physical pen placement still
requires hardware.

The Linux XTest artwork workflow waits for an explicit OpenRaster export start and
completion marker and allows up to 120 seconds for that operation. This distinguishes
a delayed export from a lost shortcut or a reported file error. The exporter validates
the document once, then converts each populated sparse layer directly to RGBA instead
of cloning and revalidating every other layer for each PNG. A unit test checks
premultiplied-alpha conversion, opacity remaining in ORA metadata, and right/bottom
tile clipping. The next Actions run is required to confirm the original failure no
longer occurs on GitHub-hosted Linux; this environment has no Xvfb for local GUI
reproduction.

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

## Raster symmetry and drawing guides

`paint-document` tests cover vertical/horizontal reflection, radial copies up to
16, transaction-captured settings, cancellation, one-step undo/redo, native
roundtrip pixels, coincident-dab deduplication and clipped guide geometry.
`canvas-demo`'s AccessKit test checks semantic, keyboard-reachable mode and origin
controls. The origin can now be positioned directly: arm **Place origin on
canvas**, then click on the canvas or use a pen contact. The capture consumes the
entire pointer contact; it must not produce paint input. Escape cancels without
changing the saved origin or project.

The full Linux/Xorg/XTest application workflow presses **Y**, selects Vertical,
places a non-centered origin using a real mouse event, verifies its normalized
workspace preference and confirms the click left artwork bytes and stroke-start
count unchanged. It then paints through the routed mouse path, checks original
and reflected layer pixels around that custom origin in the saved `.rpl`, and
compares exact project bytes after Undo/Redo. It also captures the populated
custom-origin symmetry panel.

Run the targeted tests with:

```sh
cargo test --locked -p paint-document --test document symmetry_
cargo test --locked -p canvas-demo --lib drawing_symmetry_controls_are_named_and_keyboard_exposed
cargo test --locked -p canvas-demo symmetry_origin_uses_the_live_zoomed_document_mapping_and_rejects_invalid_points
```

Run the full application integration locally with an Xorg/Xvfb display and XTest
available:

```sh
cargo build --locked -p canvas-demo
DISPLAY=:102 WGPU_BACKEND=gl LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR=/tmp \
  RPL_UI_SCREENSHOT_DIR=/tmp/bitchpixel-studio-ui \
  python3 scripts/alpha-workflow-smoke.py
```

GitHub Actions runs the same workflow under `xvfb-run` with Mesa Vulkan. The
current local execution used the dedicated Xorg display `:102` and llvmpipe GL;
it validated software-rendered interaction, not physical pen pressure, pen tilt,
Windows GUI behavior or hardware latency.

## Editable vector-path slice

The Phase 10 vector slice stores line and cubic path geometry in the owning
raster layer while keeping its tile map authoritative for raster pixels. Test
the path model, v1/v2/v3 compatibility, v4 filled-path persistence, recovery
journal, history, layer duplication, and raster/OpenRaster flattening with:

```sh
cargo test --locked -p paint-document --test vector
cargo test --locked -p paint-document --lib vector::tests
cargo test --locked -p paint-document --test vector closed_vector_fill -- --nocapture
cargo test --locked -p paint-document --test vector fill_work_budget -- --nocapture
cargo test --locked -p paint-document --lib vector::tests::vector_fill_requires_a_closed_path_and_uses_even_odd_membership -- --nocapture
cargo test --locked -p canvas-demo --lib vector_creation_visibility_and_movement_invalidate_layer_thumbnails
cargo test --locked -p paint-document --test vector dense_editable_paths_keep_document_and_layer_previews_tile_bounded
cargo test --locked -p canvas-demo --lib dense_vector_layer_thumbnail_reuses_bounded_source_tile_rows
```

The full Xorg/XTest application workflow creates a path through the V tool,
checks the live canvas preview, saves and reopens it through the Open workflow,
moves and deletes the object with exact undo/redo checks, then confirms
OpenRaster rasterizes the path. The saved RPL v3 raster and mask tile tables must
remain unchanged by vector creation; path metadata and the monotonic ID allocator
are checksum-covered. The test also exercises the previously accepted Phase 9
mask journal/recovery workflow and the existing painting/export regressions.

V draws a connected polyline on an empty canvas area by default. The optional
workspace-only **Smooth new paths** setting converts the same freehand samples
to connected cubic Bézier segments while preserving each sampled anchor. It is
off by default and old workspace preferences deserialize to the polyline mode.
The choice is not stored in `.rpl`; both modes use the existing v3 path schema.
Dragging a visible path moves the whole object; the contextual Delete path
action removes it as one undoable transaction. Anchor selection, movement,
insertion and supported deletion are available from the node editor. Existing
cubic Bézier segments expose independent incoming/outgoing handle drag and
numeric edit controls. OpenRaster and PNG rasterize vectors on export; only the
native `.rpl` format retains editable path objects.

The document-level smoothing conversion is deterministic, bounded by the same
1,024-segment path limit, preserves exact endpoints, and shares tangents at
interior anchors. Validate its geometry, invalid input handling, native save,
roundtrip, rendering, raster isolation and single-transaction Undo/Redo with:

```sh
cargo test --locked -p paint-document smoothed -- --nocapture
cargo test --locked -p paint-document --test vector smooth_vector_creation -- --nocapture
cargo test --locked -p canvas-demo smooth_vector_creation_is_backward_compatible_and_persistent -- --nocapture
```

The Xorg/XTest artwork workflow enables this option through its real contextual
control, draws through the V tool, checks the persisted cubic segments and
unchanged raster/mask tile tables, captures `vector-smoothed-path`, and verifies
Undo/Redo, application reopen and workspace-preference persistence. The same
workflow first checks the legacy polyline default and continues through native
save/reopen plus PNG/OpenRaster exports.

The extended workflow then uses the selected path's real contextual controls to
close it and apply the current brush color/opacity as a fill. It verifies that
closing alone remains v3, filled geometry opts into v4, vector/mask tile tables
remain byte-identical, undo returns to the closed-unfilled v3 file, redo restores
the v4 file, and the filled object survives application reopen. It captures the
populated `vector-filled-path` workspace state. Document tests additionally
check even-odd interior membership, fill rendering and hit testing, native
roundtrip, PNG/OpenRaster pixels, transaction history, and rejection when
pixel-area × flattened-edge work exceeds the documented cap. Fill coverage is
currently pixel-center sampled; this does not test coverage antialiasing.
The Layers thumbnail cache regression also verifies cache invalidation and
visible pixel changes for close, fill, and fill removal.

Exact native cubic-coordinate stability is covered by:

```sh
cargo test --locked -p paint-document --test vector repeated_native_roundtrips_preserve_cubic_f64_coordinates_exactly
```

The test encodes, decodes and re-encodes nontrivial `f64` path coordinates for
eight consecutive cycles, requiring both exact model equality and unchanged
native bytes. The document crate enables serde_json's `float_roundtrip` parser
feature; this improves numeric preservation without changing the RPL v1-v4
schemas or older-format compatibility.

The Xorg/XTest artwork workflow also injects a valid cubic path fixture, opens it
through the application, edits an outgoing control handle by pointer, and
verifies the saved coordinate, unchanged raster and mask tile tables, native
reopen, and exact Undo/Redo. The editor exposes keyboard-accessible named X/Y
controls plus an Apply action, represented in the AccessKit tree. A canvas
mapping captured at pointer-down remains in use through pointer-up so contextual
reflow after path selection cannot translate artwork on a stationary click.

Latest local Phase 10 validation (2026-10-29): `cargo fmt --all -- --check`,
strict host and Windows GNU Clippy, `cargo test --workspace --locked --quiet`
(**352 passed**), `cargo build --workspace --locked`, Windows GNU workspace
check, the required llvmpipe GPU smoke, and the full Xorg/XTest artwork workflow
all passed. The Xorg workflow generated 61 PNG screenshots. During forced-crash
recovery, raster and mask tile tables compare byte-for-byte; JSON document and
vector metadata allow at most `1e-12` f64 serialization drift. The local
graphics backend was Mesa llvmpipe/OpenGL on Xorg display `:102`; it does not
validate a physical GPU, Windows/macOS GUI startup, or pen hardware. Hosted
cross-platform Actions for this Phase 10 slice must pass after push.

The Home and Layer thumbnail paths now rasterize and reuse at most one or two
source-tile rows while sampling vector artwork; conservative control-point bounds
skip unrelated paths per tile. Dense preview regressions use 8 paths and 8,192
segments on a 2048 × 2048 document and confirm both thumbnails still contain the
stroke. They completed in roughly 0.6 seconds for the Home preview and 0.5
seconds for the Layer preview in the local debug test run; this is a regression
fixture measurement, not a hardware benchmark. The full Xorg/XTest workflow
passed before this thumbnail-only follow-up; the targeted dense preview tests and
workspace suite pass on the updated code, with the hosted UI workflow pending.

The post-optimization full Xorg/XTest rerun initially exposed stale test waits,
not an application regression: repeated selection/transform and mask events were
matched by earlier log markers. The script now snapshots marker counts before
input and waits for a new commit, handle report, transform start/cancel, or mask
toggle event as appropriate. It preserves the native file byte comparisons and
Undo/Redo assertions. After this correction, the complete Linux workflow passed
locally on display `:102` with Mesa llvmpipe/OpenGL and generated 61 screenshots.
The visible path-creation state and populated narrow dark Recovery view were
inspected. GitHub run `40818285789` records the pre-correction harness failure;
the corrected harness requires a fresh Actions run after its commit. The prior
vector commit passed the Windows, Linux and macOS matrix in run
[`40817631458`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40817631458),
including separate Windows canvas and stylus diagnostic artifacts.

The current continuation additionally covers vector style edits. API tests
verify width and RGBA color, geometry preservation, raster output, v3 encode,
thumbnail invalidation, and exact Undo/Redo. The live UI test changes width
through the selected-path slider and checks one transaction, persisted native
state, unchanged raster/mask tables, thumbnail refresh, and exact undo/redo;
the path is reopened through the application before movement/deletion checks.
The current selected-path UI offers width and Apply color. Alpha remains
available to the engine API, but no separate path-opacity widget is currently
present.

After the Layers automation began using the actual logged ScrollArea clip
rectangle, the complete Xorg/XTest workflow passed locally with llvmpipe/OpenGL,
including Add/Edit/Enable/Disable/Invert/Clear/Remove masks, recovery, export,
and the editable vector workflow. The workspace has **354 tests**; formatting,
strict Clippy, build, Windows GNU checks and GPU smoke passed in the same local
validation. Hosted Windows/Linux/macOS validation must still pass for the
current source commit. Do not interpret Linux llvmpipe or Windows cross-checks as
physical Windows GUI, S Pen, macOS runtime, Narrator/NVDA or hardware-GPU tests.

### Latest anchor-editing regression — 2026-10-29

The selected path's **Edit anchors** control now exposes an accessible anchor
selector and coordinate fields. The complete `scripts/alpha-workflow-smoke.py`
Xorg/XTest run verifies anchor drag and vector-width slider gestures through
actual application input. The test waits for the app to acknowledge each held
pointer frame because XTest can otherwise deliver the press and release in a
single egui frame. It asserts preview/commit behavior, path geometry in v3,
unchanged raster and mask tile tables, thumbnail refresh, save/reopen, and exact
Undo/Redo. The focused integration also rejects invalid coordinates/indices and
locked layers atomically:

```sh
cargo test --locked -p paint-document --test vector
```

Local validation on 2026-10-29 passed formatting, strict host Clippy, all
**356 workspace tests**, workspace build, Windows GNU workspace check and
Clippy, and explicit llvmpipe GPU smoke. The complete Xorg/XTest workflow passed
and generated 62 screenshots, which were inspected for the populated vector
editor, Home, New Artwork, and narrow Layers/Reference layouts. A 360 px path in
the 1800 × 2400 XTest artwork took 12.444 s for OpenRaster export in the UI and
11.993 s in a standalone debug build; this is a software-rendering/debug timing,
not a release build or hardware measurement. Hosted cross-platform Actions for
these uncommitted changes are pending; the previous green Actions run validates
only the prior commit. The node editor creates/moves anchors on existing
polylines and now inserts/deletes supported anchors as described below; it does
not yet author cubic curves, expose independent Bézier handles, or edit
fill/transforms.

### Editable vector anchor operations — 2026-10-29

`paint-document` now inserts an anchor at the exact midpoint of a line or cubic
segment (cubic segments use De Casteljau subdivision) and removes endpoints or
interior anchors between line segments without approximation. Unsupported
interior cubic joins, malformed indices, locked layers, segment-limit overflow,
and non-finite geometry are rejected without mutating the document. Each engine
operation creates one undoable transaction. Native v3 save/reopen, rendered
pixels, and raster/mask tile isolation are covered by:

```sh
cargo test --locked -p paint-document --test vector
```

The Linux XTest workflow opens the `Anchor N` menu with real pointer input,
inserts a midpoint, checks saved line/cubic segment geometry, unchanged raster
and mask storage, thumbnail invalidation, save, undo and redo, then deletes the
midpoint and checks that the original project bytes return. The screenshot suite
adds `vector-node-editor-with-inserted-midpoint`. The complete workflow passed
locally on Xorg display `:102` with Mesa llvmpipe/OpenGL and produced 63 PNGs; I
inspected the new populated anchor-editor screenshot. This environment lacks an
`Xvfb` executable but had a reusable Xorg server for XTest.

Local Rust 1.90 validation after the anchor-operation changes: formatting,
strict host Clippy, strict Windows GNU target Clippy, workspace build, Windows
GNU workspace check, all **359 workspace tests**, and the required llvmpipe GPU
smoke pass. Parent commit `8a0b5a0` passed the Windows/Linux/macOS Actions matrix
in run [`40842835450`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40842835450);
the insertion/deletion follow-up still needs its own hosted run.
This does not establish physical pen/hardware behavior.

### Cubic control handles and complete local regression — 2026-10-29

The Xorg/XTest artwork workflow now selects, drags, and applies numeric edits to
incoming/outgoing Bézier handles on an editable cubic path. It saves/reopens the
path, checks that raster/mask tile tables are unchanged, and verifies exact
Undo/Redo. A Color Mixer redo assertion found least-significant f64 metadata
drift during repeated native JSON reads; `serde_json`'s `float_roundtrip`
feature now retains exact geometry. The regression runs eight encode/decode
cycles and also passes through the application workflow. No project schema
changed.

Validated locally with Rust 1.90:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo check --workspace --locked --target x86_64-pc-windows-gnu
cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings
RPL_REQUIRE_GPU=1 WGPU_BACKEND=gl LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR=/tmp/bp-runtime \
  cargo test --locked -p canvas-demo --test gpu_smoke -- --nocapture
DISPLAY=:102 WGPU_BACKEND=gl LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR=/tmp/bp-runtime \
  RPL_KEEP_ALPHA_TEMP=1 RPL_UI_SCREENSHOT_DIR=/tmp/bp-vector-handles-smoke \
  python3 scripts/alpha-workflow-smoke.py
```

All **364 workspace test cases** passed. The required GPU smoke used Mesa llvmpipe
and passed its replay/live preview, commit/cancel, clipping, raster preview,
Undo/Redo checks. The full Linux Xorg/XTest workflow passed the painting,
masking, save/reopen, PNG/OpenRaster, recovery, gradients, shapes, vector handles,
and history assertions; it generated 64 PNGs. I visually inspected the cubic
handle workspace, mask editing, populated light Home, populated narrow Recovery,
and populated narrow Layers/Reference captures. The XTest run measured OpenRaster
export of the 1800 × 2400 vector fixture at 12.834 seconds in this debug-build,
llvmpipe environment; this is not a release or hardware performance result.
Windows GNU checks are compile/lint validation only, and hosted Actions for this
working tree are pending. Windows GUI startup, macOS GUI startup, physical pen,
Narrator/NVDA, and power-loss durability were not exercised here.

### Line-to-cubic conversion follow-up (2026-10-29)

`paint-document` covers line-to-cubic conversion, exact pixel preservation,
native v3 round trips, undo/redo, and atomic rejection:

```sh
cargo test --locked -p paint-document --test vector
```

The current full local check passed formatting, strict host and Windows GNU
Clippy, workspace build, Windows GNU target check, 366 workspace tests, and the
required llvmpipe GPU smoke. The complete Xorg/XTest artwork script exited
successfully after exercising conversion through the real anchor menu,
save/reopen, undo/redo, cubic-handle editing, mask operations, and the broader
painting/export/recovery regression. It produced 64 PNG screenshots. The exact
Color Mixer redo mismatch seen in an earlier run was traced to floating-point
metadata drift from JSON parsing; serde_json `float_roundtrip` now preserves the
native geometry through repeated decode/encode cycles and the full UI redo test
passes. No project schema changed.

GitHub Actions run
[`40867515476`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40867515476)
passed Windows and macOS. Its Linux alpha artwork step exceeded the old
15-minute timeout, so the workflow now gives that unchanged step 25 minutes.
The updated workflow passed all platforms in run
[`40887124571`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40887124571),
including the Linux artwork workflow, and produced the Windows canvas and
stylus diagnostic artifacts. The local Linux Xorg test and hosted builds do not
validate physical S Pen behavior, Windows GUI startup, macOS startup,
Narrator/NVDA, or power-loss durability.

### Canvas-first handoff regression coverage (2026-10-29)

The handoff regression checkpoint adds one mask-engine test and one mixed
raster/mask/vector document integration test. The mask test checks a single
stroke crossing into a rectangular selection, sample-by-sample clipping,
raster-tile isolation, Undo/Redo, and a wholly outside no-op stroke. The mixed
document test persists populated raster and mask tiles with an edited cubic
handle, then verifies exact native Store reopen, journal recovery, PNG pixels,
OpenRaster contents, and unchanged native state after export.

```sh
cargo test --locked -p paint-document --test masks
cargo test --locked -p paint-document --test vector mixed_raster_mask_and_edited_vector_survive_store_journal_and_exports
```

The Linux XTest artwork workflow also tests real mouse strokes crossing and
missing a mask selection, Stop Mask followed by an ordinary raster stroke, and
an uninterrupted vector create/select/convert/new-handle-drag/save/reopen
sequence. It checks native tile tables and document-space bounds instead of
treating a control click or screenshot as proof of functionality. That expanded
window workflow must pass in the hosted Linux Xorg job before this integration
checkpoint is considered validated. The current cloud shell has no local X
server/Xvfb command, so here only its Python syntax and the portable engine
tests are checked locally; the workspace suite and GPU pixel smoke are separate
validations, not substitutes for the XTest workflow.

## Canvas-first Inspector and reference regressions

The unified Inspector reuses the existing egui/winit and AccessKit integration;
it does not replace the input or renderer architecture. The layout unit tests
cover desktop docking with a protected canvas minimum, responsive compact/full-
sheet selection, toolbar collision avoidance, temporary viewport clamping,
bounded reference PIP size, and persistence of intentional PIP moves. Inspector
tabs are tested as named selected radio controls in AccessKit output. Workspace
preferences round-trip Inspector destinations and PIP placement independently
from `.rpl` documents.

The Linux Xorg/XTest `Linux alpha artwork workflow smoke` exercises the populated
Layers stack and contextual actions, switches among Inspector tabs, imports and
selects multiple reference images, pans and wheel-zooms the actual preview,
pins one image as a bounded canvas PIP, switches tabs while it remains visible,
and verifies that it never opens the legacy full-size reference windows. It
captures the real application at constrained and standard sizes; screenshots
must be inspected for clipping and canvas obstruction rather than counted as a
functional assertion by themselves. In a shell without Xorg/Xvfb, this workflow
is **unavailable**, not passed; only portable Rust tests and GPU smoke can run.

Editable vector uniform scale/rotation and duplication are document
transactions. Their tests check bounded geometry, raster/mask separation,
native v3 reopen, and exact Undo/Redo. RPL v1/v2 reading and the v3 schema are
unchanged. Application-level vector manipulation still requires the Linux
XTest workflow; Windows GNU cross-checks do not establish native GUI or pen
behavior.

## Vector-only SVG export

The SVG exporter writes visible editable line/cubic paths and closed even-odd
fills, preserving path order, visibility, stroke/fill color and alpha, and layer
opacity. It excludes the document background, raster pixels, and masks and
reports non-Normal layer blending as unsupported. Native project data is never modified, and existing
export files are not overwritten.

Run serializer and no-clobber checks with:

```sh
cargo test --locked -p paint-document --test svg
```

The Linux Xorg/XTest application workflow invokes Ctrl+Alt+E on a populated
editable vector document, parses the resulting SVG, checks that a cubic filled
path is still represented by path commands, checks that vector-only omissions
are reported, and compares the `.rpl` source bytes before and after export. It
also captures the `vector-svg-export` UI state. SVG import and unsupported SVG
shape/style features remain future work.

The GitHub Actions matrix also runs the portable Python helper tests with:

```sh
python3 -m unittest discover -s scripts -p 'test_*.py' -v
```

These cover incremental GUI smoke-log appends, partial lines, cached event
counts, stable values, and truncation. The Linux anchor-editor scenario keeps
the real Edit anchors click, then verifies the selected path's live editor state
and a rendered anchor handle; event markers alone are not considered proof of
the interaction.

### Canvas-first Inspector Layers checkpoint

The Layers hierarchy adds bounded raster and mask thumbnail checks, target
selection, the actual editable vector-child visibility transaction, and
AccessKit checks for named 44-point targets. Vector visibility is still the
existing native `VectorPath.visible` field; no `.rpl` schema was changed. Run
the relevant portable tests with:

```sh
cargo test --locked -p canvas-demo layer_thumbnails
cargo test --locked -p canvas-demo populated_layers_inspector
cargo test --locked -p paint-document --test vector vector_path_visibility
```

The complete `scripts/alpha-workflow-smoke.py` Xorg/XTest workflow uses the real
window to select raster/mask/vector targets, toggle path and layer visibility,
scroll populated layer rows, reach mask Edit/Stop, reorder layers, and verify
thumbnail changes and native save/reopen/Undo/Redo without changing unrelated
raster or mask tile data. At 640 × 480 it checks the compact single-scroll
fallback; at roomier sizes it checks a distinct layer stack viewport and the
selected-layer footer. The Inspector-specific viewport marker is used for row
clipping checks because a legacy detached Layers surface can be present at the
same time. The local checkpoint run produced 79 PNGs; inspected populated
Layers screenshots covered 640 × 480, 1280 × 800, and 1920 × 1080. That local
result is not the required six-layout/theme acceptance and does not replace
hosted Windows/Linux/macOS Actions.

### Canvas-first Inspector Milestone 2 — navigation and vector controls

The compact and desktop Inspector use the same Layers / Brush / Edit / Color /
References order. AccessKit regression tests inspect the named roles and order,
the fixed close/Canvas action, keyboard selection, and minimum 44×44 logical
point bounds. The compact strip opts into pointer/touch scrolling; the
application test marker reports its viewport, content width, and scroll offset.

The full Linux `scripts/alpha-workflow-smoke.py` test activates the selected
path's Edit anchors action with a real pointer click and no `H` shortcut
fallback. It then checks the live node-edit state and a rendered selected
handle. Existing direct-canvas path movement and cubic-handle editing remain
tested through pointer input and native save/reopen/Undo/Redo. Vector style,
anchor navigation/insertion/deletion, explicit current-color application, and
raster/mask isolation are included in the running-app regression. At compact
size, the tab test performs drag scrolling, asserts that References is fully
within the tab viewport, and selects it through its visible pointer target.

M2's local Linux run ended with `ALPHA_WORKFLOW_SMOKE: PASSED` and captured 80
PNG files. I inspected the populated `layers-panel-populated-light-narrow`,
`reference-inspector-dark-narrow`, `vector-node-editor`, and
`home-dark-high-contrast` captures. They demonstrate actual application states,
not the full six-layout acceptance: Inspector width/placement at the six
specified viewport and scale combinations and independent QA remain Milestone
3/4 work. Python helper tests can be run with:

```sh
python3 -m py_compile scripts/alpha-workflow-smoke.py
python3 -m unittest discover -s scripts -p 'test_*.py' -v
```

The complete Rust checks and Linux artwork workflow must still pass on the
implementation commit, and the existing hosted Windows/Linux/macOS Actions
matrix must pass before beginning the next milestone.

### Close-time project drafts and recovery checkpoints

The focused Linux XTest workflow exercises the real close path and local storage:

```sh
cargo build --locked -p canvas-demo
xvfb-run -a -s "-screen 0 1600x1000x24" python3 scripts/close-save-smoke.py
```

It paints through the live window, verifies a periodic recovery checkpoint
contains raster pixels, closes an untitled document into an editable `.rpl`
draft, reopens that draft through the application, paints again, and closes the
named project. It also injects an unavailable draft directory and verifies that
the application reports the filesystem cause, keeps artwork open, and permits
Retry, Cancel, and Save As recovery paths. A document-level test round-trips a
draft containing raster pixels, an editable layer mask, and vector geometry.
The document storage tests separately inject interrupted writes and verify the
previous valid project remains recoverable. The XTest test uses temporary state
directories and requires Xvfb, XTest, a working software GPU, and the normal
Linux GUI libraries; a local run without those services is not a pass. None of
these checks reproduces or validates the owner's Windows filesystem behavior.
