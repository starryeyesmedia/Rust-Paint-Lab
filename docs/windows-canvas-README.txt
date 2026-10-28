BitchPixel — portable Windows creative studio

Rust Paint Lab remains the repository and internal engineering name. This is an
unsigned evaluation build; it is not an official release.

Double-click canvas-demo.exe on Windows 10 x64. It is built as a Windows GUI
application and does not open or depend on a command terminal. This unsigned
development build needs no Rust or Visual Studio installation, installer, or
administrator privileges. Use a compatible Direct3D 12 adapter/driver; system
graphics components and GPU drivers remain required.

The application opens on a welcome screen. Choose New Document to set a name,
pixel or physical dimensions, DPI, preset, orientation, and background, or choose
Open Existing Project. Ctrl+N opens document setup. File operations use Windows'
native file picker; canceling leaves the current artwork unchanged.

Draw with the left mouse button or Windows pen inside the paper. Mouse input has
no pressure or tilt. Native Windows pen pressure and tilt are used when available;
basic S Pen drawing has been observed on the Galaxy Book2 Pro 360 in earlier
builds. Advanced pressure response, independent tilt axes, palm rejection,
pen/touch concurrency and high-DPI behavior have not been individually rechecked
on this build. Touch does not paint. A failed or interrupted stroke preview is
canceled; completed strokes remain editable in the current document session.

Use Browse / Add images to select one or more PNG/JPEG reference files (six maximum).
Reference windows can be moved, resized, assigned overlay
opacity, and removed. Their paths and layout are saved separately in local workspace
preferences. References are never document layers and do not appear in artwork
exports. Image files are limited to 32 MiB and decoded textures to 2048 × 2048 /
8 MiB each.

The movable tool palette, handedness, theme/accessibility choices, panel placement
and sizing, reference-board layout, canvas view, recent colors, and document setup
are saved as local workspace preferences, independently from artwork. The Brush
library opens from the primary palette; Color opens its own flyout; Layers opens a
movable panel with common layer actions and remains available in the detailed right
inspector. Escape closes a contextual panel before beginning the app-close flow.
Project files remain version-1 .rpl documents; PNG and OpenRaster are supported
with documented format limits. Local recovery keeps the legacy snapshot and
verified .bak fallback and appends bounded checksummed complete-state journal
records. This is not an operation history; a crash can lose edits since the latest
completed checkpoint. No raw input is automatically recorded.

Shortcuts: Ctrl+N new document, B Brush library, C Color controls, Ctrl+S save, Ctrl+O open, Ctrl+E PNG export, Ctrl+Z/Y undo/redo,
Ctrl+L floating Layers panel, Ctrl+Shift+M artwork recovery manager, Ctrl+Shift+R quick recover,
Ctrl+Shift+O open OpenRaster, Ctrl+Shift+E export OpenRaster, F11 fullscreen. Escape
closes a contextual Brush/Color/Layers panel before closing the application. See
USER-GUIDE.md for complete controls and limitations.

If startup initialization fails, BitchPixel shows an error dialog and writes a
bounded diagnostic log to %LOCALAPPDATA%\BitchPixel\logs\canvas-demo.log. To
troubleshoot graphics compatibility, run .\canvas-demo.exe from PowerShell and
retain that log plus BUILD.txt. Include Windows version, GPU/driver, display
scaling, and the error text. Direct3D 12 support varies by driver. SmartScreen or
organization policy may block unsigned development builds.

The Studio creates its AccessKit adapter while the native window is hidden and
shows the window only after GPU and UI initialization succeeds. A startup failure
closes the hidden window and reports the error in a BitchPixel dialog and log. See
docs/windows-startup-smoke.md for what the automated Windows launch check verifies
and its hosted-runner limitations.
