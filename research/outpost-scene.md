# Can the sandbox show something worth showing?

2026-10-10. The first playable demo had a capsule for a player, flat ground in triangles twelve metres wide, and a base on stilts. The developers said it looked bad and asked for a proof of concept that could impress someone. The question: with no artist, how far do free models, our own colours and the engine's built-in effects get?

## Answer

Far enough to show. The `play` experiment is now a furnished base in a meadow by a lake, with an animated figure and a builder tool, drawn in 4.4 ms a frame on a fast graphics card. It is not fast on a weak one, and none of it has been tuned.

![The outpost](../docs/images/outpost-2026-10-10.png)

## What made the difference, most first

1. **Real models.** Free CC0 packs by Quaternius, which are flat-coloured with no textures. We read only their shapes and material names and choose every colour ourselves, so trees, furniture and crates from four different packs sit in one palette.
2. **Ground you can stand close to.** A square 500 m across around the start is drawn at one-metre spacing instead of sixteen, and eases into the coarse planet at its edge so the two meet exactly. Each vertex says what grows there, so the grass that is drawn and the grass that is planted agree.
3. **Quantity.** About 13,000 trees, rocks and plants, and 650,000 blades of grass within 95 m of the base. Lushness is mostly a count.
4. **Light.** A low warm sun, cool fill from the sky, four cascades of shadow, ambient occlusion, a little bloom, haze. All built into Bevy except the sky.
5. **Movement.** One vertex shader sways every plant, and is used for the depth and shadow passes too, so nothing shimmers.
6. **A building that looks built.** The base is still tiles, but each wall gets a band, posts, seams and a parapet, openings get frames, and it sits on a foundation with steps and lamps.
7. **Building from a menu.** Added the same day at the developers' request: a dial of parts with variants, a preview of what would be built, and furniture as items you place. See [subnautica-2-building.md](subnautica-2-building.md).

## Where the models came from

| Need | Source | Notes |
|---|---|---|
| Trees, rocks, plants | Quaternius Ultimate Nature Pack | Uncompressed, 70 to 3,000 triangles each, modelled Z-up at 1:100 |
| Furniture, crates, a fire | Quaternius House Interior, Modular Sci-Fi and Survival packs | The same, in arbitrary sizes, so each is scaled to a stated size |
| A figure that moves | Quaternius Universal Animation Library | A mannequin with 46 animations on its own rig; we keep eight |

All are CC0, confirmed on Quaternius's FAQ. Details are in [THIRD_PARTY.md](../THIRD_PARTY.md).

Looked at and not used: the Ultimate Space Kit has astronauts with walk and run animations, but they are animal-headed, textured, and stored compressed. The Modular Men pack has a plain astronaut in the right style with no animations, and its skeleton does not match the animation library's, so using it means retargeting.

## Measurements

Windows build, 2400 by 1350, vsync off, 600 frames after settling. Ryzen 9 9950X3D.

| GPU | Median frame | 99th percentile |
|---|---|---|
| RTX 5090 | 4.4 ms | 5.6 ms |
| Integrated Radeon | 190 ms | 204 ms |

Start-up, from window to everything planted, is about half a second.

One lesson cost a factor of eleven. The wind's clock was first sent by changing the plants' material every frame. Bevy treats a changed material as a reason to re-prepare everything that uses it, and with 13,000 users that took 45 ms a frame. Reading the renderer's own clock in the shader instead brought the frame from 50 ms to 4.4 ms. Changing a widely shared material every frame is something to avoid everywhere.

## Limits

- **Not tuned for weak hardware.** Every tree is drawn at full detail at any distance, into four shadow maps. The integrated GPU's 190 ms says levels of detail, and grass that follows the player, come before this scene is a fair test of D25.
- **Stock shapes.** They carry our colours, not our design, and there is nothing here for a dead planet.
- **The scene is local.** Grass is planted once around the base; walk 100 m and it runs out.
- **The stand-in planet.** No voxels, no digging. A base is lifted clear of the ground and given a foundation, where the real game would have to decide how buildings meet terrain.
- **Collision is still arithmetic** (D53): circles round trunks, boxes round placed items, a grid for the base.
- **Nobody has played it yet.** Every check was a screenshot or a scripted run.
