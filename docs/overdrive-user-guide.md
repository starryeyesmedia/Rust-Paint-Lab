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
Cancellation (focus/capture loss, leaving paper, resize/mode changes) removes the
whole provisional stroke. Completed strokes remain in the editable document.

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
session-only and bounded (up to 32 snapshots, conservative 128 MiB per history queue);
large documents can retain far fewer steps. Limits produce an error, never silently
discard older committed artwork.

Toolbar: New (dimensions/DPI), Open, Save, Save As, Export PNG, Import PNG,
Undo/Redo, Dark/Light. File dialogs currently accept typed paths, not a native file
browser. New/Open safeguard dirty artwork with recovery confirmation. Save As and
PNG export refuse unrelated existing files; select a new path. PNG import creates a
layer at the top-left; oversized inputs are rejected. Save `.rpl` for editable layers,
PNG for flattened images. Color is encoded-sRGB RGBA8, not full ICC color management.

Keyboard: Ctrl+Z/Y undo/redo; Ctrl+S save; Ctrl+O open path dialog, Enter confirms;
Ctrl+E export PNG; F11 fullscreen. L returns to live artwork; P/1/2/3 choose preserved
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
3. Alt-Tab, minimize, leave paper, resize/change DPI and transfer capture mid-stroke:
   previews must cancel, completed artwork must remain; fresh contact starts cleanly.
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
