# Dungeons by rule

Status: Leaning, technical (D473, D474, D483, D484, by the agent); the generator, its sheet and the stamp are built in `terra_dungeon`, and the sandbox lays one in the forest (`--experiment dungeon`, `dungeon-mine`, `dungeon-keep`). What a dungeon is for in the game is a design question for the developers, listed at the end and not decided here. A dungeon is a place dug into its realm, as in Valheim and Enshrouded (D497), one copy shared by the whole server (D498) that stays cleared (D499), with no building and rock that cannot be dug (D502), and doors pushed open by walking into them, a locked one once its key is held (D512); [instances.md](instances.md) says how a realm is loaded in and out; this doc says how a dungeon is made.

## Constraints

- Hundreds of variants a player tells apart, not millions (D182): a theme (crypt, mine, keep) chooses parts; a seed chooses among them by rules.
- A new theme is data, not code (D136); rules in a plain crate (D28), the same on every machine (D41).
- Laid in building's half-metre cubes (D181), dug into its realm's terrain (D497).
- Checkable without a person (D69): every promise a test, the whole a sheet of numbers and maps.

## The stages

A dungeon is a theme and a seed, made in four stages, each its own module, each tested alone (`cargo test -p terra_dungeon`).

### 1. The mission graph (`graph.rs`, D473)

Rooms by role and how they are linked, before any has a place (Dormans' mission graphs, Unexplored's cyclic generation).

- **Roles**: entrance, passage, branch (the loop), guard, goal, puzzle (holds a key), shortcut, secret, rest, treasure.
- **Links**: an open arch, a door, a locked door (opens when every key of its lock is held), a one-way door (barred on one side), a secret way.
- **Shape**: a main path of 5 to 7 rooms, the edge into the guard room locked; a loop of 1 or 2 rooms from the main path back to it, no shorter than the main path between, so depth keeps rising; the key rooms as dead ends off the loop, so the loop must be walked (three braziers are three keys of one lock); a shortcut from beside the gate back to the first passage, one way; a secret room; a rest room part way down; treasure with the rest of the theme's room budget.
- **Difficulty**: depth by breadth-first search; danger from 0 at the entrance to 1 at the goal, the guard a step up. A room's mob groups come from the theme's bestiary and its loot from the theme's tables, by role and danger (none at the entrance or the rest room).
- **Promises** (tested over 1,000 seeds a theme): solvable (the goal and every key reached, picking keys up on the way); no key behind its own lock; the goal out of reach with the lock shut and in reach without secrets; size within the theme's bounds; one entrance, goal and guard, as many puzzles as keys, a treasure; a loop (edges at least rooms); danger rising along the main path; only the theme's creatures and items.

### 2. Placement (`place.rs`, `route.rs`, D473)

- **The grid** (D527): cells 4 m square (eight building cubes; a cube of wall each side leaves a 3 m corridor) and levels 6 m apart (a slab and 5.5 m clear), the ceiling brought down to the theme's headroom (`headroom`: the crypt's and the keep's corridors 4.5 m and rooms 5 m, the mine's drifts 3.5 m and stopes 5 m; flat cubes, hidden in the crypt by the barrow kit's vaults springing at 2.95 m). Doorways are the corridor's width and 3 m high, each leaf 1.5 m. A cell is `IVec3`: x east, y the level counted down, z south. The entrance is 2 by 2 at the origin with a stair up to the surface north of it.
- **Rooms** in the graph's order, each beside the room it hangs from: the best of 30 tries (a side, a gap of 2 to 4 cells, 5 to 7 with a level change) by closeness to the placed rooms it links to, a cell clear of everything. Size by role (goal 4 by 4, guard and puzzle 3 by 3, rest 2 by 3, others 2 to 3). Level from danger, at most one from the parent's.
- **Corridors** by A* from any free cell beside one room to one beside the other, shortest edges first, a turn dearer than a step; a stair is one move three cells along and a level up or down, taking two cells on both levels (the run below, its well and landing above). A placement that cannot route an edge starts again from a derived seed, up to 12 times.
- **Sides**: every cell knows its space (a room, a corridor by edge, a stair) and each side: a wall, open into the same space, a door (by edge) or a stair's way.
- **Promises** (tested over 1,000 seeds): sides agree from both cells; the cells add up to the rooms', corridors' and stairs' (nothing overlaps); only what the graph joins is joined; each edge's rooms are joined through its own cells; every cell is reached from the entrance.

