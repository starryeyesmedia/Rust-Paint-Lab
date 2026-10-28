$ErrorActionPreference = 'Stop'

$exe = Resolve-Path 'target/x86_64-pc-windows-msvc/release/canvas-demo.exe'
$logDirectory = Join-Path $env:RUNNER_TEMP 'rpl-windows-startup-smoke'
New-Item -ItemType Directory -Force $logDirectory | Out-Null
$stdout = Join-Path $logDirectory 'stdout.log'
$stderr = Join-Path $logDirectory 'stderr.log'
$applicationLog = Join-Path $logDirectory 'canvas-demo.log'
Remove-Item $stdout, $stderr, $applicationLog -Force -ErrorAction SilentlyContinue

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class RplWindowProbe {
    private delegate bool EnumWindowsCallback(IntPtr hwnd, IntPtr lParam);
    [StructLayout(LayoutKind.Sequential)]
    private struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumWindowsCallback callback, IntPtr lParam);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int maxCount);
    [DllImport("user32.dll")] private static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);

    public static string FindVisibleStudioWindow(int targetPid) {
        string found = null;
        EnumWindows((hwnd, _) => {
            uint ownerPid;
            GetWindowThreadProcessId(hwnd, out ownerPid);
            if (ownerPid != targetPid || !IsWindowVisible(hwnd)) return true;
            var title = new StringBuilder(512);
            GetWindowText(hwnd, title, title.Capacity);
            RECT rect;
            if (title.ToString().IndexOf("BitchPixel", StringComparison.OrdinalIgnoreCase) >= 0 &&
                GetWindowRect(hwnd, out rect) && rect.Right > rect.Left && rect.Bottom > rect.Top) {
                found = title.ToString();
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@

$previousLogDirectory = $env:RPL_LOG_DIR
$env:RPL_LOG_DIR = $logDirectory
$process = $null

try {
    $process = Start-Process -FilePath $exe.Path -ArgumentList '--alpha-smoke' -PassThru `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    $visibleTitle = $null
    $output = ''
    while ([DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 250
        $process.Refresh()
        $output = (Get-Content $stdout, $stderr, $applicationLog -Raw -ErrorAction SilentlyContinue) -join "`n"
        if (!$visibleTitle) {
            $visibleTitle = [RplWindowProbe]::FindVisibleStudioWindow($process.Id)
        }

        if ($process.HasExited) { break }
        if ($visibleTitle -and $output.Contains('WINDOW_STARTUP_SMOKE_FIRST_FRAME') -and
            $output.Contains('ALPHA_WINDOW_SMOKE: presented controls and raster document')) {
            $process.WaitForExit(10000) | Out-Null
            $process.Refresh()
            if ($process.HasExited) { break }
        }
    }
    $process.Refresh()
    if (!$process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        throw "Timed out waiting for the visible first Studio frame. Logs: $logDirectory"
    }

    $output = (Get-Content $stdout, $stderr, $applicationLog -Raw -ErrorAction SilentlyContinue) -join "`n"
    if ($output -match 'No compatible graphics adapter:') {
        Write-Host 'WINDOW_STARTUP_SMOKE_INCONCLUSIVE: this Windows runner has no compatible wgpu adapter.'
        Write-Host $output
        return 'INCONCLUSIVE'
    }
    if ($process.ExitCode -ne 0) {
        throw "Canvas startup process exited with code $($process.ExitCode). Logs: $logDirectory`n$output"
    }
    if (!$visibleTitle) {
        throw "Windows did not report a visible BitchPixel top-level window for process $($process.Id). Logs: $logDirectory`n$output"
    }
    if ($output -notmatch 'WINDOW_STARTUP_SMOKE_VISIBLE' -or
        $output -notmatch 'WINDOW_STARTUP_SMOKE_FIRST_FRAME' -or
        $output -notmatch 'ALPHA_WINDOW_SMOKE: presented controls and raster document' -or
        $output -match 'The AccessKit winit adapter must be created before the window') {
        throw "Canvas did not report a visible, successfully rendered Studio frame. Logs: $logDirectory`n$output"
    }
    Write-Host "WINDOW_STARTUP_SMOKE_PASSED: visible window '$visibleTitle' rendered the initial Studio UI."
    Write-Host $output
    return 'PASSED'
}
finally {
    if ($process) {
        $process.Refresh()
        if (!$process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    }
    if ($null -eq $previousLogDirectory) { Remove-Item Env:RPL_LOG_DIR -ErrorAction SilentlyContinue }
    else { $env:RPL_LOG_DIR = $previousLogDirectory }
}
