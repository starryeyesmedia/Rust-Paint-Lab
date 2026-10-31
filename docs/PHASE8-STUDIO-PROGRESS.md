# Phase 8 — Creative Studio Overdrive progress

## Approved UI implementation — Inspector responsiveness and reference safety (2026-10-31, local validation)

This short-lived implementation branch starts from verified `main`
`b0a4d7e60ec3c86dc45a1c8f209e64c50fe2495e`. The unified Inspector now chooses
first-run dock widths from the effective logical viewport: 360 points at
1920×1080, 336 at 1280×720 (the 150% 1920×1080 case), 320 at 1280×800, and 288
at 853×533. The 640×480 drawer remains 256 points when the canvas budget allows;
the 150%-scaled compact viewport uses the full-height sheet. An explicit
workspace preference distinguishes an artist-resized dock from a responsive
default; older version-1 settings with a non-default width migrate as customized.
Temporary viewport clamping does not replace the roomy saved width.

Pinned references now avoid the movable palette, Inspector, and visible
Zoom/Fit controls for all docking edges. Dragging within 24 logical points of a
safe edge snaps the reference overlay there; movement remains workspace-only,
temporary clamping is not saved, and a pointer gesture on the overlay remains
excluded from canvas painting. The Linux Xorg/XTest artwork workflow now tests
the actual pointer drag, edge snap, and absence of a canvas stroke.

Rust 1.90 local validation passed formatting, strict workspace Clippy, the full
workspace suite (**405 tests**, including the llvmpipe/GL GPU smoke), workspace
build, and the Windows GNU workspace cross-check. Python syntax compilation of
the expanded XTest workflow passed. This environment has no Xvfb/XTest server,
so the full Linux artwork UI gesture has not been run locally. The new branch's
Windows/Linux/macOS Actions run is pending until push; no hosted result or
Windows artifact is claimed yet. The six-layout populated visual review remains
Milestone 4, and the approved icon/grouped-toolbar and visual/accessibility
stages remain ahead. This is not a complete approved UI or hardware evaluation.

## Canvas-first Inspector Milestone 2 — tabs and vector editing integration (2026-10-30, locally verified)

This checkpoint builds on `3427604` on `feature/studio-overdrive-phase-8`.
The Inspector tab strip now uses the approved consistent order **Layers / Brush
/ Edit / Color / References** in both compact and desktop layouts. Compact
navigation scrolls horizontally; the Canvas/close action stays fixed outside
the scrolling region. Tab controls remain keyboard/touch operable and are
represented in the AccessKit tree as ordered, named selection controls.

The Inspector Edit tab now exposes the selected vector path's existing editing
operations in context: anchor editing and selection, anchor coordinates,
Bezier-handle controls, line-to-cubic conversion, midpoint insertion/deletion,
stroke width and color, with an explicit action to apply the current color.
Direct canvas selection, path movement and handle dragging remain available.
The Linux XTest regression activates Edit anchors by pointer alone (without
using the `H` shortcut), then confirms the application enters anchor-edit mode
and renders the node handles. Native save/reopen, Undo/Redo, raster and mask
table preservation remain exercised by the same artwork workflow.

The full Linux Xorg/XTest artwork workflow passed locally with **80 screenshots**.
It scrolled the compact tab strip from offset 0 to 181 logical points, verified
that References was wholly inside the tab viewport, and selected it by pointer.
I inspected the populated 640×480 Layers and References views, the rendered
vector node editor, and the high-contrast Home capture. The narrow workspace is
usable for these scenarios but remains intentionally dense; this is not the
six-layout/theme visual acceptance. Local Rust 1.90 validation passed
formatting, strict workspace Clippy, workspace build, Windows GNU workspace
check and strict target Clippy, **400 workspace tests**, and required llvmpipe/GL
GPU smoke. Python helper tests passed (2 tests). GitHub Actions for this M2
commit are pending after push; no Windows/macOS result is claimed yet. Software
GPU and XTest runs do not validate native display scaling, Narrator/NVDA, or
physical S Pen behavior.

The next approved UI slice is Milestone 3: viewport-derived Inspector widths,
temporary size/scale adaptation without preference overwrite, and reference
overlay edge snapping that protects canvas controls. Milestone 4 still requires
six-layout, multi-theme visual review and independent QA. Phase 8.3 remains
active.

## Phase 10 — vector-only SVG export integration (2026-10-30, local checkpoint)

Added bounded SVG export for the editable line and cubic paths already supported
by the native document model. Output preserves document dimensions, path and
layer ordering, layer visibility/opacity, path visibility, supported stroke/fill
color and alpha, and even-odd fills. SVG writes are capped at 16 MiB and use a
synchronized temporary file plus no-clobber publication; they never replace an
existing export or change the source `.rpl`. No dependency or native-format
change was required. The UI offers File → “SVG vector paths only…” and
Ctrl+Alt+E, then reports the omissions applicable to the actual document.
Background color, raster pixels, masks and non-Normal blend behavior are not
represented; SVG import, arbitrary SVG content and complete document round-trip
are explicitly unsupported.

Regression coverage parses the XML, checks cubic/fill geometry and style data,
verifies output ordering attributes and escaping, checks the no-overwrite rule,
distinguishes transparent from solid-background reports, and compares native
source bytes around a real UI export. Rust 1.90 validation passes: formatting,
strict host Clippy, workspace build, Windows GNU workspace check and strict
target Clippy, **394 workspace tests**, and required llvmpipe/GL GPU smoke. The
full Linux Xorg/XTest artwork workflow passes with **78 screenshots** and
exercises the Phase 9 mask actions, vector authoring/editing/fill/save/reopen,
PNG/OpenRaster, recovery, and the new SVG export/status assertions. I inspected
the actual `vector-svg-export.png`, populated narrow Inspector, and high-contrast
Home captures. The narrow populated Layers inspector remains crowded and is
still an open canvas-first layout issue.

These are local results; hosted run
[`41094100891`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41094100891)
passed Windows and macOS, including Windows canvas/diagnostic packaging and
startup, but failed Ubuntu step 11 (`Linux alpha artwork workflow smoke`) with
exit code 1 after 11m 17s. Its public annotation is truncated before the
assertion; the environment blocks `api.github.com` and unauthenticated artifact
downloads. The workflow now puts the final 2,800 characters of this failure log
into the step summary and annotation so the next run exposes the assertion. No
test has been weakened. The host GPU was llvmpipe/GL and does not establish Iris
Xe performance, advanced S Pen behavior, or Narrator/NVDA operation. Phase 8.3,
Phase 9 hardware validation, and Phase 10 remain active. Next: identify and fix
the actual failing Linux assertion, rerun the matrix, then correct the compact
Inspector tab order/navigation and continue vector-tool authoring.

## Phase 10 — closed vector paths and editable fills (2026-10-30)

The continuation from `85e5375` adds closed-path and fill editing to the existing
vector workflow. Closing and reopening a path is an undoable document operation;
closing an open path adds a straight final segment, and reopening removes its
fill within the same edit. Closed paths can store an optional straight-alpha
sRGB even-odd fill independently from their stroke. Fill membership is evaluated
from deterministic flattened line/cubic segments, contributes to hit-testing,
layer previews, compositing, PNG export, and OpenRaster's documented flattened
pixel output. Fill work is checked against the existing bounded vector work
budget before the document accepts it.

Filled objects use the explicit `RPLALP04` format marker. The v4 metadata extends
the existing vector-path data with an optional `fill` RGBA value; the field is
omitted for unfilled objects. Existing v1, v2, and v3 projects remain readable,
and closed-but-unfilled paths continue to serialize with the exact v3 schema.
Raster and mask tile tables remain unchanged by vector fill edits. Tests cover
v3 byte-compatible closed paths, v4 round trips, fill pixels, PNG/OpenRaster
pixels, raster/mask isolation, Undo/Redo, reopening a filled document, and
rejection of a high-cost full-size polygon.

The contextual V-tool strip now offers Close/Open path, Fill/Update fill and
Remove fill, with transformations, duplication, and deletion grouped behind a
compact keyboard-accessible Path actions menu. This avoids the prior wrapping
defect that pushed width and color controls offscreen. The Linux Xorg/XTest
workflow exercised close, fill, save, undo/redo, reopen, and OpenRaster export
through the running application. It completed with **77 rendered screenshots**;
I inspected `/tmp/bpx-vector-fill-approved/vector-filled-path.png` and verified
the closed fill is visible while the Layers inspector remains on-screen.

Rust 1.90 checks pass: formatting, strict workspace Clippy, **391 workspace
tests across 24 targets**, workspace build, Windows GNU workspace check and
strict Windows GNU Clippy. The GPU smoke passed using software llvmpipe/GL; the
full Linux Xorg/XTest artwork workflow exited successfully. GitHub Actions for
this checkpoint are pending after push. This does not validate Windows/macOS
GUI startup, the owner's Iris Xe rendering or advanced pen behaviors. Phase 8.3
and Phase 10 remain active. Next Phase 10 slice: SVG import/export for the
currently supported editable line/cubic paths, fill and stroke styles, with
explicit handling for unsupported SVG features.

## Phase 10 — connected cubic path authoring (2026-10-30)

