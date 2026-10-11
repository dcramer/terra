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
| `cargo xtask check` | Formatting, lints, tests and the dependency rule. Must pass before a commit. |
| `cargo test -p terra_sim` | One crate's tests |
| `cargo fmt --all` | Format everything |
| `cargo run -p terra_sandbox` | Open the sandbox, the test environment for graphics and utilities. See its [README](../crates/terra_sandbox/README.md). |
| `cargo xtask doctor` | Check this machine |
| `cargo xtask win -p <package>` | Under WSL, build for Windows and run on the real GPU |

A clean build of a Bevy application takes about two and a half minutes; a rebuild after a small change takes a few seconds.

## Seeing and measuring

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

Each tool should be scriptable as well as usable by hand, so an agent can use it too.

### Blender

For making and cleaning up models ([research/ai-models-for-art.md](../research/ai-models-for-art.md)). Install Blender 5.1 or later; under WSL, the Windows install is the one to use, since it renders on the real GPU.

- **Scripts, headless.** `spike/model_bakeoff/run.sh <script.py>` runs a script in the Windows Blender from WSL. Repeatable, and nothing has to be open.
- **A live session, through MCP.** Blender's official [MCP server](https://www.blender.org/lab/mcp-server/) lets an agent work in the Blender you have open, so you can watch it and change things by hand. It runs any code it is sent, so keep it to your own machine. Setting it up under WSL:
  1. In Blender, install the add-on from that page and start its server in the add-on's preferences. It listens on `localhost:9876`.
  2. WSL's default networking cannot reach Windows' `localhost`, so run the MCP server on Windows with Blender's own Python. Clone `https://projects.blender.org/lab/blender_mcp.git` into `%USERPROFILE%\.blender-mcp\src`, then install it with `python.exe -m pip install --target %USERPROFILE%\.blender-mcp\lib %USERPROFILE%\.blender-mcp\src\mcp`, using the `python.exe` under Blender's `5.x\python\bin`.
  3. Add a launcher, `%USERPROFILE%\.blender-mcp\run.py`, that puts `lib`, `lib\win32` and `lib\win32\lib` on `sys.path`, calls `os.add_dll_directory` on `lib\pywin32_system32` (a `--target` install skips pywin32's own set-up), and runs `blmcp.main()`.
  4. Register it: `claude mcp add blender -s user -- "<Blender's python.exe>" '<path to run.py>'`. Claude Code picks it up in its next session.

## Checking the game without a person (QA for agents)

The developers want the game's QA reachable by our coding agents (D69): an agent should be able to run the game, drive it, look at it and ask it what is true, the way a tester would.

What exists: scripted runs of the sandbox. `--press` holds keys on given frames, `--shot` saves a screenshot, `--vantage` fixes the camera, `--bench` measures frame time. That is how every feature so far was checked.

What it should grow into, decided technically and untested (Leaning): **Bevy's remote protocol** (`bevy_remote`, part of the pinned Bevy). A game built with it answers requests over a local connection: list and read entities and components, change them, spawn things, send input, take screenshots. Bevy ships an example of exactly this, a test driving a running app. On top of it:

- **A command line**, `cargo xtask qa ...`, for the checks we run often: start the game, teleport, build something, read the inventory, screenshot.
- **An MCP server**, a thin layer over the same protocol, if agents work better with tools than with a command line.

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
