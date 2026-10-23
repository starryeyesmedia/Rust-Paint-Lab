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
