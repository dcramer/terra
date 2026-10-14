# Building: the technical design

How walls, floors and roofs work in code. The design is [building.md](building.md), and the rules every piece follows are [proposals/construction-001.md](proposals/construction-001.md) (D192 to D194); the plan of work is [building-plan.md](building-plan.md) (D195); the prior art is [research/procedural-buildings.md](../research/procedural-buildings.md). Technical choices here are mine (D160); nothing in it changes a design decision.

## What it rests on

| | |
|---|---|
| D152 | Blueprints are walls, floors and roofs, built up piece by piece; a room is what they enclose |
| D154 | The player draws the shape; the game makes the details, as The Sims' build mode |
| D155 | The game knows its buildings and their rooms, and which rooms are indoors |
| D139 | Laid as a plan, then constructed part by part |
| D146 | Each kind of construction is a kit of pieces on the grid, laid by its rule; walls in four directions |
| D151 | A station's expansions count anywhere in its building |
| D57, D48, D181 | One half-metre grid, and everything built is cubes on it |
| D21, D51 | Everything done is an operation, the same on every machine |
| D127 | Many of everything: work out only what changed, and draw cheaply |

## The model

Shared state, in `terra_build`, with no Bevy (the rule in AGENTS.md).

**What stands is cubes** (D181): every half-metre cube of a base's grid is air or a kind of construction (logs, stone, planks, wattle and daub). This is strictly the voxel grid of D48.

| | As cubes |
|---|---|
| Walls | A row of cubes one thick, as high as drawn: three metres, six cubes, unless dragged taller or shorter (D176) |
| Openings | Cubes left empty in a wall, of any size and shape. The door tool's default is 1 m by 2 m, the window tool's a sill at 1 m and a head at 2 m, Enshrouded's way; anything else is carved. A fitted door, shutters or glass is a piece set into an opening it fits |
| Posts | A column of cubes, alone or in a wall (D179) |
| Floors | A layer of cubes, so a 3 m storey is a floor cube and 2.5 m clear above it |
| Roofs | Cubes of a roof kind cut to slopes and corners (D241), by hand or laid by "add a roof"; drawn as a roof from their neighbours, no wall or floor, and what is under them is covered |

So walls meet, cross, are carved and mix kinds by sharing cubes, with nothing to reconcile. Height is counted in cubes; a storey is only the default height of a wall.

**A kit draws cubes** ([building-kits.md](building-kits.md)): it finds the runs of cubes of its kind and lays its pieces along them, its corners where runs meet and its cut ends where cubes are empty. A piece is as thick as its kind, logs 0.3 to 0.4 m and stone the whole cube, centred in the cube; what stops a walker is the cube.

**What is planned is cubes too** (D192): each cube is air, planned, or built to a **fill level** (a log fills a cube in two courses, a block in one), with at most one change pending on it: replace, carve, take away. Fill is the unit of progress, cost and collision: a cube collides from its foot to its fill. A part, a wall or floor or roof the player lays, is only how the interface groups cubes. Ground heights under a plan are sampled when it is laid and kept with it, so every machine agrees on its footings and support.

**What is done is operations** (D21): fill a block of cubes with a kind or empty it; the room shortcut (a ring of wall cubes round a footprint, a floor under it and a roof over it, opening into rooms it meets); with plans, lay a part, fill a cube, empty one. The server checks each against the state when it arrives, undo included as its inverse. A base is saved and sent as a snapshot of its cubes, and the operations since; roof sections in older saves become roof cubes as they are read (D241). Older saves, of rooms and then of walls on the edges between cells, are still read and turned into cubes: a room becomes its walls, floor and roof, and a wall on an edge becomes the cubes on its outside, so rooms keep their size inside; a room's wall pushed or pulled is left out.

**Everything else is worked out from the parts**, by pure functions, so every machine gets the same answer and only the parts travel:

- **The layout**: the pieces of each part, in the order they go in.
- **Buildings**: parts that touch, grouped.
- **Rooms and indoors**: the air cubes each building closes in, flood-filled.
- **The roof's shape**: its cubes' tops, sampled every half cube (D241).
- **What stops a walker**: the built cubes, read directly, each to its fill: their tops hold a walker up, solid ones stop them, openings and plans let them through.
- **Support**: every built cube joined, face to face, or a roof's slope along its ramp to the cube a layer below it, to the ground or a footing that reaches it (D194, D241). Nothing is taken away while anything rests on it (D193).

Each is recomputed for the building that changed, when a part is laid, a piece goes in or comes out, never per frame.

## Sizes

