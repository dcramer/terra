# Biome demos: a mountain lake, a tundra, a living town

Status: proposal for the developers, 2026-10-15. What each place is: Leaning (D321). How each is built and checked: decided by the agent (D322 to D326). The research behind it is [research/2026-10-15-biome-demos.md](../research/2026-10-15-biome-demos.md).

The developers, 2026-10-15: "I would like to see more biome demos. I would like to see a forest with a lake in the mountains, i want mountains to have both foresty part and the high peaks. i would like to see more of a tundra effect, as a final effect i would like to see a town that is bustling with life - has NPCs, maybe some animals, etc." The existing demo is the lush forest: a tall Scots pine stand (D301), judged against Sons of the Forest's frames (D311).

## What all three share

- **Made by the world's rules, not by hand** (D8, D148, D313). Each demo is the same systems as the lush forest, reading the climate fields ([climate.md](climate.md)) and the species' niches. The sandbox experiment only picks the seed, the place, the date, the hour and the camera.
  - The place is found by a query on the fields ("a basin holding a lake, with ground 1 km above it within 3 km"), as `settlement::site_for` finds the village's level ground.
  - A tundra that only exists because someone painted it would fail the brief.
- **One world for all three.** They run on the 30 km world the building scenes already use (D13), in the start's climate (`assets/data/climate.ron`), each at the latitude and height where its climate falls.
  - On a world that size, a degree of latitude is 524 m. The fields put the start's climate at about 8 °C a year at 40°, and colder by about 1.5 °C for every kilometre walked north.
  - So the tundra lies about 11 km north of the start, and a mountain's tree line sits about 1.6 km above the sea there. These are estimates from `climate.ron`'s numbers; the field maps below give the exact ones.
  - Each place's latitude is its own, from the world's frame (D331), and the sun's height is taken where the camera stands.
- **A reference before anything** ([environment skill](../.agents/skills/environment/SKILL.md), D311):
  - frames of the target from first-party sources (Steam store screenshots and trailers, sampled every 2 s), and photographs where the real place is the target;
  - kept under `~/.cache/terra-review/refs/<name>/`, never committed;
  - measured into `tools/sandbox/looks/<name>.json` with `look.ts reference`;
  - one reference per condition (day, dusk, season).
- **The budget is the developers'** (D25): 13.3 ms at the 99th percentile, 1080p, high preset, on their RTX 3080. The lush forest walk already spends all of it: p99 14.8 to 16.0 ms (D309).
  - No demo is the lush forest plus something more. Each says what it spends, measured by its own bench scene.
  - Each also reports the integrated GPU at the low preset.
- **Checkable without a person** (D69): vantage views, a route and its films, look lines against the reference, `lodcheck`, `collide` and a bench scene for each demo. An adversarial review against the reference frames at each milestone (the environment skill, §5).
- **One renderer at a time** on the developers' machine (D319).

## Tools to build, shared

Each is general, for work we will keep doing (D71, D249, D326).

| Tool | What it does | Why |
|---|---|---|
| **Field maps**, `cargo xtask fields` | A CPU-only run over the Bevy-free crates. For a region of a world on a date it writes small PNG maps with a legend, and a JSON of numbers: the height, slope and facing; the growing season's mean temperature; snow depth from the folded past; wind exposure; lakes and their levels; each species' fit and the one that wins; the tree line's height, measured. It also answers site queries, a place meeting conditions. | The rules decide every place, so the rules are checked first, in seconds, without the GPU, before anything is drawn. A tree line that comes out at the wrong height is caught here, not in a film. |
| **Places**, one table for the sandbox tools | `vantage`, `lodcheck`, `film`, `pops`, `collide` and the bench scenes take `--place lush\|lake\|tundra\|town`. Each place names its sandbox options, its views, its route, its look reference and its bench scene, in one file (`tools/sandbox/places.ts`). | Today every view, route and scene is written for the lush forest (`VIEWS`, `FOREST_VIEWS`, `ROUTE`, `--level yard`). A new place should be a table entry, not a fork of five tools. |
| **Bands in `look.ts`** | Each frame is also measured by band: the sky, the far land, the middle distance and the near ground. That includes the far land's contrast with the sky and its colour shift. | An open view is judged on its distance: peaks lost in haze, or standing too sharp, is the gap the whole-frame means hide. |
| **Life**, `cargo xtask life` (town) | Runs a settlement through a game day at a fast clock; first headless, the town's own rules on the CPU (`terra_life`, built), then read from the game over the remote protocol (D69). It reports: who was where they should be and when; anyone stuck for more than 5 s; anyone through a wall or another person; feet sliding (the speed against the clip's pace); people and animals in view at each vantage and hour; smoke rising; what was heard, by part of the mix. | "Bustling" and "going about their day" become numbers that hold from change to change. |

