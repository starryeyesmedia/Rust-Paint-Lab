# Rust Paint Lab engineering roadmap

This is a working engineering sequence, not a release promise. Phase 8.3 completes
workspace and accessibility foundations before Phase 9 adds new drawing engines.
Artwork preservation and cross-platform validation apply to every track.

## Adopted product identity — BitchPixel

- **Product identity:** BitchPixel is the chosen name, not a provisional candidate. Rust Paint Lab continues as the engineering repository, internal package/executable names and format terminology until those identifiers are migrated deliberately, without breaking existing projects or CI.
- **Approved brand lines:** “No limits. No bullshit.” (primary positioning) and “Create without permission.” (creative-freedom message). Keep both exact phrases available in product/design work.
- **Brand meaning:** Reject paywalls, subscriptions, locked creative tools, forced accounts, advertising/tracking, and unnecessary ecosystem/device exclusion. Preserve realistic hardware constraints and do not advertise planned features as shipped.
- **Visual direction:** Design and implementation proceed together in the running application; screenshots inform iterative corrections. Current functional egui-based layouts do **not** meet the approved visual ambition; do not conflate passing CI or screenshot existence with design acceptance. Treat accessible dark/light/high-contrast typography, clear hierarchy, artist-centered artwork presentation and coherent floating tools as acceptance criteria.
- **Open implementation tasks:** design system and accessible visual treatment; name/metadata/application-ID/binary migration plan; trademark/domain due diligence; distribution license. See [product identity issue](https://github.com/starryeyesmedia/Rust-Paint-Lab/issues/1).
- **Scope:** Continue Phase 8.3 reliability and accessibility work; do not make unreviewed breaking package/format/identifier changes or merge the development branch solely for branding.

## Experience and accessibility

- **Phase 8.3 — Studio and accessibility foundation:** unified movable/dockable
  tool palette, contextual brush/layer/reference/color panels, recent-artwork
  previews, visual New Artwork flow, recovery manager, accessible preferences,
  keyboard/focus behavior, and Windows assistive-technology validation.
- **Current engine continuation:** a deterministic per-stroke One Euro
  stabilizer and document-backed raster line, rectangle and ellipse tools now
  provide an initial functional slice. This is not Phase 8.3 acceptance; continue
  stabilization workflow validation and substantive brush/editing development.
- Complete theme, scalable interface, touch target, contrast, handedness, reduced
  motion, small-window and high-DPI behavior; add manual Narrator/NVDA testing.
- Phase 9: adaptable workspace for shape/fill/selection workflows; Phase 10:
  vector/text workspace; Phase 11: animation/timelapse workspace; Phase 12:
  mobile/adaptive interfaces.

## Creative engine

- Phase 9 — QuickShape, fills, gradients, symmetry, rulers/guides,
  selection/transform improvements, masks and brush-system expansion.
- Phase 10 — editable vector paths and text, mixed raster/vector editing,
  non-destructive effects, and natural-media foundations.
- Phase 11 — frame animation, onion skin, automatic per-action timelapse, and
  interoperability expansion.
- Phase 12 — broader creative workflows and mobile-core reuse.

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
