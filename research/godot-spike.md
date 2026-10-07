# Godot spike: what we tested and learned

A record, not the current plan. From 2026-10-04 to 2026-10-05 the project was built on Godot 4 with the Voxel Tools module. On 2026-10-06 the developers chose to build everything in Rust instead; the current plan is in [tech.md](../docs/tech.md).

This is kept because its measurements are the bar the Rust build has to meet, and because the Godot build in `demo/` and `spike/` stays as a working reference until the Rust build replaces it.

## Engine evaluation

Judged against D11: simplest, agent-friendly, existing tech to plug in.

| Option | Existing planet tech | Agent-friendliness | Everything else (UI, animation, physics, audio, netcode) | Main risk |
|---|---|---|---|---|
| **Godot 4 + Voxel Tools** | Smooth terrain with level of detail, digging, saved edits, foliage instancing, and a demo with diggable planets and ground-to-space flight | Text scenes and scripts, headless command line, large training corpus; weak compile-time checking | Built in | No multiplayer for the level-of-detail terrain; needs a custom engine build or the extension build; no browser |
| Rust + Bevy | Building blocks only: origin shifting, surface meshing, hobby planet projects | Code only, strong compiler checks; API changes every release, slow compiles | Immature or third-party | We write the planet terrain system ourselves, which is the hardest piece of tech in the game |
| Unreal + Voxel Plugin | Most complete; double-precision coordinates built in | Poor: editor-driven, heavy | Built in | Bad fit for LLM-written code and for WSL |
| Unity | Scattered store assets | Poor: editor-driven | Built in | Same |
| TypeScript + Three.js | None | Good | None | Rejected (D19) |

**Decision (D12): Godot 4 with Voxel Tools, pinned to Voxel Tools 1.7 on Godot 4.7.2.** The hardest tech in the design already exists there, the rest of the engine is complete, and the spike passed on the questions that could have disqualified it. Bevy is the fallback.

What would reopen it:

- A Mac and a PC generating different terrain from the same seed, with no workaround.
- A blocking bug in the module that we can't patch or avoid. It is one maintainer's project, so we should expect to carry our own fixes.
- Needing planets large enough to require a double-precision engine build that turns out not to work.

### What was checked on 2026-10-04

- Voxel Tools targets Godot 4, ships as a C++ module with prebuilt editor binaries for Windows, Linux and macOS, and has an extension build that its roadmap still lists as not fully functional.
- It provides smooth terrain with level of detail, real-time editing and an instancing system for foliage.
- **Its multiplayer support covers the fixed-detail terrain only. For the level-of-detail terrain a planet needs, the docs say "There is no support for now, but it is planned."** Our edits-as-operations model doesn't depend on it, but replication is ours to build, including sending the edit history to a player who joins late.
- The solar-system demo has planets of 1 to 2 km radius (10 to 20 km in a large mode), saved terrain edits, ground-to-space flight and origin shifting across a system about 50 km wide. Its README says physics is buggy at the large scale without a double-precision engine build. It has no multiplayer.
- Godot's official web export is still WebGL2-only as of 4.7, and the demo expects Vulkan. Choosing Godot means giving up the browser for now.

### Track record

- **Godot** has shipped serious commercial games, including a 3D one in our genre: Road to Vostok, a first-person survival game by a solo developer, and Slay the Spire 2. Nobody has shipped a large seamless voxel world on it that we know of.
- **Bevy** has a thin record: small Steam releases, no mainstream hit, and its best-known title moved to a custom engine.
- No off-the-shelf engine is proven for this kind of game. Shipped voxel-planet games mostly run on custom engines or custom voxel tech. The risk sits in the voxel layer, and here that layer is one maintainer's module.

### Spike findings, 2026-10-04

First pass only: get the stock demo running on the Windows developer's machine. See [spike/README.md](../spike/README.md).

- The Voxel Tools 1.7 editor build (Godot 4.7.2) runs headless in WSL and imports the demo. An agent can build-check the project with no GUI.
- WSL has no hardware Vulkan, only a software renderer. Visual runs need the Windows build, launched from WSL, which used the RTX 5090.
- The demo's SQLite terrain saves fail with "database is locked" when the project is opened over the `\\wsl.localhost` path. They work on a Windows drive. Our own saves should go to the user data folder, which would avoid this; not yet verified.
- The demo is about nine months older than the module and did not run unmodified:
  - two script parse errors under Godot 4.7 (missing return statements), patched;
  - with GPU detail rendering, the default, it hangs on the loading screen: the module's GPU task queue grows to about 880 and never drains;
  - with CPU detail rendering, the module hits a fatal assertion in its detail-rendering code;
  - with detail rendering off, it loads and renders a planet from space.

