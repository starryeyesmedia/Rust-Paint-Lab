# Rust Paint Lab engineering roadmap

This is a working engineering sequence, not a release promise. Phase 8.3
workspace/accessibility work continues alongside Phase 9's raster painting and
editing tools; unresolved visual polish does not block useful creative features.
Artwork preservation and cross-platform validation apply to every track.

## First physical alpha QA corrective sequence — active

Product-owner testing of the Windows build from PR #8 found 13 concrete issues.
The approved sequence is to secure close-time saves and periodic recovery first,
then fix Smudge/Color Mixer, menu layout, direct Brush Library access, and
Eyedropper feedback. Only after those reliability and painting issues are
addressed should the toolbar lose grouped tools and the visual refinements
proceed. The approved correction gives every implemented tool a direct button,
restores direct References access, and removes the unfinished Create Path tool
from the primary rail while preserving editable vector documents. Brush
Library expansion is limited to meaningful behavior already supported by the
engine. A separate vector freehand-brush specification follows these fixes;
implementation awaits owner approval. Do not alter or merge the separate GPLv3
draft PRs.

Current starting baseline: `main` at
`1559122943e8b6e9123b653a555f72e7a894d3e6`. The first active corrective PR is
`fix/close-time-draft-preservation`. Its initial slice exposes the exact close
save error and adds application-level Linux close/recovery coverage; it does
not yet identify the owner's Windows filesystem failure. Product-owner
acceptance is required before merging artist-facing corrections. See the
current top section of `docs/PHASE8-STUDIO-PROGRESS.md` for tested commands,
limitations, and continuation steps.

The first hosted Linux close-save smoke exposed synchronization defects in the
test harness (pre-first-frame resize, stale control geometry, fixed drawing
coordinates, path typing, and expected log markers). These are corrected, and
the full close/recovery flow now passes locally on Xorg/llvmpipe with real
project pixel checks. A fresh cross-platform Actions run is still required.
The owner's original Windows cause remains unconfirmed; keep PR #9 draft and
deliver its Windows artifact for owner evaluation before merging or treating
the save issue as resolved.

## Canvas-first design handoff — active implementation

The first approved implementation slice is in progress on
`feat/inspector-responsive-references`, based on verified `main`
`b0a4d7e60ec3c86dc45a1c8f209e64c50fe2495e`. Responsive first-run Inspector
widths now follow the approved effective-viewport table while preserving
explicitly resized preferences across temporary scaling/narrowing. Pinned
references avoid the palette, Inspector, and visible Zoom/Fit strip, and now
magnetically snap on pointer release within 24 logical points; the Linux
artwork workflow asserts this real gesture does not paint. Local formatting,
strict Clippy, all 405 workspace tests (including llvmpipe GPU smoke), workspace
build, and Windows GNU check pass. The Xorg workflow's syntax check passes, but
this local environment has no Xvfb. Actions run
[`41163726555`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41163726555)
passed Windows and macOS; Linux failed the pre-existing 24%-footprint UI
assertion because the panel frame exceeded a newly constrained reference
viewport. The first eight-point frame allowance was insufficient because the
reference's 48-point minimum image plus 44-point controls established a 134-point
outer minimum. A follow-up tightens only title and outer padding, retaining the
44-point controls, useful image and unchanged 24% assertion. Run `41164373970`
confirmed the residual bound issue. The follow-up passes formatting, all 405
workspace tests, strict Clippy, workspace build, llvmpipe/GL GPU smoke, Windows
GNU check, and Python syntax validation. Run `41164917867` passed the footprint
bound but timed out waiting for the pointer drag to reach its snapped position.
The 46fa991 diagnostics confirmed that the Area never saw hover/drag, while its
child title had the explicit drag response. Movement now consumes the title
response's per-frame deltas and snaps on its release event; the unchanged Linux
pointer-level assertion will verify the fix. Test-only response and geometry
diagnostics remain available. Xvfb is unavailable locally, and generated
screenshots have not yet been visually reviewed. The six-layout visual
acceptance and subsequent approved icon, grouped toolbar, and
visual/accessibility stages remain incomplete.

The follow-up hosted run `41166324868` stopped earlier in the Linux workflow at
the existing populated Layers-row containment check: at 1280×800, a row used
the outer scroll-area width and extended into the scrollbar gutter. The row now
uses its clipped content edge for sizing and tighter spacing without shrinking
its 44-point controls; the UI assertion is unchanged. Rust tooling is absent
from the current execution shell, so the source correction still requires the
hosted cross-platform matrix before this milestone can proceed.

Run `41166928479` also found the mask Actions target 0.9 logical points outside
the 360-point Accessibility test viewport. The horizontal mask row now reserves
the leading gap and both action gaps when sizing its status label; the 44-point
controls and accessibility bounds assertion remain unchanged. This adjustment
needs the next hosted run for compilation and confirmation.

Layers, Brush, Edit, Color, and References share the existing egui/winit
Inspector. Layers and masks use real document transactions; references remain
workspace-only and stay out of artwork exports. The accepted Layers hierarchy
checkpoint is at `3427604`.

Milestone 2 is implemented locally on top of that checkpoint. The tab order is
now Layers / Brush / Edit / Color / References in a horizontally scrollable
strip, with the Canvas/close control fixed outside it. The selected-vector
anchor, handle, geometry, and style controls are integrated into Edit while
direct canvas manipulation remains available. A real pointer-only XTest enters
anchor editing and verifies rendered handles without invoking the `H` shortcut.
The full Linux Xorg/XTest artwork workflow passed locally with 80 screenshots;
the compact tab strip scrolled to References and pointer selection succeeded.
Populated narrow Layers/References and vector-edit captures were inspected.
Formatting, strict host Clippy, workspace build, Windows GNU check/Clippy, 400
workspace tests, and required llvmpipe/GL smoke were previously verified for
this Rust state; Python helper tests pass. The M2 hosted matrix is pending until
its implementation commit is pushed. The preceding cross-platform baseline run
[`41102591526`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41102591526)
is not a substitute for M2's own Actions result.

Inspector Milestone 3's responsive widths and reference placement are
implemented, but the latest hosted run exposed a 640×480 compact Layers overflow.
The local correction constrains layer rows to their true ScrollArea viewport and
preserves the 44-point tool targets; targeted UI assertions and local Rust checks
pass. A full Linux artwork run and hosted cross-platform CI for the correction
are still pending. After this milestone passes review and CI, the next focused
PR integrates the approved Material Symbols Rounded assets and compact grouped
toolbar. Inspector Milestone 4 then captures all six viewport/scale cases and
reviews populated dark/light/high-contrast layouts before independent QA. Phase
8.3 remains active.

Phase 10 now includes optional deterministic freehand-to-cubic creation and
undoable closed-path fills in the existing V tool. Smooth paths preserve all
input anchors, join curves with shared tangents, capture the authoring setting
at pointer-down and leave polyline creation as the default. Closed fills use
the explicit `.rpl` v4 marker; the v1/v2/v3 readers remain intact and unfilled
paths keep the byte-compatible v3 schema. Fill pixels appear in compositing,
PNG/OpenRaster exports and hit-testing without becoming raster tiles. Local
verification passes 391 workspace tests, strict host and Windows GNU Clippy,
workspace build/check, llvmpipe/GL GPU smoke, and the full Linux Xorg/XTest
workflow with 77 screenshots. I inspected the `vector-filled-path` capture.
Actions run [`41094100891`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41094100891)
passed Windows and macOS, including Windows GUI startup and artifact packaging,
but failed Ubuntu's alpha artwork workflow at step 11. Its public annotation
truncated the underlying assertion; the workflow now retains a concise final
log excerpt in the job summary for diagnosis on the next run. Software GPU
testing does not validate the owner's Iris Xe, S Pen or native screen-reader
behavior.

Phase 10's vector-only SVG export supports the existing line/cubic paths and
closed fills, preserving path visibility, order, supported colors/alpha, and
layer opacity. A real UI test parses the export, checks native `.rpl` source
integrity, and discloses unsupported content. SVG import and full
raster/mask/document round-trip remain unimplemented. Actions run
[`41094100891`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41094100891)
is historical: Windows and macOS passed while Ubuntu failed on the vector anchor
editing observation. The current local XTest run now passes that real pointer
interaction; the new M2 commit still needs its own cross-platform Actions result.

