# Overdrive painting alpha user guide

Build with pinned Rust 1.90: `cargo run --release --locked -p canvas-demo`.
Windows CI packages need no Rust, Visual Studio, installer or administrator rights.
Download the canvas artifact from a successful Overdrive branch Actions run,
extract all files, and launch `canvas-demo.exe`. Builds are unsigned and expire.

## Drawing and controls

Draw inside the paper with the left mouse button or Windows pen. Mouse has no
pressure or tilt. Windows pressure affects size/opacity when enabled. Pen inversion/
eraser flags select erasing for that stroke. Hover/barrel state is retained; barrel
button has no separate tool action yet. Touch does not paint; gestures are pending.
Crossing the paper edge clips the visible stroke to the document and continues the
same contact on re-entry; no line bridges the outside gap. A press that begins
outside paper does not paint. Focus/capture loss, leaving the application window,
resize, and mode changes cancel the provisional stroke. Completed strokes remain
in the editable document.

The right panel contains ten brush presets, size/opacity/flow/hardness/spacing,
pressure responses, tilt rotation, angle, smoothing and RGB color. Opacity caps the
whole stroke while flow accumulates individual dabs. Wash is a deterministic visual
experiment, not a physical watercolor simulation. Airbrush deposits on movement,
not continuously while stationary. There is no prediction. Smoothing is a simple
low-pass filter, not a full stabilizer.

Layers support add, duplicate, delete, rename, reorder, visibility, lock, opacity,
Normal/Multiply/Screen/Overlay, and clear. Bottom-to-top composition follows layer
order. Use Undo/Redo for committed strokes and supported layer/background changes.
Locked/hidden layers reject drawing. Transparency shows a checkerboard. History is
session-only and bounded (up to 32 snapshots, 128 MiB unique tile payload per history queue);
large documents with many changed tiles can retain fewer steps. Limits produce an error, never silently
discard older committed artwork.

Toolbar: New (dimensions/DPI), Open, Save, Save As, Export PNG, Import PNG,
Save brush, Load brush, Undo/Redo, Dark/Light. Brush presets are versioned `.rplbrush`
JSON files; load/save validates settings and saving refuses to replace an existing
preset. File dialogs currently accept typed paths, not a native file
browser. New/Open safeguard dirty artwork with recovery confirmation. Save As and
PNG export refuse unrelated existing files; select a new path. PNG import creates a
layer at the top-left; oversized inputs are rejected. Save `.rpl` for editable layers,
PNG for flattened images. Color is encoded-sRGB RGBA8, not full ICC color management.
Document, file-path, recovery, close, and cleanup prompts are blocking modal dialogs;
the workspace behind them does not accept drawing or toolbar input. Saving an
untitled document after a PNG export chooses the matching `.rpl` project path.

Keyboard: Ctrl+Z/Y undo/redo; Ctrl+S save; Ctrl+O open path dialog, Enter confirms;
Ctrl+E export PNG; Ctrl+Shift+B saves a brush preset and Ctrl+Alt+B loads one by path;
F11 fullscreen. L returns to live artwork; P/1/2/3 choose preserved
replays; Tab switches modes; R restarts; Space pauses. Replay never replaces live
artwork. Close/Escape prompts for dirty work; save, keep recovery, or cancel.
`--compat-mouse` runs the original session-only Phase 5 diagnostic canvas.

## Saving and recovery

Save often and retain independent copies while this alpha format evolves. Saving
checks integrity, locks writers, detects external changes, synchronizes a temporary
file and retains the previous valid `.bak`. Recovery writes local committed-document
snapshots on a bounded worker. Closing or replacing dirty work waits for a confirmed
checkpoint. A crash can lose edits since the last completed checkpoint; this is not
a per-operation journal. No raw input is automatically recorded.

Recovery choices appear in the right panel on next launch. Recover into a new
unsaved document and Save As a normal `.rpl`. Each document switch keeps its own
recovery lineage. Locations: `%LOCALAPPDATA%/RustPaintLab/recovery`, Linux
`$XDG_STATE_HOME/rust-paint-lab/recovery` (or `~/.local/state/...`), macOS
`~/Library/Application Support/RustPaintLab/recovery`. Checkpoint cleanup is manual;
only remove old recovery files after verifying an independent saved project. Do not
remove files belonging to another running instance. Disk-full/permission errors
appear in status; destructive actions stop if preservation cannot succeed.

## Physical Windows acceptance checklist — not yet verified

1. Launch on Iris Xe / RTX 3060; inspect console adapter and BUILD.txt commit.
2. Draw slow/fast pressure ramps, hover, independently tilt X/Y, test barrel and
   inversion where supported. No duplicate mouse strokes should appear.
