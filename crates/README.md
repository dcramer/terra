# Crates

The game's code so far. The split below is the working plan and may change. One rule shapes it: anything that must be the same for every player lives in a plain Rust crate that knows nothing about Bevy (D28). Only the applications, `terra_sandbox` and `terra_game`, and the world they stand on, `terra_world` (D198), may depend on it.

| Crate | What it holds | Bevy? | State |
|---|---|---|---|
| `terra_gen` | Generation toolkit: seeds, noise, repeatable random streams | No | Exists |
| `terra_core` | The shared contracts (ground, water, chunk mesh, operations) and the stand-in planet whose hills the terrain uses | No | Exists |
| `terra_voxel` | Terrain: shape and material, chunks at several detail levels, meshing, edits. Brief: [docs/terrain.md](../docs/terrain.md) | No | Exists |
| `terra_build` | Construction: half-metre cubes on a base's grid, the rooms they close in, roofs, and the kits that lay them ([docs/building-tech.md](../docs/building-tech.md)) | No | Exists |
| `terra_build_scenes` | Building's fixtures (D407): the scene registry, the fuzz bases, the kit sheet's rows and the QA cabin, and the checks over them (the mesh checker `cargo xtask artifacts`, the drawing audit, the texel check, the voxel contract) | No | Exists |
| `terra_items` | Items and containers: kinds, the backpack grid, equipment, the action bar. Brief: [docs/items.md](../docs/items.md) | No | Exists |
| `terra_climate` | Climate ([docs/climate.md](../docs/climate.md), D314, D315): the world's clock in days, a world's climate as data, temperature, daylight, precipitation, wind and the air's wetness as fields of a place and a time, and the past the weather leaves (snow, wet ground, ice, warmth and chill) folded per cell | No | Exists |
| `terra_land` | The land by its rules (D331): the world's ground as generated and its climate together, where a place is on the world (its latitude, how its ground leans), regions sampled on a grid, and the field maps, `cargo xtask fields` (D326) | No | Exists |
| `terra_town` | Settlements, laid out and lived in (D407): a market town from its seed and ground (D330), its streets, plots, yards, market and walls, its buildings `terra_build`'s village's, `cargo xtask town`; and people's days in it (D324): schedules by role at smart objects, walking a town's path graph by A* with ORCA avoidance ported from RVO2, and anyone far off placed from the clock; the life check (`cargo xtask life`) | No | Exists |
| `terra_sim` | The planet simulation: cells, fields, what machines do | No | Exists; parked with terraforming (D60) |
| `terra_net` | Network messages and transport | No | Not created yet (track D) |
| `terra_body` | Appearance data into joints and morph weights ([docs/characters.md](../docs/characters.md)) | No | Not created yet |
| `terra_world` | The world on screen in Bevy: the shell every run starts in (window, options, scripted runs, the remote protocol, sound), the planet, terrain, sky and the start, and the world layer (light, the day, what grows, the tick, levels of detail; D198). Builds and tests alone. [README](terra_world/README.md) | Yes | Exists |
| `terra_game` | The game in Bevy, on `terra_world`: the walker and gameplay layers (D198): the player, items, building, creatures, people, the game's sounds, saving | Yes | Exists, launched by the sandbox. [README](terra_game/README.md) |
| `terra_sandbox` | A test environment on top of the game: it launches it, and holds testbeds and developer tools such as the workbench and the icon baker | Yes | Exists. [README](terra_sandbox/README.md) |

Where each part of the engine stands is in [docs/engine.md](../docs/engine.md#tracks). Crates are created when their track starts, not before.

## Adding a crate

1. `cargo new --lib crates/<name>`. The workspace picks it up automatically.
2. In its `Cargo.toml`, inherit `version`, `edition`, `license` and `publish` from the workspace and add `[lints] workspace = true`, as `terra_sim` does.
3. Put shared dependency versions in the root `Cargo.toml` under `[workspace.dependencies]`.
4. Add a row to the table above.
5. Run `cargo xtask check`.