Commit [`95b098c`](https://github.com/starryeyesmedia/Rust-Paint-Lab/commit/95b098c)
adds an opt-in **Smooth new paths** setting to the existing V tool. Freehand
samples are converted deterministically to connected editable cubic Bézier
segments through every captured anchor. The first and last anchors remain exact
and interior joins share tangents. The preference is workspace-only, defaults
off (including when loading older preferences), and is captured at pointer-down
so changing a control cannot alter an active gesture. Polyline behavior remains
the default.

The document API creates the path in one undoable transaction and renders it
from the existing v3 vector object data. Raster and mask tiles remain
authoritative and unchanged; no native project schema migration was needed.
Tests cover invalid and over-limit input, C1 tangent continuity, exact anchor
preservation, rendering, v3 save/reopen, raster isolation, exact Undo/Redo and
backward-compatible preference loading. The real Xorg/XTest artwork workflow
enabled the actual contextual control, drew with the V tool, verified exact
saved pointer endpoints, cubic segment count, raster/mask isolation, undo/redo,
application reopen and persisted workspace choice. It passed alongside the
existing mask, QuickShape, gradient, recovery and export workflow.

Local Rust 1.90 validation passed: formatting, strict workspace Clippy, all
**388 tests across 24 targets**, workspace build, Windows GNU workspace check
and strict Windows GNU Clippy. Required Vulkan offscreen GPU smoke passed on
Mesa llvmpipe (`CPU` adapter). The complete Linux Xorg/XTest run generated 76
screenshots; I inspected `vector-smoothed-path` at 1280×800 and confirmed the
enabled control and resulting curve are visible in the running workspace. This
is software-rendered automated validation, not Windows/macOS GUI, Iris Xe, S Pen,
Narrator or NVDA validation. The preceding Inspector commit passed Actions run
[`41039508985`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41039508985)
on Windows, Linux and macOS; this commit still needs its own hosted run after
push.

Phase 8.3 remains active. The next Phase 10 slice is SVG interoperability for
the supported editable path subset; document and import/export validation must
continue preserving raster/mask tile isolation and legacy v1-v3 projects.

## Canvas-first Inspector and reference integration — local validation (2026-10-30)

This continuation starts from `688db12` on `feature/studio-overdrive-phase-8`.
It completes the tabbed Layers / Brush / Edit / Color / References Inspector
integration and the outstanding application-workflow regressions. The Inspector
docks beside the canvas on roomy windows and becomes a bounded floating drawer
on constrained windows. A 640×480 regression preserves a 319-point clear canvas
strip beside the movable palette. The drawer is kept inside the viewport, its
temporary dimensions do not overwrite roomy saved preferences, and the
References destination appears early enough in the compact picker to remain
clickable above the status area.

The Layers section uses the actual document state, cached bounded raster
thumbnails, accessible visibility/lock controls, contextual actions and
selection-aware clear with undo. Brush and color controls are reached through
the shared Inspector. The Reference section keeps multiple independently
selectable boards with pan/zoom, opacity, positioning and import controls;
pinning displays only the selected image as a bounded canvas overlay. Reference
pixels remain outside the project raster and all artwork exports. The selected
reference viewport remains available for input routing while the Inspector or
picture-in-picture is shown.

The full Linux Xorg/XTest workflow passed, including populated Layers and mask
operations, narrow Inspector navigation, multiple references, shape/gradient
painting, editable vector transforms, undo/redo, save/reopen, PNG/OpenRaster,
and journal recovery. It captured **75 rendered screenshots** in
`/tmp/bpx-inspector-checkpoint-68`. I inspected dark and light Home at narrow and
wide sizes, New Artwork, populated dark Layers, populated dark References,
populated light Recovery, and the narrow canvas. The populated panels remain in
bounds and scroll to their lower controls; the compact References picker and
selected image are visible in the narrow screenshot. Home content is scrollable
at 640×480, so the lower recent-artwork section is reached by scrolling. These
screenshots show the actual running UI, not design mockups.

Local Rust 1.90 validation passed: formatting; strict workspace Clippy; **384
tests across 24 targets**; workspace build; Windows GNU workspace check and
strict target Clippy. The required Vulkan offscreen GPU smoke passed on Mesa
llvmpipe (`CPU` adapter), and the complete Linux XTest application workflow
passed. This validates software rendering and automated UI interaction, not the
owner's Intel Iris Xe laptop or S Pen hardware. Cross-platform hosted Actions for
this commit are pending until it is pushed; the previous green Actions runs do
not cover this continuation. Windows/macOS GUI execution and hardware
accessibility checks remain separate validation tasks. This commit passed the
Windows/Linux/macOS matrix and produced its Windows/Linux canvas artifacts in
Actions run [`41039508985`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41039508985).

Phase 10 already has editable cubic paths and undoable path transforms; the
next continuation adds optional smooth-path creation without changing the
document schema. Phase 8.3 and Phase 10 remain active. Do not merge, release, or
infer hardware validation from CI.

## Canvas-first design handoff — regression coverage checkpoint (2026-10-29)

The approved canvas-first design handoff starts from `64c2a3e` on
`feature/studio-overdrive-phase-8`, whose last confirmed cross-platform run was
Actions [`40890033654`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40890033654).
The first implementation slice addresses the handoff's uncovered document and
application workflow cases before changing the workspace layout:

- A document regression paints a mask with a stroke that begins outside and
  crosses a rectangular selection, verifies every changed sample lies within
  selected pixel centers, checks that outside-only strokes are no-op history,
  and preserves raster tiles across mask edits.
- A mixed native-document regression creates real raster pixels and populated
  editable mask tiles, converts and edits a cubic vector handle, then verifies
  exact Undo/Redo, `.rpl` save/reopen, recovery-journal restoration, PNG pixels,
  and OpenRaster output without mutating the source document.
- The Linux XTest application workflow now crosses a selection boundary with
  real mouse input, verifies saved mask samples against document-space bounds,
  exercises an outside-only stroke, clicks Stop Mask and proves the next brush
  stroke changes raster tiles without changing masks, and runs
  create/select/convert/drag-new-handle/save/reopen as one continuous vector
  workflow.

All **368 workspace Rust test cases** pass locally, including the new mask and
mixed-document regressions. Rust 1.90 formatting, strict host Clippy, strict
Windows GNU-target Clippy, workspace build, workspace Windows GNU check, and
required llvmpipe/OpenGL GPU smoke pass. The expanded application XTest workflow
and hosted Actions matrix remain pending for this checkpoint. The current cloud
shell has no `Xvfb`/`xvfb-run`; the attempted system-package install was blocked
by read-only system directories, so no local window/screenshot result is
claimed. The workflow remains in the existing hosted Linux Xorg job. No
Inspector or Phase 10 feature work is counted in this regression checkpoint
yet.

## Latest owner-usability checkpoint — Linux workflow timeout diagnosed (2026-10-28)

The active continuation starts from the successful owner-tested `b50e7ec`
Windows launch and basic S Pen drawing. It preserves that hardware result as the
baseline; no new physical pen or laptop demo was requested or performed. The
latest application implementation commit before CI follow-up is `2979c02` on
`feature/studio-overdrive-phase-8`.

The Windows canvas executable is configured as a GUI application, with bounded
local diagnostics and a native startup error dialog. AccessKit is still created
before the window is shown, and GPU initialization completes before the visible
window appears. The independent stylus diagnostic and WM_POINTER pipeline are
unchanged. Actions run `40616773433` passed the Windows visible-startup smoke and
packaged both Windows artifacts; macOS passed as well. The earlier run
`40611993952` exposed a macOS-only test assumption about
`~/Library/Logs/BitchPixel`. The diagnostics test now accepts that standard
macOS location while remaining constrained on other platforms.

Home now uses an action-oriented “Create, open, or continue artwork.” heading,
the exact “No limits. No bullshit.” line with a sampled contrast-safe purple to
magenta text treatment, and direct Portrait/Square/Landscape document creation.
The standard New Artwork dialog puts the name field immediately below its
heading, keeps the primary flow visible at 1280 × 800, and provides scrolling at
640 × 480. Home preset bounds are logged only when responsive geometry changes,
so the XTest harness clicks the current visible card after resize rather than a
stale position. The established keyboard-accessible, illustrated format cards
and advanced precision settings remain available.

The hidden Reference scroll crash found during the dark narrow-dialog test is
fixed. A stale saved reference area can outlive a hidden panel; wheel input at
that position is now forwarded to egui's visible modal instead of unwrapping a
missing reference viewport or being swallowed. The XTest regression checks the
stale-reference path, application survival, and that Create scrolls into view.

On pinned Rust 1.90, `cargo fmt --all -- --check`, strict workspace Clippy,
workspace build, Windows GNU workspace check/Clippy, all **282 workspace tests**
and the llvmpipe/OpenGL offscreen GPU smoke pass. The complete local Xorg/XTest
workflow passes actual painting, selection/fill, shapes, linear/radial gradients,
layer reorder/copy/visibility/opacity/Multiply with save and undo/redo checks,
PNG/OpenRaster export/import, save/reopen, and forced process termination with
journal recovery. It captured **50** rendered screenshots. I inspected light and
dark Home, recent artwork, normal and narrow New Artwork, committed gradient,
populated dark Layers/References and Recovery, and high contrast. The populated
640 × 480 Layers/Reference panels do not overlap and remain in bounds, but leave
too little open canvas; compact panel ergonomics remain an identified defect.
The screenshots and logs are local under `/tmp/bp-screens-phase8c-final` and
`/tmp/rpl-alpha-window-8sh83476`.

### Linux CI duration diagnosis

Actions run [`40616773433`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40616773433)
passed the Windows and macOS jobs, including packaging the Windows canvas and
stylus diagnostic artifacts and the macOS alpha artifact. Linux formatting,
strict Clippy, workspace tests/build, and the replay and live-mouse window
smokes also passed. Its `Linux alpha artwork workflow smoke` step alone was
terminated at the configured five-minute timeout. The Windows artifacts from
that run are available as `rust-paint-lab-canvas-demo` and
`rust-paint-lab-windows-diagnostic`.

To distinguish a slow test from an application stall, I rebuilt the exact
`1978106` checkout and ran the complete Linux Xorg/XTest workflow locally with
the same software-rendered configuration. It completed successfully in **336.7
seconds**, captured all 50 screenshots, and reached the final artwork,
save/reopen, export, and forced-crash recovery assertions. The application kept
progressing through the scenarios; no hung process or failed workflow
assertion occurred. The evidence points to the five-minute CI cap being shorter
than this comprehensive software-rendered test, not missing coverage or a
stalled app. The workflow cap is now raised to 10 minutes; all scenarios and
screenshot assertions remain enabled. This adjustment awaits Actions
verification on its follow-up commit.

### Ten owner findings — current status

- **1. Console-free Windows launch — implemented and verified by the packaged
  Windows startup smoke; physical retest of this latest package is pending.**
- **2. Tool icons — partially addressed.** Selection silhouettes are distinct;
  the full icon discoverability and toolbar-density review remains.
- **3. Home/New Canvas — partially addressed.** Direct standard presets, visual
  hierarchy, top-positioned name, and responsive Create action are working;
  broader populated/missing-file/recovery Home composition still needs review.
- **4. Workspace controls and Layers — partially addressed.** Existing layer
  editing is exercised end to end. The constrained two-panel layout is bounded
  but too cramped, and the unified workspace redesign remains incomplete.
- **5. Full artist workflow — automated slice passes** through paint, edits,
  layers, save/reopen, export and recovery; this does not establish all owner
  workflows or physical-device performance.
- **6. S Pen side button — pending.** Barrel-button capture is preserved; no
  configurable temporary tool action has been implemented.
- **7. Smudge — pending.** There is no pixel-smearing tool yet.
- **8. Place Image — pending.** PNG layer import exists, but interactive move,
  scale, rotate and fit controls are not implemented.
- **9. Custom frame — deferred.** The reliable native frame remains; no safe
  Windows custom-frame behavior has been demonstrated.
- **10. Native Windows startup and hardware validation — automated startup passes;
  physical retest of this latest package and advanced stylus behavior remain
  pending.** The existing b50e7ec launch/basic S Pen result remains the only
  physical validation claimed.

### Exact continuation

1. Push the Linux timeout adjustment and this progress update; verify the full
   Linux artwork workflow within its 10-minute cap while Windows startup/artifact
   and macOS first-frame checks remain green.
2. Advance Phase 9 with a real transaction-safe Smudge tool; do not let compact
   panel refinements block creative-engine work.
3. Implement configurable WM_POINTER barrel-button actions, then transformed
   Place Image. Preserve native-window behavior unless safe custom chrome is
   proven, and retain existing artwork-format compatibility.
4. Keep Phase 8.3 active; do not merge or release.

## Phase 8.1 — Studio refinement (active)

The Phase 8.1 working slice is in progress on
`feature/studio-overdrive-phase-8`. Commit `039fa2a` contains the first pushed
refinement slice. It preserves the Phase 8 document, draft, autosave and recovery
work below.

### Completed in commit `039fa2a`

- Reference import now accepts common phone/reference photos up to 32 MiB on
  disk, 16,384 pixels per source edge, and 64 MiB of decoded source data. Large
  sources are reduced to a maximum 1,448-pixel edge before GPU upload, keeping
  each texture under 8 MiB. Per-file failures include an actionable reason and
  do not discard successfully imported siblings. The app reports the original
  image dimensions, persists accessible source paths/layout and requests another
  frame after texture creation. The current texture is a workspace preview; image
  pixels never enter document state or PNG/OpenRaster export. A generated
  2400 × 1800 JPEG test covers bounded import, oversize-file rejection, duplicate
  handling and partial success. Source decode and GPU rendering have not been
  exercised on the user's Windows device, so the precise original failure for
  that photo remains unconfirmed.
- Inspection of the original import path identified two concrete failure modes:
  it rejected sources above 2048 pixels on either edge and applied an 8 MiB
  decoder allocation limit before creating a preview, which excludes many
  ordinary phone photos. More critically, after texture creation the action
  returned early without requesting another frame; egui had already collected
  the frame's texture updates, so the new floating window and its status could
  remain unseen until unrelated input caused a redraw. The revised batch path
  scales bounded large sources, reports each rejection, persists successful
  siblings, and requests redraw. This maps the failure to code-path defects but
  does not prove which one affected the user's specific Windows selection.
- Replaced toolbar/menu Unicode symbols with original egui vector geometry for
  the functional Brush, Eraser, Rectangle, Ellipse, Lasso, Eyedropper, Deselect,
  Fill, Undo, Redo, Open, Save, Layers, Color and Settings actions. Buttons retain
  readable labels, tooltips, selected state and their existing click/key behavior.
  An egui geometry test confirms every icon emits shapes; a separate window-level
  visual check is still pending.
- Added the shared Midnight Violet appearance palette and complementary light
  palette, Dark/Light/Follow System modes, built-in Violet/Blue/Teal/Coral/Green/
  Neutral accents and validated custom HEX entry. Appearance state is in local
  workspace preferences, not artwork. Older preferences migrate from `dark_theme`;
  invalid custom settings fall back safely. Contrast tests check normal text at
  4.5:1 and active control contrast at 3:1 for supported palette combinations.
- macOS Actions run `40123065344` passed the draft persistence test and the full
  platform build/lint path; this verifies the atomic unique temp-directory fix
  introduced in `f8db5a5`.
- The Linux picker smoke still failed on `f8db5a5`: after Ctrl+Enter the title
  remained `TEST FILE PICKER`. Commit `039fa2a` first latched Enter/Numpad Enter
  at the winit boundary, but Actions run `40123715340` showed the modal still did
  not submit. Commit `212b77d` now recognizes physical/logical Enter only while
  the test-only dialog is active, closes that harness modal, and queues its
  existing action directly into the next Studio frame. Ordinary users continue to
  receive the native file dialog. Run `40124892298` confirms the Linux test
  harness received and queued four Enter submissions, and advanced through the
  picker flows into OpenRaster import. Local Xvfb is not installed here.
- Floating reference windows now have explicit physical-pixel/DPI-aware hit
  testing in the input adapter. Mouse, pen, touch-navigation and selection starts
  over the reference board are routed to the UI instead of painting or navigating
  the obscured canvas. Their existing move/resize/opacity/close controls can
  receive those events. A bounded-coordinate unit test covers inside/outside,
  invalid scale and nonfinite pointer positions. Commit `b54bf1e` contains this
  change; a window-level CI validation is still pending.

### Current local validation

- Formatting and strict workspace Clippy passed after the new changes.
- `cargo test --workspace --locked` passed, **172 Rust tests**, including the
  new vector icon, palette contrast/follow-system, JPEG reference import, old
  preference migration, and draft temp-directory concurrency tests.
- `cargo build --workspace --locked` passed.
- The offscreen GPU smoke passed on Mesa llvmpipe/OpenGL, including existing
  replay/live/document preview, commit, cancel, undo and redo pixel checks.
- `xvfb-run`/`Xvfb` is absent locally; the actual Linux picker smoke remains CI
  dependent. The existing Linux smoke in run `40123065344` reached the modal but
  could not submit it.
- Run [`40123065344`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40123065344)
  is based on `f8db5a5`: macOS passed; Linux failed at the native-window workflow
  step waiting for `artwork.rpl`; Windows has built/uploaded the separate stylus
  artifact and was still compiling the canvas executable at last inspection.
  No cross-platform pass is claimed for that run.
- After reference-window hit testing, full workspace tests and build, the required
  llvmpipe GPU pixel suite, Windows GNU target workspace check/Clippy, and local
  strict Clippy pass.
- Actions run [`40123715340`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40123715340)
  failed Linux at the same `TEST FILE PICKER` timeout; the UI-level latch in
  `039fa2a` did not resolve it. Commit `212b77d` recognizes physical/logical
  Enter only while the test-only dialog is active, closes that harness modal, and
  queues its existing action directly into the next Studio frame. Ordinary users
  continue to receive the native file dialog. It also emits test-only diagnostic
  markers so a further failure identifies which dispatch stage ran. `40124176543`
  was canceled as stale. The newer run [`40124892298`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40124892298)
  confirms the path picker submits and reaches the recovery portion of the Linux
  smoke, where it then fails waiting for an initial recovery snapshot.

### Completed workspace refinement in `885b213`

- The brush list now uses artist-facing names (for example, Hard round, Ink pen,
  Soft brush and “Wash · experimental”), and the erase toggle is labelled
  “Eraser mode.” Advanced dynamics are grouped under “More brush settings.”
- The layer area is now a distinct headed section. Its list keeps selection,
  visibility and lock in a compact row; rename, percentage opacity and blend mode
  live under per-layer “Layer settings.” New, duplicate and delete actions are
  grouped; delete is disabled when one layer remains, and Raise/Lower are disabled
  at stack boundaries. Top-of-list is explicitly identified as frontmost.
- The primary footer no longer displays raw pressure/tilt/button values, tile and
  undo counts, or timing diagnostics; it presents save state, contextual status,
  pixel dimensions and DPI. Follow System appearance requests a frame when winit
  reports an OS theme change.
- Formatting, strict Clippy, all **173 workspace tests**, workspace build, llvmpipe
  GPU smoke, and Windows GNU workspace check/Clippy pass locally. The actual Linux
  window cannot run locally because Xvfb is absent. No screenshot from a running
  Studio window or physical Windows reference-board test is available yet.
- GitHub Actions run [`40124642888`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40124642888)
  is validating `885b213`; all platform jobs had reached full workspace tests at
  last inspection. The test-only path-prompt fix, reference input guard, portable
  Windows packages and Linux virtual-window workflow remain under CI validation.

### Completed in `f672cec`

- The Color section now shows a bounded recent brush-color palette. Selecting a
  color updates the active brush; user-picked, HEX-entered and eyedropper colors
  are remembered newest-first, deduplicated and capped at 12. Colors live in
  local workspace preferences, are sanitized on load, and remain separate from
  the accent color and document artwork. Existing version-1 preferences load with
  an empty palette.
- Persistence/ordering/bounds and named touch-sized swatches have automated tests.
- Formatting, strict Clippy, all **175 workspace tests**, workspace build, llvmpipe
  GPU smoke, and Windows GNU workspace check/Clippy pass locally for this slice.
  At the time of this commit, Actions run `40124892298` had passed Windows and
  macOS but failed Linux because the smoke saved the imported project before
  waiting for its debounced recovery snapshot. Recovery checkpoints are only
  submitted while the project is dirty.

### Recovery and export smoke correction in `8df1340`

- Reordered the Xvfb workflow to observe a valid recovery checkpoint while the
  imported project is still dirty, then explicitly save it. The test continues
  through a durable journal append, forced process termination, recovery with
  the paired snapshot removed, byte-for-byte checkpoint comparison, and an
  assertion that the previously saved source project remains unchanged.
- The smoke decodes its exported PNG and asserts that the independent reference
  image's magenta sentinel pixel does not leak into artwork.
- The Linux alpha workflow passed in Actions run `40125273125` with the corrected
  sequence. Its Windows and macOS jobs passed and uploaded
  `rust-paint-lab-windows-diagnostic`, `rust-paint-lab-canvas-demo`, and the
  macOS alpha artifact; Linux has passed formatting, Clippy, tests, build, and
  all window/recovery smoke checks and is packaging the Linux alpha. The
  preceding `40124892298` Windows and macOS jobs also passed, while its Linux
  job failed at the recovery ordering described above.

### Reference-board navigation slice in `8d2328a`

- Each reference window now supports wheel zoom (bounded from 25% to 800%),
  drag-to-pan inside a clipped image viewport, a position lock, and Reset View.
  Zoom, pan, lock, opacity, size and window position persist as workspace state.
- Version-1 preferences without navigation fields load with fit-view, zero-pan,
  unlocked defaults. Invalid or extreme saved zoom/pan values are rejected or
  clamped before reaching egui. Tests cover legacy defaults and the image fit,
  zoom, pan and invalid-geometry conversions.
- Workspace state remains isolated from the document and export pipeline.
  Local formatting, strict Clippy, all **177 workspace tests**, workspace build,
  and llvmpipe GPU smoke pass. Actual Windows touch/stylus interaction with the
  floating reference board has not been hardware tested.

### Next Phase 8.1 work

1. Verify Actions run `40125783796` for the reference-navigation code, including
   Linux workflow smoke, both Windows artifacts and macOS packaging.
2. Obtain a real Windows run of Add Reference Images with small PNG, phone JPEG,
   multiple selections, one bad file, cancellation and restart persistence; record
   the dialog/result/status behavior before declaring the confirmed issue fixed.
3. Window-level inspect Midnight Violet/icons, the new Brush/Layers layout and
   appearance persistence/custom contrast; test panel resize and reference-window
   move/resize/opacity input routing.
4. Full draggable docking, a richer welcome screen, HSV selection, and a complete
   accessibility audit remain outstanding.

This is the active continuation record. Phase 7 work remains documented in
[the Phase 7 record](PHASE7-STUDIO-PROGRESS.md); it is preserved and not repeated.

## Starting point

- Repository: `starryeyesmedia/Rust-Paint-Lab`.
- Phase 8 branch: `feature/studio-overdrive-phase-8`, renamed locally from the
  previous feature branch without changing its history.
- Starting commit: `4e3afd8` (Phase 7 journal/workspace validation record).
- Latest preceding validated code commit: `26b3173`; Actions run
  [40112079927](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40112079927)
  passed Windows, Linux, and macOS and uploaded the canvas and stylus artifacts.
- Previous checked suite: 148 Rust tests. No merge or release was performed.
- Latest pushed Phase 8 code commit: `100d103` (Ctrl+Enter submission in the
  test-only path picker).

## Implemented Phase 8 slice: welcome, document setup, and file selection

- The studio opens with a start surface offering New Document, Open Existing
  Project, recent saved projects, and detected recovery checkpoints. While it is
  active, canvas input is blocked. New/open/recovery still use the existing
  preservation and native storage paths.
- New Document supports names, pixel/inch/millimeter/centimeter units, DPI,
  orientation swap, current-format-safe presets (including a 3840 × 2160 screen
  preset), RGB background, and transparency. Unit conversion and name sanitization
  are isolated in `apps/canvas-demo/src/document_setup.rs`; the resolver rejects
  dimensions above 4096 × 4096 before creating state. It reports that DPI changes
  print-size conversion only. A blank new document allocates no raster tiles and
  retains the existing 8192-tile/128 MiB cap.
- Project Open, Save As, PNG import/export, OpenRaster import/export, brush preset
  load/save, and multi-select Add Reference use rfd dialogs. Selection still flows
  through current bounded decoders and no-clobber storage. Reference images remain
  workspace-only and cannot enter artwork exports.
- `RPL_TEST_PATH_DIALOG=1` plus optional `RPL_TEST_DOCUMENT_NAME` selects a
  deterministic path-entry test harness for Xvfb only. Normal usage does not ask
  artists to type paths. Its Continue action uses Ctrl+Enter so it works while the
  path text field has focus. Linux native dialogs use the XDG portal and require a
  desktop portal service.
- The Windows stylus diagnostic, live drawing pipeline, recovery journal, and
  existing exporters were not changed.

## Implemented Phase 8 slice: automatic close saves and persistent drafts

- Normal close saves a dirty named project through the existing `Store`, so
  external-change conflicts and atomic backup behavior remain active. Failure
  leaves the app open with retry, Save As, and cancel choices.
- Closing an unnamed project writes a normal editable `.rpl` under a separate
  local `drafts/` directory. Generated filenames include a readable project stem;
  drafts appear on the next welcome screen and reopen through the existing native
  project reader. The list is capped at eight and a failed/full draft save blocks
  close until Save As succeeds or the user cancels. No old draft is deleted
  automatically.
- The welcome screen now offers an explicit confirmation before removing a draft.
  The removal routine accepts only a regular `draft-*.rpl` child of the drafts
  directory and removes its matching storage backup; it rejects symlinks, ordinary
  artwork paths, and paths outside the managed folder. Cleanup is never automatic.
- Empty new documents also become drafts on normal close, so configured dimensions
  and background survive even before a stroke is drawn. Brush-setting changes now
  become revisioned document metadata once synchronized, so recovery/close detects
  them instead of silently losing a settings-only edit.
- Recovery snapshots/journals remain a separate safety mechanism and are not
  treated as the persistent draft itself.

### Validation

- `cargo fmt --all --check`: passed.
- Workspace Clippy with `-D warnings`: passed.
- `cargo test --workspace --locked`: passed, **165 tests**, including draft cleanup,
  backward-compatible preferences, recent dimensions, autosave scheduling, and HEX
  color parsing/validation.
- `cargo build --workspace --locked`: passed.
- Required offscreen pixel test passed on Mesa llvmpipe/OpenGL. Forcing Vulkan in
  this container did not find a Vulkan adapter; the default backend selected
  llvmpipe and completed the pixel checks.
- Windows GNU workspace `cargo check` and Clippy passed. This is cross-checking,
  not Windows linking or execution.
- The real alpha Xvfb window workflow could not run locally because this container
  does not provide `xvfb-run`. CI is configured to run it and now starts with the
  welcome/New Document flow in the test-only path-entry mode.
- GitHub Actions run [`40116227316`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40116227316)
  for `d29bd2f` failed in the Linux window assertion (which expected the old 800 ×
  500 demo size) and macOS supplemental license generation (missing the `block2
  0.6.2` notice). The assertion and full upstream license/provenance were corrected
  in the next pushed commit. Run [`40116926028`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40116926028)
  then passed macOS but failed the Linux alpha window smoke while waiting for the
  reopened window title to show `artwork.rpl`; Windows was still compiling. Draft
  cleanup, remembered settings, and menu/palette commits are newer and still require
  CI. The latest Actions run is in progress for the menu/palette commit; autosave
  debounce and test-prompt synchronization are locally validated but unpushed.
- The second close-save/draft slice has passed focused `canvas-demo` tests and
  now passes complete workspace tests/build/Clippy, required local GPU smoke, and
  Windows GNU target check/Clippy. License generation passes for both Windows MSVC
  and macOS ARM64 target graphs (152 and 164 packages respectively). The real alpha
  Xvfb workflow remains unavailable locally.
- A third slice adds safe, user-confirmed cleanup of persistent drafts so a full
  draft quota can be managed without weakening the bounded storage policy. Its
  regression tests cover backup cleanup and reject unrelated/outside paths. Local
  fmt, strict Clippy, workspace tests/build, and the required llvmpipe GPU smoke
  all pass for this slice.
- Workspace preferences now retain the last successfully created document's size,
  units, DPI, background, and transparency. Older version-1 preference files load
  with safe defaults; malformed or out-of-range settings are rejected before they
  reach the setup dialog.
- The permanent row of file-operation buttons is now a compact File/Edit/Image/
  Layer/View/Window/Help menu bar. Primary brush, rectangle, ellipse, lasso, and
  eyedropper tools moved into a 64-point vertical palette with selected states,
  tool names, shortcut tooltips, and touch-sized targets. Selection fill, erase,
  and deselect actions remain directly below the tools. Menu entries invoke existing
  supported operations; this does not add unsupported shape or transform tools.
- Recovery checkpoints now coalesce dirty revisions until 700 ms without a new edit
  and are not submitted during active pointer contact. A full worker queue or write
  failure backs off for two seconds instead of retrying continuously. The event loop
  sleeps until the next checkpoint deadline and requests a frame only when due. The
  footer distinguishes saved, untitled, unsaved, and active recovery-save states.
  Explicit Save and close-time flush retain their existing semantics.
- The Xvfb window workflow waits for test-only title markers before confirming New
  Document and native-file-picker test prompts. This removes fixed-delay UI races and
  includes the current window title in failed status assertions.
- The Brush panel keeps preset, color, eraser, numeric size, and opacity as primary
  controls. Flow, hardness, spacing, angle, smoothing, pressure response, and tilt are
  grouped in a collapsed Advanced dynamics section. Brush behavior and preset format
  did not change.
- New Document now offers the eight most recently created pixel-size/DPI pairs.
  Entries are deduplicated newest-first and validated/bounded on load; older
  workspace preference files default to an empty recent-size list.
- Brush color now has a collapsed sRGB value editor with validated `#RRGGBB` entry
  and direct R/G/B numeric controls. The existing swatch picker remains available;
  this does not claim ICC color management.
- The Window menu can hide or restore the right Brush/Layers controls panel to
  give the canvas more room. Visibility persists in workspace preferences and old
  preferences default to visible. Hiding the panel does not change brush or layer
  state. The panel remains a single resizable panel; it is not a dockable layout.

### Latest checkpoint validation

- Commit `100d103`: workspace formatting, strict Clippy, all **165 tests**, and
  workspace build passed. The required llvmpipe GPU pixel smoke and Python
  workflow-script syntax check also passed.
- GitHub Actions rerun for `100d103`
  ([40118885388](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40118885388))
  failed before any job step on all three platforms; all jobs had runner ID 0.
  The same runner-provisioning failure affected runs
  [40118673479](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40118673479)
  and [40118752060](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40118752060).
  Retrying the latest run for this progress update
  ([40118971603](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40118971603))
  failed the same way, before any platform steps could start.
  Superseded in-progress runs were canceled to free Actions capacity.
- Linux UI smoke in earlier run
  [40118257862](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40118257862)
  reached the test-only file picker but timed out because synthetic plain Enter
  was consumed by the focused path field. The smoke now submits the test-only
  picker with Ctrl+Enter, which the modal handles explicitly. The correction has
  not yet been verified with Xvfb; `xvfb-run` is absent locally and the latest CI
  rerun failed before setup.
- The Linux Xvfb real-window workflow still cannot be exercised in this container
  because `xvfb-run` is absent. Latest platform CI must confirm the native window,
  dialogs, artifact packaging, and smoke script.
- A Windows GNU workspace `cargo check --target x86_64-pc-windows-gnu` passes for
  the current code. This does not replace the Windows MSVC build, packaging, or
  runtime checks in Actions.

## Partial and unimplemented Phase 8 requirements

- Home is now a full-window surface with recent projects, but has no artwork
  thumbnails, settings pages, or session continuation. The engine's internal
  initial placeholder remains hidden and input-blocked until the artist creates
  or opens a project.
- Document setup remembers the last-created settings and eight recent pixel-size/DPI
  pairs, but has no explicit physical-paper preset catalog beyond current presets or
  automated app-level reopen/export dimension workflow. Preset dimensions remain
  bounded to current format.
- Drafts now have a confirmation-gated remove action, but there is still no
  rename, pin, or age sorting. Recovery checkpoints debounce dirty changes and
  expose basic save status, but store full committed snapshots rather than
  incremental edits.
- File dialogs have compiled on native Linux and Windows GNU targets; Windows,
  Linux portal, and macOS visual/runtime behavior needs CI or manual verification.
- The first compact menus, vertical tool palette, and tabbed Layers/Brush/Edit
  inspector are implemented, and everyday brush/color controls are less crowded,
  but there are no independently draggable/dockable panels,
  snap previews, palette placement/size preferences, complete accessibility review,
  brush thumbnails/library, HSV wheel, recent-color palette, or polished layer panel.
  Menu categories remain limited to currently supported actions.
- Reference board supports multi-image import, bounded internal zoom/pan, touch
  routing and lock/reset controls. It still lacks rotation, project-scoped
  configurations, native drag/drop, and color sampling.
- Shape tools, QuickShape, symmetry, Brush Studio, vector/animation/timelapse and
  natural-media work remain future tasks in the original request.

## Architecture decisions

- Keep unit conversion and new-document validation in the canvas-demo shell's
  platform/GPU-independent `document_setup` module; the shared document engine
  remains responsible for format validation and sparse storage.
- Keep native file browsing in the UI shell. Dialog output is only a path; all
  reads still use existing bounded parsers and all saves retain current conflict,
  integrity, backup, and no-clobber behavior.
- Use rfd 0.15.4 (MIT). Windows and macOS use native desktop integrations; Linux
  uses its XDG portal feature and therefore depends on a desktop portal service.
  No bundled GTK or separately packaged runtime is added to the Windows executable.
- Keep reference imagery outside document state and export pipelines.
- Keep persistent drafts as independently readable native files under the app state
  directory. Do not treat the recovery journal as the intentional draft store.

## Exact next steps

1. Verify the Ctrl+Enter picker fix in Linux Xvfb CI and get a fresh successful
   Windows/Linux/macOS run with the canvas and stylus artifacts.
2. Continue functional workspace work: recent-color palette and more complete
   layer workflow, then accessible rearrangeable panels and reference-board
   navigation. Do not mark dockable panels or a full artist workspace complete
   until those behaviors exist and have window-level tests.

No physical Windows picker, stylus, touch, power-loss, or hardware-performance
validation has been performed in this phase.

## Phase 8.2 — Reference navigation and first artist Home/workspace slice

### Implemented in the current working checkpoint

- Reference windows now recover against `egui::Context::content_rect()` rather
  than a large arbitrary coordinate bound. Saved windows are clamped on resize,
  are limited to the visible area, and expose Center, Reset position, and an
  independent image-view Reset. Zoom is bounded and pointer/finger anchored;
  wheel zoom, two-contact pinch, and drag pan operate inside the image viewport.
- Added a per-contact touch target router for the canvas, an individual reference,
  or suppressed UI/out-of-surface input. A contact remains owned by its starting
  surface; a second finger on another surface suppresses the gesture, and ending
  both contacts clears ownership before pointer ID reuse. Stylus-active touch is
  ignored. Winit touch, pinch, and pan events feed the existing navigation model;
  they do not enter the painting input pipeline.
- Canvas zoom and pan persist in bounded workspace preferences. A small zoom HUD
  exposes percentage, zoom in/out, and Fit Canvas. Reset Workspace restores the
  panel width/visibility, Layers tab, canvas view, and reference panel placement.
- Replaced the centered welcome modal with an opaque full-window Home surface.
  It has primary New/Open actions, a useful first-run state, recent project cards,
  recoverable work and draft lists. Recent cards show the readable filename,
  dimensions/DPI from a bounded header-only read, file size, last-opened age, and
  an explicit missing-file state. `recent.json` now stores per-path UTC open times;
  legacy path-only files load with an unavailable time and update on next open.
  Full project validation/checksum still runs when opening. Recent metadata is
  cached when Home opens/when a project is opened; artwork is not loaded for cards.
- Split the right inspector into persisted Layers, Brush, and Edit tabs. Layers is
  the default; selecting the Brush tool opens the Brush tab. Recovery deletion and
  recent files were removed from the normal painting inspector. The reference
  image action remains directly available from the left tool palette.

### Validation and limitations

- `cargo fmt --all --check`: passed after formatting.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: passed, **183 tests**, including the software
  GPU pixel smoke on Mesa llvmpipe/OpenGL.
- `cargo build --workspace --locked`: passed.
- Windows GNU target workspace check and strict Clippy both passed. This is a
  cross-check only, not a Windows executable build or runtime test.
- GitHub Actions run [`40211959723`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40211959723)
  for commit `1ad2b62` passed Windows, Linux, and macOS. Linux Xvfb passed the
  virtual-window, live-mouse, and alpha artwork/recovery workflows. Windows
  produced separate `rust-paint-lab-canvas-demo` and
  `rust-paint-lab-windows-diagnostic` artifacts; Linux X64 and macOS ARM64 alpha
  artifacts were also uploaded. These are CI builds and smoke checks, not physical
  Windows touch/stylus hardware tests.
- No actual running-window screenshot was available here (CI ran the application
  under Xvfb). There
  are no Home artwork thumbnails yet. Independent docking/snapping, panel dragging,
  recovery manager views, arbitrary window layout presets, and broader accessibility
  review remain incomplete. The reference panel has native egui move/resize rather
  than independently dockable panels.
- Synthetic unit tests validate route ownership, focal-point math, persistence
  bounds and recent-header safety. Windows Precision Touchpad delivery, S Pen and
  touchscreen interactions, high-DPI/multi-monitor moves, palm rejection, and
  Iris Xe performance still require hands-on Windows validation. Trackpad events
  may depend on the OS/window backend; no platform-specific gesture hook was added.

### Exact continuation

1. Manually verify the Home layout, reference windows, touch/trackpad gestures, and
   DPI behavior on the Galaxy Book; the automated CI window workflow passes, but it
   cannot emulate the physical hardware surface.
2. Add a bounded thumbnail cache and an explicit recovery manager with readable
   project labels/timestamps; verify stale paths and drafts remain safe.
3. Implement panel move/resize/snap or docking behavior with interaction tests, then
   validate touch/trackpad behavior on the Galaxy Book before declaring gestures
   complete.

## Phase 8.3 — Accessibility foundation (active)

The implementation notes below are chronological checkpoint records. Any earlier
“pending CI” or “pending visual validation” statement describes that checkpoint;
the current status and verification evidence are recorded at the end of this
section.

### Accessibility foundation and palette slice (validated locally; awaiting CI)

- Added a shared UI token module for spacing, rounded controls, and a 44-point
  baseline interaction target. Applied through the existing egui context; the
  current tool buttons retain their own measured sizes.
- Added persisted interface scaling (bounded 0.8–1.6) and an optional high-contrast
  control palette. Both are in View/Accessibility controls and Reset Appearance is
  separate from workspace layout reset. High contrast changes application chrome,
  not artwork or canvas color. Version-1 preferences without these fields migrate
  to scale 1.0 and high contrast off.
- Added tests for scale migration/bounds, shared target sizing, and text/border
  contrast in both dark and light high-contrast modes.
- Added `docs/accessibility-assessment.md` and the active multi-track engineering
  roadmap at `docs/ROADMAP.md`.
- Enabled egui-winit's optional AccessKit feature and initialized its adapter from
  the existing winit event loop. A typed app event carries AccessKit action
  requests while preserving the Windows pointer-capture observer wake event.
  Generated-tree coverage verifies labels for representative toolbar and document
  controls. This validates tree generation and event routing only; Narrator/NVDA,
  UI Automation, full focus traversal, and complete widget semantics remain
  unverified.
- Replaced the fixed left-side tool strip with a draggable palette that snaps to
  left/right edges or remains floating. Collapse state, floating position, dock
  side, and left/right handedness persist independently of artwork. The palette
  exposes Brush, selection/edit tools, Layers and Reference Images; Brush opens
  its contextual brush inspector tab. Existing light/dark/Follow System and
  high-contrast appearance remains available. Tests cover migration defaults,
  position bounds, and palette preference round trips.
- Updated generated dependency inventory and canvas packaging license supplements
  for AccessKit, AT-SPI, and zbus dependencies.

### Accessibility capability finding

The project pins egui/egui-winit 0.33.3. Its optional AccessKit support is now
enabled, and the typed app event carries both semantic requests and the existing
capture wake. A tree-generation test passes. This is a partial accessibility
foundation; manual screen-reader and full tree/focus review are outstanding.

### Validation for this slice

- Canvas-demo library tests: **43 passed**.
- Full workspace strict Clippy, build, and **187 tests** passed locally. The
  offscreen GPU smoke passed on Mesa llvmpipe/OpenGL. Windows GNU workspace
  cross-check and strict Clippy pass after fixing an event-proxy ownership issue
  found by cross-compilation.
- Offscreen GPU pixel smoke passed on Mesa llvmpipe/OpenGL.
- The latest completed CI before this work is run [`40218184731`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40218184731),
  which passed on commit `4dcfc88` and uploaded separate canvas and stylus
  diagnostic artifacts. This new code has not yet run in GitHub Actions.
- Xvfb/XTest tools are not installed in this execution environment, so no current
  application window was launched or screenshot inspected locally. The prior CI
  window smoke predates AccessKit and palette changes. Do not treat the generated
  AccessKit tree as a visual UI inspection or a screen-reader pass.

### Exact continuation

1. Verify CI for `5ba5c5a`, `2a1fd15` and `01b49b3`; the latest CI runs are in
   progress/queued. Preserve both Windows artifacts.
2. Add image previews to the recovery manager after verifying bounded decoding and
   fallback safety. Add an Xvfb workflow for opening the manager, recovering, and
   confirming discard.
3. Add favorites/search and saved-preset workflow to the contextual Brush library;
   add Color, Layer and Reference flyouts and independently dockable/resizable
   panels. Current handedness persists but only influences dock/reset defaults.
   Make Layers an optional detached panel.
4. Finish Home slogan styling, New Artwork card/layout interaction tests,
   image-first reference chrome, focus traversal and responsive workspace
   persistence. Capture and inspect actual windows in CI or another available
   virtual display before calling visual review complete.

### Visual document creation slice (in progress)

The New Artwork modal now leads with selectable Portrait, Landscape and Square
format cards, followed by a live aspect-ratio preview that shows either the chosen
solid color or a transparency checkerboard. Name remains prominent; exact units,
dimensions, DPI, background, saved presets and recent dimensions remain available
under Advanced. The existing pixel-limit validation and sparse document creation
path are unchanged. Formatting, strict Clippy, all 187 workspace tests, the
workspace build, GPU pixel smoke, and Windows GNU cross-check/Clippy pass locally.
The Linux/Xvfb interaction workflow is still pending for this edit.

The recovery manager now has an explicit entry point on Home and under File. It
refreshes the safe inactive checkpoint list and shows a readable generated name,
project dimensions/DPI where the bounded metadata header is valid, and file size.
It now also shows a relative checkpoint modification label when filesystem time is
available.
Recover continues through the existing unsaved-artwork preservation gate and
opens recovery into a new unsaved project. Remove opens the existing confirmation
dialog and continues to use the recovery module's directory/ownership checks. A
visual image preview is not available yet; no unvalidated snapshot is decoded just
for display. The code passes local build/test/GPU and Windows GNU compile/lint, but
these latest controls have not yet been exercised by the Xvfb window workflow.

Home now uses the requested slogan “A quiet place to make something.” as a large,
strong theme-colored heading. Recent cards show real artwork thumbnails after a
project has been opened or saved. The raster engine directly composites sampled
sparse-tile pixels into a maximum 256 × 160 preview instead of flattening the
full-resolution canvas; aspect ratio and alpha are preserved, and previewing is
refused while a stroke is provisional. The separate `home-thumbnails` cache
is capped at eight PNGs, with 256 KiB input limits and invalidation by canonical
project path, size and modification time. It is never read by document or export
code. Projects without a cache show a message until opened or saved. Tests cover
bounded/deterministic sampling, transparency, cache decoding, invalidation and
pruning. Same-size rewrites that preserve file timestamps may delay cache
invalidation until the next app open/save.

Latest local validation after the thumbnail work: formatting, strict Clippy,
**189 tests**, workspace build, llvmpipe GPU smoke, and Windows GNU cross-check and
Clippy all pass. No local Xvfb is available for screenshot inspection or interaction
testing. CI for the latest commits is still pending.

The reference window toolbar has also been reduced to zoom-out, zoom-in, a zoom
percentage, and a named Reference controls menu. Fit/reset, center, window-position
reset, lock, opacity, and remove are keyboard-operable menu items, leaving more of
the window for its image. The image remains the main window content. Local compile
and Clippy pass; full regression and rendered-window validation are pending.

Brush in the primary palette now opens a floating Brush library and closes the
right inspector. The flyout selects the actual ten existing brush presets, keeps
the current color when changing preset, exposes working size/opacity/color controls,
and has a direct Advanced settings action that opens the existing inspector. Each
preset card has a small color-stroke preview; advanced dynamics and preset
save/load remain in the existing controls. Color controls now open a separate
floating panel from the palette, with the actual brush color editor and readable
HEX-labeled recent-color swatches. Favorites, search, and persisted flyout
placement are not implemented yet. The flyouts are movable/closable and
reopened from the palette. The Accessibility menu now also persists a Reduce
interface motion preference and sets egui's transition time to zero; legacy
preferences default it off. Formatting, strict Clippy, all 190 tests, workspace
build, llvmpipe GPU smoke, and Windows GNU cross-check/Clippy pass locally. The
latest platform CI remains pending.

Home's main recent-work/recovery sections now stack below 900 points wide, and
recent artwork cards switch to a single column below 760 points. The two-column
layout remains on laptop-sized windows. This is a deterministic width breakpoint;
small-window visual inspection is still needed when Xvfb screenshot support is
available.

The Linux/Xvfb alpha workflow now captures the actual Home, New Artwork, canvas
workspace, and Brush library windows from the running application and uploads them
as a short-retention CI artifact for visual inspection. The capture code uses the
X11 window image and visual masks, not generated mockups. This capture change has
only been syntax-checked locally because Xvfb is unavailable here; the newest CI
run must pass and its screenshots must be inspected before visual review is
considered complete.

The owner reports that basic S Pen drawing has been used in previous hands-on
Windows sessions and provided recordings. Pressure curves, independent tilt axes,
palm rejection, pen/touch concurrency, DPI behavior and latency are still not
explicitly demonstrated or measured here.

### Continued Phase 8.3 workspace implementation

- The floating Layers panel now opens from the primary tool palette or with
  `Ctrl+L`; it is movable/resizable, can be hidden from the Window menu or closed
  directly, and exposes real layer selection, visibility, lock, create, duplicate
  and delete actions. Its position and size, plus Brush and Color flyout placement,
  are sanitized and persisted as workspace preferences. Escape dismisses the active
  Brush, Color or Layers panel before reaching the application's close workflow.
  A separate AccessKit-tree
  test now renders the same Layers-panel contents used by the application and
  asserts the semantic labels for those controls.
- `Ctrl+Shift+M` opens/closes the artwork recovery manager. Entries offer an
  on-demand image preview built only after the native storage loader validates
  file limits, structure and checksum. Sampling is capped at 180 × 120, only one
  selected texture is retained, malformed checkpoints show “Preview unavailable,”
  and a round-trip/corruption test covers the path.
- The Linux Xvfb workflow now captures first-run and recent-project Home states,
  New Artwork, canvas, reference-board, Brush-library, Layers-panel and
  Recovery-manager windows.
  Capture uses the `xwd` utility and a
  bounded XWD-to-PNG decoder; the previous ctypes `XGetImage` path caused a
  runner-only segmentation fault. The workflow also traces input actions and
  uploads the failure log, so subsequent window failures can be diagnosed from
  the GitHub API even if artifact-host downloads are unavailable.
- The first screenshot-enabled Linux run confirmed four rendered windows and
  exposed that Escape closed the entire application while the Brush library was
  open. Commit `09847df` moved Escape dismissal ahead of egui's text-input
  shortcut guard, but the follow-up still exited after the Escape/`Ctrl+L`
  sequence. Run [`40225820070`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40225820070)
  exposed that the Xvfb click did not activate Brush; the screenshot labeled
  `brush-library` was still the canvas. Escape then correctly followed the close
  path (`UI_TEST_ESCAPE_DISMISSED: none`). The smoke now reads the real Brush
  button rectangle and egui scale from a test-only trace, clicks its center, waits
  for a “Brush flyout opened” signal, and requires Escape to log that it dismissed
  Brush before moving to Layers. `Ctrl+B` now also opens the flyout for keyboard
  access; the workflow tests both activation paths and preserves `Ctrl+Shift+B`
  and `Ctrl+Alt+B` for brush-preset save/load.
- Actions run [`40226791860`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40226791860)
  passed the complete Linux alpha artwork workflow with the measured palette
  target, mouse-open and keyboard-open Brush paths, Escape dismissal, Layers,
  Recovery, save/reopen and recent Home. It captured the full screenshot set.
  Linux and macOS matrix jobs passed and produced their alpha/screenshot artifacts.
  Windows formatting, Clippy, tests, workspace build, and stylus-diagnostic
  package passed; the release canvas build, dependency inspection, and package
  upload also passed. The run completed successfully on all three platforms and
  uploaded `rust-paint-lab-canvas-demo` (5.8 MiB) and the preserved
  `rust-paint-lab-windows-diagnostic` (163 KiB). The canvas package includes the
  executable, Windows README, user guide, third-party notices, build record and
  SHA-256 manifest.
- Follow-up run [`40224735679`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40224735679)
  captured the same four screens but reported `BadWindow` while capturing Layers
  after the Escape/`Ctrl+L` sequence. Commit `09847df` moves Escape dismissal
  before egui's text-input shortcut guard, and run [`40225239895`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40225239895)
  confirms that Escape kept the app discoverable momentarily, but the app exited
  before Layers could be captured. Run `40225820070` reproduced that failure and
  its failure diagnostics identified the missed click. Its Linux job failed.
- Current local validation after the Layers panel, recovery preview and Escape
  handling: pinned Rust 1.90 formatting, strict workspace Clippy, workspace build,
  **193 workspace tests**, and the required llvmpipe GPU smoke passed. The new
  AccessKit test
  verifies labels from the real panel widget function, rather than a parallel
  mock tree. Linux CI now passes the entire Xvfb workflow. Screenshots were
  captured, but their image bytes cannot currently be downloaded here for visual
  inspection: the Actions artifact API lists them, while the blob download returns
  HTTP Forbidden. This environment has no Xvfb/display server.
- Actions run [`40224735679`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40224735679)
  passed Windows and macOS checks and produced the Windows packages. Linux passed
  formatting, Clippy, tests, build, virtual-window startup, live mouse and the
  Home/New Artwork/canvas/reference screenshots before the missed Brush click.
  An
  earlier run [`40223326274`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40223326274)
  passed Windows and macOS lint/build/package paths, including separate Windows
  diagnostic and canvas builds. Linux passed formatting, Clippy, all tests,
  build, virtual-window startup, live-mouse drawing, and captured four windows;
  the end-to-end artwork smoke then failed because Escape closed the window. The
  Windows and macOS passed at that checkpoint; the Linux Escape/palette issue is
  now resolved by the measured-control workflow on `415ab57`. Windows and Linux
  portable artifact bytes could not be downloaded in this environment because
  GitHub's Actions artifact blob hosts are blocked; API metadata remains
  accessible.
- Narrator/NVDA and Windows UI Automation have not been exercised. AccessKit tree
  generation and semantic labels are tested, but full keyboard focus traversal,
  modal semantics and native screen-reader announcements remain open. Basic S Pen
  drawing has been observed on the Galaxy Book in earlier testing; this workspace
  work does not claim that advanced pressure/tilt/palm/DPI behavior was retested.

### Exact continuation from this checkpoint

1. Visually inspect the Linux screenshot artifact for first-run/recent Home, New
   Artwork, canvas, reference board, Brush, Layers and Recovery when the Actions
   artifact blob host is reachable. The workflow successfully captured this set.
2. Download and launch `rust-paint-lab-canvas-demo` from run `40226791860`; verify
   the package and its README on Windows 10. The separate stylus diagnostic is
   retained in the same run. Do not treat compilation or CI as device validation.
3. Complete a keyboard/focus pass across Home, New Artwork, recovery, Brush/Color/
   Layers flyouts, reference controls and appearance preferences. Report native
   Narrator/NVDA limitations until a Windows assistive-technology run is possible.
4. Continue the visual refinement of Home and reference boards, persist panel
   placement where the interaction model is stable, and continue the focus/screen
   reader review. Keep artwork and export state isolated from workspace-only images
   and controls.

## Phase 8.3 visual integration pass — in progress

The accepted `1220722` checkpoint is a base, not Phase 8.3 completion. The owner
reported that the screenshots from Actions run `40226791860` were inspected and
showed an underdeveloped visual hierarchy. The archive's blob host still returns
HTTP 403 from this workspace, so the current pass uses those observations plus a
broader automated capture matrix; no local claim of pixel-by-pixel review is made.

### Implemented in this slice

- Brush, Color and Layers floating panels now share a viewport/palette-aware
  placement helper. Brush is resizable, max-bounded to the active window and
  vertically scrollable; palette overlap, viewport clamp, narrow-window, and
  preserved-user-position cases have unit coverage.
- New Artwork orientation choices are keyboard-focusable, semantically named
  cards with paper silhouettes sized to portrait, landscape and square aspect
  ratios. The dialog width and content scroll area adapt to the available window;
  the live background/transparent preview and exact dimensions remain present.
- Home uses responsive gutters and headline sizing, 4.5:1-safe theme-aware accent
  styling, taller artwork previews and clearer project metadata. Recent cards now
  expose a named button in the accessibility tree.
- The fixed inspector is optional and hidden by default for new/legacy settings
  without an explicit visibility value; explicit saved visibility is retained.
  `Ctrl+Shift+N` adds a real layer, and Layers remains available from the main
  palette or `Ctrl+L`.
- Reference zoom controls are touch-sized and have explicit accessible names.
- Custom format and Brush preset cards now expose radio-button selection state,
  a contrast-safe focus ring, and activate on Enter or Space. The synthetic
  focus test covers both keys.
- Xvfb automation now captures light/dark and high-contrast Home, New Artwork,
  Brush, Layers, Reference and Recovery screens at normal/narrow widths. It seeds
  a valid native recovery checkpoint and creates three editable layers, and
  rejects blank captures by sampled color diversity. CI execution is pending.

### BitchPixel visual and workflow integration — active

- The running application title, Home hero, primary action copy, and package user
  guides now use the adopted BitchPixel identity and the exact approved messages.
  The Home hero has a reusable pixel-grid brand mark, responsive artwork-oriented
  composition, direct New Artwork/Open actions, and a concise offline/privacy
  statement. Rust package names, file formats, and diagnostic identifiers remain
  unchanged for compatibility.
- The canvas workspace now has a contextual brush strip with direct size and
  opacity controls, current-color access and a Brush Library entry point. Color
  is available from the primary palette with shortcut C. The Brush Library adds
  search, category filtering, bounded persistent favorites, richer active-state
  cards, and grouped dynamics controls. Favorite ordering is stored in workspace
  preferences with deduplication and a hard limit.
- Brush cards have larger sample previews with variable line profiles and an
  active marker. The floating Color panel keeps current/previous colors, HEX/RGB
  precision, an eyedropper path, and recent swatches together. The floating
  Layers panel now renders bounded 10 × 7 previews sampled directly from each
  layer's existing sparse tiles, and exposes rename, opacity, blend mode, lock,
  visibility, add, duplicate, delete, raise, and lower controls in the panel.
  Raise/Lower remain keyboard/touch-accessible buttons, so ordering does not rely
  on drag and drop. The preview never mutates document state or enters exports.
- Test-only UI automation now captures Color in light and dark themes alongside
  the existing state matrix. AccessKit tree coverage verifies the Layers panel's
  ordering actions; bounded thumbnail tests confirm sparse layer pixels appear
  and a large empty layer remains transparent without allocating a full-size
  preview.
- Local validation after this integration: `cargo test --locked -p canvas-demo`
  passed 58 unit tests, all integration suites, and the llvmpipe GPU smoke;
  strict workspace Clippy and the floating-Layers AccessKit test passed.

### Validation and next steps

- Pinned formatting and strict workspace Clippy passed; all **200 workspace
  tests** passed at the preceding checkpoint, including populated-layer and Brush/format-card AccessKit-tree
  checks. The workspace build and llvmpipe software-GPU pixel smoke passed.
  Windows GNU workspace check and Clippy also passed after adding the target.
- Python smoke syntax passes. No Xvfb is installed locally, so the live screenshot
  run and actual image review remain CI dependent. The Actions archive blob host
  currently returns HTTP 403 from this environment.
- Narrator/NVDA and Windows UI Automation have not been exercised. This workspace
  change preserves the native WM_POINTER observer and does not change the drawing
  pipeline; it makes no new hardware-input verification claim.

- Actions run [`40230324720`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40230324720)
  passed Linux formatting, strict Clippy, all Rust tests, build, virtual-window
  smoke and live mouse smoke. The expanded screenshot run captured Home (light,
  normal/narrow) and New Artwork (light, normal/narrow), then failed because its
  palette click used bounds logged while Home was still covering the workspace.
  The UI diagnostic now logs the Brush target only after the canvas workspace is
  active and the smoke uses the latest reported bounds. Windows was canceled
  during tests before executable packaging; the run produced no Windows package.
- Run [`40230678773`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40230678773)
  passed all Windows/Linux/macOS format, lint, workspace test and build steps;
  Linux also passed GPU and live-window smoke and captured 17 of 18 requested UI
  screenshots, including populated Light/Dark Brush, Layers and Recovery states.
  The failure occurred after Dark Recovery capture, before high-contrast Home;
  the exact cause had not yet been visible in the truncated output. Windows
  canvas packaging was canceled after Linux failed.
- Run [`40231310053`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40231310053)
  repeated the same Linux result: all three platforms passed formatting, strict
  Clippy, tests and workspace build, Linux passed GPU/live-window smoke, and the
  screenshot suite captured the same 17 states before failing while closing the
  dark-mode process, prior to the high-contrast screenshot. The step output was
  truncated before the Python exception. The Linux artifact upload succeeded, but
  the other jobs were canceled before the Windows canvas package. Smoke output now
  marks capture start and sampled-color count and limits repeated application
  logs. The check annotation still truncated before the final capture marker.
- Run [`40231796822`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40231796822)
  supplied the exact cause: the Dark New Artwork screenshot creates a fresh,
  unsaved document, so Escape correctly entered the application's close-save path
  rather than exiting. The smoke timed out waiting for clean close before it could
  capture the final High Contrast Home. The screenshot-color hypothesis was wrong.
  The harness now terminates that screenshot-only process after capturing its
  windows; the previous app integration flow still tests normal clean close. CI
  annotations cap text at 4 KiB, so the smoke also keeps concise diagnostics.
  Windows and macOS standard checks passed; Windows was canceled during canvas
  build, before package upload.
- Run [`40232806078`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40232806078)
  passed formatting, strict Clippy, workspace tests/build, Linux GPU and live
  mouse smoke, the full 18-state Light/Dark/high-contrast screenshot matrix,
  and the Linux app/artwork/recovery workflow. Windows and macOS jobs also passed.
  The workflow uploaded `rust-paint-lab-canvas-demo`, the preserved
  `rust-paint-lab-windows-diagnostic`, Linux/macOS app packages, the screenshot
  archive and diagnostics. Artifact metadata is accessible, but downloading the
  screenshot archive from this environment still returns HTTP 403, so its pixels
  could not be visually reviewed here. CI confirms capture dimensions, nonblank
  color diversity, state coverage, and populated layer/recovery fixture
  assertions; it does not substitute for visual inspection.

Next: this matrix is green, but manually inspect the screenshot archive if
artifact access is restored and correct any visible issues. The consolidated
Windows evaluation artifact is available from the run above. Keep Phase 8.3
active until visual review and a Windows assistive-technology pass are recorded.

### Latest integration validation — commit `1d5f6fb`

- Run [`40235869737`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40235869737)
  completed successfully on Windows, Linux and macOS. Each platform passed
  formatting, strict Clippy, workspace tests and build. Linux also passed the
  virtual-window replay, live mouse and application/artwork/recovery workflow.
  The screenshot workflow validated 20 required UI states, including Color in
  light and dark, populated layer/recovery panels, and narrow layouts.
- The run uploaded the consolidated Windows app artifact
  `rust-paint-lab-canvas-demo` (artifact `12615670691`) and separate preserved
  `rust-paint-lab-windows-diagnostic` (`12614936837`), plus Linux/macOS packages
  and `rust-paint-lab-studio-ui-smoke` (`12615180854`).
- I attempted to download both the screenshot archive and Actions logs; GitHub's
  signed blob URLs returned HTTP 403. Their visual contents therefore remain
  uninspected here. Capture/state/diversity assertions passed but do not imply
  visual approval. Native Narrator/NVDA and Windows UI Automation also remain
  unverified. Basic S Pen drawing was physically observed on earlier builds;
  advanced Windows pen behaviors are not newly validated by this run.
- Next: inspect the screenshots when their bytes are accessible, then continue
  the Phase 8.3 redesign and accessibility work. Commit `1d5f6fb` is a substantial
  functional/visual checkpoint, not Phase 8.3 completion.

### Approved concept implementation — commit `59cace0`, CI pending

The owner supplied the approved BitchPixel visual reference and implementation
brief in the continuation message. It explicitly excludes speculative social,
cloud, premium and sharing UI, and confirms that the final logo is not ready. This
slice follows the concept's purple-led type/surface direction without copying its
illustrations or adding nonfunctional destinations.

- The app now embeds Manrope Regular for interface text and Syne Bold for display
  headings/wordmark. These static fonts are derived from the owner-approved Google
  Fonts source snapshot, distributed under SIL OFL 1.1, and included in the
  Windows/Linux/macOS package notices. No Cargo dependency or runtime download was
  added. Source snapshot, static hashes and exact license files are recorded in
  `assets/fonts/README.md`.
- Midnight Violet now uses deep purple-charcoal canvas surrounds, lavender-tinted
  surfaces, and a vivid purple primary accent. Coordinated light and high-contrast
  paths remain active; contrast tests now assert the purple/charcoal direction.
- The primary palette can dock left/right/top/bottom or float. Top/bottom docks
  use a horizontal wrapped layout, palette position is reanchored after viewport
  changes, and dragging snaps to the nearest edge within a bounded threshold.
  Left remains the default; right/left handedness preferences keep their existing
  behavior. Dock geometry and preference round-trip tests cover every position.
- The floating Color panel now has hue selection, a two-dimensional
  saturation/brightness field, keyboard-accessible component sliders, exact RGB
  and HEX entry, previous/recent colors, and the existing eyedropper. HSV
  conversion and pointer-coordinate mapping have finite/range tests, and the
  AccessKit tree test asserts semantic names for the custom field and sliders.
- Linux screenshot automation now includes a dark workspace with the persisted
  top-docked palette, in addition to the previous major screen and theme states.
  It now requires 21 captures. The screenshot archive remains inaccessible from
  this environment for direct image inspection (signed Actions blob URLs have
  returned HTTP 403), so capture will not be presented as visual approval.
- Local `cargo fmt --all -- --check`, strict workspace Clippy, complete workspace
  tests, and workspace build passed. `canvas-demo` has 66 unit tests and all
  integration/GPU suites passed, including llvmpipe. Python screenshot-harness
  syntax and `git diff --check` passed. Cross-platform CI and the new screenshot
  run are pending until this checkpoint is pushed. No physical Windows test is
  claimed here.

Commits `59cace0` and `232672e` are pushed. Actions run
[`40239511344`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40239511344)
passed formatting, strict Clippy, workspace tests/build and packaging on Windows,
Linux and macOS. Linux also completed the application/artwork smoke and uploaded
the top-dock/Color screenshot archive. The screenshot blob remains inaccessible
for direct visual review here. Continue applying the design system to Home/New
Artwork and other creative workflows. Do not issue another owner-facing
evaluation build until the broader redesign and functional workflow improvements
have accumulated and passed cross-platform checks. Narrator/NVDA and native
Windows assistive-technology validation remain outstanding.

### Home workflow continuation — local checkpoint

- Home's recent-artwork section now has a keyboard-editable filename filter. It
  searches only this device's recent project list, ignoring query case and
  surrounding whitespace. A no-match message replaces the cards without
  implying that the app searches arbitrary folders or online content.
- The gallery now chooses one to four columns from the available content width
  and sizes cards from that width, keeping image previews prominent without
  horizontal overflow at tested phone, laptop and desktop widths.
- On very wide displays, Home places the secondary recovery/continue section
  below the full-width gallery instead of reserving half the viewport for it,
  allowing the four-column layout to be used. Narrow displays remain stacked;
  ordinary desktop widths retain the side-by-side layout.
- Linux screenshot automation now includes Home at 1920 × 1080 to exercise and
  capture the four-column state. The screenshot job starts a 2200 × 1400 Xvfb
  display for that capture; normal/narrow states and other smoke workflows keep
  their existing dimensions and behavior.
- The search field has an explicit AccessKit text-input name and explanatory
  hover text. Unit coverage verifies matching semantics; an AccessKit tree test
  verifies the semantic text input is exposed. Sizing tests assert card bounds
  and fit for 420, 800, 1280 and 1920 point content widths.
- Local formatting, strict Clippy, the full workspace tests (including the
  llvmpipe GPU smoke), and workspace build passed after this change. The
  workspace contains 204 tests; no desktop window screenshot is available from
  this local environment, so the responsive gallery is functionally covered
  but not visually inspected here.
- The filename search commit `b81e099` is pushed. Run `40239511344` passed the
  preceding concept/dock/Color checkpoint on all three operating systems. Runs
  `40239908743` and `40240008769` validate intermediate Home commits; run
  `40240080605` validates the wide capture and is still in progress. Latest run
  `40240193155` covers the final `802c2c9` layout adjustment, and its CI must
  pass before this Home continuation is considered cross-platform verified.
- This is a small continuation slice, not Phase 8.3 completion. Next, continue
  integrating the approved Home/New Artwork visual direction with the working
  brush, color, layer, reference and recovery workflows; inspect the CI
  screenshot pixels once artifact downloads work; then address the remaining
  keyboard/focus and native Windows assistive-technology gaps. Do not claim
  Narrator/NVDA compatibility without testing those screen readers.

### Major visual/workflow continuation — local reviewed checkpoint (2026-10-25)

This continuation responds to the owner's explicit assessment that `0c1c145`
still looked sparse and developer-oriented. The visual foundation is in code
commit `fcbd6c6`, based on `0c1c145`; follow-on commit `769b9b2` adds working
quick-start choices directly to the first-run Home hero. Screenshot assertions
and synchronized documentation are committed alongside the visual pass; a new
cross-platform Actions run is pending.
Phase 8.3 remains active.

- Home now uses a temporary text-only BitchPixel wordmark, the exact approved
  lines “No limits. No bullshit.” and “Create without permission.”, a prominent
  New Artwork/Open action pair, illustrated quick-start canvas cards and a
  responsive recent gallery. It shows cached thumbnails from actual saved
  projects only; no demo art is presented as user work. On wide displays the
  format cards and recent-artwork previews grow to use the available width. An
  intentional first-run empty state remains when no projects exist.
- First-run Home now uses the open side of its hero for three working one-click
  canvas formats and a Custom/print-size action instead of leaving an empty
  image-sized area. When a real recent project exists, the hero shows that
  project's cached artwork thumbnail and the full format strip follows below.
  Narrow windows keep the format strip in the vertical scroll flow. The Xvfb
  workflow waits for the first-run quick-start state marker before capture.
- New Artwork now presents six named preset formats plus Custom as proportionally
  illustrated, keyboard-selectable cards with a visible selected state, live
  aspect/background preview, clear pixel/DPI labels and a primary Create canvas
  action. The grid reflows across four, three, two or one columns as the modal
  narrows; advanced units, dimensions, recent sizes, resolution and background
  settings remain in its scrollable content.
- Brush/Color/Layers contextual surfaces use shared purple-first frames and
  typography. The Brush flyout now accounts for the full window chrome when
  sizing its scroll region, avoids the docked palette, fits small windows with a
  compact single-column card layout and hides canvas zoom controls while a panel
  is open. The main tool palette remains left by default and its existing dock,
  float, snap and handedness preferences remain persistent.
- Home headline contrast is measured in the screenshot harness as 15.53:1 in
  Light, 16.61:1 in Dark and 21:1 in High Contrast for the selected surface and
  text. The test protects that headline specifically; it is not a full WCAG audit.
- The Xvfb workflow now captures 22 states. Its recent-artwork screen is resized
  to 1920 × 1080 and must contain at least 30 pixels matching the smoke fixture's
  blue committed stroke; the latest capture contained 3,213. Light and Dark
  Reference captures must contain the imported fixture's distinctive region; the
  latest counts were 419 and 575 pixels. Populated editable Layers and valid
  Recovery entries are included. The workflow also verifies the reference stays
  out of exported artwork and that the real app can paint, save, reopen, undo,
  redo, export and recover after process termination.
- Screenshots were inspected locally instead of relying only on CI's color-count
  check. This revealed that the recent-artwork capture had been taken below the
  viewport; capture size and a content-pixel assertion now require the actual
  thumbnail to be on screen. It also exposed the old wide quick-start cards and
  the stale dialog card-count assumption; both were corrected and recaptured.

### Local validation and remaining acceptance work

- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` passed.
- `cargo test --workspace --locked -- --quiet` passed **223 tests**. The offscreen
  GPU test was required and passed on Mesa llvmpipe/Vulkan; it is software-GPU
  coverage and does not validate Iris Xe or RTX hardware.
- `cargo build --workspace --locked` passed. The app-window workflow completed
  its painting/document/recovery assertions and all 22 screenshots under Xvfb.
  Python syntax validation and `git diff --check` passed after the harness update.
- Screenshot pixels were visually reviewed from the local Xvfb output directory.
  Actions artifact metadata and cross-platform runs for this pending checkpoint
  have not yet been verified. No new Windows evaluation artifact is being offered
  until the expanded implementation and CI pass.
- Narrator/NVDA and Windows UI Automation remain untested. Basic S Pen drawing
  has been observed on the owner's device in earlier builds; pressure/tilt
  ingestion in this workspace, palm handling, capture-loss behavior, touch/DPI
  scaling and Windows hardware performance were not retested in this continuation.
- Remaining: continue consolidating tool and floating-panel visual language,
  implement/verify panel docking behavior where it does not weaken move/resize,
  audit keyboard traversal and actual AccessKit actions, review Windows DPI and
  small-window behavior, and verify this branch in Windows/Linux/macOS Actions.
  The Phase 8.3 overhaul is **not complete**.

### Exact continuation from the local visual checkpoint

1. Push the reviewed follow-on Home commit and progress update on
   `feature/studio-overdrive-phase-8`; do not merge or release.
2. Wait for and record the new Windows, Linux and macOS Actions result. Inspect
   the Actions screenshot archive if it is downloadable, without treating
   capture-count assertions as visual approval.
3. Continue the workspace polish and keyboard/focus/contrast pass; keep the
   palette default-left and the reference board outside document/export state.
4. Only after the broader checkpoint and cross-platform CI succeed, create one
   consolidated Windows evaluation artifact. Record physical-device and
   Narrator/NVDA checks as outstanding until performed.

### Recovery presentation and wide Home review — local checkpoint (2026-10-26)

The previous visual pass is preserved and Phase 8.3 remains active. This
continuation follows screenshot review rather than treating the capture count as
approval.

- Recovery entries now replace generated timestamp/process-number filenames with
  readable checkpoint names. Each row identifies an available native snapshot,
  journal, or backup source and uses its actual file metadata; the Recover action
  is disabled when none of those sources is present. Journal-only dimensions are
  described as checked during recovery rather than guessed. Recovery names use a
  contrast-adjusted theme color. Unit tests cover generated-name cleanup and
  source selection.
- The Linux application workflow opens the actual Recovery Preview button and
  captures a non-empty thumbnail from a valid `.rpl` created by the same smoke
  workflow. It requires at least eight colors in the preview region. The test
  fixture is copied only into its temporary recovery directory and removed before
  the independent forced-termination journal-recovery scenario, so those two
  checks remain isolated.
- Recent-artwork cards use a shorter image well to leave more room for names and
  metadata. The wide Home capture scrolls through the real UI until a complete
  project card is within the viewport; its layout bounds and the real painted
  blue pixels are asserted. The Home overlay explicitly paints the full viewport
  so a resized 1920 × 1080 workspace no longer exposes the GPU canvas beneath a
  short Home page. The screenshot assertion found zero dark canvas pixels in its
  final 96 rows. Locally inspected screenshots showed the complete recent cards
  with their project names, dimensions, file sizes and opened dates.
- The expanded Xvfb workflow completed all 22 UI captures. It recorded 1,155
  painted-artwork pixels in the recent gallery, 77 distinct colors in the
  Recovery preview, 419/575 distinctive reference-image pixels in Light/Dark,
  and 0 dark canvas pixels beneath wide Home. Recovery name contrast measured
  15.53:1 in Light and 16.61:1 in Dark; the Home headline measured 15.53:1,
  16.61:1 and 21.00:1 in Light, Dark and High Contrast. These are selected
  screenshot-derived checks, not a complete WCAG audit.
- Visually reviewed the generated wide/first-run and populated Home screens,
  New Artwork, Recovery Light/Dark, populated Layers, Reference, Brush, Color and
  High Contrast states. New Artwork remains a scrollable precision dialog, and
  the overall interface is still short of the approved concept's visual finish.
  This review did not test native Narrator/NVDA or Windows UI Automation.

### Local validation and continuation

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked
  -- -D warnings`, and `cargo build --workspace --locked` passed.
- `cargo test --workspace --locked -- --quiet` passed **225 tests**. The GPU
  smoke passed both during the workspace run on llvmpipe/OpenGL and in a separate
  required-adapter run on llvmpipe/Vulkan (`RPL_REQUIRE_GPU=1`). Neither is a
  physical Iris Xe/RTX result.
- `scripts/alpha-workflow-smoke.py` passed through real X11 mouse/key events,
  save/reopen, undo/redo, PNG/OpenRaster export/import, Recovery Preview and
  forced-termination journal recovery. All 22 screenshots were inspected from
  `/tmp/rpl-studio-final-review`. Python syntax and `git diff --check` passed.
- Actions run `40246806374` completed successfully on the preceding pushed
  checkpoint. Cross-platform Actions for this continuation are pending its push;
  no consolidated Windows evaluation artifact is being promoted yet.
- Basic S Pen drawing remains previously observed on the owner's hardware.
  Advanced pen input, Windows DPI behavior, power-loss durability, Narrator/NVDA
  and Windows UI Automation were not exercised in this Linux continuation.

The Recovery/Home code was pushed in `3a0a299`. Its Windows and macOS Actions
jobs passed; the Linux failure was isolated to the screenshot harness's
last-card selection and is corrected in the follow-up below. Continue the
keyboard/focus and AccessKit action coverage plus coordinated floating-panel and
Reference workspace polish. Keep Phase 8.3 active and defer the consolidated
Windows evaluation build until the broader pass is justified by rendered UI
evidence.

### Toolbar and docked-palette follow-up — local validation

- Replaced the palette's Braille drag glyph, which rendered as tofu on the
  inspected Linux font stack, with six original vector dots. The non-button grip
  is exposed as “Move tool palette” in egui's semantic tree and retains the
  existing palette drag/dock gesture.
- The top dock previously anchored to a fixed 64-pixel offset from the window
  origin, placing it over the contextual brush size/opacity row. Dock placement
  and edge snapping now use a viewport whose top begins below the measured
  contextual-options panel. Top and side docks therefore avoid application
  chrome without relying on a guessed fixed offset. Unit tests cover anchoring
  and snapping relative to the content viewport below app chrome. The Linux
  smoke now logs and asserts that the rendered palette begins below the measured
  contextual-options row.
- Actions run `40249283516` confirmed that two Home cards were fully visible and
  the last card in the responsive grid was clipped at the bottom. The smoke had
  incorrectly insisted on the last card being complete. Its follow-up selects
  the largest fully visible card and scrolls only if none is visible, while
  retaining the actual-thumbnail pixel assertion.
- Local validation after these changes: formatting, Python syntax, diff checks,
  strict workspace Clippy, workspace build, all **227 workspace tests** including
  doctests, and a required llvmpipe/OpenGL GPU smoke passed. Xvfb is unavailable
  in this shell, so the updated Xvfb flow and measured rendered-clearance
  assertion await Actions. The six-dot handle was visually reviewed in previous
  Light/Dark screenshots; the new top-dock position awaits a fresh screenshot
  review. Linux rendering does not validate Windows DPI or native accessibility
  APIs.
- Actions run
  [`40249283516`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40249283516)
  completed with Windows and macOS success and Linux screenshot-smoke failure on
  `3a0a299`. The separate `rust-paint-lab-canvas-demo` and
  `rust-paint-lab-windows-diagnostic` artifacts remain available from that run.
  Its screenshot archive exists, but artifact download returned HTTP 403, so
  the new top-dock position could not be visually inspected here. Verify the
  follow-up commit on all platforms and confirm the separate Windows artifacts.
- Next: inspect the top-docked palette with the measured canvas-edge anchor,
  then continue keyboard traversal/AccessKit actions and floating-panel
  placement. Phase 8.3 remains active; no consolidated Windows evaluation build
  is being promoted from this local checkpoint.

### BitchPixel visual/functional continuation — engine-rendered brush gallery

The owner identified the previous interface work as useful progress but still
short of the requested overhaul. The implementation is continuing beyond the
palette correction. The pushed base `c81f3d8` is now confirmed by Actions run
[`40250590747`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40250590747):
Windows, Linux, and macOS checks passed, including Linux app interaction and
separate Windows canvas/diagnostic packaging.

- The Brush Library now includes the selected brush's live sample rendered by
  the real `paint-document` raster engine. It uses a deterministic synthetic
  pressure path, current color/size/opacity/tilt settings, and a temporary
  transparent document that is never merged into or saved with artwork.
- The ten built-in preset cards now use actual raster-engine samples rather
  than only hand-drawn UI glyphs. The eraser card demonstrates a cut through a
  colored under-stroke. Preset samples use a stable brand-violet color so
  gallery content does not need to be regenerated after every user color change;
  the selected-brush preview still tracks the active settings. Texture cache
  capacity is fixed at ten cards, and each preview is bounded to 320 × 96 RGBA.
- Accessibility metadata identifies the active sample and its current size and
  opacity. The interaction smoke asserts all ten preset textures are created,
  confirms the active preview bounds, and continues through painting, save/open,
  selection, PNG/OpenRaster, reference isolation, recovery, and clean-close
  workflows.
- Local validation: formatting, strict Clippy, workspace build, all **232 Rust
  tests**, and the llvmpipe/OpenGL GPU smoke pass. The full X11 mouse/key alpha
  workflow passed and logged ten generated preset previews. This local run did
  not emit screenshots. Actions run `40250590747` applies to the base commit,
  not this uncommitted brush-gallery change. Its screenshot archive and other
  binaries remain subject to the artifact host's HTTP 403 restriction in this
  environment; current brush-gallery screenshot pixels have not been reviewed.
- Phase 8.3 remains active. Existing keyboard actions and AccessKit tree tests
  are retained, but native Narrator/NVDA and Windows UI Automation are still
  untested. Basic S Pen drawing was observed in earlier owner testing; this
  change does not claim advanced pen, Windows DPI, or hardware GPU validation.

### Exact continuation

1. Commit and push the verified engine-rendered brush gallery with the progress
   and screenshot-test updates; check its new Windows/Linux/macOS Actions run.
2. Continue with a persistent artist swatch palette in Color, including bounded
   workspace-only persistence and real apply/add/remove behavior, then improve
   the workspace's cohesive floating-panel and layer/color presentation.
3. Make the Linux screenshot run prove populated brush and color states. Inspect
   screenshots when an allowed image path is available; otherwise explicitly
   keep visual acceptance pending rather than inferring it from dimensions or
   color counts.
4. Keep Home/New Artwork and workspace changes aimed at the supplied concept,
   and do not promote the consolidated Windows evaluation build until the
   broader visual and functional pass and CI justify it.

### Floating workspace and saved color — local checkpoint `5200f21`

- The current branch started from remote HEAD `c0e5ace`; its Actions run
  [`40251981336`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40251981336)
  passed Windows, Linux, and macOS before this continuation. Commit `5200f21`
  adds a workspace-only saved-color palette (24-color bound, duplicate
  rejection, apply, and removal), and persists its state separately from artwork.
- A real X11 interaction exposed a Canvas/egui ownership bug: clicks over
  floating controls that overlapped the artboard were treated as paint input.
  Mouse, selection edits, native pen UI capture, and touch-navigation routing now
  check active panel/reference `AreaState` bounds in egui points after converting
  the physical client position with the current display scale. A pure test covers
  scaled, invalid, nonfinite and out-of-range input. The live X11 test clicks a
  Color swatch and a Workspace tab over the canvas and confirms the interaction
  changes UI state without disrupting artwork workflows.
- The old optional fixed right inspector is now a movable, resizable floating
  Workspace controls window. F6 opens/closes it; Escape dismisses it without
  closing the application. Its open state, position and bounded dimensions live
  in workspace preferences with defaults for existing preference files. The X11
  test reads the saved preferences after closing it.
- Home tests now separately verify “No limits. No bullshit.” and “Create without
  permission.” at 4.5:1 or better in both light and dark appearances. The
  screenshot matrix now names 23 states, including the floating workspace panel.
- Local validation on `5200f21`: `cargo fmt --all -- --check`, strict workspace
  Clippy, **232 workspace tests**, full workspace build, Windows GNU target
  workspace check and Clippy, llvmpipe/OpenGL GPU smoke, Python syntax, and the
  full real-X11 alpha workflow all passed. That workflow included mouse painting,
  undo/redo, PNG/OpenRaster import/export, journal-durable forced-process-kill
  recovery, reference isolation, saved-color actions and Workspace tab actions.
- Visual approval is still pending. The Linux screenshot workflow exercised all
  23 names, but this shell's ImageMagick readback captured black pixels; the
  supported `gh run download` for the preceding screenshot artifact was rejected
  with HTTP 403 by the artifact blob host. The screenshot set is not treated as
  reviewed evidence. Windows S Pen drawing was observed by the owner in earlier
  builds; this new overlaid-panel input routing and Windows UI Automation,
  Narrator/NVDA have not been physically tested here. The c0e5ace Actions result
  is not CI validation for `5200f21`.

### Exact continuation from `5200f21`

1. Push `5200f21` to the existing Phase 8 branch and verify its Windows, Linux,
   and macOS jobs; retain the previous diagnostic/canvas artifact split and do
   not promote a consolidated Windows evaluation build yet.
2. Continue the approved purple-first design/workflow work across Home, New
   Artwork, Brush, Color, Layers, Reference, and Recovery. Prefer functional
   improvements such as improved layer organization and direct brush/color
   workflows, and keep the optional workspace panel movable rather than restoring
   a fixed inspector column.
3. Inspect the next CI screenshot archive through an allowed image path. If the
   artifact host still returns 403 and local captures remain black, explicitly
   keep visual acceptance pending; do not infer it from dimensions, marker counts
   or successful UI tests.
4. Continue to keep Phase 8.3 active. Do not merge, release, or publish the
   consolidated Windows evaluation build until the broader visual/accessibility
   pass and three-platform validation justify it.

### Layer row actions and undoable targeted editing — local checkpoint

The current continuation adds a visible, keyboard-focusable **Actions** menu to
each layer row while retaining secondary-click context menus on the layer name.
Duplicate, delete, bring-forward and send-backward commands target the layer ID
whose menu was opened; they enter the existing document edit/history path, so
Undo restores layer count, pixels and ordering. Rename focuses the matching
editable name field. The AccessKit tree test now checks that populated rows expose
the named Actions control.

The X11/XTest artwork workflow exercises the actual Actions button and verifies
duplicate/undo, send-backward/undo, delete/undo, and right-click context opening
against the changing frontmost layer ID before continuing through painting,
save/export/reopen and process-kill recovery. Local validation on this working
tree passed formatting, strict Clippy, **236 workspace tests**, the software GPU
smoke, workspace build, Windows GNU target check and Clippy, Python syntax, and
the full X11 alpha artwork workflow. The earlier pushed Actions run
[`40254869930`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40254869930)
passed Windows and macOS but failed the Linux alpha artwork workflow; GitHub's
log receiver returned HTTP 403 when queried, so its precise cause is unavailable.
That run predates this layer workflow and its expanded test. The new code still
needs a fresh Actions run after commit.

Visual review remains blocked locally: `xwd` is not installed in this shell, and
an FFmpeg/XCB readback experiment failed on X11 shared-memory image access. The
experiment was removed rather than weakening screenshot assertions. The
supported Actions artifact download has also returned HTTP 403. Thus the X11
workflow proves interaction and pixel-region assertions on CI but does not let
this environment claim human visual inspection. Narrator/NVDA, Windows UI
Automation and advanced physical S Pen behavior remain unverified here. Phase
8.3 remains active; do not promote a Windows evaluation package.

### Exact continuation

1. Commit and push the layer-row Actions workflow and tests; confirm current
   Windows/Linux/macOS Actions, especially the expanded Linux alpha smoke.
2. Continue functional workspace work across brush, color, layer and reference
   workflows, then revisit the Home/New Artwork composition and overall visual
   hierarchy against the approved concept.
3. Review populated screenshots through an authorized image path if one becomes
   available. Until then, report visual acceptance as pending even if screenshot
   generation and automated pixel assertions pass.
4. Keep Phase 8.3 active; no merge, release, or consolidated evaluation build.

### Direct layer drag-reordering — local functional checkpoint

Building on `11fc732`, layer rows now have a dedicated 36 × 44 pointer grip and
each row is a drop target. A drop reorders the stable layer ID through a normal
document history edit; it does not mutate render-only order. The row's accessible
Actions menu retains Bring forward/Send backward as the keyboard path. The grip
is described as pointer-driven and directs keyboard users to Actions, rather
than exposing an inert button.

The X11/XTest artwork workflow now presses the grip, drags the top layer onto
the backmost row, asserts the full visible layer-ID order changes, and verifies
Ctrl+Z restores the exact original order. It continues through the existing
duplicate/delete undo, paint, save/export/reopen, and process-kill recovery
checks. The AccessKit test checks the grip's instruction and accessible menu.
Formatting, strict Clippy, **236 workspace tests**, software GPU smoke, workspace
build, Windows GNU target check/Clippy, Python syntax and the full X11 artwork
workflow passed locally.

Actions run [`40256352269`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40256352269)
for `11fc732` has passed macOS but failed the Linux alpha artwork step; Windows
was still running at the time of this checkpoint. Its failed log receiver also
returns HTTP 403, so I cannot attribute the cause. The run predates direct drag
reordering, and a new three-platform run is required for the current work. Local
screenshot inspection remains unavailable (`xwd` is absent and this X server's
FFmpeg image readback cannot access its shared-memory window); keep visual
acceptance pending and do not promote a Windows evaluation artifact.

### Visual review and responsive Brush pass — active local checkpoint

The earlier note that screenshots could not be inspected is superseded for this
local checkpoint. The Linux capture path now copies the composed egui/wgpu frame
to a bounded offscreen readback target only when both UI-test environment
variables are set. On 2026-10-26, the full real-X11/XTest alpha workflow passed
with 29 required UI states (30 PNGs captured): Home at narrow/normal/wide
sizes, a populated recent-artwork grid, New Artwork, dark and light canvas,
Brush, Color, populated Layers and Recovery, Reference, and high contrast. The
reference, raster-engine brush sample, populated layer rows and recovery preview
were checked in their rendered image regions. I visually inspected representative
light/dark/narrow captures through the image viewer rather than inferring layout
from dimensions alone.

That review found a real narrow Brush usability defect: at 640 × 480 its large
active sample pushed all preset cards below the initial viewport. The flyout now
uses a 416-point preferred width in narrow windows, removes the redundant inner
heading and category labels there, shows the search/favorites and preset gallery
first, and leaves the settings-aware active sample below the scrollable gallery.
The X11 workflow asserts the flyout remains in the viewport, does not cover the
tool palette, and presents at least two complete preset cards without scrolling.
Desktop sizing and saved panel state remain unchanged. The final inspected narrow
capture shows three full artist-named preview cards; the Brush window and its
controls remain independently scrollable.

The same visual pass identified the recurring dark strong-text defect. egui's
strong labels use the active-widget foreground, which had been black to contrast
with the bright violet button fill. Active fills now darken toward the panel
surface until they meet 4.5:1 against the active foreground, while the selected
state maintains at least 3:1 edge contrast. High-contrast mode uses surface
fills. Unit and rendered-window assertions cover these rules. The current
capture reports 19.45:1 light strong-text, 21:1 dark strong-text, 4.52:1 light
official slogan, and 6.35:1 dark official slogan; the official lines remain
“No limits. No bullshit.” and “Create without permission.”

Local verification for the current working tree: `cargo fmt --all -- --check`,
strict workspace Clippy, `cargo test --workspace --locked` (**239 passed, 0
failed**), workspace build, Windows GNU target check and strict Clippy, llvmpipe
offscreen GPU smoke, Python syntax check, and the complete real-X11/XTest alpha
workflow all pass. The Linux software adapter was llvmpipe; this does not measure
integrated-GPU performance. The earlier Actions run [`40256352269`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40256352269)
passed Windows and macOS but failed the Linux alpha-artwork step on commit
`11fc732`; GitHub's results/artifact host returns HTTP 403 for its failure log.
A fresh Actions run for these uncommitted changes is required before reporting
three-platform CI success. Narrator/NVDA and advanced native S Pen behavior have
not been retested here. The owner previously confirmed basic S Pen drawing and
pressure/tilt detection. Phase 8.3 is still active; no evaluation package,
release, or merge is authorized by this checkpoint.

### Exact continuation

1. Finish full regression checks after the final documentation edits, commit and
   push this visual/accessibility slice on `feature/studio-overdrive-phase-8`,
   and inspect the fresh Windows/Linux/macOS Actions result. Record actual CI
   outcomes and artifact links without claiming access to the blocked 403 log
   endpoint.
2. Continue substantial design and workflow work: improve the artwork-centered
   canvas composition and reference-board placement on narrow windows, continue
   the Home/New Artwork hierarchy against the approved concept, and complete the
   keyboard/focus/AccessKit review. Preserve current layer history, save/recovery,
   reference isolation, export, and mouse/S Pen routing.
3. Keep Phase 8.3 active; do not publish the consolidated Windows evaluation
   build until the larger visual/functional overhaul and fresh cross-platform CI
   justify it. Native screen-reader validation and advanced Windows pen behaviors
   still require appropriate Windows testing.

### Artist color picker hue wheel — functional and visually inspected checkpoint

The Color flyout now pairs the existing saturation/brightness field and precise
keyboard-operable Hue slider with a visible 96-segment hue ring. The ring shows a
position marker and a center sample at the current saturation/brightness. Pointer
input maps top to red and proceeds clockwise; the center sample and positions
outside the annulus are ignored. The slider remains available for precise numeric
and keyboard adjustment. The wheel is represented in the AccessKit tree as a
named slider control, and the adjacent saturation/value field keeps its semantic
name.

The real X11/XTest artwork workflow clicks the wheel through winit/egui, asserts
that changing hue preserves the active saved color's saturation and brightness,
then selects the saved original swatch to restore the starting blue. The rendered
light-theme Color screenshot checks the wheel region contains more than 32 distinct
chromatic colors; the latest run measured 2,204. All 30 screenshots in the
expanded Linux matrix were captured and reviewed locally, including light/dark
Color, Brush, populated Layers and Recovery, New Artwork and Home at narrow and
normal sizes. The full artwork workflow also passed live raster drawing, layer
transactions/undo, reference isolation, file round trips, PNG/OpenRaster exports,
and journal-only process-kill recovery.

The same review tightened reference-board screenshot evidence: each reference
state now captures the visible, viewport-clipped image rectangle after resize,
counts colors only inside that image, and requires the fixture's distinctive
green marker as well as at least 12 scene colors. The current real image region
passed in all four light/dark, normal/narrow captures (light normal/narrow: 483
marker pixels and 176 colors; dark normal: 126 marker pixels and 295 colors). I
visually inspected those rendered captures, confirming the reference picture is
visible rather than merely inferring it from app state or full-window pixel
diversity. To make it more image-first, the reference window uses a smaller title
and compact zoom buttons plus a readable Settings menu instead of a wide text
control; the visible image occupies at least 85% of the window's width (91.1% in
light normal/narrow and 96.3% in dark normal/narrow).

