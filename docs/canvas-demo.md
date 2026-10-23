# First GPU canvas demo (Phase 4)

A separate `apps/canvas-demo` workspace application displays simulated strokes.
It has no live painting input, brush engine, panels, layers, document persistence,
undo, or telemetry. The original Windows stylus diagnostic remains separate.

## Build and run

Use pinned Rust 1.90.0. Windows needs MSVC/Windows SDK to **build**, but the
Actions artifact needs neither to **run**. Linux needs an X11 or Wayland desktop
and graphics libraries/driver (Ubuntu prerequisites below); macOS needs its normal
Rust linker/Xcode command-line tools and a Metal-capable system.

```sh
cargo run --locked -p canvas-demo
cargo build --release --locked -p canvas-demo
```

On Ubuntu, for development and headless tests:

```sh
sudo apt-get update
sudo apt-get install -y libx11-dev libxkbcommon-dev libwayland-dev mesa-vulkan-drivers xvfb xauth
```

Windows users can download `rust-paint-lab-canvas-demo` from a successful
`feature/gpu-canvas` Actions run. Extract and run `canvas-demo.exe`. Its README.txt
contains detailed RTX 3060 test steps and troubleshooting. The release build
statically links the CRT and uses Windows' FXC compiler, so no separate VC++
redistributable or DXC DLL bundle is expected. Windows graphics libraries and a
compatible Direct3D 12 driver remain required; Vulkan is an optional driver-backed
alternative. This package is unsigned and is not a GitHub release.

## Scenarios and controls

| Key | Expected behavior |
| --- | --- |
| 1 | A: pressure-varying pen draws a zigzag; orange preview becomes blue at commit |
| 2 | B: orange preview grows, holds, then disappears at capture loss, leaving no committed stroke |
| 3 | C: two independent pen paths commit separately; simulated central touch activity draws nothing |
| R | Clear current scene and restart the selected scenario |
| Space | Pause/resume demonstration playback |
| Escape | Close and release window/GPU resources |

The title identifies scenario, status, colors, and shortcuts. Resize to check the
light document rectangle stays centered/aspect-preserving against the dark outer
background. Mouse, touch, and pen events do not paint in this milestone.

## Architecture

```
embedded v1 Replay fixture -> Processor -> Router -> immutable command sequence
  -> Playback::advance_to(explicit_elapsed_ms) -> bounded Scene -> mesh -> wgpu
```

Replay::run invokes the real lifecycle Processor, and the actual paint-stroke
Router creates the commands. Command generation never observes wall time. The
winit application supplies an explicit elapsed demonstration clock at roughly
60Hz, with pause/restart controls. Equal-time commands retain pipeline order.
The CPU playback class is independently testable without sleeping or a GPU.

Scene stores provisional and committed sample vectors in separate BTreeMaps keyed
by StrokeId. Begin creates a preview, Update appends attributes, Commit appends the
final sample and moves the transaction into committed state, and Cancel removes
only provisional state. Each frame clears the entire render target and rebuilds
geometry from current state, so cancelled pixels cannot remain baked into content.
Samples retain pressure/tilt/buttons/timing even though geometry uses only position.
There is no brush or persistent document model behind these demo maps.

CPU geometry uses opaque triangle quads for segments plus square joins/caps.
Width is always 8 **document units**, independent of pressure, scaled with the
canvas. There is no antialiasing or natural-media simulation. The 800x500 document
fits the physical client viewport with a 16px margin and preserved aspect ratio;
+y down becomes clip-space +y up. This canvas mapping is separate from the
router's client-to-document Viewport. The demo's router viewport is identity.
The renderer validates finite/ranged document f64 values before conversion to f32;
there is no unsafe transmutation or direct bytemuck dependency.

## Explicit budgets and failure policy

- 64 total active/committed strokes, 1024 samples per stroke, 8192 total samples.
- At most 98,310 vertices (12 per sample plus paper), 24 bytes per vertex;
  one fixed ~2.25 MiB GPU vertex buffer, bounded temporary CPU mesh/upload buffers.