The hang and the crash are yellow flags for the module's robustness, not a verdict. Detail rendering is an optional visual feature and the demo may simply be stale.

### Spike findings by question, 2026-10-04

Tests live in [spike/planet_test](../spike/planet_test). They run headless on a bare 200 m planet, so they check voxel data, not visuals.

**1. Co-op digging: passed.** A host, a client that digs, and a client that joins late all ended with byte-identical voxel data. The host is authoritative and sends each dig as an operation (centre and radius); every peer generates the same planet and applies the same operations. The module's missing multiplayer support for this terrain type did not matter, because we never send voxel data.

Limits of that result:

- All three processes ran the same binary on one machine. A Mac (ARM) and a Windows PC (x86) may generate slightly different terrain from the same seed. Untested, and it is the real co-op case for this team.
- A late joiner replays the whole log, at about 1.7 ms per dig. That is fine for thousands of digs and too slow for hundreds of thousands, so a long-lived world needs snapshots of edited blocks as well.

**2. Changing terrain under player edits: not supported by the module; workable at a cost.** The module stores an edit as baked voxel blocks, not as a difference from the generated terrain. After digging a hole and then growing the planet by 5 m:

| Approach | Untouched ground | Ground within about 15 m of the hole | The hole |
|---|---|---|---|
| Change the generator's input in place | Rises | Stays at the old height, leaving a stale island with a step at its edge | Survives |
| Assign a new generator to the terrain | Rises | Rises | Wiped |
| Fresh terrain, then replay the dig as an operation | Rises | Rises | Survives |

Only replaying works, at the same 1.7 ms per dig, for every dig on the planet, every time the shape changes. It also raises a design question the engine can't answer: a hole dug at the old surface is a buried pocket once the ground has risen above it.

This is not specific to Godot. Any engine that stores edits as voxel data has the same problem.

**3. Rendering from orbit to ground: works. Feel: not yet judged.** A bare planet with a flat-colour material was rendered on an RTX 5090 at 1600x900, launched from WSL through the Windows build with the project left in place on the WSL filesystem.

| Radius | View | Triangles | Draw calls | Memory |
|---|---|---|---|---|
| 2 km | Orbit | 59 thousand | 17 | 82 MB |
| 2 km | Ground | 641 thousand | 245 | 87 MB |
| 20 km | Orbit | 54 thousand | 26 | 62 MB |
| 20 km | Ground | 771 thousand | 308 | 100 MB |

- Cost is about the same at both sizes, so level of detail is doing its job and planet size is not a rendering constraint in this range.
- Digging a 5 m crater took about 10 ms and the worst frame in the following 240 was under 8 ms, so there is no visible hitch.
- Terrain around a new camera position finished loading in under a second on this machine.
- Frame rates were 470 to 950 uncapped, which says nothing about a mid-range GPU. That still needs measuring.
- Seen but not chased: faint seam lines between detail levels from orbit, and a few stray dark pixels at 20 km. The first is expected without the module's terrain shader; the second may be precision.
- Not covered: movement, so nothing here speaks to popping during flight or to how digging feels. A developer still needs to fly it.

**4. Fluids: ours to build.** The module's fluid support is for blocky terrain only and is rendering only; its docs say "fluids won't simulate behavior on their own. That part is for you to implement." The demo's sea is a plain sphere.

**5. Agent workflow: good on WSL, untested on macOS.** Both tests were written and run headless from WSL with no editor GUI, using the engine's own class reference (`--doctool`) to look up the module's API. Rendering needs the Windows build.

### Performance, 2026-10-04

Measured on a bare 2 km planet at 1920x1080 with shadows on, collision meshes on, and three extra viewers standing in for other players elsewhere on the planet. The machine has a Ryzen 9 9950X3D, an RTX 5090 and an integrated Radeon; the integrated GPU is the low-end check.

| Measurement | RTX 5090 | Integrated Radeon | Integrated Radeon, terrain limited to 4 threads |
|---|---|---|---|
| Standing on the ground | 451 fps | 67 fps | 70 fps |
| Descent from orbit to ground in 10 s, average | 524 fps | 108 fps | 109 fps |
| Descent, worst frame | 13.8 ms | 22.5 ms | 20.1 ms |
| Surface flight at 100 m/s, average | 419 fps | 89 fps | 89 fps |
| Surface flight at 100 m/s, worst frame | 14.6 ms | 18.7 ms | 16.1 ms |
| Surface flight at 400 m/s, worst frame | 15.7 ms | 57.5 ms | 17.7 ms |
| Worst frame after a dig | 9.0 ms | 17.6 ms | 15.9 ms |
| Terrain catch-up after stopping | none | 0.1 s | 0.5 s |
| Memory | 131 MB | 134 MB | 124 MB |

