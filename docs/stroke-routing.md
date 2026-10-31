# Stroke command routing

`paint-stroke` is a separate, safe, platform-independent crate depending only on
local `paint-input`. It contains a router and coordinate types, not graphics,
windowing, a brush engine, document mutations, persistence, or undo history.

## Integration

```
PointerSample -> Processor -> ordered lifecycle Output -> Router -> StrokeCommand
```

Feed one Router all outputs from one Processor, in order, exactly once. Keep
cancellation outputs; do not reconstruct input from mouse-like UI events. Use a
fresh Processor/Router pair for each replay/session; namespace IDs when combining
sessions. The router returns zero or one command per lifecycle output. Processor
can emit Cancelled followed by Started for a repeated Down; routing both produces
CancelStroke followed by a fresh BeginStroke.

Existing v1 replay fixtures require no schema change:

```rust
use paint_input::replay::Replay;
use paint_stroke::Router;

let replay: Replay = "paint-input-replay 1\n".parse().unwrap();
let mut router = Router::default();
let commands: Vec<_> = replay.run().iter()
    .filter_map(|output| router.route(output)).collect();
assert!(commands.is_empty());
```

The `stroke_commands` Cargo example uses the existing synthetic pen fixture and
prints its four commands. Tests assert exact streams for complete, mixed, mouse,
and interrupted fixtures. No input is recorded automatically.

## Command and transaction model

| Command | Consumer responsibility |
| --- | --- |
| BeginStroke | Allocate an isolated provisional transaction and start a preview |
| BreakStrokeSegment | Continue the same provisional transaction as a disconnected subpath; reset interpolation without commit/cancel |
| UpdateStroke | Extend only that provisional transaction |
| CommitStroke | Include the final sample, finalize that transaction exactly once, then retire its preview |
| CancelStroke | Discard that transaction and preview; do not add completed document content |

StrokeId is an increasing u64 identity within one Router session, distinct from
the reusable pointer ID. It never wraps (new strokes stop at counter exhaustion).
Key provisional state by StrokeId. Consumers merging independent routers must
provide a session namespace rather than assuming IDs are globally unique.

Begin/Update/Commit carry StrokeSample: pointer ID, tool, physical client position,
DocumentPoint, pressure and independent tilt availability, buttons, original
sample time, and lifecycle dispatch time. Cancel carries the last *accepted*
StrokeSample, dispatch time, and reason. It does not invent a lift position or
pressure. Reasons distinguish input cancellation/capture/interruption/EOF from
invalid routing data, invalid coordinate mapping, mouse primary release, tool
mismatch, and explicit router teardown. Pen pressure and tilt pass through;
mouse commands deliberately carry None for both, even if supplied upstream.

The router stores only active strokes, not their full paths or completed data.
A future brush engine consumes samples into provisional geometry/dabs. The future
renderer displays committed content and each provisional preview separately.
Cancel must be able to remove a preview without contaminating committed content;
do not draw irreversible preview pixels into a committed target. On Commit the
document engine accepts finalized brush pixels and the renderer retires the preview
without double drawing it. The Router itself keeps no transaction payload, brush
implementation, or undo stack.

`BreakStrokeSegment` is an adapter-directed path discontinuity for cases such as
artboard clipping. The app clips physical pointer motion to its transformed paper
rectangle. Before routing a re-entry sample, it calls
`Router::break_before_next_move(pointer_id)`. The next valid Move retains its
`StrokeId` and sample attributes but tells downstream consumers not to interpolate
from the prior in-paper point. The document brush resets segment spacing and the
legacy canvas stores a path break. This is not an input lifecycle transition and
does not alter the Processor contact. Focus loss, capture loss, leaving the window,
and invalid data still produce true cancellation. A contact beginning outside
paper never starts a painting transaction.

## Routing policy

- Pen Started begins a stroke, Moved updates it, Ended commits it. Barrel, eraser,
  and inversion flags are preserved as data; no brush/tool selection is implied.
- Hover never starts painting. Touch and unknown tools are reserved, regardless of
  primary flags. A future navigation consumer may observe the same lifecycle stream.
- Mouse requires primary=true on Started and Moved. Secondary-only contact cannot
  start painting. Losing primary during Moved cancels; a later primary Move cannot
  invent a new stroke without a fresh Started. Ended may release primary and commit.
- Duplicate Started on an already active ID is ignored; repeated identical Moved
  with the same sample and dispatch time is ignored. Orphan updates and repeated
  terminal events are ignored. Equal-time distinct samples retain arrival order.
- Stale noncancel lifecycle outputs are ignored. A matching cancellation still
  aborts locally at a nondecreasing command time. Cancellation must carry the exact
  last raw sample seen for that transaction, as Processor guarantees; mismatching
  old cancellation data cannot abort a reused ID's new stroke.
- Malformed phase/contact/numeric data for an active ID cancels, never commits.
  A tool mismatch during update/end cancels. Nonfinite transformed positions cancel
  an active stroke; an unmappable start is ignored without consuming a stroke ID.
- At most 64 painting transactions are retained (the Processor contact bound).
  New starts over capacity are ignored; completed/cancelled state is removed.
  Router::interrupt is a shell teardown safeguard that cancels all remaining
  transactions in ascending pointer-ID order. No background timeout is used.

Defensive duplicate handling is not a durable event deduplication service. There
is no source generation number or unbounded tombstone history: byte-identical
same-ID/same-time reuse cannot be distinguished from an old retransmission after
completion. Preserve the single ordered lifecycle stream contract. Processor's
conservative treatment of stale raw terminal samples also remains unchanged.

## Coordinates and viewport changes

Viewport::new takes a finite physical-client origin and finite zoom > 0. The
origin is the client-pixel position of document (0,0), not a document offset:

```
document = (physical_client_pixels - origin_client_px) / zoom
```

+x points right and +y down. At zoom=1, one document unit occupies one physical
client pixel. Translation [10,-20] and zoom=2 maps client [14,-12] to document
[2,4]. This is not a DIP/DPI conversion, perspective transform, rotation, clipping,
or gesture controller. Negative/out-of-window positions are valid finite data at
this shared router boundary; the canvas adapter clips live samples to its paper.

Viewport fields are private so invalid values cannot be introduced after
construction. Mapping rejects nonfinite source points and finite arithmetic that
overflows. DocumentPoint is a distinct output type, and commands retain original
client positions for inspection. Each active stroke captures its viewport at
Begin; set_viewport affects only future starts. Navigation should be disabled or
explicitly coordinated during strokes until a different tested policy is chosen.
No silent mid-stroke remapping is allowed. Renderer conversion to f32/GPU ranges
will be a separate validated boundary.

## Current limitations and next milestone

No UI hit testing, gestures, palm rejection, pressure curves, prediction, or native
pen window adapter live in this crate. The router remains graphics-independent.
Its commands feed the [canvas and document engine](architecture.md), which keep
provisional content separate through commit/cancel and reset raster spacing at a
clipped path break. The native diagnostic remains a separate app. Tests are
synthetic and do not verify Galaxy Book pen hardware.
