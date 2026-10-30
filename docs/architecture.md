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
6. **Document model:** `paint-document` owns sparse raster tiles, editable layers
   and alpha masks, bounded undo/redo, provisional brush transactions, and checked
   native storage; only Commit finalizes a raster stroke transaction. It is
   OS/UI/GPU-independent. Unmasked projects retain v1, mask projects use v2,
   and editable vector path metadata/allocator use v3; filled vector paths use
   explicit v4 metadata while sharing the existing raster/mask tile sections.
   All existing v1/v2/v3 projects continue to read. Vector paths share
   raster-layer ordering and mask compositing, while raster pixels are derived
   only for display/export. The V tool creates freehand polylines by default,
   with an optional workspace-only Catmull–Rom-to-cubic mode, edits anchors and
   handles, and can close paths and apply an even-odd fill. Unfilled paths keep
   the v3 schema; a fill is a single undoable v4 document edit. See the alpha
   format and overdrive progress document for limits and current boundaries.

The separate canvas demo now implements a bounded command consumer and renderer
for synthetic playback and interactive mouse drawing. The physical-pixel winit
mouse adapter in canvas-demo/interaction.rs feeds PointerSample -> Processor ->
Router. Live and replay own separate scenes; mode switching cancels previews,
while completed live content remains in memory. The Windows-only capture observer
adds notifications without replacing winit's event loop/capture ownership. See
[live mouse semantics](live-mouse.md). Scene uses separate provisional/committed maps keyed by
StrokeId, includes the final Commit sample, retains pressure/tilt, and rebuilds
render geometry each frame so cancellation leaves no committed residue. The alpha
workspace can route live samples to the independent raster document engine; legacy
fixed-width demo scenes remain for replay and compatibility. winit owns one event loop, wgpu owns the
surface/device and fixed vertex buffer, and the pure Playback class takes explicit
elapsed demo time. See [canvas architecture and limits](canvas-demo.md).

For a live pen or mouse contact that starts on paper, the app adapter clips each
physical client-space segment against the current document rectangle before it
reaches Processor/Router. A clipped exit/re-entry emits a `BreakStrokeSegment`
command under the original `StrokeId`; consumers reset interpolation at the
boundary without ending the transaction. This prevents both demo polylines and
raster brush spacing from connecting across regions traversed outside paper. A
contact that begins outside cannot paint. Focus loss, capture loss, invalid input,
resize, mode change, and leaving the window still use the explicit cancellation
path. This clipping belongs in the canvas adapter because it knows the transformed
artboard; shared lifecycle and routing remain window-agnostic.

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

Brush stabilization is applied inside each document stroke transaction. A bounded,
constant-memory One Euro filter uses sample timestamps and derivative-adjusted
cutoff: slow movement receives stronger filtering while fast movement raises the
cutoff to retain responsiveness. Only document-space position is filtered;
pressure, tilt, button state and source timing are preserved from the input sample.
Setting stabilization to zero is an exact position passthrough. A clipped
`BreakStrokeSegment` resets filter velocity so re-entry cannot bridge the outside
region. Commit connects the filtered path to the actual release sample and adds its
final dab, preventing a visible endpoint gap. Cancellation drops both preview tiles
and filter state.

64px premultiplied encoded-sRGB tiles use Arc COW for snapshots. Authoritative and
preview tile count, layers, dimensions, history, file sizes, decoder allocations and
recovery queues are bounded. Tiny legacy demo geometry limits do not limit alpha
painting. GPU display uses only a composited texture; small strokes upload dirty
tiles rather than rebuilding the full document. Metadata operations can invalidate
all tiles. The CPU document is authoritative and editable without GPU resources.
Raster layer masks are sparse 8-bit tiles (one quarter the size of RGBA tiles) with
missing samples revealing the source. Mask-paint strokes use the same preview,
commit/cancel, selection and bounded history lifecycle without modifying source
raster bytes. Mask tiles share the document's tile-count budget. OpenRaster export
applies enabled masks to per-layer pixels and reports that mask editability is
flattened; the native `.rpl` remains editable.
The application exposes mask operations in the selected layer's expandable
settings panel: Add, Edit/Stop, Enable/Disable, Invert, Clear, and Remove are
direct actions rather than nested popovers. This keeps the painting mode and
metadata controls reachable when the Layers list is scrolled or the window is
compact, while all edits still enter normal document transactions.

