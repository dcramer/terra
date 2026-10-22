# Crates

The game's code so far. The split below is the working plan and may change. One rule shapes it: anything that must be the same for every player lives in a plain Rust crate that knows nothing about Bevy (D28). Only the applications, `terra_sandbox` and `terra_game`, the world they stand on, `terra_world` (D198), and the crates cut from the game (D407), `terra_bases`, `terra_walker`, `terra_survival`, `terra_creatures` and `terra_folk` so far, may depend on it.

| Crate | What it holds | Bevy? | State |
|---|---|---|---|
| `terra_gen` | Generation toolkit: seeds, noise, repeatable random streams | No | Exists |
| `terra_core` | The shared contracts (ground, water, chunk mesh, operations) and the stand-in planet whose hills the terrain uses | No | Exists |
| `terra_voxel` | Terrain: shape and material, chunks at several detail levels, meshing, edits. Brief: [docs/terrain.md](../docs/terrain.md) | No | Exists |
| `terra_build` | Construction's rules (D407): half-metre cubes on a base's grid, the operations that change them and their checks, support, the rooms they close in, roofs, plans, costs, loss, saves and the kits' data ([docs/building-tech.md](../docs/building-tech.md)) | No | Exists |
| `terra_build_layout` | Building laid out (D407): where a base's pieces go by its kits' rules (courses, blocks, frames, trims, floors, stairs, roofs), how doors and shutters hang in their holes, and walking a base; the rules tests (`cargo xtask rules`) | No | Exists |
| `terra_build_mesh` | Building drawn (D407): a base's laid pieces made into surfaces by material, a section at a time at each detail, dressed, worn and broken into fragments; the kit checker (`cargo xtask kits`), the packs' checks and the mesh inspector | No | Exists |
| `terra_build_scenes` | Building's fixtures (D407): the scene registry, the fuzz bases, the kit sheet's rows and the QA cabin, and the checks over them (the mesh checker `cargo xtask artifacts`, the drawing audit, the texel check, the voxel contract) | No | Exists |
| `terra_items` | Items and containers: kinds, the backpack grid, equipment, the action bar. Brief: [docs/items.md](../docs/items.md) | No | Exists |
| `terra_climate` | Climate ([docs/climate.md](../docs/climate.md), D314, D315): the world's clock in days, a world's climate as data, temperature, daylight, precipitation, wind and the air's wetness as fields of a place and a time, and the past the weather leaves (snow, wet ground, ice, warmth and chill) folded per cell | No | Exists |
| `terra_land` | The land by its rules (D331): the world's ground as generated and its climate together, where a place is on the world (its latitude, how its ground leans), regions sampled on a grid, and the field maps, `cargo xtask fields` (D326) | No | Exists |
| `terra_town` | Settlements, laid out and lived in (D407): a market town from its seed and ground (D330), its streets, plots, yards, market and walls, and its buildings (`village`: plans, schemes, roofs and the settlement's palette, put up by building's rules), `cargo xtask town`; and people's days in it (D324): schedules by role at smart objects, walking a town's path graph by A* with ORCA avoidance ported from RVO2, and anyone far off placed from the clock; the life check (`cargo xtask life`) | No | Exists |
| `terra_sim` | The planet simulation: cells, fields, what machines do | No | Exists; parked with terraforming (D60) |
| `terra_net` | Network messages and transport | No | Not created yet (track D) |
| `terra_body` | Appearance data into joints and morph weights ([docs/characters.md](../docs/characters.md)) | No | Not created yet |
| `terra_bake` | The texture bake (D188), `cargo xtask cook`: models', materials' and the fire's pictures cooked into BC7, BC5 and BC4 KTX2 with mipmaps into the cache `terra_core::cooked` names, read by the game where fresh | No | Exists |
| `terra_world` | The world on screen in Bevy: the shell every run starts in (window, options, scripted runs, the remote protocol, sound), the planet, terrain, sky and the start, and the world layer (light, the day, what grows, the tick, levels of detail; D198). Builds and tests alone. [README](terra_world/README.md) | Yes | Exists |
| `terra_bases` | The base on screen in Bevy, on `terra_world` (D407): a base's site, its kits, its acts applied, and its drawing (sections laid and merged, texture arrays, detail by distance, merged cells far off, rooms' light, debug views) and memory; what the walker reads of the hammer. Re-exported by the game as `build`. [README](terra_bases/README.md) | Yes | Exists |
| `terra_walker` | The walker layer in Bevy (D198), on `terra_world` and `terra_bases` (D407): a body this machine steers, its camera, controls, options menu and figure, and the character's look (appearance, skinning, lift, swing, pace). Re-exported by the game at its old paths (`play::walker`, `appearance`, ...). [README](terra_walker/README.md) | Yes | Exists |
| `terra_survival` | What a player carries, makes and needs, in Bevy, on `terra_walker` (D407): the inventory, bag and storage, the catalogue and crafting at stations, props set down, claims, harvesting, digging and work, vitals, warmth and death, skills, wear, what is held and worn, and the item icons. Re-exported by the game at its old paths (`play::inventory`, `icons`, ...). [README](terra_survival/README.md) | Yes | Exists |
| `terra_creatures` | Creatures and fighting, in Bevy, on `terra_survival` (D407): the kinds of creature and the creatures, wildlife, fliers, seasonal coats, staged figures, and both sides of a fight: hit zones, sword and shield, the bow, combat and how a blow is shown. Re-exported by the game at its old paths (`play::creatures`, ...). [README](terra_creatures/README.md) | Yes | Exists |
| `terra_folk` | Townsfolk, in Bevy, on `terra_creatures` (D407): people, the first quest's settlement and the place where magic pools, a market town laid out and living its days, its people's bodies, animals, fences, stalls, gardens and murmur, and a town rising as a time-lapse with its scaffolds. Re-exported by the game at its old paths (`play::town`, ...). [README](terra_folk/README.md) | Yes | Exists |
| `terra_game` | The game in Bevy, on `terra_world`: the rest of the gameplay layer (D198) and the app: building play, the game's sounds, saving | Yes | Exists, launched by the sandbox. [README](terra_game/README.md) |
| `terra_sandbox` | A test environment on top of the game: it launches it, and holds testbeds and developer tools such as the workbench and the icon baker | Yes | Exists. [README](terra_sandbox/README.md) |

Where each part of the engine stands is in [docs/engine.md](../docs/engine.md#tracks). Crates are created when their track starts, not before.

## Adding a crate

1. `cargo new --lib crates/<name>`. The workspace picks it up automatically.
2. In its `Cargo.toml`, inherit `version`, `edition`, `license` and `publish` from the workspace and add `[lints] workspace = true`, as `terra_sim` does.
3. Put shared dependency versions in the root `Cargo.toml` under `[workspace.dependencies]`.
4. Add a row to the table above.
5. Run `cargo xtask check`.
