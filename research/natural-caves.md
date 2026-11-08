# Natural caves: how shipped games make them, and what fits our ground

2026-11-08. Question: D591 puts natural caves in the world's generation and in v0, one of them the troll's, fitting a 4 m body and a fight and reachable from the surface on every seed. How do shipped games and engines make caves in generated ground, and which way fits ours?

## Answer

Carve **cave systems placed by rule** out of the ground's distance field: each a few chambers and passages (ellipsoids and capsules along a bent path, roughened by noise) with a mouth on a hillside, found from the seed as the gate is (D641), subtracted from the ground as a signed distance. Not a 3D noise threshold through the whole crust. Proposed as D643.

## What was read

| Source | How it makes caves | What it costs or gives |
|---|---|---|
| Minecraft 1.18's noise caves ([custom world generation](https://minecraft.wiki/w/Tutorial%3ACustom_world_generation), the wiki; code proprietary, docs only) | Branches of the world's one density function: "cheese" where a 3D noise is below zero, "spaghetti" and "noodle" tunnels from other noises, joined to the land by min and add, tapered near the world's floor | Endless and unplanned; every cell of the crust is asked several 3D noises; nothing says a cave is reached or what fits in it |
| Minecraft's older carvers (the same wiki; a Mojang [bug report](https://bugs-legacy.mojang.com/browse/MC-240516) tells the two apart) | Tunnels placed from a chunk's seed and carved after the land is made | Bounded and placed; kept in 1.18 beside the noise caves |
| Voxel Tools for Godot ([procedural generation](https://voxel-tools.readthedocs.io/en/latest/procedural_generation/), [graph nodes](https://voxel-tools.readthedocs.io/en/latest/graph_nodes/); MIT, so its code may be ported, D196) | The terrain's SDF minus a cave SDF by `SdfSmoothSubtract`; its example caves are a noise thickened along the height | The subtraction is the part to port: a smooth minimum of two distances, which keeps the result a distance |
| Valheim's troll caves ([Jötunn's zones](https://valheim-modding.github.io/Jotunn/tutorials/zones.html), a modding guide, not the studio's) | A location: of each 64 m zone, at most one, chosen from its biome's list when the world is made, kept with the save; the cave itself is a made place behind a mouth | A boss's cave is guaranteed by placing it, never by hoping noise makes one |
| Deep Rock Galactic (community write-ups of the studio's talks, [one](https://devtrackers.gg/deep-rock-galactic/p/8cd5ca9c-below-decks-at-ghost-ship-cave-generation-in-deep-rock-galactic); no first-hand source found) | Solid rock, rooms authored as clusters of shapes, a random layout joining them, all carved out | Rooms that play well, joined by rule; the nearest shipped thing to "systems placed by rule" |

## What was read in our code

- The ground is a signed distance (`terra_voxel::Field`): a generated planet's is its height along the point's direction (`GeneratedPlanet::distance_in`), so today nothing is hollow.
- Which chunks are meshed is decided from that distance at a chunk's middle (`lod::may_hold_surface`): within the chunk's reach times the ground's steepest slope. A carve that keeps the field a distance needs no other rule: a chunk by a cave's wall is near a surface and is meshed, one in solid rock is far from any and is not. A noise threshold is not a distance and would need every chunk of the crust asked.
- The mover walks the same field, so a carved cave is walked with no further work; the sun's shadow and the sky's light underground, and the camera in a tight place, are separate work (v0-tracks, Land).

## Why placed systems, not noise through the crust

- **The troll's cave must exist, fit and be reached on every seed** (D591): a placed system is made to; a noise cave is found by luck, and a sweep can only reject seeds.
- **Hundreds of variants, not millions** (D182): a system is made of parts by rules (how many chambers, how wide, how deep, which mouth), a theme choosing; a player tells a troll's den from a crawl.
- **Cost**: a system's parts are a handful of ellipsoids and capsules found through a grid; rock far from any costs one lookup. Noise caves cost 3D noise at every voxel of every chunk near the surface.
- **Streaming**: as above, the chunk rule holds unchanged.

Noise caves stay open for a later theme that wants a honeycombed world; nothing here rules them out.

## Limits

- Read from documentation and modding guides, not the games' code; Deep Rock Galactic's method is second-hand.
- Nothing was measured: the cost of the carve in `distance_in`, and whether the chunk rule holds at a cave's mouth on steep ground, are for the first build to measure.
- Light underground and the camera in tight space are not looked into here.
