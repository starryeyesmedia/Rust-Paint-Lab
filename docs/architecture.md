# Architecture foundation

## Implemented pipeline

```
Windows WM_POINTER adapter             Synthetic version-1 text fixtures
  -> PointerSample (normalized units)     -> Replay parser
  -> 32-bit timestamp extension           -> ordered records
                  \                       /
                   shared lifecycle Processor
                     -> ordered Output batches
                     -> existing diagnostic display / automated assertions
                     -> paint-stroke Router (live mouse and replay sessions)
                        -> Begin/Update/Commit/Cancel commands in document units
                        -> canvas-demo Scene / fixed-width triangle geometry / wgpu
                        -> future brush engine and document (not implemented)
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

## Stroke routing and future brush boundary

See [v1 fixture grammar and commands](replay-format.md) and
[automated testing instructions](testing.md). Tests use only synthetic fixtures.
The implemented `paint-stroke` crate consumes Started/Moved/Ended/Cancelled;
pen contact paints, touch is reserved, mouse primary paints with unavailable
pressure/tilt. Hovered/Ignored do not paint. The separate crate depends only on
`paint-input`; lifecycle processing remains independent of painting policy.
It maps physical client pixels into document coordinates using a validated
axis-aligned Viewport captured at stroke start. Commands carry unique per-session
stroke IDs, pointer/tool identity, original and transformed positions, button and
capability data, source timing, and dispatch timing. Cancellation preserves the
last accepted stroke sample and a distinct terminal reason.

[Routing and transaction semantics](stroke-routing.md) specify duplicate/stale
handling, transform errors, bounded state, and provisional-versus-committed work.
Replay integration tests assert exact command streams without a window or brush.
The existing native diagnostic intentionally remains a lifecycle inspector; no
Win32 code or event handling was changed for this milestone.

Future components have distinct responsibilities:

1. **Adapters:** translate OS data, coordinate/timestamp units, and capture loss.
2. **Lifecycle:** validate samples and repair contact sequences; no paint policy.
3. **Router:** choose painting tools, map coordinates, and emit transactions.
4. **Brush engine:** consume routed samples and produce brush geometry/dabs; no OS
   input handling or surface/window ownership. Respect None versus supported zero.
5. **Canvas renderer:** display committed content plus isolated provisional
   previews; remove cancelled previews without committing them. Convert document
   coordinates to GPU units with explicit range checks at this boundary.
6. **Document model:** own accepted stroke results and future edit semantics;
   only Commit may finalize a transaction. No persistence or undo exists yet.

The separate canvas demo now implements a bounded command consumer and renderer
for synthetic playback and interactive mouse drawing. The physical-pixel winit
mouse adapter in canvas-demo/interaction.rs feeds PointerSample -> Processor ->
Router. Live and replay own separate scenes; mode switching cancels previews,
while completed live content remains in memory. The Windows-only capture observer
adds notifications without replacing winit's event loop/capture ownership. See
[live mouse semantics](live-mouse.md). Scene uses separate provisional/committed maps keyed by
StrokeId, includes the final Commit sample, retains pressure/tilt, and rebuilds
render geometry each frame so cancellation leaves no committed residue. This is
not a document model or brush engine. winit owns one event loop, wgpu owns the
surface/device and fixed vertex buffer, and the pure Playback class takes explicit
elapsed demo time. See [canvas architecture and limits](canvas-demo.md).

## Planned boundaries (not implemented)

- Platform adapters: Android MotionEvent tool/IDs/history/cancel and pressure/tilt
  conversion need their own tests. Android orientation/radians are not Windows
  tilt axes. Linux/macOS/iPadOS need capability investigations.
- Input routing: gesture arbitration, palm policy, history/coalescing, smoothing,
  prediction, subpixel positions, and multiple adapter sessions remain future work.
- Application core: OS/UI/GPU-independent document model and edit commands.
- Rendering: Phase 4 uses wgpu 30.0.1 and winit 0.30.13 only in canvas-demo.
  The user reports Phase 4 GPU success on Galaxy Book/Iris Xe; backend-specific
  and RTX 3060 validation remain separate. Device-loss recovery
  is a clear diagnostic exit, not automatic device recreation. Mobile is deferred.
- Controls: proposed egui, validate accessibility, focus, touch targets, and routing.
- Storage: proposed serde/versioned metadata and image payloads, with atomic saves,
  journaling/recovery, exports, and local timelapse designed separately.

Windows shell minimum remains Windows 10 1703+ x64; other platforms can run shared
processing tests. Automated tests do not prove physical pointer delivery or hardware latency.
The user reports successful Galaxy Book Phase 4 GPU/replay operation and pressure/
tilt detection in the standalone S Pen diagnostic. Phase 5 mouse/capture behavior
and integrated pressure-sensitive canvas drawing require their own manual checks.
Project license, final UI stack, open document format, and platform support policy
remain unresolved. The replay-fed GPU canvas milestone follows the
[original plan](next-canvas-milestone.md); [Phase 5](live-mouse.md)
adds live mouse routing; the next milestone should connect native Windows pen
samples before expanding the brush or document architecture. No brush engine, document
storage, layers, or undo exists.


## Overdrive authoritative artwork path

`WM_POINTER/mouse -> paint-input Processor -> paint-stroke Router -> paint-document
Engine -> sparse CPU raster tiles -> dirty-tile composite -> wgpu texture -> egui
workspace`. Replay keeps its original separate scene/session. Native pointer history
is bounded and ordered before shared processing; promoted Windows mouse input is
suppressed. No competing event loop or OS dependency enters shared crates.

The useful new component is `paint-document`, currently containing separate brush,
storage and recovery modules rather than empty future crates. StrokeId keys scratch
transactions; Begin/Update preview without mutating the document, Commit merges with
stroke opacity and adds a history entry, Cancel drops scratch. Brush settings are
captured at Begin. Spatial dab spacing/interpolated pressure is independent of event
frequency; the final raw endpoint is included after smoothing. Mouse unavailable
pressure remains None and uses explicit uniform brush fallback.

64px premultiplied encoded-sRGB tiles use Arc COW for snapshots. Authoritative and
preview tile count, layers, dimensions, history, file sizes, decoder allocations and
recovery queues are bounded. Tiny legacy demo geometry limits do not limit alpha
painting. GPU display uses only a composited texture; small strokes upload dirty
tiles rather than rebuilding the full document. Metadata operations can invalidate
all tiles. The CPU document is authoritative and editable without GPU resources.

Layer blend math is encoded-sRGB (not fully linear-light or ICC managed). Display
converts unpremultiplied samples to linear output for the sRGB surface. Export writes
straight-alpha PNG with sRGB/DPI metadata. See RPL-ALPHA-FORMAT.md for native storage,
locking, backups, fault boundaries and recovery limitations. The UI owns file paths,
modal input routing and preservation confirmations; it does not bypass transactions.
