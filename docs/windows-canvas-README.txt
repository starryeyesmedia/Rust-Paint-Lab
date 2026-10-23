Rust Paint Lab - First GPU Canvas Demo (development build)

LAUNCH ON WINDOWS 10
Extract the whole ZIP into a normal user folder. Run canvas-demo.exe. A console
window is normal and prints the selected GPU/backend and errors. No local Rust,
Visual Studio, installer, administrator rights, or accounts are required. Keep
the bundled README, BUILD.txt, checksum, and THIRD-PARTY-NOTICES.txt together.

Requires Windows 10 x64 with a compatible Direct3D 12 graphics adapter and driver.
The initial manual target is Ryzen 7 5700X / NVIDIA RTX 3060, with current NVIDIA
drivers. Physical RTX 3060 and Galaxy Book/Iris Xe execution has NOT been verified
by software-rendered tests or CI compilation. Older drivers, Remote Desktop/VM
sessions, or restricted systems may not provide a usable adapter.

CONTROLS AND EXPECTED DISPLAY
1: Scenario A, completed pen. An orange fixed-width zigzag appears progressively,
   then becomes blue and remains. Pressure is retained but does not control width.
2: Scenario B, cancelled pen. An orange preview grows, holds briefly, then
   disappears completely at capture loss (~2.6 seconds). No blue stroke remains.
3: Scenario C, concurrent input. Two independent pen paths grow, then both become
   blue. Simulated touch in the center does not draw anything.
R: restart the selected scenario and clear its old state.
Space: pause/resume playback. Escape or the close button: exit.

Expect a light paper rectangle on a dark border/background. The title identifies
the scenario, playback status, color meanings, and keys. Resize the window to
check aspect-preserving document mapping. No live pen, mouse, or touch painting
is implemented; only keyboard controls affect the demo. Playback stops at the
end and waits for restart or scenario selection.

FAILURES AND TROUBLESHOOTING
If the window closes immediately, run from PowerShell in the extracted folder:
  .\canvas-demo.exe
Look for "Canvas adapter", "Canvas failed", shader validation, surface, or device
errors. Record the commit in BUILD.txt, Windows version, driver version, adapter,
backend, scenario, and reproduction steps. Missing strokes, a stuck orange path
after Scenario B finishes, flicker, or a blank canvas in A/C are failures.

Update your graphics driver if initialization fails. For a driver-supported
Vulkan alternative, try in PowerShell:
  $env:WGPU_BACKEND = 'vulkan'
  .\canvas-demo.exe
  Remove-Item Env:WGPU_BACKEND
Vulkan needs a compatible driver; it is not guaranteed to fix a driver problem.
The normal Direct3D path uses Windows' d3dcompiler_47.dll (FXC); no DXC DLL bundle
or separate Visual C++ redistributable is expected. The C runtime is static.
Windows system graphics libraries and vendor drivers are still required.

LIMITATIONS
Unsigned executable: SmartScreen or security policy may warn/block it. Verify the
repository/run/commit and follow organization policy; do not disable protection.
Compare the executable with SHA256SUMS.txt using:
  Get-FileHash .\canvas-demo.exe -Algorithm SHA256
A checksum is not a publisher signature. Artifacts expire after 30 days, subject
to repository policy, and downloading may require GitHub sign-in/access.

The demo fits an 800x500 document in the window. Strokes are 8 document units
wide (scaled with the canvas), opaque triangles with square caps/joins and no
antialiasing. Pressure/tilt are retained, not simulated as brush effects. Window
render size is capped at 4096 pixels per dimension, with bounded scene/vertex
budgets. Surface loss is retried a bounded number of times; device loss, validation
errors, exhausted scene budgets, or out-of-memory errors cause a diagnostic exit.

No layers, saving, undo, live input, brush engine, vector tools, telemetry, or
network services. This is a replay renderer, not a painting product. The separate
stylus-diagnostic.exe remains in rust-paint-lab-windows-diagnostic; the applications
are not combined. Third-party licenses are bundled; the original project's own
license is still undecided. No GitHub release is created.
