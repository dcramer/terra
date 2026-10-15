# Building: the technical design

How walls, floors and roofs work in code. The design is [building.md](building.md), and the rules every piece follows are [proposals/construction-001.md](proposals/construction-001.md) (D192 to D194); the plan of work is [building-plan.md](building-plan.md) (D195); the prior art is [research/procedural-buildings.md](../research/procedural-buildings.md). The cube, who owns each face, edge and corner between cubes, the invariants that keep a building closed and every junction with its rule and its check are [building-voxels.md](building-voxels.md) (D348), not repeated here. Technical choices here are mine (D160); nothing in it changes a design decision.

## What it rests on

| | |
|---|---|
| D152 | Blueprints are walls, floors and roofs, built up piece by piece; a room is what they enclose |
| D226 | Players build by hand, cube by cube, roofs too, as in Enshrouded; laying a room, a wall and "add a roof" stay as conveniences (amends D154) |
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
| Openings | Cubes made open in a wall, of any size and shape. The door tool's default is 1 m by 2 m, the window tool's a sill at 1 m and a head at 2 m, Enshrouded's way; anything else is carved. A window is the hole, framed by its role pieces, with no glass (D225); a door or shutters are a prop fitted into a hole they fit, hung by `terra_build::fitting` |
| Posts | A column of cubes, alone or in a wall (D179) |
| Floors | A layer of cubes, so a 3 m storey is a floor cube and 2.5 m clear above it |
| Roofs | Cubes of a roof kind cut to slopes and corners (D241), by hand or laid by "add a roof"; drawn as a roof from their neighbours, no wall or floor, and what is under them is covered |

So walls meet, cross, are carved and mix kinds by sharing cubes, with nothing to reconcile. Height is counted in cubes; a storey is only the quick tools' default height (D277): what is wall and what floor is read from the cubes round each one.

**A kit draws cubes** ([building-kits.md](building-kits.md)): it finds the runs of cubes of its kind and lays its pieces along them, its corners where runs meet and its cut ends where cubes are empty. A piece is as thick as its kind, logs 0.3 to 0.4 m and stone the whole cube, centred in the cube; what stops a walker is the cube.

**What is planned is cubes too** (D192): each cube is air, planned, or built to a **fill level** (a log fills a cube in two courses, a block in one), with at most one change pending on it: replace, carve, take away. Fill is the unit of progress, cost and collision: a cube collides from its foot to its fill. A part, a wall or floor or roof the player lays, is only how the interface groups cubes. Cubes laid by hand join the part of the cubes of their kind beside them laid on the same terms, merging every such part they touch, and a part taken out of the plan in pieces is split, so a mass built cube by cube is one part (D260). Ground heights under a plan are sampled when it is laid and kept with it, so every machine agrees on its footings and support.

**What is done is operations** (D21): fill a block of cubes with a kind or empty it; the room shortcut (a ring of wall cubes round a footprint, a floor under it and a roof over it, opening into rooms it meets); with plans, lay a part, fill a cube, empty one. The server checks each against the state when it arrives, undo included as its inverse. A base is saved and sent as a snapshot of its cubes, and the operations since; roof sections in older saves become roof cubes as they are read (D241). Older saves, of rooms and then of walls on the edges between cells, are still read and turned into cubes: a room becomes its walls, floor and roof, and a wall on an edge becomes the cubes on its outside, so rooms keep their size inside; a room's wall pushed or pulled is left out.

**Everything else is worked out from the parts**, by pure functions, so every machine gets the same answer and only the parts travel:

- **The layout**: the pieces of each part, in the order they go in.
- **Buildings**: parts that touch, grouped.
- **Rooms and indoors**: the space the built cubes close in all round, flood-filled in three dimensions (D261).
- **Doorways and their steps**: any opening at a floor with a floor on one side and none on the other is a doorway, and its steps down to the ground are stair and half cubes in the plan, of the footings' kind (D261).
- **The roof's shape**: its cubes' tops, sampled every half cube (D241).
- **What stops a walker**: the built cubes, read directly, each to its fill: their tops hold a walker up, solid ones stop them, openings and plans let them through.
- **Support**: every built cube joined, face to face, or a roof's slope along its ramp to the cube a layer below it, to the ground or a footing that reaches it (D194, D241). Nothing is taken away while anything rests on it (D193).

Each is recomputed for the building that changed, when a part is laid, a piece goes in or comes out, never per frame.

## Sizes