3. Cross the paper edge and re-enter during a stroke; verify clipping, continuation,
   and no bridge across the gap. Alt-Tab, minimize, leave the window, resize/change
   DPI, and transfer capture mid-stroke: previews must cancel and completed artwork
   must remain; fresh contact starts cleanly.
4. Try each brush; change size/color/flow/opacity and pressure toggles. Erase on a
   transparent layer, cancel erasing, then undo/redo committed erasing.
5. Add/rename/duplicate/reorder/hide/lock layers, vary blend/opacity, save/reopen.
   Verify exact dimensions, layer structure and visible appearance.
6. Export PNG, open in an independent image viewer, check transparency and size.
7. Save to a normal user folder repeatedly, inspect `.bak`, close/recover unsaved
   work; use disposable artwork for process-kill and disk-full recovery testing.
8. Exercise 2048/4096 documents, long strokes and large brushes. Record frame-time
   status, working-set growth and driver details; responsive 60 FPS is a target,
   not yet a measured hardware guarantee.

Known gaps and continuation steps: [progress](OVERDRIVE-PROGRESS.md).
Standalone `stylus-diagnostic.exe` remains separately packaged and unchanged.

## Selection and editing slice

B selects brush; M rectangle selection; E ellipse; K lasso; I eyedropper. Drag inside
paper to select, then switch back to Brush to constrain painting to that selection.
Selection outlines are black/white. Lasso is capped at 1024 points (2 document-pixel
minimum spacing); reaching the cap cancels the whole lasso with a visible status
message instead of committing a truncated boundary. Complex shapes use even/odd
pixel-center boundaries, no feathering.
Leaving paper, focus loss, resize or mode change cancels an incomplete selection.

Ctrl+F fills selected pixels (or whole selected layer if nothing is selected) with
current color/opacity. Ctrl+X deletes selected pixels; this is **not clipboard cut**.
Ctrl+D deselects. Toolbar buttons perform the same edits. Eyedropper samples the
visible composite and rejects fully transparent pixels.

Expand “Edit selected pixels” for move X/Y, uniform scale, rotation, horizontal/
vertical flip and crop. Apply transform affects the selected layer, using nearest-
neighbor sampling; content moved outside the document is clipped. Crop uses the
selection bounding rectangle and affects all layers. Successful operations enter
undo history, including dimensions/layer pixels; cancellation never does. Transform
clears the selection; undo restores it. Selections are transient and not saved in
`.rpl`; reopening preserves artwork, not a selection outline. There is no selection
feathering, arbitrary transform handles or clipboard integration yet.

## Canvas navigation

Mouse wheel over paper zooms around the cursor; middle-button drag pans. “Fit 0”
or the 0 key resets the view. Zoom is 0.25–8 times the fit scale. Two touch contacts
pan/pinch only while no painting transaction is active. A single finger never paints;
third/additional contacts suspend gesture deltas. Touch state is bounded and clears
on interruption. This is a basic palm-avoidance policy, not anatomical palm rejection.
Actual Windows multi-touch delivery and simultaneous S Pen behavior need hardware
testing. Navigation cancels a provisional stroke before changing its transform and
never edits committed pixels or enters history. Canvas rotation is not implemented.

Recent projects appear in the right panel after a successful native save/open;
paths are local preferences, capped at 20, never sent anywhere. Recovery “Delete…”
opens an explicit confirmation for a selected inactive checkpoint and its backup.
Verify a separate saved project first. Active owners and ordinary artwork filenames
are refused. There is no automatic age-based deletion; preservation takes priority.

OpenRaster: **Export ORA** / Ctrl+Shift+E keeps supported raster layers; **Open ORA** /
Ctrl+Shift+O imports into a new unsaved native document. Background becomes a layer;
locks, brush presets and history are omitted. Groups/masks/nonzero offsets and
unsupported blend modes are rejected. Save `.rpl` afterward. See openraster.md for
exact capabilities/limits. External application interoperability still needs testing.

Ctrl+Shift+R restores the latest inactive recovery into a new unsaved document.
If the primary snapshot is corrupt or missing, a verified `.bak` is offered through
the same recovery entry; status explicitly warns that newer edits may be missing.
Neither source is rewritten. Save the restored work under a normal native filename.
A missing/corrupt primary plus missing/corrupt backup reports an error and keeps the
current artwork. Force-kill testing in CI uses disposable synthetic artwork only.

Native pen feedback includes an uncluttered hover/brush-size ring and a fixed-height
status line for pressure, each tilt axis, barrel and eraser/inversion. Unsupported
values say “unavailable”; hovering does not paint. The ring is the selected full
brush diameter, not a predicted committed stroke. Native source-counter precision
is retained through shared commands for future latency/dynamics work. Physical
hover/pressure/tilt behavior of this alpha still requires the Windows checklist.
