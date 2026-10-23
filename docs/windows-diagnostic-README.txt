Rust Paint Lab - Windows stylus diagnostic (development build)

REQUIREMENTS AND RUNNING
Windows 10 version 1703 or newer, x64 (Intel/AMD). Initial target: Samsung Galaxy
Book2 Pro 360. Extract the entire downloaded ZIP into a normal user folder.
Double-click stylus-diagnostic.exe. No installer, administrator rights, Rust,
Visual Studio, network connection, or separate VC++ redistributable is expected.
The C/C++ runtime is statically linked; Windows system DLLs and working pen
hardware/drivers are still required. Close the diagnostic window to exit.

A console window may also open; this is normal. If startup fails, run from
PowerShell in the extracted folder to keep the error visible:
  .\stylus-diagnostic.exe

This executable is unsigned. Windows SmartScreen or antivirus may warn, and
organization security policies may prevent execution. Verify the download's
repository/run/commit and follow your organization's policy. Do not disable
security protection to run it. BUILD.txt identifies the commit, compiler, and
DLL imports. SHA256SUMS.txt contains the executable checksum; compare with:
  Get-FileHash .\stylus-diagnostic.exe -Algorithm SHA256
The checksum helps detect changed files; it is not a publisher signature.

WHAT TO TEST ON THE GALAXY BOOK
- Hover, touch, draw, and lift: Pen tool, expected phases, contact/range changes.
- Vary light-to-firm pressure: normalized 0-1 values should vary; inspect raw
  pressure and capability mask. None means unavailable, not zero.
- Lean the pen in both directions: note signed tilt changes or unsupported axes.
- Press/release barrel button; test inversion/eraser only if the pen supports it.
- Drag outside the window, lift, and interrupt a contact with Alt-Tab: no stuck
  active contacts; capture loss should be reported as cancellation.
- Try two fingers and pen-plus-palm/finger: record distinct tool types/contact IDs.
- Resize, rotate the screen, and change display scaling; check client coordinates
  and readability. Try an external monitor if available.
- Make fast strokes and run for several minutes; record errors or missed events.

Record the commit from BUILD.txt, Windows version, driver versions, pen model,
scaling/orientation, expected/observed behavior, and screenshots of errors or raw
flags/masks. Do not include hardware serial numbers. Full checklist:
https://github.com/starryeyesmedia/Rust-Paint-Lab/blob/feature/project-foundation/docs/hardware-testing.md

LIMITATIONS
This is an input inspector, not a painting application. It does not draw strokes,
implement gestures/palm rejection, record input to files, or measure latency.
Only current samples are read; coalesced history is not processed. Pressure,
tilt, and buttons depend on the pen and driver. Touch may be filtered by Windows.
Automated tests/builds do not verify physical hardware. Android and ARM Windows
are not validated by this package. No telemetry, accounts, or services are used.

GitHub Actions artifacts may require sign-in and expire (requested retention:
30 days, subject to repository policy). This package is not a GitHub release.
THIRD-PARTY-LICENSE.txt contains the MIT notice for windows-sys/windows-link.
The project's own distribution license has not yet been selected; dependency
notices do not grant a license to the original application source.
