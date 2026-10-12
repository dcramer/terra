# Biomes: how a world fills with life

Status: Open. A draft for the developers, 2026-10-12, written after they named Sons of the Forest's world design as an inspiration (D85): "beautiful, full, looks organic". Nothing here is decided. It proposes goals to agree, then how the technology could reach them on procedural voxel terrain. How shipped games do it is surveyed in [research/procedural-vegetation.md](../research/procedural-vegetation.md).

What this covers: everything that grows or lies on the ground, from grass to the canopy, plus deadfall and rocks. The industry calls it vegetation and procedural placement, or set dressing when done by hand. Creatures, settlements and ruins are placed by the same machinery later, but they're not covered here.

## Where we are

- **Planted once, around the start.** `play::flora` plants trees out to 340 m from the start site, and grass out to 95 m, on a flat map laid over the ground. Walk past that and the ground is bare.
- **Kinds are data.** `assets/data/flora.ron` has 12 kinds in five layers (trees, rocks, bushes, plants, flowers), each chosen by noise. Trees, rocks and plants are one entity each; grass is built in 10 m squares. Everything sways in one wind.
- **It follows digging.** Plants go where the ground is dug, and grass grows back around a dig.
- **The whole planet is affordable.** The `forest` experiment held a million choppable trees on a 2 km planet by computing them from the seed and remembering only what was cut (D47; [research/forest-scale-test.md](../research/forest-scale-test.md)).
- **What's missing:**
  - rules from the land (slope, water, shade, soil);
  - ground cover other than grass;
  - deadfall;
  - the forest seen from far off;
  - more than one biome, and the blend between them.

## Goals

Proposed, for the developers to change. Each goal says what it means on screen, so it can be checked.

1. **Full.** Close to the player in a living place, no ground is bare unless the bareness means something: a path, rock, sand, a dug hole, a camp. Ground cover is continuous, and its density near the camera matters most.
2. **Layered, as a real forest is.** Five storeys of growth:
   - ground cover: grass, moss, needles, leaf litter;
   - herbs: ferns, flowers, mushrooms;
   - shrubs and saplings;
   - understorey trees;
   - the canopy.

   Among them lies what has fallen: logs, stumps, branches, and rocks from pebbles to boulders. Nothing stands on bare ground: a boulder has moss on its top and ferns at its foot.
3. **Organic.** What grows follows the land and the other plants:
   - slope, height, nearness to water, facing the sun, the soil;
   - shade under the canopy;
   - species growing in groves;
   - edges that grade from forest through shrubs to meadow.

   The check: no screenshot shows a grid, a repeat, or a hard line where one rule stops.
4. **Varied at every scale.** At the scale of a region, different biomes. At the scale of a stand, a grove of birch among pines, a clearing, a bog. At the scale of a patch, a ring of mushrooms or a bed of ferns by a log. A few minutes' walk always passes something new.
5. **Playable.** Full doesn't mean impassable. You can walk through the understorey, see between trunks, and find clearings to build in (D131). How thick a forest may be is a question below.
6. **Life-sized** (D110). Trees stand 15 to 35 m, and boulders reach more than a person's height. Detail scales with the world, not with the object.
7. **Alive.** Everything moves in the wind, grass and bushes give way as you pass, and dug or built ground clears. Whether cut things grow back is open (D43).
8. **Every world its own.** A biome is data in a planet's theme (D59). A gothic realm's dead trees and roots, or a techno realm's growth, are new data, not new code.
9. **Interactive at scale.** Every tree can be felled, and the server keeps nothing per plant but what players changed (D47, D52).
10. **Fast.** 60 fps on the mid-range target, checked on the integrated GPU too (D25).

**Not goals:**

- Photographic detail. The look stays stylised (D14).
- Anything placed by hand. Any number of worlds (D17) means every place comes from rules.

## How shipped games do it

From [the survey](../research/procedural-vegetation.md):

- **Placement is rules, not hand work.**
  - Horizon Zero Dawn places all its nature from density maps and blue-noise patterns, deterministically. Three people made its nature assets and one its ecosystems ("ecotopes", our biomes). Changing one map changes only what is near the change.
  - Far Cry 5 and Ghost Recon Wildlands have species compete for ground, and big trees seed smaller ones.
- **Fullness comes from layers, density near the camera, and clumping**, not from detailed models.
- **Grass is generated on the GPU,** a curve per blade in Ghost of Tsushima, clumped and fading into painted ground with distance.
- **Far trees are impostors.**
- **Only edits are stored** over the generated world (Valheim, Astroneer, Rust).
- **Regrowth, where it exists, is a refill:**
  - Rust tops resources back up to a density;
  - The Forest and Sons of the Forest regrow a share of trees while you sleep, unless the stump is gone.

## How it could work

Technical, so mine to choose, but it rests on the goals above, so it is Leaning until they are agreed and nothing is built on it before then. It follows Horizon Zero Dawn's model, the best documented and the closest to our rules (data-driven, deterministic, locally stable, little authoring).

### Three tiers, by who has to agree

