## Summary

What changed, and what artist-facing problem does it solve?

## Implementation and compatibility

- Affected components (input, stroke, document, rendering, UI, persistence):
- Native `.rpl` format version / migration impact:
- Existing artwork, recovery, Undo/Redo, and interchange considerations:
- Accessibility, keyboard, touch, or stylus considerations:
- New/updated dependencies and license notices (if any):

## Validation

Mark only checks that actually passed. Include failure details and links for skipped or incomplete checks.

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] `cargo build --workspace --locked`
- [ ] Relevant GPU / pixel-level / save-reopen / recovery tests
- [ ] Windows GitHub Actions job passed
- [ ] Linux GitHub Actions job passed (including applicable Xorg/XTest workflow)
- [ ] macOS GitHub Actions job passed

**CI run link(s):**

## Visual and device testing

Screenshots / rendered output reviewed (if UI changed):

Physical S Pen, Windows accessibility, performance, or other hardware testing completed or outstanding:

## Known limitations and risk

What isn't implemented, remains unverified, or might regress?

## Reviewer checklist

- [ ] PR targets the current integration branch, not `main`, unless this is a separately approved release/legal/docs change.
- [ ] No unapproved breaking document-format changes or silent loss of editable artwork.
- [ ] No telemetry, forced account, subscription, or paid feature gates.
- [ ] Third-party license notices and original content rights are preserved.
- [ ] All required CI jobs are green before merge.
- [ ] Explicit human approval before merge; Codex must not self-merge.
