# BitchPixel: contributor and agent guidance

## Product and scope
Build BitchPixel, an original, cross-platform painting and illustration
application inspired by Fresco, Krita, and Procreate. Rust Paint Lab remains the
engineering repository and existing internal package/format name until a safe
migration is planned. Approved product lines are “No limits. No bullshit.” and
“Create without permission.” Initial hands-on target: Galaxy Book2 Pro 360
(Windows 10, Intel Iris Xe, 16 GB). Future targets: Windows/Linux/macOS, Android
with S Pen, and eventually iPadOS. Favor low latency, accessibility, reliable local
autosave/recovery, open formats, and modest resource use. No telemetry, accounts,
or cloud service requirements. Studio Overdrive requires that all application
functionality remain free, with no subscription/paywall, advertising, tracking, or
forced online dependency. Overdrive explicitly authorizes a multi-checkpoint
program; deliver tested vertical slices and maintain
`docs/PHASE8-STUDIO-PROGRESS.md`. `docs/PHASE7-STUDIO-PROGRESS.md` is the prior
phase's historical checkpoint and remains available for context.

## Architecture boundaries
- `crates/input`: safe, OS-independent input structures, normalization, bounded lifecycle processing, and deterministic replay. No
  Win32, windowing, UI, GPU, filesystem, or application policy dependencies.
- `crates/stroke`: OS-independent painting routing policy, document-coordinate
  mapping, and stroke transaction commands. Depends on `paint-input`, never the
  other way around. No brush algorithms, GPU resources, document storage, or IO.
- `crates/document`: safe OS/GPU/GUI-independent raster brushes, sparse tiles,
  editable layers, bounded transactions/history, checked storage, recovery and PNG.
  Do not merge provisional pixels into authoritative artwork before Commit.
- `apps/canvas-demo`: winit/wgpu/egui painting shell, dirty-tile presentation,
  live input and separate deterministic replay. Native APIs stay in the isolated
  Windows subclass. Never bypass Processor/Router for canvas painting.
- `apps/stylus-diagnostic`: disposable diagnostic shell, not the future app core.
  Win32 calls stay in its Windows module with documented unsafe invariants.
- Future core owns document operations; rendering consumes snapshots/commands;
  persistence owns format versioning and crash-safe writes; platform adapters
  translate device data to shared samples. Do not create empty crates merely to
  represent planned components. See `docs/architecture.md`.
- Never synthesize pen pressure from mouse buttons. Preserve unavailable data as
  `None`, distinguish pen/touch/mouse, and retain contact IDs and cancellation.
- Keep pixel/DIP/document coordinates and time units explicit. Preserve input
  history before adding stroke smoothing or gesture arbitration.

## Input processing and replay conventions
- Keep one Processor per ordered adapter stream; explicitly cancel on capture loss
  and interrupt on application/session teardown. Do not infer missing Up events
  from wall-clock time or synthesize a Down for orphan Moves.
- Preserve bounded state and deterministic output order. Consumer code must handle
  cancellation separately from completion and must not retain unbounded live logs.
- Replay v1 is documented in `docs/replay-format.md`. Fixtures are synthetic;
  never automatically record user input. Version incompatible format changes.
- Include expected outputs, missing-versus-zero values, interruptions, ID reuse,
  timestamp ordering/rollover, and long-stream cleanup in processing tests.
- Shared lifecycle/replay changes must pass the hardware-free suite in
  `docs/testing.md`; hardware validation remains separate and must not block it.

## Stroke routing conventions
- Feed one Router the complete, ordered output of one Processor; do not omit
  cancellations or reuse an old Router for a fresh replay session.
- Begin/Update are provisional. Only Commit completes a transaction; Cancel must
  discard previews. Key consumer state by StrokeId, not a reusable pointer ID.
- Pen paints on contact, touch is reserved, and mouse requires primary-button
  contact. Mouse pressure/tilt remain None. Do not remap barrel/eraser buttons to
  brush behavior without a separately tested policy.
- Capture the viewport at Begin. Coordinate units, error behavior, bounded state,
  and cancellation semantics are documented in `docs/stroke-routing.md`.
- Add exact command-sequence replay assertions for policy changes. Keep render,
  brush, and document responsibilities separate; no GPU tests are needed here.

## Canvas conventions
- Preserve command identity/attributes and include the final Commit sample.
  Never bake provisional geometry irreversibly into committed content.
