# Desktop accessibility assessment

## Current foundation

The canvas shell uses egui/egui-winit 0.33.3 and winit 0.30.13. The optional
egui-winit AccessKit feature is enabled. `State::init_accesskit` initializes its
adapter from the existing event loop. A typed application event routes AccessKit
requests and the Windows capture observer's wake event through the same winit
loop; this adds neither a competing event pump nor a replacement for native
Windows pointer capture. A headless egui integration test enables tree generation
and verifies the Tools toolbar role, Paint Effects radio-group role, Smudge and
Color mixer radio-button names, and minimum 44-point targets. The broader suite
also checks representative document controls. The tree is generated and action
requests are forwarded to egui. Tool popups restore focus to their trigger when
closed, and Escape remains a dismissal path.

The studio also centralizes spacing, rounding and 44-point default interaction
targets, and exposes persisted interface scaling and high-contrast controls.
Theme contrast is covered by automated checks.

## Not yet verified

No Narrator or NVDA session has been performed. Roles, accessible names, values,
selection state, focus announcements, and Windows UI Automation exposure have not
been validated on Windows. Linux/macOS assistive-technology behavior is likewise
untested. The generated-tree test checks a small representative sample, not the
entire UI. Current contrast tests exercise theme colors, not every rendered
control or WCAG conformance. High contrast is an application palette, not a claim
of native OS high-contrast integration.

S Pen drawing has been observed on the owner's Galaxy Book in previous builds.
This change preserves the existing native capture wake path. Advanced input,
focus, and screen-reader behavior have not been tested together on hardware.

## Next implementation and verification

1. Expand generated-tree inspection to dialogs, layer/selection state and
   reference controls; add keyboard alternatives where interaction is currently
   pointer-first.
2. Test focus traversal and keyboard operation in Linux UI automation; manually
   test Windows Narrator and NVDA, including dialogs, brush selection, layer state,
   errors and recovery.
3. Record platform gaps rather than treating tree generation as proof of usable
   screen-reader support.
