# Mountains that look proper: ours against real ranges and shipped games (2026-10-19)

The question: what makes a range read as real, rocky, tall mountains, why ours do not yet, and how our voxel terrain gets there in a few hundred variants (D182). The developer, on the settlement's mountains: "really, i like mountains that look rocky and tall, but it needs to have variety and look great. i dont care if they are smaller vs. bigger as long as they just look really proper."

It follows [the far mountains note](2026-10-16-far-mountains.md) (D394, D395, D401, D403), which fixed the far wall's artefacts and broke it into ranks. This note looks at what is still missing, and ranks it.

## Ours against the references

**Stills.** Main at 66f4bb55, 09:00 (dawn at the lake), from the places' own sites (`places.ts`), shot after frame 60 (D420). They are in `~/.cache/terra-review/mountains/ours/`, outside the repository:
- the settlement's valley, 60 m up, looking north at the range (`settlement-00390.png`);
- the lake, from up the shore across the water, 10 June (`lake-00390.png`);
- the tundra's fells, 8 October, at 13:00, since 09:00 there is before dawn (`tundra4-00990.png`): rounded fells with snow in streaks, washed pale under a white sky.

**References.** 12 frames in `~/.cache/terra-review/mountains/refs/`, with `credits.txt`:
- 6 photographs: the Tatras over Morskie Oko, the Dolomites' Sella and Langkofel, the Odle in snow, the Matterhorn, Fitz Roy, and Lauterbrunnen's walls (Wikimedia Commons, CC BY-SA).
- 6 game frames from Steam store screenshots: Witcher 3's Toussaint peak and Kaer Morhen's cliffs, RDR2's Grizzlies, Horizon Forbidden West's Daunt, and Ghost of Tsushima's ridges and Iki's cliffs.
- Kingdom Come 2's store frames hold no mountains.

**Measured.** Band numbers come from `look.ts measure --against morskie-oko-day` (D326), on the sky, far, middle and near bands. Contrast is local contrast within a band; "far under sky" is how much darker the far band is than the sky.

| | Settlement | Lake | Tundra | 5 references, mean (range) |
|---|---|---|---|---|
| Far band contrast | **0.032** | 0.162 | 0.191 | 0.210 (0.12 to 0.29) |
| Middle band contrast | **0.050** | 0.106 | 0.237 | 0.202 (0.12 to 0.27) |
| Far under sky | **-0.006** (far land as bright as the sky) | **0.526** (cut-outs near black) | 0.553 (white sky) | 0.06 to 0.20 |
| Far saturation | 0.053 | 0.167 | 0.184 | 0.222 |
| Sky saturation | 0.265 | **0.034** | **0.032** | 0.417 |

The references are Witcher 3's Toussaint, Horizon's Daunt, and the Sella, Odle and Matterhorn photographs.

The lake's and the tundra's skies are white overcast, the climate's weather on those dates. The references are almost all clear blue, and a white sky behind a pale range takes away the one contrast left. The tundra's band numbers come mostly from its near heath and boulders.

**A run to check, not diagnosed:** at eye height, the settlement's north view (`valley` place, 12 m up) and the lake's `peaks` view show no range at all, even at frame 990. The settlement's horizon is flat. The lake's is the painted cones over black water, with flat grey shards hanging in the sky. The same ranges show from 60 m up. The view was held from frame 0 or 200 and shot at 190, 390 or 990, with the same result.

**What reads wrong, by eye, the stills beside the frames:**

1. **Silhouette and shape (settlement).** The range is soft domes and rounded shoulders, like draped clay, with no vertical faces anywhere. Every reference reads by its faces: walls of 60 to 90° where rock stands bare, lit on one side and dark on the other, with sharp crests, towers, notches and horns. Our skyline has the right height and ranks (D401), but each top is a rounded hump, not a peak.
2. **Rock against snow against scree.**
   - One pale warm grey covers every face. D403's tiers, weathering and skirts show only as faint pencil lines, and as faint pencil lines read as painted stripes, not as cliff bands with ledges.
   - At the lake, grass climbs to the crests, and snow lies as round blobs on gentle domes. The domes read as snowy hills, not as mountains.
   - The references are dark rock in shade, bright snow on ledges and in gullies, and pale scree fans under the walls: three values that touch.
   - Ours has no dark value, so the far contrast is a sixth of theirs.