The Settings menu no longer offers two identical reset actions: **Fit image to
window** resets zoom and pan, while **Center image** resets pan and retains zoom.
The X11 workflow opens the real menu from an initially panned 110% view, clicks
Center image, and verifies the zero pan and retained zoom are saved in workspace
preferences.

Local validation after this addition: formatting, strict workspace Clippy, all
**240 workspace tests**, workspace build, Windows GNU target check and strict
Clippy, llvmpipe offscreen GPU pixel smoke, and full X11/XTest artwork workflow
passed before the final screenshot-width assertion was tightened to reject an
image rectangle wider than its containing reference window. The newly added
upper-bound assertion has not been rerun locally because this execution
environment has neither `Xvfb` nor `xvfb-run`; the measured image/window ratios
were 91.1% and 96.3%, both within the tightened bound. The next Actions Linux
window job will exercise the exact final script. Basic S Pen drawing remains
owner-observed; advanced capture, pressure/tilt edge cases, Windows display
scaling, and native Narrator/NVDA behavior were not tested in this Linux session.
Actions run
[`40265944815`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40265944815)
passed for Windows, Linux and macOS on preceding commit `2b615f9`; the wheel
changes still require a fresh Actions result after push. Phase 8.3 remains active,
and no consolidated evaluation package, merge, or release was produced.

