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

- [ ] This is one focused feature or correction, and its `feat/*`, `fix/*`, `test/*`, `docs/*`, or `chore/*` branch starts from current `main`.
- [ ] PR targets `main`; no changes were committed directly to `main`.
- [ ] Existing painting behavior, editable artwork compatibility, recovery, and other working functionality are preserved or their intentional changes are explained and tested.
- [ ] Untested physical hardware and accessibility behavior are explicitly documented; CI or synthetic tests are not presented as device validation.
- [ ] This is alpha development, not a production release; no release or licensing change is implied by merging it.
- [ ] No telemetry, forced account, subscription, or paid feature gates; third-party notices and original content rights are preserved.
- [ ] `check (ubuntu-latest)`, `check (windows-latest)`, and `check (macos-latest)` are green on this PR commit before merge.
- [ ] An eligible human reviewer approved the PR before merge; Codex must not self-merge.
