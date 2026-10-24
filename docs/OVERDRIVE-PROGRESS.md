# Overdrive painting alpha — durable checkpoint

Baseline: e29a98c, Phase 5; GitHub Actions run 40047220819 succeeded on all three desktop platforms. Branch: feature/overdrive-painting-alpha. No release or merge authorized.

## Completed before Overdrive
67 tests, shared lifecycle/replay/router, mouse canvas, GPU pixel checks, Linux real-event window tests, standalone native pen diagnostic, Windows portable artifacts.

## Current work
6A core implemented: WM_POINTER pen history, independent masks/tilt/pressure/buttons, subpixel himetric coordinates, disjoint pointer IDs, bounded queue/deduplication, capture cancellation, promoted mouse suppression. Mouse/replay preserved. Synthetic pen tests added. Shared time remains millisecond resolution; native performance counters are carried to dispatch but not stored in artwork. No prediction/stabilization yet; pressure-responsive rasterization is the next slice. Native hardware validation pending.

## Pending program
6B brushes; 6C documents/layers/history; 6D selections/transforms; 6E vectors; 6F editable saves/recovery; 6G PNG and interchange; 6H timelapse; 6I workspace UI; 6J color/compositing; 6K measured performance; 6L desktop builds; 6M Android; 6N extended QA; 6O packaging. None of these is complete merely because code compiles.

## Decisions
Keep existing crates and the standalone diagnostic. Isolate native APIs in the canvas binary. No empty subsystem crates. Original license remains undecided. No telemetry, accounts, remote storage, signing, or official releases.

## Validation and defects
Latest validated baseline: e29a98c. Checkpoint 6A local validation: 70 Rust tests plus software GPU pixel runner, workspace formatting/lint/build and Windows target check/lint. CI will be checked after pushing. No new physical hardware claims. Physical Overdrive pen tests pending; earlier diagnostic pressure/tilt success is user-reported, not an integrated-canvas test. No new performance measurements yet.

## Exact next steps
1. Complete native pen adapter, synthetic regressions, Windows cross-check/lint, full suite, commit.
2. Add deterministic tiled raster painting/document transactions and exercise through the application.
3. Add bounded history, editable roundtrip saving, previous-file preservation/recovery and PNG before optional expansion.
4. Add actual usable controls for implemented features, update artifacts, run all desktop CI, record exact incomplete requirements here.
