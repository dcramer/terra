# Terrain: brief for `terra_voxel`

The brief for track A of [engine.md](engine.md): the terrain core (A1), in `crates/terra_voxel`, and the terrain on screen (A2), in `terra_game`. It says what to build and how to know it works, not how to write it. Where it stands is in [engine.md](engine.md#tracks).

## What it is

A plain Rust crate, with no Bevy dependency, that turns a description of shape and material at every point into the meshes of a round, diggable planet at several levels of detail. `terra_game` will draw what it produces.

Voxels carry a material as well as a shape, because resources are harvested by digging (D39). That is part of the data model from the first version.

## Scope of the first checkpoint (A1)

| In | Out, for later milestones |
|---|---|
| A field trait giving shape and material at a point, and a planet built from the seed's large hills plus fine noise, with a few placeholder materials laid down by rule | Drawing anything (A2) |
| Storage for edited voxels over the generated terrain | Streaming as a camera moves (A2) |
| Sphere edits, add and remove, as operations. Removing reports how much of each material came out. | Collision meshes for rigid bodies (later) |
| A tree of chunks at several detail levels around one or more viewers | Networking (track D) |
| Meshing a chunk, with seams stitched against coarser neighbours and material weights on each vertex | Plants and instancing |
| Point queries: distance to ground, and a ray cast | Saving to disk |

## Shape of the interface

A sketch to react to, not a specification.

- A **field** answers "how far is this point from the ground, and what is it made of?" for a batch of points.
- A **planet** owns a density field, the edits made to it, and a seed.
- An **edit** is an operation with a shape, a position and a size. Applying it reports which chunks changed and, for a removal, the volume of each material taken.
- A **viewer** is a position. Given the viewers, the planet says which chunks should exist and at what detail.
- **Meshing a chunk** returns positions, normals, material weights and indices, ready to hand to a renderer.

Everything is in planet-local coordinates, in metres, with the planet's centre at the origin.

## A1 is done when

- `cargo xtask check` passes, with no GPU.
- **No cracks.** A test finds no open edges between chunks at different detail levels.
- **Edits behave.** Digging then filling restores the ground. The same operations in the same order give the same voxels.
- **Harvest adds up.** The volume reported for a dig matches the volume that disappeared, split correctly between materials.
- **It is fast enough to build on.** Meshing a 32-cubed chunk takes no longer than the 6 ms measured in the toolchain check, on both the Linux and Windows builds ([research/terrain-meshing.md](../research/terrain-meshing.md)).

## A2: on screen

The game's ground is the voxel planet (`crates/terra_game/src/terrain.rs` draws it, `crates/terra_game/src/planet.rs` answers for it): the first view, about 890 chunks, is meshed across every core before the first frame; after that chunks are meshed in the background as the camera moves, the old kept until the new are ready; any edit chooses the chunks again and re-meshes those it reached. Each vertex carries what grows there, what it is made of, and how much sky it sees, so a dug pit is dark at the bottom. A pickaxe on bare ground digs; digs are saved and replayed. Grass, plants, rocks and trees go from ground that has been dug, and grass grows back round it. Ground rising more steeply than about 50 degrees and higher than a 0.7 m step is a wall, slid along, so a pit holds you until you dig your way out; ground at chest or head height keeps you out from under an overhang; the camera comes in front of ground behind the player as it does of walls. All of it is stand-in arithmetic against the field until collision is designed (D53). What is built into a slope cuts the ground away where it stands (D131).

## Where it plugs in

The game asks every question about the ground through `terra_game`'s `Planet`: where something at a point stands (`ground_at`), whether a point is under the sea, which way the ground faces, how far a point is above it, and a ray cast. Nothing reaches past those, so voxel terrain replaces what answers them rather than the code that asks. `ground_at` takes a point, not a direction, because with voxels there can be ground above you as well as below: from a point in the air it finds the ground below, from one in the ground the surface above, stepping and then halving the step, since beside a dug hole the distance to the ground is not the height above it.

## Where to read in Voxel Tools

Version 1.7. The port follows its design; see [research/voxel-tools-port.md](../research/voxel-tools-port.md).

| Our piece | Read in Voxel Tools | Lines |
|---|---|---|
| Chunk tree and which chunks exist | `terrain/variable_lod/voxel_lod_terrain_update_clipbox_streaming`, `..._update_octree_streaming` | 1,700 and 800 |
| Scheduling updates | `terrain/variable_lod/voxel_lod_terrain_update_task` | 1,200 |
| The terrain as a whole | `terrain/variable_lod/voxel_lod_terrain` | 4,600, much of it Godot plumbing |
| Voxel storage | `storage/voxel_buffer`, `storage/voxel_data`, `storage/voxel_data_map` | 2,300, 1,800 and 700 |
| Materials per voxel | `storage/mixel4`: up to four materials and their weights per voxel | 200 |
| Sphere edits | `edition/funcs`, `edition/voxel_tool_lod_terrain` | 1,500 and 700 |
| Ray cast | `edition/raycast` | 500 |
| Meshing and seams | `meshers/transvoxel/transvoxel`, `transvoxel_tables` | 1,800 and 1,100. Only needed if the `transvoxel` crate falls short (D32). |
| Work priorities | `engine/priority_dependency`, `engine/voxel_engine` | 100 and 800 |

Code ported from Voxel Tools keeps its MIT notice; see [THIRD_PARTY.md](../THIRD_PARTY.md).

## Open questions for whoever builds it

- Answered, for now: **which streaming scheme.** An octree split by distance to each viewer, two chunk sizes, kept to chunks the surface may pass through and balanced so neighbours differ by a level at most. About 880 chunks around one viewer on the 2 km planet, chosen in 8 ms. Voxel Tools' clipbox scheme stays the reference if streaming as the camera moves needs it.
- Answered: **how edited voxels are stored.** Not as voxels: edits are kept as the operations themselves and laid over the generated field whenever a point is asked about, as D21 already syncs and saves them. Deterministic and small; the cost grows with the number of edits near a point, so a spatial index, and later cached blocks, come when that shows. The original uses 16-bit values in 16-cubed blocks.
- Whether the `transvoxel` crate's need to re-mesh on detail changes is acceptable (D32).
- How materials are stored. The original packs four material indices and four weights into each voxel. The `transvoxel` crate lets a voxel carry extra data and hands both voxels to the mesh builder, so weights can be interpolated there.
