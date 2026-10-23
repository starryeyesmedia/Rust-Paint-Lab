# Automated input validation

Use the pinned Rust toolchain, from the workspace root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo run --locked -p paint-input --example replay -- crates/input/tests/fixtures/mixed.replay
```

The input crate contains seven normalization unit tests plus fourteen processing
integration tests. They exercise exact output sequences/data; simultaneous touch
and pen; hover/contact/lift; varying and zero/unsupported pressure/tilt; cancellation,
capture loss, interruptions and replay EOF; reused IDs and duplicate/unexpected
sequences; stale terminal cleanup; equal/stale timestamps and 32-bit Windows
rollover; invalid input and parser/version/resource limits; and bounded state over
50,000 generated events. Replay output is compared across repeated executions.
Tests do not sleep, read wall time, use network, or require hardware. Fixture
limits are tested at the parser boundary; long-stream tests do not retain outputs.
The native diagnostic binary itself has no automated hardware test suite.

For focused development:

```sh
cargo test --locked -p paint-input --test processing
```

On Linux/macOS also check the Windows adapter without linking/executing:

```sh
rustup target add x86_64-pc-windows-gnu
cargo check --workspace --locked --target x86_64-pc-windows-gnu
cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings
```

Existing CI checks format/lint/tests/native builds on Windows, Linux, and macOS.
The Windows leg additionally builds and packages the release diagnostic; it does
not exercise a physical pen. The downloadable package includes an updated short
README, with hardware tests in [the full checklist](hardware-testing.md).
Do not infer actual Windows execution from cross-compilation, or pen behavior
from either native builds or synthetic replay. The Galaxy Book checklist is still
pending and is independent of completing the automated processing milestone.