The sizes every part should have are in [building.md](building.md#sizes) (D175). Each kit is checked against them as it is read: its course heights divide the storey, its wall is as thick as its kind says, its doors and windows are the agreed sizes.

## Drawing a kind: by role, from its kit

Each kind's cubes are drawn by its kit ([building-kits.md](building-kits.md)), in grid units from kit data only, so the server needs no art:

- **A block kind** (stone, earth, wattle and daub) is one surface over all its cubes, cut to their shapes, in its material's textures and relief (D197).
- **A timber kind** lays logs along its runs, courses along a wall's faces however thick (D200, D263), and decks and boarded shapes on its floors and cut cubes (D206, D231).
- **A framed kind** lays its frame by `layout::frames`'s rule over its infill (D236).
- **Each exposed edge is dressed by its role** (D215), worked out from the cubes and the ground when a section is laid, never stored, with the kit's hand-made pieces of its own materials: profiles swept along an edge, modules repeated along a line, stones set at a point, made in Blender by `tools/kits` (D251, D258).
- **Where kinds meet**, each kit's `meets` table says which runs through and which ends against it (D232).

**Patterns are code, kits are data** (D136, D210): a new material on a pattern is a file and its pieces, with no code. Rule tables, not wave function collapse: WFC can fail or need backtracking, which a building game cannot.

**Layouts follow the cubes and the current kit.** Progress is kept per cube, not per piece, so a layout can always be worked out again: a kit tuned later redraws what is built, and a corner looks the same whichever wall went up first. Versions are picked by a fixed hash of the base's seed and the cube's place.

## Roofs: cubes (D241)

A roof is cubes of a roof kind cut to slopes, outer and inner corners, halves and wholes, built by hand as Enshrouded's roof blocks are (D226), so a roof has any shape and storeys go over or beside it. Its look comes from its cubes' neighbourhood (`layout::roofs`): every roof cube with nothing over it gives its top, sampled every half cube in four triangles a square, a square carrying on into the next where their corners are at the same heights; creases are ridges, hips and valleys, open edges eaves and verges carried on by the kit's eaves, edges against something standing up flashing. Laid per section from the whole roof that reaches it, so its long pieces join as laying the whole base would. Drawn on the one material system (D251, D258): the covering is the roof cubes' sloped surface in the covering's material, thick and pushed out by its height map, and each line, and the frame under it, is a module of the kit's hand-made pieces repeated along it (`modules`), as Enshrouded draws its roofs: the material on the surface, modelled pieces at its edges.

**"Add a roof"** (`terra_build::roofer`) lays a plan of roof cubes over a footprint from a height field rising a cube a cube from its edges: a hip's is the chessboard distance to what is outside it, which gives hips at outer corners and valleys at inner ones over any plan whose walls run the two ways (the straight skeleton of such a plan); a gable's, over each largest rectangle the footprint holds, how far a point is from its nearer long side, the highest where they overlap; each cube cut to the shape its corners' heights make. Walls under its gable ends are carried up to it.

**Deferred**: other pitches than a cube a cube, curved and conical roofs.

## Drawing many buildings

On the Bevy side, in `terra_game` (D213; [building-plan.md](building-plan.md#drawing)):

- **A base is an entity, its sections of 16 cubes a side its children.** A change sets the sections of its building waiting; they are laid and merged on worker threads, nearest the camera first, and put on screen a few a frame. A section laid the same as before is left alone. Never an entity per stone.
- **Built pieces are merged** into a mesh per section, kind and material, their corners shared.
- **Pieces not yet built are ghosts**, a unit box or prism each in one ghost material, which Bevy batches, drawn as a blueprint; the next piece is drawn as it will be, lit.
- **Far off, one mesh per section** with its textures baked into its colours, as Unreal's HLOD, coarser again further, switched with Bevy's `VisibilityRange`; near detail is made only near the camera.
- **Measured** (D185): the `village` bench, fifty buildings in view, and the town of three hundred, a castle and far views, each against the same view with nothing built, on the fast GPU and the integrated one.

## Constructing

- **A part's progress is its cubes' fill**, built from the bottom, as what each rests on is in (D194).
- **The next piece** is the lowest not yet in, nearest the builder, so holding the key builds a wall up course by course, and two players can work from both ends.
- **A roof's cubes go in a layer at a time from the eaves**, each in four steps: frame, battens, covering, trims (D241).

## Buildings and indoors

- **Buildings**: parts that touch, by a flood fill over parts, when one is laid, finished or removed. Each building is an entity in the game with its rooms, so stations (D151) and weather (D78) read it.
- **Rooms** (D261): the cubes not built that have something built over and under them in their column, flooded face to face in three dimensions; a flood that reaches the sky or the open ground is the outside, the rest each a room, of any height and shape, two storeys and a mezzanine in one. Only what is built closes a room in, and an opening is part of its wall; how weather comes in through one is D78's.
- **Indoors**: every room, as it is closed all round, roof and floor too (D155). Saying which wall or roof is missing is later work.
- **Natural shelter** under a cliff or in a cave is not a building, and is left to the weather's design.

## Where it is worked on

On the yard, a flat test level: `cargo xtask yard`, or `--level yard` on the play experiment for the sandbox remote. Nothing on it, building free and instant, nothing saved.

## Order

Steps 1 to 5 of [building-plan.md](building-plan.md) are built: the model on cubes, roofs as cubes, plans and constructing, the kits and the one material system (D251, D258); what remains is there. Each step is played and checked by script before the next (D69).

## Where the risk is

| Routine | Hard | Deferred |
|---|---|---|
| The model, buildings, indoors; straight walls; roofs of cubes over any plan; progress bits; merging | Where roofs meet walls and other roofs; joins and openings where kinds meet; the hand-made pieces of each kit, where looks are won or lost | Diagonal and round walls, other pitches, curved roofs, impostors |
