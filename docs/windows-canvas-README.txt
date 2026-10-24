Rust Paint Lab — portable Windows painting alpha

Run canvas-demo.exe on Windows 10 x64. This unsigned development build needs no
Rust or Visual Studio installation, installer, or administrator privileges. Use a
compatible Direct3D 12 adapter/driver; system graphics components and GPU drivers
remain required. Run it from PowerShell to retain startup diagnostics.

The application opens on a welcome screen. Choose New Document to set a name,
pixel or physical dimensions, DPI, preset, orientation, and background, or choose
Open Existing Project. Ctrl+N opens document setup. File operations use Windows'
native file picker; canceling leaves the current artwork unchanged.

Draw with the left mouse button or Windows pen inside the paper. Mouse input has
no pressure or tilt. Native Windows pen pressure and tilt are used when available;
real-device behavior still needs hardware testing. Touch does not paint. A failed
or interrupted stroke preview is canceled; completed strokes remain editable in
the current document session.

Use Browse / Add images to select one or more PNG/JPEG reference files (six maximum).
Reference windows can be moved, resized, assigned overlay
opacity, and removed. Their paths and layout are saved separately in local workspace
preferences. References are never document layers and do not appear in artwork
exports. Image files are limited to 32 MiB and decoded textures to 2048 × 2048 /
8 MiB each.

Theme and brush-panel width are also saved as workspace preferences. Project files
remain version-1 .rpl documents; PNG and OpenRaster are supported with documented
format limits. Local recovery keeps the legacy snapshot and verified .bak fallback
and appends bounded checksummed complete-state journal records. This is not an
operation history; a crash can lose edits since the latest completed checkpoint.
No raw input is automatically recorded.

Shortcuts: Ctrl+N new document, Ctrl+S save, Ctrl+O open, Ctrl+E PNG export, Ctrl+Z/Y undo/redo,
Ctrl+Shift+R recover, Ctrl+Shift+O open OpenRaster, Ctrl+Shift+E export OpenRaster,
F11 fullscreen. See USER-GUIDE.md for complete controls and limitations.

If startup or rendering fails, run .\canvas-demo.exe from PowerShell and retain
BUILD.txt. Include Windows version, GPU/driver, display scaling, and the error text
when diagnosing graphics compatibility. Direct3D 12 support varies by driver.
SmartScreen or organization policy may block unsigned development builds.
