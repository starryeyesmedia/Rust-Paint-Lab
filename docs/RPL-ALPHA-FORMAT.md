# RPL editable alpha format versions 1, 2, and 3

Extension `.rpl`, with `RPLALP01`, `RPLALP02`, and `RPLALP03` as the versioned
8-byte magic. This is an original, intentionally small alpha format, not
OpenRaster/PSD. Existing version 1 and 2 projects remain readable. The writer
retains version 1 for raster-only documents, version 2 for mask-only documents,
and uses version 3 whenever vector metadata or its monotonic ID allocator is
present. Removing the last vector object does not reset its allocator or
downgrade the project. Unknown versions/metadata fields are rejected rather than
silently losing editing information.

Binary layout (little endian integers):
1. Eight-byte magic, then u32 JSON metadata length (65,536 bytes for versions 1
   and 2; 8 MiB for version 3).
2. UTF-8 JSON State: dimensions, DPI, ordered raster layers with IDs/names/visibility/
   locking/opacity/blend, selected layer ID, background RGB or null, and last explicitly
   saved brush settings. In version 2, each masked layer has optional mask metadata
   (`enabled`, `inverted`); mask tile bytes are not embedded in JSON. A missing mask
   tile is fully revealing (alpha 255). Version 1 metadata has no mask field and
   deserializes as an unmasked layer. Version 3 adds an ordered `vector_paths`
   list to each layer and a document-level `next_vector_id` allocator.
3. u32 total tile count. Each entry: u64 layer ID, u32 tile x/y, then exactly 16,384
   bytes representing a 64×64 premultiplied encoded-sRGB RGBA8 tile.
4. Versions 2 and 3: u32 mask tile count followed by entries of u64 layer ID, u32 tile
   x/y, then exactly 4,096 8-bit mask values for the 64×64 tile. Mask samples are
   stored before inversion; an enabled normal mask multiplies premultiplied layer
   pixels by the sample, and an enabled inverted mask uses `255 - sample`. A disabled
   mask is retained but bypassed for compositing.
5. Version 3 path geometry is included in the checksummed JSON metadata. A path
   has a stable nonzero u32-range object ID, visibility, a start point, ordered
   `line_to`, `cubic_to`, or terminal `close` segments, straight-alpha RGBA stroke
   color, and document-pixel stroke width. Paths are ordered within their raster
   layer and composite above its raster pixels; layer ordering, visibility,
   opacity, blend mode, and editable mask still apply. Raster output is derived for
   display and export and never replaces the native path records. IDs are
   document-global and monotonically allocated; deleting an object does not
   permit reuse after save/reopen.
6. SHA-256 of every preceding byte (32 raw bytes). This detects corruption; it is
   not authentication or a signature.

Limits: 1–4096 pixels per dimension, 1–9600 DPI, 1–32 layers, 8192 raster+mask
tiles across the document and current provisional work (at most 128 MiB for raster
tiles, with masks sharing that tile-count budget), and 160 MiB maximum input file.
Tiles must have unique, in-range coordinates and valid premultiplied components.
Layer IDs must be unique/nonzero and selection valid. Version 3 additionally
allows at most 1,024 paths and 8,192 stored segments (1,024 per path), finite
coordinates within ±16,384 document pixels, widths from 0.25 to 512 pixels, and a
conservative estimated 32 million segment-bounds pixel checks per composite
across visible paths. Cubic segments use deterministic 16-step flattening for
rasterization and hit testing. The current editor can move path anchors without
changing the version 3 record layout; adjacent cubic handles retain their
offsets. It creates polyline paths only and does not yet expose independent
Bézier-handle editing, anchor insertion/deletion, path fills, scale/rotation, or
SVG round-trip. No decompression is used. The reader validates checksum, bounded
lengths/counts, metadata and tile data before returning a document. It does not
deserialize GPU objects or platform handles.

Saves take an OS-backed per-path `.lock`, check the expected previous file digest,
write a unique same-directory temporary with create-new, flush/sync, preserve the
previous valid native file as `.bak`, then atomically rename the new file. Unix also
syncs the parent directory. Fault injection verifies failures before replacement
leave the old valid project unchanged. Other instances/changed files produce an
error. Save As refuses to overwrite an existing unrelated file; choose a new name
or Open the existing project first. This is deliberately conservative. Filesystem/
power-loss behavior on physical Windows storage and network filesystems is not
certified. Backups and recovery are additional safeguards, not a universal durability claim.

Recovery uses separate, local, checksummed native snapshots with ownership locks.
A bounded background writer retains at most one queued and one running immutable
snapshot; a destructive action/close waits for a successful flushed checkpoint.
An active owner's checkpoints are excluded from recovery choices. Previous snapshot
backups survive replacement. The UI exposes recovery choices on next launch.

Each background checkpoint is also written to a sibling `.rpl.journal` file using
the versioned `RPLJRN01` record format: magic, monotonically increasing u64 record
sequence, u64 payload length, a complete checksummed version 1, version 2, or
version 3 `.rpl` snapshot payload,
and SHA-256 over the record header and payload. Repeated identical snapshots are
idempotent. Recovery uses the latest complete, valid record; an incomplete or
corrupt tail falls back to the previous complete record, and the existing `.rpl`
snapshot plus `.bak` remain fallback paths. The journal is limited to two maximum
project snapshots; when an append would exceed that cap it atomically compacts to
the new full-state checkpoint. Each record is validated with the native format's
existing allocation limits before use. Recovery cleanup removes the journal with
its associated snapshot after obtaining the session ownership lock.

This is a **full-state snapshot journal**, not a per-operation WAL: it does not
store individual strokes or editing commands, and it does not preserve the undo
stack. Checkpoint cadence remains controlled by the app, so a hard crash can lose
work since the most recently synced journal record. A checksum detects accidental
corruption, not malicious tampering. Checkpoint retention and obsolete-file cleanup
remain explicit/manual; no recovery file is silently deleted. Physical power-loss
behavior and filesystem durability have not been certified.

Not stored: undo stack, original raster brush dab paths, native performance
counters, layer groups, selections, ICC profiles, or timelapse. Version 3 vector
objects are editable native artwork, but are not original raster brush histories.
Brush presets have a separate versioned JSON
schema `{ "version": 1, "brush": ... }` and are validated before use; sharing UI
and migrations are unfinished.

PNG import converts the first image to 8-bit RGBA; 16-bit precision/ICC/APNG animation
are not preserved. Oversized images are rejected rather than silently cropped.
Export flattens visible content to straight-alpha RGBA8 with sRGB and DPI metadata.
It refuses to overwrite an existing export. Native projects retain editable raster
layers, masks, and version 3 vector paths. OpenRaster export applies each enabled
mask and rasterizes vector paths into layer PNGs and reports that those features
are flattened; the `.rpl` remains the editable source.
Compositing is explicitly encoded-sRGB premultiplied arithmetic, not full
linear-light or ICC color management.

PNG publication requires local hard-link support for atomic no-clobber behavior;
unsupported filesystems fail safely. Views/selections are transient; stored pixels
and layers remain editable after crop/transform.
