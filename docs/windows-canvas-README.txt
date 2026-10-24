Rust Paint Lab - Interactive Mouse Canvas (Phase 5 development build)

RUN ON WINDOWS 10
Extract the entire ZIP into a normal user folder and run canvas-demo.exe.
No local Rust, Visual Studio, installer, administrator rights, or accounts are
required. A console is normal; it prints GPU/backend information and errors.
Keep BUILD.txt, SHA256SUMS.txt, and THIRD-PARTY-NOTICES.txt with the executable.
Requires Windows 10 x64, a compatible Direct3D 12 graphics adapter, and its driver.
The static C runtime and Windows-supplied FXC compiler avoid a separate VC++
redistributable or DXC bundle. Windows graphics libraries/drivers remain required.

LIVE DRAWING (DEFAULT)
Move inside the light paper and drag the LEFT mouse button. Orange is the active
preview; releasing commits it in blue. Hover and right-click do not paint.
Pressure and tilt are unavailable for mouse strokes. This is not native pen input.
Only session memory retains completed strokes; closing loses everything.

CONTROLS
C: clear the current live canvas, including any preview (LIVE only).
L: switch to LIVE drawing, retaining earlier completed live strokes.
P: select/restart replay. 1/2/3: select replay scenario A/B/C.
Tab: switch modes. R: restart replay. Space: pause/resume replay.
Escape or the close button: cancel active work and close.

Replay A: orange zigzag becomes blue and remains.
Replay B: orange preview grows, holds, then disappears at capture loss (~2.6s).
Replay C: two pen paths commit separately; simulated touch produces no geometry.
Switching modes discards interrupted previews, never completed live work.

TEST ON YOUR GALAXY BOOK2 PRO 360
The user has confirmed Phase 4 GPU/replay/scaling operation and real pressure/
tilt in the separate native diagnostic. Phase 5 mouse/capture behavior needs its
own testing; prior results do not prove integrated pen painting.
- Draw several left-button strokes. Check final points, separate strokes, and no
  painting on hover/right-click. Mouse input may come from a mouse or trackpad.
- Start a stroke and Alt-Tab, minimize, move outside paper/window, or switch mode.
  The WHOLE interrupted preview should disappear; returning while held must not
  resume it. Release, move inside, and make a fresh press to draw again.
- Resize or change display scaling. Completed geometry should keep its document
  position; resize/DPI changes during a stroke cancel that preview safely.
- Switch to replay and back with L; live completed strokes should remain. Test C
  both while idle and during a stroke. Close/relaunch should start an empty canvas.
- Native Windows capture transfer without focus loss still needs a targeted
  manual check when another window/tool can steal capture.

FAILURES
If startup closes immediately, run from PowerShell to keep errors visible:
  .\canvas-demo.exe
Record BUILD.txt commit, Windows/driver version, scaling, mode, and reproduction
steps for blank paper, orphan previews, unexpected commits, flicker, or exits.
Check the console-selected adapter. A driver-supported Vulkan alternative is:
  $env:WGPU_BACKEND = 'vulkan'
  .\canvas-demo.exe
  Remove-Item Env:WGPU_BACKEND
Vulkan is optional and is not guaranteed to fix a driver issue. Restricted remote
or VM sessions may have no compatible adapter. Do not download random graphics DLLs.

LIMITATIONS
Boundary exit cancels the whole current stroke; drawing does not continue outside
the paper or automatically resume on reentry. Resize/mode switches cancel previews.
Rendering is fixed 8 document units wide with square joins/caps and no antialiasing.
Budgets: 64 strokes, 1024 samples per stroke, 8192 total samples. Very long/dense
strokes may be cancelled at the limit. Older commits remain; C clears the live
canvas to make room. Capacity feedback is nonfatal. Render size is capped at 4096
pixels per dimension. Device loss/initialization failures remain diagnostic exits.
No native S Pen painting, brush engine, layers, saving, undo, exports, or telemetry.

The executable is unsigned. SmartScreen/security policy may warn/block it; verify
repository/run/commit and follow your policy, without disabling protection.
Compare with SHA256SUMS.txt using:
  Get-FileHash .\canvas-demo.exe -Algorithm SHA256
A checksum is not a publisher signature. Artifacts expire after 30 days subject to
repository policy and may require GitHub sign-in. This is not a GitHub release.
The unchanged stylus diagnostic remains in rust-paint-lab-windows-diagnostic.
Original application licensing is undecided; third-party notices do not grant a
license to the project's original source.
