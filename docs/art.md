# Art direction

Status: Decided (D14), 2026-10-12, to be tried. The style is **direction D, graphic layers**: bold simple shapes, big flat areas of colour, crisp shadow shapes and strong silhouettes, with atmospheric depth. One rule has held from the start: it must not look chunky or Minecraft-like.

- **Making anything the game draws** follows [art-style.md](art-style.md), the style guide. It is the same for every realm and setting.
- **This page** is the world-level look: what a realm brings, how the renderer makes the look, and what is open.
- **How we got here** is research: Forever Ago, the four mood boards, the painted four-world board and our early scenes are in [research/art-explorations.md](../research/art-explorations.md). They are references, not targets.

## Why D

- **It survives many realms.** A model carries shapes and palette roles; a realm brings its palette and its light. The bake-off showed the same files recoloured into starter, cold, gothic, ashen and crystal realms, and all of them convincing (D17, D59, D86).
- **It is mostly a shader.** Our own models, rendered with banded light and a cool shadow, read like the D concepts. The look lives in code we control, not in each model.
- **It forgives generated models.** Big flat colour areas hide the small inconsistencies that generated models carry (D1). Painted textures (C) and fine facets (A) both show them.
- **It is ambitious.** The developers found it "pretty dang attractive" and said it "feels more ambitious" (2026-10-11).

## What a realm brings

A realm is data the renderer and the palette read, never different code or different models (D58). This holds for any setting: medieval, cyber, tech or alien.

| A realm sets | For example |
|---|---|
| Its palette: a colour for each role | Warm ochres and greens; cold steel and teal; ash greys with ember accents |
| Its light: sun colour and height, the shadow tint, the number of bands | A low gold sun with violet shadow; a pale white sun with hard blue shadow |
| Its haze and sky | Thick warm haze; a clear black sky with stars |
| What glows, and in what colour | Lanterns, neon, runes, fungi |
| Its motifs and parts | Timber and thatch; panels and cables; chitin and crystal |

The starting realm reads more fantasy than medieval, given the lore ([vision.md](vision.md)), so its things need not be historically true: hand-made, with a touch of carved ornament and organic curves (the developers, 2026-10-12).

## Life-sized

The world is at real-world scale (D110, Decided). A broadleaf tree stands 15 to 25 metres, a pine taller; boulders range from knee-high to bigger than a house; hills rise tens of metres and mountains hundreds. The player should feel small against the land, which is also what makes haze and layered distance work. Simple shapes do not mean small things: a model is simplified, never shrunk.

## How the look is made

| Ingredient | Where it comes from | State |
|---|---|---|
| Shapes and roles | Models made to [art-style.md](art-style.md): concept, Tripo, clean-up into palette roles (D147, D115) | Working for props; characters still carry Tripo's painted textures |
| Banded light | A lighting shader that cuts light into a few crisp bands, the shadow band tinted cool | Not built. Shown in Blender in the bake-off; the game uses Bevy's standard lighting |
| Crease-aware normals | Normals smooth across curves, split at creases over about 50 degrees | Not built. The prop clean-up flat-shades every face |
| A palette per realm | Each role's colour, from the realm's data | Roles work; one palette, the starter's |
| Haze and layered distance | Distance fog tinted toward the horizon | Working |
| Sky | A gradient shader with soft clouds, day and night | Working |
| Bloom and grading | Camera effects built into Bevy | Working |
| Dense foliage and wind | Instanced plants, merged grass blades, one wind shader | Working near the start |
| Ground cover | Each ground vertex says what grows there; the shader paints it | Working on the stand-in planet |
| Water | A see-through surface mirroring the sky at a low angle | Working; no waves |
| Shade in corners | Screen-space ambient occlusion | Working; to be checked against the bands |

## Principles

- **The art is the simulation rendered.** Sky colour from air pressure, ground tint from moisture and life, plant density from the simulation.
- **One material pipeline for everything.** Every model goes through the same roles, palette and shader, so the game reads as one thing.
- **Effort goes into light and atmosphere**, which is shader code and shared by everything.
- **Fullness comes from quantity.** Ground cover, undergrowth and canopy in layers, as in Sons of the Forest (D148), drawn in the same simple shapes.

## Open questions

- **What does a dead planet look like?** It has to be beautiful too, with no life to fill it. Its palette and light have to carry it (D59, D17).
- **How do the bands look on smooth voxel terrain?** Terrain is lit per vertex today; banded light on fine ground may look like contour lines.
- **How does a palette change through seasons and phases** (D149)? Each realm needs a palette made by rule, and nobody has drawn one.
- **Night.** Banded light from a moon and from lamps.
- **Blur and framing**, which matter more in third person.
