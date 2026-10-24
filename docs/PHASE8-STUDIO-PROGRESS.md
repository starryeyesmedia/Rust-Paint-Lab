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
  sequence. The Windows and macOS jobs are still in progress. The preceding
  `40124892298` Windows and macOS jobs both passed and uploaded
  `rust-paint-lab-windows-diagnostic`, `rust-paint-lab-canvas-demo`, and the
  macOS alpha artifact; its Linux job failed at the recovery ordering described
  above.

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

- Welcome screen is not yet a full separate workspace with thumbnails, settings
  pages, or session continuation. The engine's internal initial placeholder
  remains hidden and input-blocked until the artist creates or opens a project.
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
- The first compact menus and vertical tool palette are implemented, and everyday
  brush/color controls are less crowded, but there are no draggable/dockable panels,
  snap previews, palette placement/size preferences, complete accessibility review,
  brush thumbnails/library, HSV wheel, recent-color palette, or polished layer panel.
  Menu categories remain limited to currently supported actions.
- Reference board supports multiple selected references but still lacks internal
  zoom/pan, rotation, lock, project-scoped configurations, native drag/drop, and
  color sampling.
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