What this shows:

- Terrain streaming keeps up with flight at 400 m/s, even on four threads.
- The terrain alone holds 60 fps on a weak integrated GPU, with almost no room left. A mid-range discrete GPU is many times faster, so the target is plausible, but everything else the game draws has to fit in what remains.

What it does not show:

- The planet is bare: no textures, atmosphere, water, plants, structures, creatures or physics. Those are where the frame budget will actually go.
- The CPU is one of the fastest available. Limiting threads does not make each core slower.
- Windows only.

Simulation tick, one tick over four fields with six neighbours per cell:

| Cells | Cell size on a 2 km planet | GDScript | Rust in the engine |
|---|---|---|---|
| 2,562 | about 140 m | 2.3 ms | 0.02 ms |
| 10,242 | about 70 m | 8.8 ms | 0.10 ms |
| 40,962 | about 35 m | 36 ms | 0.44 ms |

Handing one field from Rust to the engine each tick costs 0.07 to 0.23 ms. GDScript was measured in the editor build, which is slower than a release build, but not by anything like this margin. Rust leaves room for finer cells, more fields and fast-forwarding hours of simulation; GDScript does not.

### Tech demo, 2026-10-04

The playable demo in [demo](../demo) joins voxel terrain, the Rust simulation and a first-person player.

- The voxel ground and the simulation's ground agree to within 1.2 m, because the terrain graph is generated from the simulation's own terrain shape.
- Automated checks pass on Linux and Windows: a player dropped from 20 m lands on the ground, walks at the expected speed, and can dig and refill.
- At 1080p with the simulation running, standing on the ground: 400 to 430 fps on the RTX 5090 and 65 to 71 fps on the integrated Radeon. From orbit: 551 and 155.
- The terrain graph's expression parser has two quirks we work around: it has no unary minus, and it reads a name followed by `*(` as a function call.
- Not yet covered: co-op in the demo.

### Lag report and renderer load test, 2026-10-04

A developer played the demo and found it laggy, which the scripted runs had not shown.

- **Cause found:** the simulation ran inside the render frame. At x100 the frame took 143 ms (7 fps). It now runs on its own thread: 112 fps with vsync on at x100, with the simulation holding 6,000 ticks a second.
- **Also fixed:** the ground texture refresh (now about 1 ms, four times a second) and mouse look, which was tied to the 60 Hz physics step and now updates every rendered frame.
- **Not explained:** with vsync on, frames arrive about every 9 ms on a 165 Hz display, with 1 in 100 taking 18 ms, although a frame renders in 1.3 ms. With vsync off, 1 in 100 frames still takes about 6 ms. A GPU load test was also running on the same machine while the developer played.
- **Lesson:** measure the loop a player actually runs, at the settings the instructions tell them to use.

Renderer load on top of the terrain, 1080p, vsync off:

| Added load | RTX 5090 | Integrated Radeon |
|---|---|---|
| None | 543 fps | 62 fps |
| 100 thousand instanced plants | 518 fps | 29 fps |
| 400 thousand instanced plants | 244 fps | 11 fps |
| 1 million instanced plants (21 million triangles) | 113 fps | 5 fps |
| 2 thousand separate objects | 558 fps | 67 fps |
| 10 thousand separate objects | 367 fps | 67 fps |
| 30 thousand separate objects | 134 fps | 60 fps |

- Instanced geometry is limited by the GPU, not the engine. The integrated GPU cannot draw dense vegetation at all, so the minimum spec has to be a discrete GPU or plant density has to scale down.
- Separate objects cost the CPU about 0.9 ms per 10 thousand, and draw calls stayed flat, so the engine batches them. Tens of thousands are fine; beyond that they should be instanced with their state kept in Rust.
- Not tested: animated creatures, particle swarms, transparent water, shadows over dense vegetation.

Nothing here shows Godot as the bottleneck for this design.

### A custom Rust engine (D27)

Plausible, and it is what most of the games we are inspired by did. The realistic version is not from scratch: Bevy supplies the entity system, renderer, animation, audio, UI and asset loading, and we write the voxel terrain, planet rendering and netcode.

