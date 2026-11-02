# Legacy compatibility fixture intake

## Evidence classification

The repository history, all local Git refs, and tracked files were searched on
2026-11-02. They contain no captured `.rpl`, `.rplbrush`, `.rpl.bak`,
`.rpl.journal`, or recovery snapshot files. Existing tests construct temporary
documents, presets, backups, and journal entries during test execution. Those
tests are not committed historical artwork.

This directory now includes small **synthetic project states serialized by the
historical source commits**. The binaries are genuine outputs of those commits'
serializers and are useful for decoder and save-compatibility tests. They are not
artist-created documents, files recovered from a user profile, or proof that a
released desktop binary emitted the same bytes. They must never be labeled as
authentic historical artwork.

The controlled raster pattern has two layers, opaque red and blue blocks,
premultiplied half-alpha magenta, transparent pixels, and a partial-alpha green
patch on the second layer. Versions add only features that their source supports:
V2 adds a mask, V3 adds an editable open vector, and V4 adds a closed filled
vector. The companion `.rplbrush` is also synthetic and uses the original
version-1 brush serializer.

`manifest.csv` records filename, extension, version/magic, SHA-256, byte length,
dimensions, DPI, layer count, feature summary, exact serializer commit and
function, expected reopen behavior, provenance class, and acquisition status.
The checked-in expected pixel and metadata assertions are in
`crates/document/tests/legacy_compatibility_fixtures.rs`.

## Reproducing the historical-serializer samples

Use the pinned Rust 1.90 toolchain and a complete repository history. The script
archives each listed commit without changing the current branch, inserts its
version-specific generator into that archived source tree, and runs the
historical `paint-document` serializer with that commit's lockfile:

```sh
python3 crates/document/tests/fixtures/bp_compat/generate_historical.py
```

The four artwork serializers are `paint_document::storage::encode` at the exact
source commits listed in the manifest. The preset serializer is
`paint_document::brush::Brush::encode` at `173bf1c92113f6ecc90f3246dfc24bb2cc8b7223`.
The generated hashes must match the manifest and the integration-test constants.
The script requires the historical source commits to be present locally and
their locked dependencies to be available to Cargo; it uses Cargo offline mode.

Backups, journal records, and recovery snapshots are **not** checked-in as
historical artifacts because none were found. The integration tests derive these
at runtime from the historical-serializer project samples using the current
`Store`, `Journal`, and `Recovery` APIs. The backup fallback test checks that the
`.bak` contains the exact original fixture bytes; the journal/recovery checks
verify state and pixels. The derived artifacts are tests of present-day recovery
behavior, not historical recovery samples.

## Acquiring authentic legacy material

Before treating the synthetic files as sufficient migration acceptance evidence,
obtain owner-approved files created by actual historical application builds or
approved copies from real legacy profiles:

1. Request one source `.rpl` per V1–V4 generation and one existing `.rplbrush`
   preset. Prefer known synthetic artwork saved by the historical desktop build;
   if real user artwork is supplied, confirm it contains no private or identifying
   content before checking it in.
2. Record the application version, source/build commit when available, operating
   system, serializer/save path, original filename, and provider-approved
   provenance. Keep source copies read-only and calculate SHA-256 before and after
   tests.
3. Independently record expected dimensions, DPI, layer order and properties,
   raw selected-layer pixels, composite RGBA samples, masks, vector geometry and
   fills where available. Derive the manifest from a historical app/reference or
   independently reviewed decoder, never from the `.bp` migration code under
   test.
4. Obtain backup, journal, and recovery examples from approved disposable test
   profiles. Preserve their source snapshot and record the exact operation that
   created each artifact. Do not describe a runtime copy made by the current test
   as a historical `.bak` or journal.
5. Use disposable test copies for edits, in-place saves, Save As, recovery, and
   directory-migration tests. Verify source file hashes are unchanged afterward.

Do not fabricate provenance to fill a `NOT_ACQUIRED` status row. `.bp` backend work
remains gated on PR #9 acceptance/merge and an Engineering Lead release of its
storage-file lease.
