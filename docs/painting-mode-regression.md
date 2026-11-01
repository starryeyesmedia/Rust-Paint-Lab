# BP-TEST-001 — Painting mode differentiation

This specification makes Brush, Smudge, and Color Mixer comparable under
identical document, brush, and pointer conditions. The engine-level integration
tests live in `crates/document/tests/painting_modes.rs`; they call public
`Engine`, `State`, `StrokeMode`, and storage APIs. They do not change the painting
algorithms or modify the existing shared test file.

## Deterministic fixture

Create one 128×64 document with no paper background, then clone that exact
`State` before each mode run. Layer 1 is selected and contains:

- Opaque premultiplied red `[255, 0, 0, 255]` at x=8..23, y=24..39.
- Adjacent opaque blue `[0, 0, 255, 255]` at x=24..39, y=24..39.
- Half-alpha magenta `[80, 0, 80, 128]` at x=40..47, y=24..39.
- Transparent pixels from x=48 onward, including the stroke destination.

Layer 2 contains premultiplied green `[0, 180, 0, 180]` at x=96..111,
y=24..39. Tests verify its bytes stay unchanged when Layer 1 is selected. Fixed
Brush settings are Round, size 10, hardness 1, opacity 0.75, flow 0.5, spacing
0.2, pressure-size off, pressure-opacity on, no tilt, no smoothing, and yellow
foreground `[255, 255, 0]`.

The primary stroke uses identical samples for all modes: begin at `(12.5, 32.5)`
and commit at `(60.5, 32.5)`, with pressure `Some(1.0)`. Additional runs use
`Some(0.25)`, `Some(0.5)`, `Some(1.0)`, and unavailable pressure `None`.

Initial composite checks are `[255,0,0,255]` at `(16,32)`,
`[0,0,255,255]` at `(31,32)`, `[159,0,159,128]` at `(44,32)`, and transparent
`[0,0,0,0]` at `(72,32)`. The half-alpha composite value reflects the engine's
integer straight-RGBA conversion; its stored layer bytes remain
`[80,0,80,128]`.

## Functional assertions

### Brush

On the cloned fixture, `StrokeMode::Paint` deposits the configured yellow
foreground. Under the primary fixed settings, `(20,32)` is exactly
`[253,182,0,255]`; a full-loading dot with opacity 1 and pressure 1 is exactly
`[255,255,0,255]`. Flow 0 and opacity 0 are no-ops. Increasing loading and
pressure from 0.25 to 0.5 to 1.0 increases the foreground contribution.

### Smudge

On the same cloned fixture, Smudge starts on red and passes over blue, partial
magenta, and transparency. It redistributes sampled canvas pigment instead of
stamping the yellow foreground. Under the primary settings, `(58,32)` is
composite `[99,0,191,44]`, with selected-layer premultiplied bytes
`[17,0,33,44]`. The latter satisfies RGB ≤ alpha. Flow controls pigment pickup;
opacity and pressure control redistribution strength. On an empty selected
layer, Smudge is a no-op and does not stamp the foreground color.

### Color Mixer

On the same cloned fixture, Mixer uses yellow foreground paint and samples the
red/blue/partial-alpha canvas. With strength 0.5 and primary brush settings,
`(20,32)` is `[255,161,0,255]` and `(31,32)` is `[141,141,106,255]`. At a
single red pixel with full opacity, flow, and pressure, strengths 0.25, 0.5, and
0.75 produce `[255,112,0,255]`, `[255,192,0,255]`, and `[255,239,0,255]`.
At strength 0.5, a full-load dab on the half-alpha magenta pixel produces
`[231,191,40,255]`; a full-load dab on a transparent pixel produces the yellow
foreground. Flow 0 is a no-op, and greater loading increases the deposit.

### Differential and transaction assertions

The three full RGBA images from the primary stroke must all differ pairwise.
The selected-layer tiles must remain valid premultiplied RGBA, including across
the partial-alpha and transparent edges. A second layer must remain byte-for-byte
unchanged.

For each mode independently:

1. Begin and Update render a provisional preview that differs from the original.
2. Cancellation restores the exact original composite and document state, without
   adding history.
3. Commit writes authoritative pixels. Undo restores the exact initial state;
   Redo restores the exact committed state.
4. Saving to a temporary `.rpl` with `Store`, reopening it, and constructing a
   fresh `Engine` preserves the exact committed `State` and RGBA output.
5. A rectangular selection clips the operation to its bounds. The unselected
   second layer receives no writes.

The tests use actual RGBA bytes and sparse tile data. Transaction completion,
dirty tiles, or file-size changes alone do not satisfy these assertions.

## Remaining functional evidence

The integration file proves deterministic document-engine behavior and `.rpl`
roundtrip for synthetic samples. It does not prove the application toolbar routes
each selected tool to the expected mode on Windows, physical S Pen pressure,
hover/lift/barrel/palm behavior, color-managed display output, or artist acceptance.
Those require a separate app-route smoke and owner-run Galaxy Book2 Pro 360 session
using this fixture and exact saved project. CI cannot close the physical-device
finding.
