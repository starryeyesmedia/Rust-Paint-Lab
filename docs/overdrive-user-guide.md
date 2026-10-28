# Overdrive painting alpha user guide

Build with pinned Rust 1.90: `cargo run --release --locked -p canvas-demo`.
Windows CI packages need no Rust, Visual Studio, installer or administrator rights.
Download the canvas artifact from a successful Overdrive branch Actions run,
extract all files, and launch `canvas-demo.exe`. Builds are unsigned and expire.

The app opens to a welcome surface with New Document, Open Existing Project, recent
saved projects, and detected recovery checkpoints. New Document supports a name,
pixel/inch/millimeter/centimeter units, DPI, starter presets, portrait/landscape swap,
RGB background, and transparency. It shows the resolved pixel size and rejects
dimensions over 4096 × 4096. DPI affects print-size conversion, not pixel detail.
The document is sparse at creation; painted tiles remain within the existing 128 MiB
tile budget. The last successfully created document's dimensions, units, DPI, background,
and transparency are remembered locally in workspace preferences. The last eight
created pixel dimensions/DPI pairs also appear as quick choices. Ctrl+N opens New Document.

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

Brush opens a floating Brush library from the primary tool palette, so preset
selection does not require a permanent right sidebar. It offers ten working
presets named Hard round, Ink pen, Soft brush and so on; Wash is marked
experimental. The flyout provides working size, opacity and color controls. The
Brush Library's Flow and dynamics section includes a Stabilization slider: zero
disables filtering; higher values smooth slow hand movement more strongly, while
faster movement stays responsive. The filter uses input timestamps and changes
position only; it does not invent pressure or tilt. The released endpoint is
always connected. This is adaptive stabilization, not stroke prediction.
Advanced brush settings in the inspector provide direct size and opacity entry
plus flow, hardness, spacing, tilt, angle and pressure responses. Color controls open
their own floating panel with an interactive hue slider and saturation/brightness
field, keyboard-adjustable saturation/brightness sliders, validated six-digit HEX
and exact RGB entry, eyedropper, current/previous color, and recent-color swatches.
Recent colors are newest first and can be selected by mouse or touch.
The list is capped at twelve unique local workspace colors and is independent of
the interface accent. Opacity caps the whole stroke while flow accumulates individual dabs. Wash
is a deterministic visual experiment, not a physical watercolor simulation. Airbrush
deposits on movement, not continuously while stationary. There is no prediction or
coalesced-history rendering.

Use **M** and **E** for rectangle and ellipse selections; they remain selection
tools. Use **L** for a straight line, **R** for a drawn rectangle and **O** for a
drawn ellipse. Drag on the paper to preview, then release to commit. **Shift**
constrains line angles or makes rectangle/ellipse proportions square or circular.
The contextual options row sets shape stroke width from the active brush size and
offers a Fill shape toggle for rectangles and ellipses; current brush color and
opacity apply. **Escape** cancels an in-progress preview. Shape edits respect the
selected layer's visibility/lock and active selection, are rasterized into that
layer, and form one Ctrl+Z/Ctrl+Y transaction. Saved projects and PNG exports
include committed shapes; shape geometry is not separately editable after commit.

Use **G** to choose the Gradient tool. Select **Linear** or **Radial** in the
contextual options, then drag across the canvas: linear uses the drag direction;
radial uses the first point as its center and the drag length as its radius. The
active brush color is the start stop; select the End swatch to choose the second
color, or use **Swap** to reverse the stops. Brush opacity controls the gradient's
overall alpha. A live preview follows the drag, and **Escape** cancels without
changing artwork. Gradients respect the current selection and require the selected
layer to be visible and unlocked. Releasing commits ordinary raster pixels as one
Undo/Redo step, saved in `.rpl` and included in PNG export; the stops and gradient
geometry are not retained as editable objects. Stop colors interpolate in
linear-light sRGB, while the document compositor remains encoded-sRGB and is not
fully color-managed.

Use **S** for Smudge. It samples and carries pixels from the active layer; the
current brush's size, hardness, smoothing and pressure toggles apply. In the
context row, **Strength** controls how much pixels move and **Pickup** controls
how quickly the carried color mixes with the layer under the stroke. Smudge
works through the same pen/mouse lifecycle, clips to the active selection,
previews transactionally, and commits as one undoable edit. It affects only the
selected visible unlocked layer; transparent pixels can receive carried pigment.
Device eraser/inversion takes precedence. Concurrent contacts are ignored while
a Smudge transaction is active to avoid conflicting edits to that layer.

