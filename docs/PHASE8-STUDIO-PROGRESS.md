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

### Validation

- `cargo fmt --all --check`: passed.
- Workspace Clippy with `-D warnings`: passed.
- `cargo test --workspace --locked`: passed, **153 tests**.
- `cargo build --workspace --locked`: passed.
- Required offscreen pixel test passed on Mesa llvmpipe/OpenGL. Forcing Vulkan in
  this container did not find a Vulkan adapter; the default backend selected
  llvmpipe and completed the pixel checks.
- Windows GNU workspace `cargo check` and Clippy passed. This is cross-checking,
  not Windows linking or execution.
- The real alpha Xvfb window workflow could not run locally because this container
  does not provide `xvfb-run`. CI is configured to run it and now starts with the
  welcome/New Document flow in the test-only path-entry mode.
- No GitHub Actions run exists for the Phase 8 changes yet.

## Partial and unimplemented Phase 8 requirements

- Welcome screen is an overlay on the current shell, not yet a full canvas-free
  startup workspace with thumbnails, settings pages, session continuation, or
  persistent drafts. The engine's internal initial placeholder remains hidden and
  input-blocked until the artist creates or opens a project.
- Document setup has no remembered settings/recent-dimension list, explicit unit
  presets for physical paper beyond current pixel presets, or automated app-level
  reopen/export dimension workflow. Preset dimensions are bounded to current format.
- Normal close still uses the existing save/recovery modal. Persistent drafts,
  conflict-aware close autosave, debounced incremental persistence, and a full
  save-state indicator remain unfinished.
- File dialogs have compiled on native Linux and Windows GNU targets; Windows,
  Linux portal, and macOS visual/runtime behavior needs CI or manual verification.
- Workspace still has the previous dense top toolbar. Vertical tool palette, proper
  menus, docking/snap groups, accessibility review, and polished brush/layer/color
  controls are not yet implemented.
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

## Exact next steps

1. Run `scripts/alpha-workflow-smoke.py` in CI; its test path harness now creates a
   document from the welcome screen before exercising save/reopen/recovery.
2. Update the canvas dependency inventory and Windows artifact notices for rfd's
   locked Windows dependency graph.
3. Commit and push the first Phase 8 slice on `feature/studio-overdrive-phase-8`,
   then verify the new Windows, Linux, and macOS Actions jobs and portable artifacts.
4. Next implementation checkpoint: exercise new document dimensions through native
   save/reopen/export integration; then add bounded persistent untitled drafts and
   safe close autosave before broad visual redesign work.

No physical Windows picker, stylus, touch, power-loss, or hardware-performance
validation has been performed in this phase.