- Document sample coordinates must be finite with absolute value <= 1,000,000.
  Mapping accepts a small larger outline domain and rejects invalid/nonfinite data.
- Nonzero surface dimensions capped at 4096 (or a smaller device limit). Surface
  images/driver allocations are additional to the vertex budget; driver memory
  use is not controlled by these CPU limits.
- Restart replaces scene and playback state. Repeated scenarios do not accumulate.
  Capacity or invalid-coordinate errors abort an affected preview and are surfaced;
  they never convert a partial preview to committed content.

The application owns one winit event loop and an Arc-backed safe wgpu Surface.
Zero-sized windows skip configure/render. Redraws use timed wakeups during playback,
wait when finished/paused, and resume for resize/input. Lost/outdated surfaces are
recreated/configured with an eight-attempt bound; suboptimal frames are presented
then reconfigured; timeout/occlusion skip a frame. Device loss, validation/OOM,
initialization failure, or exhausted budgets produce a terminal diagnostic error
and nonzero exit. There is no silent success after a failed initialization.
Device recreation is deliberately deferred; restarting the demo is the recovery.

## Automated validation versus hardware

See [testing.md](testing.md). CPU tests cover all three scenarios, command samples,
final endpoints, pressure preservation, independent transactions/touch exclusion,
coordinate mapping/errors, duplicate/orphan safety, restart, and resource budgets.
An offscreen GPU runner uses this same shader/pipeline and real routed commands.
It reads back pixels for orange previews, blue final segments, canvas/background
contrast, and cancellation cleanup using broad regions/thresholds, not exact images.

The custom GPU runner explicitly prints GPU_SMOKE: SKIPPED when no compatible
adapter is available, outside Rust's passed-test counts. Device/shader/readback
failures after adapter selection are failures. RPL_REQUIRE_GPU=1 makes missing
adapters fail; Linux CI requires this with Mesa software Vulkan installed. CI also
runs the actual window to completion under Xvfb using --exit-after-demo (a testing
option); it must present its final frame before reporting WINDOW_SMOKE success.
This verifies a virtual/software path, not physical RTX 3060 or Iris Xe execution.

## Ryzen 7 / RTX 3060 manual checklist (pending)

1. Use Windows 10 x64 with current NVIDIA graphics drivers; extract the canvas
   artifact into a user folder and launch from PowerShell to retain error output.
2. Record BUILD.txt commit, Windows build, GPU/driver/backend printed in the
   console. Verify the selected adapter is the RTX 3060 rather than assuming it.
3. Check A stays blue after completion, B becomes completely blank after cancel,
   and C has two independent blue paths with no central touch marks.
4. Test R repeatedly, scenario switching during a preview, Space pause/resume,
   resize, minimize/restore, display scaling, and close/relaunch. No growth of
   retained strokes across restarts is expected.
5. Report blank/missing paper, incorrect colors/endpoints, cancelled residue,
   stuck playback, flicker, GPU errors, or unexpected exits with scenario and steps.
   Capture loss here is simulated; it does not prove physical pointer handling.

## Recommended Phase 5: live mouse before pen

Add a focused mouse adapter in the canvas shell using the existing shared sample,
lifecycle, and router contracts. Route primary-button down/move/up through the
real Processor/Router alongside the existing replay mode; keep pressure/tilt None.
Use the inverse of the displayed canvas mapping for client-to-document routing,
freeze it per stroke, and test physical-pixel/DPI/resize behavior. Explicitly
cancel on focus/capture loss and switching modes; keep replay reset separate from
live sessions so StrokeIds cannot collide with retained geometry. Add synthetic
mouse/focus-loss tests and manual desktop checks before considering WM_POINTER pen
integration. No pressure-sensitive pen or brush engine is part of Phase 5's initial
scope; keep the native stylus diagnostic available for its pending hardware tests.
