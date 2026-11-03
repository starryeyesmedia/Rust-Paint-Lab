# Shared input adapter contract

Status: BP-PLAT-005 proposed contract, grounded in the existing `paint-input`,
`paint-stroke`, and canvas navigation APIs. Synthetic tests do not establish
Win32, Android, or physical stylus behavior.

## Boundary and ownership

Each platform adapter translates one ordered OS event stream into
`paint_input::PointerSample` values. It owns platform event extraction, active
pointer identity, capability discovery, and platform cancellation. A shared
`Processor` validates lifecycle and orders samples; `paint_stroke::Router`
turns eligible pen or primary mouse contacts into provisional stroke commands.
The shell owns UI hit testing, touch navigation, window behavior, and the final
decision about whether a contact belongs to a UI surface or the canvas.

There is one `Processor` and one `Router` per ordered adapter stream. Pointer
IDs are unique among active contacts in that stream and may be reused only
after `Up` or `Cancel`. IDs and source counters are not durable device
identifiers. If streams are combined, the shell must namespace IDs and keep
their clock domains separate.

## Sample semantics

| OS meaning | Shared sample | Required behavior |
| --- | --- | --- |
| Pen contact starts | `Pen / Down` | One Down; route only if the shell assigned canvas ownership. |
| Pen contact changes | `Pen / Move` | Keep original order and every historical sample. Preserve available values. |
| Pen leaves contact | `Pen / Up` | End only that pointer; never infer an Up from focus or capture loss. |
| Pen in range without contact | `Pen / Hover` | Update hover state only; never create a stroke. |
| Touch contact | `Touch / Down, Move, Up, Cancel` | Preserve lifecycle, but never route to painting. Shell may assign a touch gesture. |
| Primary mouse contact | `Mouse / Down, Move, Up` | May paint only with primary button and canvas ownership; pressure and tilt are `None`. |
| Unknown device | `Unknown / …` | Preserve lifecycle where possible; never paint. |
| OS cancellation | `… / Cancel` or processor interruption | Cancel active state; do not synthesize Down, Up, or Commit. |

`position_px` is expressed in physical pixels relative to the canvas window's
client origin. `pressure` is normalized to `[0, 1]` only when the platform
reports a usable pressure capability; missing remains `None`, and a reported
zero remains `Some(0.0)`. Tilt axes are independently optional and retain the
platform's documented signed-angle convention; do not derive two axes from a
single scalar unless the Android adapter documents and tests that conversion.
Buttons, including barrel and eraser state, are carried separately from tool
identity.

`timestamp_ms` is monotonic within one adapter stream. Preserve a higher
precision `SourceCounter` only in its own clock domain. Do not compare source
counters from different adapters. When multiple samples share the same
millisecond timestamp, retain their original arrival order and distinct source
counters; do not deduplicate merely because timestamps match.

Adapters that receive coalesced or historical samples emit them oldest first,
then emit the current sample. If bounded buffering overflows, cancel the
affected active pointer and expose a bounded diagnostic; do not silently skip
samples and then commit the resulting partial stroke. `paint-input` currently
provides bounded active-contact processing, not an OS event-history queue.

## Cancellation and transformation changes

Capture loss, focus loss, OS cancel, window teardown, and an adapter reset must
cancel affected active pointers. A valid new Down after cancellation starts a
fresh transaction, including when the OS reuses the pointer ID. An orphan Move
must not restart a stroke. Cancellation must not create a committed stroke.

If the window's coordinate mapping changes during a contact (DPI transition,
orientation change, surface resize, or view reparent), cancel the active
contact before installing the new transform unless the adapter can prove that
all samples in the transaction remain in the same physical client coordinate
space. The shared router captures its viewport at stroke Begin; the shell's
mapping update path must preserve that invariant.

## Coordinates, DPI, and orientation

The adapter converts native screen or view-local coordinates to physical
canvas-client pixels exactly once. The shell may then convert to egui points
using its current `pixels_per_point`; it must not rescale an already physical
sample. `paint-stroke::Viewport` maps client pixels to document coordinates and
does not apply DPI or rotation. On Android, `MotionEvent.getX/getY(pointerIndex)`
are view-local pixel coordinates; do not multiply by density again. If the
canvas is offset inside the Android view, subtract that offset once. Use the
current view dimensions after configuration/orientation changes.

