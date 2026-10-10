# Tech

Status: provisional, like everything in pre-production. The direction below is our current answer. Everything is written in Rust (D11), on Bevy, with our own voxel terrain (D12). Library choices that have not been exercised yet are marked Leaning in [decisions.md](decisions.md).

The earlier Godot build and its measurements are in [research/godot-spike.md](../research/godot-spike.md). Those numbers are the bar this build has to meet.

## Requirements

- Round planets in a real solar system, seamless from surface to space (D4).
- Fully diggable smooth terrain (D9). Terraforming never reshapes the ground (D20).
- Co-op for up to 4, host-authoritative (D2).
- Performant: 60 fps at 1080p on a mid-range discrete GPU and a 4-core CPU while hosting three other players (D25).
- Developed on Windows via WSL and on macOS (D7), with all code written by agents (D1).

## Stack

| Layer | Choice | Status |
|---|---|---|
| Language | Rust for everything. No scripting language. | Decided |
| Engine framework | Bevy 0.19: entity system, renderer, windowing, input, assets, audio, UI | Decided |
| Graphics | Bevy's renderer on wgpu (Vulkan, Metal, DirectX 12). Our own shaders for ground, sea and sky. | Decided |
| Voxel terrain | Our own crate, ported from the parts of Voxel Tools we need (MIT-licensed) | Decided |
| Terrain meshing | The `transvoxel` crate to start | Leaning |
| Simulation | Our own crate, already written and running on its own thread (D26) | Decided |
| Physics | Avian for rigid bodies; the player walks on voxel data directly | Leaning |
| Networking | Our own messages over `renet`; Steam transport later | Leaning |
| UI | Bevy's UI for the game; `bevy_egui` for developer tools | Leaning |
| Maths in shared state | The `libm` crate for sine and friends, not the system maths library | Decided |
| Saves and messages | `serde` with `postcard` | Leaning |
| Solar-system coordinates | One coordinate frame per planet; `big_space` when a second planet exists | Leaning |

We write the terrain, the simulation, the networking model and the game. We do not write a renderer, an animation system, an asset pipeline or a windowing layer; Bevy supplies those.

## How the code is split

One Cargo workspace. The rule that shapes it: **anything that must be the same for every player lives in a plain Rust crate that knows nothing about Bevy.**

| Crate | What it holds | Depends on Bevy? |
|---|---|---|
| `terra_sim` | The planet simulation: cells, fields, machines' effects | No |
| `terra_voxel` | Terrain: the density field, the tree of chunks at different detail levels, meshing, edits | No |
| `terra_core` | The shared game state and the operations that change it | No |
| `terra_net` | Network messages and the transport wrapper | No |
| `terra_game` | The Bevy application: drawing, input, the player, UI, audio | Yes |

Why this split:

- **Tests are fast and need no GPU.** The plain crates run under `cargo test` in seconds.
- **The host can run without a window.** The same crates serve a headless host.
- **Bevy's releases only touch one crate.** Its API changes every few months; the game's rules do not live there.
- **It was already working.** `terra_sim` moved from Godot to this plan without changing.

The plain crates share one maths library with Bevy (`glam`), so positions pass between them without conversion.

## State, and how co-op shares it

Three kinds of state, each shared differently:

| Kind | Examples | How it is shared |
|---|---|---|
| World changes | Digging, placing a machine | As operations, applied in the host's order by every player (D21) |
| Simulation | Temperature, sea level, life | The host runs it and sends snapshots. It is about 10 thousand cells, so a snapshot is small. |
| Moving things | Players, creatures | The host sends positions; other players smooth between them |
| Local only | Rendering, UI, sound, particles | Not shared |

Only the host runs the simulation, so players never have to compute identical floating-point results for it. Terrain is generated on each machine from the same seed; small differences between machines would shift a mesh by a hair and are harmless. Using `libm` keeps even that from happening.

A save is the seed, the operations, the simulation state and periodic snapshots of edited terrain.

## Terrain

The design follows Voxel Tools, which we measured and know works. Our port needs about a seventh of it; see [research/voxel-tools-port.md](../research/voxel-tools-port.md).