The sizes every part should have are in [building.md](building.md#sizes) (D175). Each kit is checked against them as it is read: its course heights divide the storey, its wall is as thick as its kind says, its doors and windows are the agreed sizes.

## Walls: laid by a rule, from a kit

A wall's layout is a pure function of its plan, how it meets its neighbours, its kit and a seed:

1. Split it at its openings.
2. Find how each end meets the next wall: an end, an L, a T or a cross.
3. Lay its courses by the kit's pattern, choosing each piece by its role: a whole block, a half, a corner (alternating by course), a jamb, a sill, a lintel, a cap.
4. Pick a version of each piece by a hash of where it is, so long walls do not repeat.

**Patterns are code, kits are data.** Four patterns cover the kinds of construction in building.md: coursed (cut stone), stacked (logs), framed (posts, rails and planks), staked (wattle and daub). A kit is a file in `assets/data/` (D136): the pattern, its sizes, and each piece's model, size, role and versions. A new material on a pattern is a new file and its models, with no code; that is what makes many variants cheap. Rule tables, not wave function collapse: WFC can fail or need backtracking, which the research found Townscaper accepts and a building game cannot.

**A course must divide the storey**, or a top plate takes what is left; each kit is checked for it as it is read.

**Each exposed edge is dressed by its role** (D215): a role worked out from the cubes and the ground when a section is laid, never stored, and the piece the kit's file gives it, belonging to that edge's cube ([building-kits.md](building-kits.md)).

**Layouts follow the cubes and the current kit.** Progress is kept per cube, not per piece, so a layout can always be worked out again: a kit tuned later redraws what is built, and a corner looks the same whichever wall went up first, with nothing lost (since 2026-10-12; layouts were frozen once a piece was in). Layouts are in grid units from kit data only, so the server needs no art, and versions are picked by a fixed hash of the base's seed and the cube's place.

## Roofs: cubes (D241)

A roof is cubes of a roof kind cut to slopes, outer and inner corners, halves and wholes, built by hand as Enshrouded's roof blocks are (D226), so a roof has any shape and storeys go over or beside it. Its look comes from its cubes' neighbourhood (`layout::roofs`): every roof cube with nothing over it gives its top, sampled every half cube in four triangles a square, a square carrying on into the next where their corners are at the same heights; creases are ridges, hips and valleys, open edges eaves and verges carried on by the kit's eaves, edges against something standing up flashing. Laid per section from the whole roof that reaches it, so its long pieces join as laying the whole base would.

**"Add a roof"** (`terra_build::roofer`) lays a plan of roof cubes over a footprint from a height field rising a cube a cube from its edges: a hip's is the chessboard distance to what is outside it, which gives hips at outer corners and valleys at inner ones over any plan whose walls run the two ways (the straight skeleton of such a plan); a gable's, over each largest rectangle the footprint holds, how far a point is from its nearer long side, the highest where they overlap; each cube cut to the shape its corners' heights make. Walls under its gable ends are carried up to it.

**Deferred**: other pitches than a cube a cube, curved and conical roofs.

## Drawing many buildings

On the Bevy side, in `terra_game` (D213; [building-plan.md](building-plan.md#drawing)):

- **A base is an entity, its sections of 16 cubes a side its children.** A change sets the sections of its building waiting; they are laid and merged on worker threads, nearest the camera first, and put on screen a few a frame. A section laid the same as before is left alone. Never an entity per stone.
- **Built pieces are merged** into a mesh per section, kind and material, their corners shared.
- **Pieces not yet built are ghosts**, a unit box or prism each in one ghost material, which Bevy batches, drawn as a blueprint; the next piece is drawn as it will be, lit.
- **Far off, one mesh per section** with its textures baked into its colours, as Unreal's HLOD, coarser again further, switched with Bevy's `VisibilityRange`; near detail is made only near the camera.
- **Measured**: the `village` bench, fifty buildings in view, against `village-bare`, the same view with nothing built, on the fast GPU and the integrated one.

## Constructing

- **A part's progress is its cubes' fill**, built from the bottom, as what each rests on is in (D194).
- **The next piece** is the lowest not yet in, nearest the builder, so holding the key builds a wall up course by course, and two players can work from both ends.
- **A roof's cubes go in a layer at a time from the eaves**, each in four steps: frame, battens, covering, trims (D241).

## Buildings and indoors

- **Buildings**: parts that touch, by a flood fill over parts, when one is laid, finished or removed. Each building is an entity in the game with its rooms, so stations (D151) and weather (D78) read it.
- **Rooms**: air cubes flood-filled within the building's walls; open doorways and windows let weather through, a shut door does not. A cell is covered if a roof's cubes or a floor is over it.
- **Indoors**: a room closed all round and covered. When it is not, the game knows which wall or roof is missing, to say so (D155).
- **Natural shelter** under a cliff or in a cave is not a building, and is left to the weather's design.

## Where it is worked on

On the yard, a flat test level: `cargo xtask yard`, or `--level yard` on the play experiment for the sandbox remote. Nothing on it, building free and instant, nothing saved.

## Order

Each step is played and checked by script before the next (D69):

1. **The model** (built, on cubes, D181): walls, floors and roofs on the grid, rooms worked out from the walls, plain box meshes with trim; walls drawn inside and out, with a footing on the ground; older saves converted; built whole. Buildings as entities, and indoors as closed and covered, come with weather and stations that read them.
2. **Roofs** (built, D241): roof cubes by hand, and "add a roof" laying them in four styles over a room or a rectangle drawn; eaves, verges, ridges, hips, valleys and flashing from their neighbours.
3. **Plans and constructing** (D139): parts laid as plans; pieces put in one by one, from the pack and storage (D141).
4. **Kits**: the four patterns with script-made placeholder pieces, then the merged mesh, the stand-in, and the fifty-cabin measurement. Logs first ([building-kits.md](building-kits.md), D177, D178): the stacked pattern, logs made in code, the three states and a kit sheet on the yard, built.
5. **The art of the kits**, by the developers' art pipeline: about ten to fifteen pieces a material, a few versions each.

## Where the risk is

| Routine | Hard | Deferred |
|---|---|---|
| The model, buildings, indoors; straight walls; roofs of cubes over any plan; progress bits; merging | Where roofs meet walls and other roofs; joins and openings across four patterns; the art of each kit, where looks are won or lost | Diagonal and round walls, other pitches, curved roofs, impostors |
