# Tech

Status: provisional, like everything in pre-production. The direction below is our current answer. Library choices that have not been exercised yet are marked Leaning in [decisions.md](decisions.md).

Terra is first and foremost a sandbox engine (D187): many procedural worlds, and a system for each part of the game acting on the others, so that the game is what they make together, for up to a thousand players (D52). Everything is written in Rust (D11), on Bevy, with our own voxel terrain (D12). The parts and where each stands are in [engine.md](engine.md); how to work in the repository is in [development.md](development.md). The Godot build that came first, and its numbers, still the bar (D12), are in [research/godot-spike.md](../research/godot-spike.md); the check of this toolchain is [research/rust-stack-spike.md](../research/rust-stack-spike.md).

## Requirements

- Many round worlds with no space between them, reached probably by portals (D4, D72).
- Fully diggable smooth terrain (D9). Only players reshape the ground (D20).
- Multiplayer built around a server that decides everything (D51), toward a thousand players on one persistent server (D52).
- Performant: 60 fps at 1080p on a mid-range discrete GPU and a 4-core CPU, as a client of a server holding a thousand players, with dozens of them in view (D25).
- Developed on Windows via WSL and on macOS (D7), with all code written by agents (D1).

## Stack

| Layer | Choice | Status |
|---|---|---|
| Language | Rust for the game; TypeScript on Node for tooling outside it (D121). No scripting language in the game. | Decided |
| Engine framework | Bevy 0.19: entity system, renderer, windowing, input, assets, audio, UI | Decided |
| Graphics | Bevy's renderer on wgpu (Vulkan, Metal, DirectX 12). Our own shaders for ground, sea and sky. | Decided |
| Voxel terrain | Our own crate, ported from the parts of Voxel Tools we need (MIT-licensed) | Decided |
| Terrain meshing | The `transvoxel` crate | Decided (D32) |
| Simulation | Our own crate, `terra_sim`, on its own thread; parked with terraforming (D26, D60) | Parked |
| Physics | Avian for rigid bodies; the player walks on voxel data directly | Leaning (D33) |
| Networking | Our own messages over `renet`; Steam transport later | Leaning (D34) |
| UI | Bevy's UI for the game; `bevy_egui` for developer tools | Leaning (D35) |
| Sound | Bevy's audio; ambience and music generated in code; recorded clips in named sets ([audio.md](audio.md)) | Decided (D113) |
| Maths in shared state | The `libm` crate for sine and friends, not the system maths library | Decided (D30) |
| Saves | `serde` with `ron`; network messages not chosen yet | Leaning |
| Coordinates | One frame per world, relative to a moving origin; no `big_space` | Leaning (D36) |

We write the terrain, the networking model and the game. We do not write a renderer, an animation system, an asset pipeline or a windowing layer; Bevy supplies those.

## How the code is split

One Cargo workspace. The rule that shapes it: **anything that must be the same for every player lives in a plain Rust crate that knows nothing about Bevy** (D28). Only the applications, `terra_game` and `terra_sandbox`, may depend on Bevy, and `cargo xtask check` enforces it. The crates are listed in [crates/README.md](../crates/README.md).

Why this split:

- **Tests are fast and need no GPU.** The plain crates run under `cargo xtask check` in seconds.
- **The server can run without a window.** The same crates are the server, whether it runs inside a player's game or alone on another machine.
- **Bevy's releases only touch the applications.** Its API changes every few months; the game's rules do not live there.

The plain crates share one maths library with Bevy (`glam`), so positions pass between them without conversion.

## State, and how it is shared

The game is server-first (D51): it always talks to a server, which decides everything. When friends play together, that server runs inside one player's game. On a persistent world it runs alone on another machine. Both are the same code. What a thousand players ask of it is in [multiplayer.md](multiplayer.md).

| Kind | Examples | How it is shared |
|---|---|---|
| World changes | Digging, building | As operations, applied in the server's order by every player (D21) |
| Moving things | Players, creatures | The server sends positions; players smooth between them |
| Local only | Rendering, UI, sound, particles | Not shared |

Terrain and everything placed by rule are generated on each machine from the same seed; using `libm` keeps machines from differing even by a hair. A save is the seed and the operations, with snapshots of edited areas once there are very many edits.

## Terrain

The design follows Voxel Tools, which we measured and know works. Our port needs about a seventh of it; see [research/voxel-tools-port.md](../research/voxel-tools-port.md). The brief is [terrain.md](terrain.md).

- **Density field.** A Rust function gives the distance to the ground at any point: the world's large shape from its seed, plus fine noise.
- **Edits.** Kept as the operations themselves, laid over the generated terrain (D21).
- **Chunk tree.** Each world has a tree of chunks; chunks near a viewer are fine, distant ones coarse. The server tracks one viewer per player.
- **Meshing.** Runs on worker threads, nearest chunks first. Finished meshes are handed to Bevy a few per frame.
- **Seams.** Neighbouring chunks at different detail levels are stitched with the Transvoxel method.
- **Collision.** The player and creatures query voxel data directly, so they never wait for a collision mesh. Rigid bodies would get collision meshes for the finest chunks near them (D33, D53).

One known gap: the `transvoxel` crate bakes seam stitching into each mesh, so a chunk must be re-meshed when a neighbour changes detail level. Voxel Tools avoids that by switching seams in the shader. If re-meshing proves costly we port its mesher too (about 4,600 lines).

## Rendering

- **Ground:** one material, blending the voxel materials, with fine detail added in the shader.
- **Sea:** one sphere per world at its sea level.
- **Sky:** our own sky shader.
- **Plants and objects:** instanced and baked together by area (D47).

## Risks

| Risk | What we do about it |
|---|---|
| Bevy changes under us | Pinned version, only the applications exposed to it, upgrades on our schedule (D31) |
| A library we lean on falls behind | `big_space` already trails by one Bevy release; each such crate is either small enough to fork or not on the critical path |
| Bevy's UI and animation are younger than other engines' | Reassess as the game needs real menus and more animation |
| A thousand players is far beyond anything measured | Every decision is tested against it (D52); a handful of players work first, then larger worlds are measured (D51) |
| Performance differs between the Windows build we cross-compile and a native one | Measure every benchmark on Windows, not only Linux; switch to the MSVC toolchain if the gap matters |

## Sources

- [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/)
- [transvoxel crate](https://gnurfos.github.io/transvoxel_rs/)
- [Voxel Tools](https://github.com/Zylann/godot_voxel)
- [Avian](https://github.com/avianphysics/avian)
- [big_space](https://github.com/aevyrie/big_space)
