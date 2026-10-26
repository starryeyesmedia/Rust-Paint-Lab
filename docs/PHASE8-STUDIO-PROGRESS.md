# Phase 8 — Creative Studio Overdrive progress

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
jobs were still running at the time of this update, and this test-only timing
fix needs a fresh CI run.

### Exact continuation

1. Push the saved-preference wait guard; verify Linux XTest and all platform
   builds, then record the run result here.
2. Continue the actual Home/canvas presentation and keyboard/focus/AccessKit
   integration. Keep the more complete narrow layouts, existing artwork
   protection, and full Phase 8.3 status; do not merge or release.
