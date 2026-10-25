# Desktop accessibility assessment

## Current foundation

The canvas shell uses egui 0.33.3 through egui-winit 0.33.3 and winit 0.30.13.
Egui provides keyboard navigation and focused/hovered widget states, and ordinary
widgets carry their displayed labels into egui's widget-information layer. Custom
icon buttons use readable labels/tooltips. The studio now centralizes spacing,
rounding and 44-point default interaction targets, and exposes persisted interface
scaling and high-contrast controls. Theme contrast is covered by automated checks.

The accessibility bridge is **not currently enabled**. The pinned egui-winit
0.33.3 manifest provides an optional `accesskit` feature that enables egui's
AccessKit tree and `accesskit_winit::Adapter`. Its `State::init_accesskit` requires
an event-loop proxy whose user event can carry `accesskit_winit::Event`. The
application currently owns `EventLoop<()>`, including a Windows capture observer
that uses that proxy for wakeups. Enabling the feature therefore requires a shared
user-event enum and routing both wake events and AccessKit events through the one
existing event loop; adding the dependency feature alone would not connect Narrator
or NVDA.

## Not yet verified

No Narrator or NVDA session has been performed. Roles, accessible names, values,
selection state, focus announcements, and UI Automation exposure have not been
validated on Windows. Linux/macOS assistive-technology behavior is likewise
untested. Current contrast tests exercise theme colors, not every rendered control
or WCAG conformance. High contrast is a stronger application palette, not a claim
of native OS high-contrast integration.

## Next implementation and verification

1. Add a typed app user-event enum and preserve the current wake/capture path.
2. Enable egui-winit `accesskit`, initialize its adapter from the active event loop,
   and forward AccessKit user events without creating another event loop.
3. Give custom canvas, tool and reference controls explicit accessible descriptions
   and state, then inspect the generated tree with AccessKit tooling.
4. Test focus traversal and keyboard operation in Linux UI automation; manually
   test Windows Narrator and NVDA, including dialogs, brush selection, layer state,
   errors and recovery.
5. Document platform gaps rather than treating a generated tree as proof of usable
   screen-reader support.