Recovery writes a bounded append-only sequence of complete native snapshot records
alongside the existing atomic snapshot and verified backup. Replay selects the most
recent complete validated state, so retries are idempotent; a damaged tail falls
back to the previous record. This is not yet an operation-level journal and does
not retain undo history. See RPL-ALPHA-FORMAT.md for the record layout and limits.

Layer blend math is encoded-sRGB (not fully linear-light or ICC managed). Display
converts unpremultiplied samples to linear output for the sRGB surface. Export writes
straight-alpha PNG with sRGB/DPI metadata. See RPL-ALPHA-FORMAT.md for native storage,
locking, backups, fault boundaries and recovery limitations. The UI owns file paths,
modal input routing and preservation confirmations; it does not bypass transactions.

The `paint-document::editing` module implements portable pixel-center rectangle,
ellipse and bounded polygon selections, fill/delete, nearest-neighbor affine edits,
eyedropper and all-layer crop. Editing snapshots retain transient selection state
for undo, but the native v1 file intentionally skips it. Painting still traverses
Processor/Router and clips raster dabs against the active selection. Selection-tool
gestures are shell editing actions, not an alternative painting stream. Large lasso
fills/transforms prepare bounded bit masks with scanline intersections rather than
retesting every polygon edge for every output pixel. No feathering is claimed.

Line, rectangle and ellipse creation are separate raster editing operations, not
selections or vector objects. The workspace keeps drag previews transient; release
calls `Engine::draw_shape`, which clips antialiased coverage to the document and
active selection, checks the selected layer and global tile budget, then publishes
the affected tiles through one undo snapshot. Current brush color, opacity and size
provide the shape color, opacity and fixed outline width. Rectangle/ellipse may be
filled instead of outlined. Native save/reopen and PNG export see the same raster
pixels; shapes are not individually editable after commit.

Linear and radial gradients are likewise document-backed raster edits. Their stop
colors are sampled through a bounded 1025-entry ramp interpolated in linear-light
sRGB; the document's existing premultiplied encoded-sRGB source-over compositor
still governs blending against layer pixels. A linear drag defines a clamped
projection and a radial drag defines center plus radius. The active brush color is
the first stop, the workspace's selectable RGB color is the second, and brush
opacity controls overall alpha. Gradient writes honor the current pixel-center
selection, reject a hidden or locked active layer, obey the tile budget and create
one undo transaction only when pixels change. Native v1 storage and PNG need no
schema change because the result is ordinary raster tile data. The interactive
preview uses one transient texture capped at 512 pixels on its longest edge; it is
drawn in the canvas panel layer beneath floating references and tools, and never
enters document tiles or recovery state. Escape, pointer cancellation, or tool
interruption drops the provisional preview without committing artwork.

Smudge is an explicit, transient `StrokeMode` captured by `BeginStroke`, not a
brush-preset or native-schema field. It samples only the active layer's
premultiplied RGBA tile data (transparent regions included), carries that sample
along deterministic brush dabs, and writes a private COW tile map. Opacity is
smudge strength; flow controls pickup/mixing rate; size, hardness, smoothing and
the existing pressure toggles affect the raster path. Selection pixel centers,
document bounds, visibility/lock state, and the global tile budget remain in
force. Preview tiles replace the active layer's provisional composite; commit
publishes changed tiles in one undo transaction, while cancel, focus loss or
capture interruption discards them. Device inversion/eraser still overrides
Smudge. Because the current transaction model does not merge concurrent edits to
the same layer, a second contact is ignored while Smudge owns the layer; committed
pixels remain ordinary v1 raster data and save/export without migration.

Color Mixing is a second transient `StrokeMode`, also captured at stroke begin
and excluded from the `.rpl` and brush-preset formats. It blends the active brush
RGB into each covered selected-layer pixel, then composites a bounded pigment
deposit using brush opacity, flow, hardness, pressure, shape, tilt and selection
coverage. Unlike Smudge it introduces the active brush color; unlike ordinary
painting it modifies the selected layer's existing pixel colors. It uses a
private copy-on-write tile map and exclusive layer ownership for preview,
commit, cancel and undo safety. The current mixer operates on encoded sRGB byte
values and is not a wet-media or color-managed pigment simulation. Existing
documents remain compatible because only the resulting raster tiles are saved.

