# Terrain: brief for `terra_voxel`

The brief for track A of [engine.md](engine.md): the terrain core (A1), in `crates/terra_voxel`, and the terrain on screen (A2), in `terra_game`. It says what to build and how to know it works, not how to write it. Where it stands is in [engine.md](engine.md#tracks).

## What it is

A plain Rust crate, with no Bevy dependency, that turns a description of shape and material at every point into the meshes of a round, diggable planet at several levels of detail. `terra_game` will draw what it produces.

Voxels carry a material as well as a shape, because resources are harvested by digging (D40). That is part of the data model from the first version.

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

The game's ground is the voxel planet (`crates/terra_world/src/terrain.rs` draws it, `crates/terra_world/src/planet.rs` answers for it): the first view, about 890 chunks, is meshed across every core before the first frame; after that chunks are meshed in the background as the camera moves, the old kept until the new are ready; any edit chooses the chunks again and re-meshes those it reached. Each vertex carries what grows there, what it is made of, and how much sky it sees, so a dug pit is dark at the bottom. A pickaxe on bare ground digs; digs are saved and replayed. Grass, plants, rocks and trees go from ground that has been dug, and grass grows back round it. Ground rising more steeply than about 50 degrees and higher than a 0.7 m step is a wall, slid along, so a pit holds you until you dig your way out; ground at chest or head height keeps you out from under an overhang; the camera comes in front of ground behind the player as it does of walls. All of it is stand-in arithmetic against the field until collision is designed (D53). What is built into a slope cuts the ground away where it stands (D131).

## Mountains (D322)

A world large enough for a range (10 km radius and up) has mountains in its generated ground (`terra_voxel::mountains`), by its theme's numbers (`Mountains::START`: valleys 950 m up, crests 1,800 m over them, ridges 10 km apart, ranges 15 km across on three tenths of the land):

- **Ridges:** Musgrave's ridged multifractal, each octave weighted by the ridges before it so spurs branch and valleys stay smooth, damped by the noise's own slope as Quilez damps his, its space warped so main ridges wander. The floors rise over a range's edge and are level within, hollowed where the ridged sum falls lowest, which is where lakes form.
- **Gullies:** stripes down the fall line, laid cell by cell as Runevision's erosion filter lays them, worked from its write-up (its code is MPL-2.0); three sets, each following the slope the last cut.
- **Walls and couloirs:** above half the climb from valley to crest, the climb bends convex (`walls`), so the upper slopes stand forward as walls over the valleys' floors, which stay as they were; and where the ground falls past about 31° the gullies cut 40 m deeper (`couloirs_m`), splitting the walls into ribs. Cut as deep on gentler ground, they left 1,100 pits that filled as puddles. On such a world chunks of 256 m and coarser split within three of their sizes, not two (`Detail::far_split`), so the walls a few kilometres off are meshed at a third of the cell they were and their couloirs stand in the mesh: 4,544 chunks in the lake's first view against 2,569.
- **What lies on them,** a topsoil's depth, decided where the ground is asked (`field`): bare rock past 40°, scree from 24 to 38° with a cliff within 45 m uphill, gravel in a gully's bed from 5 to 28°.
- **Steep ground in the octree:** a field says the tangent of its steepest slope (`Field::steepest`), 3 for mountains (72°), 1.5 for hills, so walls are not culled.
- **Checked:** no step anywhere (`the_mountains_never_jump`), gullies only cut down, the shares of valley and crest; `cargo xtask fields` maps the result. On the 30 km world's high ground, 31% of it under 20°, 25% to 30°, 24% to 40°, 21% over (the lake demo's region).
- **Cost, measured** (`cargo run --release -p terra_voxel`, Ryzen 9 5900X, loaded): a 16 m chunk in mountains 8 to 9.5 ms against 3.7 on hills, a point of the mountains 4 to 5 µs; round a viewer in a valley 3,133 chunks chosen, 1,318 with ground, 3.1 million triangles, 90 to 107 s of meshing on one thread, a few seconds across the cores. Over the brief's 6 ms; a cheaper field for far chunks is the next step if the first view's wait or streaming shows it.

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
- Answered: **how edited voxels are stored.** Not as voxels: edits are kept as the operations themselves and laid over the generated field whenever a point is asked about, as D21 already syncs and saves them. Deterministic and small; the cost grows with the number of edits near a point, so a spatial index, and later cached blocks, come when that shows. The original uses 16-bit values in 16-cubed blocks. Digs union as one `min`, which is only a bound inside: two boxes dug face to face would leave a wall of no thickness on the face they share (each box 0 there), which the mover and the mesher would meet, so such boxes also dig the boxes their union holds across that face, whenever each was dug and in frames turned up to 2 cm of lean apart (boxes each square to the ground at its own spot on a curved planet), never through a fill made since (`terra_voxel` planet/seams.rs). Today every sample scans every edit (about 7 ns each): a spatial index is due before edits reach the thousands.
- Whether the `transvoxel` crate's need to re-mesh on detail changes is acceptable (D32).
- How materials are stored. The original packs four material indices and four weights into each voxel. The `transvoxel` crate lets a voxel carry extra data and hands both voxels to the mesh builder, so weights can be interpolated there.
