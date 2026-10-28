# Third-party licenses and notices

BitchPixel's project-level GPL-3.0-only license applies to original project code; it does **not** replace independent licenses of bundled fonts, dependencies, notices, or other third-party content.

- [Dependencies and licenses](docs/dependencies.md) records direct packages and distribution-notice procedures.
- [Locked dependency license inventory](docs/dependency-licenses.md) lists license expressions from the Cargo.lock dependency graph.
- [Bundled fonts](assets/fonts/README.md) documents source revisions, derived faces, checksums, and the SIL Open Font License 1.1. The complete font license texts are preserved at `assets/fonts/Outfit-OFL.txt` and `assets/fonts/Manrope-OFL.txt`.
- [Supplemental dependency license notices](docs/licenses/) contain additional upstream notices.
- The packaging script `scripts/canvas-licenses.py` generates `THIRD-PARTY-NOTICES.txt` for downloadable builds. Keep the generated notices with each redistributed binary package.

Before accepting a new dependency or third-party asset, verify that its terms are compatible with the intended distribution and retain all required copyright, attribution, source, and license notices. Avoid copying third-party code or artwork without permission.

**Review required:** Verify project copyright ownership, contributor permissions, and compatibility of all distributed content before merging the proposed project-level license.
