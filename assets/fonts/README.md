# Bundled interface fonts

BitchPixel bundles all interface fonts locally; normal UI rendering works
offline and does not depend on system font installation.

## Typeface roles

- **Outfit Bold 700, ExtraBold 800 and Black 900** are the display family for
  the temporary wordmark, Home hero, page/section headings, and compact panel or
  card titles. Explicit static faces give egui the requested real weights without
  synthesizing bold text.
- **Manrope Regular 400, SemiBold 600, and Bold 700** cover body copy, actions,
  labels, and interface hierarchy.
- **Manrope SemiBold Tabular** is a local metrics variant used only by numeric
  controls. It keeps Manrope's original glyph outlines while assigning all ten
  default digit glyphs the same advance width (1,284 font units) and centering
  each outline in that cell. This gives changing measurements stable alignment
  in egui, which does not currently expose OpenType `tnum` feature selection.

## Source, generation, and licenses

Outfit and Manrope source TTFs and both SIL Open Font License 1.1 texts are from
Google Fonts revision
[`e7db6ca603fd4862010383ff17ffd6cc4ffac1c0`](https://github.com/google/fonts/tree/e7db6ca603fd4862010383ff17ffd6cc4ffac1c0).
Outfit's variable `wght` axis was instantiated at 700, 800, and 900 with
fontTools 4.61.1. Manrope's variable source was instantiated at 400, 600, and
700; the tabular variant was derived from the 600 instance as described above.
The saved font metadata identifies each static weight. Source files are not
required at runtime. Both complete license notices are retained beside the
assets and included in Windows, Linux, and macOS package notices.

SHA-256 checksums of the shipped TTF files:

| File | SHA-256 |
| --- | --- |
| `Outfit-Bold.ttf` | `a0bc3263ec47ce7b5c287fc5cab3cde669d932ede16a1e08e1e72acf0afdbb30` |
| `Outfit-ExtraBold.ttf` | `f7c89f13586a68d9e44c454bf5995e811c5766ad19455f98bb0888cacfcac8a0` |
| `Outfit-Black.ttf` | `6faf338b48c392447ddafb3e87ee3e4a2a3b167c1a21ab59f606b7099a2b761a` |
| `Manrope-Regular.ttf` | `e82a2c322c2a4f89bc81de57fe6e41a4ea1c062746d86b40d021591c1344931d` |
| `Manrope-SemiBold.ttf` | `23af9c0e39d3bc465af72267000d16a6cf828e32bd53db443001d3ce0f203891` |
| `Manrope-SemiBold-Tabular.ttf` | `5a6684eaff937f5d433ffce944592634b7125a1430598f435c256d5be76b4a71` |
| `Manrope-Bold.ttf` | `8a0a4b5cfa880b8bb8dac3d727f5f632be5745ffdf42fc54d72c465f3c4969ba` |

Outfit has a compact Latin character set; unsupported scripts and symbols use
the remaining egui proportional fallback fonts, with Manrope first in the
fallback chain. Icons remain vector geometry and do not depend on font glyphs.
The test suite shapes Latin, accented, punctuation, multiplication and CJK text
through the role system to exercise fallback. The earlier Space Grotesk display
faces have been removed from the application and package notices.
