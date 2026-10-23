# Dependencies and licenses

The shared `paint-input` crate uses only Rust's standard library. `paint-stroke`
depends only on local paint-input. Graphics/window dependencies belong exclusively
to the separate canvas-demo application; the native Windows diagnostic is unchanged.
Cargo.lock pins the complete graph. No egui, serde, image export, or telemetry
package is directly added for the canvas.

| Direct package | Version | Purpose / enabled features | Declared license |
| --- | --- | --- | --- |
| windows-sys | 0.61.2 | Native Windows diagnostic ABI bindings | MIT OR Apache-2.0 |
| wgpu | 30.0.1 (exact) | GPU device/surface/pipeline; std, parking_lot, dx12, metal, vulkan, gles, wgsl; web defaults disabled | MIT OR Apache-2.0 |
| winit | 0.30.13 (exact) | Stable desktop lifecycle/windowing, native raw-window-handle 0.6; default X11/Wayland/CSD support | Apache-2.0 |
| pollster | 0.4.0 (exact) | Small native future executor for device/adapter initialization, no async runtime | Apache-2.0 OR MIT |

windows-link 0.2.1 remains the diagnostic's transitive MIT OR Apache-2.0 import
helper. Other transitive packages are enumerated with version and declared SPDX
expression in [the locked inventory](dependency-licenses.md), including platform
packages not actually compiled on each OS. The substantial transitive graph is
from native graphics/window backends, not additional product features.

## Selection and compatibility investigation

On 2026-10-23 the crates.io sparse manifests reported Rust MSRVs 1.87 for wgpu
30.0.1 and 1.70 for winit 0.30.13; both are unyanked stable versions compatible
with pinned Rust 1.90.0. Their actual source/API and crate license files were
inspected and native builds/tests are required. wgpu and winit are maintained by
the gfx-rs and rust-windowing projects. winit 0.31 beta was deliberately not used.

The simple WGSL shader needs no optional GPU features. Native backends cover
Direct3D 12, Metal, Vulkan, and OpenGL/GLES fallback. Device requests use baseline
limits plus adapter texture resolution; application surface size is separately
bounded. Windows explicitly selects the system FXC shader compiler for this basic
SM5 pipeline, avoiding a dynamic DXC package. FXC is a legacy compiler: revisit
DXC packaging when modern shader features require it. Driver/backend compatibility
must be tested; compatible MSRV and successful compilation are not hardware proof.

Supporting direct dependencies were minimized: no math library (small explicit
axis-aligned transforms), no direct bytemuck (safe explicit float byte encoding),
no general async runtime, no UI panels. winit and wgpu still require their own
transitive support packages, all frozen by the lockfile.

Sources: [wgpu](https://crates.io/crates/wgpu/30.0.1),
[winit](https://crates.io/crates/winit/0.30.13),
[pollster](https://crates.io/crates/pollster/0.4.0), and the versioned Cargo
manifests/source in Cargo's checksum-verified registry cache.

## Distribution notices

The original Windows diagnostic retains [its MIT notice](licenses/windows-rs-MIT.txt).
The canvas packaging script generates THIRD-PARTY-NOTICES.txt from all reachable
Windows canvas dependencies and their bundled license/notice files, not the entire
unrelated workspace. Crate archives omitting root notice files have exact-version
supplements with [upstream revision provenance](licenses/canvas-supplemental/SOURCES.md).
Missing notice files for a new dependency fail packaging. All available alternative
license texts are retained; MIT/Apache alternatives are used where selectable.
Inventory alternatives such as LGPL are not a requirement to choose LGPL.

```sh
python scripts/canvas-licenses.py --inventory --output docs/dependency-licenses.md
python scripts/canvas-licenses.py --output target/canvas-notices/THIRD-PARTY-NOTICES.txt
```

These scripts use normal Cargo registry verification and no custom network fetch.
Build tooling/source may have separate licenses from runtime-linked dependencies;
notices conservatively include the reachable Windows build graph. Native OS APIs,
SDKs, and installed GPU drivers are prerequisites and are not redistributed in
the artifact. No third-party implementation source is copied into application code.

The owner has not selected a license for Rust Paint Lab's original code; third-party
notices are not a distribution license for the application itself. Before adding
or upgrading packages, update the lockfile, inventory, and notices; inspect
maintenance, MSRV, enabled features, target compatibility, and licensing again.
