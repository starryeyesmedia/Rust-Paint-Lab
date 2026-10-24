# Rust Paint Lab

Rust Paint Lab is an original cross-platform digital painting application in active alpha development. The current desktop alpha has pressure-responsive raster brushes, editable raster layers, bounded undo/redo, selections and basic transforms, local project saving/recovery, PNG and restricted OpenRaster interchange, and a wgpu canvas. Native Windows WM_POINTER input is wired to the same shared input and stroke pipeline as mouse input.

This is not yet a production replacement for established painting software. Native pen delivery, capture behavior, storage durability, and performance still require testing on the target Windows hardware. Keep independent copies of important artwork. The application is unsigned development software; the project has no selected distribution license.

Start with the [alpha user guide](docs/overdrive-user-guide.md), [current implementation status](docs/OVERDRIVE-PROGRESS.md), [native project format](docs/RPL-ALPHA-FORMAT.md), and [known interchange limits](docs/openraster.md).

## Workspace

- `crates/input` (`paint-input`): platform-independent pointer normalization, lifecycle processing, and deterministic versioned replay.
- `crates/stroke` (`paint-stroke`): platform-independent stroke routing and document coordinate mapping.
- `crates/document` (`paint-document`): CPU raster brushes, sparse tiles, layers, bounded transactions/history, checked storage, recovery, PNG and restricted OpenRaster.
- `apps/canvas-demo`: winit/wgpu/egui painting workspace, native Windows pen adapter, mouse input and preserved replay demonstrations.
- `apps/stylus-diagnostic`: separate native Windows pointer inspector.

See [architecture](docs/architecture.md), [dependency and license inventory](docs/dependencies.md), [testing instructions](docs/testing.md), and the [hardware checklist](docs/hardware-testing.md).

## Build and run

Rust 1.90.0 is pinned in `rust-toolchain.toml`; rustup installs the matching compiler, rustfmt, and Clippy. Windows builds use the x64 MSVC toolchain and Windows SDK. Linux requires a C linker and desktop/window-system libraries; macOS requires Xcode Command Line Tools. GPU drivers are needed to run the desktop app, but not to run the portable document/input tests. The renderer does not require a dedicated GPU; software rendering is used in Linux CI.

```sh
cargo run --release --locked -p canvas-demo
```

From GitHub Actions, download `rust-paint-lab-canvas-demo` for the canvas app or `rust-paint-lab-windows-diagnostic` for the separate diagnostic. Extract the artifact ZIP and launch the executable. The Windows package targets x64 Windows 10, needs no Rust, Visual Studio, installer, or administrator rights, and statically links the C runtime. It remains unsigned and requires a compatible graphics driver. Artifacts are temporary CI outputs, not releases. Linux and macOS development packages are also attached to successful CI runs, with platform runtime requirements documented in their package readmes.

## Painting controls

The app opens in live drawing mode. Drag the left mouse button or supported Windows pen on the paper; right-click and hover do not paint. Pressure and independent tilt are kept when the adapter supplies them. Touch is reserved for navigation and does not paint. Completed work remains in the current editable document; cancellation removes the provisional stroke.

- `B`, `M`, `E`, `K`, `I`: brush, rectangle, ellipse, lasso, eyedropper.
- `Ctrl+Z` / `Ctrl+Y`: undo / redo; `Ctrl+S`: save; `Ctrl+O`: open path dialog; `Ctrl+E`: PNG export.
- `Ctrl+F`: fill; `Ctrl+X`: delete selected pixels; `Ctrl+D`: deselect.
- `Ctrl+Shift+E` / `Ctrl+Shift+O`: OpenRaster export / import.
- `0`: fit canvas; mouse wheel: zoom; middle-button drag: pan; `F11`: fullscreen.
- `L`: live document; `P` or `1`/`2`/`3`: deterministic replay; `Tab`: switch modes; `R`: restart; `Space`: pause replay.
- `Escape`: close prompt. Dirty artwork can be saved, preserved through a confirmed local recovery snapshot, or kept open.

The UI exposes ten brush presets, size/opacity/flow/hardness/spacing, pressure responses, tilt rotation, brush angle/smoothing/color, raster layer operations and blend modes, selections, fill/delete, eyedropper, layer transforms/crop, project/recovery actions, and file import/export. The [user guide](docs/overdrive-user-guide.md) explains what is implemented and what each format drops.

## Validation

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

Linux CI additionally requires software Vulkan GPU pixel validation and runs the interactive app under Xvfb with synthetic OS mouse/key events, including save/reopen, editing, OpenRaster interchange, and forced-process recovery. A passing CI build does not establish physical pen, touch, driver latency, or Windows filesystem durability. Follow the [manual hardware checklist](docs/hardware-testing.md). The standalone diagnostic remains useful for inspecting actual WM_POINTER pressure and tilt without modifying artwork.

Third-party notices are generated for packaged dependencies. No cloud services, accounts, telemetry, or remote storage are used. See [the progress record](docs/OVERDRIVE-PROGRESS.md) for tested commit IDs, CI results, measured cloud-host workloads, unresolved defects, and exact next steps.
