# Biomes: how a world fills with life

Status: Open, partly answered (D148). A draft for the developers, 2026-10-12, written after they named Sons of the Forest's world design as an inspiration (D85): "beautiful, full, looks organic". The same day they answered its first questions: forests as dense as Sons of the Forest's, plants that grow back (D150), seasons (D149), worn paths as a goal if it can be done, and biomes made by rules, not chosen. It proposes goals, then how the technology could reach them on procedural voxel terrain. How shipped games do it is surveyed in [research/procedural-vegetation.md](../research/procedural-vegetation.md).

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

Proposed, and reshaped by the developers' answers of 2026-10-12. Each goal says what it means on screen, so it can be checked.

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
5. **As thick as a real forest, and still playable.** As dense as Sons of the Forest's: "more realistic, and that's our goal" (2026-10-12, D122). In deep woods you can't see far. You can still walk through the understorey, see between the nearest trunks, and find clearings to build in (D131).
6. **Life-sized** (D110). Trees stand 15 to 35 m, and boulders reach more than a person's height. Detail scales with the world, not with the object.
7. **Alive.** Everything moves in the wind, grass and bushes give way as you pass, and dug or built ground clears. What is cut grows back, and you see it grow rather than find it whole again (D150).
8. **Every world its own, by rules.** Nobody writes a list of biomes. Each planet's come from its theme and seed (D8, D17, D59), so two temperate planets still grow different forests. A gothic realm's dead trees and roots, or a techno realm's growth, are new rules and data, not new code.
9. **Interactive at scale.** Likely every plant can be harvested, perhaps not some grass (D43). The server keeps nothing per plant but what players changed (D47, D52).
10. **Fast.** 60 fps on the mid-range target, checked on the integrated GPU too (D25).
11. **Seasons** (D149). Leaves turn and fall, snow comes and goes, and grass browns and greens again. They change play as well as the look: cold, food, growth.
12. **Paths worn by use.** A stretch goal: where players and creatures walk often, the grass wears into a path, and it grows over again when no one does.

**Not goals:**

- Photographic detail, while the look is stylised (D14). The developers like Sons of the Forest's art style but think it is likely beyond us (2026-10-12), so its realism applies to the forest's density, not the look.
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
- **Leaves as flat cards or solid painted clumps** is open: the developers have no view yet. Cards are what realistic forests use (SpeedTree, and very likely Sons of the Forest); clumps suit a painted look. Proposed: a tree generator grows a branch skeleton, and leaves are a layer on it in either form, so both can be rendered side by side on the vantage sheet and chosen by eye. Leaves as a layer also give bare winter trees (D149).

### Biomes from rules, not a list