Use **X** for Color Mixer. It deposits the active brush color while mixing it
with the existing pixels on the selected layer. **Mix** controls the color
blend, **Paint loading** controls deposited opacity together with the brush's
Opacity and Flow, and the brush size, hardness, smoothing, pressure and tilt
settings continue to shape the dabs. Unlike Smudge, Color Mixer introduces the
selected color; unlike an ordinary brush stroke, it works directly into the
existing layer pixels. It is a bounded raster operation, respects selection
and layer lock/visibility, previews as one transaction, and supports undo/redo
and ordinary `.rpl`/PNG persistence. Current mixing is performed in encoded
sRGB bytes; it is an initial practical color mixer, not a wet-paint simulator
or color-managed pigment model. Pressure toggles are available in the compact
Dynamics menu. Concurrent contacts are ignored while its private layer
transaction is active.

On Windows, **View → S Pen barrel button** assigns the native pen side button.
The default **Hold for Eraser** splits an active brush/Smudge/Color Mixer contact
at the button transition, commits the held eraser segment, then starts a fresh
segment with the previously selected tool on release. This preserves the
pointer lifecycle and makes each segment independently undoable. **Hold for
Eyedropper** samples the visible composite; **Hold to Pan** moves the canvas;
**Disabled** leaves the button unassigned. Eyedropper and Pan contacts are
suppressed until the physical pen lifts so they cannot restart a paint stroke.
Assignments are workspace preferences. The existing Windows pointer adapter
still owns native capture, pressure, tilt, inversion and capture-loss handling;
the new assignments need physical validation on the Galaxy Book2 Pro 360.

Layers support add, duplicate, delete, rename, reorder, visibility, lock, opacity,
Normal/Multiply/Screen/Overlay, and clear. The frontmost layer appears at the top;
use Raise/Lower to reorder, and expand Layer settings to rename or adjust opacity
and blend mode. Delete is disabled when only one layer remains. Bottom-to-top
composition follows layer order. Use Undo/Redo for committed strokes and supported
layer/background changes.
Locked/hidden layers reject drawing. Transparency shows a checkerboard. History is
session-only and bounded (up to 32 snapshots, 128 MiB unique tile payload per history queue);
large documents with many changed tiles can retain fewer steps. Limits produce an error, never silently
discard older committed artwork.

Use **Ctrl+B** or the Brush control in the primary palette to open the Brush
library; it can be resized and scrolls on narrow windows; Escape dismisses it.
Use **Ctrl+L** or the Layers palette control for the floating layer panel and
**Ctrl+Shift+N** to add a layer. The optional right-side inspector starts hidden
to leave the canvas and primary palette uncluttered; open it from Window when you
need advanced brush dynamics or the full layer editor. Use the compact File menu for New/Open/Save, image import/export, brush presets,
recent projects and the artwork recovery manager. Edit contains Undo/Redo and
selection fill/erase; Image contains Crop; Layer contains the existing layer
operations; View switches appearance and accessibility preferences; Window offers
workspace visibility, floating Layers, palette docking and handedness. Brush,
selection, shape and utility tools are in the rounded palette, left by default.
Drag an empty area
of it to move; it snaps to the nearest window edge on release and uses a
horizontal wrapped layout on top/bottom docks. Palette positions and dock choice
are remembered. Each icon has a name and shortcut
tooltip, and selection/utility actions sit below them. File actions open the
operating-system file browser.
The default shell appearance is Midnight Violet. View → Appearance selects Dark,
Light, or Follow System and one of the built-in Violet, Blue, Teal, Coral, Green,
Neutral, or Custom accents. Custom accepts a validated `#RRGGBB` value and updates
without restarting; appearance settings are stored in workspace preferences and do
not change document or brush colors. View → Accessibility offers high-contrast
controls, reduced motion and interface scaling (0.8×–1.6×). These are saved with
the appearance preferences and do not affect artwork. Older preference files
inherit their previous dark/light setting. The welcome screen's small preferences
shortcut still toggles dark/light directly.
Brush presets are versioned `.rplbrush` JSON files; load/save validates settings and
saving refuses to replace an existing preset. New/Open safeguard dirty artwork with
recovery confirmation. Save As and
PNG export refuse unrelated existing files; select a new path. **File → Import →
Place image (PNG/JPEG)** opens a temporary transform preview over the canvas. Drag the
preview or click a canvas location to position it; use Scale, Rotate, Fit, Fill,
Center, or Reset before choosing **Place on new layer**. Cancel or close the panel
without changing the document. Confirmation rasterizes the transform into one
independent editable layer and one Ctrl+Z transaction. The aspect ratio stays locked
while scaling. Images are limited to 4096 pixels per edge and 64 MiB decoded RGBA;
oversized inputs are rejected before artwork changes. PNG alpha is retained. JPEG
is decoded to 8-bit RGBA; embedded ICC profiles and EXIF orientation are not
interpreted. Save `.rpl` for editable layers, PNG for flattened images. Color is
encoded-sRGB RGBA8, not full ICC color management.
Document, file-path, recovery, close, and cleanup prompts are blocking modal dialogs;
the workspace behind them does not accept drawing or toolbar input. Saving an
untitled document after a PNG export chooses the matching `.rpl` project path.

