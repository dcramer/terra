# Crates

The game's code. One rule shapes the split: anything that must be the same for every player lives in a plain Rust crate that knows nothing about Bevy (D28).

| Crate | What it holds | Bevy? | State |
|---|---|---|---|
| `terra_gen` | Generation toolkit: seeds, noise, repeatable random streams | No | Exists |
| `terra_sim` | The planet simulation: cells, fields, what machines do | No | Exists |
| `terra_voxel` | Terrain: shape and material, chunks at several detail levels, meshing, edits | No | Track A. Brief: [docs/terrain.md](../docs/terrain.md) |
| `terra_core` | The shared contracts (ground, water, chunk mesh, operations) and a stand-in planet | No | Exists |
| `terra_net` | Network messages and transport | No | Track D |
| `terra_game` | The Bevy application: drawing, input, player, UI, audio | Yes | Track B |

Crates are created when their track starts, not before. The tracks are in [docs/engine.md](../docs/engine.md).

## Adding a crate

1. `cargo new --lib crates/<name>`. The workspace picks it up automatically.
2. In its `Cargo.toml`, inherit `version`, `edition`, `license` and `publish` from the workspace and add `[lints] workspace = true`, as `terra_sim` does.
3. Put shared dependency versions in the root `Cargo.toml` under `[workspace.dependencies]`.
4. Add a row to the table above.
5. Run `cargo xtask check`.
