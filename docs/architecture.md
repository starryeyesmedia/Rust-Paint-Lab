# Architecture foundation

## Implemented pipeline

```
Windows WM_POINTER adapter             Synthetic version-1 text fixtures
  -> PointerSample (normalized units)     -> Replay parser
  -> 32-bit timestamp extension           -> ordered records
                  \                       /
                   shared lifecycle Processor
                     -> ordered Output batches
                     -> diagnostic display / automated assertions
                     -> future stroke router and brush engine (not implemented)
```

`paint-input` is safe, OS-independent Rust without external dependencies. Its
sample definitions, normalization, lifecycle processing, clock extension, and
replay reader are separate modules. The diagnostic owns a single native Win32
event loop and display; replay is an explicit development/test facility, never an
automatic recorder. File IO belongs to the CLI example, outside the library.

## Sample and timing contract

PointerSample retains adapter-local ID, tool, phase, physical client pixel
position, relative milliseconds, pressure, independent tilt axes, buttons, and
contact/range state. `None` means unavailable; supported zero is preserved.
Pressure is [0,1] and tilt [-90,90] degrees. Normalizers clamp finite outliers and
reject invalid numeric inputs. Processor validates positions, ranges, and contact
flags before forwarding noncancel samples. Pen and touch never share invented
pressure, and mouse buttons are never treated as pressure data.

One Processor belongs to one ordered adapter stream. Merge/namespacing multiple
adapters is a future shell responsibility. Outputs carry nondecreasing dispatch
`time_ms`; embedded samples retain their original timestamps. Equal timestamps
are accepted in input order. Older nonterminal samples are ignored without
replacing newer state. Older terminal events conservatively cancel that ID at the
current dispatch time. Cancellation outputs carry the last accepted contact sample
and an explicit reason, rather than fabricated end position/pressure.

Windows dwTime wraps every ~49.7 days. MillisecondClock32 extends wrapping u32
milliseconds to u64 using forward modular deltas smaller than 2^31. Duplicate and
small regressive readings clamp to the last time without moving the raw anchor.
This is deterministic but not high-resolution timing or packet reordering: late
samples clamped by the adapter can still be processed at the same dispatch time.
Observations must be less than ~24.8 days apart; longer suspension requires a new
adapter session after interrupting old contacts. Replay uses unwrapped u64 times.

## Lifecycle policy

| Incoming event | Outcome |
| --- | --- |
| Down on unused ID | Started; retain latest sample, unless capacity reached |
| Move on active ID with same tool | Moved; replace latest retained sample |
| Up on active ID | Ended and remove contact |
| Cancel / lost capture | Cancelled and remove; unknown ID is explicitly ignored |
| Hover | No retained hover state; cancel any contact on that ID, emit Hovered |
| Duplicate Down | Cancel old contact as Replaced, then Started (even at capacity) |
| Tool changes without new Down | Cancel old contact; orphan move/up ignored |
| Move or Up without Down | Ignored as MissingDown; no synthetic stroke start |
| Invalid noncancel sample | Cancel existing contact as InvalidData, otherwise ignore |
| Application interruption | Cancel all active IDs in ascending order |
| Replay EOF | Cancel all unfinished IDs as EndOfReplay |

Processor retains at most 64 active samples in a BTreeMap, no per-ID tombstones,
hover history, or unbounded logs. A sample produces at most two outputs; global
interruption produces at most 64. Duplicate Moves remain updates; duplicate Ups
become explicit ignores. IDs are reusable once ended/cancelled. No generation
number is available from a bare pointer ID: an old terminal event after ID reuse
can conservatively cancel the new contact. Replay preserves this policy.

No processor can infer an absent terminal if there are no subsequent events.
Adapters must call `interrupt` on focus/application loss or teardown, and `cancel`
on capture loss or invalid retrieval. There is no timeout thread or heuristic
stroke completion. Lifecycle cancellation means interrupted/aborted input, not
committed brush output; the future application will decide transaction policy.

