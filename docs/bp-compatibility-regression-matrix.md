# BP `.bp` Compatibility Regression Plan

**Status: fixture and test planning only.** No `.bp` storage implementation is
started here. The approved product decision is `.bp` for new artwork and
continued read/edit support for historical `.rpl` files and `.rplbrush` presets.
The current binary identifiers `RPLALP01`–`RPLALP04` remain in scope; `.bpbrush`
is not approved as a preset format.

## Acceptance matrix

Every passing persistence case compares a normalized document manifest and
actual raw-layer and composite RGBA samples. It records SHA-256 for all source,
backup, and journal bytes that must remain unchanged. A successful `open`, changed
file size, tile count, extension, or nonempty alpha is not sufficient.

| ID | Fixture and operation | Required pass criteria |
|---|---|---|
| BP-FMT-01 | Authentic V1 `.rpl`; open, edit, close, reopen | Verify `RPLALP01`, dimensions/DPI, two or more raster layers, layer metadata, exact opaque/partial-alpha/transparent raw and composite RGBA samples; edit remains undoable before save; reopened state equals approved expected manifest. |
| BP-FMT-02 | Authentic V2 `.rpl`; open, edit mask, save, reopen | V1 assertions plus exact mask enabled/inverted flags and mask tile samples; raster and mask data remain independently editable. |
| BP-FMT-03 | Authentic V3 `.rpl`; open, edit vector, save, reopen | V2 assertions plus vector IDs, next-ID state, geometry, stroke/style, ordering, visibility, exact rendered samples, and proof vector pixels were not rasterized into source tiles. |
| BP-FMT-04 | Authentic V4 `.rpl`; open, edit fill, save, reopen | V3 assertions plus closed geometry and fill color/rule; exact filled, edge, and outside composite pixels; source raster tiles remain unchanged. |
| BP-FMT-05 | Each `.rpl` version; save in place | The file path and `.rpl` extension remain unchanged. After edit/save/reopen, exact expected state and pixels persist. The generated `.rpl.bak` equals the prior source bytes exactly and independently reopens to the prior state. |
| BP-FMT-06 | Each `.rpl` version; Save As `.bp` | New `.bp` reopens to the same normalized artwork and edit. Original `.rpl`, preexisting `.rpl.bak`, and `.rpl.journal` bytes and hashes remain unchanged. No source file is renamed, deleted, or redirected. |
| BP-FMT-07 | New artwork; create, save `.bp`, close, reopen, edit, save again | Default new-project path/extension is `.bp`; the exact document manifest and RGBA samples survive two close/reopen cycles. Later saves remain at `.bp`; no unexpected `.rpl` sibling or migration copy appears. |
| BP-FMT-08 | Legacy and new files; backup/journal/recovery failure paths | Checkpoint each project, verify snapshot and complete journal restore exact state, truncate/corrupt a journal tail and recover the previous complete record, then disable/remove the journal, corrupt the primary snapshot, and recover only from the verified `.bak`. Assert source artwork and recovery bytes are unchanged by restore. Exercise both `.rpl` and `.bp` path derivation. |
| BP-FMT-09 | Seed old and new application-data roots together | Use disposable roots for legacy `%LOCALAPPDATA%/RustPaintLab` and approved BitchPixel data paths plus their XDG/macOS equivalents. Discover old drafts, recovery snapshots, recent paths and presets without moving or rewriting them; direct all new writes to the approved root; test same-name collisions and document deterministic display precedence. |
| BP-FMT-10 | Existing `.rplbrush`; open, select, save-copy, reopen | Historical preset bytes remain unchanged; the preset loads with exact brush kind, size, opacity, flow, hardness, spacing, color, pressure flags and tilt settings; save-copy and reload preserve supported settings. Do not require or write `.bpbrush`. |

For Save As, hash the original file, backup, and journal before and after the
operation. For recovery tests, state explicitly which source won (latest journal,
previous complete journal record, primary snapshot, or verified backup). Verify
that restoring does not modify the source project. Run corrupt-tail and failed
restore cases on disposable copies.

## Fixture status and provenance

The current repository history and tracked tree contain no authentic artist
`.rpl` files, no owner-provided `.rplbrush`, and no committed historical backup,
journal, or recovery snapshot artifacts. Existing tests generate temporary files
at runtime.

The fixture intake directory contains synthetic files produced by the historical
source serializers. They are useful parser and save-compatibility regressions but
are **not authentic artist artwork or evidence of an actual historical desktop
save**. Exact commit, serializer function, SHA-256, dimensions, layer count, and
expected reopen assertions are in
[`manifest.csv`](../crates/document/tests/fixtures/bp_compat/manifest.csv). The
reproducer is
[`generate_historical.py`](../crates/document/tests/fixtures/bp_compat/generate_historical.py):

| Version/artifact | Serializer source commit | Serializer | Feature coverage |
|---|---|---|---|
| Synthetic V1 project | `d322c55619b50278e3a2ced50cd9fc27cbe1ff49` | `paint_document::storage::encode` | Layered raster, opaque red/blue, partial alpha, transparency. |
| Synthetic V2 project | `4b4e6c4bacc813148d1b35cccb64ae888f7cfe88` | `paint_document::storage::encode` | V1 raster plus editable mask bytes. |
| Synthetic V3 project | `4a5a808bc1f20bd7432a22d3f2cac17885888d89` | `paint_document::storage::encode` | V2 coverage plus editable open vector geometry. |
| Synthetic V4 project | `0d26bd60b14fd42ac11a6d77dadcbcb97b2474e0` | `paint_document::storage::encode` | V3 coverage plus closed vector fill. |
| Synthetic brush preset v1 | `173bf1c92113f6ecc90f3246dfc24bb2cc8b7223` | `paint_document::brush::Brush::encode` | Validated version-1 `.rplbrush` JSON. |

The added integration tests decode these files, assert manifest raster and feature
bytes, exercise in-place `.rpl` saves, check that `.bak` preserves the exact old
bytes, and test backup fallback plus current journal/recovery handling. Backup,
journal, and recovery artifacts are generated in temporary directories by those
tests; they are not labeled as historic. The tests do not implement `.bp` Save As
or `.bp` persistence.

Authentic historical samples are still required before final compatibility
acceptance. Obtain owner-approved project and preset bytes from actual historical
application builds or safe legacy-profile copies. Record source application/build,
creation process and provenance; preserve read-only originals; independently
derive exact pixel and metadata manifests; and test only disposable copies. Do not
add private artwork or identifying user data.

## Migration gate and ownership

At the latest checked repository state, `main` is
`1559122943e8b6e9123b653a555f72e7a894d3e6`. PR #9 remains a draft and owns the
storage, close-save, CI, and smoke paths it changed. Do not start `.bp` backend
implementation or storage-coupled tests until PR #9 is accepted and merged and
the Engineering Lead explicitly releases the storage lease. File-dialog and
application-data-root changes also require an explicit UI/backend handoff.
