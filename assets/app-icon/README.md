# Artheffect app icon

**Creature:** a unicorn, as an engraved head-and-shoulders bust looking out of the tile.

**Style:** an ink engraving portrait in the Crafting Apps' owl-template framing (the FilmCraft owl):
full-bleed colour field, tight head-and-shoulders crop, the body running off the bottom and right edges.

**Tile:** 512 × 512 viewBox, rounded square `rx=112`, clipped; no border or roundel.

**Palette** (exactly three colours):

| Colour | Hex | Used for |
|---|---|---|
| App colour (Artheffect pink) | `#e0368f` | the full-bleed field |
| Paper | `#efe9dc` | the figure |
| Ink | `#0b0b0c` | line work and contour |

**Provenance:** the project owner drew it in ArtCraft (a 2880 px engraving-style image). It was
vectorised with potrace, which keys out the field and traces the ink.
Licence: see [LICENSE.txt](LICENSE.txt) (MIT OR Apache-2.0, like the repo).

## Files

| File | What |
|---|---|
| `artheffect.svg` | master vector, traced at 2048 px (canonical) |
| `artheffect-small.svg` | lighter vector (~350 KB), traced at 1024 px; also the web favicon |
| `artheffect-1024.png` | 1024 px render |
| `artheffect-macos-512.png` | runtime Dock icon on macOS, on Apple's 824/1024 grid |
| `artheffect.icns` | macOS icon (Apple grid) |
| `artheffect.ico` | Windows icon, 16–256 px; embedded in `artheffect.exe` by `apps/artheffect/build.rs` |
| `hicolor/<n>x<n>/apps/ai.storyteller.artheffect.png` | Linux icon theme, 16–512 px; the 256 px one is the runtime icon on Windows and Linux |
| `hicolor/scalable/apps/ai.storyteller.artheffect.svg` | Linux scalable icon |

App ID: `ai.storyteller.artheffect` (Wayland app ID, `packaging/linux/ai.storyteller.artheffect.desktop`).

## Regenerate

After replacing `artheffect.svg` / `artheffect-small.svg`, run `packaging/icons.sh` (needs `resvg`; `iconutil`
on macOS for the `.icns`). Copy `artheffect-small.svg` to `apps/artheffect-web/web/favicon.svg` too.
