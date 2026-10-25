# Rust Paint Lab engineering roadmap

This is a working engineering sequence, not a release promise. Phase 8.3 completes
workspace and accessibility foundations before Phase 9 adds new drawing engines.
Artwork preservation and cross-platform validation apply to every track.

## Provisional product identity — next phase

- **Working candidate:** BitchPixel. Rust Paint Lab remains the development name for the repository, application identifiers, builds, and formats until the owner approves a final rename.
- **Approved taglines:** “No limits. No bullshit.” (primary positioning) and “Create without permission.” (creative-freedom message). Keep both exact phrases available for the next phase.
- **Interpretation:** Reject artificial commercial and ecosystem barriers: subscription tiers, paywalled features, forced accounts, ads/tracking, and unnecessary device exclusion. Do not suggest that any device can run every capability or that future creative engines are already implemented.
- **Open decisions:** final name, trademark/domain availability, visual branding, accessible slogan treatment, metadata/About description, app identifiers and binary names, and distribution license. See [identity planning issue](https://github.com/starryeyesmedia/Rust-Paint-Lab/issues/1).
- **Scope:** This decision must not interrupt Phase 8.3 accessibility, stability, or UI work. Final public branding is deferred.

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
was after the dark Recovery capture, before the final high-contrast screenshot.
Runs `40231310053` and `40231796822` repeated it. The latter's concise failure
message showed that the dark New Artwork capture left a fresh unsaved document;
Escape correctly opened the save-on-close path, causing the harness's 20-second
wait to expire. The screenshot color-count hypothesis was incorrect. The smoke
now terminates that screenshot-only process after capture; normal clean-close is
still covered earlier in the same app workflow. The corrected full matrix and
consolidated Windows artifact are pending. The screenshot archive could not be
inspected here because Actions artifact blob downloads return HTTP 403. See
[`PHASE8-STUDIO-PROGRESS.md`](PHASE8-STUDIO-PROGRESS.md) for current CI evidence,
limitations and exact continuation steps. Phase numbering and boundaries are
provisional.
