# Structures: the technical design

How walls, floors and roofs work in code, for the building the developers agreed on 2026-10-12. The design is [construction.md](construction.md); the prior art is [research/procedural-buildings.md](../research/procedural-buildings.md). Technical choices here are mine (D160); nothing in it changes a design decision.

## What it rests on

| | |
|---|---|
| D152 | Blueprints are walls, floors and roofs, built up piece by piece; a room is what they enclose |
| D154 | The player draws the shape; the game makes the details, as The Sims' build mode |
| D155 | The game knows its buildings and their rooms, and which rooms are indoors |
| D139 | Laid as a plan, then constructed part by part |
| D146 | Each kind of construction is a kit of pieces on the grid, laid by its rule; walls in four directions |
| D151 | A station's expansions count anywhere in its building |
| D57, D48 | One half-metre grid; the voxel volume comes later and these become one way of writing it |
| D21, D51 | Everything done is an operation, the same on every machine |
| D127 | Many of everything: work out only what changed, and draw cheaply |

## The model

Shared state, in `terra_build`, with no Bevy (the rule in AGENTS.md).

**What stands is kept by the grid** (built, step 1):

| | Kept as |
|---|---|
| Walls | Each edge between two cells that has one, with what fills it: solid, a window or a doorway. A wall drawn is a line of these. |
| Floors | Each cell with a floor at its foot |
| Roofs | Each cell with a roof over it; flat for now, styles and sections in step 2 |

So walls meet, cross and join by sharing edges and grid points, with nothing to reconcile.

**What is planned is parts** (step 3): each wall, floor or roof section the player lays, with an id that never changes, its kind of construction, and which of its pieces are in. When a part is finished, its edges and cells join what stands.

**What is done is operations** (D21): set a wall on an edge or take it away, lay or take away floors and roofs over a block, the room shortcut (walls round a block, a floor and a roof, opening into rooms it meets); with plans, lay a part, put a piece in, take one out. A base is saved and sent as its operations. Saves from before are read: a room becomes its walls, floor and roof; a room's wall pushed or pulled is left out.

**Everything else is worked out from the parts**, by pure functions, so every machine gets the same answer and only the parts travel:

- **The layout**: the pieces of each part, in the order they go in.
- **Buildings**: parts that touch, grouped.
- **Rooms and indoors**: per storey, the cells inside each building, flood-filled.
- **The roof's shape**: a height field over the grid.
- **What stops a walker**: from walls, floors and roofs, as now from the hull.

Each is recomputed for the building that changed, when a part is laid, a piece goes in or comes out, never per frame.

## Sizes

