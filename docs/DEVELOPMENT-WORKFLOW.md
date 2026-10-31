# BitchPixel development and pull-request workflow

**Current integration baseline:** The owner merged verified alpha baseline PR #6.
`main` is the alpha integration branch at
`ac375418dfe4c833b932bcaecee785810532681f`. This establishes an alpha development
baseline, not a production release. Releases, code signing, and licensing decisions
remain separate owner-approved actions.

## Branch roles

- `main` is the integration target for all new work. Do not commit directly to it.
- Start each assignment from the latest fetched `main` on a short-lived
  `feat/<topic>`, `fix/<topic>`, `test/<topic>`, `docs/<topic>`, or
  `chore/<topic>` branch.
- Keep one focused PR per coherent feature or correction. A coherent multi-step
  implementation may use several commits in one PR.
- `alpha/verified-baseline-2b3728c` points at the exact pre-merge verified
  candidate. Preserve it as historical provenance; do not move it.
- `feature/studio-overdrive-phase-8` preserves the legacy development history.
  Do not rewrite, force-push, or delete it. New assignments start from `main`.

Preserve the full artwork and repository history. Do not squash or rebase the
baseline history. Keep merge commits enabled; do not require linear history.

## Pull-request workflow

1. Fetch `origin/main`, create a short-lived branch from that exact current tip,
   and keep the change focused. Never commit directly to `main`.
2. Preserve existing painting behavior, native artwork compatibility, recovery,
   Undo/Redo, exports, and other working functionality. Explain and test any
   intentional changes to these guarantees.
3. Add behavior-focused automated tests. Never remove meaningful assertions or
   weaken tests to obtain a passing result.
4. Open a draft PR targeting `main` when the work is meaningfully underway. Use
   the repository PR template to report behavior, compatibility and recovery
   effects, test results, CI links, screenshots for UI changes, and limitations.
5. Before proposing merge, pass all required platform checks on the PR commit:
   `check (ubuntu-latest)`, `check (windows-latest)`, and
   `check (macos-latest)`. Diagnose failures on the PR branch.
6. An eligible human reviewer must review and approve the change before merge.
   Codex must not self-merge. If a platform or hardware check is unavailable,
   record that limitation and exact continuation steps rather than claiming it
   passed.
7. Do not publish a release, make a licensing decision, or alter repository
   protection settings without separate owner authorization.

For interface changes, generate populated screenshots, inspect them, and link the
evidence in the PR. Explicitly state which physical hardware and accessibility
behaviors remain untested. Passing CI or synthetic tests does not establish
physical pen, display-scaling, Narrator, or NVDA behavior.

## Required platform checks

- `check (ubuntu-latest)`: formatting, strict Clippy, workspace tests and build,
  GPU smoke, Linux Xorg/XTest artwork and recovery workflows, and Linux package.
- `check (windows-latest)`: Windows build/tests, visible canvas startup smoke,
  and portable canvas/stylus artifacts.
- `check (macos-latest)`: macOS build/tests, startup smoke, and alpha package.

A check is required only when its workflow is configured for the PR, but all three
platform jobs must pass before merge. If a workflow does not run or a check is
unavailable, report that instead of treating it as passed.

## Recommended `main` branch rules (administrator action)

An owner or repository administrator can configure these settings under
**Settings → Rules → Rulesets → New branch ruleset**, targeting the `main` branch.
These recommendations have not been enabled by this document:

- Require a pull request before merging; no direct pushes to `main`.
- Require the three platform status checks listed above and require review
  conversations to be resolved.
- Require one independent approving review when another eligible reviewer is
  available. A PR author cannot approve their own PR. If the owner is the only
  eligible reviewer, either add an eligible reviewer or set the required-review
  count to zero while retaining the PR and CI requirements; the owner still
  performs an explicit manual review before merging. Do not create a rule that
  makes the sole owner unable to merge.
- Block force pushes and branch deletion.
- Keep bypass permissions minimal and exceptional, limited to the owner/admin
  when necessary for repository recovery.
- Keep merge commits enabled in **Settings → General → Pull Requests**. Do not
  require linear history, since that conflicts with preserving merge history.

Changing these settings requires repository administrator access. Codex must not
change them without explicit owner authorization.
