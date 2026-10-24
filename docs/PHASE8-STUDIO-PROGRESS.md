# Phase 8 — Creative Studio Overdrive progress

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
  artists to type paths. Linux native dialogs use the XDG portal and require a
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
- `cargo test --workspace --locked`: passed, **160 tests**, including draft cleanup
  and backward-compatible, validated document-setup preference persistence.
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
  reopened window title to show `artwork.rpl`; Windows was still compiling. The
  draft cleanup and remembered setup commits are newer and still require CI.
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

## Partial and unimplemented Phase 8 requirements

- Welcome screen is not yet a full separate workspace with thumbnails, settings
  pages, or session continuation. The engine's internal initial placeholder
  remains hidden and input-blocked until the artist creates or opens a project.
- Document setup now remembers the last successfully created settings, but has no
  recent-dimension list, explicit physical-paper preset catalog beyond current
  presets, or automated app-level reopen/export dimension workflow. Preset dimensions
  are bounded to current format.
- Drafts now have a confirmation-gated remove action, but there is still no
  rename, pin, or age sorting. Continuous debounced autosave and a full save-state
  indicator remain unfinished; the existing recovery worker still checkpoints
  committed revisions.
- File dialogs have compiled on native Linux and Windows GNU targets; Windows,
  Linux portal, and macOS visual/runtime behavior needs CI or manual verification.
- The first compact menus and vertical tool palette are implemented, but there are
  no draggable/dockable panels, snap previews, palette placement/size preferences,
  complete accessibility review, or polished color/brush/layer panels. Menu
  categories remain limited to currently supported actions.
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

1. Push the locally validated menu/palette slice and rerun CI; verify Linux
   welcome/open/save smoke, Windows package build, macOS packaging, and artifacts.
2. Investigate the Linux reopen timeout from the latest workflow output; add a
   visible test-harness dialog state or deterministic open interaction if needed.
3. Continue compact brush/layer/color controls and close-save failure coverage in
   the real window workflow.

No physical Windows picker, stylus, touch, power-loss, or hardware-performance
validation has been performed in this phase.
