# Engine: the parts and the order

**Status: a working plan, all still to be determined.** This is how we currently think the engine divides, written down so there is something to react to. Nothing here is a commitment.

What we would build before, and separately from, core gameplay. The stack and the reasons behind it are in [tech.md](tech.md); this says what the pieces are and how the work divides.

## What we expect to want

Our current expectation, however the gameplay turns out (D39):

- **Voxel planets**: round, smooth, fully diggable.
- **The transition**: from standing on the ground to orbit and back, with no seam and no loading.
- **Organic harvesting**: resources are in the world and you get them by taking the world apart, not by clicking a node.
- **Procedural generation almost everywhere**: the world is computed from a seed, not authored or stored.

**Under review (D87):** the developers might drop flight to space and terraforming, with worlds as alternate dimensions instead. If so, the transition below, planet rendering from space and the solar-system frame leave the plan.

Since then the developers have set out the worlds: several planets, each themed, with caverns, water features, varied plants, gravity and a breathable starter planet. What that asks of the engine is in [worlds.md](worlds.md#what-it-asks-of-the-engine).

## What each of those asks of the engine

| Want | Consequence |
|---|---|
| Voxel planets | The terrain core. Already the plan. |
| The transition | Not one part but a path through several: terrain detail, atmosphere, coordinates and gravity all have to change smoothly together. It gets its own test, a scripted descent from orbit to the ground. |
| Organic harvesting | **Voxels carry a material, not just a shape.** Digging reports what was removed and how much. This has to be in the terrain's data model from the start; it is expensive to add later. Scattered objects such as rocks and plants must be removable one by one. |
| Procedural everywhere | **A shared generation toolkit comes first**, because terrain, materials, scattering, plants and sky all draw on it. Everything is a function of the seed and a position, so only changes are ever stored or sent. |

## The parts

| # | Part | What it is | Needs first |
|---|---|---|---|
| 0 | Generation toolkit | Seeds, noise, repeatable random streams keyed by place and purpose, sampling on a sphere. The same result on every machine. | Nothing |
| 1 | Terrain core | Shape and material at every point, edits, chunks at several detail levels, meshing, ground queries, harvest yields | Part 0 |
| 2 | Terrain on screen | Streaming chunks as the camera moves, uploading meshes, a ground material that blends voxel materials | Part 1's mesh output |
| 3 | Planet rendering | Sky and atmosphere from inside and from space, sea, sun and lighting | A radius and a few numbers |
| 4 | World frame | Planet-centred coordinates, gravity toward the centre, the ground-to-orbit path | Nothing |
| 5 | Player and camera | Walking and flying on a round world, colliding with the ground, a dig tool | A ground query |
| 6 | App shell | Window, settings and the options menu (D80), input with remapping, frame-time readout (D77), debug menu (D81), scripted camera paths, frame-time capture, screenshots | Nothing |
| 7 | Scattering | Things placed by rule on the terrain, with culling: objects that are tracked one by one, and cover that is too dense to track | Parts 0 and a ground query |
| 8 | Procedural objects | Meshes grown from parameters: rocks first, then plants | Part 0 |
| 9 | Networking | A server and the clients that join it, ordered operations, snapshots, late join | Nothing |
| 10 | Simulation runtime | Running the simulation on its thread, snapshots, fields into textures | The existing crate |

Saving, physics for loose objects, audio and UI are engine work too, but nothing needs them yet. Collision is needed now and is described below.

## What makes them separable

Nearly everything outside the terrain only needs to know where the ground is, and we already have that as a formula: the terrain waves give the ground height anywhere with no voxels involved. A **stand-in planet**, a sphere displaced by that formula, lets parts 3 to 10 be built and measured before the voxel terrain exists. When part 1 is ready it replaces the stand-in behind the same interface.

A few small contracts are agreed in code before the tracks start. They live in `terra_core`, with the stand-in planet that implements them:

- **Ground query**: how far is this point from the ground, and where does this ray hit it.
- **Chunk mesh**: what the terrain core hands the renderer, including material weights.
- **Operations**: the format of a change to the world, and what it yielded. Shared by terrain, scattering, networking and saving.
- **Water**: the water level at a position.
- **Simulation snapshot**: the fields and globals the server sends and the renderer reads. Not written yet: the simulation's rules are still open, so this waits until the stand-in world needs it.
- **Planet description**: radius, seed, sun direction.

## Tracks

| Track | Parts | Why it stands alone |
|---|---|---|
| Foundation | Contracts, then part 0 | Small, and everything else leans on it. Done first, by one person. |
| A: terrain | 1, then 2 | The hardest work and the critical path. One crate, one owner, tested against Voxel Tools. |
| B: a walkable stand-in world | 6, 4, 3, 5 | The shell is the harness everything is measured with. The rest gives the transition, on the stand-in. |
| C: things on the ground | 7, 8 | Needs only the toolkit and a ground query, so it also starts on the stand-in. |
| D: networking | 9 | A plain crate tested with two peers in one process. Touches nothing else. |

Each track lives in its own crates, so people and agents working in parallel do not collide. Tracks A and B meet when real terrain replaces the stand-in; that is the point the Godot demo is matched and retired.

## What waits for design

- The simulation's rules: which fields exist and how they interact.
- What the materials and resources actually are, and what they are for.
- Machines, survival, building and creatures.
- Art style. Shaders and generators can be built now and tuned later.

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
- **Bases**: generated from the base's shape, the same way its hull is.
- **Objects**: computed per area, like the objects themselves, so a million trees do not mean a million shapes.

What exists today is a stand-in. The sandbox's player walks using arithmetic: the ground's formula for height, and the base's grid for floors, walls and doors. There are no collision shapes, nothing stops the player at a tree or a rock, and none of it would survive real voxel terrain.

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

- **How fine are pools?** The simulation's regions are tens of metres across, so pools follow those. A puddle from one melted block would need something finer.
- **How many materials meet at one point?** Voxel Tools blends up to four per voxel. More is costlier to store and draw.