The Windows WM_POINTER observer remains the sole native pen-capture path and
continues to provide barrel state, pressure, tilt, history, capture loss and
promoted-mouse suppression. A platform-independent `BarrelRouter` assigns that
already-normalized button to Eraser, Eyedropper, Pan or Disabled. `CanvasSession`
turns the route into ordinary Up/Down/Hover samples before the existing
Processor → Stroke Router → document path; temporary Eyedropper/Pan contacts
stay suppressed until lift. The Studio applies bounded pan deltas and persists
the chosen action in workspace preferences. Physical side-button behavior is
not yet hardware-validated; native pointer capture and its adapter are unchanged.

`paint-document::openraster` is an explicit interchange boundary, not native storage.
It converts supported raster layers to individual PNG assets plus stack.xml and
rejects unsupported editing structures. The shell imports into a fresh unsaved
native document, preserving the prior project/recovery lineage. See openraster.md.

Timing now has two explicit representations: normalized lifecycle milliseconds for
ordering/rollover, and optional exact `SourceCounter { ticks, ticks_per_second }` for
future velocity/latency work. Windows attaches POINTER_INFO.PerformanceCount plus
QueryPerformanceFrequency when available; no conversion to coarse mouse timing.
The shared structs/router preserve it, and replay v2 supplies deterministic coverage.
Native v1/v2 documents persist raster artwork and (in v2) editable raster masks,
not raw input or performance counters.

Custom brush settings use the existing version-1 `Brush::encode/decode` schema and
are stored as standalone `.rplbrush` JSON files. The document storage boundary
validates extension/schema/settings, bounds reads to 16 KiB, and publishes with a
same-directory atomic no-clobber hard link. The GUI provides typed-path save/load;
loading changes the active brush and an ordinary project save stores those settings
in the document metadata. This keeps brush presets portable without coupling the
brush engine to dialogs or platform APIs. Filesystems without hard-link support
fail safely, and an existing preset must be given a different filename.

The Studio workspace stores theme, Inspector tab/position/size, toolbar position
and dock, canvas zoom/pan, handedness, reference selection/pinning/PIP position,
and reference image paths and view settings in a separate versioned
`workspace.json`. Preferences never enter artwork files. The unified Inspector has
Layers, Brush, Edit, Color, and References tabs. On roomy windows it docks on the
right while preserving a 320-point canvas viewport; compact windows use a bounded
drawer or full sheet. Responsive geometry does not replace the saved roomy panel
preferences. The Layers tab presents the stack before selected-layer name,
opacity, blend and mask controls; row actions remain available from accessible
context menus. The primary toolbar remains independently movable and edge-snapping.

Reference images decode into bounded egui textures. The References Inspector holds
up to six workspace-only images with a selected preview, thumbnail navigation,
opacity, zoom, pan, centering and removal. They are not document layers and cannot
enter artwork composition, recovery records or exports. References are hidden
outside that board by default. Explicit pinning shows only the selected image in a
movable canvas picture-in-picture, sized to at most 24% of the unobstructed canvas;
there are no automatic full-size reference windows. Each touch contact remains
captured to its starting surface until it ends; a split-surface gesture is
suppressed rather than affecting either reference or canvas. Canvas touch uses the
same navigation processor, and stylus-active touch is ignored. Where a backend
emits winit pinch/pan events, they route to the reference preview under the cursor
or canvas; pixel scrolling pans references/canvas, and Control plus scroll zooms
the canvas. Saved legacy reference-window geometry remains accepted by workspace
preferences but is not used by the canvas-first presentation.

## Phase 8 startup and file selection slice

The canvas shell starts with a full-window Home surface rather than making an
artwork document or its placeholder visible behind an opening modal. It offers
prominent new/open actions, recent-project cards with bounded header-only
dimension/DPI labels, file size, persisted last-opened age, and explicit missing-file
state, plus recoverable checkpoints and unnamed drafts. Existing `.rpl` recent
lists migrate with an unavailable timestamp until next opened. Header-only metadata
is informational: opening still uses the normal size, structure and checksum
validation. Document replacement still passes through the
existing preservation gate. The creation dialog resolves physical units to pixels
using the requested DPI, validates the 4096-pixel edge limit before allocation,
and only exposes the existing sRGB RGBA8 background choices and transparency.
The sparse document engine allocates no pixel tiles at creation; its existing
8192-tile/128 MiB budget remains authoritative. `document_setup` holds this
conversion and validation outside platform and GPU code.