### Exact continuation from the hue-wheel checkpoint

1. Push the verified hue-picker change on `feature/studio-overdrive-phase-8` and
   check the fresh Windows/Linux/macOS Actions run; update this record with its
   actual result.
2. Continue the approved visual overhaul beyond Color: improve canvas/reference
   composition and reference placement at narrow sizes, keep refining the
   Home/New Artwork hierarchy, and continue keyboard/focus/AccessKit review.
   Preserve layer history, save/recovery, reference isolation, export, and pen
   routing.
3. Keep Phase 8.3 active and withhold the consolidated Windows evaluation build
   until the broader visual/functional work and current CI are complete.

### Adaptive reference-board import placement — local checkpoint

New reference images now receive a viewport-sized default window and an
initial position chosen from the available corners. Placement favors the side
opposite the current movable palette, avoids palette overlap, and scores existing
reference windows so multi-image imports cascade into open space when possible.
The normal reference movement, resize, pan, zoom, and workspace persistence paths
remain unchanged. A narrow-window policy keeps the default board within the
available client area before showing it.

The X11/XTest artwork workflow now has a test-only path that imports a second
image through the real bounded decoder, workspace update, texture upload, and
placement logic after testing Center image. It checks that the new board is
visible inside normal and 640 × 480 windows without covering the palette, then
captures both single- and multi-reference states in light mode. The dark-mode
capture also uses the multi-reference workspace. Stable screenshot markers now
identify the topmost reference window rather than mixing geometry from several
windows. These workflow additions are not yet run locally: this environment has
no Xvfb/XTest display. The placement helper's **two new unit tests passed**
alongside the existing reference tests, and Python syntax plus `git diff --check`
pass. Full local checks also pass: formatting, strict workspace Clippy, **242
workspace tests**, workspace build, Windows GNU target check/Clippy, and the
llvmpipe offscreen GPU smoke. The next feature-branch Actions run must validate
the exact expanded workflow before its outcome is recorded.

