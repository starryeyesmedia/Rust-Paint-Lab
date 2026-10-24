# Automated input validation

Use the pinned Rust toolchain, from the workspace root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo run --locked -p paint-input --example replay -- crates/input/tests/fixtures/mixed.replay
```

The input crate contains seven normalization unit tests plus fourteen processing
integration tests. They exercise exact output sequences/data; simultaneous touch
and pen; hover/contact/lift; varying and zero/unsupported pressure/tilt; cancellation,
capture loss, interruptions and replay EOF; reused IDs and duplicate/unexpected
sequences; stale terminal cleanup; equal/stale timestamps and 32-bit Windows
rollover; invalid input and parser/version/resource limits; and bounded state over
50,000 generated events. Replay output is compared across repeated executions.
Tests do not sleep, read wall time, use network, or require hardware. Fixture
limits are tested at the parser boundary; long-stream tests do not retain outputs.
The native diagnostic binary itself has no automated hardware test suite.

`paint-stroke` adds nineteen routing/coordinate integration tests, for **40 total
workspace tests**. Existing fixtures exercise exact Begin/Update/Commit/Cancel
sequences, pressure changes, missing versus zero data, hover and barrel buttons,
pen/touch mixing, ID reuse, and interruptions/capture loss. A synthetic mouse
fixture covers primary-button gating and unavailable mouse pressure/tilt. Direct
lifecycle boundary tests cover duplicates, orphans, stale outputs, malformed
values, and independent transactions. Coordinate tests cover translation, zoom,
viewport changes during a stroke, invalid values, and arithmetic overflow.
Additional tests verify dispatch/source timing, bounded state over 10,000 strokes,
explicit router teardown, and identical command streams on repeated replay.
These checks do not implement or validate a renderer, brush engine, or document
consumer. Cancellation is asserted as a distinct command, never a Commit.

For focused development:

```sh
cargo test --locked -p paint-input --test processing
cargo test --locked -p paint-stroke --test routing
cargo run --locked -p paint-stroke --example stroke_commands
```

On Linux/macOS also check the Windows adapter without linking/executing:

```sh
rustup target add x86_64-pc-windows-gnu
cargo check --workspace --locked --target x86_64-pc-windows-gnu
cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings
```

Existing CI checks format/lint/tests/native builds on Windows, Linux, and macOS.
The Windows leg additionally builds and packages the release diagnostic; it does
not exercise a physical pen. The downloadable package includes an updated short
README, with hardware tests in [the full checklist](hardware-testing.md).
Do not infer actual Windows execution from cross-compilation, or pen behavior
from either native builds or synthetic replay. The Galaxy Book checklist is still
pending and is independent of completing the automated processing milestone.

## Phase 4 canvas checks

The canvas adds twelve hardware-independent tests, bringing the Rust suite to
**52 tests**. They cover the actual routed A/B/C fixtures, geometry and document
mapping, final samples and retained pressure, preview/commit/cancel transitions,
independent IDs/touch exclusion, nonfinite/range errors, fixed width/zero-length
segments, duplicates, restart determinism, and explicit sample/stroke/vertex limits.

```sh
cargo test --locked -p canvas-demo --test geometry
cargo test --locked -p canvas-demo --test gpu_smoke
cargo run --locked -p canvas-demo
```

The offscreen smoke runner is an additional custom harness, outside Rust's unit
passed-test count. It prints **GPU_SMOKE: PASSED** after real pixel checks or
**GPU_SMOKE: SKIPPED** if no adapter exists. After finding an adapter, device,
shader, rendering, readback, or pixel failures fail the command. Set
`RPL_REQUIRE_GPU=1` to make absence fail too; Linux CI does so with Mesa installed.
The runner records its actual adapter/backend and appends outcome to the Actions
step summary. Do not describe a skip as passing GPU validation.

Linux CI additionally runs the actual window under a virtual display:

```sh
xvfb-run -a cargo run --locked -p canvas-demo -- --exit-after-demo
```

The normal demo stays open; this testing flag exits only after replay completion
and presentation of the final frame. Use an external timeout in automation (CI
limits this step to two minutes). Software-rendered/offscreen/virtual-window tests
are not physical RTX 3060, Iris Xe, or stylus validation. The downloadable canvas
and stylus diagnostic remain distinct artifacts. See the [manual checklist](canvas-demo.md).

## Phase 5 live mouse validation

Fifteen mouse/session integration tests bring the hardware-independent Rust suite
to **67 tests**. They run the real sample/lifecycle/router/scene pipeline and cover
primary/secondary/hover/orphan input, final samples and unavailable pressure/tilt,
focus/capture/window exit, nonfinite/outside points, duplicate events, ID reuse,
resize and physical-pixel mapping, mode isolation, replay/clear semantics, and
nonfatal bounded-state cleanup. The GPU runner additionally checks live preview,
commit/cancel pixel regions and scissor clipping at the paper edge.

```sh
cargo test --locked -p canvas-demo --test mouse
cargo test --locked -p canvas-demo --test gpu_smoke
xvfb-run -a python3 scripts/live-mouse-window-smoke.py
```

The Linux window test needs libXtst (`libxtst6` on Ubuntu) in addition to the
existing display prerequisites. It injects real X11 button/motion/focus/key events
into the normal winit executable, checks transaction/mode feedback, and exits
cleanly. It does not use an application test-input shortcut or record user data.
Both virtual-window tests and required software-GPU validation remain in CI.

The isolated Windows capture subclass observes actual WM_CAPTURECHANGED/CANCELMODE,
while suppressing expected release during normal WM_LBUTTONUP. It is cross-checked/
linted for Windows and built in native CI; theft/release behavior still needs
manual Windows validation. Linux/macOS use winit focus/leave notifications; capture
stealing without either notification is not independently observed there.
See [Galaxy Book interactive checklist](live-mouse.md#manual-galaxy-book-checklist).


## Overdrive alpha validation

`cargo test -p paint-document --locked` tests brush determinism, pressure semantics,
opacity versus flow, endpoint smoothing, scratch cancellation/concurrency, layers,
bounded history, blend invariants, corrupted/truncated native files, injected save
failures, external-change conflicts, recovery ownership/flush and PNG round trips.
The existing offscreen runner now also checks real raster shader pixels across
preview, cancellation, commit, undo and redo. A missing adapter remains SKIPPED;
Linux CI requires software GPU execution. Existing 70 Rust tests are preserved.

```
cargo run --release --locked -p paint-document --example metrics
xvfb-run -a python3 scripts/alpha-workflow-smoke.py
```

The latter injects real OS mouse/key events into the default alpha, verifies a
checksummed editable save, undo/redo tile results, backup, PNG dimensions and reopen.
It uses a temporary working/recovery directory. The older mouse window script uses
`--compat-mouse` to preserve its original regression contract. These tests are not
physical pen, Windows filesystem, Iris Xe/RTX frame-rate or tablet UX certification.

History hardening adds unique COW allocation accounting and 4 storage/history tests
(91 total Rust tests). Run the same workspace suite and target-specific license
notice generator when packaging additional desktop platforms.

Selection/editing adds seven portable tests (98 total). The real alpha window
workflow additionally selects a rectangle, fills, saves, undoes and verifies the
original native file is restored byte-for-byte. Physical pen editing remains untested.