The current shared viewport is axis-aligned. An adapter that receives rotated
display coordinates must normalize them into canvas-client orientation before
submitting samples. Orientation transforms must be tested as adapter
conversions; a `CanvasMapping` portrait/landscape round trip only verifies the
shared mapping after that conversion.

## Palm rejection and touch navigation

Prefer explicit OS cancellation/palm signals. Never relabel a touch pointer as
pen, and do not reject a contact solely because it is near a pen or occurs
while a pen is in range. A palm-rejection policy may suppress a touch gesture
while a canvas pen owns the interaction, but it must retain or explicitly
cancel the touch lifecycle and must not discard an independent legitimate
stroke. Touch remains nonpainting in the shared router.

Two-finger navigation requires two contacts owned by the same surface. A third
contact is suppressed for that gesture. A touch sequence keeps its initial
target through End or Cancel. Navigation samples must never be forwarded to
painting.

## UI and native-window ownership

The shell hit-tests on Down before forwarding a contact to the canvas. Toolbar,
menu, dialog, floating-panel, and Undo/Redo hit regions belong to UI for the
whole pointer sequence and must never also generate a canvas stroke. A
canvas-owned stroke keeps canvas ownership while crossing a floating panel so
that a moving overlay cannot steal or duplicate the stream. Pen and mouse use
the same ownership rule. Touch navigation is separately target-captured.

On Windows, Winit remains the native window and event-loop owner. The
`WM_POINTER` adapter observes pointer messages and forwards unhandled messages
to `DefSubclassProc`; it must not consume non-client/window-manager behavior
needed for dragging, Snap, or resizing. Keep native capture-loss cancellation
and use client coordinates in physical pixels. Apply DPI conversion once and
keep the existing Winit scale-factor behavior. Any change to window
non-client handling, capture policy, or shared shell event dispatch requires a
coordinated Desktop UI integration handoff.

Android MotionEvent adapter ownership belongs to Android Engineering. That
adapter should map `ACTION_DOWN` and `ACTION_POINTER_DOWN` to Down for the
changed pointer; expand historical samples in event order on Move; map only the
changed pointer for `ACTION_POINTER_UP`; cancel all active IDs on
`ACTION_CANCEL`; and represent stylus hover as nonpainting Pen Hover. Android
Engineering owns the Activity/View event loop, pointer-ID mapping, native
capability extraction, density/view-to-canvas conversion, and platform tests.
Stylus & Platform owns this shared contract and review of its adapter boundary;
it does not add a second Android event loop or renderer.

## Evidence and validation boundary

The synthetic suite in `apps/canvas-demo/tests/input_adapter_contract.rs`
checks shared lifecycle, routing, metadata, navigation, and coordinate mapping.
Existing `tests/pen.rs`, `tests/navigation.rs`, `crates/input/tests/processing.rs`,
and `crates/stroke/tests/routing.rs` also cover synthetic behavior. They do not
exercise Win32 window management, Android MotionEvent delivery, device palm
rejection, physical S Pen pressure/tilt, or actual hardware presentation
latency. Shell-level UI hit regions, especially Undo/Redo and floating panels,
still need deterministic event-routing tests in the shell owner’s files.

## Input-to-visible-stroke latency plan

Measure the interval from OS sample arrival to the first presented frame that
contains the corresponding visible stroke. Record dispatch-to-render-submit
and submit-to-present separately where platform tracing permits. Run at least
100 warmed-up stroke starts and short moves with a 256 px brush, reporting
P50/P95/P99 and maximum, brush mode, canvas size, display refresh rate, power
state, OS, app commit, and device class. Use QPC/ETW or presentation tracing on
Windows and Perfetto/frame timelines on Android; use monotonic timestamps in
the same clock domain. Avoid per-sample diagnostic logging during measurement.
No latency target or performance claim is established by this assignment.

## Manual hardware acceptance worksheet

