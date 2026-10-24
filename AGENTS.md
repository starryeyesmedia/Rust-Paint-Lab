# Rust Paint Lab: contributor and agent guidance

## Product and scope
Build an original, cross-platform painting and illustration application inspired
by Fresco, Krita, and Procreate. Initial hands-on target: Galaxy Book2 Pro 360
(Windows 10, Intel Iris Xe, 16 GB). Future targets: Windows/Linux/macOS, Android
with S Pen, and eventually iPadOS. Favor low latency, accessibility, reliable local
autosave/recovery, open formats, and modest resource use. No telemetry, accounts,
or cloud service requirements. Do not attempt the full product in one task.

## Architecture boundaries
- `crates/input`: safe, OS-independent input structures, normalization, bounded lifecycle processing, and deterministic replay. No
  Win32, windowing, UI, GPU, filesystem, or application policy dependencies.
- `crates/stroke`: OS-independent painting routing policy, document-coordinate
  mapping, and stroke transaction commands. Depends on `paint-input`, never the
  other way around. No brush algorithms, GPU resources, document storage, or IO.
- `apps/canvas-demo`: winit/wgpu shell with separate live mouse and replay sessions,
  bounded transaction state, CPU polyline geometry, and GPU rendering. Graphics
  dependencies stay here. The Windows capture observer is isolated in the binary;
  no OS APIs enter shared input/routing logic. No pen integration, persistence,
  layers, or brush algorithms in the current mouse milestone.
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
  before f64-to-GPU conversion. Demo width is fixed, independent of pressure.
- Playback generation is deterministic; only the shell supplies elapsed demo time.
  Scenario restart must clear retained replay state. Live samples may remain in
  bounded session memory to render strokes; never automatically persist or log them.
- Run CPU geometry tests plus the offscreen runner. A missing GPU is explicitly
  SKIPPED, not a passed unit test; Linux CI requires software GPU execution via
  RPL_REQUIRE_GPU=1 and tests the window under Xvfb. See docs/canvas-demo.md.
- Preserve both separate Windows artifacts and their notices; use
  scripts/canvas-licenses.py when dependency versions change.

## Live mouse conventions
- Feed mouse samples through Processor and Router; primary button only, with
  pressure/tilt None. Never paint from hover, right-click, or an orphan Move.
- Use physical CursorMoved positions and physical inner sizes without multiplying
  DPI twice. Reuse the inverse canvas mapping, capture it per stroke, and cancel
  active input before resize/DPI/mode changes.
- Cancel on focus/window/canvas exit or capture loss. Require a new Down after
  interruption. Keep live/replay scene namespaces separate and retain completed
  live work across mode switches; clear is an explicit user action.
- Capacity errors cancel previews and show feedback, not a fatal app exit.
- Capture FFI is confined to windows_capture.rs with documented unsafe blocks;
  do not add competing loops or native pen APIs. Winit remains capture owner.
- Run mouse tests, GPU pixel validation, and Linux Xvfb/XTest interaction smoke.
  Distinguish these from pending manual Windows mouse/capture validation.

## Development conventions
Use the pinned Rust toolchain and Cargo workspace. Prefer safe Rust, small
interfaces, established maintained dependencies, and explicit errors. Keep
platform dependencies target-gated. Add dependencies only for current needs;
update Cargo.lock and `docs/dependencies.md` with versions/licenses and notices.
Do not select a project distribution license without the owner's decision.
Keep features focused; no layers, vectors, natural media, or complex brush engine
until separately scoped. Do not add device identifiers to persistent logs by default.

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

## Git
Work on feature branches, never directly on main. Preserve existing work. Do not
merge or create releases unless separately requested. Keep commits reviewable.