### 3. Dressing (`dress.rs`, D474)

Wave function collapse (Gumin's simple tiled model, as Townscaper and Bad North use it) over the theme's modules, a module turned to each facing a variant.

- **Sockets** on each side: `open`, `wall`, `door`, `stair`, `rubble`, `water` and the theme's own (D483). A wall, door or stair side must take what placement put there; an open side must share an interior socket (every socket but wall, door and stair) with the module across. `any` is every socket but water and rubble.
- **Rules by socket** (D483): a socket few modules take is structure. Pillars take `aisle` along their row, so a row runs wall to wall; loculi take `alcove` beside them and end at a corner, door or end piece; decorated walls alternate two beats (`ra`, `rb`: a sconce or relief, then a plain bay); runners and rails (`runner`, `rail`) run unbroken from a door, stair or feature to another or to an end piece.
- **Held as given**: the goal's dais, each key room's altar, lever or brazier, the rest room's fire (the cell of its room with the most open sides and no door) and every stair cell take their module in any facing that fits.
- **Water** meets dry floor only at a `shore`, steps down from its dry side; a pool takes no stair, so a flooded gallery has a shore before its stair.
- **Rubble**: a collapse (blocks its cell) sits in a room's corner and spills rubble into the cells beside it.
- **Solver**: the cell with fewest variants left, a pick by weight, arc consistency over open sides; a contradiction, or a dressing that cuts a cell off from the entrance, starts again from a derived seed, up to 8 times; then the theme's plain floors, which always solve.
- **Promises** (tested over 1,000 seeds): every cell filled with a variant its sides take; every open side's two modules share a socket; water ends only at a shore; every cell not blocked and every held feature reached from the entrance; the solve rate without the fallback above a bound.

### 4. Themes as data (`theme.rs`, D136)

A file a theme in `assets/data/dungeons/`: its goal in a sentence, rooms, levels, keys, building kinds for walls and floors, the held features, bestiary, loot and modules. The three now are working assumptions, not designed:

| Theme | Goal | Rooms, levels | Feel |
|---|---|---|---|
| Crypt, "The Ossuary Gate" | take the relic from behind the locked gate; the key in the ossuary | 11-15, 2 | greystone, loculi, sarcophagi, pillars, collapses |
| Flooded mine, "Deep Seam" | reach the trapped miner; the pump room's lever drains the gate | 12-16, 3 | earth, timbering, flooded galleries, rails, cave-ins |
| Ruined keep, "Three Braziers" | light three braziers to open the sanctum | 13-17, 2 | limewash, banners, weapon racks, collapses |

Spider is the only hostile creature in play; Drillhog, Worman and Seawish join a bestiary once rigged. Keys, the relic, levers and braziers are dungeon objects, not inventory items.

## The sheet

`cargo run --release -p terra_dungeon --bin dungeons -- --seeds 1000 --maps <dir>`: a line a theme (solvable, placed and tries, dressed without the fallback and tries, connected, verified, median time a stage, room and cell counts, variety: the Jaccard distance between neighbouring seeds' cells and the entropy of module use) and a PNG a theme of its first 8 dungeons, a column a level, rooms tinted by role, the graph's nodes and links over the grid, mobs and loot as pips.

## The stamp and the run-in (`stamp/`, D484)

What the world lays, as plain data in a local frame (metres, x east, y up, z south; the origin the roof's top at the entrance cell's north-west corner; level L's floor top at -4(L+1) m).

- **Digs**: a terrain box a level, one under its basins and shores and one up the entrance well, each to 0.03 m under the floor's top (creatures stand on the ground, not on cubes).
- **Shell** in the theme's wall, floor and trim kinds: floor and ceiling slabs, a wall a cube thick where placement put one, doorways and stair ways as `Fill::Open` cubes so each space stays a closed room to building's light, stairs of 8 risers over 6 m, a parapet and lintel at the surface.
- **Modules** to cubes and props by words in their names (pillar, alcove, rubble, basin, shore, runner, decor, ...); an unknown name is plain floor. A pillar row's pillars stand at their cells' middles, the row's end one against its end wall as a respond, none in front of a doorway (a middle one left a metre to the wall on 4 m cells, and the tour squeezed through it). A corridor walled both sides gets a trim rib across its ceiling every cell and its edges worn (a half cube or two at a wall's foot, now and then a stone).
- **Run-in**: the graph's mobs at free spots away from doors, a chest a room with its loot, the goal object on its plinth between two fire bowls on stone pillars, so it is the brightest thing in its room, and the keys at their features (dungeon objects, not items), studded leaves at locked and one-way doors, each with its lock (a key, or the side a one-way door is barred on), up to 30 torches and braziers (a room's first, then the end walls a corridor's straight legs run into, so no vista ends in a black wall, then a second at the far end of a room of six cells or more, then a corridor's every third cell; past the cap, the rooms' own torches it cut and a torch on a side wall of each stair no torch lights yet, 2 m over its lower cell's step, so no stair is a black hole), the start on the surface and a tour of floor points to the goal by the longest way that enters no room twice, so it walks the loop and never turns round in a dead end (a key room is seen from its doorway), short of a room's middle where its way out is beside its way in, and at the corner where they cross where its doorways are on walls at right angles (round 7 of the barrow: a look swung 25° toward the middle, then 115° back out).
- **Furnishing** (`stamp/furnish.rs`): after the run-in, a room is dressed by what it is for, as a level designer sets out a guard room with its fallen and a treasury with its grave goods: the theme's `dressing` names, by room role, a room type's hero prop, wall props, clutter and vignette ([room types](#room-types-and-their-dressing-d539-open-a-proposal-for-the-developers)), so many per four cells, laid against the walls of cells whose module leaves them bare, 1.3 m off the tour, 1.6 m from a doorway and 1 m from anything standing (tested over 40 seeds: 8 rooms in 10 dressed, none within 0.9 m of the eye).
- **Promises** (tested over 200 seeds): a walker stepping up a cube and down two reaches every object, chest, mob and fire from the start; air never leaves its space but up the entrance well; every model exists.

### Room types and their dressing (D539, Open: a proposal for the developers)

A crypt makes sense when each room reads as a place something was done in, and something then happened in: environmental storytelling (Smith and Worch, GDC 2010, "What Happened Here?"), set dressing laid over a modular kit as Skyrim's Nordic ruins lay their clutter (Burgess and Purkeypile, GDC 2013), Diablo IV's tile sets dressed by props, interactives and light (its 2022 environment update). The shipped crypts share their room types: Skyrim's burial halls of urns and draugr in alcoves, its guard posts and robbed tombs; Elden Ring's catacombs of ossuary walls and grave candles; The Witcher 3's crypts of sarcophagi, cobwebs and looters' leavings. Their terms: a **hero prop**, the room's one focal piece; **wall props** in rhythm along its walls; **floor clutter**, small things to find in its corners; a **vignette**, a few props laid together that tell a moment.

| Role | Room type | Hero (one) | Wall props | Clutter | Vignette |
|---|---|---|---|---|---|
| Entrance | antechamber, where mourners left offerings | offering altar | urns, candles | candles, stone | - |
| Passage | ossuary passage | a wooden coffin | bones, urns, shrouded dead | stone, candles | - |
| Branch | robbed tomb | its coffin, lid pushed aside | broken jars, fallen stone | plates, bowls | the looter's leavings: a chest, spilled bowls, a lantern |
| Guard | guard post | the guards' bench | the fallen, their axes, a lantern | bones, stone | the last stand: a shroud, shields leant against the wall, axes laid down |
| Puzzle | ossuary (holds the key) | its altar (the feature) | heaped bones, shrouds, urns | candles, bones | - |
| Treasure, Secret | treasury of grave goods | a chest | jars, plates, bowls, urns, chests | brass, plates | an offering: urns about a chest |
| Shortcut | chapel | an altar | candles, a shelf of the dead's records, urns | candles | - |
| Goal | the lord's burial chamber | the relic on its dais (the feature) | urns, jars, brass, candles | candles | - |
| Rest | camp of earlier delvers | the campfire (the feature) | a barrel, a log, buckets | bowls, tools | where one slept: a bedroll, its blanket folded, a pack, a lantern |

The town's goblets read as clear glass and its stew pot and sack as glossy white tureens and skulls under torchlight (barrow dress round 2's reviewers), so the crypt lays plates, bowls and brass and the camp a barrel, tools and a folded bedroll in their place.

Rules (`stamp/furnish.rs`, data in the theme's `dressing`):
- **Hero**: one a room, on the bare wall spot farthest from its doorways, so it faces the way in; a room with a feature (the goal's dais, the key's altar, the rest's fire) has that as its hero. It keeps 1.6 m clear along its wall.
- **Lamp**: where no light stands within 3 m of the hero, the type's `lamp` (lit candles) is laid beside it with a small unshadowed light of its own, as games light a focal prop from a source in the fiction (346 heroes over 40 seeds: 146 lit before, 344 after). A room's feature (a tomb, the key's altar) takes one too, where no fire or lamp stands within 3 m: a torch hung over a tomb lights its wall, not its face (139 features: none lit before, 138 after).
- **Wall props**: so many per four cells, against the bare walls a metre apart, in two groups as set dressers lay them: nearest the hero (or the vignette) and nearest the wall spot farthest from it, the walls between bare.
- **Clutter**: so many a cell, a piece round each prop the room's dressing laid in turn, so each group takes its share; then the corners and the wall spots left. Tested: 5.4 props a group (3.75 before), 93% in groups of three or more, 2% alone.
- **Scatter**: last, a type's `scatter` (fallen stones and their grit, broken bones) `strewn` so many a cell over the open floor the groups leave bare, each 1.2 m from all else and nudged off the floor's grid, 1.65 m off the tour. Tested over 40 seeds: of 530 rooms of four cells or more, 152 had anything 2 m in from their walls before, 517 after, two pieces a cell at most.
- **Vignette**: its props laid by their offsets round one wall spot; where none fits all of it, drawn closer, then cut short from its end (its first piece and one more kept), then the same on the open floor facing the nearest doorway, so every room of its type tells it (laid all or none, the crypt's guard post's, 2.2 m wide, dropped in 3 of 40).
- **Paths clear**: everything 1.3 m off the tour, 1.6 m from a doorway's middle and apart from what stands (1 m, clutter 0.6 m), so no prop is ever in the way or the camera.

## In the sandbox

`cargo xtask sandbox --experiment dungeon` (also `dungeon-mine`, `dungeon-keep`; theme and seed in `assets/data/dungeon_demo.ron`) digs the most level site near the start, lays the stamp as one building with footings off and spawns the rest; the walker starts on the grass north of the stair. The run's log gives the tour as `--route` points and each leg as a `--vantage`. It also logs the film camera's eye along the tour (`Dungeon view i`, kept off walls) and where it looks from each (`Dungeon look i`): at the way 4 m on (tilted 10° up under a kit's vaults so they read), turned toward the next doorway on the way once it is in sight within 8-12 m, drawn to the goal or a key once near and in sight while the way on stays in frame, turning at most 20° a metre, 60° while lagging past 0.5 rad off the way on, as a composer's hard limit keeps its target in frame, pitched at most 30°, holding the way on where it comes back to the eye at a dead end; and the dolly's pace (`Dungeon pace i`, seconds to the next eye): 2.5 m/s, slowed so the look turns at most 26° a second (round 7 of the barrow saw 45° as a swing), eased (stamp/look.rs).

### Doors (D512, `door.rs`)

A door stands shut until a body walks into it, then its leaves swing away from that body, a quarter turn in half a second, and stay open (into the room each folds flat on its wall, or lies on a side wall flush with its jamb, or stands clear where neither runs; only where the room is solid just inside the jamb does a leaf turn out of the room however it is pushed, 0.6% of leaves, tested in `leaf_tests.rs`): a body pushes when it is across the doorway, within 1.5 m in front of the leaves and walking at them faster than 0.3 m/s, so they are moving before a walker or a camera at a walk reaches them. A locked door pushes only while every key of its lock is held; a one-way door only from its free side, and once open it is open for both. Tested in `door.rs` without a GPU. The sandbox pushes with every player and camera (a film's camera walks the tour as a body would), and holds every key until taking keys is built, so its gate opens as any door.

## Next

- **Light underground**: a room's probe now wins over the forest's sky volume (D485) and holds what its openings let in (D486), but Bevy keeps only the 8 room probes nearest the camera, so farther boxes of the crypt's 168 still take the sky; `node tools/sandbox/indoorlight.ts` (a continuous check) fails until they don't.
- **Taking keys**, so a locked gate stays shut in the demo until its keys are held (D512); a door's leaves as bodies the mover collides with; chests that hold their loot as storage, water surfaces in basins.

## Themes (D527)

Status: Decided (D527): the developers on the desk (q-dungeon-themes), "Yes, all four, bigger grid", and on the door film, "this dungeon feels too cramped, it needs to be bigger". The kits are not built yet; the bigger grid is the first step. The developer asked on the desk (r-dungeons) for dungeons "more spacious and some of them be cavernous". [research/2026-10-26-dungeon-themes.md](../research/2026-10-26-dungeon-themes.md) has the comparison with Skyrim, The Witcher 3, Elden Ring, Valheim, Enshrouded, Diablo IV and Baldur's Gate 3. Their grids are about twice ours. They build caves apart from halls. They share a few themes, each a kit reused across many dungeons. Round 26's spike showed that cubes cannot fake vaults ([research/2026-10-25-dungeon-vaults.md](../research/2026-10-25-dungeon-vaults.md)). So the built themes need pieces modelled for them, and the natural ones need carved rock.

**More spacious, for every built theme.** The cell grows from 3 m to 4 m: a 3 m corridor between walls a cube thick. Levels go from 4 m apart to 6 m, a slab and 5.5 m clear, so a vault springs at 2.5-3 m and its crown is at 4.5-5 m. A hall spans 2-4 cells, 8-16 m, its vaults carried on piers. The goal room may take two levels' height. The third-person camera, 3-4 m behind the body, then has room to follow without brushing walls.

| Theme | Like | Scale | Architecture | Materials and props | Built from |
|---|---|---|---|---|---|
| **Barrow halls** (today's crypt; the keep's undercroft a dressing of it) | Skyrim's Nordic ruins, Elden Ring's catacombs, Valheim's burial chambers | 4 m cells, 3 m corridors under barrel vaults crowned at 4.5 m; halls 8-16 m on piers, groin vaults at crossings, up to 8-10 m | round or pointed arches on imposts, transverse ribs a cell apart, loculi 0.5-0.6 m deep in rows, an ossuary wall, a raised sarcophagus hall | rubble walls with ashlar for arches, piers and quoins (a second stone); bones, skulls, shrouded bodies, urns, candles, cobwebs; for the keep: limewash, timber-beamed ceilings, banners, racks | building's cubes for walls and floors, and a **dressed kit** laid over them as a roof covering lies over its cubes |
| **Hewn mine** (today's flooded mine) | Skyrim's mines, Elden Ring's tunnels, Valheim's infested mines | drifts 2.5-3 m wide and high, tight on purpose; opening into stopes and galleries 10-20 m across and shafts 2-3 levels deep | hewn rock (tool-marked, not cubes), timber sets every 1.5-2 m (two posts and a cap), cribbing, ladders and a winch at shafts | the realm's rock and its ore veins; timber, rails, carts, lamps, flooded workings | **carved voxel terrain** (digs) for drifts and stopes, a **timber kit** standing in them |
| **Natural caverns** (cavernous) | Skyrim's caves and ice caves, Elden Ring's caves, Valheim's troll and frost caves, Deep Rock Galactic | passages 3-6 m wide, winding and rising; chambers 15-40 m across and 8-20 m high; the goal a great cavern 50 m and more, its roof in the dark | no masonry: flowstone, ledges, rock bridges, a stream or pool, a shaft of daylight; at most a few built pieces (a stair cut in rock, a bridge) | the realm's rock and its strata; stalactites and stalagmites, crystals or glowing fungi as the light, roots, bones of what lairs there | **carved voxel terrain**: chambers as intersecting spheres and planes crumpled by noise (Deep Rock Galactic's rooms), passages as noisy tubes along the routed way; rock props, no cubes |
| **Ancient engineered halls** | Skyrim's Dwemer ruins, The Witcher 3's elven ruins, Enshrouded's vaults | monumental: halls 12-20 m wide and 10-15 m high, galleries and balconies over them, corridors 4 m | colonnades, coffered or barrel vaults, great doors, mechanisms (gears, sluices, lifts), each made by its realm's lost people | dressed stone or metal, of the realm; machines, statues, braziers, water channels | a **larger-grid kit**, a module of 2 m or 4 m, set into a carved cavern so its outside edge is rock |

**What each needs.**

- **Barrow halls**:
  - Made (`tools/props/barrow.py`, procedural in Blender, laid by the stamp's `kit.rs`): a barrel vault, a groin vault (crossings and hall bays, scaled), a doorway arch, a pier (a pilaster half in a wall), a loculus front, shrouded and bare remains; the ashlar pack `large_sandstone_blocks`. A sarcophagus over the `sarcophagus` module's cubes. As first planned, kit pieces made with the models skill and Tripo: a barrel-vault segment over a 3 m span, a cell long; a groin vault for a crossing; a transverse rib with its springers and imposts; a square pier with a base and capital; an arch for a doorway or bay; a loculus front, empty or filled; a sarcophagus.
  - A second stone pack, ashlar.
  - Props: bones, skulls and shrouds.
  - Building gains a dressed piece laid over a run of cubes.
- **Hewn mine**:
  - Kit: a timber set, a cribbing stack, rail straight and curved, an ore cart, a ladder, a winch.
  - The stamp's digs turn into shaped carves (rough walls, not boxes). D502 still keeps the rock undiggable.
- **Natural caverns**:
  - Placement on a coarser grid (8 m cells) with volumes in place of cells. The mission graph, A* routing and the promises stay. Dressing places props rather than tiles.
  - Props: speleothems, crystals, fungus lights, rock arches.
  - Light comes from the cave's own glow and the player's torch.
- **Ancient engineered halls**:
  - Kit pieces: a column, a coffer bay, a balcony rail, a great door, and machine props.
  - The 4 m grid with two-level halls.

**The generator's sizes per theme** (data in the theme file, D136):

| Theme | Cell | Level | Corridor clear | Room, in cells | Goal |
|---|---|---|---|---|---|
| Barrow halls | 4 m | 6 m | 3 m | 2-3, halls 3-4 | 4x4, two levels high |
| Hewn mine | 4 m | 6 m, shafts 2-3 levels | 2.5-3 m | stopes 3-5 | a flooded gallery 4x5 |
| Natural caverns | 8 m | 10-20 m, varying | 3-6 m | volumes of 2-5 cells | a great cavern 6x6 and up |
| Ancient engineered halls | 4 m | 6 m, halls 2 levels | 4 m | 3-5 | 5x5, two levels high |

**Still the developers'**: the order to build them in (recommended: barrow halls on the new grid first, as the crypt is closest; then natural caverns, the developer's "cavernous"); whether the keep stays a theme of its own or becomes a dressing of the barrow halls; whether caverns are their own dungeons or also open into the realm's own caves (worlds.md asks whether caverns are a feature of every world).

## Open questions (design, the developers')

- Whether keys are items carried out, or objects of the dungeon.
- Difficulty by rank (a dungeon's level) or by depth inside it.
- The loot economy: what a dungeon gives against the open world.
- Respawn: a cleared dungeon stays cleared by default (D499); whether some kinds fill again, and when.
- Whether monsters form inside (magic pooling, D103).
- One-way and secret links as player skills (perception) or always visible.
