# Development

How to set up a machine and work in the repository. The reasons behind these choices are in [tech.md](tech.md) and [decisions.md](decisions.md).

## Set up

Install [rustup](https://rustup.rs), clone the repository, then from its root:

```bash
cargo xtask doctor
```

It reads the pinned compiler version from `rust-toolchain.toml`, installs nothing itself, and lists what is missing with the command that fixes each item. The first `cargo` command in the repository downloads the pinned compiler.

| Platform | What you need beyond rustup |
|---|---|
| Linux, including WSL | `pkg-config`, `libasound2-dev`, `libudev-dev`, `libwayland-dev`, `libxkbcommon-dev` |
| WSL, to use the Windows GPU | `mingw-w64`, and the `x86_64-pc-windows-gnu` Rust target |
| macOS | Xcode command line tools |

macOS has been checked only by CI, which builds and tests there. Nobody has run a window on a Mac yet.

## Everyday commands

| Command | What it does |
|---|---|
| `cargo xtask play` | Play the tech demo on the real GPU, wherever you are; your game is kept between runs. `cargo xtask play --fresh` starts a new one. |
| `cargo xtask loop` | Play the demo's loop by script and check every step happened |
| `cargo xtask check` | Formatting, lints, tests and the dependency rule. Must pass before a commit. |
| `cargo test -p terra_sim` | One crate's tests |
| `cargo fmt --all` | Format everything |
| `cargo run -p terra_sandbox` | Open the sandbox, the test environment for graphics and utilities. See its [README](../crates/terra_sandbox/README.md). |
| `cargo xtask doctor` | Check this machine |
| `cargo xtask win -p <package>` | Under WSL, build for Windows and run on the real GPU |

A clean build of a Bevy application takes about two and a half minutes; a rebuild after a small change takes a few seconds.

## Seeing and measuring

Sound is on in the sandbox except in scripted runs, which print what they would have played instead, and under WSL, which usually has no sound device: pass `--sound` to try it there, or `--mute` anywhere ([audio.md](audio.md)).

WSL has no hardware Vulkan, which splits the work in two:

- **To see output**, run the Linux build. It renders on the CPU, slowly but correctly, and can save screenshots. This works anywhere, including for agents.
- **To measure performance**, build for Windows and run that build on the real GPU, in one step:

```bash
cargo xtask win -p <package> -- <program arguments>
```

It cross-compiles the package, copies the program and its assets to the Windows temp folder, runs it there and passes the output back. Windows will not replace a program that is running, so `--copy <name>` runs a separate copy from its own folder: an agent tests with `--copy test` while a person plays the default copy, and neither blocks the other. It runs from the temp folder because a Windows program started from a WSL folder gets an unusable working directory, so anything the program writes, such as screenshots, lands there; the command prints the path. Add `--release` for an optimised build.

Keep debug builds on the workspace's default profile: full debug information produces a file Windows refuses to load.

Every application we build gets scripted modes that fly a fixed camera path, print frame times and save screenshots, so results can be compared between machines and over time.

## Logs and the frame-time readout

Every build shows a line of frame times in the top-left corner, all the time (D77): how long a frame's work takes against the 16.7 ms budget (D25), coloured green, amber over three quarters, red over; then the game's CPU time, the render passes' CPU time, GPU time, and frames a second. F3 adds the spread over the last two seconds, the GPU, resolution and build type, entities, memory, and the costliest render passes. GPU times need timestamp queries (Vulkan and DirectX 12); on Metal they show as `-`. A debug build is several times slower on the CPU than a release build, so judge budgets on `--release`.

The log is for what is worth reading: the engine's routine messages are filtered out (`RUST_LOG` overrides the filter), and once a minute it records frame times as median, 99th percentile and worst, with a warning whenever frames take over twice the budget. Software rendering skips the warning, since its times mean nothing.

## Debug menu

<a id="debug-menu"></a>Wanted (D81): an in-game menu, present only when the game is started with a flag, opened with the backtick key. It does what developers need while playing: warp to a place or planet, give items, change the time of day or the weather, show colliders and chunk borders, switch systems off to see what they cost. Each entry should be a command that QA for agents (D69) can send too, so the menu and the agents' tools are the same list. Not built.

## Developer tools

The developers want tools that help refine each system (D71). The sandbox's experiments are where these live: each is a small plugin switched on from the command line ([sandbox README](../crates/terra_sandbox/README.md)). Wanted, first:

- **A model workbench**: load any model against a simple patch of terrain, scale it, recolour it or put textures on it, look at it from set angles and in the game's light, and save the result. It is also where models are tested and reviewed:
  - **Checks an agent runs**: triangle count, size against a reference figure, parts that float, every material named and in the palette, and a sheet of renders from set angles beside models already accepted.
  - **Review by a person**: step through candidates in the game's light, accept or reject each, and leave a note on it, or on a spot on it, that the agent reads on its next pass. Some developers using AI tools do this with a viewer where you point at a part and write a note for the agent.

  What it needs to check is in [research/ai-models-for-art.md](../research/ai-models-for-art.md#the-tools-the-agent-would-need).

  Built so far: the `workbench` experiment shows models in the game's renderer, side by side, animated, holding and wearing things, and with `--sheet` saves a contact sheet per animation for an agent to read ([sandbox README](../crates/terra_sandbox/README.md#the-workbench)). Any sandbox run started with `--remote` can be driven live, to move things, hold the camera on them, pose and screenshot without a restart ([tools/sandbox](../tools/sandbox/README.md)). Characters are checked before that by `tools/characters/rig_check.py`, which compares a rig with the hand-rigged mannequin on every animation. Not yet built: scaling, recolouring and the review by a person.

Each tool should be scriptable as well as usable by hand, so an agent can use it too.

### Blender

For making and cleaning up models ([research/ai-models-for-art.md](../research/ai-models-for-art.md)). Install Blender 5.1 or later; under WSL, the Windows install is the one to use, since it renders on the real GPU.

- **Scripts, headless.** `tools/blender/run.sh <script.py>` runs a script in Blender, from WSL in the Windows Blender ([tools/blender](../tools/blender/README.md)). Repeatable, and nothing has to be open.
- **A live session, through MCP.** Blender's official [MCP server](https://www.blender.org/lab/mcp-server/) lets an agent work in the Blender you have open, so you can watch it and change things by hand. It runs any code it is sent, so keep it to your own machine. Setting it up under WSL:
  1. In Blender, install the add-on from that page and start its server in the add-on's preferences. It won't start unless Preferences → System → Network → Allow Online Access is on, even for localhost. It listens on `localhost:9876`.
  2. WSL's default networking cannot reach Windows' `localhost`, so run the MCP server on Windows with Blender's own Python. Clone `https://projects.blender.org/lab/blender_mcp.git` into `%USERPROFILE%\.blender-mcp\src`, then install it with `python.exe -m pip install --target %USERPROFILE%\.blender-mcp\lib %USERPROFILE%\.blender-mcp\src\mcp`, using the `python.exe` under Blender's `5.x\python\bin`.
  3. Add a launcher, `%USERPROFILE%\.blender-mcp\run.py`, that puts `lib`, `lib\win32` and `lib\win32\lib` on `sys.path`, calls `os.add_dll_directory` on `lib\pywin32_system32` (a `--target` install skips pywin32's own set-up), sets `BLENDER_PATH` to Blender's `blender.exe`, makes `subprocess.run` default to `stdin=subprocess.DEVNULL` (the `*_for_cli` tools start a background Blender, and on Windows one that inherits the server's stdin pipe hangs until it times out), and runs `blmcp.main()`.
  4. Register it: `claude mcp add blender -s user -- "<Blender's python.exe>" '<path to run.py>'`. Claude Code picks it up in its next session. The `blender` skill has how agents use it.

### Tripo

For making models, textures, rigs and animations with AI (D115, D120). Everything goes through one command-line tool, [tools/tripo](../tools/tripo/README.md), which does what Tripo's Studio web app does and spends our subscription's credits:

```bash
node tools/tripo/tripo.ts image "a weathered supply crate, plain grey background" -o crate.png
node tools/tripo/tripo.ts model crate.png -o crate.glb      # prints the project id
node tools/tripo/tripo.ts rig <project>                     # a figure in a T- or A-pose
node tools/tripo/tripo.ts animate <project> walk run idle -o crate-anim.glb
```

- **Set up** once: put the `ory_kratos_session` cookie of a browser signed in to Studio in `.env` as `TRIPO_STUDIO_COOKIE` (the README says where to find it). It lasts about 30 days; when a command says the login failed, copy it again.
- **Every command prints one line of JSON** with the project, the credits spent and the files written, so commands chain. A project keeps every step done to it, and each command works on its latest.
- **It costs credits**: about 25 for a model, 20 for a rig, 10 for a texture; images and preset animations are free. `balance` says what is left. Credits count in the token stats beside Claude's tokens ([tools/token-stats](../tools/token-stats/README.md)), read from Tripo's own history, so work done by hand in the web app counts too.
- **Use the CLI, not the web app's API by hand, and not the bake-off's `tripo.py`**, which spends API credits. When Tripo changes its requests and a command breaks, fix the CLI; [research/tripo-studio-api.md](../research/tripo-studio-api.md) says how to capture what the web app sends now.

## Checking the game without a person (QA for agents)

The developers want the game's QA reachable by our coding agents (D69): an agent should be able to run the game, drive it, look at it and ask it what is true, the way a tester would.

What exists: scripted runs of the sandbox. Every run starts on the loading screen (D135), and a script's frames are counted from when it goes, so a script does the same thing however long loading took. `--press` holds keys on given frames, `--shot` saves a screenshot, `--vantage` fixes the camera, `--bench` measures frame time. In the play experiment, `--at`, `--face`, `--warp` and `--give` set a scene up, and every on-screen notice is logged, which is how features were checked until the live commands below.

Live, in one running game (D69): any sandbox run started with `--remote` answers Bevy's remote protocol, and [tools/sandbox](../tools/sandbox/README.md) drives it from the command line. Besides moving things and taking screenshots, a script can press keys, stop and step the clock exactly, read everything the run has said and wait for words to be said, and in the play experiment read a digest of the player's state (`state`) and set a scene up (`warp`, `give`). A check acts, then waits for what the game says or reads what is true, rather than counting frames or reading the screen. When a check needs a fact the game does not yet answer, add it to the game's methods; a command line over them is the interface, and an MCP server would only be a thin layer over the same methods if some client needed one.

Built on it:

- **The loop, played live**: `cargo xtask loop` plays the MVP's loop in one running game and checks each step, restarting once to load what it saved ([tools/sandbox](../tools/sandbox/README.md#scripts)).
- **Same start, same game**: with `--trace FILE.jsonl` the play experiment writes its whole state every frame, and `tools/sandbox/diverge.ts --twice -- <a scripted run>` runs it twice and names the first frame and the values where the runs part. Two runs of the hunt agree on every frame.
- **Shared code stays repeatable**: `cargo xtask check` refuses a shared-state crate that reads the clock or the system's randomness; time and seeds are passed in.
- **Property tests over action sequences** in `terra_items`: random moves, and random stowing, paying and wearing, over many seeds, checked against a simple tally after every step. A failure names its seed and step.

Off in release builds, since it lets anything on the machine control the game.

Playing the game as an agent, inside its rules rather than as a tester, is a separate want: see [multiplayer.md](multiplayer.md#agents-as-players) (D73).

## Conventions

- The rules for working in the workspace are in [AGENTS.md](../AGENTS.md). They apply to people as well as agents.
- How crates are split, and how to add one, is in [crates/README.md](../crates/README.md).
- Dependencies are pinned through `Cargo.lock`, which is committed.
- Bevy is upgraded deliberately, once the crates we depend on support the new release (D31).

## Continuous integration

Every push and pull request runs `cargo xtask check` on Linux, macOS and Windows, and a compile check for the Windows target we cross-compile from WSL. Nothing in CI uses a GPU.

To run the tests on that cross-compiled Windows build yourself, from WSL: `cargo test --workspace --target x86_64-pc-windows-gnu`.
