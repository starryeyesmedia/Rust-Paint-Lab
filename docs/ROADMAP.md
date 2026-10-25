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
The current local slice enables AccessKit event/tree integration and adds a
persisted draggable/snap-docked palette with collapse and handedness preferences.
The next local slice adds visual document-format cards and a live paper/background
preview plus a dedicated, confirmation-safe recovery manager. The recovery manager
shows validated document metadata but does not yet render previews. Full platform
CI is pending. These checkpoints do not complete the Phase 8.3 workspace overhaul. See
[`PHASE8-STUDIO-PROGRESS.md`](PHASE8-STUDIO-PROGRESS.md) for the exact tested
checkpoint and continuation steps. Phase numbering and boundaries are provisional.