File actions request a typed intent from the workspace and call rfd at the shell
boundary. The selected path then enters the existing `Store`, PNG, OpenRaster,
brush-preset, or reference-image validation path; the picker does not weaken any
format or allocation limits. Multi-select is used for UI-only references, which
remain separate from document layers and export. Automated Xvfb workflow tests can
set `RPL_TEST_PATH_DIALOG=1` to use a path-entry harness; normal desktop operation
uses native dialogs. Linux desktop users need a functioning XDG portal backend.
The welcome and creation dialogs block canvas input until a document is selected
or created.

PNG/JPEG Place Image keeps its source RGBA bytes and bounded, 2048-edge preview
texture in temporary workspace state. Mouse or native pen contact on the canvas
moves the preview; the independent Reference board continues to be UI-only and
never becomes artwork. Fit/Fill/scale/rotation produce one source-to-document affine
matrix. Only the explicit Place action invokes `Engine::place_rgba_image`, which
clips to document bounds, enforces layer/tile limits, premultiplies alpha, and
creates one new raster layer inside a single ordinary engine edit. Cancel discards
the pending source and texture without changing document revision, history, or the
v1 `.rpl` schema. The raster transform currently uses nearest-neighbor sampling;
embedded ICC profiles and EXIF orientation are not applied.

OpenRaster layer exports convert only the populated sparse tiles of each layer to
straight-alpha PNG pixels. Visibility, opacity and blend mode remain in the ORA
stack metadata. The merged image is still rendered from the complete document.
This avoids constructing and revalidating a cloned document for every layer while
exporting; validation remains once at the start of the transaction.

Move Selection to New Layer shares the pixel-center selection geometry with Copy.
It clears selected nontransparent pixels from the active visible/unlocked source,
removes empty source tiles, and inserts the moved raster pixels immediately above
the source with the source opacity and blend mode. The editor's transaction snapshot
keeps undo exact; the final document tile count is checked before commit. No project
schema change is required.

Normal-close persistence stores a dirty named project through its existing
`Store`, preserving the content-hash conflict check, atomic replacement, and `.bak`
behavior. An unnamed project is encoded as a regular editable `.rpl` in a separate
local `drafts/` directory and is listed on the next welcome screen. The draft
manager bounds retained drafts to eight and uses generated, human-readable names;
when draft saving fails, close is blocked and the artist can retry, Save As, or
cancel. Recovery snapshots and journals remain a separate fallback path. This does
not claim protection from forced termination or sudden power loss beyond existing
recovery evidence.

## Phase 10 cubic path editing

Vector paths remain editable metadata attached to their owning raster layer;
raster tiles are derived for compositing and export. The V tool can select,
move, style, insert and remove supported anchors. For native v3 paths that
already contain cubic segments, the selected anchor exposes independent
incoming/outgoing control points. Artists can drag a handle on the canvas or
select it and enter document-space X/Y coordinates with an explicit Apply
action. `VectorHandle` identifies the adjacent segment endpoint relationship;
the document transaction changes only that control point and rejects malformed
coordinates or locked/hidden owner layers. A provisional handle preview is
rendered before commit; Escape or input interruption discards it. Undo/Redo
retains full editable path geometry. The path tool can convert line segments to
cubics, insert/remove supported anchors, move anchors and handles, edit stroke
color/width, close paths, apply or remove a fill, and apply uniform
scale/rotation or duplicate a selected path as undoable document transactions.
These remain editable vector metadata on their owning raster layer; the
operations do not bake into authoritative raster tiles. Unfilled paths retain
the v3 schema, while a filled path opts into v4 so an older application rejects
the file instead of silently dropping the style. Fill membership uses an
even-odd rule over deterministic flattened geometry, with a bounded pixel-edge
work estimate. Fill edges are sampled at pixel centers; advanced vector
antialiasing and SVG interoperability remain future work. No GPU/windowing
dependencies are added to `paint-document`.

Native document JSON uses serde_json's `float_roundtrip` feature. This prevents
repeated load/save cycles from perturbing cubic control coordinates and makes
save/undo/redo stable for mixed raster/vector documents. It does not alter v1,
v2, v3, or v4 schemas and improves numeric decoding for earlier supported formats.
