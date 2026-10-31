# BitchPixel development and pull-request workflow

**Status:** The owner approved a transition to PR-based development targeting
`main`. The exact alpha baseline is pinned at
`alpha/verified-baseline-2b3728c`, commit
`2b3728c2f8316113a1a409c5fec43562c1c5e1b0`, verified by Windows, Linux, and
macOS in [Actions run 41153331593](https://github.com/starryeyesmedia/Rust-Paint-Lab/actions/runs/41153331593).
The baseline is an alpha development integration, not a production release.
`main` becomes the integration branch only after its baseline PR is explicitly
reviewed and merged by the owner. This document alone does not enable GitHub
branch protection.

## Branch roles

- `main`: the approved alpha integration branch after the baseline PR merges.
  All subsequent feature, fix, and documentation PRs target `main`.
- `alpha/verified-baseline-2b3728c`: an immutable branch pointing to the exact
  verified baseline commit. Never add later development commits to this branch.
- `feature/studio-overdrive-phase-8`: the preserved legacy development history
  and continuation point through the baseline. Do not rewrite, force-push, or
  delete it. Once `main` contains the baseline, continue unfinished approved
  Inspector work from a short-lived branch based on `main`.
- `feat/<topic>`, `fix/<topic>`, `docs/<topic>`, and `chore/<topic>`:
  short-lived branches cut from the latest `main` after the baseline merge.

The pinned baseline branch is a descendant of the current `main` history. Use a
normal merge commit to preserve its complete commit ancestry; do not squash or
rebase the baseline history. A merge does not publish a release, sign a binary,
or make a licensing decision.

## Codex workflow

1. Before the baseline merge, preserve the approved candidate and all existing
   development history. The owner reviews and merges the pinned baseline PR.
2. After that merge, fetch the latest `main` and create a new short-lived
   `feat/*`, `fix/*`, `docs/*`, or `chore/*` branch for each focused assignment.
   Never commit directly to `main` or continue pushing new assignments onto
   `feature/studio-overdrive-phase-8`.
3. Implement tested vertical slices and push checkpoints to the same work
   branch. Separate unrelated work into focused PRs; one PR may contain several
   checkpoints that form a coherent change.
4. Open a draft PR to `main` once the change is meaningfully underway. Use the
   repository PR template for behavior, compatibility, test results, CI links,
   screenshot review where applicable, and unverified hardware limitations.
5. Diagnose and fix failed CI on the PR branch. Never remove meaningful tests
   or weaken assertions merely to obtain a green run. Mark a PR ready only when
   it is coherent and required automated checks have passed.
6. The owner performs human review and decides whether and when to merge.
   Codex must not self-merge, create releases, or make licensing decisions.
7. If testing is blocked, leave the work as a draft PR with its exact failing
   check, commit, and continuation steps.

## Merge gate

Require these checks on the relevant PR commit:

- `check (ubuntu-latest)` — formatting, strict Clippy, workspace tests/build,
  GPU smoke, Linux Xorg/XTest artwork, recovery, and Linux package workflows.
- `check (windows-latest)` — Windows build/tests, visible canvas startup smoke,
  and portable canvas/stylus artifacts.
- `check (macos-latest)` — macOS build/tests, startup smoke, and alpha package.

Review document-format and recovery effects, Undo/Redo, exported pixels, and
artwork preservation. Include actual screenshots for interface changes and
describe what they demonstrate. CI does not prove physical S Pen behavior,
native Windows display scaling, Narrator, or NVDA.

## GitHub rules to configure

Under repository **Settings → Rules → Rulesets** (or branch protection), create
a rule for `main` that:

- Requires pull requests and all three platform checks above.
- Requires one approving review where a separate reviewer is available, and
  dismisses stale approvals after new commits.
- Requires conversation resolution and (optionally) the PR branch to be current
  with `main` before merge.
- Blocks force pushes and branch deletion.
- Limits any bypass to the owner/admin and keeps bypass exceptional.

Do not require linear history if that would force squash/rebase of the approved
baseline. An administrator must configure and save these GitHub settings; they
cannot be enabled by repository documentation or a PR alone. If the project is
operated by one reviewer, the owner must decide whether to require an external
approver or use an owner-controlled bypass.

## Existing workflow drafts

PR #4 adds the PR template to `main` and should use the main-target policy above.
PR #5 is currently a documentation PR based on the legacy feature branch; it
describes the transition and must be retargeted to `main` only after the pinned
baseline has landed. Neither draft is merged automatically.

## Release and licensing separation

Merging the alpha baseline or a later development PR is not a production
release. Releases, code signing/notarization, and licensing/copyright decisions
remain separate owner-approved actions. Preserve third-party licenses and
attributions.
