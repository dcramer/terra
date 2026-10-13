# Engine: the parts and the order

Terra is first and foremost a sandbox engine (D187): many procedural worlds, and a system for each part of the game acting on the others, so that the game is what they make together, for up to a thousand players (D52). This says what the engine's parts are, how the work divides, and where each part stands. It is a working plan, provisional like everything in pre-production. The stack and the reasons behind it are in [tech.md](tech.md).

## What we expect to want

Our current expectation, however the gameplay turns out (D39):

- **Voxel worlds**: round, smooth, fully diggable.
- **Organic harvesting**: resources are in the world and you get them by taking the world apart, not by clicking a node.
- **Procedural generation almost everywhere**: the world is computed from a seed, not authored or stored.

Worlds are reached probably by portals (D72), which is a part not yet designed: loading the world beyond while a player approaches, with the crossing able to cover what is left, and no traditional loading screen. Terraforming is parked (D60), and with it the simulation runtime. What the worlds ask of the engine is in [worlds.md](worlds.md#what-it-asks-of-the-engine).

## What each of those asks of the engine

| Want | Consequence |
|---|---|
| Voxel worlds | The terrain core. |
| Organic harvesting | **Voxels carry a material, not just a shape.** Digging reports what was removed and how much. This has to be in the terrain's data model from the start; it is expensive to add later. Scattered objects such as rocks and plants must be removable one by one. |
| Procedural everywhere | **A shared generation toolkit comes first**, because terrain, materials, scattering, plants and sky all draw on it. Everything is a function of the seed and a position, so only changes are ever stored or sent. |

## The parts

| # | Part | What it is | Needs first |
|---|---|---|---|
| 0 | Generation toolkit | Seeds, noise, repeatable random streams keyed by place and purpose, sampling on a sphere. The same result on every machine. | Nothing |
| 1 | Terrain core | Shape and material at every point, edits, chunks at several detail levels, meshing, ground queries, harvest yields | Part 0 |
| 2 | Terrain on screen | Streaming chunks as the camera moves, uploading meshes, a ground material that blends voxel materials | Part 1's mesh output |
| 3 | World rendering | Sky and atmosphere from the ground, sea, sun and lighting | A radius and a few numbers |
| 4 | World frame | Planet-centred coordinates relative to a moving origin, gravity toward the centre | Nothing |
| 5 | Player and camera | Walking and flying on a round world, colliding with the ground, a dig tool | A ground query |
| 6 | App shell | Window, settings and the options menu (D80), input with remapping, frame-time readout (D77), debug menu (D81), scripted camera paths, frame-time capture, screenshots | Nothing |
| 7 | Scattering | Things placed by rule on the terrain, with culling: objects that are tracked one by one, and cover that is too dense to track | Parts 0 and a ground query |
| 8 | Procedural objects | Meshes grown from parameters: rocks first, then plants | Part 0 |
| 9 | Networking | A server and the clients that join it, ordered operations, snapshots, late join | Nothing |
| 10 | Simulation runtime | Running the simulation on its thread, snapshots, fields into textures. Parked with terraforming (D60) | The existing crate |

Saving, physics for loose objects and UI are engine work too. Sound has a first version ([audio.md](audio.md)). Collision is needed now and is described below.

## What makes them separable

Nearly everything outside the terrain only needs to know where the ground is. A **stand-in planet**, a sphere displaced by a formula (`terra_core`), let parts 3 to 10 be built and measured before the voxel terrain existed. The voxel terrain has since replaced it behind the same interface, and still takes its large hills from the stand-in's formula.

A few small contracts, in `terra_core`, are what the parts share:

- **Ground query**: how far is this point from the ground, and where does this ray hit it.
- **Chunk mesh**: what the terrain core hands the renderer, including material weights.
- **Operations**: the format of a change to the world, and what it yielded. Shared by terrain, scattering, networking and saving.
- **Water**: the water level at a position.
- **Planet description**: radius, seed, sun direction.
- **Simulation snapshot**: not written, parked with the simulation (D60).

## Tracks

The one place that says where the engine stands. The MVP's game systems stand as [prototypes.md](prototypes.md#where-it-stands) says.

| Track | Parts | Why it stands alone | Status |
|---|---|---|---|
| Foundation | Contracts, then part 0 | Small, and everything else leans on it | Built: `terra_core` and `terra_gen`, the toolkit giving the same output on Linux, macOS and Windows |
| A: terrain | 1, then 2 | The hardest work and the critical path. One crate, one owner ([terrain.md](terrain.md)) | A1 built in `terra_voxel`; A2 built: the game streams, draws and digs it. Not yet: frame times on an integrated GPU |
| B: a walkable world | 6, 4, 3, 5 | The shell is the harness everything is measured with; the rest gives a world to walk | Built in `terra_game`: the shell (options, remapping, frame-time readout, scripted runs, screenshots, the remote protocol), sky, sea, sun, and a third-person player on the round world. Not yet: the debug menu (D81), a moving origin |
| C: things on the ground | 7, 8 | Needs only the toolkit and a ground query | Scattering built: trees, rocks, plants and grass, removable one by one. Procedural objects are the lush forest spike ([research/lush-forest-spike.md](../research/lush-forest-spike.md)) |
| D: networking | 9 | A plain crate tested with two peers in one process. Touches nothing else | Not started; no `terra_net` yet. The game has no server inside it (D51) |

What done means, beyond each crate's tests:

- **A**: terrain keeps up with a camera at 100 and 400 m/s, with no worst frame above the Godot build's; a dig causes no visible hitch. A1's own criteria are in [terrain.md](terrain.md).
- **C**: 100 thousand instances within 150 m cost no more than in the Godot load test; removing one is an operation.
- **D**: two peers in one process end in the same state, and one that joins late catches up, with no window.
- **All**: frame times from the real play loop, on the fast GPU and an integrated one (D25).

Each track lives in its own crates, so people and agents working in parallel do not collide. Performance is measured one way, `cargo xtask bench` (D185); the Godot build's numbers in [research/godot-spike.md](../research/godot-spike.md) are the bar (D12).

## What waits for design

- Portals, and loading the world beyond (D72).
- What the materials and resources are, and what they are for (D43).
- Collision (D53, below).
- The simulation's rules, parked with terraforming (D60).

Building is designed ([building.md](building.md), [building-tech.md](building-tech.md); D48, D152, D154, D181) and so is the art direction ([art-style.md](art-style.md), D14).

## Harvesting

Three kinds of thing can be harvested, and each needs a different mechanism. Which of them the game has, and what they yield, is design (D43).

| Kind | Examples | Mechanism | Part |
|---|---|---|---|
| The ground | Rock, ore | Dig voxels; the yield is the volume of each material removed | 1 |
| Objects | Trees, boulders | Individually placed by rule from the seed. Each has an identity, so removing one is an operation. | 7 |
| Cover | Grass | Too many to track one by one. Density comes from a rule; cutting clears an area, and the yield is the amount in it. | 7 |

All three go through one contract: a harvest is an operation, and applying it returns a yield.

## Collision

Required by the developers (D53): things in the world need collision shapes that work with the voxel terrain and everything else. A player has to be stopped by a cliff, a tree, a wall and another player alike.

What has to collide with what:

| Thing | Collides with |
|---|---|
| Players and creatures | The ground, including holes and tunnels that were dug; bases; trees, rocks and other objects; each other |
| Loose objects, if we have them | The same |
| Rays, for aiming tools | All of the above, through one question: what is under the crosshair? |

Where each shape would come from:

- **Voxel terrain**: generated with each chunk's mesh, for the finest chunks near anything that can move, and rebuilt when the ground is dug.
- **Bases**: from the base's cubes (D181).
- **Objects**: computed per area, like the objects themselves, so a million trees do not mean a million shapes.

What exists today is a stand-in. The player walks using arithmetic: the voxel field for the ground, the base's cubes for floors, walls and doors, circles for trees and boxes for placed items. There are no collision shapes, and loose objects have none of it.

The plan on record is a physics engine (Avian, D33) for shapes and loose objects, with the player and creatures asking the voxel data directly so they never wait for a shape to be built. That plan predates this requirement and has not been tested. It needs its own design doc before anything is built.

## Many things, all interactive

Performance is a baseline concern (D25): the world should be able to show very many objects and let the player interact with any of them. The pattern that makes this cheap, tried in the sandbox's forest experiment (D47):

- **Compute, don't keep.** Objects are a function of the seed and a place, so they can be produced when needed and discarded.
- **Remember only changes.** A removed object is a bit, and an operation.
- **Draw only what is near**, baked together by area so thousands of objects are a handful of draws.
- **Interact by lookup.** The object being pointed at is found by searching the area around that point, with no per-object entity.

A million trees over a 2 km planet came to seven thousand on screen, 216 KB of state if every one were chopped, and a median 3.4 ms a frame on the RTX 5090. Details and limits are in [research/forest-scale-test.md](../research/forest-scale-test.md).

We expect to need tactics like these everywhere, given large worlds and procedural generation. The habit is to ask of anything numerous: can it be computed instead of stored, and does it need to exist when nobody is near?

## Water and melting

The ground is never reshaped (D20). Two consequences if things are to melt into pools (D44):

- **What melts is a layer on the ground, not the ground.** Ice and snow are a thickness lying over the terrain. Melting or mining it lowers the thickness; the voxels underneath never change.
- **Water is a level that can differ from place to place.** A pool is a basin filled to its own level, and the sea is the largest basin. Anywhere the ground is below the local level, including a hole a player dug, there is water. Nothing flows.

The engine asks "what is the water level here?" rather than reading one global number. With a single sea that question has a constant answer, so the simple version costs nothing extra and pools remain possible.

## Open questions

- **How fine are pools?** Basins tens of metres across are cheap; a puddle from one melted block would need something finer.
- **How many materials meet at one point?** Voxel Tools blends up to four per voxel. More is costlier to store and draw.
