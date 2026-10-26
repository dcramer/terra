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

- **The grid**: cells 3 m square (six building cubes; a cube of wall each side leaves a 2 m corridor) and levels 4 m apart (a slab and 3.5 m clear). A cell is `IVec3`: x east, y the level counted down, z south. The entrance is 2 by 2 at the origin with a stair up to the surface north of it.
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
- **Modules** to cubes and props by words in their names (pillar, alcove, rubble, basin, shore, runner, decor, ...); an unknown name is plain floor. A corridor walled both sides gets a trim rib across its ceiling every cell and its edges worn (a half cube or two at a wall's foot, now and then a stone).
- **Run-in**: the graph's mobs at free spots away from doors, a chest a room with its loot, the goal object on its plinth between two fire bowls on stone pillars, so it is the brightest thing in its room, and the keys at their features (dungeon objects, not items), studded leaves at locked and one-way doors, each with its lock (a key, or the side a one-way door is barred on), up to 30 torches and braziers, the start on the surface and a tour of floor points to the goal by the longest way that enters no room twice, so it walks the loop and never turns round in a dead end (a key room is seen from its doorway), short of a room's middle where its way out is beside its way in.
- **Promises** (tested over 200 seeds): a walker stepping up a cube and down two reaches every object, chest, mob and fire from the start; air never leaves its space but up the entrance well; every model exists.

## In the sandbox

`cargo xtask sandbox --experiment dungeon` (also `dungeon-mine`, `dungeon-keep`; theme and seed in `assets/data/dungeon_demo.ron`) digs the most level site near the start, lays the stamp as one building with footings off and spawns the rest; the walker starts on the grass north of the stair. The run's log gives the tour as `--route` points and each leg as a `--vantage`. It also logs the film camera's eye along the tour (`Dungeon view i`, kept off walls) and where it looks from each (`Dungeon look i`): at the way 4 m on, drawn to the goal or a key once near and in sight while the way on stays in frame, turning at most 20° a metre, 60° while lagging past 0.5 rad off the way on, as a composer's hard limit keeps its target in frame, pitched at most 30°, holding the way on where it comes back to the eye at a dead end; and the dolly's pace (`Dungeon pace i`, seconds to the next eye): 2.5 m/s, slowed so the look turns at most 45° a second, eased (stamp/look.rs).

### Doors (D512, `door.rs`)

A door stands shut until a body walks into it, then its leaves swing away from that body, a quarter turn in half a second, and stay open: a body pushes when it is across the doorway, within 1.5 m in front of the leaves and walking at them faster than 0.3 m/s, so they are moving before a walker or a camera at a walk reaches them. A locked door pushes only while every key of its lock is held; a one-way door only from its free side, and once open it is open for both. Tested in `door.rs` without a GPU. The sandbox pushes with every player and camera (a film's camera walks the tour as a body would), and holds every key until taking keys is built, so its gate opens as any door.

## Next

- **Light underground**: a room's probe now wins over the forest's sky volume (D485) and holds what its openings let in (D486), but Bevy keeps only the 8 room probes nearest the camera, so farther boxes of the crypt's 168 still take the sky; `node tools/sandbox/indoorlight.ts` (a continuous check) fails until they don't.
- **Taking keys**, so a locked gate stays shut in the demo until its keys are held (D512); a door's leaves as bodies the mover collides with; chests that hold their loot as storage, water surfaces in basins.

## Open questions (design, the developers')

- Whether keys are items carried out, or objects of the dungeon.
- Difficulty by rank (a dungeon's level) or by depth inside it.
- The loot economy: what a dungeon gives against the open world.
- Respawn: a cleared dungeon stays cleared by default (D499); whether some kinds fill again, and when.
- Whether monsters form inside (magic pooling, D103).
- One-way and secret links as player skills (perception) or always visible.