The engine plan already splits harvesting three ways ([engine.md](engine.md#harvesting)). Placement and drawing follow the same split:

| Tier | Examples | Must agree | Placed | Drawn | Changed |
|---|---|---|---|---|---|
| **Objects** | Trees, saplings, boulders, logs, stumps, big bushes | The server and every client: they collide and can be cut | On the CPU, from the seed, in a crate without Bevy, the same on every machine | Instanced meshes with levels of detail, impostors far off | One operation per object (D47) |
| **Cover** | Grass, moss, ferns, flowers, pebbles, litter | No one: it is only seen, except where it is cut | On the client, from the terrain's own mesh, on the GPU where that pays | Generated around the camera, thinning with distance into the ground's colour | With the terrain: regenerated with its chunk; a cut area is an operation clearing a disc |
| **Far** | The forest seen from a hill | No one | From the same rules, coarsely | Impostors, then the ground's own tint | Not at all |

Only objects cost the server anything, and only when changed. Cover is most of what makes the world look full, and it costs only the client.

### Rules: a stack of fields

Everything is a function of the seed and the place, worked out wherever someone is (D41, D47). From the bottom up:

1. **The land**, from the terrain: height, slope, which way the ground faces, the soil (the voxel material), and nearness to water.
2. **The climate**, from the theme: temperature and wetness across the planet, with noise.
3. **Biome weights.** At every point, how much of each biome there is, blended smoothly so there are no lines. A biome is data: the kinds in each layer and how dense each is against the fields.
4. **Layer by layer, top down.** The canopy first. Then the layers below read what is above them:
   - shade and root space under trees;
   - understorey liking or avoiding shade;
   - deadfall near old trees;
   - ground cover by shade and wetness.

   This is how real ecosystems layer, and it gives the organic look without simulating growth.
5. **Spacing and clustering.** As in Horizon Zero Dawn:
   - each cell's density is thresholded against a blue-noise pattern spaced to each kind's footprint, so nothing lines up and trunks don't overlap;
   - layers that share a footprint take separate threshold ranges, so they never collide and need no collision pass;
   - species clump by low-frequency noise;
   - big trees seed smaller ones around them, as in Wildlands.

### On voxel ground

Heightmap games sample a height. We have digs, overhangs and caves, so:

- **Objects stand on the ground as generated, before any edit.** Edits then remove the objects whose ground went. If placement read edited ground, one dig would move every tree near it.
- **Cover grows on the terrain's mesh**, seeded by world position so it doesn't shift between levels of detail. It covers any surface, follows every dig, and is rebuilt with its chunk.
- **The ground carries much of the fullness.** The terrain's vertices already carry how grassy and how lush the ground is. Adding moss, needles and litter, driven by the same rules (needles under pines), makes the far ground read full at no cost in geometry.
- **Caves get their own layers later**, by the same rules, with darkness instead of sun.

### Drawing

Bevy 0.19, as pinned, has:

- GPU-driven instancing and culling;
- GPU occlusion culling;
- `VisibilityRange` for dithered cross-fades between levels of detail, down to an impostor;
- volumetric fog, ambient occlusion, contact shadows and decals.

It has no grass, scatter, impostors or wind of its own, and no community crate for them is ready for the current version. Those are ours to build. Two things to know:

- **A custom material must be bindless** to stay on the GPU-driven path, and our wind material is custom.
- **Meshlets (virtual geometry) can't draw foliage.** They take no alpha mask and no vertex shader of their own, so there is no wind.

Bevy 0.20 (out 2026-10-08) adds mesh shaders, which its notes suggest for grass; worth a look when we upgrade. What we'd add:

- **Levels of detail for every model.** The pipeline already cuts a model to several triangle budgets ([research](../research/ai-models-for-art.md#the-bake-off-first-round)).
- **Impostors for distant trees.** A tree is drawn as a few camera-facing images from about 150 m out, so a forest can stretch to the horizon.
- **Grass as generated blades** near the camera, fading into the ground's colour, so the edge of the grass never shows.
- **Light through the canopy:** dappled sun, and fog catching it. This is most of Sons of the Forest's mood, and Bevy has the pieces.
- **Shadows only where they show:** full near, cheap or none far.

### What each layer needs from art

- **Trees** are the hardest, and the weakest area for generated models. Trees made by code, with rules for trunk and branches (as SpeedTree does), would give many variations per species, at life size, in our painted look. Each species would be parameters, not models.
- **Rocks, logs, stumps, mushrooms and the like** suit Tripo. The pipeline already makes them.
- **Ferns, grass and flowers** are few triangles and many copies. Small generators or simple models are enough.
- **Whether leaves are flat cards or solid painted clumps** is part of the look, so it belongs with D14.

### How we'd know (D69)

- **A vantage sheet.** Scripted views of fixed seeds:
  - the forest floor at eye height;
  - a forest edge;
  - a clearing;
  - a stream;
  - a hilltop view.

  Rendered side by side, and compared with Sons of the Forest screenshots by link (not committed).
- **A measure of bare ground.** The share of the ground in view within 30 m that nothing covers, in a living biome.
- **A walk bench.** The camera flown a few kilometres along a path, recording frame times and hitches while the world streams in, on the RTX 5090 and on the integrated GPU.

### Order of work, once the goals are agreed

1. **Placement everywhere.** Rules and biomes as data, with objects streamed in cells around every player instead of planted once at the start. Measured.
2. **Cover on the terrain**, and the new ground channels.
3. **The far forest:** impostors and draw distance.
4. **Content:** a tree generator, more kinds per layer, and a second biome to prove a biome is only data.

Each step is judged on the vantage sheet.

## Open questions for the developers

1. **How thick is a forest?** As dense as Sons of the Forest, where you can't see far, or thinner and easier to cross?
2. **Is every plant harvestable,** or only trees, bushes and some herbs (D43)?
3. **Does what is cut grow back,** and how fast? Outside claims the world heals (D94); does a felled forest?
4. **Seasons?** Sons of the Forest has them: snow falls and melts, and leaves turn. They change the look the most, and cost the most.
5. **Do players wear paths** that stay?
6. **Which biomes on the starter planet?** Proposed to react to: temperate forest, meadow, shore, wetland, and alpine.
7. **Flat-card leaves or solid clumps?** This is part of the look (D14).
