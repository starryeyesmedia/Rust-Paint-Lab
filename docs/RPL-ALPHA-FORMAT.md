# RPL editable alpha format v1

Extension `.rpl`, magic `RPLALP01` (8 bytes). This is an original, intentionally
small alpha format, not OpenRaster/PSD. Unknown versions or unknown metadata fields
are rejected rather than silently losing unsupported editing information.

Binary layout (little endian integers):
1. Eight-byte magic, then u32 JSON metadata length (maximum 65,536 bytes).
2. UTF-8 JSON State: dimensions, DPI, ordered raster layers with IDs/names/visibility/
   locking/opacity/blend, selected layer ID, background RGB or null, and last explicitly
   saved brush settings. Layer tiles are not embedded in JSON.
3. u32 total tile count. Each entry: u64 layer ID, u32 tile x/y, then exactly 16,384
   bytes representing a 64×64 premultiplied encoded-sRGB RGBA8 tile.
4. SHA-256 of every preceding byte (32 raw bytes). This detects corruption; it is
   not authentication or a signature.

Limits: 1–4096 pixels per dimension, 1–9600 DPI, 1–32 layers, 8192 tiles (128 MiB)
across the document and current provisional work, and 160 MiB maximum input file.
Tiles must have unique, in-range coordinates and valid premultiplied components.
Layer IDs must be unique/nonzero and selection valid. No decompression is used.
The reader validates checksum, bounded lengths/counts, metadata and tile data before
returning a document. It does not deserialize GPU objects or platform handles.

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
sequence, u64 payload length, a complete checksummed v1 `.rpl` snapshot payload,
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

Not stored: undo stack, original stroke paths, native performance counters, vector
objects, layer groups/masks, selections, ICC profiles, or timelapse. These features
are not advertised as supported. Brush presets have a separate versioned JSON
schema `{ "version": 1, "brush": ... }` and are validated before use; sharing UI
and migrations are unfinished.

PNG import converts the first image to 8-bit RGBA; 16-bit precision/ICC/APNG animation
are not preserved. Oversized images are rejected rather than silently cropped.
Export flattens visible content to straight-alpha RGBA8 with sRGB and DPI metadata.
It refuses to overwrite an existing export. Native projects retain editable raster
layers. Compositing is explicitly encoded-sRGB premultiplied arithmetic, not full
linear-light or ICC color management.

PNG publication requires local hard-link support for atomic no-clobber behavior;
unsupported filesystems fail safely. Views/selections are transient; stored pixels
and layers remain editable after crop/transform.