Use **Browse / Add images…** to select one or more PNG/JPEG references (up to six).
Each floating reference window can be moved, resized, assigned an overlay opacity,
zoomed, panned, locked, or removed. Palette docking/position, handedness, tool-panel
placement, theme, accessibility settings, right-panel width, canvas view, new
document setup and reference arrangement are stored in a separate local
`workspace.json`; reference images stay linked to their
files and never become document layers or appear in PNG/OpenRaster exports. Files
are limited to 32 MiB, source dimensions to 16,384 pixels per edge and decoded source
memory to 64 MiB; larger images are reduced to a 1,448-pixel edge / 8 MiB texture
for display. Errors explain which limit was exceeded. Linux native
browsing requires an available XDG desktop portal service. The automated Xvfb workflow
uses `RPL_TEST_PATH_DIALOG=1` for a test-only path-entry harness.
Reference windows overlay the canvas but are routed as workspace UI: drawing,
selection and canvas navigation do not start under their visible bounds.

Keyboard: `[` / `]` reduce/increase brush size by ten percent; Shift+`[` / Shift+`]`
make fine one-percent changes. The size is bounded from 1 to 256 px and the
shortcut is ignored while a text field is active. Ctrl+Z/Y undo/redo; Ctrl+S save;
Ctrl+N New Document; Ctrl+O open project
picker; Ctrl+E export PNG; Ctrl+Shift+B saves a brush preset and Ctrl+Alt+B loads one;
Ctrl+L opens or closes the floating Layers panel; Ctrl+Shift+N adds a layer;
Ctrl+Shift+M opens or closes the
artwork recovery manager; Escape closes an active Brush/Color/Layers panel before
the application close workflow. Format and Brush preset cards accept Enter or
Space when focused. F11 fullscreen. L returns to live artwork; P/1/2/3 choose preserved
replays; Tab switches modes; R restarts; Space pauses. Replay never replaces live
artwork. Normal close saves named projects to their existing path, or writes an
unnamed document as a bounded local editable draft. If saving fails, the app stays
open and offers retry, Save As, or cancel.
`--compat-mouse` runs the original session-only Phase 5 diagnostic canvas.

## Saving and recovery

Save often and retain independent copies while this alpha format evolves. Saving
checks integrity, locks writers, detects external changes, synchronizes a temporary
file and retains the previous valid `.bak`. On normal close, a named project is
saved to its existing path. Untitled work is stored as a native `.rpl` draft under
the app's local state directory and appears on the welcome screen; up to eight
drafts are retained, and a full draft list prevents silent closure until Save As
succeeds or the user cancels. These are normal editable documents, separate from
disposable recovery checkpoints. Recovery writes local committed-document
snapshots plus a bounded, checksummed journal of complete states on a background
worker. Dirty revisions coalesce until 700 ms without a new edit before a checkpoint
is submitted; a busy writer or failure retries after two seconds. The footer shows
saved/unsaved state and when a recovery checkpoint is actively saving. These are
recovery checkpoints, not writes to the named project file: use Save for that.
A damaged journal tail falls back to its previous complete snapshot; the
existing recovery `.rpl` and `.bak` remain additional fallbacks. Closing or replacing
dirty work waits for a confirmed checkpoint. The journal is not an operation log
and does not preserve undo history; a crash can lose edits since the last completed
checkpoint. A forced kill or power loss during the debounce window can lose changes
since the last completed checkpoint. No raw input is automatically recorded.

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

B selects brush; S Smudge; X Color Mixer; M rectangle selection; E ellipse; K lasso; I eyedropper. Drag inside
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
If the latest journal entry is incomplete/corrupt, recovery uses the previous
complete journal state; if the journal is unusable, it tries the primary snapshot
and verified `.bak`. Status warns when recovery may have missed newer edits.
Neither source is rewritten. Save the restored work under a normal native filename.
A missing/corrupt primary plus missing/corrupt backup reports an error and keeps the
current artwork. Force-kill testing in CI uses disposable synthetic artwork only.

Native pen feedback includes an uncluttered hover/brush-size ring and a fixed-height
status line for pressure, each tilt axis, barrel and eraser/inversion. Unsupported
values say “unavailable”; hovering does not paint. The ring is the selected full
brush diameter, not a predicted committed stroke. Native source-counter precision
is retained through shared commands for future latency/dynamics work. Physical
hover/pressure/tilt behavior of this alpha still requires the Windows checklist.
