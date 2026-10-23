# Rust Paint Lab

An original Rust digital painting and illustration project, starting with a
small, testable input foundation. This is **not yet a painting application**.
The first executable is a Windows pen-input diagnostic for the Galaxy Book2 Pro
360; hardware behavior has not yet been verified.

## Workspace

- `crates/input` (`paint-input`): OS-independent pointer samples and normalization.
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

## Current boundaries and next step

No painting, GPU canvas, persistence, Android shell, or touch gestures are
implemented. The proposed wgpu/winit/egui/serde stack remains under evaluation.
First collect Galaxy Book pen results, fix any input delivery issues, and then
add deterministic replay plus contact lifecycle/cancellation processing before a
minimal single-stroke canvas experiment. See the architecture document for the
Windows input decision and future platform boundaries.

Third-party notices are documented separately. A license for the original project
has not yet been selected; no open-source distribution grant is implied.
