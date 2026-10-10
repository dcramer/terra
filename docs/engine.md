# Engine: the parts and the order

What we build before, and separately from, core gameplay. The stack and the reasons behind it are in [tech.md](tech.md); this says what the pieces are and how the work divides.

## What we know we want

Certain regardless of how the gameplay turns out (D39):

- **Voxel planets**: round, smooth, fully diggable.
- **The transition**: from standing on the ground to orbit and back, with no seam and no loading.
- **Organic harvesting**: resources are in the world and you get them by taking the world apart, not by clicking a node.
- **Procedural generation almost everywhere**: the world is computed from a seed, not authored or stored.

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
| 6 | App shell | Window, settings, input, debug overlay, scripted camera paths, frame-time capture, screenshots | Nothing |
| 7 | Scattering | Things placed by rule on the terrain, with culling: objects that are tracked one by one, and cover that is too dense to track | Parts 0 and a ground query |
| 8 | Procedural objects | Meshes grown from parameters: rocks first, then plants | Part 0 |
| 9 | Networking | Host and join, ordered operations, snapshots, late join | Nothing |
| 10 | Simulation runtime | Running the simulation on its thread, snapshots, fields into textures | The existing crate |

Saving, physics for loose objects, audio and UI are engine work too, but nothing needs them yet.

## What makes them separable

Nearly everything outside the terrain only needs to know where the ground is, and we already have that as a formula: the terrain waves give the ground height anywhere with no voxels involved. A **stand-in planet**, a sphere displaced by that formula, lets parts 3 to 10 be built and measured before the voxel terrain exists. When part 1 is ready it replaces the stand-in behind the same interface.

A few small contracts are agreed in code before the tracks start. They live in `terra_core`, with the stand-in planet that implements them:

- **Ground query**: how far is this point from the ground, and where does this ray hit it.
- **Chunk mesh**: what the terrain core hands the renderer, including material weights.
- **Operations**: the format of a change to the world, and what it yielded. Shared by terrain, scattering, networking and saving.
- **Water**: the water level at a position.
- **Simulation snapshot**: the fields and globals the host sends and the renderer reads. Not written yet: the simulation's rules are still open, so this waits until the stand-in world needs it.
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

## Water and melting

The ground is never reshaped (D20). Two consequences if things are to melt into pools (D44):

- **What melts is a layer on the ground, not the ground.** Ice and snow are a thickness lying over the terrain. Melting or mining it lowers the thickness; the voxels underneath never change.
- **Water is a level that can differ from place to place.** A pool is a basin filled to its own level, and the sea is the largest basin. Anywhere the ground is below the local level, including a hole a player dug, there is water. Nothing flows.

The engine asks "what is the water level here?" rather than reading one global number. With a single sea that question has a constant answer, so the simple version costs nothing extra and pools remain possible.

## Open questions

- **How fine are pools?** The simulation's regions are tens of metres across, so pools follow those. A puddle from one melted block would need something finer.
- **How many materials meet at one point?** Voxel Tools blends up to four per voxel. More is costlier to store and draw.
