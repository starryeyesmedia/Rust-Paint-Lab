# Input replay fixture format, version 1

Purpose: deterministic, synthetic input for tests and development. This is not a
document format or an automatic input recorder. No live input is collected or
written to disk. The library accepts text in memory; an explicit CLI example reads
a file chosen by the developer. There are no new dependencies: a small strict
line-based grammar avoids bringing JSON/serde into the diagnostic just for fixtures.

## Grammar

UTF-8 text. First nonblank, noncomment line is exactly:

```
paint-input-replay 1
```

Blank lines and full lines starting with `#` (after trimming) are ignored.
Subsequent records use whitespace-separated fields; names are case sensitive.
No inline comments, quoting, trailing fields, or implicit defaults are accepted.
Unknown versions, records, malformed numbers, unsupported button bits, NaN,
infinities, out-of-range attributes, and inconsistent contact flags are errors
with a 1-based line number (0 for whole-file errors).

```
sample TIME ID TOOL PHASE X Y PRESSURE TILT_X TILT_Y BUTTONS CONTACT RANGE
capture_lost TIME ID
interrupt TIME
```

| Field | Meaning |
| --- | --- |
| TIME | u64 adapter-relative milliseconds, already unwrapped; not wall time |
| ID | u64 pointer identity, scoped to a single adapter/processor |
| TOOL | `pen`, `touch`, `mouse`, or `unknown` |
| PHASE | `hover`, `down`, `move`, `up`, or `cancel` |
| X, Y | finite f64 physical client pixels; negative/outside-client positions allowed |
| PRESSURE | f32 in [0,1], or `-` for unavailable |
| TILT_X, TILT_Y | independent f32 degrees in [-90,90], or `-` for unavailable |
| BUTTONS | decimal bitset: primary=1, secondary=2, barrel=4, eraser=8, inverted=16 |
| CONTACT, RANGE | exactly `0` or `1` |

Down/move require contact=1 and range=1. Up/cancel require contact=0. Hover
requires contact=0, range=1, and tool pen or mouse. Unsupported pressure/tilt use
`-`; supported zero uses `0`. Primary contact and barrel buttons are independent
attributes, not a mechanism for synthesizing pressure. Fixtures may contain
out-of-order timestamps or duplicate events to test repair behavior, but must
contain valid typed samples; direct Processor tests exercise malformed samples.

Example (simultaneous pen and touch, interrupted pen, released touch):

```
paint-input-replay 1
sample 0 7 pen hover 10 20 - - - 0 0 1
sample 1 7 pen down 10 20 0 - - 1 1 1
sample 1 9 touch down 50 50 - - - 1 1 1
sample 2 7 pen move 11 21 0.5 0 - 5 1 1
capture_lost 3 7
sample 4 9 touch up 50 50 - - - 0 0 0
```

## Execution and limits

`Replay::run()` creates a fresh Processor, processes records in file order,
then cancels all remaining contacts with EndOfReplay. Equal times retain file
order; there is no sorting, sleeping, randomness, or wall-clock dependency.
Identical typed inputs produce identical ordered Outputs. Capture loss targets
one ID; interruption clears every active ID in ascending ID order. Unsupported
versus zero attributes remain distinct all the way into outputs.

The reader caps text at 8 MiB and 100,000 records, with bounded temporary field
allocation. `Replay` owns its records and `run` collects its output; both use
memory proportional to fixture size. Processor itself retains at most 64 active
samples and no event/ID history. Production consumers should process each small
output batch and release it rather than collecting an infinite stream.

The fixtures in `crates/input/tests/fixtures` are handwritten synthetic scenarios:
`pen.replay`, `mixed.replay`, and `interrupted.replay`. They are checked with
expected event types, values, cancellation reasons, cleanup, and repeated runs.
Windows timestamp rollover is tested directly using MillisecondClock32; fixture
timestamps are portable unwrapped values, not raw platform counters.

From the repository root:

```sh
cargo test --locked -p paint-input
cargo run --locked -p paint-input --example replay -- crates/input/tests/fixtures/pen.replay
```

The example prints processed outputs to the terminal and exits on parsing errors.
It reads only the explicitly named file and does not save anything. A future
version must get a new header and documented migration rather than silently
reinterpreting v1 fields. There is no writer or realtime playback scheduler yet.

## Stroke-command integration

The separate `paint-stroke` crate routes these same v1 lifecycle outputs without
changing the fixture grammar. Touch remains available to future navigation code
but emits no painting commands. Capture loss, interruption, and unfinished EOF
strokes produce CancelStroke rather than CommitStroke. `paint-stroke` tests use
all three existing fixtures plus `crates/stroke/tests/fixtures/mouse.replay`.
See [routing rules](stroke-routing.md) for exact transaction semantics. The
`stroke_commands` example routes the existing pen fixture to stdout explicitly;
it does not record input or create a painting window.
