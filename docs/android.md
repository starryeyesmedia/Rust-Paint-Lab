# Android debug application

The first Android host is a real `NativeActivity` built from the existing
`canvas-demo` Rust library. Winit owns Activity/window events, wgpu presents to
the Android surface, and the existing Rust document/stroke pipeline owns the
artwork. The initial package target is ARM64 (`aarch64-linux-android`). The APK
does not require a network service or account. Package metadata declares no
permissions; the generated APK manifest still needs inspection during the first
successful build.

The host enables winit's NativeActivity support and uses the already locked
`android-activity` 0.6.1 crate. `cargo-apk` 0.10.0 is a build tool, not an APK
runtime dependency. The pinned desktop `rfd` crate has no Android backend, so a
small Android adapter keeps the Studio call sites buildable until Storage Access
Framework integration is implemented.

## Build environment

Install Rust 1.90.0 from the repository toolchain file, Android SDK Platform 34,
Android Build Tools 34.0.0, and Android NDK 27.2.12479018. Set `ANDROID_HOME` and
`ANDROID_SDK_ROOT` to the SDK directory and `ANDROID_NDK_HOME` to its
`ndk/27.2.12479018` directory. Add the Rust target and the pinned APK packager:

```sh
rustup target add aarch64-linux-android
cargo install cargo-apk --version 0.10.0 --locked
```

Build the debug APK from the repository root:

```sh
scripts/android-build.sh
```

The script builds `com.starryeyesmedia.bitchpixel` for ARM64, then prints the
source commit, APK path, and SHA-256 digest. The output is named
`bitchpixel.apk` below Cargo's target directory. Install it on an attached ARM64
device with:

```sh
adb install -r /path/to/bitchpixel.apk
```

No emulator, Android SDK, NDK, `adb`, or Rust toolchain is bundled into the
repository. The build command does not claim that the APK has launched or that a
physical device supports the selected graphics backend.

## Current behavior

- A single contact that begins on the paper is adapted to a shared Rust
  `PointerSample` with `Tool::Touch` and routed through `paint-input`,
  `paint-stroke`, and `paint-document`. Touch painting is explicitly enabled
  after the Android host arbitrates contacts versus navigation. Pressure, tilt,
  buttons, and source-clock values stay unavailable at this input layer. Winit
  does not report Android's tool type in this event, so this first slice does not
  distinguish a finger from an S Pen; no S Pen compatibility claim is made.
- Touches that begin over UI or references stay with the existing UI path. A
  touch stroke cancels if it crosses an interactive overlay. A second contact
  also cancels the in-progress stroke and suppresses painting until all contacts
  lift. Two-finger navigation is not implemented in this checkpoint.
- Recovery, drafts, preferences, recent artwork metadata, thumbnails, and logs
  live below the NativeActivity private data directory. On background/suspend,
  the app interrupts active input and flushes dirty committed artwork through
  the existing recovery writer. On Activity recreation, Home exposes those
  `.rpl` recovery snapshots for explicit restore.
- The existing desktop file dialog library has no Android backend. Android
  document-provider pickers for Open, Save As, image placement/import, and export
  are not wired in this checkpoint. Private recovery and drafts remain available
  without those pickers.
- Orientation and density changes interrupt active contacts before the surface
  and canvas mapping are resized. The Activity requests those changes to be
  delivered to the current instance where Android permits.
- The existing studio controls are hosted as-is. This is not the completed
  mobile layout or touch target/accessibility milestone.

## Validation record

For each build, record the exact source commit, toolchain/SDK/NDK versions, APK
SHA-256 and size. Test APK installation, launch, GPU/backend initialization,
canvas rendering, a saved stroke, Undo/Redo, private recovery restore, pause and
resume, and rotation as separate checks. Keep these results distinct from
physical Galaxy S23 Ultra testing.

The initial implementation checkpoint has not yet produced an APK in the current
workspace: Rust/Cargo, Android SDK/NDK, and `adb` are absent from the execution
environment. No launch, GPU, canvas, drawing, storage round-trip, performance,
APK-size, or hardware result is claimed. See
[`hardware-testing.md`](hardware-testing.md) for the device checklist.
