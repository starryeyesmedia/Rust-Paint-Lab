# Contributing to BitchPixel

Thank you for helping make creative software accessible and free.

BitchPixel is a Rust-based, offline-first digital painting application. Please read [AGENTS.md](AGENTS.md), [the roadmap](docs/ROADMAP.md), and [the testing guide](docs/testing.md) before proposing substantial changes.

## Proposing changes

- Open an issue first for significant design, feature, format, or architecture changes.
- Keep pull requests focused, explain the problem and approach, and include tests for affected behavior.
- Do not merge incomplete features, break old `.rpl` artwork, or weaken accessibility and recovery guarantees.
- Do not introduce telemetry, mandatory accounts, paid feature gates, or unlicensed third-party content.
- For UI changes, include screenshots where feasible and preserve keyboard, touch, high-contrast, and screen-reader behavior.
- Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, and `cargo build --workspace --locked`. Document platform-specific limitations honestly.

## Licensing contributions

BitchPixel's original project code is licensed under the GNU General Public License version 3.0 only (GPL-3.0-only). Copyright © 2026 Cecelia Thompson for original material she owns. See [LICENSE](LICENSE) and [COPYRIGHT.md](COPYRIGHT.md). Independent contributors retain copyright in their original contributions unless expressly assigned.

By submitting a contribution for inclusion, you represent that you have the right to submit it and agree that it can be distributed under GPL-3.0-only. Retain existing copyright and attribution notices, and identify any third-party code or assets and their licenses in the pull request. Do not submit proprietary code copied from another project or material that has incompatible licensing.

No copyright assignment or contributor license agreement is required by this document. Contributors retain copyright in their original contributions.

## Brand and assets

The GPL code license is not a trademark license or permission to imply an official endorsement. BitchPixel names, logos, and other branding may be subject to separate rights. Third-party components and bundled fonts retain their respective licenses; see [THIRD-PARTY.md](THIRD-PARTY.md).

## Security

Avoid posting secrets, private test documents, or sensitive crash logs in public issues. Report potentially exploitable vulnerabilities to the maintainers privately rather than publishing working exploit details.