- **Density field.** A Rust function gives the distance to the ground at any point: the planet's large shape from the simulation's terrain waves, plus fine noise.
- **Edits.** Stored as small blocks of changed voxels over the generated terrain, and changed only by operations.
- **Chunk tree.** Each planet has a tree of chunks; chunks near a viewer are fine, distant ones coarse. A host tracks up to four viewers.
- **Meshing.** Runs on worker threads, nearest chunks first. Finished meshes are handed to Bevy a few per frame.
- **Seams.** Neighbouring chunks at different detail levels are stitched with the Transvoxel method.
- **Collision.** The player and creatures query voxel data directly, so they never wait for a collision mesh. Rigid bodies get collision meshes for the finest chunks near them.

While porting, the original keeps running headless next to the port, and the two are compared on the same density function.

One known gap: the `transvoxel` crate bakes seam stitching into each mesh, so a chunk must be re-meshed when a neighbour changes detail level. Voxel Tools avoids that by switching seams in the shader. If re-meshing proves costly we port its mesher too (about 4,600 lines).

## Rendering

- **Ground:** one material. Colour comes from the simulation as a map of the whole planet, refreshed a few times a second, with fine detail added in the shader.
- **Sea:** one sphere per planet at the simulation's sea level.
- **Sky:** driven by air pressure. Bevy has built-in atmospheric scattering; whether it works on a 2 km planet seen from orbit is untested, and our own sky shader from the Godot demo is the fallback.
- **Plants and objects:** instanced. The Godot load test showed instancing is limited by the GPU, not the engine.

## Development workflow

- **One command tests everything:** `cargo test`, no GPU needed.
- **Agents can see their own output anywhere.** The Linux build renders under WSL's software renderer, slowly but correctly, so screenshots do not need the Windows GPU.
- **Performance is measured on real GPUs.** The Windows build is cross-compiled from WSL and launched from there; macOS builds natively.
- **Every binary has scripted modes** that fly a camera path, print frame times and save screenshots, as the Godot demo did.
- **Versions are pinned:** the Rust toolchain, Bevy, and every dependency through the lock file.
- **Bevy upgrades are deliberate.** We move to a new release once the crates we depend on support it. Bevy 0.20 is in release-candidate now; we stay on 0.19 until they do.

## What has been checked

A throwaway app proved the toolchain end to end on 2026-10-06: Bevy 0.19 and `transvoxel` build from WSL for Linux and Windows, render under WSL with no GPU, and run on the RTX 5090 at a median 1.8 ms a frame. Details and numbers are in [research/rust-stack-spike.md](../research/rust-stack-spike.md).

The workspace, the pinned compiler and the checks exist (see [development.md](development.md)). Of the crates, only `terra_sim` exists. The terrain port is next; its brief is [terrain.md](terrain.md).

## Risks

| Risk | What we do about it |
|---|---|
| The terrain port takes longer than hoped, or is fragile | It is the first milestone, tested against the original, and measured against the Godot numbers before anything is built on it |
| Bevy changes under us | Pinned version, one crate exposed to it, upgrades on our schedule |
| A library we lean on falls behind | `big_space` already trails by one Bevy release; each such crate is either small enough to fork or not on the critical path |
| Bevy's UI and animation are younger than Godot's | Neither is needed before the terrain and co-op milestones; reassess then |
| Performance differs between the Windows build we cross-compile and a native one | Measure every benchmark on Windows, not only Linux; switch to the MSVC toolchain if the gap matters |

## What happens to the Godot build

`demo/`, `spike/` and `prototypes/sim_toy/` stay as they are until the Rust build passes the same checks. They are the reference for behaviour and the source of the numbers to beat. `terra_sim` moves into the new workspace unchanged.

## Sources

- [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/)
- [Bevy 0.18 release notes](https://bevy.org/news/bevy-0-18/)
- [transvoxel crate](https://gnurfos.github.io/transvoxel_rs/)
- [Voxel Tools](https://github.com/Zylann/godot_voxel)
- [Avian](https://github.com/avianphysics/avian)
- [big_space](https://github.com/aevyrie/big_space)
