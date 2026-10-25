# Bundled interface fonts

BitchPixel bundles the following Google Fonts families for offline, consistent
rendering. These files are presentation assets only; no font dependency was added
to Cargo.

- **Manrope Regular** — interface controls and body copy. Derived from the
  Manrope variable font at `wght=400`. Source: [Google Fonts `ofl/manrope`](https://github.com/google/fonts/tree/e7db6ca603fd4862010383ff17ffd6cc4ffac1c0/ofl/manrope).
  Bundled static-font SHA-256:
  `1e1f9be7585101a0e0d3e7463761ad3c64d7d7e3973cccbfcd5f49f84629c4c6`.
- **Syne Bold** — display headings and wordmark. Derived from the Syne variable
  font at `wght=700`. Source: [Google Fonts `ofl/syne`](https://github.com/google/fonts/tree/e7db6ca603fd4862010383ff17ffd6cc4ffac1c0/ofl/syne).
  Bundled static-font SHA-256:
  `b706f4697e71e08ce90ea78936249614bf40fad449d48fa4ec44fc40c125ae33`.

Both families are distributed under the SIL Open Font License 1.1. The exact
license texts are retained beside each font as `Manrope-OFL.txt` and
`Syne-OFL.txt`. They may be bundled with the application under the terms in those
files. The source snapshot used was Google Fonts commit
[`e7db6ca603fd4862010383ff17ffd6cc4ffac1c0`](https://github.com/google/fonts/tree/e7db6ca603fd4862010383ff17ffd6cc4ffac1c0).
Fonts are installed locally at startup and are never downloaded at runtime.