Proposed, technical. The standard way to classify climate is by temperature and wetness (Whittaker's biome diagram); Minecraft's generation picks biomes from noise fields for temperature, humidity and more. We can go one step further, as Far Cry 5's species "fight for the ground":

- **Each species has a niche:** the temperature, wetness, shade and soil it tolerates, and how strongly it competes.
- **The climate fields say what each place offers.** The species that fit there grow, the strongest most.
- **A biome is what emerges,** named afterwards for maps and talk, never listed. A theme sets the ranges of the fields (a frozen realm is cold everywhere) and the families of species it draws from.
- **Species are generated too.** A species is the parameters of a generator (height, form, bark, leaf colour), drawn by the seed within its family. The starter planet's pine is one point in that space, and another planet's is a neighbour. This asks for trees and plants as generators, with Tripo models as parts and variants rather than as whole species (D127).

### Regrowth

That things grow back is decided (D150); how is open. Proposed, to react to:

- **In stages, from time alone.** A cut leaves a stump, and the stored change records when it happened. The plant's stage (stump, sapling, young, grown) is worked out from the time since, the species' growth rate and the season. Nothing ticks. When the plant is grown again its change is forgotten, so the world's stored state shrinks back on its own (D47, D52).
- **Seen to grow, never popping.** A stage changes only while no player can see the plant. You never watch a tree jump, but come back after a while and the clearing has saplings, later young trees. Shipped games do the same out of sight: Rust refills resources away from players, and The Forest regrows trees while you sleep.
- **At each kind's own pace** (decided, D150): a growth rate per kind, as data. Grass takes hours, bushes days, and great trees much longer.
- **Removing the stump stops it** (decided, D150, to revisit if it causes trouble), as in The Forest. That gives a way to clear land for good.
- **Inside a claim, nothing grows back by default** (D150, to revisit). The world heals outside claims (D94), and inside them is the owner's.

### Seasons

That there are seasons is decided, and that they change play, not only the look (D149). One season covers a whole world: the hemispheres are not opposite, which would be complex for little use (2026-10-12). Realms give the same variety instead: through a portal, another world may be in another season.

#### How long a year is

Proposed, for the developers. How others do it:

| Game | Year | Season | Real time |
|---|---|---|---|
| [Sons of the Forest](https://dotesports.com/indies/news/sons-of-the-forest-season-length-how-long-does-winter-last) | 20 days | 5 days by default; 10 on Long, 90 on Realistic | Paused when no one plays |
| [Don't Starve Together](https://gamerempire.net/dont-starve-together-summer-winter-start-and-end-time/) | 70 days | 15 to 20 days | About 9 hours of 8-minute days, paused when the server is empty |
| [Wurm Online](https://wurmpedia.com/index.php/Day), a persistent MMO | 336 days | | 42 real days; a day is 3 real hours |

The difference that matters: those first two pause, and ours is a persistent world (D52), so its year passes in real time whether you play or not.

What the length has to do:

- **Long enough to prepare for and live through.** Autumn is for laying in food and wood, and winter must last long enough to test it.
- **Short enough to meet.** A player who plays some evenings should meet every season within a few weeks.
- **Not a whole number of real days or weeks,** so whoever plays on the same evening each week doesn't always find the same season.

**Proposal: a year of 365 days, as a real one (D122).** At the half-hour day (D134), that is about 7.6 real days, so a season is about 45 real hours: one or two evenings for most players, a whole year in about a week, and drifting against the real week. It is a number per world in its theme, so a realm can have a long year, a short one, or none. Days could also lengthen in summer and shorten in winter around the half-hour, as real ones do. Tuned by playing.

#### How seasons change play

Proposed, to choose from; most build on survival (D61) and creatures:

- **Cold.** Winter wears down health without warmth: clothing, fire and shelter matter.
- **Food.** Berries and fruit come in summer and autumn and are scarce in winter. Laying food in and preserving it (drying, smoking) becomes a loop.
- **Growth.** Plants and regrowth grow only in the growing season. Spring is when cut land greens again.
- **Creatures.** They breed in spring, migrate or sleep through winter, and are bolder when hungry.
- **The land.** Snow slows walking and shows tracks, which fits knowing by looking (D93). Lakes freeze and can be crossed, and rivers rise with the thaw.
- **Seeing.** A bare winter wood can be seen through, and a summer one hides.
- **Weather by season** (D78): summer storms, winter blizzards.

#### What it changes in the look

Mostly data per kind:

- colour through the year;
- deciduous canopies thinning to bare branches;
- leaf litter in autumn;
- snow on the ground and on surfaces facing up;
- grass browning;
- flowers in their months.

It costs mostly shader parameters and colour tables. Snow needs a ground channel, and bare winter trees need leaves as a separable layer.

### Worn paths

A stretch goal, and feasible at moderate cost, because it reuses cover and regrowth:

- **The server counts footfall** on a coarse grid, about a metre, wherever players and creatures walk. It already knows where everyone is, so this costs no network. Only walked cells exist.
- **Wear thins the cover, then bares the ground** into a path, through a ground channel.
- **Regrowth fades it** when unused, so what is stored is only what was walked recently.
- **Paths change slowly,** so they are sent rarely and in batches, like other world changes.
- **Creatures wear game trails, and settlements generate paths between their places,** so the world has paths before any player arrives.
- **The hard parts:** keeping paths readable lines rather than muddy blobs, by thresholds and smoothing; and the volume of footfall from a thousand players, which the coarse grid and decay bound.

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
4. **Content:**
   - a tree generator, with leaves as a layer (cards and clumps compared);
   - species with niches;
   - a second planet's theme, to prove its forests come from rules alone.
5. **Time:** regrowth by stages and seasons.
6. **Worn paths.**

Each step is judged on the vantage sheet.

## Open questions for the developers

Answered on 2026-10-12:

- **Density:** as Sons of the Forest's.
- **Harvesting:** likely every plant, perhaps not some grass (D43).
- **Regrowth:** yes, at least by time passed, and perhaps seen to grow rather than coming back whole (D150).
- **Seasons:** yes, changing play, one season per world (D149).
- **Regrowth's pace:** by kind; nothing regrows in a claim by default (D150).
- **Worn paths:** "would be amazing as a goal", if it can be done.
- **Biomes:** made procedurally, not chosen.
- **Leaves:** no view yet.
- **The look:** Sons of the Forest's art style is liked, "ambitious, we probably can't pull it off". D14 is unchanged.
- **Stumps:** removing one stops regrowth (D150).

Still open:

1. **Which grass can't be harvested?** Proposed: short ground cover can't be, and tall grass and reeds can, giving fibre.
2. **How long a year is.** Proposed: 365 days, about 7.6 real days ([above](#how-long-a-year-is)).
3. **Which of seasons' effects on play** we want ([above](#how-seasons-change-play)).
4. **Cards or clumps** for leaves, chosen by eye from a side-by-side once a tree generator exists.
