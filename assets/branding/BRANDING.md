# SubTake — Porcelain (A1.3)

Selected by the user on 15 September 2026, then approved for implementation.
Keep the warm ivory tile, charcoal open frame brackets, tall coral playhead and
rounded-square handle. Preserve this geometry when changing icon renditions.

- `app-icon.png`: canonical full-colour master, extracted using the built-in
  image-generation tool from the selected A1.3 concept.
- `SubTake.icns`: macOS app rendition; regenerate with `python3 scripts/build-icons.py`.
- `menu-bar.svg`: optically drawn 18pt outline companion.
- `../../native/BrandMark.swift`: matching AppKit vector drawing, rendered as a
  native template image so macOS supplies the light/dark foreground colour.

Both packaged and development builds use this identity. Runtime app artwork is
embedded in Rust, so running with Cargo also uses the Porcelain Dock icon.

## Extraction prompt

Extract and faithfully reproduce ONLY the full-colour A1.3 / Porcelain app icon
from the lower left of the refinement board as a production macOS app icon master.
Preserve the warm ivory rounded-square porcelain tile, charcoal opposing open
rounded rectangular frame brackets, tall coral vertical playhead through the
central gap with rounded-square coral handle above, and restrained shallow bevels.
One centered icon with genuine transparent alpha outside the tile; no board,
labels, glyph studies, background or other icons. Preserve selected proportions.

Reference: `../../design/icon-explorations/2026-09-15/04-a1-refinements.png`.