3. **Erosion detail.** It is a fine vertical comb of even flutes over smooth bulk. The references have few, deep, branching gullies that start at notches in the crest, ribs and buttresses between them, and debris cones where they end. Ours lacks the scale between the ridges (km) and the gullies (100 m): the spur-and-valley branching that flow erosion makes.
4. **Scale cues.**
   - No trees run up the far slopes to a tree line; the far forest stops at 1 to 2 km.
   - No scree cones or boulders read at the foot of a wall.
   - Nothing stands against the walls to measure them by.
   - The references' tree lines, larches, villages and paths make a 1,000 m wall read as one.
5. **Aerial perspective.**
   - At the settlement, the far land is as bright as the sky (-0.006 against 0.06 to 0.20), washed out rather than layered.
   - At the lake, the gaps between the near domes show `Sky::far_land`, the ridges painted in the sky: a row of identical dark sawtooth cones, nearly black, over a near-black band. It is the one thing in the stills that reads as fake at a glance.
   - The lake draws them (`scene.rs` `far_ranges`, D273) because the 30 km world is small. The horizon from an eye 1.7 m up is about 320 m off, and a 2 km summit sinks under it about 11 km away. So real far ranks can only come from ground within about 10 km.
   - The references step each farther range paler and bluer by a small, even amount. Their nearest walls keep near-black shade.
6. **Light.** The settlement's wall faces south and is lit from the front, so its faces barely differ. At that distance it casts no shadows on itself. Most of the references are side-lit; their contrast comes from faces turned away and from shadow cast across faces. Front light flattens even a good shape, so a view's hour is chosen for side light. A shape with no faces stays flat at any hour.

## What makes mountains read as proper, ranked

From the references and the table above, the largest first:

1. **Steep bare rock faces: shape, not texture.** Faces of 50 to 90° with crisp edges, cliff bands and ledges in them, towers and horns on the crest. These hold the dark value and the silhouette. Without them, nothing else reads as mountain.
2. **Value structure: dark rock, bright snow, pale scree.** Snow lies only where the slope holds it: under about 50°, on ledges, in gullies and on lees. Rock shows dark through it. Debris is paler than the face it fell from. That gives the far band its contrast of 0.12 to 0.29.
3. **Erosion at every scale between ridge and gully.** Branching valleys and spurs (km), couloirs from notches in the crest (100 m), ribs and buttresses, talus cones at about 35° where the couloirs end. The ridge reads sharp because its flanks are cut back to it.
4. **A clear silhouette per range kind.** A horn, a tower group, a saw-edge, a table, a cone: the type of range is told by its outline at a glance. Variety across a horizon comes from two or three kinds side by side, not from noise.
5. **Layered air.** Each range back is a step paler and bluer, its shade lifted but still darker than the sky. Real far ground, no painted cut-outs.
6. **Scale cues.** A tree line climbing the slopes and fraying in gullies, a scree fan at the foot of a wall, and a lake, village or path beside it.

## How shipped engines and tools make mountains read

Sources were checked on 2026-10-19. "Abstract only" means the talk itself is not public.

