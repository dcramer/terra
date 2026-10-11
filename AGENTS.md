# Terra

A multiplayer survival sandbox across a dozen or so procedural planets, some of which can be terraformed from dead to living, built toward a thousand players on one persistent server. Pre-production. The worlds are in `docs/worlds.md` and the survival attributes in `docs/survival.md`.

Everything is built in Rust on Bevy, with our own voxel terrain. The plan is in `docs/tech.md`, the parts and build order are in `docs/engine.md`, and the checkpoints are in `docs/prototypes.md`.

| Folder | What it is |
|---|---|
| `docs/` | Decisions and plans. Start here. |
| `research/` | Findings, surveys and measurements, one dated note per question. |
| `crates/` | The Rust workspace: the game's code. See `crates/README.md`. |
| `xtask/` | Project tasks, run as `cargo xtask <task>`. |
| `demo/` | The playable tech demo on Godot. A working reference until the Rust build replaces it. |
| `prototypes/sim_toy/` | A globe for watching the planet simulation, on Godot. |
| `spike/` | Throwaway experiments. |

## Before doing anything

Read `docs/README.md`, then `docs/decisions.md`. Before building a component, read its brief in `docs/` and `docs/development.md`.

## Rules

- `docs/decisions.md` is the source of truth. Don't contradict a Decided item; raise it instead.
- When a decision is made or changes, update `docs/decisions.md` and any doc it affects in the same change, and add a line to its log.
- Don't record a proposal as Decided. Unconfirmed ideas are Leaning or Open. When a developer thinks out loud about what the game might be, that is input, not a decision, unless they say it is settled.
- The whole project is provisional while in pre-production. Write plans as current thinking, not commitments.
- Keep docs short: intent, constraints and reasons, not implementation detail.
- Design before building. A feature gets a design doc the developers have read before it gets code. Don't start an experiment because an idea came up in conversation; write it down and ask.
- Don't overbuild. Nothing is built on a Leaning or Open decision except a spike, and a spike answers one stated question with the least code that can answer it, then stops. No game systems until the decisions they rest on are Decided.
- Technical decisions are yours to make: decide, record the reason in `docs/decisions.md`, and keep going. Design decisions belong to the developers: give a recommendation, mark it Open, and don't build on it.
- When you investigate something, write it up as a dated note in `research/` and link it from `research/README.md`. Research informs decisions; it does not make them.
- This is a public repository. Don't commit engine binaries, build output, logs, or anything personal.
- Work on `main`. There are no branches or pull requests: commit to `main` and push it.
- Commit and push after every incremental change. Others work on this project too, and the goal is that everyone is up to date between changes. Before pushing, pull with rebase (`git pull --rebase`), resolve any conflicts, and run the checks again if anything came in. Never force-push `main`.

## Working in the Rust workspace

- **`cargo xtask check` must pass before you commit.** It runs formatting, lints, tests and the dependency rule below. CI runs the same command on Linux and macOS.
- **`cargo xtask doctor`** tells you what a machine is missing.
- **Model the game in ECS** (D58). Everything in a world is an entity, its state is components, and behaviour is systems. Use a resource only for what is truly one of a kind. Things that differ by planet (gravity, air, what grows) are data read by systems, not code written per planet.
- **Keep systems apart.** Each feature is its own plugin in its own module, owning its components, resources and systems, and talking to others through components, events and small shared types rather than reaching into their internals. Prefer small systems that do one thing over one system that does everything, and split a module before it grows into a monolith. Someone should be able to work on one system without reading the rest.
- **Shared state stays out of Bevy.** Anything every player must agree on lives in a crate that does not depend on Bevy. Only `terra_game` may. The check enforces this.
- **Use `libm` for trigonometry and exponentials**, never `f32::sin` and friends. The system maths library differs between platforms in both results and speed. The lints enforce this.
- **No `unsafe`.** It is forbidden workspace-wide.
- **Tests live next to the code** and need no GPU. A behaviour you rely on gets a test.
- **Make what you build checkable without a person** (D69). Today that means the sandbox's scripted runs (`--press`, `--shot`, `--vantage`, `--bench`); later, Bevy's remote protocol. A feature is not done until an agent can drive it and see that it worked.
- **Check the real API, don't recall it.** Bevy changes every release. Read the examples and source that ship with the pinned version under `~/.cargo/registry/src/` before writing against it.
- **Look at what you made.** The Linux build renders on the CPU under WSL, so you can save a screenshot and read it without a GPU: `cargo run -p terra_sandbox -- --shot <file.png>`.
- **Measure performance on a real GPU, in the loop a player runs**, and report the integrated GPU as well as the fast one. Software rendering numbers mean nothing. Under WSL, `cargo xtask win -p <package>` builds for Windows and runs there.
- **Don't add a dependency casually.** Check it supports the pinned Bevy version and is maintained, and say why in the commit.
