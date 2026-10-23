# Architecture foundation

## Current implementation

```
Windows WM_POINTER messages
  -> native diagnostic adapter (GetPointerInfo / GetPointerPenInfo)
  -> paint-input PointerSample + normalization
  -> bounded live text inspector (native GDI)
```

Only `paint-input` and a diagnostic executable exist. The shared library forbids
unsafe Rust and has no dependencies. The diagnostic owns the Win32 event loop,
DPI awareness, and painting its text display. FFI is contained in one module,
uses initialized output buffers, checks fallible input calls, and documents unsafe
invariants. It does not measure painting latency or reproduce the final UI.

## Input contract

A sample carries adapter-local pointer ID, tool (pen/touch/mouse/unknown), phase
(down/move/up/hover/cancel), client physical pixel position, adapter-relative
milliseconds, contact/range state, buttons, optional normalized pressure, and
independently optional tilt axes in degrees. Preserve `None` versus a supported
zero value. Pressure uses the platform's reported range; finite outliers clamp
to [0,1]; invalid readings/ranges produce `None`. Tilt clamps to [-90,90] and
rejects non-finite readings. Adapters must validate position coordinates before
creating samples (Win32 supplies integer coordinates).

Windows uses `dwTime`, a 32-bit millisecond timestamp that wraps after about 49.7
days. The diagnostic displays this raw timestamp widened to u64; it does not
pretend it is globally monotonic. Future processing must unwrap per adapter or
use a higher resolution clock, preserving timestamp provenance. IDs may be reused
after release/cancellation. The adapter respects pen capability masks rather than
inferring support from nonzero data. Raw rotation is diagnostic only and valid
only when its mask is present. Positions use `ptPixelLocation` converted with
`ScreenToClient`, under per-monitor-v2 DPI awareness; subpixel/himetric input is
not yet used. DPI changes reposition the window using the system suggestion.

The inspector tracks up to 32 active contacts and ten recent events, removes
contacts on up/leave/capture loss/cancel, and surfaces retrieval errors. Windows
implicitly captures a contacting pointer. Capture changes are treated as local
cancellation without depending on a subsequent up. This is diagnostic bookkeeping,
not a tested production stroke lifecycle or gesture engine. No mouse events are
converted into pressure-sensitive samples. Touch pressure/geometry are not queried
in this first adapter and remain unavailable. Pen and touch IDs/tools stay distinct
so future arbitration can handle simultaneous fingers, palm rejection, and pen.

## Windows decision: native WM_POINTER now

Investigation baseline: **winit 0.30.12**, not a claim about all future versions.
Its Windows backend already handles `WM_POINTERDOWN/UPDATE/UP`, uses pointer frame
history, and queries `GetPointerPenInfo` for pressure. Therefore generic mouse
input is not the only path in winit. However, public `WindowEvent::Touch(Touch)`
contains location, phase, force, and ID without pen tool identity, tilt, barrel,
inversion, or eraser fields. `TouchpadPressure` is a different device event and
is not a pen solution. The backend's pressure helper also maps zero to unavailable;
our diagnostic preserves valid zero when the pen mask advertises pressure.

Use direct Win32 messages for the first diagnostic to inspect all required pen
fields. GDI text avoids a GPU/UI dependency tree and driver requirements before
input works. No winit, wgpu, or egui version is committed as a dependency yet.
For a future GPU shell evaluate then-current winit APIs and its Windows message
hook (or a native HWND owner), ensuring only one path consumes a pen stream.
Do not bolt a second competing event loop onto winit. Confirm ownership, pointer
promotion, capture/cancel, DPI, and UI event routing in a small spike before
integrating egui. Native access does not mean bypassing the OS's driver limits.

Sources inspected (2026-10-23):

- [winit 0.30.12 public events](https://github.com/rust-windowing/winit/blob/v0.30.12/src/event.rs)
- [winit Windows WM_POINTER implementation](https://github.com/rust-windowing/winit/blob/v0.30.12/src/platform_impl/windows/event_loop.rs)
- [Microsoft WM_POINTERDOWN contract](https://learn.microsoft.com/en-us/windows/win32/inputmsg/wm-pointerdown)
- [Microsoft POINTER_PEN_INFO and masks](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-pointer_pen_info)

Microsoft documentation source was read through its official GitHub mirror when
the documentation site denied automated requests. Pressure is 0–1024; tilt axes
are signed degrees. Pen masks determine validity, not device marketing specs.

## Planned boundaries (not implemented)

1. **Platform adapters:** Windows translates native pointer data; Android will
   translate MotionEvent tool type, pointer IDs, historical samples, pressure,
   orientation/tilt, and cancellation. Convert radians/orientation to our documented
   axes explicitly and test the convention; do not assume Android tilt equals
   Windows tiltX/tiltY. macOS/iPadOS/Linux need their own capability investigations.
2. **Input processing:** timestamp ordering, contact lifecycle, history/replay,
   coalescing, then gesture arbitration and stroke routing. Preserve raw samples
   before smoothing/prediction. UI hit testing and pen-versus-touch policy belong
   here or in the application shell, not in the normalization functions.
3. **Application core:** document model and explicit edit commands, independent of
   OS, UI toolkit, and GPU. Test document transitions without opening a window.
4. **Rendering:** proposed wgpu renderer consumes document/canvas snapshots and
   viewport state. Keep device/surface handling outside the core. Validate Windows
   D3D12 on Iris Xe first; evaluate Vulkan/Metal/GLES and mobile limits separately.
   A renderer must tolerate device loss and avoid unbounded GPU/RAM allocations.
5. **Controls:** proposed egui for panels and accessible controls after validating
   integration, touch targets, focus, and accessibility. Canvas input must remain
   separate from generic UI pointer events; pen data must not be flattened away.
6. **Storage:** proposed serde-based, explicitly versioned document metadata plus
   documented binary image/tile payloads. Choose format/version migration, atomic
   save/flush/rename strategy, recovery journal, and compatibility policy in a
   dedicated task. Autosave, crash recovery, exports, and timelapse are future
   local subsystems. Do not serialize raw GPU objects or platform handles.

The initial shell targets Windows 10 1703+ because per-monitor-v2 DPI awareness is
required. This is not a final platform-support commitment. Android packaging,
iPadOS feasibility/distribution, final UI stack, project license, open file format,
GPU limits, accessibility, and timing/latency budgets remain decisions to validate.

## Next bounded milestone

Run and record the Galaxy Book checklist first. Then introduce a deterministic
sample replay format for development fixtures and a shared contact lifecycle with
cancellation/ID-reuse tests. Add history retrieval with ordering tests if hardware
shows coalescing. Only then spike a minimal wgpu single-stroke canvas and confirm
that the chosen window/UI integration preserves pen capabilities. No layers or
complex brushes in that milestone.