| | Stay on Godot + Voxel Tools | Bevy plus our own terrain |
|---|---|---|
| Voxel terrain | Exists, native, measured | Ours to write: level of detail, seams, streaming, collision, edit storage. The hardest piece in the game. |
| Languages | Rust for heavy logic, GDScript for glue, a pinned custom C++ engine build | Rust only, compiler-checked throughout |
| Dependencies | One maintainer's module, with rough edges we have already hit | A large community project whose API changes every release |
| Many machines and creatures | Avoid scene nodes at scale; keep state in Rust | Native fit for the entity system |
| UI, animation, tooling | Mature | Younger |
| Browser | No | Possible again |
| Cost to get back to today's demo | None | Weeks at best, with high variance, mostly terrain robustness |

What exists to build on in Rust, checked 2026-10-05: parts, but no equivalent of Voxel Tools.

| Piece | Rust option | State |
|---|---|---|
| Engine | Bevy 0.19 | Active, large community |
| Smooth meshing | `fast-surface-nets`, `transvoxel` (which also handles seams between detail levels) | Small, stable algorithm crates; last updated in 2025 |
| Solar-system coordinates | `big_space` | Active |
| Physics | Avian, Rapier | Active |
| Voxel world framework | `bevy_voxel_world` | Active, but blocky cubes on flat worlds |
| Smooth sculpting | `bevy-sculpter` | A few hundred downloads; one person's project |
| Smooth voxel planets | A handful of hobby projects (Voxanet, Minos) | Days or weeks old, a few stars, one without a licence |
| Older frameworks | `feldspar`, `building-blocks` | Abandoned in 2022 |

The system that ties these together is ours to write: the tree of chunks at different detail levels, threaded streaming as the player moves, edit storage, and collision meshes. That is what Voxel Tools provides and nothing in Rust does.

Porting Voxel Tools itself is the realistic way to write that system. It is MIT-licensed, and at our pinned version it is about 142 thousand lines of C++ of its own, of which we need roughly a seventh:

| Part of Voxel Tools | Lines | Needed in a Rust port? |
|---|---|---|
| Level-of-detail terrain (chunk tree, streaming, updates) | 9,800 | Yes, the core |
| Voxel storage and edited blocks | 8,100 | Partly |
| Editing tools (sphere, raycast) | 7,800 | A small part |
| Task engine | 3,100 | Partly; Rust has good equivalents |
| Transvoxel mesher | 4,600 | No, the `transvoxel` crate covers it |
| Node-graph generators | 14,100 | No, our generator is plain Rust |
| Plant instancing | 9,200 | Later, or our own |
| Saving (region files, SQLite) | 8,200 | No, we store edits as operations |
| Blocky terrain, fixed-detail terrain, editor, detail rendering, utilities, tests | about 77,000 | No, or replaced by Rust's standard library |

The original also keeps running headless beside a port, so the port can be tested against it: same density function in, same voxel data and mesh out.

A middle path is the one we are already on: keep Godot and Voxel Tools as the renderer and terrain host, and write everything else in Rust.

What carries over either way: the simulation crate, the edits-as-operations model, and the design. That is why heavy logic stays in engine-independent Rust.

The cheapest way to decide is the comparison spike already planned as the fallback: the same bare planet (orbit to ground, digging, the same flight and load tests) on Bevy, time-boxed, compared on numbers and on how hard it was to build.

### Still to verify in the spike

- The same seed producing the same terrain on a Mac and a Windows PC.
- Frame times on a real mid-range GPU and CPU, and with real content on the planet.
- Building the Rust library on macOS. The Linux and Windows builds both load and run at the same speed.
- Surface quality, digging feel and ground-to-orbit, by a developer flying the demo.
- Behaviour at larger radii, with and without a double-precision build.
- That co-op edits also look right on screen, not only in the data.
- How terrain is textured, since smooth voxel texturing is still on the module's roadmap.

## Sources

- [Voxel Tools for Godot](https://github.com/Zylann/godot_voxel)
- [Voxel Tools multiplayer documentation](https://voxel-tools.readthedocs.io/en/latest/multiplayer/)
- [Solar System demo](https://github.com/Zylann/solar_system_demo)
- [Voxel Tools blocky terrain and fluids](https://voxel-tools.readthedocs.io/en/latest/blocky_terrain/)
- [Godot: exporting for the web](https://docs.godotengine.org/en/latest/tutorials/export/exporting_for_web.html)
- [Slay the Spire 2 sales on Godot](https://www.gamingonlinux.com/2026/03/the-godot-powered-slay-the-spire-2-has-already-hit-over-3-million-sales/)
- [Road to Vostok](https://en.wikipedia.org/wiki/Road_to_Vostok)
- [Bevy in 2026: indie readiness](https://www.strayspark.studio/blog/bevy-rust-game-engine-2026-indie-guide)