The sizes every part should have are in [construction.md](construction.md#sizes) (D175). Each kit is checked against them as it is read: its course heights divide the storey, its wall is as thick as its kind says, its doors and windows are the agreed sizes.

## Walls: laid by a rule, from a kit

A wall's layout is a pure function of its plan, how it meets its neighbours, its kit and a seed:

1. Split it at its openings.
2. Find how each end meets the next wall: an end, an L, a T or a cross.
3. Lay its courses by the kit's pattern, choosing each piece by its role: a whole block, a half, a corner (alternating by course), a jamb, a sill, a lintel, a cap.
4. Pick a version of each piece by a hash of where it is, so long walls do not repeat.

**Patterns are code, kits are data.** Four patterns cover the kinds of construction in construction.md: coursed (cut stone), stacked (logs), framed (posts, rails and planks), staked (wattle and daub). A kit is a file in `assets/data/` (D136): the pattern, its sizes, and each piece's model, size, role and versions. A new material on a pattern is a new file and its models, with no code; that is what makes many variants cheap. Rule tables, not wave function collapse: WFC can fail or need backtracking, which the research found Townscaper accepts and a building game cannot.

**A course must divide the storey**, or a top plate takes what is left; each kit is checked for it as it is read.

**Once a piece is in, a part's layout is frozen.** It keeps the kit version it was laid with, and it cannot be changed, only taken out piece by piece and laid again. So a piece's identity never shifts under a builder, which the research names as the subtle problem, and a changed kit never re-lays a half-built wall.

## Roofs: a height field

Because walls run in four directions only (D146), a hip roof over any footprint, L, U, T or a courtyard, is simply: **height = pitch × how far the point is from outside**, measured as the larger of the two axis distances. Every face is flat, and every ridge, hip and valley falls on the grid's lines or diagonals, so there are no rounding errors to chase and no skeleton solver; none in Rust fits our licence anyway. A test checks it on L, T, U and courtyard plans before anything is drawn from it.

**Sections**, The Sims' way: a roof is one or more rectangles, each hip, gable, lean-to or flat at the roof's pitch, and the roof is the highest of them at each point, which makes valleys and gable ends by itself. The roof tool's first offer over enclosed walls is one hip over the whole footprint; the player can draw sections instead, for gables.

**Read off the height field**: the covering, a surface per face in its material; trims along eaves, ridges, hips and valleys; rafters on the grid lines and hip and valley rafters on the diagonals; gable walls, a wall's raked top, cut by a plane when the pieces are baked.

**Deferred**: mixed pitches in one roof, dormers, curved and conical roofs, splitting a footprint into gables automatically.

## Drawing many buildings

On the Bevy side, in `terra_game`:

- **A finished part is merged** into one mesh per building and material, rebuilt in a background task when a part changes. Never an entity per stone.
- **Pieces being built are drawn on their own**, instanced, with the ghost of what is still to go in, until the part is finished.
- **Far off, a stand-in made from the plan**: walls as boxes and the roof as its few planes, in the material's colour, faded in with Bevy's `VisibilityRange`.
- **Measured before relying on it**: a village of fifty cabins, on the fast GPU and the integrated one, in the loop a player runs.

## Constructing

- **A part's progress is one bit per piece**, in the layout's order: bottom course first, corners first. A 6 m stone wall a storey high is about thirty. Saved and sent as the forest's cut trees are.
- **The next piece** is the lowest not yet in, nearest the builder, so holding the key builds a wall up course by course, and two players can work from both ends.
- **Roofs go in by stages per face**, frame, then battens, then covering, not tile by tile.

## Buildings and indoors

- **Buildings**: parts that touch, by a flood fill over parts, when one is laid, finished or removed. Each building is an entity in the game with its rooms, so stations (D151) and weather (D78) read it.
- **Rooms**: per storey, cells flood-filled within the building's walls; open doorways and windows let weather through, a shut door does not. A cell is covered if a roof's height field or a floor is over it.
- **Indoors**: a room closed all round and covered. When it is not, the game knows which wall or roof is missing, to say so (D155).
- **Natural shelter** under a cliff or in a cave is not a building, and is left to the weather's design.

## From rooms to parts

The base today is rooms of cells with a generated hull (`terra_build`). It is replaced, not kept beside:

- **Saves**: a room's operation becomes its four walls, its floor and a flat roof on loading, so games in progress keep their bases.
- **The tools**: the room tool's drawn rectangle lays walls and a floor (D152); the wall tool draws walls inside and out; a roof tool is new.
- **Walking, raycasts and the meshing** move from the hull to the parts.

## Where it is worked on

On the yard, a flat test level: `cargo xtask yard`, or `--level yard` on the play experiment for the sandbox remote. Nothing on it, building free and instant, nothing saved.

## Order

Each step is played and checked by script before the next (D69):

1. **The model** (built, 2026-10-12): walls, floors and roofs on the grid, rooms worked out from the walls, plain box meshes with trim; walls drawn inside and out, with a footing on the ground; rooms in saves converted; built whole, as before. Buildings as entities, and indoors as closed and covered, come with weather and stations that read them.
2. **Roofs** (a first version, 2026-10-12): the height field, the four styles, laid over a room by a click or drawn as a rectangle, gable ends closed with wall. Not yet: eaves, trims, the covering's pieces, valleys where sections meet walls, an automatic hip over any footprint.
3. **Plans and constructing** (D139): parts laid as plans; pieces put in one by one, from the pack and storage (D141).
4. **Kits**: the four patterns with script-made placeholder pieces, then the merged mesh, the stand-in, and the fifty-cabin measurement.
5. **The art of the kits**, by the developers' art pipeline: about ten to fifteen pieces a material, a few versions each.

## Where the risk is

| Routine | Hard | Deferred |
|---|---|---|
| The model, buildings, indoors; straight walls; one hip roof over any plan; progress bits; merging | Where roofs meet walls and other roofs; joins and openings across four patterns; the art of each kit, where looks are won or lost | Diagonal and round walls, carving, mixed pitches, dormers, curved roofs, impostors |
