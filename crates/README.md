# Crates

The game's code so far. The split below is the working plan and may change. One rule shapes it: anything that must be the same for every player lives in a plain Rust crate that knows nothing about Bevy (D28). Only the applications, `terra_sandbox` and `terra_game`, may depend on it.

| Crate | What it holds | Bevy? | State |
|---|---|---|---|
| `terra_gen` | Generation toolkit: seeds, noise, repeatable random streams | No | Exists |
| `terra_sim` | The planet simulation: cells, fields, what machines do | No | Exists; parked with terraforming (D60) |
| `terra_voxel` | Terrain: shape and material, chunks at several detail levels, meshing, edits | No | Terrain A1 built: the field, edits, harvest, meshing and the tree of chunks. A2, drawing it, next. Brief: [docs/terrain.md](../docs/terrain.md) |
| `terra_core` | The shared contracts (ground, water, chunk mesh, operations) and a stand-in planet | No | Exists |
| `terra_build` | Construction: bases shaped room by room, with the hull, windows and doors generated | No | Exists, as an experiment |
| `terra_items` | Items and containers: kinds, the backpack grid, equipment, the action bar. Brief: [docs/items.md](../docs/items.md) | No | Exists |
| `terra_net` | Network messages and transport | No | Track D |
| `terra_sandbox` | A test environment: window, stand-in planet, free camera, live tweaks, experiments | Yes | Exists. [README](terra_sandbox/README.md) |
| `terra_game` | The Bevy application: drawing, input, player, UI, audio | Yes | Track B |

Crates are created when their track starts, not before. The tracks are in [docs/engine.md](../docs/engine.md).

## Adding a crate

1. `cargo new --lib crates/<name>`. The workspace picks it up automatically.
2. In its `Cargo.toml`, inherit `version`, `edition`, `license` and `publish` from the workspace and add `[lints] workspace = true`, as `terra_sim` does.
3. Put shared dependency versions in the root `Cargo.toml` under `[workspace.dependencies]`.
4. Add a row to the table above.
5. Run `cargo xtask check`.
