# Legacy compatibility fixture intake

This directory is reserved for owner-approved historical compatibility fixtures.
No binary artwork or brush preset was present in the reviewed `main` checkout, so
no synthetic bytes are labeled as authentic legacy evidence.

Before migration implementation, obtain one real `.rpl` from each native format
generation V1–V4 and one existing `.rplbrush`. Keep originals read-only, record
their SHA-256 and provenance in `manifest.csv`, and derive a separate expected
pixel/metadata manifest from an independent reference. Test only disposable
copies. Do not add private artwork or identifying user data.

`docs/bp-compatibility-regression-matrix.md` is the acceptance matrix. Fixture
intake does not authorize `.bp` storage code or tests coupled to the leased
storage API before PR #9 is accepted and the Engineering Lead releases that file.
