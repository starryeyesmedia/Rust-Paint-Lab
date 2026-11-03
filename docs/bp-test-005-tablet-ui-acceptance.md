# BP-TEST-005 tablet and integrated-shell UI acceptance

This is the executable acceptance contract for the next integrated UI
candidate. It requires the real `canvas-demo` application, real pointer/keyboard
interaction, and inspected screenshots. Component previews and icon catalogs do
not satisfy any case below.

## Directly accessible controls

The application must expose these 15 independently named and operable tool
buttons without opening a family/group navigator:

1. Brush
2. Eraser
3. Eyedropper
4. Smudge
5. Color Mixer
6. Fill
7. Gradient
8. QuickShape
9. Line
10. Rectangle
11. Ellipse
12. Rectangle Selection
13. Ellipse Selection
14. Lasso Selection
15. Create Path

The same visible shell must expose References and Brush Library entry points.
Tool selection is verified by the selected tool's accessible state and one
real tool-specific canvas action where the current app supports it. Capture a
real application screenshot at each required viewport; do not infer visibility
from a declarative toolbar catalog.

## Tablet history test

Start from a saved, two-layer document with a deterministic pixel digest and no
undo/redo history. Use a direct pointer tap on the canvas to confirm that it
does not begin a stroke. Then paint three visibly different, committed strokes
with different tools/settings and capture the document pixel/layer digest after
each commit (`S0` through `S3`).

| Step | Action | Required state/evidence |
| --- | --- | --- |
| Empty history | Open the document before editing | Undo and Redo are visible, named, focusable, and disabled. |
| Commit | Paint one stroke | Undo becomes enabled; Redo stays disabled; document is `S1`. |
| Direct Undo | Tap Undo once | Canvas does not receive a paint start; document is byte/pixel-state `S0`; Redo is enabled. |
| Direct Redo | Tap Redo once | Document returns exactly to `S1`; Undo is enabled; Redo is disabled. |
| Repeated cycle | Commit strokes to reach `S3`, Undo three times, Redo three times | Each intermediate digest equals its recorded `S0`/`S1`/`S2`/`S3` state. |
| Branch history | Undo once, then commit a new stroke | Redo is disabled and cannot restore the discarded branch. |
| Tool/settings/navigation | Repeat Undo/Redo with a non-default tool, brush color/size/opacity, and a panned/zoomed canvas | Selected tool, brush settings, active layer, selection, zoom, and pan remain unchanged by the history-button action except for the intended document history step. |
| Keyboard/accessibility | Tab/Shift+Tab to each control; activate with Enter and Space; invoke its AccessKit action | Accessible name, role, focus, and enabled state match the visible control; activation has the same exact document result. |

For the no-paint assertion, compare the committed document state and stroke-start
counter before/after each button activation, and verify that no preview remains.
Save/reopen after the final redo and compare the exact expected committed
document state. This UI test supplements document-engine Undo/Redo tests; it does
not replace them.

## Viewport, scaling, menu, and real-app evidence matrix

| View | Setup | Required assertions and captures |
| --- | --- | --- |
| Compact 100% | 640×480 client window, app scale 100% | Integrated application shell; canvas visible; all 15 tools reachable individually; References and Brush Library reachable; Undo/Redo visible and operable; no grouped-tool navigation; file-menu overflow remains inside the window and is keyboard reachable. Capture `bp-test-005-compact-100.png`. |
| Compact 150% | 640×480 client window, app scale 150% | Same control reachability and behavior by scrolling only where the design provides it; no clipped or overlapping Undo/Redo; canvas remains visible; keyboard focus can reach overflow items. Capture `bp-test-005-compact-150.png`. |
| Wide 100% | 1280×800 client window, app scale 100% | Integrated shell and canvas; all direct tools and entry points visible or directly reachable; no family/group navigation; file-menu overflow and keyboard paths work. Capture `bp-test-005-wide-100.png`. |
| Wide 150% | 1920×1080 client window, app scale 150% | Same as wide 100%; verify physical bounds and focus order at increased scale. Capture `bp-test-005-wide-150.png`. |

At each size, record control rectangles, accessible names/roles/enabled states,
window dimensions, effective scale, selected tool, canvas bounds, history
availability, and screenshot path. Check that every button click preserves the
document digest and creates no stroke unless it is a documented canvas action.
Use actual app-window screenshots and pixel inspection to confirm that the
canvas and controls rendered; also retain interaction logs and expected/actual
digests.

File-menu overflow checks must open the real menu at compact width, reach every
overflowed command with keyboard navigation, activate one harmless command,
close with Escape, and confirm focus returns to the shell. Test keyboard menu
opening (Alt+F or the platform's equivalent) and tab order; do not validate only
by finding menu labels in source.

## Automation handoff and current coverage gap

Run the existing owned X11/XTest application workflow after the Engineering
Lead releases its smoke-script and Studio-file lease. The workflow should launch
the built application, collect the real screenshots above, and add test markers
for direct-tool rectangles, Undo/Redo rectangles and enabled states, accessible
roles/names, focus order, canvas bounds, and document/stroke digests. Keep these
markers as observation hooks; verify the behavior with actual taps and keys.

At this checkpoint, the latest `main` still renders three direct tool controls
and grouped tool-family navigation. The 15-item catalog groundwork in PR #14 is
not the integrated toolbar. PR #9 still leases `apps/canvas-demo/src/studio.rs`,
`.github/workflows/ci.yml`, and the existing alpha smoke scripts/helpers. The
current shell has no Xvfb/XTest launcher available. Therefore no UI automation,
real screenshot, direct tablet Undo/Redo, 15-button integration, or physical
tablet acceptance is claimed by BP-TEST-005. Do not edit the leased shell,
smoke, or CI files as part of this test-only slice. Engineering Lead release of
those files and an integrated UI candidate are prerequisites to turning this
contract into the owned smoke extension.
