# Rust Paint Lab

An original Rust digital painting and illustration project, starting with a
small, testable input foundation. This is **not yet a painting application**.
The first executable is a Windows pen-input diagnostic for the Galaxy Book2 Pro
360; hardware behavior has not yet been verified.

## Workspace

- `crates/input` (`paint-input`): OS-independent pointer samples, lifecycle processing, and deterministic replay.
- `crates/stroke` (`paint-stroke`): portable stroke commands, routing policy, and client-to-document coordinates.
- `apps/canvas-demo`: progressive replay-driven wgpu canvas, separate from live input.
- `apps/stylus-diagnostic`: native Windows pointer inspector.
- [Architecture](docs/architecture.md), [dependencies and licenses](docs/dependencies.md),
  [hardware test checklist](docs/hardware-testing.md), [agent guidance](AGENTS.md).

## Prerequisites

Install [Rust via rustup](https://rustup.rs/). `rust-toolchain.toml` pins Rust
1.90.0 with Clippy and rustfmt; Cargo selects it automatically in this checkout.
On Windows use the **x86_64-pc-windows-msvc** toolchain, Windows 10 version 1703
or newer, and Visual Studio 2022 Build Tools with **Desktop development with C++**
(including the Windows SDK and MSVC x64 build tools). A GPU SDK is not needed.
Linux needs a C linker (for example `build-essential` on Ubuntu); macOS needs
Xcode Command Line Tools. Shared tests work on all three desktop platforms.
No secrets, services, accounts, or runtime downloads are required.

## Run the downloadable Windows diagnostic

No local Rust or Visual Studio installation is needed for the CI-built package.
On GitHub, open **Actions → Rust checks**, select a successful run for
`feature/gpu-canvas`, then download **rust-paint-lab-windows-diagnostic**
from its Artifacts section (a GitHub sign-in/repository access may be required).
Extract the ZIP to a normal user folder and follow its `README.txt`.

The package contains the x64 release executable, instructions, dependency notice,
build information, and executable SHA-256 checksum. It targets Windows 10 1703+
and statically links the C runtime to avoid a separate VC++ redistributable.
No installer or elevation is required. It is unsigned; SmartScreen or organization
policy may warn or block it. Artifacts expire after 30 days (subject to repository
policy) and are development builds, not GitHub releases. See the
[packaged instructions](docs/windows-diagnostic-README.txt).

## Build and run on Windows

From PowerShell in the repository root:

```powershell
rustup show
cargo build --locked -p stylus-diagnostic
cargo run --locked -p stylus-diagnostic
# Optional optimized build:
cargo build --locked --release -p stylus-diagnostic
.\target\release\stylus-diagnostic.exe
```

Use the pen over the window's client area. The display shows the latest pointer
position (physical client pixels), pressure (0–1 or `None`), tilt axes (degrees or
`None`), buttons, contact/range status, raw masks/flags, message code, event time,
frame, history count, active contact IDs, and ten recent events. Native errors
appear in the display; startup errors appear in the terminal. Mouse movement is
not used to fake pen data. Touch contacts are reported if delivered by Windows;
there is no gesture recognizer or palm-rejection policy yet. Nothing is saved.

## Automated checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

On Linux/macOS, the diagnostic builds a stub that exits with an explanatory
message and status 1; this is not a native diagnostic on those platforms.
To check Windows-specific code on Linux without executing or linking it:

```sh
rustup target add x86_64-pc-windows-gnu
cargo check --workspace --locked --target x86_64-pc-windows-gnu
cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings
```

CI performs formatting, lint, tests, and native builds on Linux, Windows, and
macOS. A successful build does not verify a stylus or driver. Follow the
[Galaxy Book checklist](docs/hardware-testing.md) before making hardware claims.

## Synthetic input processing and replay

The shared processor handles bounded contact lifecycles, hover, cancellation,
capture loss, interruption, and reused IDs. Versioned text fixtures replay without
hardware or wall-clock timing; nothing is automatically recorded. Run:

```sh
cargo run --locked -p paint-input --example replay -- crates/input/tests/fixtures/pen.replay
```

See the [fixture format](docs/replay-format.md), [processing architecture](docs/architecture.md),
and [testing instructions](docs/testing.md). The diagnostic now displays shared
lifecycle outcomes; its native event loop and capture behavior remain unchanged.

## Stroke commands

`paint-stroke` converts ordered lifecycle outputs into provisional Begin/Update
and terminal Commit/Cancel commands. Pen paints, touch is reserved, and mouse
painting requires the primary button without synthesizing pressure or tilt.
A validated viewport maps physical client pixels to document coordinates and is
captured per stroke. Run the existing synthetic pen fixture through the router:

```sh
cargo run --locked -p paint-stroke --example stroke_commands
```

See [routing and transaction semantics](docs/stroke-routing.md). The Windows
inspector remains a lifecycle diagnostic; it does not paint or route brush work.

## GPU canvas demo

Download **rust-paint-lab-canvas-demo** from a successful `feature/gpu-canvas`
Actions run, extract, and run `canvas-demo.exe` on Windows 10 x64 with a compatible
graphics driver. No local Rust, installer, or administrator rights are needed.
It is separate from **rust-paint-lab-windows-diagnostic**, which remains available.

From source:

```sh
cargo run --locked -p canvas-demo
```

Keys **1/2/3** select completed/cancelled/concurrent replay scenarios, **R** restarts,
**Space** pauses, **Escape** exits. Orange means provisional, blue means committed;
cancellation removes orange geometry. No live painting is implemented.
[Canvas documentation](docs/canvas-demo.md) includes Linux prerequisites, controls,
resource/error policy, automated test distinctions, and the RTX 3060 checklist.

## Current boundaries and next step

The canvas renders routed synthetic commands; it is not yet a live painting app.
There is no brush engine, persistence, Android shell, or gesture system. egui and
serde remain prospective dependencies. Physical RTX 3060/Iris Xe/stylus tests are
pending. Next add live mouse input through the shared pipeline, with pressure and
tilt unavailable, before native pressure-sensitive pen integration. See the
[Phase 5 scope](docs/canvas-demo.md#recommended-phase-5-live-mouse-before-pen).

Third-party notices are documented separately. A license for the original project
has not yet been selected; no open-source distribution grant is implied.
