# Galaxy Book2 Pro 360 hardware validation

Status: **not performed**. Synthetic lifecycle/replay tests and Windows-target compilation
cannot verify a digitizer, driver, native event delivery, or display behavior.

Build/run from README on Windows 10 1703+ with MSVC. Record Windows build,
Samsung/Intel/digitizer driver versions, screen scaling/orientation, pen model,
application commit, and observations below. Do not record hardware serial numbers.

| Test | Expected evidence | Result |
| --- | --- | --- |
| Launch/close/resize/minimize/restore | Readable text, responsive window, clean close; no input API errors | Not run |
| Hover then contact/lift | Tool Pen; Hover/Down/Move/Up; contact/range change; active contact clears | Not run |
| Light to firm strokes | Supported pressure changes smoothly within 0–1; mask and raw 0–1024 agree; no mouse-derived pressure | Not run |
| Pen unsupported fields | Unavailable capability shows None, valid zero stays Some(0); raw fields interpreted only with masks | Not run |
| Tilt in both axes | Supported signed angles change with lean, upright near zero; note None if unsupported | Not run |
| Barrel button hover/contact/release | Barrel and raw pen flags update; note whether device reports hover button state | Not run |
| Inversion/eraser if pen supports it | Flags change for compatible tool; otherwise record unsupported | Not run |
| Drag out of window then lift | Captured updates continue or cancel is reported; no stuck active contact | Not run |
| Alt-Tab/window interruption during contact | Shared lifecycle reports cancellation on capture/app loss; next stroke works | Not run |
| Two fingers down/move/lift | Separate Touch IDs and contact lifecycles; no invented pen pressure | Not run |
| Pen plus palm/finger | Record actual OS-delivered tools/IDs; no promise of palm rejection | Not run |
| Rotation and 100/150/200% scaling | Physical client coordinates align with window bounds; content stays usable | Not run |
| External monitor, including negative screen coordinates | Client-origin conversion stays correct; moving between DPI settings works | Not run |
| Fast strokes and several minutes of input | Bounded recent list/contact set, responsive window; note historyCount > 1 and missed detail | Not run |
| Ordinary mouse | No fabricated pen samples/pressure; mouse fallback is not a pen test | Not run |

For each issue capture the latest display (message, masks, flags, error, and ID),
reproduction steps, and expected versus observed behavior. The raw numbers in
unsupported pen fields are not evidence of capability. Touch may be filtered by
OS palm rejection or pen settings; distinguish absence from a confirmed app bug.
The inspector reads only the current sample, not coalesced history. It does not
measure end-to-end latency, render strokes, implement gestures, calibrate pressure,
or establish Android S Pen compatibility. Android tests need a separate adapter.
