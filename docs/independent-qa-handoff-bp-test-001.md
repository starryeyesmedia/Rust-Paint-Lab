# Independent QA Handoff — BP-TEST-001

**Review type:** independent, read-only QA. **PR:** [#10](https://github.com/starryeyesmedia/Rust-Paint-Lab/pull/10).
The independent QA reviewer owns no implementation changes. Review the current
PR head; the painting-mode test code was first validated at exact head
`2ced32254d62504b64c3c110d437d0b5636987e8`.

## Verified engineering evidence

At `2ced32254d62504b64c3c110d437d0b5636987e8`, GitHub workflow run
[41270730757](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41270730757)
completed successfully for Ubuntu, Windows, and macOS. Formatting, strict
workspace Clippy, build, and the configured Linux/Windows smoke steps passed.
This verifies the exact checkpoint source and automated scenarios only; the
compatibility-fixture additions in this PR's later head require their own CI run.

The same-fixture integration tests in
[`painting_modes.rs`](../crates/document/tests/painting_modes.rs) call the public
document `Engine` directly. They clone the same layered state for Brush, Smudge,
and Color Mixer, then assert selected exact RGBA outputs and pairwise whole-image
difference. Additional assertions cover supported Mixer strength values,
Brush/Smudge strength and pressure, `None` pressure fallback, partial alpha,
transparent pixels, premultiplied layer tiles, selection/layer bounds,
preview cancellation, commit, Undo/Redo, and exact `.rpl` state/RGBA after
reopen.

The historical-format tests in
[`legacy_compatibility_fixtures.rs`](../crates/document/tests/legacy_compatibility_fixtures.rs)
exercise source-commit-generated V1–V4 samples and a version-1 `.rplbrush` file.
Those samples are synthetic serializer outputs, not artist-created legacy files.
They test current decode, in-place `.rpl` save, exact backup preservation and
fallback, and current journal/recovery round trips from the decoded states.

## Limits and false-positive risk

These tests prove document-engine results and transaction/persistence behavior.
They do not prove that the application toolbar routes the selected control to
the intended mode, that the preview looks the same in the live window, or that
Windows hardware input uses the same samples. They do not validate physical
S Pen pressure, tilt, hover, lift, barrel button, palm rejection, latency,
color-managed display output, or artist acceptance.

The painting tests compare full outputs for inequality, but only pin selected
pixels to expected values. A future implementation could match those checkpoints
and still produce wrong pixels elsewhere along the stroke. The lifecycle tests
derive the committed/redo/reopen expectation from the same engine run, so they
prove consistency and reversibility rather than independently checking every
artist-facing pixel. Smudge flow 0 vs 1 is required to change output, but the
tests do not define which direction source pickup must move. Confirm that
behavior contract with Backend and the Engineering Lead before adding a
directional flow assertion; do not freeze an arbitrary current algorithm detail.

## QA disposition

Review the current PR head and its exact-head CI. Treat the pixel fixtures as
document-engine regression evidence, not closure of an owner finding. **Do not
mark BP-010 (Smudge) or BP-011 (Color Mixer) fixed** from these tests or CI alone.
The findings remain open until the intended tool behavior passes independent
review and the Product Owner accepts it on the target Windows device. CI does not
establish S Pen compatibility.
