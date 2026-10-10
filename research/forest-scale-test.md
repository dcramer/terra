# A planet of choppable trees

2026-10-10. Question: can the world hold a very large number of objects that are each interactive? The developers' example: with a lot of trees, can you chop any of them, can you chop all of them, and what does that cost?

## Answer

Yes, at this scale, with room to spare on a discrete GPU. The trick is to never store the trees.

## How it works

The sandbox's `forest` experiment (`crates/terra_sandbox/src/experiments/forest.rs`):

- **Trees are computed, not kept.** The planet's surface is divided into 13,824 cells of about 65 m. The trees in a cell are a function of the seed and the cell, so any cell can be produced on demand and discarded.
- **Only changes are remembered.** A chopped tree is one bit in its cell's mask. That bit, as an operation, is also all that would be sent to another player or saved.
- **Only what is near is drawn.** Cells within about 520 m of the camera are each baked into a single mesh. Chopping a tree rebuilds that one cell's mesh.
- **Interaction needs no per-tree object.** The tree being pointed at is found by searching the nine cells around that point.

## Measurements

A 2 km planet at 1600x900, with the `look` experiment also on (bloom, haze, shadows, 1,400 boulders and shrubs).

| Measurement | Result |
|---|---|
| Trees over the whole planet | Up to 995,328 (fewer where the sea covers them) |
| Trees drawn around the camera | 7,292, in 123 cells |
| Building one cell's mesh | 0.2 ms on the developer's machine |
| Chopping one tree | One bit set and one cell rebuilt |
| Chopping every tree in view (7,292) | 2.7 ms to update state; worst frame 9 ms |
| Memory if every tree on the planet were chopped | 216 KB |
| Frame time, RTX 5090 | Median 3.4 ms, 99th percentile 5.2 ms, worst 8.5 ms |
| Frame time, integrated Radeon | Median 48 ms |

## What this means

- **"Can you chop every tree?" costs almost nothing to answer yes to.** State grows with what players change, capped at one bit per tree.
- **A small round planet helps a great deal.** The ground curves away, so only a few hundred metres are ever in view from the surface. A million trees on the planet is seven thousand on screen.
- **Regrowth is a design choice, not a technical problem.** Remembering when a tree was cut, instead of only that it was, costs a few bytes per chopped tree.
- **The integrated GPU is far below target here**, at about 21 fps. Most of that is not the trees: the stand-in planet is one uniformly dense mesh of 786 thousand triangles with no levels of detail, and bloom and shadows are on. It still needs explaining before anything is promised for that class of hardware.

## Limits

- Trees are one kind, 46 triangles each, with no levels of detail and no wind.
- Nothing is drawn beyond 520 m, so from high up or from orbit the forest disappears. A distant view needs a cheaper stand-in, such as tinting the ground.
- Meshes are built on the main thread, a few cells per frame. That was fast enough here and would not be with heavier trees.
- The trees do not block movement. Colliding with them would use the same per-cell lookup.
- Tested on a formula planet, not voxels.
- Measured in scripted runs with the camera still. Nobody has flown through it yet.
