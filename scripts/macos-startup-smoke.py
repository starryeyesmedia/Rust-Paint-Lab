#!/usr/bin/env python3
"""Launch the packaged macOS Studio and require its first rendered frame."""
from pathlib import Path
import os
import subprocess
import sys


binary = Path("target/release/canvas-demo")
if not binary.is_file():
    raise SystemExit(f"Missing packaged canvas executable: {binary}")

environment = os.environ.copy()
environment["RPL_STARTUP_SMOKE"] = "1"
environment["WGPU_BACKEND"] = "metal"
try:
    result = subprocess.run(
        [str(binary), "--alpha-smoke"],
        capture_output=True,
        text=True,
        timeout=90,
        env=environment,
        check=False,
    )
except subprocess.TimeoutExpired as error:
    output = "\n".join(
        part.decode(errors="replace") if isinstance(part, bytes) else part or ""
        for part in (error.stdout, error.stderr)
    )
    print(output, file=sys.stderr)
    raise SystemExit("Timed out before the macOS Studio's initial frame completed")

output = result.stdout + result.stderr
print(output, end="")
if "No compatible graphics adapter:" in output:
    print("::warning title=macOS GUI startup smoke inconclusive::This runner has no compatible wgpu adapter.")
    with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as summary:
        summary.write(
            "macOS GUI startup smoke was inconclusive because this runner exposed no compatible wgpu adapter.\n"
        )
    raise SystemExit(0)
if result.returncode:
    raise SystemExit(f"macOS canvas startup exited with code {result.returncode}")

required = (
    "WINDOW_STARTUP_SMOKE_VISIBLE",
    "WINDOW_STARTUP_SMOKE_FIRST_FRAME",
    "ALPHA_WINDOW_SMOKE: presented controls and raster document",
)
missing = [marker for marker in required if marker not in output]
if missing or "The AccessKit winit adapter must be created before the window" in output:
    raise SystemExit(f"macOS startup missed expected UI markers: {', '.join(missing) or 'AccessKit panic'}")

print("WINDOW_STARTUP_SMOKE_PASSED: macOS presented the initial Studio UI and exited cleanly.")
