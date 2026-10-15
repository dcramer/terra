# Tech

Status: provisional, like everything in pre-production. The direction below is our current answer. Library choices that have not been exercised yet are marked Leaning in [decisions.md](decisions.md).

First and foremost, we are building an extremely ambitious sandbox engine, with multitudes of procedural worlds and intricate systems for each component of the game (D187). Everything is written in Rust (D11), on Bevy, with our own voxel terrain (D12). The parts and where each stands are in [engine.md](engine.md); how to work in the repository is in [development.md](development.md). The Godot build that came first, and its numbers, still the bar (D12), are in [research/godot-spike.md](../research/godot-spike.md); the check of this toolchain is [research/rust-stack-spike.md](../research/rust-stack-spike.md).

## Requirements

- Many round worlds with no space between them, reached probably by portals (D4, D72).
- Fully diggable smooth terrain (D9). Only players reshape the ground (D20).
- Multiplayer built around a server that decides everything (D51), toward a thousand players on one persistent server (D52).
- Performant: 75 fps comfortably at 1080p on the developers' RTX 3080 at the high preset ([the frame budget](development.md#measuring-performance)); 60 fps at 1080p on a mid-range discrete GPU and a 4-core CPU, as a client of a server holding a thousand players, with dozens of them in view (D25).
- Developed on Windows via WSL and on macOS (D7), with all code written by agents (D1).

## Stack

| Layer | Choice | Status |
|---|---|---|
| Language | Rust for the game; TypeScript on Node for tooling outside it (D121). No scripting language in the game. | Decided |
| Engine framework | Bevy 0.20: entity system, renderer, windowing, input, assets, audio, UI | Decided |
| Graphics | Bevy's renderer on wgpu (Vulkan, Metal, DirectX 12). Our own shaders for ground, sea, plants and clouds; Bevy's atmosphere for the sky. | Decided |
| Voxel terrain | Our own crate, ported from the parts of Voxel Tools we need (MIT-licensed) | Decided |
| Terrain meshing | The `transvoxel` crate | Decided (D32) |
| Simulation | Our own crate, `terra_sim`, on its own thread; parked with terraforming (D26, D60) | Parked |
| Climate | Our own crate, `terra_climate`: fields of a place and a time, and the past the weather leaves, per cell ([climate.md](climate.md)) | Decided (D314, D315) |
| Physics | Avian for rigid bodies; the player walks on voxel data directly | Leaning (D33) |
| Networking | Our own messages over `renet`; Steam transport later | Leaning (D34) |
| UI | Bevy's UI for the game; `bevy_egui` for developer tools | Leaning (D35) |
| Sound | Bevy's audio; ambience and music generated in code; recorded clips in named sets ([audio.md](audio.md)) | Decided (D113) |
| Maths in shared state | The `libm` crate for sine and friends, not the system maths library | Decided (D30) |
| Saves | `serde` with `ron`; network messages not chosen yet | Leaning |
| Coordinates | One frame per world, relative to a moving origin; no `big_space` | Leaning (D36) |

We write the terrain, the networking model and the game. We do not write a renderer, an animation system, an asset pipeline or a windowing layer; Bevy supplies those.

## How the code is split

One Cargo workspace. The rule that shapes it: **anything that must be the same for every player lives in a plain Rust crate that knows nothing about Bevy** (D28). Only the applications, `terra_game` and `terra_sandbox`, may depend on Bevy, and `terra_world`, the world on screen they stand on (D198); `cargo xtask check` enforces it. The crates are listed in [crates/README.md](../crates/README.md).

Why this split:

- **Tests are fast and need no GPU.** The plain crates run under `cargo xtask check` in seconds.
- **The server can run without a window.** The same crates are the server, whether it runs inside a player's game or alone on another machine.
- **Bevy's releases only touch the applications.** Its API changes every few months; the game's rules do not live there.

The plain crates share one maths library with Bevy (`glam`), so positions pass between them without conversion.

### Layers of the game (D198)

The game is three Bevy plugins, each standing only on those before it, bundled as the plugin group `GamePlugins`. The world is its own crate, `terra_world`, which builds, tests and runs without the game; the walker and gameplay are `terra_game`, on top of it:

| Layer | Plugin | What it holds |
|---|---|---|
| World (`terra_world`) | `WorldPlugin` | Light and air, the day, what grows and its data file, the tick, levels of detail. Under it, always on: the shell every run starts in, sound, the planet, the terrain and **the start**, the fixed place every east-and-south position is measured from |
| Walker (`terra_game`) | `WalkerPlugin` | A body this machine steers, its camera, the controls and the options menu |
| Gameplay (`terra_game`) | `GameplayPlugin` | Everything a player does: building, items, crafting, creatures, people, combat, survival, the map, saving, the remote probe, the game's sounds |