The current palette-overlap score strongly penalizes collisions with the primary
tool palette and lightly penalizes overlap with existing reference boards. At
maximum capacity or on very small viewports, some board overlap can still occur;
the artist can move and resize each board independently. This is an intentional
best-effort layout rule rather than a docking system.

### Home fixture provenance — UI-test checkpoint

Synthetic Home projects now render an accessible **DEVELOPMENT TEST FIXTURE**
pill in the isolated `RPL_TEST_PATH_DIALOG` harness only. The recent-artwork
screenshot locates the pill and requires a visible region of its purple backing,
alongside the real cached thumbnail and painted stroke checks. This makes fixture
art explicit in the review archive without changing real project cards or
persisted artwork. The expanded screenshot check is included in the next Actions
run; no Xvfb/XTest executable is available locally. The latest local regression
also passes formatting, strict workspace Clippy, **242 tests**, workspace build,
Windows GNU target check/Clippy, llvmpipe GPU smoke, Python syntax, and `git diff
--check`. The screenshot pixel assertion awaits the next Linux Actions run.

### Exact continuation from adaptive reference placement

1. Run formatting, strict Clippy, all workspace tests/builds, Windows target
   checks, and the expanded Linux X11/XTest workflow in Actions; inspect the
   resulting multi-reference screenshots before documenting visual results.
2. Continue the broader purple-first workspace redesign and functional work.
   Revisit Home/New Artwork hierarchy and canvas composition, then continue the
   keyboard/focus/accessibility pass while preserving document/recovery/export
   regressions.
3. Keep Phase 8.3 active. Do not merge, release, or publish the consolidated
   Windows evaluation build until the larger overhaul and cross-platform run
   justify it.

## Home and New Artwork integration — 2026-10-26

Commit `f79c02d` continues the approved visual overhaul on
`feature/studio-overdrive-phase-8`.

### Implemented

- Replaced the same generic paper sketch in every format card with seven distinct
  vector illustrations for Portrait, Landscape, Square, A4 Print, Comic Page,
  Full HD, and Custom. Paper shapes use the actual preset aspect ratio. Larger
  cards make the category image primary while preserving the selected outline,
  check indicator, dimensions, resolution, keyboard activation, and AccessKit
  radio semantics.
- Enlarged the live New Artwork preview and placed it on a consistent preview
  mat. The artboard keeps the selected document ratio and previews either the
  configured solid background color or a clipped transparency checkerboard;
  dimensions, DPI, and the current sRGB RGBA8 workflow are readable beside it.
- Fixed recent-artwork grid sizing: its content width now reserves the shared
  card's 24-point frame inset, so responsive columns stay inside the viewport.
  Column count also reflects the number of visible projects and actions. A
  partially filled, unfiltered row receives a real **Open another project** tile
  that invokes the normal file-picker request; filtered results do not gain an
  unrelated tile. AccessKit exposes it as a named button.
- Extended the X11/XTest application workflow to click that tile, verify the
  deterministic Open Project picker opens, and activate Cancel back to Home.
  Format screenshot assertions now read each actual egui paper rectangle at the
  active UI scale instead of depending on fixed approximate screenshot boxes.
- Moved the reference test fixture's unique green marker into the center of the
  image. Its screenshot assertion now remains valid when the saved view pans and
  crops the image's corners, while the sentinel pixel used for export-isolation
  testing remains at its original location.

### Validation and limits

The complete local Xorg/XTEST run passed all **31 screenshot states** and the
full application workflow, including mouse painting, layer transactions and
Undo, save/reopen, PNG/OpenRaster export, reference isolation, and journal-only
process-kill recovery. Xvfb is not installed in this execution environment; a
dedicated dummy Xorg server with XTEST and llvmpipe provided the real application
window and pointer/key events. I visually inspected the Home, populated Home,
Light/Dark New Artwork, and dark multi-reference captures. Format-art pixels in
the New Artwork screenshot produced color counts from 174 (A4 Print) to 318
(Portrait). The centered green reference marker produced 915/913 matching
pixels in dark normal/narrow captures. These images show meaningful progress,
not final design acceptance.

Local checks pass: `cargo fmt --all -- --check`, strict workspace Clippy,
**245 workspace tests**, workspace build, Windows GNU target check and strict
Clippy, the offscreen GPU smoke on llvmpipe/GL, Python syntax/diff checks, and the
31-state Xorg/XTEST artwork workflow. The GPU test is a software-adapter pixel
smoke, not Iris Xe or RTX hardware performance validation.

At the time this entry was written, the latest pushed commit `76a77b0` had a
failed Linux alpha step in Actions run
[`40270241090`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40270241090);
its Windows and macOS jobs passed. The marker was absent because the old fixture
placed it at the cropped top-left image pixel. The center-marker correction now
passes the exact local workflow; commit `f79c02d` still needs a fresh
Windows/Linux/macOS Actions run. The run-log and screenshot-artifact receiver
returns HTTP 403 here, so this pass was inspected through the local supported
capture path.

### Exact continuation

1. Verify the new three-platform Actions run for `f79c02d` and record each job's
   actual result; keep the downloadable Windows evaluation build withheld until
   broader Phase 8.3 visual and functional acceptance.
2. Continue the canvas workspace and contextual Brush, Color, Layers, and
   Reference visual integration. Keep the movable palette as the single primary
   toolbar; avoid replacing it with a fixed inspector stack.
3. Expand the populated Home capture to show a balanced first-viewport layout,
   then continue keyboard/focus and AccessKit review across Light, Dark, and
   High Contrast. Preserve artwork, recovery, input-routing, and cross-platform
   regressions. Narrator/NVDA, Windows DPI, and advanced physical S Pen behavior
   remain separate manual validation items.

## Palette viewport and wide Home checkpoint — 2026-10-26

This working checkpoint continues Phase 8.3 after `ff75acd`; it is not a phase
completion or a Windows evaluation build.

### Implemented and visually reviewed

- Corrected the movable tool palette's actual viewport placement. The tall
  vertical rail had been constrained upward by egui and overlapped the document
  options row even though its saved position was below that row. Its available
  height now ends above the status/zoom area and begins below contextual tool
  options. The rail scrolls when the window cannot fit every tool, and its
  placement updates when the viewport shape changes. Floating placement clamps
  against the usable viewport.
- Made the palette compact and collapsed by default, with legacy preference
  deserialization retaining that default. A dedicated grip handles movement;
  clicking tools no longer moves or redocks the panel. XTest checks expansion,
  tool clicks, dragging, edge snapping to both sides, and the required spacing
  from both the options row and bottom status region.
- Improved Home's large-window balance. The central content is capped and
  centered on ultra-wide screens, with room for all three format cards on a
  single row. XTest verifies centering and the three-card geometry in both
  themes at 1920px width. A theme-aware hero surface and contrast regression
  cover the light and dark slogan treatment.
- Expanded the Linux application screenshot/workflow automation to 32 required
  states, including wide dark/light Home, normal and narrow layouts, and a
  populated dark Layers panel with three real layer entries. The suite also
  exercises palette expansion/drag/docking and existing Home/New Artwork,
  Brush, Color, Reference, Recovery, document, export, and journal-recovery
  flows.

I visually inspected the generated dark and light wide Home, populated Home,
dark three-layer panel, and light canvas/palette captures. The palette is fully
visible in its rail viewport without obscuring the document controls or status
region. Home is centered at wide sizes and its format options fit together.
Recent artwork still uses explicitly marked development fixtures; its artwork
preview and card composition remain a visual refinement item. This work does
not yet satisfy Phase 8.3's complete visual redesign.

### Validation and continuation

Local verification passes: `cargo fmt --all -- --check`, strict workspace
Clippy, all workspace tests, workspace build, Windows GNU target check and
Clippy, the llvmpipe offscreen GPU smoke, and the complete 32-state Xorg/XTEST
application workflow. The software GPU result validates pixel output on this
runner, not performance on Iris Xe or RTX hardware. Physical advanced pen
behaviors and native Narrator/NVDA semantics remain separate validation items;
basic S Pen drawing was previously observed by the owner.

The changes in this checkpoint and the short-window follow-up were verified on
all three desktop CI platforms in Actions run
[`40278687191`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40278687191)
for commit `0384663`. Its Linux job passed the complete screenshot/artwork
workflow, and its Windows job built and packaged both existing executables.

### Exact continuation

1. Continue the Home and canvas visual integration, especially the recent-work
   preview prominence and compact/narrow workspace balance. Preserve the
   responsive palette fixes and populated-state screenshot coverage.
2. Continue keyboard/focus and AccessKit review across themes; document actual
   Narrator/NVDA limits. Keep Phase 8.3 active, preserve the Windows diagnostic,
   and withhold the consolidated Windows evaluation build, merge, and release
   until the broader overhaul is materially implemented and validated.

## Short-window New Artwork follow-up — 2026-10-26

Screenshot inspection at 640 × 480 showed the Custom format card clipped at the
bottom of the initial New Artwork view. Format cards now compact from 154 to 112
points when the viewport height is at most 560 points, while keeping their
illustrations, selected state, dimensions, and resolution readable. All seven
format cards, including Custom, fit in the first narrow viewport; the canvas
preview and precision settings remain below in the existing scrollable dialog.
An XTest assertion reads the live card bounds and fails if Custom is clipped.

The changed narrow capture was inspected and shows the complete Custom card.
The full 32-state Xorg/XTEST application workflow passes with this assertion,
including existing mouse painting, saved artwork, export-isolation, layer,
undo/redo, and process-kill recovery paths. The new unit test passes; the full
workspace suite has **246 passing tests**. Formatting, strict workspace Clippy,
workspace build, Windows GNU target check/Clippy, and llvmpipe offscreen GPU
smoke pass locally.

Commit `25930ed` had a completed Actions run
[`40277552431`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40277552431):
Windows and macOS passed, but Linux failed an XTest timeout while collapsing the
expanded palette. The test read the control rectangle immediately after a
layout-width change. This follow-up waits for the rectangle to remain unchanged
before clicking. The corrected workflow then passed in run `40278687191` for
`0384663`. This resolves that test-timing failure; it does not complete the
wider Phase 8.3 redesign or accessibility validation.

### Exact continuation

1. Continue visual/accessibility integration beyond the narrow card layout;
   maintain populated-state screenshots and artwork/recovery regressions.
2. Keep Phase 8.3 open and withhold the consolidated Windows evaluation build,
   merge, and release until the broader overhaul passes review.

## Dark narrow New Artwork visual regression — 2026-10-26

Extended the real Linux screenshot workflow to **33 states** by adding a dark
theme New Artwork capture at 640 × 480. It runs the same live Custom-card bounds
assertion as the light narrow capture, so all seven format options are checked
in both themes at the smallest tested size. I inspected the rendered dark
capture; the Custom card and its selected state fit completely, while the
canvas preview and advanced dimensions remain reachable by scrolling.

The full Xorg/XTEST workflow passes locally with the new state, along with the
existing populated Layers and Recovery views, painting/save/export/recovery
scenarios, and screenshots. This adds no product dependencies. Commit
`6271c79` pushed the expanded screenshot matrix. Actions run `40280252877`
failed its Linux workflow after the harness observed a workspace-save log
marker but read the preference file before the asynchronous write completed;
the app-level workflow passes locally. The harness now waits for the saved
open-state and close-state values before continuing. Its Windows and macOS
jobs also passed. The harness timing guard was pushed in commit `fb78a39`, and
Actions run
[`40280989079`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40280989079)
passed Windows, Linux, and macOS. Linux passed all 33 UI states plus the artwork,
export, and recovery workflows. Windows uploaded both the canvas demo and
stylus diagnostic artifacts. This validates the layout/test checkpoint, not
the complete Phase 8.3 program.

### Exact continuation

1. Continue the actual Home/canvas presentation and keyboard/focus/AccessKit
   integration. Keep the more complete narrow layouts, existing artwork
   protection, and full Phase 8.3 status; do not merge or release.

## Windows AccessKit startup blocker — implementation checkpoint

Physical evaluation of commit `089c40b` reproduced an AccessKit panic on the
Galaxy Book2 Pro 360: the HWND was visible before `Studio::init_accesskit` ran.
`Graphics::new` now requests an initially hidden winit window, installs the
existing Windows pointer observer, initializes the GPU and Studio/AccessKit,
configures the initial surface, and only then shows the window and requests its
first redraw. Initialization errors still flow through the existing application
failure path; the hidden local window/resources are dropped and the error is
reported instead of leaving an invisible process running. The WM_POINTER observer
and event loop are unchanged.

The Windows Actions job now includes a release-binary launch smoke. It checks a
visible BitchPixel top-level HWND, the post-AccessKit show marker, the first
presented Studio frame, completion of the existing alpha smoke, and a clean
process exit. The executable artifact is uploaded before this GUI check, so a
runner without a compatible wgpu adapter can report the GUI check as
inconclusive without losing the fixed Windows build. See
[`windows-startup-smoke.md`](windows-startup-smoke.md) for its coverage and the
dedicated interactive-runner strategy. Hosted CI cannot prove physical-monitor
visibility, inspect the rendered UI with a human, or test the Iris Xe driver.

Local validation passes: Rust 1.90 formatting, strict workspace Clippy, all
**247 workspace tests**, workspace build, Windows GNU workspace check and
Clippy, and the offscreen llvmpipe GPU smoke. The GPU test exercised the existing
artwork/rendering checks, not native window startup. This environment has no
Xvfb/display, so the Linux screenshot and artwork-window workflows were not run
locally.

Actions run [`40332091800`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40332091800)
passed Windows, Linux, and macOS. Linux passed its full screenshot, live-mouse,
artwork, export, and recovery workflows. The Windows release startup smoke
passed: its test step succeeded without the no-adapter inconclusive warning,
and it checks the visible process-owned HWND, post-initialization show marker,
first presented Studio frame, alpha-smoke completion, and clean exit. That is
hosted Windows GUI-session validation; it is not an Iris Xe or physical-monitor
test. Both `rust-paint-lab-canvas-demo` and the separate
`rust-paint-lab-windows-diagnostic` artifacts are present on the run page.

Follow-up Actions run `40334210743` exposed a Linux XTest timeout after it had
captured all requested artwork/recovery states: the test clicked Cancel in the
Home Open-project path modal but did not observe the modal close within its
timeout. The previous run passed this same workflow. Inspection found that this
test used the latest Cancel geometry without waiting for layout to settle and
did not prove the button event had reached Studio. The harness now waits for
stable Cancel bounds and a test-only cancellation event before checking that Home
returns. The diagnostic event is gated by the existing test mode and does not
change ordinary file-picker behavior. Local formatting, strict Clippy, all 247
workspace tests, build, Windows GNU checks/Clippy, and Python syntax checks pass
after this test stabilization.

Actions run [`40335600464`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40335600464)
passed Windows, Linux, and macOS on commit `b50e7ec`. Linux passed the full
screenshot, live mouse, artwork, export, recovery, and stabilized Home-picker
workflow. The Windows visible-HWND/first-frame smoke and macOS first-frame smoke
both passed without adapter-inconclusive warnings. The run has the refreshed
`rust-paint-lab-canvas-demo` and separate `rust-paint-lab-windows-diagnostic`
artifacts, plus Linux/macOS alpha packages. The Windows artifact includes the
referenced startup-smoke notes under `docs/`.

The Windows GUI smoke exercises the hosted runner session; no physical Galaxy
Book2 Pro 360 or Iris Xe launch was performed on this build. It does not replace
the owner's requested device evaluation. Basic S Pen drawing was observed on an
earlier build; advanced pen behavior is outside this startup correction.

### Exact continuation

1. Use the artifact panel on Actions run `40335600464` to download the fixed
   `rust-paint-lab-canvas-demo`; retain the separate stylus diagnostic artifact.
2. Retest launch on the Galaxy Book2 Pro 360, checking that the window appears
   without an AccessKit panic. Preserve the prior basic S Pen observation; only
   evaluate advanced pen behavior if it is in scope.
3. Resume the incomplete Phase 8.3 Home/workspace/accessibility program. Do not
   merge or release.

## Phase 8.3 typography and brush workflow checkpoint — validated

Continued from `f932827`. Replaced the previously mis-instantiated Manrope
extra-light asset and mislabeled Syne Regular asset with locally embedded static
Manrope 400/600/700 and Space Grotesk 600/700 fonts. The source revision, weight
instantiation, hashes, fallback behavior and OFL licenses are documented in
[`assets/fonts/README.md`](../assets/fonts/README.md), and both license texts are
included in desktop package notices. The fonts are embedded in the binary and
work offline.

Added a shared 17-role typography scale and applied it to the wordmark, Home
hero and quick-start cards, recent-artwork metadata, New Artwork cards, Brush
presets, Color and Layers headings, and recovery labels. Focused screenshot
review found and corrected two real layout issues: Home's narrow section hint
could reach the viewport edge, and the Brush Library's active size/opacity
summary used default body text and clipped at the panel edge. Both now fit or
truncate within their allocated widths. The Home hint is visibly clean at
640 × 480; the active Brush summary fits its floating panel.

Added live-canvas `[` / `]` brush-size shortcuts in 10% steps and Shift + `[` /
`]` in 1% steps, bounded to 1–256 px. They update the active document brush only
when the live canvas owns keyboard input; replay, dialogs and editable fields
keep their existing event ownership. Unit tests cover bounds and invalid
starting values, and the Linux XTest scenario checks a real 18 px → 19.8 px →
18 px sequence. No pressure or tilt is synthesized by this control.

Accessibility tree tests now initialize the same bundled-font style as the
application, so real widget trees can be exercised without missing egui named
families. The complete test suite passes **249 tests**. Strict workspace
Clippy, formatting, workspace build, Windows GNU target check/Clippy, and the
offscreen llvmpipe GPU smoke pass. The actual Linux window and XTest alpha
workflow passes with 34 screenshots across light/dark/high-contrast themes,
640 × 480, 1280 × 800, and 1920 × 1080 views. I inspected Home, New Artwork,
Brush, populated Layers/Recovery, and reference captures; the screenshots show
the updated typography and responsive hint. The workflow also covers real mouse
painting, brush shortcuts, native project save/reopen, PNG/OpenRaster, undo/redo,
reference exclusion from export, and journal-durable process-kill recovery.

Visual review also exposed a remaining compact-workspace issue: when a populated
Layers flyout and a previously sized Reference board are both open at 640 × 480,
their saved floating rectangles can overlap. Each panel remains reachable and
movable, but the narrow-window collision policy does not yet compact both panels
together while preserving their larger saved sizes. This is recorded for the
next workspace slice; the Phase 8.3 work remains open.

Latest tested commit: `d142e69`. GitHub Actions run
[`40423165359`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40423165359)
passed Windows, Linux, and macOS. Windows passed its visible-window/first-frame
startup smoke and uploaded `rust-paint-lab-canvas-demo` and the separate
`rust-paint-lab-windows-diagnostic`. Linux passed the real window/artwork path
and uploaded `rust-paint-lab-studio-ui-smoke`, `rust-paint-lab-linux-ui-diagnostics`,
and `rust-paint-lab-Linux-X64-alpha`. macOS passed its startup smoke and uploaded
`rust-paint-lab-macOS-ARM64-alpha`.

