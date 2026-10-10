# Terrain: brief for `terra_voxel`

Status: not started. This is the brief for milestone R1 in [prototypes.md](prototypes.md). It says what to build and how to know it works, not how to write it.

## What it is

A plain Rust crate, with no Bevy dependency, that turns a density function into the meshes of a round, diggable planet at several levels of detail. `terra_game` will draw what it produces.

## Scope of R1

| In | Out, for later milestones |
|---|---|
| A density field trait, and a planet built from the simulation's terrain waves plus fine noise | Drawing anything (R2) |
| Storage for edited voxels over the generated terrain | Streaming as a camera moves (R2) |
| Sphere edits, add and remove, as operations | Collision meshes for rigid bodies (R3) |
| A tree of chunks at several detail levels around one or more viewers | Networking (R4) |
| Meshing a chunk, with seams stitched against coarser neighbours | Plants and instancing |
| Point queries: distance to ground, and a ray cast | Saving to disk |

## Shape of the interface

A sketch to react to, not a specification.

- A **density field** answers "how far is this point from the ground?" for a batch of points.
- A **planet** owns a density field, the edits made to it, and a seed.
- An **edit** is an operation with a shape, a position and a size. Applying it reports which chunks changed.
- A **viewer** is a position. Given the viewers, the planet says which chunks should exist and at what detail.
- **Meshing a chunk** returns positions, normals and indices, ready to hand to a renderer.

Everything is in planet-local coordinates, in metres, with the planet's centre at the origin.

## Done when

- `cargo xtask check` passes, with no GPU.
- **It matches the original.** On the same density function, voxel values and meshes agree with Voxel Tools within a stated tolerance. The Godot build in `spike/planet_test` runs headless and is the reference.
- **No cracks.** A test finds no open edges between chunks at different detail levels.
- **Edits behave.** Digging then filling restores the ground. The same operations in the same order give the same voxels.
- **It is fast enough to build on.** Meshing a 32-cubed chunk takes no longer than the 6 ms measured in the toolchain check, on both the Linux and Windows builds.

## Where to read in Voxel Tools

Version 1.7. The port follows its design; see [research/voxel-tools-port.md](../research/voxel-tools-port.md).

| Our piece | Read in Voxel Tools | Lines |
|---|---|---|
| Chunk tree and which chunks exist | `terrain/variable_lod/voxel_lod_terrain_update_clipbox_streaming`, `..._update_octree_streaming` | 1,700 and 800 |
| Scheduling updates | `terrain/variable_lod/voxel_lod_terrain_update_task` | 1,200 |
| The terrain as a whole | `terrain/variable_lod/voxel_lod_terrain` | 4,600, much of it Godot plumbing |
| Voxel storage | `storage/voxel_buffer`, `storage/voxel_data`, `storage/voxel_data_map` | 2,300, 1,800 and 700 |
| Sphere edits | `edition/funcs`, `edition/voxel_tool_lod_terrain` | 1,500 and 700 |
| Ray cast | `edition/raycast` | 500 |
| Meshing and seams | `meshers/transvoxel/transvoxel`, `transvoxel_tables` | 1,800 and 1,100. Only needed if the `transvoxel` crate falls short (D32). |
| Work priorities | `engine/priority_dependency`, `engine/voxel_engine` | 100 and 800 |

Code ported from Voxel Tools keeps its MIT notice; see [THIRD_PARTY.md](../THIRD_PARTY.md).

## Open questions for whoever builds it

- Which of Voxel Tools' two streaming schemes to follow. Our Godot tests used the older octree one; the clipbox one is newer.
- How edited voxels are stored: the original uses 16-bit values in 16-cubed blocks.
- Whether the `transvoxel` crate's need to re-mesh on detail changes is acceptable (D32).