Status: not run. Record the app commit, diagnostic commit, OS version, display
mode, scaling/density, refresh rate, brush preset/settings, and Pass/Fail/Blocked
for each case. Do not record serial numbers, device IDs, account names, or
personal files. Capture only bounded diagnostics and screenshots of test marks
when useful.

| Device/build | Action | Expected result | Evidence and failure symptoms |
| --- | --- | --- | --- |
| Galaxy Book2 Pro 360 + Samsung S Pen, Windows; Canvas Demo and `stylus-diagnostic` built from the same exact commit | At 256 px brush size, draw slow and fast lines from light to firm pressure. | Pressure responds according to the selected preset; hover remains nonpainting; separate strokes start immediately after lift. | Pass/Fail; diagnostic phase/capability/error categories and timestamp deltas. Fail on flat/missing pressure, hover marks, lost samples, or delayed restart. |
| Same | With a tilt-aware preset, draw at several pen angles and rotate the pen in both axes. | Available tilt axes change smoothly with stable sign convention; unsupported axes remain unavailable. | Pass/Fail; capability availability and bounded adapter errors. Fail on swapped/sign-flipped axes, jumps, or fabricated values. |
| Same, Windows display scaling set to 100%, then 150% | Repeat a short stroke near canvas edges; move the app between displays at different scale factors if available. | Pointer and visible stroke stay aligned without a scale jump. A scale transition during contact cancels safely. | Record scale, display refresh, and app commit. Fail on offset, double scaling, clipping, or a committed partial stroke after transition. |
| Same | Rest a hand on the display while drawing with the S Pen; then perform two-finger pan/zoom without the pen. | Pen stroke remains usable; touch never paints; two-finger input navigates only. | Pass/Fail; bounded modality/cancel diagnostics. Fail on palm marks, suppressed independent pen contact, or touch paint. |
| Same | Interrupt a stroke with Alt+Tab, window focus loss, capture change, and app close/reopen; lift and immediately draw again. | Interrupted provisional work cancels without commit; next valid Down starts a clean stroke. | Pass/Fail; cancellation reason category and post-interruption fresh stroke. Fail on stuck pointer, ghost segment, partial commit, or failure to restart. |
| Same | Tap/drag from Undo, Redo, menus, toolbar, window chrome, and floating-panel hit regions; then start on canvas and cross a floating panel. | UI-started contacts never paint. A canvas-owned stroke remains one canvas transaction across an overlay and does not invoke UI actions. | Pass/Fail; before/after undo state and canvas image. Fail on paint-through, duplicate stroke, unintended Undo/Redo, or lost owner. |
| Galaxy S23 Ultra + Samsung S Pen, Android; Android Engineer's Canvas Demo and diagnostic APK from the same exact adapter commit | Repeat pressure, tilt, hover, lift/restart, and interrupted-stroke cases. | Same shared contract as above, using the adapter's documented Android axis mapping and capability availability. | Record app/diagnostic commit and APK hashes, Android version, display size/density/orientation, and bounded diagnostic categories. Fail on dropped/reordered history, capability fabrication, stale contact, or restart failure. |
| Same | Rotate portrait/landscape; repeat strokes near all four canvas edges at each orientation. | Points align with the visible canvas after one view-to-canvas conversion; any transform change during contact cancels safely. | Pass/Fail with orientation and density only. Fail on rotated axes, offset, double density scaling, or an un-cancelled jump. |
| Same | Rest a hand while drawing; then use two-finger navigation and test toolbar, Undo/Redo, menus, and floating panels. | Palm contacts do not paint or suppress an independent pen stroke; touch navigation remains nonpainting; UI-owned contacts never reach the canvas. | Pass/Fail; bounded adapter events/cancel reasons and before/after undo state. Fail on palm marks, accidental paint, UI-through paint, or stuck contacts. |

The current repository contains no Android application, Gradle project, or
MotionEvent adapter. The Galaxy S23 Ultra rows therefore require the Android
Engineer to supply an Android app/diagnostic build and document its axis and
capability mapping before execution. BP-PLAT-005 does not provide that build.
