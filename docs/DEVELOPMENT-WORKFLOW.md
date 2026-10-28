# BitchPixel development and pull-request workflow

**Status:** Proposed in a pull request. The repository is not protected by this document alone. Branch rules must be enabled separately in GitHub settings.

## Branch roles

- `main`: stable, explicitly approved integration and eventual release history. It presently does **not** contain the active painting application; do not merge unfinished development into it to establish this workflow.
- `feature/studio-overdrive-phase-8`: the existing **integration branch** for ongoing BitchPixel application development. Use it as the PR target until the owner explicitly adopts a new integration branch such as `develop`.
- `feat/<topic>`, `fix/<topic>`, `docs/<topic>`, `chore/<topic>`: short-lived branches for cohesive work, cut from the current integration HEAD.

The current integration history was built by checkpoint commits directly on that branch. Do not rewrite or rebase that historical work merely to start using PRs.

## Codex workflow

1. Confirm the integration branch HEAD and read `AGENTS.md`, the roadmap, and the current engineering progress notes. Check for uncommitted and concurrent work.
2. Create a **new short-lived branch** from the latest integration commit for each substantial feature or cohesive fix. Never push directly to `main` or the integration branch.
3. Implement vertical slices and commit/push checkpoints on **that work branch**, allowing multi-hour work without an uncommitted backlog. Keep unrelated features in separate branches/PRs.
4. Open a **draft PR** targeting `feature/studio-overdrive-phase-8` once the work is meaningfully underway. Continue commits on that PR branch; pushing updates the PR and triggers CI.
5. Fill out the repository PR template with scope, tests, format compatibility, screenshots when appropriate, CI links, and unresolved risks. All claims must reflect actual verification.
6. Diagnose and fix failed CI on the PR branch. Do not remove tests, falsely mark a skipped hardware check as passed, or loosen pixel assertions merely to turn a run green.
7. Mark the PR ready for review only when the work is coherent and available automated checks are green. The owner decides whether and when to merge. Codex must **not** self-merge, force-push to protected branches, or publish releases without explicit authorization.
8. If testing or environment restrictions prevent completion, leave a clearly marked draft PR with the failing checks, commits, and continuation notes.

A single PR may contain several tested checkpoints when they form one cohesive feature; **do not create one PR for every individual commit**.

## Merge gate

Before an integration-branch merge, require:

- Successful `check (ubuntu-latest)`, `check (windows-latest)`, and `check (macos-latest)` jobs for the relevant PR head.
- Existing required format/lint/tests/build and Linux UI/artwork/recovery workflows as applicable.
- Review of effects on native file versions, undo, recovery, interchange and possible data loss.
- Honest manual-device and accessibility status. CI cannot prove Samsung S Pen behavior or Windows Narrator/NVDA support.
- Explicit human review and merge approval, separate from Codex's implementation.

Changes limited to documentation may still run CI because the existing workflow runs on every push and PR.

## GitHub enforcement to configure manually

Under **Settings → Rules → Rulesets** (or available branch-protection UI), create rules for `main` and the existing integration branch. Prefer:

- Require pull requests for changes to the protected branches.
- Require the three platform-specific GitHub Actions checks before merge.
- Block force pushes and branch deletion.
- Require an independent approving review where a reviewer is available and the repository plan supports it.
- Make emergency bypass exceptional, auditable, and owner-controlled.

These settings are **not implemented merely by merging this document**. GitHub administrative access is required to configure rulesets. Check the exact status-check names when creating the rule.

The pull-request template must be on GitHub's **default branch** (`main`) to be available by default when contributors create PRs; it is proposed separately for that branch.

## Release and licensing separation

Do not merge the integration branch into `main` or create a public release without specific owner approval. The separately proposed GPLv3 licensing/copyright updates live in their own draft PRs and should be reviewed independently. Preserve all third-party license notices and rights.