The full game turns on all three. A run can leave layers out to build and test the ones below alone: the sandbox's `walk` is the world and the walker, a walk through the forest with nothing else of the game (`GamePlugins.build().disable::<GameplayPlugin>()`), and `cargo run -p terra_world --example world` is the world alone, compiled without the game.

The layers are big on purpose: a few large, pluggable chunks, not a plugin per feature to pick from. Each feature is still its own plugin module inside its layer (AGENTS.md).

How a lower layer stays free of a higher one, in the usual Bevy ways:

- **It never orders itself by a system above it.** A higher layer orders itself against a lower one's system sets (`PlayerInput`, `MenuSystems`, `Grading`), never the other way.
- **It publishes small types for the layer above to fill in.** The walker owns `Hands` (what the player's pointer, mouse, wheel and hands are on), `ArriveAt` and `WarpTo`; gameplay's screens and tools write them.
- **It adds what it owns when an entity appears, with observers.** The walker spawns the body; gameplay adds what a player carries and feels when a `LocalPlayer` is added.
- **It owns the small types the layers above meet it through.** The world has `Footprint` (where something built stands, which flora keeps out of; the base keeps its own up to date), `Viewpoint` (what detail is graded from; a player requires one) and flora's kinds, which name the items they give by name (gameplay's `FloraGives` makes them items).
- **What may be missing is optional.** A query over another layer's components is simply empty without it; a resource another layer may provide is `Option<Res<_>>`, or the system runs only when it exists (`run_if(resource_exists::<_>)`).

#### Where shared types go

In the lowest layer that has to understand them, defined by the side that asks the question; never in a "common" or "types" crate, where everything ends up because it is easy and then everything depends on everything.

| Kind of type | Goes in | Examples |
|---|---|---|
| Rules every player must agree on, plain Rust | The engine-free crate of their domain (D28) | `terra_core`'s ground and operations, `terra_items`' `ItemKind`, `terra_build`'s `Base` |
| A Bevy type a lower layer asks the layers above to fill in | The lower layer, which owns the question | `terra_world`'s `Footprint` (flora asks what is built here) and `Viewpoint`; the walker's `Hands` (the camera asks what is on screen), `ArriveAt`, `WarpTo` |
| A type one layer alone uses | That layer's own module | Gameplay's `FloraGives`, `Site` |

Two parts of one layer that share a type: the part that defines the idea keeps it and the other imports it; it moves down only when a lower layer needs it too. The player's own components (`Player`, `Motion`, `Stance`, `View`, `Downed`) are the walker's, which spawns and moves the body. Bevy itself works this way: `bevy_render` defines what `bevy_pbr` fills in.

Cargo keeps the world below the game, since `terra_game` depends on `terra_world`. The walker shares `terra_game` with gameplay and still names a few gameplay types, though it runs without them: movement and the camera ask the player's building (`Site`, `Built`) what to stand on and bump into, and movement asks props, other bodies and stamina (`Prop`, `Body`, `Vitals`) what is in the way and what a jump costs. Giving the walker its own crate would mean turning those into shared types (a "solid" that stops a walker, a "surface" that holds one up) and taking the character's look (`appearance`, `character`, `skinning`) with it; it is not needed yet.

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
- **Sky and air:** Bevy's physical atmosphere for the sky's colour and the haze, with our own shader drawing clouds, stars, the moon and the dusk over it (D212), and a world's far land below the horizon in the air's own colours (D273).
- **Plants and objects:** instanced and baked together by area (D47).
- **Quality presets:** low, medium and high, chosen from the GPU and overridable; low draws at three fifths of the screen's size and scales up, with cheaper shadows, no screen-space shade, haze shafts or bloom, and detail and grass nearer (D239). Crossfades between levels of detail are dithered for TAA on every material (D240).

## Risks

| Risk | What we do about it |
|---|---|
| Bevy changes under us | Pinned version, only the applications exposed to it, upgrades on our schedule (D31) |
| A library we lean on falls behind | `big_space` already trails by one Bevy release; each such crate is either small enough to fork or not on the critical path |
| Bevy's UI and animation are younger than other engines' | Reassess as the game needs real menus and more animation |
| A thousand players is far beyond anything measured | Every decision is tested against it (D52); a handful of players work first, then larger worlds are measured (D51) |
| Performance differs between the Windows build we cross-compile and a native one | Measure every benchmark on Windows, not only Linux; switch to the MSVC toolchain if the gap matters |

## Sources

- [Bevy 0.20 release notes](https://bevy.org/news/bevy-0-20/) and [migration guide](https://bevy.org/learn/migration-guides/0-19-to-0-20/)
- [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/)
- [transvoxel crate](https://gnurfos.github.io/transvoxel_rs/)
- [Voxel Tools](https://github.com/Zylann/godot_voxel)
- [Avian](https://github.com/avianphysics/avian)
- [big_space](https://github.com/aevyrie/big_space)
