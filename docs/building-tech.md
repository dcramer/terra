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
| Roofs | The one thing not cubes: a surface from the height field, over the cubes it covers, which count as covered |

So walls meet, cross, are carved and mix kinds by sharing cubes, with nothing to reconcile. Height is counted in cubes; a storey is only the default height of a wall.

**A kit draws cubes** ([building-kits.md](building-kits.md)): it finds the runs of cubes of its kind and lays its pieces along them, its corners where runs meet and its cut ends where cubes are empty. A piece is as thick as its kind, logs 0.3 to 0.4 m and stone the whole cube, centred in the cube; what stops a walker is the cube.

**What is planned is cubes too** (D192): each cube is air, planned, or built to a **fill level** (a log fills a cube in two courses, a block in one), with at most one change pending on it: replace, carve, take away. Fill is the unit of progress, cost and collision: a cube collides from its foot to its fill. A part, a wall or floor or roof section the player lays, is only how the interface groups cubes. Ground heights under a plan are sampled when it is laid and kept with it, so every machine agrees on its footings and support.

**What is done is operations** (D21): fill a block of cubes with a kind or empty it; a roof section; the room shortcut (a ring of wall cubes round a footprint, a floor under it and a roof over it, opening into rooms it meets); with plans, lay a part, fill a cube, empty one. The server checks each against the state when it arrives, undo included as its inverse. A base is saved and sent as a snapshot of its cubes and roofs, and the operations since. Older saves, of rooms and then of walls on the edges between cells, are still read and turned into cubes: a room becomes its walls, floor and roof, and a wall on an edge becomes the cubes on its outside, so rooms keep their size inside; a room's wall pushed or pulled is left out.

**Everything else is worked out from the parts**, by pure functions, so every machine gets the same answer and only the parts travel:

- **The layout**: the pieces of each part, in the order they go in.
- **Buildings**: parts that touch, grouped.
- **Rooms and indoors**: the air cubes each building closes in, flood-filled.
- **The roof's shape**: a height field over the grid.
- **What stops a walker**: the built cubes, read directly, each to its fill: their tops hold a walker up, solid ones stop them, openings and plans let them through.
- **Support**: every built cube joined, face to face, to the ground or a footing that reaches it; a roof section held by the cubes or posts under its eaves (D194). Nothing is taken away while anything rests on it (D193).

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

**Layouts follow the cubes and the current kit.** Progress is kept per cube, not per piece, so a layout can always be worked out again: a kit tuned later redraws what is built, and a corner looks the same whichever wall went up first, with nothing lost (since 2026-10-12; layouts were frozen once a piece was in). Layouts are in grid units from kit data only, so the server needs no art, and versions are picked by a fixed hash of the base's seed and the cube's place.

## Roofs: a height field

Because walls run in four directions only (D146), a hip roof over any footprint, L, U, T or a courtyard, is simply: **height = pitch × how far the point is from outside**, measured as the larger of the two axis distances. Every face is flat, and every ridge, hip and valley falls on the grid's lines or diagonals, so there are no rounding errors to chase and no skeleton solver; none in Rust fits our licence anyway. A test checks it on L, T, U and courtyard plans before anything is drawn from it.

**Sections**, The Sims' way: a roof is one or more rectangles, each hip, gable, lean-to or flat at the roof's pitch, and the roof is the highest of them at each point, which makes valleys and gable ends by itself. The roof tool's first offer over enclosed walls is one hip over the whole footprint; the player can draw sections instead, for gables.

**Read off the height field**: the covering, a surface per face in its material; trims along eaves, ridges, hips and valleys; rafters on the grid lines and hip and valley rafters on the diagonals; gable walls, a wall's raked top, cut by a plane when the pieces are baked.

**Deferred**: mixed pitches in one roof, dormers, curved and conical roofs, splitting a footprint into gables automatically.

## Drawing many buildings

On the Bevy side, in `terra_game`:

- **Built pieces are merged** into meshes per fixed section of cubes and material, rebuilt in a background task only where cubes changed. Never an entity per stone.
- **Pieces being built are drawn on their own**, instanced, with the ghost of what is still to go in, until the part is finished.
- **Far off, a simplified mesh per section** with its texture baked, as Unreal's HLOD, faded in with Bevy's `VisibilityRange`.
- **Measured before relying on it**: a village of fifty cabins, on the fast GPU and the integrated one, in the loop a player runs.

## Constructing

- **A part's progress is its cubes' fill**, built from the bottom, as what each rests on is in (D194).
- **The next piece** is the lowest not yet in, nearest the builder, so holding the key builds a wall up course by course, and two players can work from both ends.
- **Roofs go in by stages per face**, frame, then battens, then covering, not tile by tile.

## Buildings and indoors

- **Buildings**: parts that touch, by a flood fill over parts, when one is laid, finished or removed. Each building is an entity in the game with its rooms, so stations (D151) and weather (D78) read it.
- **Rooms**: air cubes flood-filled within the building's walls; open doorways and windows let weather through, a shut door does not. A cell is covered if a roof's height field or a floor is over it.
- **Indoors**: a room closed all round and covered. When it is not, the game knows which wall or roof is missing, to say so (D155).
- **Natural shelter** under a cliff or in a cave is not a building, and is left to the weather's design.

## Where it is worked on

On the yard, a flat test level: `cargo xtask yard`, or `--level yard` on the play experiment for the sandbox remote. Nothing on it, building free and instant, nothing saved.

## Order

Each step is played and checked by script before the next (D69):

1. **The model** (built, on cubes, D181): walls, floors and roofs on the grid, rooms worked out from the walls, plain box meshes with trim; walls drawn inside and out, with a footing on the ground; older saves converted; built whole. Buildings as entities, and indoors as closed and covered, come with weather and stations that read them.
2. **Roofs** (a first version built): the height field, the four styles, laid over a room by a click or drawn as a rectangle, gable ends closed with wall. Not yet: eaves, trims, the covering's pieces, valleys where sections meet walls, an automatic hip over any footprint.
3. **Plans and constructing** (D139): parts laid as plans; pieces put in one by one, from the pack and storage (D141).
4. **Kits**: the four patterns with script-made placeholder pieces, then the merged mesh, the stand-in, and the fifty-cabin measurement. Logs first ([building-kits.md](building-kits.md), D177, D178): the stacked pattern, logs made in code, the three states and a kit sheet on the yard, built.
5. **The art of the kits**, by the developers' art pipeline: about ten to fifteen pieces a material, a few versions each.

## Where the risk is

| Routine | Hard | Deferred |
|---|---|---|
| The model, buildings, indoors; straight walls; one hip roof over any plan; progress bits; merging | Where roofs meet walls and other roofs; joins and openings across four patterns; the art of each kit, where looks are won or lost | Diagonal and round walls, carving, mixed pitches, dormers, curved roofs, impostors |
