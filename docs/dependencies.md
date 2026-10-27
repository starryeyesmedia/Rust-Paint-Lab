# Dependencies and licenses

The shared input/router crates remain graphics and OS independent. `paint-document`
uses serde/serde_json for bounded versioned metadata/presets, sha2 for corruption
checks, and png for bounded image decoding/encoding. Brush/raster/history logic is
safe Rust with no GPU or GUI dependency. Cargo.lock pins the complete graph.

| Direct package | Locked version | Purpose | License |
| --- | --- | --- | --- |
| windows-sys | 0.61.2 | Native diagnostic and Windows-only canvas pointer adapter | MIT OR Apache-2.0 |
| wgpu | 27.0.1 exact | GPU presentation, DX12/Metal/Vulkan/GLES/WGSL | MIT OR Apache-2.0 |
| winit | 0.30.13 exact | Desktop window/input lifecycle | Apache-2.0 |
| egui / egui-winit / egui-wgpu | 0.33.3 exact | Functional workspace controls and integration | MIT OR Apache-2.0 |
| pollster | 0.4.0 exact | Initialization future executor | MIT OR Apache-2.0 |
| serde / serde_json | 1.0.229 / 1.0.154 | Metadata, brush presets and workspace preferences | MIT OR Apache-2.0 |
| sha2 | 0.10.9 | Native project SHA-256 integrity | MIT OR Apache-2.0 |
| png | 0.18.1 | PNG import/export | MIT OR Apache-2.0 |
| zip | 9.0.5 exact | Bounded OpenRaster ZIP, deflate only | MIT |
| roxmltree | 0.21.1 exact | Bounded stack.xml; DTD disabled | MIT OR Apache-2.0 |
| image | 0.25.10 exact | Bounded PNG/JPEG decoding for external workspace references | MIT OR Apache-2.0 |
| rfd | 0.15.4 exact | Native project/image/preset selection dialogs; XDG desktop portal on Linux | MIT |

The interface bundles locally embedded Outfit Bold/ExtraBold/Black and Manrope
Regular/SemiBold/Tabular-SemiBold/Bold static faces under the SIL Open Font
License 1.1; no runtime download or Cargo dependency is used. The tabular
Manrope face uses equal-width digit metrics because egui does not expose
OpenType numeric features. Exact source revisions, weight instantiation,
checksums, and license texts are in
[`assets/fonts/README.md`](../assets/fonts/README.md).

## Compatibility decision

Pinned Rust remains 1.90.0. egui 0.34 needs Rust 1.92; egui 0.33.3 supports the
pinned toolchain and uses wgpu 27. The canvas therefore moved from wgpu 30.0.1 to
27.0.1 instead of keeping duplicate graphics stacks or writing a custom GUI renderer.
Existing GPU and replay tests must pass after this change. winit is unchanged.
wgpu native device limits are bounded; no advanced optional GPU feature is required.
Windows retains system FXC and static CRT packaging. Driver support remains necessary.

The reference board enables only PNG/JPEG codecs, limits files to 32 MiB, decoded
textures to 2048 × 2048 / 8 MiB each, and the board to six images. Those image
textures remain UI-only and are not stored in the native artwork or exports. The
image crate and enabled zune JPEG decoder are included in the canvas artifact's
generated third-party notices. Egui provides keyboard navigation, visible focus and
light/dark contrast. Its optional AccessKit feature is enabled through egui-winit;
AccessKit, AT-SPI, and platform bridge crates are included in the lockfile license
inventory and target-specific package notices. An automated tree test checks
labels for representative toolbar and document controls. Native screen-reader
usability is not yet verified; see accessibility-assessment.md. Egui clipboard
integration brings image packages transitively, but those codecs are not advertised
as implemented application import/export features. Font notices include bundled
Ubuntu/Noto/Hack attributions. No cloud services, telemetry or account dependency.
The project owner's own distribution license remains undecided.

The artist-facing file selection workflow uses rfd 0.15.4 with its default XDG
portal backend on Linux; Linux desktops need a working document portal service for
native browsing. Windows uses the platform dialog integration and macOS uses its
native dialog. The Windows executable does not bundle GTK, a language runtime, or an
installer. Dialog presentation has not yet been manually checked on physical Windows
or macOS hardware.

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

OpenRaster uses current zip 9.0.5 (Rust 1.88) after investigating zip 6 and the
current release. A bounded directory preflight runs before ZIP library entry-table
allocation. No encryption/extra codecs are enabled. See openraster.md for limits.