- Keep explicit stroke/sample/vertex/surface budgets. Reject nonfinite/range errors
  before f64-to-GPU conversion. Legacy replay geometry remains fixed-width; raster brushes explicitly consume pressure.
- Playback generation is deterministic; only the shell supplies elapsed demo time.
  Scenario restart must clear retained replay state. Live samples may remain in
  bounded session memory to render strokes; never automatically persist raw input; local document recovery stores committed artwork snapshots.
- Run CPU geometry tests plus the offscreen runner. A missing GPU is explicitly
  SKIPPED, not a passed unit test; Linux CI requires software GPU execution via
  RPL_REQUIRE_GPU=1 and tests the window under Xvfb. See docs/canvas-demo.md.
- Preserve both separate Windows artifacts and their notices; use
  scripts/canvas-licenses.py when dependency versions change.
- Keep workspace preferences separate from native artwork. Reference-board images
  are UI-only; impose file, dimension, image-count and GPU texture limits before
  decoding/upload, and never feed them to document compositing or export. Persist
  only paths/layout/appearance, not embedded image bytes. Test that recovery and
  exported artwork cannot be modified by reference-board state.

## Live mouse conventions
- Feed mouse samples through Processor and Router; primary button only, with
  pressure/tilt None. Never paint from hover, right-click, or an orphan Move.
- Use physical CursorMoved positions and physical inner sizes without multiplying
  DPI twice. Reuse the inverse canvas mapping, capture it per stroke, and cancel
  active input before resize/DPI/mode changes.
- Cancel on focus/window/canvas exit or capture loss. Require a new Down after
  interruption. Keep live/replay scene namespaces separate and retain completed
  live work across mode switches; clear is an explicit user action.
- A Down outside the paper cannot start a stroke. During contact, clip samples to
  the document boundary and continue the same transaction on re-entry. Mark a
  disconnected subpath so neither previews nor committed rasterization bridge an
  off-paper gap. Focus loss, capture loss, invalid data, resize, and window exit
  remain true cancellation paths.
- Capacity errors cancel previews and show feedback, not a fatal app exit.
- Capture FFI is confined to windows_capture.rs with documented unsafe blocks;
  do not add competing loops; native pen adapters stay isolated here. Winit remains capture owner.
- Run mouse tests, GPU pixel validation, and Linux Xvfb/XTest interaction smoke.
  Distinguish these from pending manual Windows mouse/capture validation.
- Before routing a pointer event away from egui to the canvas, exclude every
  interactive floating `Area`/`Window` under the pointer. Convert physical client
  pixels to egui points using the active pixels-per-point scale. Apply the same
  ownership rule to mouse edits, native pen UI capture, and touch navigation;
  exercise real over-canvas panel clicks in the X11 workflow.
- Persist movable/resizable workspace panel geometry in workspace preferences,
  never in the artwork document. Keep hit-testing, placement, and persisted
  dimensions bounded and finite across resize and display-scale changes.
- Layer rows expose a primary, accessible Actions menu and retain secondary-click
  context menus. Target each duplicate/delete/reorder operation by stable layer ID;
  all edits must pass through document transactions so Undo restores pixels and
  ordering. A dedicated pointer drag grip may reorder rows directly; keep the
  row Actions menu's one-step ordering actions as its keyboard-accessible path.
  Exercise both routes with real X11/XTest input, including right-click opening
  and Undo after each targeted mutation.
- The Linux alpha screenshot runner launches `target/debug/canvas-demo` directly.
  Rebuild `canvas-demo` after source changes before running it; `cargo test` alone
  may leave that executable stale. Verify every required screenshot state, and
  inspect captured pixels through the supported image viewer when available.

## Development conventions
Use the pinned Rust toolchain and Cargo workspace. Prefer safe Rust, small
interfaces, established maintained dependencies, and explicit errors. Keep
platform dependencies target-gated. Add dependencies only for current needs;
update Cargo.lock and `docs/dependencies.md` with versions/licenses and notices.
Project-level GPLv3 licensing and copyright notices are being reviewed in separate draft pull requests. Do not change or merge legal notices, assign copyright, or remove third-party attributions without the owner's approval.
Follow the owner-approved BitchPixel visual direction in the latest concept and
living progress record: purple-led Midnight Violet surfaces, readable coordinated
light/high-contrast variants, Outfit 700/800/900 display faces and Manrope UI
type system. Preserve both Outfit and Manrope OFL texts in every desktop package
notice. Keep the primary palette movable, edge-snapping and accessible; visible
creative controls must call the real engine.
Overdrive authorizes raster brushes, documents/layers/history, safe storage, exports, and usable controls in P0 order; expand only after verified slices. Do not add device identifiers to persistent logs by default.

