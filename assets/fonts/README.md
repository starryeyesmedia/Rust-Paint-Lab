# Bundled interface fonts

BitchPixel embeds two complementary Google Fonts families for offline,
consistent rendering. The application does not need a system font installation
or network access to display its interface.

- **Space Grotesk SemiBold and Bold** — brand, page titles and editorial
  headings. Its distinctive geometric forms give the product a recognizable
  voice while keeping short interface headings clear.
- **Manrope Regular, SemiBold and Bold** — body text, actions, labels, layer
  names, numeric controls and supporting information. The family has clear
  shapes at small sizes and the bundled weights keep hierarchy legible without
  relying on synthetic bolding.

The prior packaged Manrope file was generated at the variable font's 200-weight
default and the prior Syne file retained Regular naming. They are replaced here
with correctly instantiated static weights. This corrects the unusually light
body rendering and gives egui explicit weight families.

All five static files were instantiated from the variable font sources at
Google Fonts commit
[`e7db6ca603fd4862010383ff17ffd6cc4ffac1c0`](https://github.com/google/fonts/tree/e7db6ca603fd4862010383ff17ffd6cc4ffac1c0)
using fontTools 4.61.1. The source variable TTFs are Manrope
`ofl/manrope/Manrope[wght].ttf` and Space Grotesk
`ofl/spacegrotesk/SpaceGrotesk[wght].ttf`. Static weights were instantiated
from the `wght` axis at 400, 600, and 700 for Manrope, and 600 and 700 for
Space Grotesk. Their font metadata records the matching family and weight.

SHA-256 checksums of the bundled static files:

| File | SHA-256 |
| --- | --- |
| `Manrope-Regular.ttf` | `e82a2c322c2a4f89bc81de57fe6e41a4ea1c062746d86b40d021591c1344931d` |
| `Manrope-SemiBold.ttf` | `23af9c0e39d3bc465af72267000d16a6cf828e32bd53db443001d3ce0f203891` |
| `Manrope-Bold.ttf` | `8a0a4b5cfa880b8bb8dac3d727f5f632be5745ffdf42fc54d72c465f3c4969ba` |
| `SpaceGrotesk-SemiBold.ttf` | `85a3df570a9e4b4c2d9919ba94babd5034aa1d90e43ea57a6ea68359335210af` |
| `SpaceGrotesk-Bold.ttf` | `7972db8c4af0eafcdd3c85f5bb2a35eafdb7b29b380c19a53f479b8ea3efc62d` |

Both families are distributed under the SIL Open Font License 1.1. The complete
license texts are retained here as `Manrope-OFL.txt` and
`SpaceGrotesk-OFL.txt`; desktop packages include both notices. The egui custom
families retain the built-in proportional font list after the bundled fonts so
unsupported scripts and symbols have fallback coverage. Icons continue to use
the application's vector geometry and do not depend on font glyphs.