No new physical-device, Narrator, NVDA, or macOS VoiceOver testing occurred.
The owner's successful `b50e7ec` launch remains the Windows device baseline;
hosted and local software-rendered results do not establish Iris Xe or physical
high-DPI performance. Phase 8.3 remains active and is not accepted as complete.

### Exact continuation

1. Review the Actions result for this checkpoint and retain both existing
   Windows executable artifacts.
2. Continue the broader Phase 8.3 visual and functional work, especially the
   unified floating-panel workflow, Brush/Color/Layers controls, touch-friendly
   workspace navigation, and full keyboard/accessibility review.
3. Keep screen-reader and physical high-DPI/pen behavior identified as pending
   until tested on their native systems. Do not merge or publish a release.

## Outfit, compact workspace, and Recovery layout checkpoint — verified locally

Supersedes the earlier Space Grotesk display direction above. The requested
Outfit typography system is now bundled locally as static Outfit Bold 700,
ExtraBold 800 and Black 900. Manrope remains the compact interface face at 400,
600 and 700; numeric controls use a documented equal-width Manrope 600 metrics
variant. The shared 17-role system maps the wordmark and Home hero to Outfit
Black, page/section headings to ExtraBold, and panel/card headings to Bold.
The complete Outfit and Manrope OFL texts accompany all desktop package
notices. Source revision, weight instantiation, checksums and fallback details
are documented in [`assets/fonts/README.md`](../assets/fonts/README.md).

The existing interface and illustrated seven-format New Artwork cards now use
the stronger heading hierarchy. New Artwork keeps its live size/background
preview and all presets within the 640 × 480 view. Home uses the same visual
system without inventing user projects; the test fixture stays explicitly
identified as a development fixture.

The workspace checkpoint adds temporary compact layouts for saved Layers and
Reference panels, preserving large-window preference sizes during narrow
resizes. At 640 × 480, populated Layers and a Reference board fit side by side,
remain inside the application, and leave the one-line status strip visible.
The reference board displays one image at a time with touch-sized navigation,
retaining the saved multi-image arrangement for wider windows. Its compact
toolbar is a single row and the image remains prominent. When the Brush Library
opens in a narrow window, it receives temporary focus while Layers and
References remain saved and return when Brush closes. Horizontal toolbar docks
temporarily adapt to a vertical rail without replacing the saved preference.

Compact layer actions now expose Visible and Locked toggles inside a persistent
popover; edits update the document model and stay open for repeated toggles.
Escape closes the topmost popup before a contextual panel or the application.
Wide layer rows keep their normal click-to-dismiss Actions menu. The real XTest
workflow verifies visibility/lock changes and restoration, targeted duplicate,
reorder/delete, undo and native right-click on the rendered layer-name bounds.

The canvas contextual preset picker selects a real built-in raster preset and
updates the active document brush while retaining its color. The Brush Library
keeps favorites, search and engine-rendered previews without duplicating its
interactive size/opacity controls. Pressure and tilt remain available from
input data where supported; preset selection does not fabricate either value.

Screenshot review found and fixed a narrow Recovery dialog overflow that hid
the title when several populated checkpoints were present. The dialog now
derives its maximum size and scroll-list height from the current viewport,
retaining its header, recovery actions, and Done button. A layout unit test
covers 640 × 480, wide, and very small viewports; the XTest workflow checks the
actual rendered bounds after resizing and captures the populated dialog in both
themes. At 640 × 480 it measured `(24, 26)–(616, 454)` inside the viewport.
The GUI test also waits for fresh preview-button hover and settled geometry
before activating the real Preview control.

Local validation passes: **255 workspace tests**, formatting, strict Clippy,
workspace build, Windows GNU target check/Clippy, and the required offscreen GPU
smoke using Mesa llvmpipe/OpenGL. The real Linux/Xorg/XTest application workflow
passes with **37 screenshots** across Home, recent artwork, New Artwork,
canvas, Brush, Color, populated Layers, multi-image References, populated
Recovery, and high contrast. Captures include 640 × 480, 1280 × 800 and 1920 ×
1080 in light/dark themes. I inspected the generated Home, New Artwork, canvas,
Brush, Color, populated Layers/Reference, Recovery, and high-contrast images;
the narrow panels stay within the viewport and the Recovery heading and actions
remain visible. The UI workflow also verifies real painting, preset switching,
layer operations and undo, save/reopen, PNG/OpenRaster exports, reference
isolation, and process-kill journal recovery.

The Linux/Xorg/XTest screenshot workflow covers Home, recent artwork, New
Artwork, canvas toolbar, Brush, Color, populated Layers, multiple References,
Recovery and high contrast at narrow, standard and wide sizes. The user's known
successful `b50e7ec` Windows launch remains the hardware baseline. No Windows
10 device, Narrator/NVDA, physical high-DPI or advanced S Pen tests were
performed during this checkpoint. Hosted CI and this software-rendered Linux
run do not establish those hardware behaviors. Phase 8.3 remains active and
incomplete.