- **Hydraulic and thermal erosion, offline.**
  - Open worlds' mountains are heightfields run through erosion tools. Hydraulic erosion cuts the branching valleys and gullies and lays sediment in the beds. Thermal erosion breaks faces and piles talus at its repose angle.
  - [Gaea's Erosion](https://docs.quadspinner.com/Reference/Erosion/Erosion.html) is scale-aware. Feature Scale is the largest valleys' width in metres (default 2,000), and Downcutting cuts deeper gullies. Its outputs Wear, Deposits and Flow mask the materials. Its [Fractal Terraces](https://docs.quadspinner.com/Reference/Profile/FractalTerraces.html) make the stepped strata.
  - [World Machine's thermal erosion](https://help.world-machine.com/topic/device-thermalerosion/) says the talus repose angle is "typically around a 30-40 degree slope", and outputs a talus mask and depth.
  - [Houdini's HeightField Erode](https://www.sidefx.com/docs/houdini/heightfields/erosion.html) has a bank angle (wide shallow channels against narrow deep ones), a thermal repose angle, and a debris layer. Its guide notes hydraulic erosion shapes more than thermal.
- **Erosion in real time or at world scale.**
  - Shallow-water erosion runs on the GPU: [Mei et al. 2007](https://ieeexplore.ieee.org/document/4392715/), [Št'ava et al. 2008](https://diglib.eg.org/handle/10.2312/SCA.SCA08.201-210) (20 fps or more at 2048x1024), [Jákó and Tóth 2011](https://diglib.eg.org/handle/10.2312/EG2011.short.057-060) (with thermal erosion), and multi-layer, with overhangs, at about 6 ms a step at 2048² ([Nilles et al. 2024](https://diglib.eg.org/items/deefa865-6a25-4463-adcd-d8de2b37507e)).
  - World-scale shape comes from stream-power erosion on a drainage graph, from uplift ([Cordonnier et al. 2016](https://hal.archives-ouvertes.fr/hal-01262376)). It is fast enough to author interactively, settling in a few hundred iterations ([Schott et al. 2023](https://dl.acm.org/doi/10.1145/3592787), [code](https://github.com/H-Schott/StreamPowerErosion)). An analytical solution needs no iteration ([Tzathas et al. 2024](https://diglib.eg.org/handle/10.1111/cgf15033)).
  - [Jain, Benes and Cordonnier 2024](https://history.siggraph.org/?p=164136) find that fluvial erosion alone leaves "unrealistically uniform" steep slopes near ridges. Debris flow carves the scars and deposit cones there. That is our comb's problem exactly.
- **Ridged and domain-warped noise.**
  - Musgrave's ridged multifractal gives the crests. Quilez's [derivative-damped fBm](https://iquilezles.org/articles/morenoise/) leaves flats beside rough ground, and his [domain warping](https://iquilezles.org/articles/warp/) bends ridges.
  - Runevision's [erosion filter](https://blog.runevision.com/2026/03/) (March 2026, MPL-2.0) lays gullies per point, down the slope, each octave avoiding the last's creases.
  - All of this is ours already (D322). Per-point noise cannot route flow, so it cannot make branching drainage. That is the gap in our ranked cause 3.
- **Strata and cliff shading by slope and height.** Rock strata, cracks and streaks are laid on cut faces by slope, height and the erosion's flow mask. Sediment colours go in the beds, and snow and grass go on what faces up. Ghost of Tsushima's artists wrote these rules in a small language run on the GPU ([Pohlmann, GDC 2021](https://gdcvault.com/play/1027352/Samurai-Landscapes-Building-and-Rendering), abstract only). Ours do too (D403), but on smooth faces.
- **Scree and talus.** Talus lies at 33 to 37° (World Machine; the [Scottish Geology Trust](https://geoguide.scottishgeologytrust.org/page/2138.pdf): 35 to 37° above, 31 to 34° below, about 27° once grown over), concave in profile, in cones at couloirs' mouths. In tools it is thermal erosion's debris layer, laid as real geometry, not paint.
- **Snow by slope and aspect.**
  - Snow holds up to about 50°, sloughs often from about 50 to 60°, and rarely holds above 60° (avalanche-centre guidance, slope bands only). So steep faces stay dark in any season.
  - The pole-facing snowline lies lower: in the Tatras the climatic snowline is 2,250 to 2,650 m on north slopes against 2,700 to 2,800 m on south slopes (Lukniš 1973, cited in [this paper](https://bibliotekanauki.pl/articles/294601.pdf)). The asymmetry is strongest at 30 to 50° latitude ([Evans and Cox 2005](https://dro.dur.ac.uk/1235)).
- **Rock meshes over the terrain.**
  - Far Cry 5 places procedural cliff geometry on steep terrain ([Moore, GDC 2018](https://gdcvault.com/play/1025261/Terrain-Rendering-in-Far-Cry), abstract only; [Carrier, GDC 2018](https://gdcvault.com/play/1025215/Procedural-World-Generation-of-Far) on cliff rocks by rule).
  - Horizon places by GPU rules from the terrain's data ([van Muijden, GDC 2017](https://www.guerrilla-games.com/media/News/Files/GDC2017_VanMuijden_GPUBasedProceduralPlacementInHorizonZeroDawn.pdf)).
  - Unreal's Valley of the Ancient builds its walls from Nanite rock scans.
  - In every case the heightfield gives the mass and the meshes give the crisp near and middle faces, which a heightfield at metre spacing cannot hold.
  - How RDR2 and Horizon Forbidden West build their ranges is not public in detail (inferred from frames: meshes on the faces, eroded terrain under them).
- **Far-LOD silhouettes.**
  - Ghost of Yōtei bakes its distant mountains into models with their materials in textures ([PlayStation Blog](https://blog.playstation.com/?p=411201)).
  - Erosion detail far off is a normal map baked from the full field, so a coarse mesh still shades ribs (as D394 to D403 do in the shader).
  - Ranges past the playable land are often painted into the sky, as matte paintings or skybox meshes. But they are painted from real outlines and lit to the scene's light, never a repeated shape.
- **Aerial perspective.**
  - Hillaire's atmosphere (as Unreal's, and Bevy's, D212) holds in-scatter and transmittance in a froxel volume ([EGSR 2020](https://diglib.eg.org/handle/10.1111/cgf14050)).
  - It layers ranges correctly at real distances. Our distances ×18 (D212) is what washes the settlement's range, as [demos.md](../docs/demos.md) already notes ("wrong for real mountains").

## The plan for our voxel terrain

Our ground is a density field asked per point (D322), meshed in an octree of chunks. That rules out a global simulation run per point, but gives us two things a heightfield lacks: vertical and overhanging faces, and coarse grids baked once per world, as the lakes' Priority-Flood already is. The plan uses both. Each step is measured on the band numbers above and on a still of the settlement's wall, side-lit, against the frames.

0. **The reference first** (an hour). Make `mountains-day` by band (`look.ts reference --open`) from the 6 photographs and the Witcher 3, RDR2 and Horizon frames. Give each place a side-lit mountain view in `places.ts`, held by `cargo xtask looks`: far contrast ≥ 0.12, far under sky 0.06 to 0.20.
   - Done (292e4ce2). `mountains-day` is 10 frames: far contrast 0.199, far under sky 0.204. The valley's `range` view (60 m up) and `lake-range` (80 m up the shore) are shot at 08:00 for side light and held to far contrast ≥ 0.12 and far under sky 0.204 ± 0.14.
   - The valley fails today: far contrast 0.040, far under sky 0.050. The lake-range holds (0.251, 0.322), but its far rank is still the sky's painted cones.
1. **Drainage erosion, baked per world (spike first; answered below: not as planned).**
   - Stream-power erosion on a coarse grid of the ranges, about 32 m cells (940² over the 30 km world), from the current mountain term as uplift. It is a port of Schott et al.'s code; check its licence first, or work from the paper (D196).
   - It is baked in seconds on every core (`terra_gen::par`) and read bicubically in the field.
   - It gives the branching valleys and spurs, plus a flow and a wear map for the shader and placement, as Gaea's outputs do. The per-point gullies (Runevision's filter) then follow its fall lines at finer scale.
   - The spike answers one question: does it give spurs and branching on the settlement's wall within 10 s of baking, with the cuts left draining and no new pits?
   - It moves the pinned sites, so it needs the developers' call on re-finding them (raised before in the far mountains note).
2. **Faces in the density.**
   - Where the baked slope passes a range kind's cliff angle, the density stands the surface up into faces of 60 to 85°.
   - On those faces only, the surface is pushed in and out by beds of the rock kind's thickness (warped, tilted), which makes overhangs and ledges.
   - Ledges on a face drain over their edge, so no ponds, unlike the terraces cut into the height everywhere in round one.
   - The near mesh holds them within about 1 km; past that, D403's tiers shade the same beds.
3. **Debris where it falls.** Thermal erosion on the same grid. Talus cones at 33 to 37° at the couloirs' mouths and along each face's foot, as a real raise of the ground, scree on top (D322's layer), and boulders placed on it by size down the cone.
4. **Values by slope.**
   - Snow lies only under about 50°, deepest in concave ground, on ledges and lees, and thinnest on convex crests.
   - The snowline is lower on north faces by about 300 to 400 m (the Tatras' numbers), through the climate's aspect (D323).
   - Rock in shade goes to the references' dark: its albedo by kind, measured against the frames.
5. **Cliff meshes on the faces.** A kit of 4 to 6 large rock modules (10 to 40 m) per rock kind, from CC0 scans as the rocks are (D222), placed by rule on faces over 55° in near and middle distance, sunk into the density, with impostors far off. Only if steps 2 and 4 leave the middle distance soft.
6. **Far land and air.**
   - Replace `Sky::far_land`'s identical cones with real outlines: the range kinds' profiles, drawn at their true distances, paler rank by rank. On the 30 km world, real ground reaches only about 10 km before it sinks under the horizon, so whatever is drawn past that must still be painted.
   - Set the air's scale for open views by the far band against `mountains-day` (amends D299, as demos.md plans).
7. **Scale cues.** The far forest on slopes to the tree line (demos.md's far forest), so a wall has a tree line under it.

**Range kinds (D182).** A range kind is a set of the `Mountains` numbers, plus a cliff angle and bed thickness, a rock kind from the theme's materials, and a snow regime. A player tells kinds apart by their outline at a glance:

| Kind | Real model | Outline | Faces | Debris and snow |
|---|---|---|---|---|
| Horns and arêtes | High Tatras, Mont Blanc, Fitz Roy | Pyramids and knife ridges | Slabby granite, few beds | Big grey talus cones; snow in couloirs |
| Towers and tiers | Dolomites, Tre Cime | Flat-topped towers out of grassy plateaus | Vertical, in thick flat beds | Pale scree aprons; snow on ledges |
| Saw ridges | Valais schist, Skellige | A long jagged crest | Dark, in tilted thin beds, crumbling | Scree everywhere; snow in streaks |
| Trough walls | Lauterbrunnen, fjords | Level crests over sheer walls | Walls of 80°+ over U-valleys, hanging valleys | Narrow talus at the foot; little snow |
| Cones | Fuji, Yōtei | One symmetric cone | Ash ribs, dark | Snow in radial streaks |
| Fells and tors | Scandinavia, Scotland, the tundra | Rounded tops | Tors and boulder fields | Snow in lees |

- **Count.** 6 kinds, × 3 rock colours from the realm's materials, × 3 degrees of wear (young and sharp, worn, old), × 3 of ice (none, cirques, heavy). That is 162; less what does not go together (cones have no cirques, fells are never young), about 100 a player can tell apart.
- **How a realm picks.**
  - The theme lists its kinds with weights and its rock materials.
  - Each range (a cell of the range noise) takes one kind, its neighbours often another. So the settlement's horizon shows two or three kinds side by side: the variety asked for.
  - The kinds blend over a range's edge by the same noise, never in one range's middle.
- **What stays rules, not variants.** The tree line, the snowline and the zones on a slope follow from the climate and the date (demos.md).

## The spike: stream-power erosion on a 32 m grid (2026-10-19)

**Question.** Does stream-power erosion on a coarse grid, baked once per world under our per-point gullies, give branching spurs on the settlement's wall in under 10 s, without pits that fill as lakes?

**Answer: no.** It is fast enough and leaves no new lakes, but it gives no branching spurs. It deepens and smooths the valleys the ridge noise already laid out. On the wall, the water runs straight down the fall lines.

**How it was tested.** The spike is in git at 17349017 (`spike/mountain-erosion`, deleted since).
- **Grid.** 938² cells of 32 m, 30 km across, centred on the settlement's site (town@2.22,2.12).
- **Ground.** Sampled twice: as it is, and without its per-point gullies (`gullies_m` and `couloirs_m` at 0). The second was eroded, then the first's gullies were added back over it.
- **Solver.** Braun and Willett's implicit stream-power scheme (2013, from the paper), with m = 0.5 and n = 1. It drains over a Priority-Flood fill with an epsilon (Barnes et al. 2014). The erosion law follows [Schott et al.'s code](https://github.com/H-Schott/StreamPowerErosion) (MIT): a hillslope Laplacian, erodibility noise of ±25%, and area spread over the lower neighbours by slope. fastscapelib is GPL-3, so we used only its papers.
- **Lakes.** The lakes the ground had keep their beds. A hollow made by a step is filled until it drains, as FastScape does.
- **Measures.** On the wall, the ground steeper than 20° and over 1,100 m within 8 km of the site (18.9 km²): the channel network (D8, channels over 0.05 km², Strahler order), confluences per km², and relief within 290 m. On the whole grid, new water, found by `terra_land::lakes::fill`.

| | Bake | Highest Strahler order on the wall | Confluences a km² | Relief within 290 m | New hollows holding water | Lakes the ground had, grown |
|---|---|---|---|---|---|---|
| Before (the ground as it is) | | 3 | 1.96 | 346 m | | |
| A: erosion on that ground, 24 steps | 6 to 7 s | 3 | 1.59 | 311 m | 16, mostly 1 to 3 cells | 0.24 km² |
| A with stronger erosion (KT 3), uplift 0.6, 48 steps | 13 s | 3 | 1.75 | 406 m | 39 | 0.25 km² |
| B: range lowered to 10%, raised back as it erodes, 48 steps | 13 s | 3 | 1.80 | 329 m | 31 | 0.29 km² |

- **Time.** Sampling the field twice took 2.7 s on 24 cores. A light bake took 6 to 7 s and a strong one 13 s. Most of that is the routing (fill, sort, accumulate), which runs on one core. So 10 s holds for this patch but not for the whole sphere: at 32 m it is about 15 times as many cells.
- **Pits.** The bake leaves almost none, about 1.2 to 2 km² in 880 km², but it drains lakes. The lakes of 3 or more cells and 1 m or more deep fall from 618 to between 373 and 438. Their outlets cut down, so the pinned lake could drain too.
  - Adding the per-point gullies back over the eroded ground makes hundreds of 1-cell hollows. The ground as it is already shows 350 more hollows than it does without its gullies. That is the gullies (140 m apart) sampled at 32 m, not the bake.
- **Shape** (the sheet: `~/.cache/terra-review/mountains/erosion-spike.jpg`).
  - From above, the valleys run as before, only deeper and softer.
  - On the wall, the erosion cuts straight chutes down the fall lines, or smooths them, depending on the diffusion.
  - No run raised the wall's highest Strahler order above 3 or added confluences. A wall 1 to 2 km high is 30 to 60 cells at 32 m: too few for a network to branch on it. A network that branches at the spurs' scale forms only at a steady state reached over hundreds or thousands of steps, as in Schott et al. on the GPU. That is not a bake of seconds on the CPU.
  - The horizontal bands on the walls in the views are the spike's CPU ray-march, not the ground.

**What follows (a recommendation, not built).**
- Keep stream power out of the game for now.
- Branching spurs need a per-point model that makes them, since our field is asked per point. One example is "Dendry" (Gaillard et al., 2019, a procedural model for dendritic patterns, evaluated locally). Another is the debris-flow cuts of Jain, Benes and Cordonnier (2024) on the walls only.
- Step 2, the faces in the density, does not depend on this, and gives the dark faces the far band lacks (0.040 against ≥ 0.12). It should go next.
- The developers' "re-find them once" (D425) stays unused until something reshapes the ground under the pinned sites.

## For the developers (answered: D425)

Answered 2026-10-19 (D425): horns and arêtes, and Witcher 3-like ranges (Skellige, Velen), first; two thirds rock, one third snow in summer; the sites re-found once.

1. **Which range kinds to make, and which first?** Recommended: horns and arêtes, and towers and tiers, first. They are the Tatras and the Dolomites, the most "rocky and tall", and the most different from each other. Then the other four. The starting realm would show the first two side by side round the settlement.
2. **How much rock against snow?** Recommended: rock first. Above the tree line, in early summer:
   - about two thirds bare rock and scree;
   - a third snow, in couloirs, on ledges and north faces, with peaks capped only above about 2,400 m;
   - snow never on faces over 50°.

   It changes by date through the climate. This sets the snowline numbers and the dates the demos and trailer use.
3. **May the pinned places move?** Drainage erosion (step 1) reshapes the land under the lake and the settlement. Recommended: re-find them with `fields --site` and move their runs once, rather than fading the mountains round them again.