## Required validation
Before committing run:
```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```
Changes to Win32 code additionally require Windows-target checking/linting, and
native Windows builds in CI. On non-Windows, install the Windows GNU target and
run `cargo check --workspace --locked --target x86_64-pc-windows-gnu` and the
corresponding Clippy command. Cross-checking is not linking or execution.
Add behavior-focused tests for normalization and shared processing, including
missing/invalid capabilities and cancellation when processing is introduced.
Keep tests independent of real devices; document hardware tests separately in
`docs/hardware-testing.md`. Do not claim Windows execution, pen pressure/tilt,
touch, latency, or GPU compatibility based on compilation or synthetic tests.
Record commands, outcomes, and unrun checks honestly.

## Git and PR workflow
The existing `feature/studio-overdrive-phase-8` branch is currently the
integration target for BitchPixel application development; `main` is reserved
for owner-approved stable work. For all **new** feature and bug-fix work:
- Fetch the latest integration HEAD and branch into short-lived `feat/*`,
  `fix/*`, `docs/*`, or `chore/*` work branches before making changes.
- Do **not** commit or push directly to `main` or the integration branch.
  Preserve existing commits and concurrent in-progress work; never force-push
  another contributor's branch.
- Commit, push and validate meaningful checkpoints on the work branch.
  Multi-hour Codex sessions may contain several checkpoints on one coherent PR
  branch; a separate PR for every tiny commit is unnecessary.
- Open a **draft pull request** targeting the integration branch when work is
  meaningfully underway. Populate the PR template with changed behavior, CI
  results, native-format/recovery risks, screenshot review where applicable,
  and explicit device-testing limitations. Keep updating the same PR branch.
- Diagnose failed CI and fix it on the PR branch; do not bypass test failures
  by weakening meaningful checks. Require successful Windows, Linux and macOS
  jobs before proposing a merge. An automated pass does not replace owner
  review or physical S Pen/accessibility testing.
- Codex must **never self-merge**, merge development into `main`, create
  releases, or change protected branch settings without explicit owner approval.
  Leave blocked or unfinished work as a draft PR with a precise continuation.
- Follow `docs/DEVELOPMENT-WORKFLOW.md`. GitHub protection/rulesets must be
  separately enabled by a repository administrator to enforce this policy.

## Overdrive protocol
Maintain `docs/PHASE8-STUDIO-PROGRESS.md` with tested checkpoints, partial/missing
features, measured results, defects and exact continuation steps. Commit each
working slice. Artwork preservation precedes optional expansion. Never count a
control without behavior as a completed feature.

## Artwork preservation
Native format and safety limits are specified in docs/RPL-ALPHA-FORMAT.md. Reject
malformed/oversized input before allocation. Explicit Save must preserve the previous
valid file; Save As must not overwrite unrelated artwork. Keep snapshot recovery per
document, lock concurrent writers, and wait for confirmed durable recovery before
replacing dirty work. Cancelled previews never enter history or storage. Exercise
fault-injection, corruption, round trips and the real alpha window workflow after
storage/UI changes. Never claim snapshots are a full operation journal or full ICC
color management. Recovery also appends bounded, checksummed complete-state records
to a local `.rpl.journal` before replacing the existing recovery snapshot. This
supplement does not contain editing operations or undo history; preserve the `.rpl`
snapshot and verified `.bak` fallback. Do not describe it as comprehensive
crash/power-loss protection without process-kill and platform durability evidence.
Normal close must durably save a named project to its conflict-checked path or an
untitled document as an editable native draft. Keep draft persistence separate from
recovery snapshots, bound draft count/storage, and never silently close after a save
failure. Offer retry, Save As, and cancel. Do not discard old drafts automatically.
See docs/overdrive-user-guide.md for actual supported controls.

Optional SourceCounter is exact adapter-clock metadata with a nonzero frequency;
preserve it through lifecycle and stroke routing. Do not compare unrelated clock
domains or replace normalized ordering with wall-clock time. Replay v1 remains
supported; v2 appends explicit counter ticks/frequency or `- -`. Mouse has None;
never invent timing or pen capabilities. See docs/replay-format.md.
