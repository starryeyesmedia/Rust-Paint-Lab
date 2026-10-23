# Dependencies and licenses

The shared input crate uses only Rust's standard library. No dependencies are
needed on non-Windows targets. Cargo.lock pins the complete current graph.

| Package | Version | Purpose | SPDX license |
| --- | --- | --- | --- |
| windows-sys | 0.61.2 | Microsoft-generated Win32 ABI bindings, Windows only | MIT OR Apache-2.0 |
| windows-link | 0.2.1 | Transitive Windows import/link support | MIT OR Apache-2.0 |

Both are maintained in Microsoft's [windows-rs repository](https://github.com/microsoft/windows-rs).
The versioned crate manifests and bundled license files were inspected during
initialization. This project chooses the MIT option for these dependencies;
retain [their MIT notice](licenses/windows-rs-MIT.txt) with distributed binaries
and substantial copies. The MIT notice is identical in both packages. This is a
third-party notice, not a license for Rust Paint Lab's original source. The owner
must select the project's own license before publishing an open-source release.

No third-party source was copied into the implementation. Microsoft API contracts
and winit source informed the architecture investigation. winit (Apache-2.0),
wgpu (MIT OR Apache-2.0), egui (MIT OR Apache-2.0), and serde (MIT OR Apache-2.0)
are preliminary candidates, **not included dependencies**; recheck the selected
versions and transitive licenses if adopted. Native Windows APIs are supplied by
the OS. Rust and Windows SDK/build tools are development prerequisites, not
vendored application libraries.

Before adding dependencies: document the current need, maintenance, target
compatibility, enabled features, and license; update the lockfile and this list.
Avoid services/telemetry and do not add large frameworks for speculative features.