## 1. A mountain lake

A lake in a mountain valley. Spruce and pine come down to the water. Above, the forest thins and shrinks to dwarf pine, then gives out to alpine grass, scree fans, bare rock and snow on the peaks. Far ranges stand blue behind. The real place is Morskie Oko in the Tatras: a lake at 1,395 m under Rysy at 2,499 m, the tree line at about 1,550 m, and Wielki Piarg's scree running to the shore. That is about 1.1 km of relief within 3 km.

### (a) Reference

- **Games:** Sons of the Forest (1326470), whose snowy peaks and lakes match the forest's existing reference; Red Dead Redemption 2 (1174180); Far Cry 5 (552520).
  - Keep frames that look across a lake to forested slopes and rock, by day.
  - Leave out interiors, night, winter and menus.
- **Photographs** from Wikimedia Commons (licence checked per file): Morskie Oko, Pragser Wildsee, Moraine Lake.
  - The photographs judge the rules: where the trees stop against the slope, the scree's angle, where snow lies.
  - The game frames judge the look, which is lightly stylised (D14).
- **References to make:** `mountain-lake-day`, from game frames, measured by band; and a photo board for the review.

### (b) What exists and what is missing

| Part | Exists | Missing |
|---|---|---|
| Ground | `terra_voxel/src/field.rs` `GeneratedPlanet`: fBm hills, 150 m of relief (`terra_world/src/planet.rs` `RELIEF_M`); `Planet::rolling` 6 m for the lush forest; materials soil, stone, ore, sand; the octree in `terra_voxel/src/lod.rs` reaches the whole planet | Kilometre relief; ridges and erosion; steep ground, since `may_hold_surface` holds only to about 48°; scree, gravel and snow as ground layers in `assets/shaders/ground.wesl` |
| Water | One sea sphere (`Sea`, `assets/shaders/water.wesl`: two ripple sets, the sky's reflection, glitter); `Planet::under_sea`; `terra_core::Water` asked by position (D45), implemented by nothing; a lapping sound (`audio/synth.rs`) | Lakes at their own levels; depth colour, refraction, shore foam and wet band; reflections of the land; ripples by the wind |
| Climate | `terra_climate::Fields` with the lapse rate; the folded past's snow per 32 m cell (`past.rs`); read only for the sun's path (`terra_world/src/daylight.rs`) | Climate step 3 (samples, maps, a `--date`); aspect and shelter (D323) |
| Trees | Scots pine, Norway spruce, oak with forms, the floor, the middle layer, crowns thinning card by card (`terra_sandbox/src/experiments/lush/`), all chosen by noise (`species_at`) | Niches as data (D313); trees stunted toward their limit; dwarf pine; placement on slopes |
| Far | Far forest as cards to 1 km on near-flat ground (`lush/far.rs`); far ridges painted in the sky (`Sky::far_land`); Bevy's Hillaire atmosphere, distances ×18 (D212, D299) | Forest on far slopes; peaks as real ground to the horizon; an air scale that leaves 5 km peaks readable; the far ground shading itself past the last shadow cascade |

### (c) Parts and rules

**The ground** (D322):
- **A mountain term in the world's ground:** ridged fBm, damped by its own slope as Quilez's derivative noise is, then gullies cut along the downhill gradient. That is the per-point erosion filter Runevision describes, worked from the write-up; its code is MPL-2.0 and not ours to take (D196).
- **A theme's numbers set it:** relief, ridge sharpness, erosion strength. A range is rounded domes or sharp arêtes by those numbers, not by code.
- **Rock by slope:**
  - above 40°, bare rock;
  - 30 to 37° under cliffs, scree in cones and fans;
  - gravel in the gullies' beds;
  - soil and grass on the rest.

**Lakes** (D322):
- Every depression of the generated ground, before edits, filled to its spill point by Priority-Flood on a coarse height grid. It is the "basin filled to its own level" [engine.md](engine.md#water-and-melting) asks for, answered through `terra_core::Water`.
- Shores follow the ground under the water's edge:
  - rock where it is steep;
  - a scree fan where scree reaches it;
  - a gravel beach where it is gentle;
  - sedge and reeds where it is shallow and still.

**The tree line**, from the climate (D321, D323):
- Each tree species' niche says the growing-season warmth it needs (D313). The world's lapse rate and the slope's facing give each place's.
- A tree grows shorter and sparser as its place nears the edge of its niche. Past the closed forest it is a krummholz mat, then nothing.
- So the tree line is a band that rises on sunny slopes, drops in cold hollows and frays along ridges. It is never a painted contour.
- Above it, the tundra's own ground cover (§2) is the alpine meadow: one set of rules for both demos.

**Snow:**
- Snow lies where the folded past says it has not melted by the date, bent by the ground's shelter (D323).
- So peaks keep it in early summer, north faces and gullies keep it longest, and the lowest ridges are bare.
- Shaders lay it on surfaces facing up, as [climate.md](climate.md#drawn-from-the-same-fields) already plans.

**The water's colour:**
- Water is a medium by depth (D322): clear and dark green over rock, brown-dark over peat.
- It is milky turquoise when ice and snow lie above it in its catchment, as glacial rock flour colours Moraine Lake.

**Variants (D182)** a player can tell apart:
- **Lakes:** three basins (a cirque tarn, a lake dammed by a moraine, a long valley lake), four sizes, four shores (rock, scree fan, gravel, reeds) and three colours. Less what does not go together, about 100.
- **Ranges:** a few dozen, by form (domes, ridges, horns), rock (granite grey, pale limestone, dark schist: a theme's materials) and how worn they are.
- **The zones on a slope** (forest, krummholz, meadow, scree, rock, snow) are not variants. They follow from the place and the date, so the same valley differs by season.

### (d) Performance

The view is open: across the water, up the slopes. The lush forest's full density applies only to the shore forest within about 150 m.

| Part | GPU ms |
|---|---|
| Near forest, floor and middle layer, within 150 m, as the lush forest | 6.0 |
| Ground: near chunks, and far chunks to the horizon (about 11 km from a 2 km summit on this world) | 1.5 |
| Far forest: cards on the slopes to 2 km, past that the ground's own tint | 0.5 |
| Water, and its reflection | 1.5 |
| Sky, air and the sun's shadows | 1.5 |
| Post-processing, upscaling, interface | 1.0 |
| Headroom | 1.3 |
| **p99** | **13.3** |

**Far peaks:**
- They are the voxel octree's own coarse chunks, already drawn to the whole planet. A separate far-field mesh, the way Ghost of Yōtei bakes its far mountains, comes only if the chunk count or meshing time fails the bench.
- Past the last shadow cascade, the ground shades itself from a horizon map baked with each far chunk, so peaks cast shadow into far valleys at no cascade cost.

**The far forest:**
- Billboard cards, as `lush/far.rs` draws them, standing on the slope's height out to 2 km.
- Past that, the forest is the ground's colour and roughness, with a canopy normal, as Horizon and Ghost of Yōtei bake theirs. The tree line seen from across the valley is the band where that tint gives way.

**The air:** distances times 18 (D212) is wrong for real mountains. The scale is set by how open the view is and how big the world is, measured on the far-land band against the photographs (Morskie Oko to Rysy is about 2 km). It amends D299 when measured.

**Reflection** (D322): a planar reflection at a quarter of the screen. It draws only the cheap levels of detail (far ground, tree lumps and cards, the sky) and never grass, the floor or shadows, as Unreal's planar reflection takes an LOD factor and a screen percentage. Bevy's screen-space reflections need the deferred renderer, and ours is forward.

### (e) How an agent checks it

- **Field maps first:**
  - the tree line's height on each slope against the isotherm it should follow;
  - scree only at its angles;
  - every lake at its spill level;
  - snow by date and facing.
  Unit tests pin the outcomes, not tuned numbers: a lake never spills, and a colder place never grows taller trees.
- **Vantage views** (`--place lake`):
  - the shore at eye height;
  - across the lake to the far slope;
  - the forest edge at the tree line;
  - a scree fan;
  - the summit looking back down;
  - the valley from the pass.
  Each has look lines against `mountain-lake-day`, by band.
- **Films:** the shore route under the trees and out onto the gravel, and a climb from the lake to the tree line. Each is checked by `look.ts measure` and judged at milestones by the adversarial review against the game frames and the photo board.
- **`lodcheck`** on the across-the-lake and summit views: nothing pops as far chunks, far cards and the tint hand over.
- **`collide`:** scree boulders and krummholz stop a walker. A walker meeting the lake wades, then swims (D74), through `terra_core::Water`.
- **Bench scenes:** `lake` (the across-the-lake view, still) and `lake-walk` (the shore route and the climb), recorded with `--record`.

### (f) Order of work

Each step lands, passes `cargo xtask check` and is judged on its own.

1. **The reference:** frames, photos, `mountain-lake-day`, and a research note of what they show. Done: `mountain-lake-day` (Far Cry 5) and `morskie-oko-day` (photographs), by band ([research](../research/2026-10-15-mountain-lake-reference.md)).
2. **The field maps tool** and the latitude read from the camera. It shows what the start's climate puts at each height and latitude before anything is drawn. Done (D331): `cargo xtask fields`; at 40° the tree line is at 1,668 m and on 20 June the snow line at 2,400 m, so a lake at about 1,150 m has the forest 500 m above it, and peaks need to rise past 2,400 m, or the date to be earlier, to keep snow on more than their north faces.
3. **Mountain ground:** the mountain term, steep ground in the octree, rock, scree and gravel as ground layers; a `lake` place with its first views. Measured: chunks, meshing time, bench.
4. **Lakes:** basins filled by Priority-Flood, a water surface per lake, `terra_core::Water` implemented, wading and swimming in lakes.
5. **Water shading:** depth, shore band and foam, wind ripples, then the reduced reflection. Each is measured on the across-the-lake view.
6. **Niches and the tree line:** tree species' niches as data; trees stunted toward their limit; dwarf pine; placement on slopes. Needs climate step 3 (samples and `--date`).
7. **Far:** forest on far slopes, the ground's tint and canopy normal, the horizon-map shading, the air's scale measured by band.
8. **Snow:** from the folded past and the ground's shelter, on surfaces facing up (climate step 4's snow, reused).
9. **The demo:** its route, films, bench scenes, review. Then fix what the review finds.

## 2. A tundra

Cold, open, low country under a big sky.
- **The ground:** a quilt of pale lichen on dry rises, red-brown dwarf birch and crowberry in the lee, sedge tussocks and cotton grass on wet ground, and moss carpets round pools strung along a polygon net.
- **Its edge:** a few stunted spruces in islands.
- **Snow and wind:** patches of snow in the same hollows every year, and wind you can see in the grass, the cotton heads and blown snow.

It is the start's world further north, where the fields make it, about 11 km past the start on the 30 km world (D321). It is not a separate map. Above a mountain's tree line, its ground cover is the alpine meadow (§1).

### (a) Reference

- **Photographs first,** because the real tundra is the target and few games show it at our fidelity. From Wikimedia Commons: tundra lakes and ponds, alpine tundras, the forest-tundra (*Waldtundra*), Finnmark, Svalbard, Alaska.
- **Games for the reading of space and wind:** Red Dead Redemption 2's Grizzlies (1174180), Skyrim Special Edition (489830), The Long Dark (305620), Valheim (892970).
  - The Long Dark and Valheim are not our look (D14).
  - They are references for composition and motion, not for the look lines.
- **References to make:** `tundra-autumn-day` and, later, `tundra-winter-day`.

### (b) What exists and what is missing

| Part | Exists | Missing |
|---|---|---|
| Ground cover | `terra_world::cover` grass blades within 64 m and ground layers (litter, moss, needles, roots) by `CoverRules`; lichen and moss rock textures among the flora's materials | Lichen, moss carpet and sedge as ground layers; dwarf shrubs, tussocks and cotton grass as cover kinds and cards (`assets/data/foliage.ron`); the cover chosen by niche, not by noise |
| Climate | Fields, the folded past's snow, wetness and warmth per 32 m cell; wind as a mean with gusts and a prevailing direction | Climate step 3; the ground's aspect and shelter bending snowfall (D323); permafrost read from the long-run mean |
| Wind | One global wind (D255) in the grass, foliage and wind shaders, its direction a constant (`lush.rs`) | Its direction and strength from `terra_climate::Wind`; blown snow |
| Water | As §1 | Small pools, from the same basin filling at a finer grid |
| Trees | Norway spruce | Stunted spruce by niche (§1's rule); islands of trees at the forest's limit |

### (c) Parts and rules

**Where:**
- Tundra is where the warm season is too cold for trees: past the tree species' niches, by latitude or by height. The same rule as the mountain's tree line.
- The edge is a band of thinning, shrinking spruce islands, the forest-tundra, never a line.

**The ground cover:** five communities, chosen by two fields the world works out.

| Community | Where | What |
|---|---|---|
| Fellfield | Windward and exposed rises | Stones, cushion plants |
| Lichen heath | Dry ground | Pale *Cladonia* lichen |
| Dwarf-shrub heath | Moderate ground | Dwarf birch, crowberry, bilberry |
| Tussock sedge | Wet ground | Sedge tussocks, cotton grass |
| Moss carpet | Pool edges | Moss, *Sphagnum* |

- **Exposure:** Winstral's Sx, the steepest upwind slope within 100 m, against the prevailing wind (D323).
- **Wetness:** the folded past's wetness and how far a place is below its neighbours.
- **The season shows in each:** bilberry and dwarf birch turn crimson and rust in autumn, sedges go straw, lichen stays pale.

**Patterned ground:**
- Where the long-run mean temperature is below about −2 °C (permafrost), the ground takes a polygon net about 25 m across, under half a metre high, its edges as troughs.
- Basin filling at a finer grid makes pools along the troughs. Erratic boulders lie on it by noise.

**Snow patches:** the folded past's snow times the ground's catch (D323), so lee hollows and the foot of slopes keep patches into summer, in the same places every year, as real ones recur.

**Wind:**
- The global wind takes its direction and strength from `terra_climate::Wind`.
- Gusts run across the open ground as D255's fronts. Cotton heads and sedges stream.
- Blown snow lifts off patches above a speed, as cards (D325).

**Variants (D182):** five communities, by four seasons, by how exposed and how wet. About 40 distinct patches of tundra a player can tell apart, plus the pool fields, boulder fields, snowbed hollows and spruce islands that break them up. A new cold realm is new species in the same communities, not new code.

### (d) Performance

The cheapest of the three: there is no canopy. Its cost is ground cover to the horizon. Target 10 ms at p99, leaving room for weather later (a blizzard's snow and fog).

| Part | GPU ms |
|---|---|
| Cover within 60 m: shrubs and tussocks as instanced cards, sedges as blades; lichen and moss as ground layers with relief from the normal map, not geometry | 4.0 |
| Ground to the horizon, the communities as its tint past 60 m | 1.5 |
| Spruce islands and their far cards | 0.8 |
| Pools and snow patches (the lake water at small size; snow in the ground shader) | 0.7 |
| Blown snow | 0.4 |
| Sky, air, shadows | 1.5 |
| Post, interface | 1.0 |
| **Target p99** | **9.9** |

### (e) How an agent checks it

- **Field maps:**
  - the communities against exposure and wetness;
  - permafrost and polygons only below their temperature;
  - snow patches in the lee of the prevailing wind;
  - the forest-tundra edge a band, never a line.
  A test checks that a snow patch moves with the prevailing wind's direction.
- **Vantage views** (`--place tundra`): a lichen rise; a pool field; a snowbed hollow; the forest-tundra edge; the open view to the horizon; a low sun.
- **Films:** a walk across the communities into the wind and back with it. Frame differences measure the motion: the grass must move.
- **`look.ts`** against `tundra-autumn-day` by band.
- **The bare-ground measure** from [biomes.md](biomes.md#how-wed-know-d69): nothing bare that does not mean something (a stone, a pool, a patch of snow).
- **`lodcheck`** as the cover hands over to the ground's tint at 60 m, the hardest seam in an open view.
- **`collide`:** boulders stop a walker; pools wade.
- **Bench scene:** `tundra-walk`.

### (f) Order of work

1. **The reference:** photos and game frames, `tundra-autumn-day`, a note.
2. **The place, from the field maps** (§1's step 2): the tundra and its edge where the start's climate puts them. If the climate puts them somewhere useless, the theme's numbers are wrong, and that is found here.
3. **The ground bends the climate** (D323): aspect and snow catch in `terra_climate::Place`, with tests; the snow patches on the field maps.
4. **The wind from the climate:** the global wind's direction and strength from `terra_climate::Wind`. It changes every place's look, so the lush forest's checks run too.
5. **The cover:** the five communities' ground layers, then their shrubs, tussocks and cotton grass as cards, chosen by niche.
6. **Patterned ground and pools:** the polygon net, and §1's basin filling at a finer grid. Needs §1's step 4.
7. **The forest-tundra edge:** stunted spruce islands (§1's step 6 rule).
8. **Blown snow** (D325), then **the demo:** route, films, bench, review.

## 3. A living town

A market town on a gentle slope, about 30 to 40 buildings round a market square and two or three streets, from the building system as a player's base is (D108, D229). Smoke rises from chimneys and thatch. On market morning:
- people set out stalls, buy, carry and talk;
- a smith works, children run, a cart comes in;
- chickens scratch in the yards, dogs trot after their owners, goats are penned behind the houses.
By evening the square empties into lit doorways. A player can watch any one person and see a day that makes sense.

### (a) Reference

- **Games:**
  - Kingdom Come: Deliverance II (1771300), whose Bohemian towns of timber, wattle, thatch and shakes are our own materials (D236);
  - Kingdom Come: Deliverance (379430);
  - The Witcher 3 (292030);
  - Mount & Blade II: Bannerlord (261550).
  Keep frames of a square or street by day, with people.
- **Photographs:** Commons medieval markets and open-air museum villages, for the materials and the ground of a lived-in place (mud, straw, worn paths).
- **References to make:** `town-market-day` for the look, plus counted bustle. The review counts, in each reference frame, the people within 30 m, the animals and the smoke columns. The town's frames must reach those counts at the same kind of view.

### (b) What exists and what is missing

| Part | Exists | Missing |
|---|---|---|
| Buildings | `terra_build::village`: about 300 buildings a player can tell apart (8 plans, 9 schemes, 3 roofs, 3 widths); the street village (D229), `town` on a grid of plots, `hamlets`; drawn to the horizon (D278); bench scenes `village` and `town300` | A town layout with a square, streets along the slope and plots that follow the ground; chimneys; furnished interiors in villages |
| People | `play/people.rs` `Person`, `Doing { Bound, Following, Working, Standing, Sitting }`; four villagers who stand or sit; one base body of 73 joints, five templates, garments; 25 clips including walk, jog and sprint (D316), talk, sit, kneel, pick up | Days and schedules; homes and work places; walking a town; looks from a seed; character and animation LOD (characters.md phase 6) |
| Navigation | Bodies pushed out of each other and out of `Obstacles` (`play/bodies.rs`), a stand-in until collision is designed (D53) | Any path finding or avoidance; no navigation crate supports Bevy 0.20 yet |
| Animals | Deer and spiders (`assets/data/creatures.ron`), with idle, graze, wander and flee states; simulation by distance (D145) | Domestic animals: models, rigs, clips, pens, following an owner |
| Market | `MarketStall`, `Well`, `Stool` props (D304) | Goods on stalls; carts, barrels, fences, troughs, signs, lanterns |
| Smoke | None: no particle system; the blood spray is CPU cards (`play/hits.rs`) | Chimney smoke, cooking fires, a forge's sparks |
| Sound | Spatial voices by bus; `Ambient` sources in earshot; footsteps for anything walking; the bell; 88 Kenney CC0 clips | Murmur of a crowd, market calls, animals, cart wheels; any cap on how many voices play |
| Measured | `crowd` experiment: 100 dressed characters, 5.8 ms on the RTX 5090, bound by the CPU's animation and transforms | Any town or crowd number on the RTX 3080 |

### (c) Parts and rules

**The layout:**
- An extension of `terra_build::village`, as Emilien et al. lay villages and Ghost Recon Wildlands adapted them.
- A square where the roads meet on the flattest ground. Streets out of it along the contours. Plots divided along each street, the buildings' doors to it. Yards, sheds and pens behind.
- A town is a square's shape (long, triangular, a widened street) by its roads and slope, out of the 300 buildings. That makes dozens of towns a player can tell apart, by data in `settlement.ron`.

**People's days** (D324):
- **A day is a schedule of activities by role, in time windows.** Sleep, eat, work, market, gather, wander, walk home. This is Skyrim's packages and Stardew Valley's schedules.
- **Activities happen at smart objects:**
  - Things in the town advertise what can be done at them, where to stand and which clip to play: a stall, the well, a forge, a chopping block, a bench, a doorway, a hay pile.
  - Each person picks among the spots free for their activity, as The Sims' objects advertise and The Witcher 3's NPCs pool spots.
  - A new activity is data: an object kind and its clips.
- **Roles for the demo:** about ten, each a day and the tools and clothes that show it. Trader, smith, baker, farmer, carter, guard, an elder, children, a woodcutter, a washer.
- **A day's clock is fast against a walk** (a day is 30 minutes of play): crossing a town of 200 m takes one or two of its hours. So every window is long enough to walk to and stay at, people plan the next a few hours ahead and set off early enough for the walk, a window too short for the walks either side of it is passed over, and the place planned is kept for them.
- **Walking:**
  - Along the town's own path graph: street centre lines, plot paths and doorways, which the layout knows exactly.
  - No way runs past where someone stands at a spot, so walkers go round the seller and the talkers, not between them and their stall or well.
  - A body's walk plays at the speed it goes (slower than the clip's own, its stride slowed, not slid), and below a shuffle it stands.
  - A* on it, then ORCA local avoidance between walkers, ported from RVO2 (Apache-2.0).
  - It needs no navmesh, which nothing supports on Bevy 0.20, and stays cheap on a server (D52).
  - A navmesh (Recast's design, zlib) comes with collision (D53) for the wild.
- **Simulation by distance** (D145, extended to people):
  - Full within 64 m.
  - Thinking less often to 256 m.
  - Beyond that, nobody ticks: where a person is, and doing what, is a function of the clock and their schedule, as plants are of their cell (D47, D313). Kingdom Come keeps routines alive this way at its AI level of detail.

**Drawn in tiers** (D324):

| Tier | Distance | Count | How |
|---|---|---|---|
| Full | Within 25 m | 24 | Every joint, blended clips every frame |
| Reduced | 25 to 80 m | 40 | Animation at a half to a quarter of the rate, interpolated, as Unreal's budget allocator does; fingers and face dropped; the outfit merged into one mesh |
| Far | 80 to 200 m | ~100 | One instanced mesh per look, posed from a vertex animation texture of walk, idle and work |
| Beyond | — | — | The smoke and the sound carry the town |

**Animals:**
- Creatures as the deer are (D145), with two new behaviours: penned (grazing and wandering inside a yard's fence) and following (a dog after its owner).
- Chickens as many cheap bodies on the far tier's vertex animation, always.

**Smoke** (D325):
- Instanced soft cards on our own material, drifting with the climate's wind and thinning with height.
- A chimney's column, a cooking fire, a forge's sparks.
- A house with a hearth in use smokes, by the time of day.

**Sound:**
- **A murmur bed for the crowd**, after The Witcher 3: the town round the listener in four quadrants, each counting people near and what they are doing, setting the murmur's level and mix.
- **Spot sounds from activities:** a hammer at the forge, a bucket at the well, calls at the stalls, hens, a bark, cart wheels.
- **A cap on voices** (32), the quietest at the listener dropped first.
- **Assets:** new recordings, CC0 or generated (D107).

**Variants (D182):**
- **Buildings:** about 300.
- **Towns:** a few dozen layouts.
- **People:** about ten roles, by five templates and their morphs, by garments and their colours. A few hundred people a player can tell apart, mostly by what they do and wear, then by face and build.
- **Animals:** about four kinds, a few looks each.

### (d) Performance

The town is bound by the CPU (the crowd measured so) as much as the GPU, so both are budgeted.

| Part | GPU ms | Main-thread CPU ms |
|---|---|---|
| Buildings: the town near and middle, the world's far | 3.5 | 1.0 |
| Ground, cover and the trees round the town | 3.0 | 1.0 |
| People: 24 full, 40 reduced, ~100 far | 1.5 | 3.0 |
| Animals: about 25 | 0.4 | 0.5 |
| Smoke and fire | 0.4 | 0.1 |
| Hearth and lantern lights at dusk, at most 16 near, unshadowed | 0.8 | — |
| Sky, air, shadows | 1.5 | — |
| Post, interface | 1.0 | — |
| Sound | — | 0.3 |
| The world, the walker, the rest | — | 3.0 |
| Headroom | 1.2 | 4.4 |
| **p99** | **13.3** | **13.3** |

- **The counts the budget allows**, if the RTX 3080 measures as the RTX 5090's crowd suggests:
  - 60 people out and about in view (24 full, 40 reduced), about 150 living in the town in all;
  - about 25 animals: 12 chickens, 3 dogs, 6 goats, an ox or two with a cart.
- **Step 1 measures** the village, `town300` and `crowd` on the RTX 3080. The counts are then moved to what it shows, never the budget.

### (e) How an agent checks it

- **`cargo xtask life`** (the tool above): every person through a game day.
  - **It fails on:** anyone stuck for more than 5 s, through a wall or another body, sliding their feet, or more than a few minutes late for what their schedule says.
  - **It reports:** people, animals and smoke columns in view at each vantage and hour, against the counts taken from the reference; and what was heard, by bus.
- **Vantage views** (`--place town`): the square at 10:00, at 13:00 and at dusk; a street; a yard with its animals; the forge; the town from the hill above.
- **Films:** a walk from the edge into the square on market morning, and one person followed through an hour.
- **`lodcheck`** on people's and animals' tiers, as for buildings: nobody pops as they change tier.
- **`artifacts`** on the town's buildings, as for the village.
- **`collide`:** stalls, carts and fences stop a walker.
- **Bench scenes:**
  - `town-day`: a walk into the square at 10:00, with the crowd;
  - `town-dusk`: the lights;
  - `town-bare`: the same without people or animals, so their share is measured, as `village-bare` measures buildings.

### (f) Order of work

1. **The reference** and the bustle counts; then the village, `town300` and `crowd` measured on the RTX 3080, which sets the counts.
2. **The layout:** a market town with a square, streets along the slope, plots and yards, in `terra_build`, with its path graph. Bench `town-bare`. *Landed: `terra_build::town` (D330); `cargo xtask town --seed N` draws the plan. The bench waits for the game to lay it.*
3. **Walking:** A* on the path graph and ORCA avoidance. Villagers walk between given points. The first `life` check (stuck, through walls, feet).
4. **Days:** smart objects (stall, well, forge, block, bench, door) and schedules by role as data. People far off worked out from the clock. `life` checks schedules.

   *Steps 3 and 4 landed together in `terra_life` with `assets/data/life.ron`; `cargo xtask life --seeds 1-8 --ground all` runs 24 towns of 120 to 180 people through a day in about three seconds each, a step under a millisecond. Most pass; a few have a child or two held more than 5 s in the market's crowd, which is what is left to do there.*
5. **Crowd tiers:** animation rate and joint LOD, merged outfits, the far tier's vertex animation; `lodcheck` on them; the counts raised to the budget.

   *In the game (`play::town`, `play::townsfolk`): the town laid on the most level ground near where it is wanted, everyone of its life a body that is drawn only out of doors and near a viewer; a body's pose worked out every frame within 25 m, every second frame to 50 m, every third to 80 m and every sixth beyond, its clip played that much faster, and not at all while it is indoors, as Unreal's budget allocator lowers a far character's rate. Fewer joints, merged outfits and the far tier's vertex animation are still to do, and wait for the numbers.*
6. **Looks from a seed:** templates, morphs and garments by role.

   *Built: `assets/data/townsfolk.ron` gives each role its bodies (a template and a little of another's morphs) and what it wears, each garment dyed one of the role's colours and a shade of its own; children are two thirds of a grown body's height. The wardrobe is the game's five garments (coat, trousers, two packs, a quiver), so a role reads mostly by its colours and what it carries: aprons, dresses, hoods and tunics are the character pipeline's to make.*
7. **Animals:** chickens, a dog and a goat from Tripo, rigged and animated as the 2026-10-14 animation note proposes; penned and following.

   *Built in `terra_life::animals` from data (`animals` in `life.ron`): hens loose in the hen yards, goats in the pens, dogs at their owners' heels along the way they walked, or lying by the door while they are in; drawn by `play::town_animals` for each kind that has a model in `townsfolk.ron`. The models are Tripo's to make, on credits the developers approve over about a hundred.*
8. **Smoke** (D325), chimneys on the buildings, goods on the stalls, the props a town needs.

   *Smoke is built (`terra_world::smoke`, `shaders/smoke.wesl`): each emitter a mesh of cards whose places the GPU works out from each card's phase, the time and the climate's wind; a home's hearth smokes through its roof's ridge, thickly at the morning's and evening's cooking while anyone is home, the forge while its smith works. A home's firelight shows within at dusk, the sixteen nearest the eye lit. The streets, the square and the doors' thresholds are trodden to bare earth and the yards worn thin, by the town's own ground cover (`TownCover`), grass growing elsewhere as the planet has it. Chimney stacks, a trough's model, goods beyond the stall's own, carts and barrels are still to come.*
9. **Sound:** the murmur bed, spot sounds, the voice cap.

   *The murmur is built (`Sound::Murmur`, a dozen voiced syllables shaped by moving formants, no word in it; `play::town_sound` sets it by how many are out near the listener), measured into the world's band of the mix. Spot sounds and the cap on voices are still to do.*
10. **The demo:** route, films, bench scenes, review.

## Open questions for the developers

Each has a recommendation. The tools and the engine's parts (field maps, mountain ground, lakes, water, the climate bent by the ground) can go ahead; what each place holds waits on these answers (D49).

1. **How tall are the mountains?** Recommended: true to life, about 1.1 km from lake to summit as at Morskie Oko, with the tree line about 500 m above the lake. Compressed mountains would need the climate's lapse rate bent to keep the tree line, and the climate would then be wrong everywhere else (D122).
2. **Which season and hour is each demo?**
   - Lake: early summer by day, when the peaks still hold the winter's snow.
   - Tundra: early autumn (the crimson and rust of dwarf shrubs, the first snow on the rises), then winter.
   - Town: market morning in late summer, then dusk.
   Each comes from the date, so the others follow later at no cost.
3. **Where is the tundra?** Recommended: the starting world's own north, where its climate puts it, about 11 km from the start, which proves it is the same systems. Other choices are only above the mountains' tree line, or a cold realm of its own. A cold realm comes later as a theme.
4. **Which trees at the limit?** Recommended: the lush forest's Norway spruce and Scots pine, stunted toward their limit, plus two new kinds: dwarf mountain pine for krummholz, and dwarf birch for the tundra. No larch for now.
5. **What water colour for the first lake?** Recommended: by rule from what lies above it (D322). The demo's lake is clear and dark green over rock, as Morskie Oko is. Turquoise glacial water comes from a lake with snow and ice in its catchment.
6. **What kind of town?** Recommended: a market town of 30 to 40 buildings round a square on a slope, from the building system as it stands: no walls, no church or castle yet, no new materials (D236).
7. **Who lives there, and what do they do?** Recommended: about ten roles (trader, smith, baker, farmer, carter, guard, an elder, children, a woodcutter, a washer), each with a day of work, market, meals and home. Talk stays the short lines people.md has until its question 3 is answered.
8. **Which animals?** Recommended: chickens, dogs, goats, and an ox with a cart. Horses come with mounts (D100).
9. **Chimneys?** Recommended: stone chimney stacks on stone and hewn houses; smoke seeping through the ridge of thatch, as medieval houses without chimneys did.
10. **What do the crowd's voices say?** Recommended: a murmur with no words for the demo, and calls by tone. Speech, and in what tongue ([naming.md](naming.md)), is audio.md's open question 4.

## Decisions

- **D321:** the three demos as places, made by the world's rules (Leaning, the developers').
- **D322:** lakes, water and the ground's mountain term.
- **D323:** the ground bends the climate (aspect and snow catch).
- **D324:** town life (schedules at smart objects, a path graph, people by distance, crowd tiers).
- **D325:** smoke and blown snow as instanced cards.
- **D326:** the demos' checks: field maps, one table of places for the sandbox tools, look by bands, the life check.