## Windows integration and research

The existing HWND, GDI text display, and event loop remain. GetPointerInfo and
GetPointerPenInfo are queried during native dispatch. Capability masks gate
pressure/tilt; raw fields and message details remain visible. Physical screen
pixels are converted with ScreenToClient under per-monitor-v2 DPI awareness.
WM_DPICHANGED keeps the suggested window position. Unsupported touch pressure and
geometry remain None; touch IDs/tool types are kept distinct.

The diagnostic now uses Processor for its active contacts and displays processed
outcomes. Capture-change notifications cancel one ID locally. Application
deactivation and destruction interrupt all contacts; failed pointer retrieval
cancels the affected contact. Timestamp extension runs before processing while
raw dwTime remains visible. These are local bookkeeping changes only: no new
capture API calls, competing event loop, OS gesture changes, or coalesced-history
retrieval were introduced. Windows' implicit capture remains in charge.

The foundation investigated **winit 0.30.12**: its Windows backend already uses
WM_POINTER and GetPointerPenInfo for pressure, but public Touch events omit pen
identity, tilt, barrel/inversion/eraser state. Its pressure helper maps zero to
unavailable; this adapter preserves valid zero with capability masks. Therefore
the diagnostic uses native WM_POINTER. GDI avoids committing to GPU/UI integration
before validating pen input. Reevaluate current winit APIs and its native message
hook or HWND ownership in a later focused integration spike. Do not consume the
same pen stream through two paths. TouchpadPressure is not a pen substitute.

Sources inspected during foundation (2026-10-23):
- [winit 0.30.12 events](https://github.com/rust-windowing/winit/blob/v0.30.12/src/event.rs)
- [Windows implementation](https://github.com/rust-windowing/winit/blob/v0.30.12/src/platform_impl/windows/event_loop.rs)
- [Microsoft WM_POINTERDOWN](https://learn.microsoft.com/en-us/windows/win32/inputmsg/wm-pointerdown)
- [Microsoft POINTER_PEN_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-pointer_pen_info)

## Replay and future brush boundary

See [v1 fixture grammar and commands](replay-format.md) and
[automated testing instructions](testing.md). Tests use only synthetic fixtures.
A future stroke router consumes Started/Moved/Ended/Cancelled, resolves tool/UI
routing, transforms client pixels into document coordinates, and feeds brush
transactions. Hovered is cursor feedback; Ignored is diagnostics. Pressure/tilt
must reach the brush unchanged unless an explicit calibration stage is added.
Replaying the same fixture through that router should test brush commands without
a window, clock, or GPU. Do not put brush policy inside Processor, and do not
silently commit a cancelled stroke as a complete one.

## Planned boundaries (not implemented)

- Platform adapters: Android MotionEvent tool/IDs/history/cancel and pressure/tilt
  conversion need their own tests. Android orientation/radians are not Windows
  tilt axes. Linux/macOS/iPadOS need capability investigations.
- Input routing: gesture arbitration, palm policy, history/coalescing, smoothing,
  prediction, subpixel positions, and multiple adapter sessions remain future work.
- Application core: OS/UI/GPU-independent document model and edit commands.
- Rendering: proposed wgpu, validate D3D12/Iris Xe first; device-loss handling and
  resource limits before mobile expansion. No rendering code is implemented here.
- Controls: proposed egui, validate accessibility, focus, touch targets, and routing.
- Storage: proposed serde/versioned metadata and image payloads, with atomic saves,
  journaling/recovery, exports, and local timelapse designed separately.

Windows shell minimum remains Windows 10 1703+ x64; other platforms can run shared
processing tests. Physical pen behavior, capture/interruption delivery, tilt,
latency, GPU compatibility, and Android S Pen remain unverified by these tests.
Project license, final UI stack, open document format, and platform support policy
remain unresolved. The next milestone should add a pure stroke-command router and
fixture assertions for cancellation/commit and pen-versus-touch routing; keep GPU
and brush algorithms out until that contract is testable.
