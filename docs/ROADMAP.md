# Rust Paint Lab engineering roadmap

This is a working engineering sequence, not a release promise. Phase 8.3 completes
workspace and accessibility foundations before Phase 9 adds new drawing engines.
Artwork preservation and cross-platform validation apply to every track.

## Experience and accessibility

- **Phase 8.3 — Studio and accessibility foundation:** unified movable/dockable
  tool palette, contextual brush/layer/reference/color panels, recent-artwork
  previews, visual New Artwork flow, recovery manager, accessible preferences,
  keyboard/focus behavior, and Windows assistive-technology validation.
- Complete theme, scalable interface, touch target, contrast, handedness, reduced
  motion, small-window and high-DPI behavior; add manual Narrator/NVDA testing.
- Phase 9: adaptable workspace for shape/fill/selection workflows; Phase 10:
  vector/text workspace; Phase 11: animation/timelapse workspace; Phase 12:
  mobile/adaptive interfaces.

## Creative engine

- Phase 9 — shapes, QuickShape, fills, gradients, symmetry, rulers/guides,
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
`feature/studio-overdrive-phase-8`, starting from `28e1921` for this continuation.
The current implementation includes AccessKit event integration and generated-tree
tests, a movable/dockable palette, brush and color flyouts, an optional floating
Layers panel, visual New Artwork cards, a confirmation-safe recovery manager, bounded
recent-artwork thumbnails, compact reference controls, reduced motion, interface
scaling and responsive Home cards. The floating Layers panel supports real layer
actions with persisted position/size; bounded recovery previews load only on
request after native-file validation. The current visual pass adds responsive
flyouts, illustrated format cards, stronger Home hierarchy, focusable
format/brush selection, and a broad Light/Dark/high-contrast screenshot matrix.
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
is not complete. A visual integration pass from commit `1220722` is now in
progress: Brush placement/size/scrolling, responsive visual format cards, Home
hierarchy and theme-safe heading contrast, and populated light/dark/high-contrast
UI screenshot coverage are being improved. Actions run `40230324720` confirmed
the first eight light-theme normal/narrow screenshots but exposed stale test-only
palette geometry when the smoke clicked after leaving Home; that was corrected.
Run `40230678773` passed three-platform format, lint, tests and build, Linux GPU
and live-window smoke, then captured 17 of 18 requested UI states. Its only failure
was during the final high-contrast screenshot check. The generic sampled-color
threshold was lowered for that state, but run `40231310053` repeated the failure.
The Actions annotation truncates the Python exception, so the exact cause is still
unconfirmed. The smoke now reports capture stage and color count and keeps the
output short enough to retain the failure detail. The corrected full matrix and
consolidated Windows artifact are pending. The screenshot archive could not be
inspected here because Actions artifact blob downloads return HTTP 403. See
[`PHASE8-STUDIO-PROGRESS.md`](PHASE8-STUDIO-PROGRESS.md) for current CI evidence,
limitations and exact continuation steps. Phase numbering and boundaries are
provisional.
