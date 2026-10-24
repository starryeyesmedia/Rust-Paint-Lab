# Live canvas input semantics

The default canvas mode is now LIVE mouse. Drawing stays entirely in memory:
there is no saving, undo, brush simulation, native pen integration, or telemetry.
The previous replay renderer and separate native pressure/tilt diagnostic remain.

## Pipeline and session boundaries

```
winit MouseInput / CursorMoved (physical client pixels)
  -> LiveMouse adapter -> PointerSample (Tool::Mouse, pressure/tilt None)
  -> paint-input Processor -> paint-stroke Router -> StrokeCommand -> live Scene
```

The platform-free adapter and CanvasSession live in canvas-demo/interaction.rs;
GPU/window dependencies have not entered paint-input or paint-stroke. Native mouse
motion and button events do not directly append geometry. One logical mouse ID is
reused; Router assigns monotonically increasing transaction IDs across clear and
mode switches. Live and replay own separate scenes, providing distinct session
namespaces even when a replay Router generates ID 0 again.

Left Down inside paper begins, held movement updates, and Up commits the last
known position as the final sample. Right/middle buttons, hover, duplicate Downs,
and orphan Moves/Ups cannot paint. Mouse pressure and both tilt axes remain None.
Winit mouse-emulation from other devices, if provided by a driver, is still only
mouse data; it is not native pressure-sensitive S Pen integration.

Cancel on focus loss, window/canvas exit, capture loss, suspend/close, invalid
coordinates, resize/DPI change, clear, or mode switch. Cancellation travels through
the lifecycle and router and removes only the active preview. No Move can restart
a cancelled stroke; a fresh Down and valid position are required. A Down outside
the paper never starts painting. While contact is active, paper-boundary crossings
are clipped to the document edge and the same transaction continues on re-entry.
The clipped gap is a disconnected subpath, so no line bridges the outside area.
Leaving the application window, focus loss, or capture loss still cancels.

Resize/DPI changes cancel the current preview before replacing the mapping.
Physical CursorMoved coordinates and physical inner_size share the same unit;
scale_factor is never multiplied a second time. CanvasMapping exposes its exact
inverse as the Router Viewport. The mapping is fixed per stroke, while committed
document coordinates retain their meaning across window sizes. GPU scissors
confine stroke caps/segments to the displayed paper bounds.

## Modes and controls

| Control | Action |
| --- | --- |
| Left drag/release | Draw/commit inside paper in LIVE mode |
| C | Explicitly clear live content and cancel active work (LIVE only) |
| L | Return to LIVE, keeping completed live strokes |
| P | Select/restart the current replay scenario |
| 1 / 2 / 3 | Select completed/cancelled/concurrent replay scenario |
| Tab | Switch between live and replay |
| R | Restart replay (does not clear live work) |
| Space | Pause/resume replay |
| Escape / Close | Cancel active live work and close |

Switching mode cancels active live work and resets replay, so neither mode can
retain an abandoned provisional transaction. Live committed content survives replay
switches but not C or application close. The title displays LIVE/REPLAY, completed
count, DRAWING/READY state, and capacity feedback. Drawing is fixed-width; orange
is provisional and blue is committed.

## Windows capture loss

Winit 0.30.13 owns implicit mouse capture but consumes WM_CAPTURECHANGED without
exposing a WindowEvent. Its message-loop hook cannot reliably observe synchronously
sent capture notifications. The canvas therefore installs a small Windows-only
ComCtl32 SetWindowSubclass observer, forwards every message once, and wakes the
existing winit event loop via a proxy. A pending flag is checked before native
input is routed, so capture loss cannot silently become an Up/Commit.

The observer suppresses capture release nested in normal WM_LBUTTONUP; winit
releases capture before emitting its mouse-up event. Other capture transfers and
WM_CANCELMODE cancel an active transaction. It never calls SetCapture/ReleaseCapture,
uses no pen APIs, and introduces no second event loop. FFI is isolated in
windows_capture.rs; the remainder of the binary and shared library deny/forbid
unsafe code. Callback state is held in a UI-thread-local map of Rc values, not
raw Rust userdata pointers. Registration/removal and native destruction are
accounted for, and no RefCell borrow spans reentrant dispatch.

Linux/macOS rely on winit focus/window-exit cancellation. An external capture/grab
change that delivers neither notification is not separately observed on those
platforms in this milestone. Native Windows capture delivery is not proven by CPU
tests or successful linking; include interruption tests below.

## Budgets and responsiveness

The existing limits remain: 64 retained strokes, 1024 samples per stroke, 8192
samples overall, fixed vertex budget. Exceeding a live limit cancels that preview,
retains older completed strokes, and displays “C to clear”; it does not terminate
the application. No points are silently evicted or converted to committed content.
Very long/dense strokes can hit the per-stroke limit. This is a prototype bound,
not an unlimited document model. No smoothing or new sampling/coalescing policy
is introduced. Renderer input stays finite/ranged and GPU buffers remain bounded.

Live redraws are requested for changed state and are idle otherwise. Replay retains
its explicit playback clock and timed redraws. Samples are not logged to disk,
exported, or automatically turned into recorded fixtures. Clear/mode changes remove
abandoned state; GPU initialization/device failures retain the existing diagnostic
exit behavior rather than attempting untested recovery.

## Manual Galaxy Book checklist

User-reported baseline: Phase 4 rendered/replayed/resized successfully on the
Galaxy Book2 Pro 360; the standalone S Pen diagnostic detected real pressure and
tilt. That is useful prior evidence, not validation of this new interactive build.

1. Download rust-paint-lab-canvas-demo from the latest feature/live-mouse-drawing
   run, extract all files, and launch canvas-demo.exe. It starts in LIVE mode.
2. Use a mouse/trackpad to draw several strokes inside the light paper. Confirm
   orange previews turn blue on left release and remain; hover/right-click do not
   paint. No pressure or tilt is expected here.
3. Start a stroke, cross beyond each paper edge, re-enter, and release. Confirm the
   same preview remains active, the visible line stops at each edge, and no line
   bridges the gap. Starting outside paper and dragging inside must not paint.
   Alt-Tab, minimize, leave the window, or switch to replay mid-stroke; confirm the
   preview disappears without committing. Native capture transfer with focus
   unchanged needs separate testing with a reproducible tool/window.
4. Draw, resize, and change display scaling. Completed shapes should keep document
   positions and fit the paper; a resize during a stroke cancels that preview.
5. Press P or 1/2/3 for replay; R/Space should still work. Press L to return and
   verify completed live strokes remain. Switch during previews in both modes.
6. Use C while idle and mid-stroke; expect an empty live canvas, no resumed orphan
   stroke, and working fresh strokes. Close/relaunch: session content is lost.
7. Try many/long strokes; capacity feedback should allow C and further drawing
   without a crash. Record commit, Windows build, driver/scaling, mode, and steps
   for any stuck previews, missing final points, or unexpected commits.

## Recommended Phase 6

Add a Windows-native pen adapter using the proven diagnostic's WM_POINTER data,
feeding the same Processor/Router/live Scene. Preserve masks, pressure zero versus
None, independent tilt axes, barrel/eraser flags, identity, timestamps/rollover,
hover, and cancellation. Prevent promoted mouse events from duplicating pen input.
Decide mouse/pen concurrency and per-session namespacing explicitly before coding.
Retain replay and mouse regressions, extend synthetic pen fixtures/GPU checks, and
validate on the Galaxy Book's actual S Pen. Keep fixed-width geometry initially;
pressure-driven brushes, layers, persistence, and history remain separate work.
