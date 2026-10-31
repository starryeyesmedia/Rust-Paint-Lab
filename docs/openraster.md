# OpenRaster interoperability (restricted alpha subset)

Use **Export ORA** / Ctrl+Shift+E and **Open ORA** / Ctrl+Shift+O. Opening imports a
new unsaved native document; Save As `.rpl` afterward. This is layered raster
interchange, not a native project replacement or a claim of complete OpenRaster
support. Original native artwork stays untouched on export/import failure.

Preserved: dimensions, layer names/order, straight-alpha PNG pixels, visibility,
opacity, Normal (`svg:src-over`), Multiply, Screen and Overlay. Native background
becomes an explicit editable bottom layer. Equal X/Y DPI is supported (fractional
DPI rounds to integer). Export includes first/stored `mimetype`, `stack.xml`,
individual RGBA8 layer PNGs, and a flattened `mergedimage.png` preview. Version
3 native vector paths are rasterized into their owning layer's PNG; layer masks
are applied to exported alpha. The application reports both losses, while the
`.rpl` source retains editable paths and masks.

Not preserved: native brush settings, undo history, layer locks, selections and
view state. These losses are stated in the UI. Groups, masks, vector layers,
nonzero layer offsets, unequal resolution and unsupported blend modes are refused
on import; flattening them silently would lose editing structure. ICC/16-bit/APNG
features follow the same documented 8-bit sRGB PNG limitations. No thumbnail yet.
External Krita/MyPaint round trips still need manual verification; internal round
trips and an independent Python ZIP reader are automated, not interoperability
certification across every application/version.

Safety: 256 MiB ZIP input, at most 40 entries/64 KiB central directory, 64 KiB XML,
80 MiB per decompressed PNG and all native dimension/layer/tile limits. A bounded
preflight validates the ZIP directory before the ZIP library allocates its entry
table. ZIP64, encryption, multidisk, duplicate/unsafe names, symlinks and excessive
entries are rejected. No entries are extracted to filesystem paths. DTD/entity
loading is disabled. CRC checks, PNG decoder limits and pixel validation apply.
Exports publish without overwriting an existing target; hard-link support required.

Dependencies: maintained zip 9.0.5 (MIT, Rust 1.88) with only deflate-flate2 feature,
and roxmltree 0.21.1 (MIT OR Apache-2.0), compatible with pinned Rust 1.90. Encryption,
extra archive codecs, filesystem extraction and external XML resolution are unused.
PNG was already required by the painting alpha. Lockfile/inventory/notices updated.