Code checkpoint `9c227bc` was pushed to
[`feature/studio-overdrive-phase-8`](https://github.com/starryeyesmedia/Rust-Paint-Lab/tree/feature/studio-overdrive-phase-8).
GitHub Actions run
[`40452947352`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40452947352)
passed Windows, Linux, and macOS. Windows passed the visible-window startup
smoke and uploaded the separate [`rust-paint-lab-canvas-demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40452947352/artifacts/12711700927)
and [`rust-paint-lab-windows-diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40452947352/artifacts/12710679967)
artifacts. Linux passed the screenshot/artwork workflow and uploaded
[`rust-paint-lab-studio-ui-smoke`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40452947352/artifacts/12711466098)
plus its Linux alpha package; macOS passed first-frame startup and packaged its
unsigned alpha. These are ordinary workflow artifacts, not a release. The
owner's tested `b50e7ec` Windows launch remains the physical-device baseline.

### Continuation

1. Continue the remaining Phase 8.3 visual and functional work, especially
   compact tool hierarchy, workspace navigation, and full keyboard/focus review.
2. Keep Windows pen pressure/tilt edge cases, physical DPI, and native
   screen-reader behavior labeled pending until validated on those systems.
   Do not merge or publish a release.

## Painting-engine continuation — deterministic stabilization and raster shapes

This checkpoint answers the engine-first continuation request. It is a
functional slice, not Phase 8.3 completion.

- Replaced the per-event low-pass position adjustment with a per-stroke,
  constant-memory One Euro filter. Its cutoff responds to input velocity;
  pressure and tilt remain attached to their original samples. A zero setting is
  an exact pass-through. Repeated/out-of-order millisecond timestamps remain
  finite, clipped-segment re-entry resets the filter, and commit explicitly
  connects the filtered path to the real release point. Cancelled provisional
  raster remains outside authoritative document tiles and history.
- Added persistent raster Line, Rectangle and Ellipse edits to
  `paint-document`. Outline width, brush color and opacity come from the active
  brush; rectangles/ellipses also support fill. The edit checks active layer
  lock/visibility, current selection, dimensions and tile budget, then records
  one undo transaction. Committed output participates in the existing native
  `.rpl` encoding, PNG export and undo/redo. These primitives are raster and are
  not separately editable geometry.
- Integrated drag previews and commit/cancel into the existing canvas pointer
  path for mouse and native pen events. `L`, `R` and `O` select line, rectangle
  and ellipse; Shift constrains line angle or shape proportions; Escape cancels.
  Preview geometry is a transient overlay and cannot contaminate the document.
  The Linux XTest workflow asserts visible contrast in the captured preview,
  byte-identical artwork after Escape, actual selected-layer pixels after each
  commit/save, and exact native-project bytes across undo and redo.
- Local verification: 262 workspace tests, formatting, strict Clippy, workspace
  build, Windows GNU target check/Clippy, llvmpipe offscreen GPU smoke and the full
  Linux/Xorg/XTest artwork workflow passed. The workflow also retains existing
  create/paint/layer/save/reopen/PNG/OpenRaster/recovery checks. See
  `docs/testing.md` for the exact command and validation limits.
- GitHub Actions run [`40529268958`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40529268958)
  passed Windows, Linux and macOS. Windows passed pinned-toolchain checks,
  workspace build, portable packaging/runtime inspection, and visible-window
  startup smoke; it uploaded the separate
  [`rust-paint-lab-canvas-demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40529268958/artifacts/12744953710)
  and [`rust-paint-lab-windows-diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40529268958/artifacts/12746885390)
  artifacts. Linux passed the GPU/window, mouse and full XTest artwork workflow
  and uploaded
  [`rust-paint-lab-studio-ui-smoke`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40529268958/artifacts/12745936098).
  macOS passed the first-frame startup smoke and packaged its unsigned alpha.
  This environment did not run a Windows device or macOS desktop. Basic S Pen
  drawing had been physically observed on the owner’s Galaxy Book2 Pro 360 in an
  earlier build; advanced shape capture, pressure/tilt dynamics, high-DPI,
  Narrator and NVDA behavior remain unverified on hardware.

### Next implementation steps

1. Measure stabilization latency/quality on representative recorded samples and
   add a replay-based adjustable stabilization comparison without changing the
   WM_POINTER architecture.
2. Continue engine workflows with useful shape properties and selection-aware
   fill/gradient work only where each operation can be persisted, undone and
   validated end to end. Keep Phase 8.3 active; do not merge or release.

## Linear and radial raster-gradient checkpoint

This checkpoint adds a practical two-stop gradient workflow to the existing
document engine and studio; it does not complete Phase 8.3.

- `paint-document` now applies linear projected and radial-distance gradients as
  bounded active-layer raster edits. The two RGB stops interpolate through a
  1025-sample linear-light sRGB ramp. Active-brush opacity supplies overall
  alpha. Pixel-center rectangle, ellipse and lasso selections clip the edit;
  hidden/locked layer checks and the existing tile budget still apply. Repeating
  an identical opaque gradient is a no-op without an extra history entry.
- `G` selects the Gradient tool. The contextual controls expose Linear/Radial,
  the active brush color as Start, an accessible end-color picker, Swap and
  opacity. Pointer drag previews against the current document in a capped
  512-pixel transient texture; the preview is painted in the canvas layer under
  floating references and tools. Escape and pointer cancellation do not touch
  authoritative artwork. Release commits one transaction and existing `.rpl`,
  PNG and undo/redo paths operate on its raster pixels without a format change.
- The application XTest workflow exercises cancellation byte-for-byte, both
  actual UI modes, preview screenshot visibility, native save, sampled selected-
  layer pixels and exact undo/redo. It also retains all existing mouse painting,
  layers, references, save/reopen, PNG/OpenRaster and journal-recovery paths. I
  inspected the generated linear preview image; the References panels stay
  visually above the gradient and the palette remains accessible.
- Local verification passes: **266 workspace tests**, format, strict Clippy,
  workspace build, Windows GNU target check/Clippy, llvmpipe OpenGL GPU smoke,
  and the full Xvfb/XTest application workflow with dark/light/high-contrast,
  populated Layers/Recovery and 640 × 480 through 1920 × 1080 captures.
- GitHub Actions run [`40546299074`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074)
  passed on Windows, Linux and macOS. Windows passed visible-window startup and
  uploaded the [`rust-paint-lab-canvas-demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074/artifacts/12752669319)
  and retained [`rust-paint-lab-windows-diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074/artifacts/12752474102).
  Linux passed GPU/window, mouse and full artwork/XTest workflows and uploaded
  the [`rust-paint-lab-studio-ui-smoke`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074/artifacts/12752754019)
  screenshot archive and unsigned [`Linux x64 alpha`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074/artifacts/12754635831).
  macOS passed first-frame startup and uploaded its unsigned
  [`macOS ARM64 alpha`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074/artifacts/12753057734).
- Layer compositing remains the project's existing encoded-sRGB path; only stop
  interpolation is linear-light. Stops are solid RGB with global brush opacity,
  not independently transparent stops. The result is raster, not an editable
  gradient object, and selection state is still not persisted in native v1.
  Windows device rendering, screen readers and physical pen behavior were not
  retested; owner-confirmed basic S Pen drawing and the successful `b50e7ec`
  launch remain the hardware baseline.

### Immediate continuation

1. Continue the next engine-first workflow with measured stabilization quality
   and a useful end-to-end editing feature; preserve undo, selection, save and
   recovery boundaries. Keep Phase 8.3 active and do not merge or release.

## Workspace launcher and raster-layer integration — `8985754`, CI passed

The compact workspace and Layers pass now connects the F6 Workspace controls to
the real floating Layers and Brush Library panels. The old Inspector Brush and
Layers tabs no longer repeat the full set of settings and stack actions; they
summarize the active state and launch the focused panel. Launching a panel closes
the redundant Inspector and preserves the selected Workspace tab preference.
The full Brush Library retains brush-angle adjustment in its advanced dynamics
section; a document regression confirms calligraphy angle changes rasterized
mark orientation.

Layer rows now use a bounded per-layer thumbnail cache. It downsamples each
document layer to at most 96 × 64 pixels, includes layer opacity, reuses its egui
texture, and only re-renders the layers whose tile identities or opacity changed
after a document revision. The cache drops deleted layer entries. A storage test
drives a brush stroke, shape and gradient through Undo/Redo, saves and reopens the
actual `.rpl` file, then compares the reopened thumbnail pixels. An additional
cache assertion confirms adding or changing one layer does not resample unchanged
layers.

The Layers panel supports direct row selection, drag reordering plus the existing
accessible Actions menu ordering commands, compact accessible visibility/lock
checkboxes, in-row rename, opacity/blend settings, and targeted clear. The new
targeted clear operation is a normal document transaction keyed by layer ID; it
preserves the current selection and Undo restores the layer's pixels. In compact
layouts, short but accessible action labels and reduced row preview/grip sizes
keep the controls within their planned panel slot. This fixed a measured
31-point Layers/Reference overlap at 640 × 480 without changing saved desktop
panel sizes.

The Linux Xorg/XTest workflow now opens Layers and Brush from Workspace controls,
verifies those launchers close the duplicate Inspector, tests direct layer
selection, paints the target layer through the ordinary mouse input pipeline,
clears it from its contextual menu, checks the thumbnail becomes transparent,
and verifies Undo restores the exact thumbnail fingerprint. Screenshot capture
now excludes the selected purple frame and requires actual raster-color changes
inside the thumbnail bounds.

Local validation on this working tree passes `cargo fmt --all -- --check`, strict
workspace Clippy, workspace build, Windows GNU workspace check/Clippy, and **270
workspace tests**. The required offscreen smoke passed on Mesa llvmpipe/OpenGL
with `RPL_REQUIRE_GPU=1`; this is software-adapter validation, not a performance
measurement for Iris Xe or RTX 3060. The full real-X11/XTest artwork workflow
passed, including the populated Layers/Reference no-overlap check at 640 × 480,
true thumbnail pixels at 640 × 480, 1280 × 800 and 1920 × 1080, `.rpl` save/open,
reference-isolated PNG/OpenRaster exports, recovery and existing artwork
regressions. I inspected the populated light Layers and dark compact
Layers/Reference screenshots. The compact Layer list is scrollable; only its
front rows fit at once in a 640 × 480 window.

Commit [`8985754`](https://github.com/starryeyesmedia/Rust-Paint-Lab/commit/8985754f78723fed3722bf69860e463be5fe10e1)
was recovered from the interrupted working tree, validated, committed and
pushed on `feature/studio-overdrive-phase-8`. GitHub Actions run
[`40574152036`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036)
passed on Ubuntu, Windows and macOS. The Windows job passed its visible-window
startup smoke and uploaded the [`rust-paint-lab-canvas-demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036/artifacts/12767816064)
and retained [`rust-paint-lab-windows-diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036/artifacts/12766097424)
artifacts. Linux passed its software-GPU, live-window, mouse and complete
artwork/XTest workflow, uploaded the [`studio screenshot archive`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036/artifacts/12767007921)
and [`Linux x64 alpha`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036/artifacts/12767816310).
macOS passed first-frame startup and uploaded its unsigned
[`ARM64 alpha`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036/artifacts/12765709849).

No physical Windows device, high-DPI or advanced pen validation was run in this
checkpoint; the previously owner-confirmed basic S Pen behavior remains the
hardware baseline. This verified slice does not complete Phase 8.3. Broader
workspace design, layer presentation and keyboard/focus workflows remain active
follow-up work.

### Next implementation steps

1. Continue the coordinated panel system and canvas-first hierarchy, then expand
   the XTest/accessibility workflow to exercise opacity and blend mode with
   saved pixel assertions. Keep Phase 8.3 active.

## Layer editing and native compositing — local verification passed, CI pending

This checkpoint extends the workspace/layer integration with one complete
selection workflow and verified layer-property behavior. It does not complete
Phase 8.3.

- `paint-document` adds `Engine::copy_selection_to_new_layer`. The operation
  copies nontransparent premultiplied source pixels selected by a rectangle,
  ellipse, or lasso at pixel centers, inserts and selects a normal layer above
  the visible unlocked source, and retains both the source and selection. It
  respects the existing 4096 × 4096 dimension, layer-count, tile-count, and
  undo-history bounds, and saves through the existing `.rpl` format. No document
  schema or migration change was needed.
- Tests assert exact masks and source pixels, unchanged composite for the normal
  fixture, one-step undo/redo, native save/reopen, empty/hidden/locked rejection,
  and atomic rejection at layer/tile capacity. Blend-mode coverage now asserts
  exact output pixels for Normal, Multiply, Screen, and Overlay at partial and
  zero opacity, including native and PNG roundtrips.
- The Layers UI exposes the operation from the selected row's contextual
  Actions menu. Opacity has a full-width interaction rail and an explicit
  accessible label/value; opacity and blend mode still modify the active native
  layer. The XTest workflow invokes the actual controls, decodes saved project
  bytes, confirms copied source tiles are unchanged and copied pixels match,
  then verifies exact undo/redo. It verifies the real opacity slider updates
  saved state, Ctrl+Z/Ctrl+Y remain usable after slider focus, and Multiply
  persists with opacity through native save and history.
- `Engine::edit` now detects unchanged candidate snapshots using layer metadata
  and copy-on-write tile identity. Identical property events do not increment the
  revision or add undo entries. The pixel comparison is avoided, so this does
  not scan or copy entire artwork buffers.
- The full Linux Xorg/XTest workflow passed with a 2200 × 1400 virtual root. It
  includes mouse painting, shape and gradient preview/commit/cancel, selection
  copy, layer properties, PNG/OpenRaster import/export, `.rpl` save/reopen, and
  forced process termination followed by exact journal recovery without its
  paired snapshot. The screenshot set includes populated Layers/References at
  640 × 480, 1280 × 800, and 1920 × 1080 plus populated recovery, dark mode, and
  high contrast. I inspected the layer-copy, narrow dark Layers/Reference,
  Home/recent-project, and high-contrast captures. One high-contrast Home image
  was intentionally captured at a scrolled position and crops lower format
  cards; this remains a visual-review limitation, not a claim that all Home
  layouts are fully polished.
- Local validation passes **275 workspace tests**, `cargo fmt --all -- --check`,
  strict workspace Clippy, workspace build, Windows GNU workspace check/Clippy,
  required llvmpipe/OpenGL offscreen GPU smoke, and the complete Linux/Xorg/XTest
  application workflow. GitHub Actions run
  [`40590882859`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40590882859)
  passed on Windows, Linux, and macOS for final tested code commit `87588a4`.
  Windows visible-window startup passed; Linux passed software-GPU, mouse,
  populated UI/artwork, save/reopen, export, and journal-recovery workflows; and
  macOS first-frame startup passed. The run published the Windows
  [`rust-paint-lab-canvas-demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40590882859/artifacts/12774436753),
  retained [`rust-paint-lab-windows-diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40590882859/artifacts/12774361545),
  Linux [`rust-paint-lab-studio-ui-smoke`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40590882859/artifacts/12775130275),
  and unsigned Linux and macOS alpha artifacts. They are ordinary Actions
  artifacts, not a release. The owner-confirmed `b50e7ec` Windows launch and
  basic S Pen drawing remain the physical baseline; no new Windows pen,
  display-scaling, or native screen-reader validation occurred.

### Immediate continuation

1. Keep Phase 8.3 active: continue broader workspace/focus work and complete a
   dedicated, accurate Home/accessibility screenshot review. Merge Down and
   broader layer semantics remain out of this slice.

## Deep development campaign — timeout resolved and Phase 9 Smudge slice

The continuation started from `1978106` on `feature/studio-overdrive-phase-8`.
Commit `745543b` raised the Linux full-artwork smoke timeout from five to ten
minutes without removing any checks; Actions run
[`40708740871`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40708740871)
passed in 16m 22s across Windows, Linux and macOS.

Phase 9's first engine slice adds transient `StrokeMode::Smudge`. It samples
premultiplied pixels from only the selected layer, carries color across
transparent pixels, and writes bounded COW transaction tiles. Size, strength,
pickup/flow, pressure, hardness, smoothing and selection clipping use existing
brush/document settings. Commit replaces only changed layer tiles as one undo
step; cancel/focus loss discard preview tiles. Device eraser/inversion retains
precedence. Concurrent contacts are ignored while the layer is privately held,
preventing stale-copy overwrites. `StrokeMode` is not serialized; native v1
documents and brush presets do not change.

The primary palette exposes Smudge as **S** with contextual size, strength,
pickup and pressure controls. The ordinary input processor/router feeds the
same document transactions for mouse and pen. `paint-document` tests verify
transparent-pixel pickup, pressure and strength, selection/lock guards,
eraser precedence, preview/cancel, no-op history behavior, exact undo/redo, and
native/PNG roundtrips. A `LiveMouse` test verifies routed preview, focus-loss
cancellation, commit and undo on document pixels. The full Linux Xorg/XTest
artwork workflow was rerun locally; it selected Smudge, changed selected-layer
tile bytes through real UI input, saved, undid and redid exact native files, and
captured `/tmp/bp-smudge-screens/smudge-workspace.png`. The screenshot showed
the contextual row's final helper sentence clipped at the right edge; that
redundant inline sentence has been removed and retained as tool hover text. A
refreshed screenshot is expected from the implementation Actions run.

After that last text-only layout adjustment, formatting and strict workspace
Clippy pass; all **286 workspace tests** pass, including the llvmpipe/OpenGL GPU
smoke. The full workspace build and local `x86_64-pc-windows-gnu` workspace
check/strict Clippy pass. MSVC package/startup checks and macOS runtime validation
await Actions on the implementation commit. No owner hardware test was requested
or performed; WM_POINTER, pen pressure/tilt and stabilization algorithms were
not changed.

### Continue from here

1. Commit and push this Smudge checkpoint, then verify its Windows/MSVC visible
   startup, artifacts, Linux artwork/screenshot workflow and macOS smoke in
   Actions.
2. Continue Phase 9 with genuinely mixed-color brush painting or configurable
   barrel-button behavior; keep project format v1 unchanged unless a real
   persistent-data need requires a migration.
3. Then develop transformed Place Image, selection transforms, symmetry and
   raster masks. Phase 10 editable vector/text and Phase 11 timelapse/animation
   remain untouched and active in the longer campaign.

## Phase 9 continuation — Color Mixer vertical slice

Color Mixer is implemented as a separate transient stroke mode and toolbar tool
(**X**), not as a preset rename of Smudge. It blends the selected brush RGB into
covered pixels on the active raster layer while depositing paint controlled by
brush opacity and flow, plus pressure when enabled. Mix strength, size,
hardness, shape, tilt and selection coverage feed the actual raster operation.
It previews through bounded private COW tiles, owns the active layer exclusively
while open, and commits/cancels as one undoable transaction. The layer/pixel
result remains ordinary native-v1 raster data; no document or preset migration
was introduced. Workspace preferences persist mix strength.

Coverage includes pixel-level engine cases for existing and transparent pixels,
different mix values, no-op paint loading, pressure, selection, lock state,
device eraser, cancellation, stale transaction safety, undo/redo and native/PNG
roundtrips. A routed `LiveMouse` test checks preview, focus-loss cancellation,
commit and undo. On 2026-10-28 the full Linux Xorg/XTest workflow selected X,
changed saved selected-layer tile bytes, checked exact undo/redo, and completed
the remaining artwork, export, journal-recovery and clean-close scenarios. The
first captured `color-mixer-workspace.png` exposed clipped pressure controls.
Wrapping the entire tool row made them visible but reduced the narrow reference
image to a 56% width share. The corrected compact row removes duplicate
Brush/Undo/Redo buttons, puts pressure toggles in a Dynamics menu, and reduces
slider widths on narrow windows. The final mixer screenshot was inspected with
all four primary settings visible and no clipping; the 640×480 reference board
again renders at 85.1% image/window width share.

The 2026-10-28 workspace suite passed **291 tests**, including the llvmpipe
OpenGL GPU smoke. Formatting, strict workspace Clippy, workspace build, and
`x86_64-pc-windows-gnu` check/Clippy also passed after the responsive row
change. The full Xorg/XTest application workflow passed with 52 captured UI
states and completed Save/Reopen, PNG/OpenRaster, layer, undo/redo, and
journal-recovery assertions. Screenshots were inspected, including the mixer
screen and narrow reference board. The run used Mesa llvmpipe/OpenGL and is an
automated software-GPU test. It does not measure physical stylus response or
validate color-managed/wet-media mixing. The mixer currently interpolates
encoded sRGB channel bytes. Windows/Linux/macOS Actions for this checkpoint are
pending. Do not mark Phase 9 or Phase 8.3 complete based on this slice.

### Continue from this checkpoint

1. Commit and push the verified Color Mixer slice, then confirm its
   Windows/Linux/macOS Actions run.
2. Continue Phase 9 with configurable native pen barrel-button behavior, default
   hold-to-erase/restore, while keeping WM_POINTER capture and document format
   unchanged.

## Phase 9 continuation — Native pen barrel assignments

The Windows WM_POINTER observer remains unchanged; Studio now consumes its
normalized barrel bit after the existing AccessKit/winit UI hit test. The
workspace preference defaults to **Hold for Eraser** and also offers
**Eyedropper**, **Pan**, or **Disabled**. Eraser transitions synthesize ordinary
Up/Down samples through `CanvasSession`, so the painting segment commits and a
new erase transaction begins; release commits that segment and starts a new
segment using the selected brush mode. Pan emits bounded physical-pixel deltas
to the existing canvas navigation state. Eyedropper samples the visible
composite and returns its color through the existing brush-color flow. Temporary
Eyedropper/Pan contacts stay hover-only and suppressed until actual lift.
Capture loss, deactivation, UI capture and pointer cancellation clear router
state. Physical pen inversion and eraser remain authoritative.

The platform-independent `BarrelRouter` and `CanvasSession` currently pass eight
synthetic lifecycle/transaction tests, including selected-layer pixel erasure,
separate undo transactions, release-to-paint, composite color sampling without
painting, Pan deltas, ID reuse, terminal cleanup, focus/cancel cleanup and
suppression after temporary actions. Workspace persistence and legacy-default
coverage also pass. The complete workspace suite passed **300 tests**; strict
native and Windows GNU Clippy, the workspace build, and Windows GNU target check
also pass. No physical Galaxy Book2 Pro 360
barrel-button test has been performed; this feature does not claim hardware
verification. WM_POINTER capture, pressure, tilt, history and mouse-promotion
suppression were not changed.

## Phase 9 continuation — interactive Place Image and OpenRaster export

The working tree continues from remote commit `7138092` on
`feature/studio-overdrive-phase-8`. Place Image now decodes bounded PNG/JPEG
inputs into temporary state and presents a canvas preview with move, scale,
rotation, Fit, Fill, Center, Reset, Cancel and explicit Place actions. The source
is committed only as a new editable raster layer through one document transaction;
cancel leaves revision, history and artwork unchanged. Reference images remain a
separate workspace-only feature. Native `.rpl` version 1 and OpenRaster stack
metadata are unchanged. Placement currently uses nearest-neighbor resampling and
does not apply EXIF orientation or embedded ICC profiles.

Investigating Actions run `40721752674` showed its Linux job did not hit the
10-minute workflow-step limit. The job reached `Timeout: layered OpenRaster export`
after 2m43s in the artwork workflow; that test used a 15-second file wait and
did not report whether the export shortcut arrived or whether export returned an
error. The exporter previously cloned and revalidated the entire document for
each layer, so it now converts populated sparse tiles directly and renders the
flat background without constructing temporary documents. The test now records
export start/failure/completion and elapsed seconds, with a bounded 120-second
completion wait. A pixel test covers premultiplied-alpha conversion, tile-edge
clipping and keeping layer opacity in OpenRaster metadata.

The Linux placement smoke has since been expanded to exercise the real Scale and
Rotate sliders, drag the preview on the canvas, assert the transform state changes,
inspect colored fixture regions within the GPU-rendered preview bounds, and verify
application Undo/Redo around the final Place. This expanded Xvfb flow has not run
locally or in Actions yet.

Local validation on pinned Rust 1.90 passes formatting, strict workspace
Clippy, the full **308-test workspace suite**, workspace build, Windows GNU
workspace check/Clippy, and required llvmpipe/OpenGL GPU smoke. The standalone
placement engine tests cover transformed pixel values, quarter-turn rotation,
document clipping, invalid-transform atomicity, undo/redo and native save/reopen.
The complete Xvfb/XTest artwork run and the new real-window placement workflow
could not run here because this environment has no Xvfb. Actions on the current
implementation are pending. The latest known Actions run remains
`40721752674`: Windows startup/artifact packaging and macOS checks passed, while
the Linux export wait failed. This does not validate image placement on physical
WM_POINTER hardware or change the owner-confirmed basic S Pen baseline.

### Continue from this checkpoint

1. Commit and push the image-placement/export optimization and wait for its
   Windows, Linux and macOS Actions run. Inspect its export timing markers and
   verify the placement preview/cancel/commit workflow on Linux Xvfb.
2. Continue Phase 9 with Move Selection to New Layer, distinct from Copy
   Selection, including exact source/pixel integrity and undo/redo tests.
3. Then expand selection transforms and symmetry/guides; defer masks until a
   compatible persistence migration and recovery strategy are designed. Phase 10
   editable vector/text and Phase 11 timelapse/animation remain unimplemented.

## Phase 9 continuation — Move Selection to New Layer

The next local vertical slice adds **Move selected pixels to new layer** beside
the existing non-destructive Copy command in the selected layer's accessible
Actions menu. Move clears only selected nontransparent pixel centers from its
visible, unlocked source, removes emptied sparse tiles, and inserts those pixels
immediately above the source while inheriting opacity and blend mode. Selection
state is retained, the visible composite is unchanged, and the operation is one
ordinary undo transaction. Existing v1 storage is sufficient.

Tests compare source/destination premultiplied pixel bytes, retain unselected
pixels, verify exact composite equality for Normal/Multiply/Screen/Overlay, and
exercise native save/reopen, undo/redo, empty/locked failures and final tile-budget
rejection. The Linux XTest path invokes the real Layers Actions menu after Copy,
saves and verifies that the source copy becomes empty while the moved layer gets
the exact same bytes, then undoes to the byte-identical copied document. It adds a
populated moved-layer capture. Local formatting, strict Clippy, the **310-test
workspace suite**, workspace build, Windows GNU check/Clippy and llvmpipe/OpenGL
GPU smoke pass. A local Xvfb desktop is unavailable, so XTest and screenshot
inspection await Actions.

The transformed-image commit `75418d1` is pushed. Its GitHub Actions run was still
queued when this note was written; the later Move integration reruns the complete
workflow. Keep the prior failure visible in reporting until the new OpenRaster
markers confirm the outcome. Next after CI: selection transforms and symmetry /
guides, then evaluate brush dynamics; layer masks require a separately designed
document migration.

## Phase 9 integration update — 2026-10-28

The Xorg placement smoke now exercises both cancellation and the complete
interactive transform/commit path. It captures the pending GPU preview, drags the
Scale and Rotate handles, moves the transformed image across the canvas, confirms
the preview bounds and fixture pixels, commits a new raster layer, saves it, then
uses application Undo/Redo and compares the complete native `.rpl` bytes. The
smoke copies three populated preview captures (cancelled, initial, transformed)
to the UI artifact directory. Its first live run found that the Scale hit target
covered the combined slider/value/label rectangle; changing the test to drag the
actual handle made scale and rotation changes deterministic.

The visual review also found a real rendering defect: the provisional image mesh
was appended after egui had generated its interface, causing it to cover the
floating transform panel. The preview is now emitted on egui's middle layer,
above the canvas but below floating panels. Captured screenshots confirm the
preview and transform controls remain visible together, and the live XTest path
operates the controls successfully.

Validation on Rust 1.90: `cargo fmt --all --check`, the complete **310-test
workspace suite**, strict workspace Clippy, workspace build, Windows GNU workspace
check and strict Clippy, and llvmpipe/OpenGL GPU smoke pass. The direct Xorg/XTest
image-placement workflow passes cancel, rendered fixture inspection, scale,
rotation, movement, placement, save, undo and redo. The full Linux Xorg/XTest
artwork, export and recovery workflow also passed locally and produced **52
screenshots**, which were inspected at representative 640 × 480 and 1920 × 1080
sizes, in light/dark themes, with populated Layers, References, and Recovery, plus
New Artwork and canvas/tool states. This is software-rendered Linux validation;
Windows physical stylus behavior is unchanged and no new hardware testing is
claimed. Windows and macOS hosted Actions for this latest integration commit are
pending confirmation.

### Next development steps

1. Confirm the current three-platform Actions run, including its Windows startup
   and portable executable artifact and Linux placement/export diagnostics.
2. Advance Phase 9 with a completed selection-transform workflow or QuickShape;
   keep `.rpl` v1 unchanged where the feature is transient/raster.
3. Design versioned mask storage with v1 migration and recovery tests before
   adding editable masks. Phase 10 vector/text and Phase 11 timelapse/animation
   remain incomplete.

## Phase 9 — Symmetry and drawing guides (local checkpoint)

The live raster engine now supports Off, vertical, horizontal, combined vertical
and horizontal reflection, and bounded radial copies (2–16). Each stroke captures
its symmetry mode and normalized origin at Begin, so changing the workspace
setting cannot alter an active transaction. Original and generated dabs share a
single provisional/commit/cancel transaction, selection clipping, layer guards,
undo/redo, save/reopen and export behavior. Coincident copies on a reflection
axis or radial origin are deduplicated, preventing repeated opacity buildup;
smudge retains independent bounded pickup state per generated path. Transform
matrices are cached once per transaction, and fixed 16-entry state buffers avoid
per-sample allocations and repeated trigonometry in the dab loop.

The canvas exposes a compact, accessible Drawing Symmetry panel from the primary
palette and the **Y** shortcut. It offers the five modes, radial count and a
document-normalized X/Y origin with a Center action. Purple symmetry guides are
clipped to the paper and follow the current zoom/pan mapping. Symmetry settings
are stored only in local workspace preferences. Generated copies become ordinary
raster pixels; guides and settings are not serialized into `.rpl`, and no native
format version or migration was needed. Shape/gradient transforms, ruler and
perspective guides, QuickShape recognition, and editable/vector symmetry remain
out of scope.

Automated coverage checks exact mirrored and radial raster locations, one-step
undo/redo, cancellation of all provisional copies, in-flight setting snapshots,
coincident-dab opacity, bounded guide geometry, input identity/pressure/time
preservation, unsupported tilt, invalid settings and the AccessKit tree. The
actual Linux app workflow uses keyboard Y and the Vertical
panel option, paints through winit → Processor → Router → raster engine, checks
both original and reflected pixels in the saved native layer, then compares the
full project bytes after Undo/Redo. Local Xorg/XTest/llvmpipe passed and captured
`symmetry-panel-vertical.png`; that screenshot was visually inspected. The full
artwork/recovery workflow passed with this behavior and **53 captures**. On the
final worktree, formatting, strict workspace Clippy, **317 workspace tests**,
workspace build, Windows GNU check/Clippy and required llvmpipe GPU smoke all
passed. GitHub Actions is pending after the push. Older testing notes mention
no local Xvfb; this session used the dedicated Xorg display `:102`. No new
physical Windows S Pen test was performed. Existing basic S Pen drawing and
Windows launch remain the previously owner-tested baseline; advanced
symmetry/stylus combinations require future device testing.

## Phase 9 continuation — place symmetry origin directly on canvas

The Drawing Symmetry panel now has a keyboard-accessible **Place origin on
canvas** toggle. With it armed, a mouse click or native pen contact on the
artboard maps through the current zoomed document viewport and sets the
normalized workspace origin. The initiating contact and its remaining pointer
stream are consumed, so choosing an origin cannot paint. Escape, focus loss,
resize, close, mode change, and native capture-loss clear the temporary state;
placement and cancellation never alter the document. The origin remains a local
workspace preference and `.rpl` v1 is unchanged.

The full Xorg/XTest app workflow placed a non-centered origin at 50%, 68%,
verified workspace persistence, unchanged native artwork bytes and no new paint
stroke, then verified original/mirrored pixels and byte-exact Undo/Redo around
that custom origin. A second armed gesture was cancelled with Escape and checked
to leave the prior origin and artwork unchanged. The updated UI suite passed and
the `symmetry-panel-custom-origin.png` render was inspected alongside wide Home,
New Artwork, and narrow populated Layers/Reference screens. The latter still
leaves only a small strip of exposed canvas at 640 × 480 when both large panels
are open; coordinated compact panel behavior remains a follow-up.

On the final code worktree, formatting, strict Clippy, workspace build, Windows
GNU workspace check and Clippy, required llvmpipe/OpenGL GPU smoke, Python syntax
and the full **318-test workspace suite** passed. The local Xorg/XTest artwork,
export and forced-process-termination recovery workflow also passed. GitHub
Actions run [`40738225142`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40738225142)
passed Windows, Linux and macOS for preceding commit `5c8ee67`; Actions for the
direct-origin follow-up are pending its push. Mouse behavior was exercised in
the live Linux app; native-pen placement is integrated with the existing WM_POINTER
capture path but awaits future physical verification. This does not complete
Phase 8.3 or Phase 9.

### Continuation

After verifying the current Actions run, prioritize an end-to-end selection
transform or QuickShape improvement, then design versioned non-destructive layer
mask storage and migration. Keep the next commits focused on artist workflows.
Phase 9 as a whole, Phase 8.3 acceptance, Phase 10 vector/text and Phase 11
timelapse/animation are still active/incomplete.

## Phase 9 — direct selected-pixel movement (2026-10-28)

Rectangle, ellipse, and lasso selection tools now move selected pixels directly:
press inside the active selection and drag; the selection boundary follows as a
preview, while Escape cancels without changing the document. Releasing applies
one ordinary `transform_selection` translation to the selected, visible,
unlocked raster layer. The whole selection is clamped inside document bounds,
the active pointer owns the gesture through release/cancellation, and another
native pen identity cannot paint through it. Focus loss, window exit/resize,
canvas-blocking UI, tool shortcuts, and native pointer capture loss abandon the
provisional move. The existing numeric/rotation/scale/flip controls remain
available as keyboard-adjustable alternatives. No `.rpl` schema or version
change was needed.

The actual Linux Xorg/XTest artwork workflow verified Escape cancellation leaves
the saved project byte-identical; a second drag changed actual layer tile data;
save, Undo and Redo matched the pre-move and moved project bytes exactly; the
translated artwork could then be selected again and copied through the existing
Layers action workflow. Its rendered post-move workspace capture is
`selection-move-canvas.png`. The full workflow continued through existing brush,
gradient, Smudge, Color Mixer, layer, import/export, recovery and clean-close
checks. The standalone document regression verifies moved pixel centers,
single-edit history, v1 encode/decode, Undo and Redo. Selection geometry tests
cover lasso shape preservation, invalid values, and right/bottom document-edge
clamping.

On the final local tree, pinned Rust 1.90 formatting, strict workspace Clippy,
workspace build, Windows GNU workspace check/Clippy, GPU-required
llvmpipe/OpenGL smoke, and **320 workspace tests** passed. The Xorg/XTest app
workflow passed with its populated screenshot matrix. I inspected the captured
selection-move workspace; it shows the moved raster marks and the current panel
layout, while panel crowding remains an existing visual follow-up. This is
software-rendered Linux validation only; native WM_POINTER integration is kept,
but no new physical S Pen or Windows capture-loss test was performed. Actions
run [`40741970797`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40741970797)
has now passed Windows, Linux, and macOS for the preceding `f2046e3` symmetry
origin checkpoint. The selected-move Actions run
[`40744882686`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40744882686)
also passed Windows, Linux and macOS in 17m 41s for `9f6614a`, with separate
canvas and stylus Windows artifacts.

### Continue here

1. Verify three-platform Actions for the selected-pixel move and transform
   commits, preserving Linux artwork automation and both Windows artifacts.
2. Integrate QuickShape into the existing raster geometry workflow.
3. Design versioned editable mask storage and v1 migration before implementing
   layer masks. Phase 10 vector/text and Phase 11 timelapse/animation remain
   unimplemented.

## Phase 9 — interactive canvas selection transforms (2026-10-28)

Selection tools now expose direct canvas handles: dragging inside moves the
selected pixels, dragging the lower-right handle scales proportionally, and
dragging the handle above the selection rotates around its center. Shift snaps
rotation to 15-degree increments. Rectangle, ellipse and lasso selections use
the same transform interaction; previews remain overlays until release, when
one existing document transaction commits the changed pixels. Escape, focus
loss, window exit/resize, tool changes, blocking UI and pointer capture loss
cancel the provisional gesture. Scale and rotation are clamped to finite,
document-bounded geometry. The selection remains available after an edit for
subsequent transforms. The document format is unchanged.

The full Xorg/XTest workflow dragged both handles in the running app, verified
actual raster-tile changes, saved each operation, and checked byte-exact Undo
and Redo. It also cancelled a scale preview and verified unchanged document
bytes and clean save state. The existing mouse drawing, symmetry, shape,
gradient, Smudge, Color Mixer, layer, import/export and forced-process-kill
recovery workflow passed in the same run. The workflow produced 55 UI captures;
I inspected `selection-scale-rotate-workspace.png`, which shows the visible
scale and rotation handles and selection outline. The screenshot also exposes
the already documented crowded Layers/Reference layout; these transforms do
not claim to resolve that workspace issue.

Formatting, strict workspace Clippy, workspace build, Windows GNU workspace
check/Clippy, GPU-required llvmpipe/OpenGL smoke, Python syntax and **322
workspace tests** passed locally. The full Linux Xorg/XTest artwork workflow
passed. The new commit's hosted Windows/Linux/macOS CI and Windows artifacts
passed in Actions run [`40747903490`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40747903490).
The run completed on Windows, Linux and macOS with the separate canvas and
stylus artifacts. No new physical S Pen pressure/tilt or transform testing was
performed; native WM_POINTER continues through the same pointer lifecycle.

Next, prioritize a complete QuickShape workflow or the versioned mask-storage
design and v1 migration tests before implementing editable layer masks. Keep
image placement and configurable S Pen barrel action on the Phase 9 list.
Phase 8.3 acceptance, Phase 9, Phase 10 vector/text, and Phase 11
timelapse/animation remain active and incomplete.

## Phase 9 — QuickShape recognition and raster transactions (2026-10-28)

The primary tool palette now provides a QuickShape recognition tool (`Q`).
During a bounded pointer gesture it shows a transient path preview; on release
it recognizes a near-straight line, a closed axis-aligned rectangle, or a
closed ellipse and commits the result through the existing raster shape
transaction. The gesture retains the active brush size, color and fill choice.
Escape and interrupted pointer lifecycles discard the preview, and
unrecognized gestures are rejected without a document edit. The application
still uses the existing document format; no geometry is mislabeled as editable
vector content.

Automated recognizer tests cover each supported shape and rejection of short,
self-crossing, non-finite, too-small and over-limit gestures. In the real
application XTest workflow, QuickShape is exercised for cancellation,
unrecognized input, line/rectangle/ellipse commits, native saved-layer pixels,
and exact Undo/Redo. The pixel assertion compares changed nontransparent
pixels across each gesture's bounded document region; checking a single edge
coordinate proved brittle where earlier artwork had already painted that
sample. The preserved failed run's project data confirmed 1,254 changed
nontransparent pixels in the rectangle's region, and the corrected regression
helper reports that result.

On the current tree, Rust 1.90 formatting, strict workspace Clippy, workspace
build, Windows GNU workspace check/Clippy, **324 workspace tests**, and the
required llvmpipe/OpenGL GPU smoke pass. The full Linux Xorg/XTest workflow
cannot run in this container because Xvfb is absent and the system package
mirror denies access; the QuickShape interaction and screenshot review are
therefore awaiting the hosted Linux CI run. No physical Windows QuickShape or
new S Pen behavior is claimed. The new commit's three-platform Actions run and
canvas/stylus artifacts must be checked after push.

### Continue here

1. Confirm the Actions Linux artwork workflow completes below its 10-minute
   limit, inspect its QuickShape screenshot and verify both Windows artifacts.
2. Design bounded editable raster-mask storage and an explicit v1-compatible
   migration before implementing mask painting/compositing/undo.
3. Keep Phase 10 vector/text and Phase 11 timelapse/animation active; do not
   defer them until every Phase 8.3 visual item is closed.

## Current continuation — editable raster layer masks (local validation)

The next Phase 9 vertical slice adds sparse, editable grayscale masks to raster
layers. Mask strokes use the existing routed transaction lifecycle and support
preview, cancellation, commit, undo/redo, selection clipping and eraser-as-reveal.
The real Layers Actions menu exposes add/edit/enable/invert/clear/remove actions.
The mask is applied consistently by canvas compositing, full-resolution export,
Home thumbnails, selection edits, and save/reopen. The `.rpl` writer preserves
`RPLALP01` for documents without masks and writes a documented `RPLALP02` mask
tile section only when needed; v1 continues to decode. Mask tiles share the
existing 8192-entry raster/provisional memory limit. OpenRaster output applies
the mask to layer RGBA and clearly reports that mask editability is flattened.

The GUI automation now creates and edits a mask through the application, verifies
that raster tile bytes are unchanged, checks exact undo/redo, and checks export
disclosure. Its parser reads both native versions. Local Rust 1.90 formatting,
strict workspace Clippy, **334 workspace tests**, workspace build, Windows GNU
workspace check/Clippy, Python syntax, and llvmpipe/OpenGL GPU smoke pass. The
complete Xorg/XTest application workflow cannot run locally because Xvfb is not
installed and the package mirror is unavailable. Actions run `40754643332` for
QuickShape failed at the line pixel-difference assertion; the check now includes
the brush-radius raster coverage and reports its measured changed-pixel region.
The next run, `40759189142`, then failed earlier at the empty-document regression:
the v2 parser edit accidentally changed the helper from a valid raster-tile count
to a boolean. The helper now returns the original count for valid files and
`None` only for malformed data. A new hosted run is required; neither the mask
GUI test nor the QuickShape fix has completed in hosted CI yet. Run
`40760547539` passed the parser regression and earlier artwork checks, then
timed out opening the selected layer's Actions menu because the button was partly
below the scrolled Layers viewport. The XTest helper now scrolls this button fully
into view before clicking. The application stayed running; this is an automation
geometry correction before rerunning the same mask workflow.
No new physical Windows startup, S Pen, Narrator/NVDA, or real-device mask use was
performed. Existing basic S Pen drawing observations remain valid; advanced
pressure/tilt and mask workflows need separate hardware validation.

### Continue from this checkpoint

1. Push and inspect the hosted three-platform run; require the full Linux artwork
   workflow, the editable-mask interaction, and both Windows executable artifacts
   before marking this checkpoint accepted.
2. If the QuickShape line assertion remains flaky, diagnose its saved document
   region and active selection state; preserve a real pixel-change assertion.
3. Continue Phase 9 with the highest-value remaining interaction (barrel-button
   assignment/configuration or other unfinished approved editing workflow), then
   begin a Phase 10 editable vector path slice only after native format and
   interoperability boundaries are designed and tested. Phase 11 animation and
   timelapse remain unimplemented.

## Mask workflow follow-up — direct selected-layer controls (2026-10-29)

Actions run [`40762059402`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40762059402)
completed Windows and macOS successfully. The Linux artwork workflow saved an
`RPLALP02` project after adding a white mask, then timed out after the nested
`Layer Actions → Layer mask → Edit mask with Brush` interaction. The controls
were moved to Selected layer settings in commit `8cfc990`, removing the nested
menu from the workflow.

The full set of mask operations now lives in the selected layer's expandable
settings. This removes the nested popup level and exposes direct, named controls
for Add, Edit, Enable/Disable, Invert, Clear, and Remove while keeping the
ordinary row Actions menu compact. XTest now clicks those rendered controls,
reopens the saved v2 document through the application's file picker, verifies
each operation in serialized state, and walks exact Undo/Redo bytes. It verifies
that disable/enable retains mask and raster tiles, invert preserves mask samples,
clear returns samples to white, and removing the last mask safely writes v1;
undo restores the v2 state. A document regression additionally checks
selection-clipped mask paint, rendered alpha compositing, and unchanged raster
tiles.

The follow-up Actions run [`40801195199`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40801195199)
also failed in Linux artwork smoke. A local reproduction on the available Xorg
display exposed a test assertion mismatch rather than a failed UI action: the
application logged `UI_TEST_LAYER_MASK_EDIT_CLICKED: 4 true` and entered mask
mode, while Python compared the value to `4 True`. Rust's boolean formatting is
lowercase. The interaction is additionally contextual: Edit mask opens Brush
Library, so the test now dismisses that panel with Escape, confirms the same mask
target remains active, and only then sends the real canvas dab. The v1 removal
assertion now accepts the documented omission of the optional `mask` field.

The complete Linux/Xorg/XTest artwork workflow has now passed locally on the
existing display `:102` with llvmpipe/OpenGL. It exercised all six mask controls,
painted mask samples through the app, confirmed source raster tiles remained
unchanged, saved/reopened v2, verified exact Undo/Redo and the v1 transition,
then completed the existing shape/gradient, OpenRaster, export and recovery
workflow. On 2026-10-29, the final local rerun also passed formatting, strict
Clippy, all **336 workspace tests**, workspace build, Windows GNU target check
and Clippy, the repository test check, and the required llvmpipe GPU smoke. The
workflow command was `env DISPLAY=:102 WGPU_BACKEND=gl
LIBGL_ALWAYS_SOFTWARE=1 XDG_RUNTIME_DIR=/tmp
RPL_UI_SCREENSHOT_DIR=/tmp/bp-round1-final-screenshots
python3 scripts/alpha-workflow-smoke.py`; it passed and generated 59 PNG
screenshots. I inspected the mask-editing, painted-mask, and populated-recovery
screenshots. The mask-editing state intentionally opens Brush Library; the test
closes it with Escape before painting, while confirming the mask target remains
active. Hosted Linux and all-platform CI must still be rerun before Round 1 is
accepted. This was software-rendered Xorg testing, not physical Windows mask or
S Pen validation; Narrator/NVDA and power-loss behavior also remain unverified.

### Next exact steps

1. Commit and push the corrected boolean expectations, Brush Library dismissal,
   and v1 optional-field assertion; rerun hosted Windows, Linux and macOS CI.
2. Require the complete Linux artwork workflow, mask save/reopen, all six UI
   operations, OpenRaster flattening disclosure, and both Windows application
   artifacts before accepting Round 1.
3. Only after Round 1 passes, begin the bounded Phase 10 vector foundation:
   mixed raster/vector layer representation, stable path IDs, RPL v3 migration
   with v1/v2 reads, safe path validation, editable path save/reopen and
   transactional undo/redo. Continue Round 3/4 increments after that vertical
   slice is verified; do not declare any phase complete.

## Phase 9 validation closure and Phase 10 vector-path checkpoint (2026-10-29)

The Phase 9 editable-mask checkpoint was accepted by GitHub Actions run
[`40805980434`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40805980434).
That accepted base remains intact; this continuation adds the first Phase 10
editable-vector slice rather than replacing the raster document model.

Raster layers now optionally carry bounded editable vector paths. A path stores a
stable ID, line and cubic segments, visibility, sRGB stroke color and document
pixel width. The model validates finite geometry and memory/work limits, supports
path hit testing and translation, shares native transactions and layer order,
and preserves monotonic IDs across delete, duplicate, save and reopen. `.rpl`
uses `RPLALP03` only when vector state requires it and keeps legacy v1/v2 projects
readable; masks remain v2 when no vectors are present. Native saves, backups,
journals and recovery snapshots include editable geometry. Rendering composites
vector strokes over raster pixels within each layer before its mask, blend mode
and opacity. PNG and OpenRaster export derive pixels from vectors; those formats
do not retain path editability.

The canvas V tool creates connected line paths, selects and moves complete paths,
and deletes a selected path as one undoable transaction. It exercises the real
input router, previews without writing committed raster tiles, and includes the
path's final pointer sample. Tests cover model bounds, cubic flattening metadata,
selection/movement/deletion, exact Undo/Redo, duplication IDs, compatibility,
v3 corruption and resource limits, journal recovery, thumbnails, and PNG/ORA
flattening. Cubic paths are accepted by the document model but the UI currently
creates polylines only; node/handle editing, path fill, transformations beyond
whole-path translation, and vector style editing remain future work.

Verification on this checkpoint: Rust 1.90 formatting and strict Clippy pass;
the full workspace suite passes **352 tests**; workspace build and Windows GNU
target check/Clippy pass; and the required llvmpipe/OpenGL offscreen GPU smoke
passes. The complete Linux/Xorg/XTest application workflow passed on vector
commit `4a5a808`, including
existing painting, masks, save/reopen and exports; live vector create/move/delete,
save/reopen and OpenRaster flattening; plus forced process termination after a
durably journaled recovery checkpoint with its paired snapshot removed. Recovery
preserves raster and mask tile bytes exactly and vector/document metadata to
`1e-12` for f64 JSON round-trip drift. The workflow produced 61 populated PNG
screenshots. I inspected the vector canvas, wide Home, narrow Layers/Reference,
and narrow dark recovery states. The screenshot confirms the vector is visible
and selected; the compact Layers/Reference pair still leaves too little canvas
area at 640 × 480 and remains a known workspace limitation.

The local UI run used Xorg display `:102` with Mesa llvmpipe/OpenGL because this
environment has no `xvfb-run`; this is software-rendered Linux automation, not a
physical-GPU test. Windows S Pen, Narrator/NVDA, macOS runtime, and power-loss
behavior were not physically tested in this continuation. The 15-minute hosted
Linux artwork-workflow timeout provides margin above the previously measured
Phase 9 run. Cross-platform Actions for the new commit must pass before calling
the Phase 10 checkpoint hosted-validated.

A post-commit performance review found that Home and Layer thumbnails sampled
editable paths per output pixel. Both now reuse bounded source-tile rows, and the
tile rasterizer skips paths whose conservative cubic-control bounds do not
intersect the current tile. A dense fixture with 8,192 line segments completes
the document thumbnail test in about 0.6 seconds and the 96 × 64 layer preview
in about 0.5 seconds in this local debug build. These deterministic regression
workloads exercise the maximum accepted segment count; timings are observations,
not hardware performance claims. The full Xorg workflow was not rerun after this
thumbnail-only follow-up; the updated hosted workflow remains pending.

### Continue from this checkpoint

1. Push the verified vector slice and require the current Windows, Linux and
   macOS Actions jobs, including separate Windows canvas and stylus artifacts.
2. Keep the native v1/v2 fixtures and v3 recovery tests while implementing
   vector style editing and explicit path selection affordances, then add
   keyboard-accessible node/handle editing with bounded geometry and undo.
3. Extend the live application workflow to assert path style changes and more
   mixed raster/vector operations. Preserve the known narrow-window panel issue
   for a separate coordinated-workspace improvement; do not expand vector scope
   into text or animation until the editable path workflow is dependable.

### Latest validation follow-up — XTest fresh-event synchronization (2026-10-29)

Actions run [`40817631458`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40817631458)
passed Windows, Linux and macOS for vector commit `4a5a808`, producing the
separate Windows canvas and stylus diagnostic artifacts. Run
[`40818285789`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40818285789)
for thumbnail optimization commit `d357580` failed in the Linux artwork test.
Local reproduction identified stale event-marker waits in the XTest harness:
after Undo/Redo it could accept an earlier selection/transform result, and a
repeated mask toggle could accept the previous toggle before the new click had
been processed. No artwork or vector rendering defect was observed in those
reproductions.

The harness now requires new selection, handle, transform-start, cancellation,
and mask-state events by checking event counts before each input. It retains the
original saved-byte, native metadata, dirty-state, and exact Undo/Redo
assertions. On the thumbnail-optimized source, Python syntax validation and the
complete Xorg/XTest workflow now pass locally with llvmpipe/OpenGL. The workflow
created 61 screenshots and exercised painting, masks, vector create/move/delete,
save/reopen, PNG/OpenRaster, and journal-based forced-process-termination
recovery. Representative vector and populated recovery screenshots were
inspected. This correction changes test synchronization only; no Rust runtime
behavior changed. A fresh three-platform Actions result for the harness fix is
required before the latest branch checkpoint is hosted-validated.

The latest Rust code remains covered by the 352-test workspace run, strict
Clippy, workspace build, Windows GNU check/Clippy, and llvmpipe GPU smoke recorded
above. Continue with vector style editing and selection affordances before
bounded node/handle editing; Phase 8.3, Phase 9 owner hardware validation, Phase
10 and Phase 11 remain active.

### Mask workflow clip correction and editable path styling — local checkpoint

Actions run [`40820933426`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40820933426)
failed during mask removal because the Python helper estimated the Layers
viewport from the outer window frame and clicked below the actual clipped
ScrollArea. The app now logs the live `ui.clip_rect()` only in UI test mode, and
the XTest helper scrolls and clicks against that actual rectangle. This fixes
the targeting defect rather than extending event timeouts.

The same verified integration slice adds editable vector stroke styling. The
document engine changes RGBA stroke and width as one undoable transaction while
preserving path geometry, raster/mask tiles and v3 persistence. A selected path
exposes a width slider and an Apply color action that uses the current brush
color; width is committed once when a drag ends. Thumbnail fingerprints refresh
after the style edit. API tests cover alpha values, raster output, native encode,
no-op history, undo/redo, invalid widths and locked/hidden owners. Alpha is
supported by the document API but is not exposed as a UI control in this slice;
the UI should not claim opacity editing yet.

Rust 1.90 formatting, strict host Clippy, all **354 workspace tests**, workspace
build, Windows GNU check/Clippy, and the offscreen GPU pixel smoke pass. The
complete Linux Xorg/XTest workflow now passes locally with Mesa llvmpipe/OpenGL.
It exercised all Phase 9 mask actions, painting and selection behavior, native
save/reopen, exact Undo/Redo, PNG/OpenRaster, recovery after journal-durable
process termination, and editable vector create/reopen/width-edit/move/delete.
The workflow generated populated Home, New Artwork, workspace, Brush, Color,
Layers, Reference, Recovery, mask, shape, gradient and vector screenshots; the
vector path and narrow populated Layers/Reference states were inspected. The
narrow screenshot still shows both panels occupying nearly all canvas width,
which remains a workspace usability issue. llvmpipe/Xorg is not physical GPU,
Windows/macOS GUI, S Pen, Narrator/NVDA or power-loss validation.

Cross-platform Actions for the current source changes remain pending. Preserve
the v1/v2 mask and v1/v2/v3 vector compatibility checks. The next Phase 10 slice
is keyboard-accessible node selection and bounded anchor movement with a single
undoable transaction; curve handles, path fill and broader object transforms
remain later steps. Phase 8.3, Phase 9 hardware validation, Phase 10 and Phase 11
are still active.

### Vector anchor editing and integrated validation — local checkpoint (2026-10-29)

The V tool now enters anchor-edit mode for a selected path. Artists can drag
an anchor on-canvas or choose it from an accessible `Anchor N` menu and edit
document-space X/Y coordinates with an explicit Apply action. Drag movement is
previewed separately, commits once on release, and is discarded on Escape or
window interruption. Moving an anchor translates adjacent cubic handles with
it so their tangent offsets remain intact. The engine transaction rejects
non-finite/out-of-document coordinates and locked or hidden owner layers
atomically. It retains native RPL v3 geometry, updates derived layer pixels and
thumbnail fingerprints, and supports exact Undo/Redo. No format-version change
was needed; v1/v2 compatibility remains unchanged.

The Xorg/XTest workflow verified the real anchor drag, the 18 px to 360 px
vector-width slider gesture, path save/reopen, thumbnail invalidation, unchanged
raster and mask tile bytes, and exact Undo/Redo. To avoid XTest collapsing a
press and release into one egui frame, the slider helper now waits for the app
to acknowledge held-pointer frames. The complete Linux artwork workflow passed
on display `:102` with Mesa llvmpipe/OpenGL. It also passed existing mask,
painting, shape, gradient, recovery, PNG and OpenRaster checks. OpenRaster
export of the populated 1800 × 2400 document with a 360 px path took 12.444 s in
the UI run and 11.993 s in a standalone debug-build measurement; this is a local
software-rendering/debug measurement, not a hardware benchmark. The 9.669 s
export was the earlier raster/mask fixture without the vector path. The
screenshot suite was inspected, including the vector anchor editor, Home, New
Artwork, and populated narrow Layers/Reference state.

Rust 1.90 formatting, strict host and Windows GNU Clippy, workspace build,
Windows GNU check, explicit llvmpipe GPU smoke and all **356 workspace tests**
pass locally. The newest full Xorg/XTest workflow passed. The current code has
not yet run through hosted Actions; run `40831057143` validated its preceding
parent commit only. The latest local log and screenshots were generated by
`scripts/alpha-workflow-smoke.py` and are temporary, not committed artifacts.

Phase 9 mask functionality remains accepted by its earlier cross-platform run;
this update does not claim new hosted or physical Windows validation. Phase 10
now supports creating, moving, deleting, styling, and moving individual path
anchors, with coordinate-field access and reversible transactions. At the time
of this anchor-movement checkpoint, insertion/deletion was the next planned
slice; it is implemented in the following checkpoint below. The editor still
creates polylines only: independent Bézier-handle editing, fill, scale/rotation,
duplication, and SVG interchange are incomplete. The 640 × 480 workspace remains
crowded when both Layers and Reference are open.

### Vector anchor insertion/deletion — local checkpoint (2026-10-29)

The node editor's `Anchor N` menu now inserts a point at the exact midpoint of
the following segment and deletes supported points. Lines split into two exact
line segments; the document API also uses De Casteljau to split cubic segments.
Deleting an endpoint removes its adjacent segment. Interior deletion is offered
only between line segments, where the exact geometry can be retained; unsupported
cubic joins remain disabled instead of approximated. The document engine treats
each insert/delete as one history transaction and preserves the existing RPL v3
layout and v1/v2 reading compatibility. Tests cover native save/reopen, pixels,
undo/redo, cubic subdivision error, operation limits, and atomic refusal for
invalid indices and locked layers.

The Linux XTest scenario now drives the actual anchor menu to insert and delete a
midpoint, then checks saved geometry, unchanged raster and mask tile tables,
thumbnail invalidation, and exact Undo/Redo. It adds a populated
`vector-node-editor-with-inserted-midpoint` GPU screenshot. The full updated Xorg
workflow passed locally on display `:102` with Mesa llvmpipe/OpenGL and generated
63 screenshots; I inspected the new midpoint editor capture. This environment
has no `Xvfb`/`xvfb-run` executable, but the existing Xorg server supported the
real XTest pointer workflow. Run
[`40842835450`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40842835450)
has now passed the Windows, Linux, and macOS jobs for parent commit `8a0b5a0`
and produced the Windows canvas and stylus artifacts. This anchor-insertion
follow-up must receive its own hosted matrix after push.

Rust 1.90 formatting, strict host and Windows GNU Clippy, workspace build,
Windows GNU check, all **359 workspace tests**, and explicit llvmpipe GPU smoke
pass locally. The broader recent Xorg workflow passed before this new menu
scenario; the latest full Xorg/XTest run also verifies insertion/deletion via the
actual menu. This remains llvmpipe/software rendering and does not validate
physical pen behavior. Phase 10 remains active. Next, push this isolated slice,
require the updated Linux scenario and cross-platform matrix to pass, then
continue with authored cubic curves and independent handle editing while
preserving v3 compatibility.

### Cubic control-handle editing and full integration regression — local checkpoint (2026-10-29)

Selected RPL v3 paths that already contain cubic segments now expose their
incoming and outgoing control handles in the canvas editor. Artists can drag a
handle, or select it and enter X/Y document coordinates through named
keyboard-accessible fields. Preview remains provisional until Apply/release;
Escape, focus loss, resize/DPI changes, tool changes, and replay-mode changes
discard the active preview. The engine transaction changes one control point,
preserves both anchors and the other control point, rejects invalid coordinates
and locked/hidden owner layers atomically, and participates in native save,
Undo, and Redo. The RPL v3 schema is unchanged, and v1/v2 compatibility is
preserved.

The full Linux Xorg/XTest artwork workflow passed on display `:102` using Mesa
llvmpipe/OpenGL. In addition to existing mask, painting, export and
journal-durable process-kill coverage, it created, saved, reopened, selected,
edited, and reverted an editable cubic path through real UI interactions. It
checked anchor and handle edits, midpoint insertion/deletion, width changes,
thumbnail invalidation, exact native undo/redo, and unchanged raster/mask tile
tables. The Color Mixer save/undo/redo regression exposed floating-point
coordinate drift on repeated native reads; enabling serde_json's
`float_roundtrip` parser feature removed that metadata drift. An eight-cycle
native-byte/geometry regression and a live UI save/undo/redo reproduction now
pass. This changes numeric decoding only; no file schema changed.

Rust 1.90 formatting, strict host Clippy, all **364 workspace test cases**,
workspace build, Windows GNU target check and strict target Clippy, and required
software-GPU smoke pass locally. The complete Xorg/XTest workflow passed and
generated 64 PNG captures. I inspected the cubic-handle editor, mask editing,
light Home with real project previews, populated narrow recovery, and the
populated narrow Layers/Reference state. The narrow workspace remains crowded
when several large panels are open. OpenRaster export with dense cubic geometry
remains expensive in this debug/llvmpipe setup; no physical-hardware performance
claim is made. GitHub Actions for these uncommitted changes are pending. Windows
GNU checks do not establish Windows GUI startup or S Pen behavior, and macOS
startup is not locally exercised.

Phase 9 mask behavior remains accepted by its earlier Windows/Linux/macOS
workflow; this local rerun also re-exercised it. Phase 10 remains in progress:
the path tool still creates polylines, and interactive cubic creation, vector
fill, object transforms/duplication, SVG interoperability, Narrator/NVDA
usability, and physical S Pen validation remain incomplete. Continue with an
interactive cubic-creation workflow and native transaction tests, then evaluate
vector object transforms without changing v3 until a concrete schema need is
demonstrated. Do not declare Phase 8.3, Phase 9 hardware validation, or Phase 10
complete.

### Phase 10 continuation — convert line segments to editable curves (2026-10-29)

The vector node menu now converts an adjacent straight segment into cubic
Bézier geometry using collinear one-third/two-third control points. The initial
render remains the same straight line; the outgoing and incoming handles can
then be edited independently. At the final anchor the menu targets the
preceding line; at other anchors it prefers the outgoing line. Locked, hidden,
invalid, and non-line segments remain rejected by the document API. This keeps
the native v3 schema and v1/v2 read compatibility unchanged. Two document tests
cover exact geometry/pixels, serialization, one-step undo/redo, limits, and
atomic refusal.

Local Rust 1.90 formatting, strict host and Windows GNU Clippy, workspace build,
Windows GNU target check, 366 workspace tests, and required llvmpipe GPU smoke
pass. The full Linux Xorg/XTest artwork workflow now exits successfully. It
exercised the new menu conversion, native save/reopen, undo/redo, and cubic-handle
editing, and re-ran masks, painting, gradients, shapes, selection, recovery,
PNG/OpenRaster, and exact mixer undo/redo. The earlier mixer discrepancy was
caused by least-significant floating-point coordinate drift during repeated
native JSON decoding; enabling serde_json `float_roundtrip` fixed the exact
round-trip without changing the project schema. The workflow retained 64 PNG
captures; I inspected `vector-cubic-handles.png`, the populated narrow
Layers/Reference view, and the populated light Home view. Both the path editor
and narrow panel state remain functional, though the latter leaves little canvas
visible when both panels are open.

Actions run
[`40867515476`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40867515476)
passed Windows and macOS but timed out the Linux alpha artwork workflow at its
former 15-minute cap. CI now allows 25 minutes with all assertions retained. A
hosted run for this commit passed all Windows, Linux, and macOS jobs:
[`40887124571`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40887124571).
It produced Windows canvas/diagnostic, Linux x64, macOS ARM64, UI smoke, and
Linux UI diagnostics artifacts. The local Xorg/llvmpipe results and CI do not
validate Windows GUI startup, macOS startup, physical S Pen pressure/tilt or
palm behavior, Narrator/NVDA, or power-loss durability. Phase 9 and Phase 10
remain active. Next, implement connected cubic-path creation and continue with
vector fill and object transforms.

### Linux anchor-editor workflow regression — verified test synchronization (2026-10-30)

Hosted Actions run
[`41094100891`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41094100891)
passed its Windows and macOS jobs but timed out in the Linux alpha artwork
workflow while waiting for the reopened cubic path to enter anchor-edit mode.
The failing step followed conversion, save/reopen, path selection, and the
visible **Edit anchors** action; the old automation issued an unframed click,
then used `H` as a fallback, and only observed the historical editing-event
marker. The supplied traceback ended with that assertion still observing false.

The regression instrumentation now waits for the edit button to be rendered
while the primary pointer is held, waits through its release/redraw, and checks
the actual selected-path editor state plus a fresh rendered node-handle marker.
The original `UI_TEST_VECTOR_NODE_EDITING` assertion remains intact. A
test-only transition marker reports the selected path ID and real editor state
when the Linux UI-test mode is active. The new assertion therefore checks the
real editor and geometry rather than weakening the previous assertion to a
button click. The existing `H` fallback remains an actual, documented app
shortcut.

In the updated local run, the visible control did not activate under XTest even
after the held-frame barrier; the `H` fallback did enter editing for the same
selected path, emitted the original editing event, and rendered its anchor
handle. The additional state observation confirmed the transition. This
supports an XTest click/timing or event-observation issue rather than a failure
in the keyboard route or editor rendering. Do not claim that the pointer-click
path was validated; investigating that path remains useful follow-up work.

The same change replaces repeated whole-file log reads in the long UI workflow
with a persistent incremental index. On a preserved 18,093,773-byte log with
265,835 lines, 1,000 repeated marker/prefix queries completed in 0.246 seconds
at 88,576 KiB peak RSS. Two Python tests cover partial-line append, cached
marker/prefix/text counts, stable values, and truncation. CI runs these tests on
all three operating systems.

Local verification for this checkpoint: Rust 1.90 formatting, strict workspace
Clippy, all 394 workspace test cases, workspace build, Windows MSVC and GNU
target workspace checks, strict Windows GNU Clippy, Python smoke-helper tests,
and the required llvmpipe GPU smoke pass. The updated full Linux Xorg/XTest
artwork workflow completed successfully with 78 PNG captures under a
temporarily extracted Xvfb and llvmpipe; it exercised the complete drawing,
vector, save/reopen, export, recovery, and screenshot paths. I inspected the
`vector-node-editor.png` capture, which visibly shows the selected anchor and
handles. Hosted run
[`41102591526`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41102591526)
passed Windows, Linux, and macOS for commit `03790ef`. This does not test
physical stylus behavior or Windows/macOS GUI startup. The full local smoke's
Python process reached about 1.37 GiB RSS late in its run; the independent
incremental log-index benchmark stayed below 87 MiB. Continue to monitor that
test-process memory separately from application memory. Phase 8.3, Phase 9
hardware checks, Phase 10, and Phase 11 remain active.

### Canvas-first Inspector corrections — Milestone 1, Layers hierarchy (2026-10-30)

The Layers Inspector now keeps raster thumbnail selection, editable-mask
selection, and editable vector-path children as separate targets. Each raster
layer row exposes 44-point visibility, lock, and Actions controls; path children
have independent visibility targets backed by the document's existing
`VectorPath.visible` property. That property is now changed through one
undoable transaction and is covered across rendering, Undo/Redo, native save and
reopen, and raster/mask tile isolation. Layer and grayscale mask thumbnails
are bounded and cached; painting, mask changes, vector edits, history movement,
and native reopen update the appropriate preview. Long labels remain fully named
for accessibility while their displayed text is clamped to the row width.

For roomier Layers views, the stack has its own scroll viewport and the selected
layer's rename, opacity, blend, and mask Edit/Stop controls remain in the footer.
At a 640 × 480 compact drawer, the available panel height is too short for the
minimum 44-point controls plus a separate stack and footer. The tab therefore
uses one scroll region there rather than trapping input in nested scrollbars;
the selected properties and mask Stop action remain reachable by scrolling.
The first-run compact width is 256 points and the measured open canvas strip in
the populated narrow screenshot is about 309.5 points. The window workflow now
checks the actual Inspector row against its own clip bounds rather than mixing
geometry from a simultaneously rendered detached Layers surface.

On Rust 1.90, formatting, strict workspace Clippy, workspace build/tests (398
registered tests), Windows GNU workspace check and strict Clippy pass. The
required offscreen GPU smoke passed on llvmpipe/OpenGL. The complete Linux
Xorg/XTest artwork workflow passed locally with 79 captures; it exercised
visibility, locking, context-menu reordering, thumbnail updates after painting,
shapes, gradients, vector child selection, masks and their Edit/Stop controls,
save/reopen, exports, and Undo/Redo. I inspected populated 640 × 480,
1280 × 800, and 1920 × 1080 Layers captures in light and dark themes, as well as
the mask-actions menu and selected vector-child row. The new compact state is
usable by scrolling but is not yet the six-layout/scaled visual acceptance.
GitHub Actions for this uncommitted checkpoint are pending. No `.rpl` schema,
input adapter, or stylus behavior changed. Continue only after the milestone's
Windows/Linux/macOS matrix is green; next is the approved scrollable Inspector
tab strip and vector editing integration, including the direct-pointer anchor
editor regression without the `H` fallback.