Do not treat this checkpoint, screenshots or automated checks as Phase 8.3 or
Phase 10 completion.

## Adopted product identity — BitchPixel

- **Product identity:** BitchPixel is the chosen name, not a provisional candidate. Rust Paint Lab continues as the engineering repository, internal package/executable names and format terminology until those identifiers are migrated deliberately, without breaking existing projects or CI.
- **Approved brand lines:** “No limits. No bullshit.” (primary positioning) and “Create without permission.” (creative-freedom message). Keep both exact phrases available in product/design work.
- **Brand meaning:** Reject paywalls, subscriptions, locked creative tools, forced accounts, advertising/tracking, and unnecessary ecosystem/device exclusion. Preserve realistic hardware constraints and do not advertise planned features as shipped.
- **Visual direction:** Design and implementation proceed together in the running application; screenshots inform iterative corrections. Current functional egui-based layouts do **not** meet the approved visual ambition; do not conflate passing CI or screenshot existence with design acceptance. Treat accessible dark/light/high-contrast typography, clear hierarchy, artist-centered artwork presentation and coherent floating tools as acceptance criteria.
- **Open implementation tasks:** design system and accessible visual treatment; name/metadata/application-ID/binary migration plan; trademark/domain due diligence; distribution license. See [product identity issue](https://github.com/starryeyesmedia/Rust-Paint-Lab/issues/1).
- **Scope:** Continue Phase 8.3 reliability and accessibility work; do not make unreviewed breaking package/format/identifier changes or merge the development branch solely for branding.

## Owner evaluation follow-up — Phase 8.3 active

The owner-confirmed hardware baseline is a successful Windows launch and basic
S Pen drawing on the Galaxy Book2 Pro 360. The latest findings prioritize a
console-free Windows GUI package, clearer tool iconography, faster Home-to-canvas
creation, a canvas-first coordinated workspace, configurable S Pen barrel action,
a real raster Smudge tool, and a deliberate Place Image workflow. The GUI
subsystem change and current Home/New Artwork integration are locally verified
through code commit `2979c02`. Local Windows GNU checks pass, but packaged Windows
startup and the post-`2979c02` GitHub Actions results are pending. See
`PHASE8-STUDIO-PROGRESS.md` for exact tests and status. Keep the WM_POINTER and
stabilization paths unchanged absent a reproduced defect.

## Deep development campaign update — Phase 9 started

The reported Linux artwork-workflow timeout was caused by a five-minute step
limit shorter than the measured software-rendered run. Actions run
[`40708740871`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40708740871)
passed all three platform jobs after raising the unchanged full workflow's cap to
ten minutes. Phase 9 now has a document-backed Smudge mode integrated into the
ordinary routed mouse/pen stroke path. It preserves v1 project compatibility,
uses bounded private tiles with cancel/commit/undo semantics, and has a local
full artwork workflow that verifies saved tile changes and exact undo/redo.
The implementation's cross-platform Actions run is pending; Smudge has not been
validated on the owner's physical pen hardware. Continue with color-mixing
painting, configurable pen barrel behavior and transformed image placement.

## Experience and accessibility

- **Phase 8.3 — Studio and accessibility foundation:** unified movable/dockable
  tool palette, contextual brush/layer/reference/color panels, recent-artwork
  previews, visual New Artwork flow, recovery manager, accessible preferences,
  keyboard/focus behavior, and Windows assistive-technology validation.
- **Current engine continuation:** deterministic per-stroke stabilization,
  raster shapes and gradients, plus a transactional layer-scoped Smudge mode
  provide functional editing slices. This is not Phase 8.3 acceptance; continue
  substantive brush/editing development and validate stabilization on hardware.
- Complete theme, scalable interface, touch target, contrast, handedness, reduced
  motion, small-window and high-DPI behavior; add manual Narrator/NVDA testing.
- Phase 9: adaptable workspace for shape/fill/selection workflows; Phase 10:
  vector/text workspace; Phase 11: animation/timelapse workspace; Phase 12:
  mobile/adaptive interfaces.

## Creative engine

- Phase 9 — raster Smudge is implemented with selection/pressure, bounded
  previews, native/PNG roundtrips and undo/redo. A separate Color Mixer mode now
  deposits the chosen brush color into existing layer pixels, with configurable
  mix strength and the existing opacity/flow/pressure controls; persistence
  remains ordinary v1 raster data. Native Windows pen barrel assignments now
  route hold-to-erase/restore, eyedropper or pan without changing the WM_POINTER
  adapter. The current continuation adds PNG/JPEG Place Image with an isolated
  transform preview and one-transaction raster-layer commit; it also optimizes
  OpenRaster layer export to convert sparse tiles directly. Move Selection to
  New Layer is integrated in the real Layer Actions menu, preserves opacity/blend
  and composite pixels, is undoable and uses the v1 project representation.
  The placement preview is now layered below floating controls, not painted over
  them. A real Xorg/XTest workflow changes scale and rotation, drags the image,
  verifies cancel, commits an editable layer, and checks app-level undo/redo byte
  for byte. Locally, all **310 workspace tests**, strict Clippy, formatting,
  workspace build, Windows GNU checks, llvmpipe GPU smoke, the placement workflow,
  and the full Linux artwork/recovery workflow pass. GitHub Actions for the newest
  integration commit is still required to confirm hosted platform jobs. Next:
  selection transforms and QuickShape, then guides/symmetry and brush expansion.
- Phase 10 — editable vector paths and text, mixed raster/vector editing,
  non-destructive effects, and natural-media foundations. The first editable
  path slice now includes bounded line/cubic document geometry, stable IDs,
  v3 persistence with v1/v2 reads, whole-path move/delete, per-anchor move,
  exact segment subdivision, supported anchor removal, undo/redo, live preview,
  and PNG/OpenRaster flattening. The V tool still authors polylines only;
  independent cubic handle editing, path fills/transforms, SVG interchange, and
  richer vector object workflows remain open.
- Phase 11 — frame animation, onion skin, automatic per-action timelapse, and
  interoperability expansion.
- Phase 12 — broader creative workflows and mobile-core reuse.

## Latest Phase 9 integration — 2026-10-28

Place Image now has an Xorg-tested end-to-end flow covering preview visibility,
scale, rotation, movement, cancel-without-mutation, placing into an independent
raster layer, save, and application Undo/Redo. Fixing the preview's egui layer
ordering keeps transform controls above the image. The Move Selection to New
Layer operation is separately implemented and unit-tested for exact compositing,
save/reopen, history and storage limits. The full Linux artifact/artwork workflow
also passed locally with 52 populated screenshots spanning themes, sizes, panels,
painting tools, gradients, layers, references and recovery. These slices advance
Phase 9 without completing the phase. Follow-up work remains selection transforms,
QuickShape, symmetry/guides, brush dynamics, and other unfinished raster tools.

## Phase 9 continuation — editable raster masks

The current continuation adds editable sparse grayscale masks on raster layers.
Mask painting is an ordinary routed brush transaction with provisional preview,
cancel, undo/redo, selection clipping, and pressure/opacity/flow behavior. Layer
mask controls (add, edit, enable, invert, clear, remove) operate on the actual
document. The `.rpl` container writes `RPLALP02` only when masks are present;
unmasked documents remain version 1, and v1 files still decode unchanged. Mask
tiles share the bounded document tile budget. OpenRaster export applies masks to
layer pixels and the UI reports that mask editability is flattened there.

Local verification currently passes 334 workspace tests, strict Clippy,
formatting, workspace build, Windows GNU check/Clippy, and required llvmpipe GPU
smoke. Focused tests cover transaction behavior, corrupt/truncated v2 records,
v1 compatibility, recovery, thumbnails, selection transforms, and OpenRaster.
The Linux Xorg/XTest harness now exercises mask creation, painting, untouched
raster bytes, save/undo/redo, and OpenRaster disclosure, and its native parser
understands v2; this GUI workflow awaits hosted execution because this container
has no Xvfb. Actions run `40754643332` first exposed a QuickShape line-pixel
assertion that now measures the complete brush-radius raster region. Run
`40759189142` exposed an empty-document harness regression; the parser now
returns a valid raster-tile count and uses `None` for malformed input. Run
`40760547539` passed that check and earlier artwork steps, then timed out on a
selected-layer Actions button partly below the scrolled Layers viewport. The
automation now scrolls that control into view before clicking. A new hosted run
must verify these fixes and the mask workflow before acceptance. Phase 8.3,
Phase 9, Phase 10 vector/text, and Phase 11 timelapse/animation remain active.

## Reliability and platform engineering

- Continue bounded document recovery, atomic storage, draft preservation and
  corruption handling alongside every user-facing feature.
- Measure memory, latency, GPU behavior, thumbnail/reference caching and save
  performance on integrated graphics and low-memory configurations.
- Maintain Windows, Linux and macOS desktop builds and automated UI workflows;
  validate Android/iPadOS only when actual platform implementations exist.
- Evolve native format through documented, tested migrations before introducing
  vector, animation, or other persistent content types.

## Current status

### Latest Phase 8.3 snapshot — 2026-10-28

Code commit `2979c02` locally passes formatting, strict Clippy, the workspace
build, **282 workspace tests** (including the llvmpipe/OpenGL offscreen GPU
smoke), Windows GNU workspace check/Clippy, and the complete local Xorg/XTest
artwork workflow. The latter verified save/reopen pixels, layer operations,
shapes, gradients, export/import, reference isolation and forced-crash journal
recovery; it captured 50 screenshots that were locally inspected in representative
light/dark, narrow, populated-panel and high-contrast states. The hidden Reference
wheel crash on the dark narrow New Artwork screen is fixed and covered by that
workflow. GitHub Actions for `2979c02` is pending; the preceding run failed only
because a macOS log-directory assertion assumed a Linux/Windows path, and the
assertion now accounts for macOS `~/Library/Logs/BitchPixel`. Phase 8.3 remains
active. The Layers/Reference pair is bounded and non-overlapping at 640 × 480 but
still leaves too little canvas; side-button configuration, Smudge, transformed
Place Image and safe custom window chrome are incomplete.

Phase 8.2 is implemented through code commit `1ad2b62` with successful
three-platform CI. Phase 8.3 remains active on
`feature/studio-overdrive-phase-8`; the current continuation rebased onto the
latest owner-authored identity decisions at `13f4115` and has advanced through
code commit `1d5f6fb`.
The current implementation includes AccessKit event integration and generated-tree
tests, a movable/dockable palette, brush and color flyouts, an optional floating
Layers panel, visual New Artwork cards, a confirmation-safe recovery manager, bounded
recent-artwork thumbnails, compact reference controls, reduced motion, interface
scaling and responsive Home cards. The floating Layers panel supports real layer
actions with persisted position/size; bounded recovery previews load only on
request after native-file validation. The current visual pass adds responsive
flyouts, illustrated format cards, stronger Home hierarchy, focusable
format/brush selection, and a broad Light/Dark/high-contrast screenshot matrix.
The active visual/functional continuation also adds BitchPixel Home and window
identity, searchable/favorite brushes, a contextual brush strip, richer floating
Color controls, and bounded per-layer previews plus direct reorder and property
controls in the Layers flyout.
Formatting, strict Clippy, workspace build, 200 tests and the software GPU smoke
pass locally. Actions run `40226791860` has
passed the complete Linux UI/artwork workflow: the test clicks the rendered Brush
control by its measured bounds, exercises Ctrl+B and Escape, opens Layers and
Recovery, saves/reopens artwork, and captures the Home, New Artwork, canvas,
reference, Brush, Layers, Recovery, and recent-Home screens. Linux and macOS jobs
passed and uploaded artifacts; Windows formatting, Clippy, tests and workspace
build passed. All three jobs completed successfully; run `40226791860` uploaded
the consolidated `rust-paint-lab-canvas-demo` package and retained the separate
`rust-paint-lab-windows-diagnostic` artifact. Native Narrator/NVDA behavior and
visual review of screenshots remain outstanding; Actions metadata is available,
but artifact blob downloads return HTTP Forbidden in this environment. Phase 8.3
is not complete. The visual integration pass from commit `1220722` now includes
Brush placement/size/scrolling refinements, responsive visual format cards, Home
hierarchy and theme-safe heading contrast, and populated light/dark/high-contrast
UI screenshot coverage. Actions run `40230324720` confirmed
the first eight light-theme normal/narrow screenshots but exposed stale test-only
palette geometry when the smoke clicked after leaving Home; that was corrected.
Run `40230678773` passed three-platform format, lint, tests and build, Linux GPU
and live-window smoke, then captured 17 of 18 requested UI states. Its only failure
was after the dark Recovery capture, before the final high-contrast screenshot.
Runs `40231310053` and `40231796822` repeated it. The latter's concise failure
message showed that the dark New Artwork capture left a fresh unsaved document;
Escape correctly opened the save-on-close path, causing the harness's 20-second
wait to expire. The screenshot color-count hypothesis was incorrect. The smoke
now terminates that screenshot-only process after capture; normal clean-close is
still covered earlier in the same app workflow. Run `40232806078` passed all
Windows, Linux, and macOS checks and packaging. Linux passed the complete
18-screen Light/Dark/high-contrast matrix with real layer and recovery entries.
The consolidated `rust-paint-lab-canvas-demo` and separate
`rust-paint-lab-windows-diagnostic` artifacts are available from that run. The
screenshot archive could not be visually reviewed here because Actions artifact
blob downloads return HTTP 403. Narrator/NVDA testing remains outstanding. See
[`PHASE8-STUDIO-PROGRESS.md`](PHASE8-STUDIO-PROGRESS.md) for current CI evidence,
limitations and exact continuation steps. Phase numbering and boundaries are
provisional.

The latest integration commit `1d5f6fb` adds BitchPixel identity across the
running Home/app title and package guides, favorites/search in Brush Library,
contextual brush controls, richer Color access, and populated sparse-tile
previews plus direct rename/order/opacity/blend/visibility/lock controls in the
floating Layers panel. Actions run
[`40235869737`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40235869737)
passed Windows, Linux and macOS format/lint/tests/build and uploaded the
consolidated `rust-paint-lab-canvas-demo` and preserved diagnostic artifacts.
Linux screenshot-state assertions passed for 20 screens. The signed artifact
blob host denied screenshot and log downloads with HTTP 403, so direct visual
inspection remains outstanding; this is not phase completion.

The approved concept implementation is in code commit `59cace0`: locally bundled
OFL typography, a purple-charcoal Midnight Violet palette, a
persisted left/right/top/bottom/floating tool palette, and an HSV Color workflow
with semantic/keyboard controls. The Home continuation adds a named,
keyboard-accessible filter for recent local artwork and a responsive
one-to-four-column artwork gallery. Formatting, strict Clippy, all 204 workspace
tests, GPU smoke and workspace build pass locally. Actions run
[`40239511344`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40239511344)
passed the concept typography/theme/dock/Color checkpoint on Windows, Linux and
macOS. The Linux UI workflow now adds a 1920 × 1080 Home capture for the
four-column layout; its capture assertion and resulting 22-screen archive await
the latest run [`40240193155`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40240193155).
Screenshot pixels still cannot be downloaded from this environment, so screenshot
presence is not visual approval.
Phase 8.3 remains active. See `PHASE8-STUDIO-PROGRESS.md` for exact status and
continuation steps.

## Latest Phase 8.3 visual continuation — local checkpoint, Actions pending

On 2026-10-25 the branch advanced from `0c1c145` with the temporary text-only
BitchPixel Home identity, exact approved brand lines, wider illustrated
quick-start cards, visual/reflowing New Artwork preset cards, and responsive
Brush panel placement and sizing. First-run Home now places working popular
canvas choices and a custom/print-size action in the open side of its hero;
recent-project Home uses the real thumbnail instead. Recent-artwork and Reference screenshots now
assert pixels from actual painted and imported image fixtures, not only screenshot
dimensions or color diversity. The Xvfb matrix captured and locally reviewed 22
states. All 223 workspace tests passed with required llvmpipe/Vulkan GPU smoke;
formatting, strict Clippy and workspace build passed. Code commits `fcbd6c6` and
`769b9b2` are based on `0c1c145`; the adjacent screenshot/documentation commit
adds image assertions and updates the testing record. Cross-platform Actions
verification for these commits is pending. The recent thumbnail, first-run Home,
visual preset selection and scrollable advanced settings are described in
`PHASE8-STUDIO-PROGRESS.md` and `testing.md`.

The Phase 8.3 design remains in progress. The current Home composition and
floating tools still need refinement against the approved concept; panel docking
and keyboard/focus traversal need further verification. Narrator/NVDA, Windows
UI Automation and physical-device behavior were not tested by this local Linux
run. Do not publish the next Windows evaluation build until the wider pass and
three-platform Actions checks succeed.

The typography continuation replaces the incorrect extra-light/regular static
font instances with verified Manrope 400/600/700 and Space Grotesk 600/700
instances, plus a named cross-screen type scale. This is a working implementation
checkpoint, not Phase 8.3 acceptance; Home artwork presentation, workspace
integration, brush/layer workflows, accessibility review and screenshot inspection
remain active work.

The 2026-10-26 continuation has since added readable recovery entry/source
labels, bounded on-demand artwork previews and stronger visual assertions for
the complete recent-artwork card and full-screen Home background. See the latest
entry in `PHASE8-STUDIO-PROGRESS.md`; local checks are current, and the new
three-platform Actions run remains pending. Phase 8.3 remains active and no
evaluation build is being promoted.

The follow-on local review replaced the missing-font toolbar grip with accessible
vector geometry and corrected top/side palette anchors to start below the
measured contextual-options bar. Formatting, strict Clippy, 227 workspace tests,
the software-GPU smoke and workspace build passed locally. This shell has no
Xvfb, so the updated screenshot workflow has not run here. Actions run
[`40249283516`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40249283516)
passed Windows/macOS and failed Linux only because the screenshot harness chose
the last clipped Home card despite two complete cards being visible; the follow-up
fixes that selection and adds a measured top-palette clearance assertion. Its
artifact blob host rejects downloads with HTTP 403. The new source checkpoint
still needs three-platform Actions confirmation and fresh screenshot inspection.
Continue the Phase 8.3 interface/accessibility work; these polish slices are not
phase acceptance and do not authorize publication of the consolidated Windows
evaluation artifact.

## Latest functional workspace continuation — active

Actions run [`40251981336`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40251981336)
passed Windows, Linux, and macOS for the preceding `c0e5ace` brush-gallery
checkpoint. The current local continuation adds a workspace-only saved-color
palette with bounded persistence and working add/apply/deduplicate/remove actions;
fixes DPI-aware event ownership over the tool palette, floating Brush/Color/
Layers panels, Workspace controls, and Reference boards; and replaces the
optional fixed right inspector with a movable, resizable window whose position
and size are persisted in workspace preferences and toggled with F6. The X11
alpha workflow now exercises real clicks on over-canvas Color and Workspace
controls, including tab changes and Escape dismissal, verifies saved-color
add/apply/deduplicate/remove, and checks both official Home brand-line contrast
ratios. Local formatting, strict Clippy, Windows GNU target check/Clippy, all 232
workspace tests, full workspace build, llvmpipe/OpenGL GPU smoke, Python syntax,
and the real-X11 alpha artwork workflow pass. The 23-state screenshot matrix
ran through its layout assertions, but local ImageMagick window readback is
black; the official Actions artifact download returns HTTP 403. Visual
acceptance remains pending despite passing UI interaction and contrast checks.
Actions for this working-tree continuation are pending its push.
Phase 8.3 is active. Do not publish the consolidated Windows evaluation artifact
until this broader visual review and current three-platform CI complete.

The pushed toolbar/Home follow-up at `c81f3d8` is now verified by Actions run
[`40250590747`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40250590747)
on Windows, Linux and macOS. The current local continuation adds raster-engine
samples for every built-in Brush Library card and an accessible, settings-aware
sample for the selected brush. Local validation passes 232 Rust tests, strict
Clippy, formatting, the workspace build, llvmpipe/OpenGL GPU smoke and the
real-X11 alpha artwork workflow. This change is not yet pushed or covered by
Actions. Its new brush screenshots have not been visually inspected because
local screenshot capture returned black pixels and the Actions artifact host is
blocked with HTTP 403. Phase 8.3 remains active; no Windows evaluation build is
being promoted from this checkpoint.

The follow-up `11fc732` makes per-row duplicate/reorder/delete undoable and
three-platform verification is running. The current local continuation adds a
dedicated pointer drag grip/drop row; XTest verifies moving the front layer to
the back and restoring the exact layer-ID sequence with Undo. An AccessKit test
checks that keyboard users are directed to the row Actions menu. Local formatting,
strict Clippy, 236 workspace tests, GPU smoke, workspace build, Windows GNU
check/Clippy and full X11 artwork workflow pass. Actions
[`40256352269`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40256352269)
passed macOS but its Linux alpha workflow failed before this drag change; the
Windows job was still in progress when recorded. Failed logs are unavailable via
the artifact receiver's HTTP 403 response. Re-run/inspect current CI results and
keep Phase 8.3 active. Screenshot pixels still lack human review here, and the
consolidated Windows evaluation artifact remains withheld.

The latest local continuation gives each Layers row an accessible **Actions**
menu while keeping right-click context menus. Duplicate, reorder and delete target
the clicked layer ID and use the document history transaction path. X11/XTest
checks each operation and its Undo result, then continues through the existing
artwork/recovery workflow. The semantic-tree test checks the named action button.
Local strict Clippy, all 236 workspace tests, GPU smoke, workspace build, Windows
GNU check/Clippy and the full X11 alpha workflow pass. Actions run
[`40254869930`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40254869930)
passed Windows/macOS but the earlier Linux alpha artwork step failed; the log
receiver returned HTTP 403, and this run predates the targeted layer-menu test.
Fresh three-platform verification is required after the current commit. Local
human screenshot inspection remains pending because `xwd` is unavailable and an
FFmpeg/XCB experiment cannot read this X server's shared-memory window pixels;
the experiment was discarded. Keep Phase 8.3 active and withhold the consolidated
Windows evaluation artifact until the broader redesign is materially complete.

## Latest local visual/accessibility iteration — 2026-10-26 (Phase 8.3 active)

The current working tree extends `11fc732` with a corrected active-widget
foreground contrast rule and a narrow-window Brush Library layout. At 640 × 480,
the panel places search and favorites before its real preset previews, displays
at least two complete cards without scrolling, stays inside the viewport and
clears the primary palette. The live raster-engine sample remains available
below the gallery. Desktop layout and stored panel preferences remain intact.

The screenshot path now uses an explicit, bounded wgpu offscreen copy for test
captures. The full Linux/XTest artwork workflow passes with 29 required UI states
and 30 rendered captures. Representative light/dark/narrow canvas, Brush, Color,
Layers, Reference, Recovery, Home, populated-artwork and high-contrast images
were visually inspected locally. Pixel assertions also check real cached artwork,
reference, brush and recovery content. Local checks pass: formatting, strict
workspace Clippy, 239 workspace tests, software GPU smoke, workspace build,
Windows GNU target check/Clippy, and full mouse/save/export/recovery smoke.

Actions run [`40256352269`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40256352269)
is for base commit `11fc732`: Windows and macOS passed, while Linux failed the
alpha-artwork step. Its artifact/results log host responds HTTP 403. A fresh
three-platform run remains pending for the current uncommitted changes. This
local visual review does not complete the broader redesign: canvas/reference
placement at narrow sizes, Home/New Artwork hierarchy, and keyboard/focus and
native accessibility review remain active. Narrator/NVDA and advanced Windows
S Pen behaviors are not newly tested. Keep the Phase 8.3 program active and
withhold the consolidated Windows evaluation build, merge, and release.

## Hue and reference presentation follow-up — Phase 8.3 remains active

The next local iteration adds a pointer-driven HSV hue ring alongside the
existing saturation/brightness field and keyboard-operable Hue slider. It keeps
saturation and brightness unchanged when selecting hue, and the X11/XTest path
checks both the color result and restoration from a saved swatch. The reference
board now has a smaller filename header, compact zoom actions and an accessible
Settings menu. Screenshot assertions measure pixels inside its actual image
bounds and require the unique fixture marker, scene-color variety, and at least
85% image width. Local captures show 91.1% width in light mode and 96.3% in dark
mode at normal and narrow sizes. The screenshot rectangle is clipped to the actual
reference viewport. X11/XTest also opens Settings from a panned 110% view, selects
Center image, and verifies pan resets while zoom persists in workspace settings;
Fit image to window remains the separate zoom-and-pan reset.

The 30-image matrix and full X11 artwork workflow passed after the hue and
reference-center changes, including real live mouse strokes, layer transactions
and Undo, exports, reference-image isolation, reopen, and process-kill journal
recovery. The Color screenshot contains 2,204 distinct chromatic wheel colors.
Local formatting, strict Clippy, all 240 workspace tests, workspace build,
Windows GNU target checks/Clippy, and llvmpipe offscreen GPU smoke passed. Images
were visually reviewed through the local viewer. Actions run
[`40265944815`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40265944815)
passed all desktop platforms on predecessor `2b615f9`.

The next Linux alpha workflow expands the matrix to 31 required UI states and 32
rendered PNGs. It imports a second reference image through the real application
path and checks that automatic placement stays inside normal/narrow viewports and
clear of the movable palette. Local unit tests for placement pass, but the window
workflow itself is pending the next Actions run because this execution image has
no Xvfb/XTest. The latest local run passes formatting, strict Clippy, **242
workspace tests**, workspace build, Windows GNU target checks/Clippy, and the
llvmpipe GPU smoke. Overall Phase 8.3 visual acceptance, Windows evaluation,
advanced pen verification, and native screen-reader validation remain pending.

## Latest Phase 8.3 continuation — commit `f79c02d`

The Home/New Artwork checkpoint adds individual vector format illustrations,
larger ratio/background-aware canvas preview, inset-safe recent-project layout,
and a functional Open another project gallery tile with AccessKit semantics.
The local Xorg/XTEST run captured all 31 requested UI states and passed the full
painting/save/export/recovery workflow. Screenshots were inspected locally;
they demonstrate visible progress without completing design acceptance.

Local formatting, strict Clippy, 245 workspace tests, workspace build, Windows
GNU check/Clippy, and llvmpipe GPU smoke pass. Xvfb is absent in this container,
so the application window ran on a dedicated dummy Xorg server with XTEST. The
latest pushed run (40270241090, commit 76a77b0) passed Windows/macOS but failed
the Linux alpha workflow; the center-marker fix has passed locally and awaits a
new cross-platform Actions result. See [Phase 8 Studio progress](PHASE8-STUDIO-PROGRESS.md).
Phase 8.3 remains active; no merge, release, or consolidated evaluation build is
approved by this checkpoint.

## Latest local Phase 8.3 checkpoint — palette and Home layout

Commit `25930ed` corrects palette viewport placement/overlap, scrolling,
compact defaults, drag grip and edge docking; it also centers Home at wide sizes.
Its local 32-state Xorg/XTEST workflow and screenshots passed review. Actions
run `40277552431` passed Windows/macOS but exposed a timing-sensitive Linux
palette-collapse click. The follow-up below fixed the test geometry wait, and
`40278687191` passed all desktop jobs. Phase 8.3 remains active; recent Home
artwork presentation and the broader workspace/accessibility overhaul remain
unfinished. See [the detailed progress log](PHASE8-STUDIO-PROGRESS.md).

### Short-window New Artwork follow-up

Commit `0384663` makes all seven format cards, including Custom, fit in the
initial 640 × 480 view; XTest asserts the actual card bounds. The 32-state local
workflow, 246-test workspace suite, Windows GNU target checks, and software GPU
smoke pass. Actions run `40278687191` passed Windows, Linux and macOS. On Linux,
the full UI/artwork workflow passed; on Windows the portable canvas and stylus
diagnostic artifacts were produced. Phase 8.3 remains active. Continue the wider
workspace and accessibility overhaul; see the [detailed progress log](PHASE8-STUDIO-PROGRESS.md).

### Dark narrow New Artwork screenshot coverage

Commit `6271c79` expands the Linux UI matrix to 33 states, adding dark New
Artwork at 640 × 480 and checking that all seven cards stay inside the viewport
in both themes. Its full Xorg/XTEST workflow passes locally and the capture was
inspected. Actions run `40280252877` failed after the harness read preferences
before the asynchronous workspace-panel save finished; a wait for the saved
state now passes locally. Windows and macOS passed on `40280252877`. The wait
guard in `fb78a39` then passed all platforms in run `40280989079`; Linux passed
the full 33-state UI and artwork workflow, and Windows uploaded both existing
executables. See the [progress log](PHASE8-STUDIO-PROGRESS.md). Phase 8.3
remains active and is not accepted as complete.

### Windows AccessKit startup correction (cross-platform CI verified)

Commit `089c40b` exposed a physical Windows startup panic because the window was
shown before AccessKit initialization. Commit `140b789` creates it hidden and
shows it only after AccessKit, GPU and initial surface setup. Actions run
`40335600464` passed Windows, Linux, and macOS. The Windows release smoke
checked the visible top-level window and first Studio frame; the macOS smoke
checked its first rendered frame. Linux screenshot/artwork checks also passed
after stabilizing the Home picker Cancel assertion. The run includes
`rust-paint-lab-canvas-demo` and the separate stylus diagnostic artifacts. See
[the startup-smoke strategy](windows-startup-smoke.md) and the
[Phase 8 progress record](PHASE8-STUDIO-PROGRESS.md). Hosted smoke tests do not
replace testing on the Galaxy Book2 Pro 360. This is a blocking bug fix, not
Phase 8.3 completion.

### BitchPixel typography and brush-size workflow follow-up

The ongoing Phase 8.3 continuation has corrected the bundled static font
weights and introduced a shared typography role system using Space Grotesk and
Manrope. It applies across Home, document creation, floating Brush/Color/Layers
panels, and recovery. Narrow Home heading hints and the Brush active-setting row
now stay inside their allocated space. The live canvas also supports bounded
`[` / `]` brush-size changes, with finer Shift-modified steps, covered by a
document-level test and XTest. Local formatting, strict Clippy, 249 Rust tests,
workspace and Windows GNU checks, llvmpipe GPU smoke, and the 34-state Linux UI
and artwork workflow pass. Actions run `40423165359` passed Windows, Linux, and
macOS; Windows startup and macOS first-frame checks passed, and separate canvas
and stylus Windows artifacts were uploaded. See
[Phase 8.3 progress](PHASE8-STUDIO-PROGRESS.md); the studio overhaul remains in
progress and is not accepted as complete.
That earlier checkpoint exposed a narrow populated Layers/Reference collision;
the current continuation adds temporary coordinated compact geometry and
preserves the larger saved sizes for normal windows.

### Outfit typography and responsive workspace follow-up

The current local continuation adopts the firm Outfit display direction with
local static 700/800/900 faces and keeps Manrope for interface text, including
tabular numeric controls. It applies the shared 17-role hierarchy across Home,
New Artwork and floating panels, with both OFL notices in desktop packages.
Narrow-window panels now use transient coordinated geometry, compact Reference
image navigation and a one-line status bar; saved large-screen panel dimensions
remain intact. The compact layer menu supports repeated real visibility/lock
changes and Escape popup dismissal. The canvas preset picker updates the actual
brush engine, and Brush Library avoids a duplicate settings form.

Local validation now passes 255 workspace tests, strict Clippy, formatting,
workspace build, Windows GNU target checks, and the llvmpipe GPU smoke. The
Linux/Xorg/XTest artwork workflow passed with 37 captures at 640 × 480, 1280 ×
800 and 1920 × 1080; populated Recovery dialogs are measured and remain within
the narrow viewport. Actions run
[`40452947352`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40452947352)
passed Windows, Linux and macOS on code checkpoint `9c227bc`; Windows startup
and both separate Windows application artifacts passed, as did macOS startup
and the Linux UI/artwork run. See [the detailed progress log](PHASE8-STUDIO-PROGRESS.md).
Phase 8.3 remains active and is not accepted as complete.

### Engine-first painting checkpoint — `6d85c36`

The deterministic time-aware stabilizer and persistent raster line, rectangle
and ellipse workflow are implemented in `paint-document` and the live canvas.
All 262 local workspace tests, strict Clippy, formatting, workspace build,
Windows GNU target checks, offscreen llvmpipe smoke and Linux Xorg/XTest artwork
automation passed. Actions run
[`40529268958`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40529268958)
passed Windows, Linux and macOS; it published the ordinary
[`rust-paint-lab-canvas-demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40529268958/artifacts/12744953710)
and retained [`rust-paint-lab-windows-diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40529268958/artifacts/12746885390)
artifacts. The Phase 8.3 program remains active. Continue the brush and editing
engine with persistent, undoable end-to-end workflows before expanding into
vector or animation scope.

### Linear and radial raster gradients — `36a604c`, CI passed

The current working continuation adds selected-layer Linear and Radial gradients,
selection clipping, a transient bounded preview behind floating workspace panels,
and one-step save/undo/redo behavior. The XTest workflow asserts both actual UI
modes, Escape leaves the native project byte-identical, and committed gradients
change saved layer pixels. Local formatting, 266 tests, strict Clippy, workspace
build, Windows GNU target checks, llvmpipe GPU smoke and the full Linux/Xvfb UI
and artwork workflow pass. Actions run
[`40546299074`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074)
passed on Windows, Linux and macOS, including Windows visible-window startup,
Linux XTest artwork validation, and macOS first-frame startup. It uploaded the
Windows [`canvas demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074/artifacts/12752669319),
retained [`stylus diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074/artifacts/12752474102),
and Linux [`UI screenshot archive`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40546299074/artifacts/12752754019).
Gradients are raster pixels with solid RGB stops and global brush opacity; they
are not editable objects and full ICC color management is not implemented. See
[the Phase 8.3 progress log](PHASE8-STUDIO-PROGRESS.md).

### Workspace launcher and Layers integration — `8985754`, CI passed

F6 Workspace controls now route to the actual floating Layers/Brush panels
instead of duplicating their full controls. Layer thumbnails use a bounded,
per-layer 96 × 64 cache backed by document tile identity and opacity; targeted
clear stays in the document undo transaction and retains selection. The compact
640 × 480 layout now keeps populated Layers and Reference panels inside the
window without overlapping, while preserving saved desktop panel dimensions.
Local formatting, strict Clippy, all 270 workspace tests, build, Windows GNU
checks, required llvmpipe/OpenGL GPU smoke, and the full Linux Xorg/XTest artwork
flow pass. GitHub Actions run
[`40574152036`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036)
passed on Windows, Linux, and macOS. Windows visible startup passed and uploaded
the [`canvas demo`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036/artifacts/12767816064)
and retained [`stylus diagnostic`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036/artifacts/12766097424);
Linux uploaded the [`studio screenshot archive`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40574152036/artifacts/12767007921).
The screenshot review covered populated light Layers at 1280 × 800 and dark
Layers/Reference at 640 × 480; full phase acceptance and physical Windows
validation remain open. See [the current Phase 8.3 progress checkpoint](PHASE8-STUDIO-PROGRESS.md).

### Layer copy and compositing integration — current local checkpoint

Selection Copy now creates a real editable `.rpl` layer from rectangle, ellipse,
or lasso-selected pixels. Source pixels remain unchanged; document and tile
limits are enforced, and Undo/Redo and save/reopen preserve the transaction.
The selected-layer opacity and blend controls are exercised through the real UI
with saved-project and history assertions. Exact-pixel tests cover Normal,
Multiply, Screen and Overlay. An unchanged COW edit no longer dirties artwork or
adds a redundant undo entry. Local validation passed all 275 workspace tests,
strict Clippy, formatting, workspace build, Windows GNU checks, llvmpipe GPU
smoke, and the complete Linux Xorg/XTest artwork/recovery workflow. GitHub
Actions run [`40590882859`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40590882859)
passed Windows, Linux and macOS for commit `87588a4`; Windows startup and the
Linux application/recovery workflow passed, and the ordinary canvas, stylus,
and UI screenshot artifacts were published. See the [Phase 8.3 progress
log](PHASE8-STUDIO-PROGRESS.md). This does not complete the phase or validate
additional physical Windows pen behavior.

### Phase 9 raster drawing aid — symmetry and guides (local validation)

The live document engine applies up to 16 reflected/radial raster copies from a
workspace-only symmetry setting. A compact accessible palette panel and **Y**
shortcut select Off, vertical, horizontal, combined reflection or radial copies,
adjust radial count and move the normalized origin. Guides follow canvas zoom
and pan and remain a UI overlay; the resulting pixels are a single reversible
native raster edit. Stroke transactions snapshot their mode, cancelled copies
never commit, and coincident copies do not accumulate additional opacity. `.rpl`
v1 remains unchanged; settings are local workspace preferences.

The focused document tests and a real Linux XTest workflow validate mirrored
and radial pixels, cancellation, undo/redo, save/reopen, accessible control names
and bounded guide geometry. On 2026-10-28, the full Xorg/XTest artwork and
recovery suite passed locally with the symmetry interaction and generated
53 screenshots; the symmetry panel capture was inspected. Full workspace
validation passed: formatting, strict Clippy, all 317 workspace tests, workspace
build, Windows GNU check/Clippy, and required llvmpipe GPU smoke. GitHub Actions
for the current code checkpoint are pending. Physical Windows pen/symmetry
combinations are not newly verified. See the
[Phase 9 progress entry](PHASE8-STUDIO-PROGRESS.md). The next feature should be
a complete selection-transform or QuickShape workflow; editable layer masks
need a separate versioned-storage and recovery design first.

### Place symmetry origin directly on canvas — local checkpoint

The Drawing Symmetry panel can now arm a one-contact mouse or native pen origin
placement. The pointer is converted with the current canvas/document mapping,
consumed through its terminal event, and never reaches painting. Escape, window
deactivation, resize, close and capture loss clear the transient picker state.
The normalized origin persists only in local workspace preferences.

The live Xorg/XTest suite placed the origin at 50%, 68%, confirmed the preference
and unchanged artwork/no paint-start, checked mirrored pixels around the custom
origin, verified exact undo/redo, and cancelled a second placement with Escape.
All **318 workspace tests**, strict Clippy, format, build, Windows GNU checks,
required llvmpipe/OpenGL smoke and the full Linux artwork/recovery workflow passed.
The populated custom-origin screenshot and representative Home/New Artwork/narrow
Layers/Reference screens were inspected. The narrow multi-panel state still
obstructs most of the artboard, so coordinated compact panels remain open work.
Actions run [`40738225142`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40738225142)
passed the prior symmetry commit; the origin-placement follow-up requires its own
cross-platform run. This is one Phase 9 slice; Phase 8.3 and Phase 9 remain active.

### Direct selected-pixel movement — local integration checkpoint

Mouse and native WM_POINTER selection gestures now move a selected region on the
active visible, unlocked raster layer. The translated selection outline follows
the pointer; release performs one normal document transaction, and Escape,
window/focus loss, or capture loss cancels without altering artwork. The
selection is constrained within document bounds. No native-format migration was
needed. The full Linux Xorg/XTest workflow checked cancelled-project byte
identity, actual changed layer tiles, save/reopen-compatible bytes, and exact
Undo/Redo before reselecting and continuing the layer copy workflow.

Locally, formatting, strict Clippy, all **320 workspace tests**, workspace
build, Windows GNU check/Clippy, required llvmpipe/OpenGL GPU smoke, and the full
populated Linux artwork/export/recovery workflow passed. The post-move workspace
image was visually inspected; compact Layers/Reference crowding remains an
existing workspace issue. No new physical S Pen behavior is claimed. Actions
run [`40741970797`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40741970797)
has passed Windows, Linux and macOS for previous checkpoint `f2046e3` and
published both Windows application artifacts. This implementation needs its own
hosted run.

Next Phase 9 work: finish interactive scale/rotation selection transforms or
integrate QuickShape with the existing raster geometry tools, then design a
versioned mask format and migration before editable masks. Phase 10
editable vector/text, Phase 11 timelapse/animation, and Phase 8.3 acceptance
remain incomplete.

### Interactive canvas selection transforms — local integration checkpoint

The selection boundary now provides movable and transformable canvas controls.
Drag within the selection to move pixels, use the lower-right handle for
proportional scale, or the upper handle to rotate around the selection center.
Shift snaps rotation to 15 degrees. Each gesture previews without writing
artwork, then commits as a single native edit; Escape and lifecycle interruption
cancel it without changing document bytes. Rectangle, ellipse and lasso
selections share the behavior and `.rpl` v1 is unchanged.

The Linux Xorg/XTest workflow exercised movement, scaling and rotation on real
saved raster pixels, including cancellation and byte-exact undo/redo. Formatting,
strict Clippy, **322 workspace tests**, build, Windows GNU checks, required GPU
smoke and the complete artwork/export/recovery workflow passed locally. I
inspected the populated selection-transform screenshot. This does not add new
physical S Pen validation. Cross-platform Actions for this checkpoint are
complete in Actions run
[`40747903490`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40747903490),
including the existing Windows canvas and stylus artifact jobs. Next, implement
a complete QuickShape workflow or specify the versioned document migration
required for editable layer masks. Phase 8.3, Phase 9, Phase 10 and Phase 11
remain active; none is declared complete.

### QuickShape recognition — implementation checkpoint

The shared raster geometry workflow now has a QuickShape tool for recognizing
lines, closed axis-aligned rectangles, and ellipses from a bounded pointer
path. It previews provisionally and commits one existing raster transaction
only after recognition; cancellation and invalid gestures leave artwork alone.
The tool is tested with deterministic recognizer cases and is included in the
real application workflow for native save, pixel-level verification, Undo and
Redo. Local formatting, strict Clippy, build, Windows GNU checks, 324 workspace
tests, and required software-GPU smoke pass. The current container lacks Xvfb,
so this checkpoint's full XTest/screenshot run and cross-platform hosted CI are
pending; no physical stylus result is claimed. Phase 9 remains active. Next is
versioned, v1-compatible layer-mask storage and compositing design; Phase 10
vector/text and Phase 11 timelapse/animation remain open work.

### Editable masks — hosted Round 1 verification pending

Actions run [`40762059402`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40762059402)
failed while the nested Layer Actions mask menu was being used. Mask operations
moved to direct controls in Selected layer settings in commit `8cfc990`. The
follow-up run, [`40801195199`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40801195199),
still failed in the Linux workflow because the real app emitted
`UI_TEST_LAYER_MASK_EDIT_CLICKED: 4 true` while the Python test expected `4
True`. Local Xorg reproduced the exact event and confirmed mask mode was entered.
The harness now uses lowercase Rust bool spellings, dismisses the Brush Library
that opens contextually in mask mode before sending its canvas dab, and accepts
that v1 omits the optional mask field.

The full local Linux/Xorg/XTest artwork workflow now passes on display `:102`
with llvmpipe/OpenGL. It verifies all mask operations, v2 save/reopen, unchanged
raster tiles, exact Undo/Redo, v1 compatibility after removal, selection-clipped
compositing, OpenRaster disclosure, and the existing export/recovery scenarios.
The 2026-10-29 local validation also passed all 336 workspace tests, formatting,
strict Clippy, workspace build, Windows GNU check/Clippy, test metadata check,
and llvmpipe GPU smoke; the Xorg workflow generated 59 PNGs. I inspected the
mask-editing, painted-mask, and populated-recovery screenshots. The hosted
cross-platform gate passed in Actions run
[`40805980434`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40805980434).
Linux completed the full artwork/mask workflow and screenshots, Windows produced
both the canvas and stylus diagnostic artifacts and passed visible startup, and
macOS passed its startup smoke/package job. Round 1 is accepted as a validated
implementation checkpoint; physical Windows mask/S Pen behavior remains a
separate owner evaluation.

### Phase 10 — initial editable vector path slice, locally validated

The current implementation adds editable line and cubic path objects inside
raster layers. V selects the canvas path tool: drag on empty artwork creates a
polyline, clicking a path selects it, dragging moves the whole path, and the
contextual Delete path action is undoable. Paths remain separate from raster
tiles but share layer ordering, visibility, opacity, blend modes, masks, history,
thumbnails and derived canvas/export compositing. PNG and OpenRaster flatten the
composite; OpenRaster reports path loss while `.rpl` retains path objects.

RPL v3 stores vector metadata and a monotonic document-wide object-ID allocator;
v1/v2 reading and the prior v1/v2 write choices remain. Resource limits bound
path count, segment count, coordinate/width ranges, metadata size and estimated
raster work. Twelve focused document tests cover v1/v2/v3 behavior, mixed masks,
rendering, save/reopen, monotonic IDs, layer-duplication ID remapping,
movement/deletion history, corruption rejection, and PNG/OpenRaster output.
The real Xorg/XTest workflow now creates, saves/reopens, moves and deletes a path,
checks Undo/Redo, and verifies OpenRaster flattening alongside prior mask,
painting, export and forced-crash journal-recovery workflows. It passes locally
with strict Rust checks, the workspace build, Windows GNU target check/Clippy,
and the llvmpipe GPU smoke; 61 populated screenshots were generated and
representative workspace and vector states inspected. A one-ULP-scale JSON f64
round-trip difference is allowed only within `1e-12`; native raster/mask tile
tables remain byte-identical during recovery. The full hosted Linux workflow
previously took 596 seconds, four seconds under its former 10-minute cap; CI now
allows 15 minutes without skipping checks. The dense-path follow-up reuses
bounded source-tile rows for Home and Layer thumbnails and culls paths by
conservative bounds before tile rasterization. Two fixtures cover 8,192
segments; their local debug timings were about 0.6 seconds and 0.5 seconds,
respectively. The complete Xorg workflow above ran before this thumbnail-only
follow-up; targeted dense-preview tests pass, and hosted Linux CI will exercise
the updated UI code. Cross-platform Actions for the latest code remain pending.

Phase 8.3, Phase 10 and Phase 11 remain active. The Phase 9 mask checkpoint is
accepted by run `40805980434`; new Phase 10 Actions validation is pending. Phase
10 still needs node editing, Bézier handles, editable style controls, path
transforms and richer object operations. Phase 11 animation/timelapse and later
text/effects remain open.

#### Latest vector validation correction

Vector commit `4a5a808` passed all hosted platforms in Actions run
[`40817631458`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40817631458).
The following thumbnail optimization commit `d357580` had a Linux XTest failure
in run `40818285789`; local reproduction traced it to the harness accepting stale
selection, transform-cancel, and mask-toggle markers. The harness now requires
fresh event counts and the complete 61-screenshot Xorg/XTest workflow passes
locally against the optimized code. A new hosted matrix is required for this
test-only correction; do not treat the earlier green run as validation of
`d357580` or the harness update.

### Mask workflow correction and vector style slice (2026-10-29)

Run [`40820933426`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40820933426)
exposed a stale geometry assumption in mask-removal automation: the script
estimated the Layers scroll viewport from the panel frame even though the
Remove button was clipped below the actual egui ScrollArea. The test adapter
now reports the ScrollArea clip bounds and targets controls against those live
bounds. Local full Xorg/XTest validation passes all six mask actions and the
existing raster workflows.

Editable paths now have a tested width control and an Apply color action using
the active brush color. The document API persists stroke RGBA and width through
native v3 data, retains geometry and raster/mask separation, invalidates layer
thumbnails, and records a single undo transaction. Automated API coverage
includes alpha; the UI intentionally exposes no separate path-opacity control
yet. Rust formatting, strict Clippy, **354 workspace tests**, build, Windows GNU
check/Clippy, llvmpipe GPU smoke and the complete local Xorg/XTest workflow pass.
The current cross-platform Actions run is pending. The inspected narrow
Layers/Reference state still consumes nearly all 640-pixel canvas width. Next:
implement bounded, keyboard-accessible vector node movement and undo, then
continue curve handles, fill and transforms without claiming Phase 10 complete.

### Canvas-first Inspector corrections — current checkpoint

Milestone 1 (Layers hierarchy) is implemented locally on top of `268b4b4`:
cached raster/mask thumbnails, explicit raster/mask/vector-child targets,
undoable vector-path visibility, 44-point primary row controls, independent
desktop stack scrolling with selected-layer properties, and a single reachable
compact scroll region for short 640 × 480 panel heights. The complete Linux
Xorg/XTest artwork workflow passed locally with 79 screenshots; formatting,
strict host/Windows GNU Clippy, workspace tests/build, Windows GNU check, and
llvmpipe GPU smoke also passed. The hosted matrix is pending the checkpoint
commit. Continue with Milestone 2 (ordered horizontal Inspector tabs and
selected-vector editing in Inspector Edit), then responsive geometry and the
six-layout visual/QA milestone. Phase 8.3 remains active.

### Phase 10 Linux anchor-editor regression — verified test synchronization (2026-10-30)

Run [`41094100891`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41094100891)
passed Windows and macOS but failed Linux while entering the reopened cubic
path's anchor editor. The smoke now holds the visible control through a rendered
frame and asserts the actual selected-path editor state and fresh rendered
handle, while preserving the existing editing-event assertion. State-transition
diagnostics and incremental smoke-log indexing were added. Rust, Python helper,
Windows-target, and llvmpipe checks pass locally. The updated full Linux
Xorg/XTest artwork workflow completed with 78 screenshots; the `H` fallback
entered the actual editor and rendered an anchor handle, while the pointer click
did not activate in this Xvfb run. Hosted run
[`41102591526`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41102591526)
passed Windows, Linux, and macOS on commit `03790ef`. The editor-state timeout
is resolved in automated validation; direct pointer activation remains a
separate follow-up to diagnose. Phase 8.3, Phase 9 physical checks, Phase 10,
and Phase 11 remain active.

### Current local continuation — editable line-to-cubic conversion (2026-10-29)

The Phase 10 node editor can convert an adjacent line segment to an editable
cubic while retaining its straight appearance, then expose the new handles.
Native v3 is unchanged and v1/v2 remain readable. Engine tests and the real
Linux UI workflow verify geometry, saved path data, raster/mask table isolation,
and undo/redo. Rust formatting, strict host/Windows GNU Clippy, all 366 workspace
tests, workspace build, Windows GNU target check, and llvmpipe GPU smoke pass
locally.

The complete Linux Xorg/XTest workflow now passes locally, including the new
conversion menu, native save/reopen, exact Undo/Redo, and populated cubic-handle
editing. It re-exercises the Phase 9 mask operations and artwork/save/export/
recovery flows. An exact Color Mixer redo failure was traced to f64 metadata
drift during native JSON decoding; enabling serde_json `float_roundtrip` fixed
the repeated round-trip and the UI regression without changing the format.
Formatting, strict host/Windows GNU Clippy, all 366 workspace tests, workspace
build, Windows GNU check, and llvmpipe GPU smoke pass locally. The XTest run
completed with 64 screenshot captures; populated vector, narrow Layers/Reference,
and light Home views were inspected. Hosted run
[`40887124571`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40887124571)
passed Windows, Linux, and macOS with the 25-minute Linux workflow limit, and
produced the Windows canvas/diagnostic and supported Linux/macOS alpha artifacts.
Cubic authoring, object transforms, duplication, fill, and SVG interoperability
remain open.

### Cubic handle editing and current validation — 2026-10-29

The local Phase 10 continuation adds incoming/outgoing Bézier handle selection,
canvas dragging, and named keyboard-accessible X/Y controls for existing cubic
paths. Handle movement is provisional until commit, preserves the other path
geometry, and is an undoable RPL v3 transaction. serde_json's `float_roundtrip`
feature was enabled after the full save/undo/redo XTest found least-significant
coordinate drift across repeated native reads; eight-cycle native-byte tests and
the UI mixer regression now pass with exact results. This is a parser feature
change only: v1/v2/v3 schemas and their compatibility behavior are unchanged.

Current local validation: Rust 1.90 formatting, strict workspace and Windows
GNU-target Clippy, 364 workspace test cases, workspace build, Windows GNU
workspace check, required llvmpipe GPU smoke, and full Linux Xorg/XTest artwork
workflow pass. The XTest workflow produced 64 PNGs; the populated cubic-handle,
mask-editing, Home, narrow Recovery, and narrow Layers/Reference captures were
visually inspected. These results are local and do not yet validate hosted
Actions, Windows GUI startup, macOS startup, physical S Pen behavior, or
Narrator/NVDA. The 640 × 480 multi-panel layout remains crowded. Continue with
interactive cubic path creation and a complete save/reopen/undo/redo workflow;
then assess vector transforms and fill. Phase 8.3, Phase 9 hardware validation,
Phase 10, and Phase 11 remain active.

### Latest Phase 10 continuation — anchor insertion/removal (2026-10-29)

Path anchors can now be split at exact midpoints and safely removed through the
accessible node menu. The document model preserves cubic shape when subdividing
with De Casteljau and refuses unsupported approximate interior-curve deletion.
Native v3 layout and older project reads are unchanged. All 359 local workspace
tests, strict host/Windows GNU Clippy, Windows GNU check, workspace build, GPU
smoke, and the full 63-screenshot Linux Xorg/XTest artwork workflow pass on the
current local source. The new workflow verified real insertion/deletion,
thumbnail refresh, raster/mask isolation, and exact undo/redo; the midpoint
editor screenshot was inspected. Parent commit `8a0b5a0` passed cross-platform
Actions run [`40842835450`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40842835450).
The anchor-operation follow-up requires its own hosted matrix. Continue next with editable cubic creation and independent
handles, then path fills/transforms; Phase 8.3, 9 hardware validation, Phase 10,
and Phase 11 remain active.

### Vector anchor movement — local integration checkpoint (2026-10-29)

The latest local slice adds per-anchor vector editing through both canvas drag
and a keyboard/accessibility-friendly anchor selector with exact X/Y controls.
The document API retains adjacent cubic handle offsets, rejects invalid and
locked-layer edits atomically, and records one undoable transaction. The native
v3 geometry and monotonic object IDs are unchanged; v1/v2 remain readable.
Complete Xorg/XTest validation now exercises anchor movement and width styling
through the application, including preview, save/reopen, thumbnail invalidation,
unchanged raster/mask storage, and exact Undo/Redo. All 356 workspace tests,
formatting, strict Clippy, workspace build, Windows GNU check/Clippy, and
llvmpipe GPU smoke pass locally. Hosted Actions for this code are pending; the
green run `40831057143` is for the parent commit. OpenRaster export with a 360 px
path measured 12.444 s in the local UI run and 11.993 s in a debug standalone
measurement; the earlier 9.669 s UI export was a raster/mask fixture. These are
not release or hardware performance measurements.

The next vector slice should add and remove anchors from the line paths the UI
currently creates, with exact single-transaction Undo/Redo and native reopen
coverage. Cubic path authoring and handle editing, fills, scale/rotation, and
duplication remain unimplemented. The 640 × 480 workspace still has little
exposed canvas when both Layers and Reference panels are open. Phase 8.3, Phase
9 hardware testing, Phase 10 and Phase 11 remain active.

### Mask workflow correction and vector style slice (2026-10-29)

Run [`40820933426`](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/40820933426)
exposed a stale geometry assumption in mask-removal automation: the script
estimated the Layers scroll viewport from the panel frame even though the
Remove button was clipped below the actual egui ScrollArea. The test adapter
now reports the ScrollArea clip bounds and targets controls against those live
bounds. Local full Xorg/XTest validation passes all six mask actions and the
existing raster workflows.

Editable paths now have a tested width control and an Apply color action using
the active brush color. The document API persists stroke RGBA and width through
native v3 data, retains geometry and raster/mask separation, invalidates layer
thumbnails, and records a single undo transaction. Automated API coverage
includes alpha; the UI intentionally exposes no separate path-opacity control
yet. Rust formatting, strict Clippy, **354 workspace tests**, build, Windows GNU
check/Clippy, llvmpipe GPU smoke and the complete local Xorg/XTest workflow pass.
The current cross-platform Actions run is pending. The inspected narrow
Layers/Reference state still consumes nearly all 640-pixel canvas width. Next:
implement bounded, keyboard-accessible vector node movement and undo, then
continue curve handles, fill and transforms without claiming Phase 10 complete.
