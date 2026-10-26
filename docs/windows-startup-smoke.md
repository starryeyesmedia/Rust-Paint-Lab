# Windows canvas startup smoke

The Windows Actions job runs the release `canvas-demo.exe` with
`--alpha-smoke` after packaging the Windows artifact. The test waits for all of
the following before it passes:

1. Windows reports a non-empty, visible top-level BitchPixel window belonging
   to the launched process.
2. The application confirms it reached the explicit show step after creating
   the Studio AccessKit adapter and configuring the GPU surface.
3. The application presents its first Studio frame and reaches the existing
   alpha smoke completion marker without the AccessKit visibility panic.
4. The process exits successfully.

The smoke runs in the GitHub runner's Windows session. It exercises window
creation, the AccessKit ordering, GPU adapter/surface initialization, and the
first UI render. It does not establish that a person can see the window on a
physical monitor, inspect the screenshot visually, validate a particular GPU
driver, or exercise a physical stylus. If the runner reports that no compatible
wgpu adapter exists, CI records the GUI smoke as inconclusive; it does not count
the executable build as a GUI pass.

For a stronger Windows GUI check, use a dedicated self-hosted Windows x64
runner with an interactive desktop session, a supported graphics adapter, and
the runner process attached to that same logged-in session (not Session 0).
Run the packaged application under a desktop automation harness, assert the
visible BitchPixel top-level window and first-frame marker, and capture the
desktop or window image for visual inspection. Keep that runner isolated from
untrusted pull requests. The ordinary hosted build and portable artifact job
must continue to run even when no dedicated GUI machine is available.

The startup smoke is enabled only by `RPL_STARTUP_SMOKE=1`; normal application
launches do not emit its markers or use its test exit behavior.
