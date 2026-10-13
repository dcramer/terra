# Terra

**First and foremost, we are building an extremely ambitious sandbox engine, with multitudes of procedural worlds and intricate systems for each component of the game** (D187). The game is a multiplayer survival sandbox with RPG in it, across realms reached through portals, built toward a thousand players on one persistent server (D52, D89). Pre-production: everything here is current thinking.

**Core tenet: realistic but fun** (D122). Things work as they would in the world, so players can reason about them: arrows ride in a quiver on your belt, and what you know you found out by looking (D93). Where realism would only be a chore, fun wins.

**Core tenet: hundreds of variants, not millions** (D182). Almost everything is made from parts by rules, as Borderlands makes guns and No Man's Sky makes planets, with a theme (a realm, a maker, a people) deciding which parts and rules apply. Aim for hundreds or thousands of variants of a kind that a player can tell apart and care about, not millions of near-copies. When you design a generator, say how many distinct variants it makes and what tells them apart.

**Core tenet: where Bevy falls short, port from an engine that does it well** (D196). When Bevy is weak at something (skinning, animation, terrain, physics, rendering), find the engine or tool that does it best and reimplement its version on Bevy, rather than inventing our own or changing engine. Port from its source when its licence lets an MIT project take the code (MIT, BSD, Apache, zlib: Godot, Voxel Tools, O3DE, Jolt); otherwise work from its documentation, papers and talks, and never copy code under an incompatible licence (Unreal, anything GPL). Say in the code what it was ported from, keeping the original's licence notice where its licence asks for one. How deep into Bevy a change may go is D184.

**Art style: one guide for every realm** (D14). Everything the game draws follows [docs/art-style.md](docs/art-style.md): textured, lightly stylised realism, real materials in detailed textures, a little shaded, neither photoreal nor cartoon. Read it before you make, generate or judge a model, a concept image, a material or an icon, and check the result against its checklist. A realm, medieval or cyber or alien, changes its parts, materials and light, never the rules.

Everything is built in Rust on Bevy, with our own voxel terrain. The stack is in [docs/tech.md](docs/tech.md), the parts and their status in [docs/engine.md](docs/engine.md), and the next milestone in [docs/prototypes.md](docs/prototypes.md).

| Folder | What it is |
|---|---|
| `docs/` | Decisions and designs. Start here. |
| `research/` | Findings, surveys and measurements, one dated note per question. |
| `assets/` | Runtime assets every binary loads: models, sounds, shaders, icons, data. Cargo points Bevy here (`.cargo/config.toml`). |
| `art/` | Source art the asset pipeline makes the game's files from. See `art/README.md`. |
| `crates/` | The Rust workspace: the game's code. See `crates/README.md`. |
| `xtask/` | Project tasks, run as `cargo xtask <task>`; `cargo xtask tools` lists them all. |
| `tools/` | Team tooling outside the game, each with a README; most are started through `cargo xtask`. |
| `.agents/skills/` | Playbooks for repeated work: making characters and models, item icons, Tripo, Blender. Use the matching one. `.claude/skills` is a link to it, for Claude Code. |
| `spike/` | Throwaway experiments, each answering one question, deleted once its research note is written (D133). Nothing lasting (a skill, tool, doc or game code) may depend on a spike; move it to `tools/` or the workspace first. |

## Before doing anything

Read `docs/README.md`, then `docs/decisions.md`. Before building a component, read its doc in `docs/` and `docs/development.md`.

## Decisions

- `docs/decisions.md` is the source of truth. Don't contradict a live decision; raise it instead.
- **Decided** is our current answer. **Leaning** is a working assumption. **Open** is not chosen. **Superseded** rows are kept only so their numbers resolve.
- Design decisions (what the game is: gameplay, art, scope) belong to the developers. Give a recommendation, mark it Open, and don't build on it. When a developer thinks out loud, that is input, not a decision, unless they say it is settled.
- Technical decisions (engine, architecture, tooling, workflow) are yours: decide, record the reason, and keep going. The developers can overrule.
- **Say who decided.** The By column is Developers or Agent. An agent's decision is soft: another agent may change it with a reason and a log line, without asking; a developer's decision is changed only by the developers. A choice that other work will build on gets a row, even a small one; one left only in chat or code didn't happen.
- When a decision is made or changes, update its row and every doc it affects in the same change, and add a line to the log at the bottom. A new decision takes the next free number, goes in its area's section, with the Owner column saying Design or Tech, the By column saying who made it, and the developers' own words in the Why column. `cargo xtask check` checks the table's shape and that every decision cited exists.

## How we work

- **Design before building** (D49). A feature gets a design doc the developers have read before it gets code. Don't start an experiment because an idea came up in conversation; write it down and ask.
- **Don't overbuild.** Nothing is built on a Leaning or Open design decision except a spike, and a spike answers one stated question with the least code that can, then stops.
- **Build tools for the work you keep doing** (D71). When testing, debugging or checking something takes several manual steps, a restart, or squinting at a picture, and you will do it again, build the tool that makes it one step. Prefer general tools over ones for a single case.
  - **Would build:** setting a joint, a grip or a camera in the running game and seeing the result at once; a workbench sheet that puts two models side by side through the same animation; a checker that measures a rig against a known-good one.
  - **Wouldn't build:** a tool for something done once; a polished editor where a command-line flag does the job; a tool for a feature that is not decided yet.
- **Do it the way shipped games do.** Before designing a system or a pipeline, find out how shipped games and other engines do it, use the industry's terms, and say so when we depart from it. When Bevy lacks something, port it from an engine that has it (D196).
- **Check third-party things are current.** Before building on, or debugging, an asset pack, library or tool, check we have its latest release and read its changelog. Prefer the original source to mirrors.
- **Step back when something keeps failing.** Before a second attempt at the same symptom, list the approaches (our own, an industry tool, a service, a known-good reference), pick one an agent can verify by measurement, and compare against a known-good reference to find the fault.
- **Correct before fast.** Make things look and behave right first. Measure performance so the numbers are known, but optimise later unless something is unusable.
- **Demos look finished.** Anything shown to the developers has real models, light, ground and props, small in scope but never placeholder in appearance. Check it with a screenshot first.
- **Research goes in `research/`**, as a dated note linked from `research/README.md`. Research informs decisions; it does not make them.
- **Keep docs short:** intent, constraints and reasons, not implementation detail or history. Git has the history.
- **Code is Rust or TypeScript** (D121). The game and anything it shares is Rust; tooling outside it is TypeScript run by Node 22.18 or later, with no build step and, where possible, no dependencies. A short shell script is fine as glue; once it has real logic, write it in TypeScript or Rust. No new Python or other languages, except code that must run inside a program that only speaks Python, such as Blender. Code nothing uses is deleted, not kept or ported.
- **This is a public repository.** Don't commit engine binaries, build output, logs, or anything personal.
- **Everything lands on `main`, with no pull requests.** Commit and push after every incremental change, so everyone is up to date between changes. In a worktree, commit on its branch, then `git pull --rebase origin main`, run the checks again if anything came in, and `git push origin HEAD:main`. Never force-push `main`.

## Working in the Rust workspace

- **Assume many of everything** (D127). There will be a large quantity of every kind of object: items, gear, creatures, characters, props, places. Build every system to perform with many of each and many kinds, change only what changed rather than recomputing every frame, and make a new kind of thing data (a file, a catalogue entry) rather than code (D136). If code names a specific item, ask whether it should be data. How many is worth making is D182.
- **Model the game in ECS** (D58). Everything in a world is an entity, its state is components, and behaviour is systems. Use a resource only for what is truly one of a kind. Things that differ by world (gravity, air, what grows) are data read by systems, not code written per world.
- **Keep systems apart.** Each feature is its own plugin in its own module, owning its components, resources and systems, and talking to others through components, events and small shared types. Prefer small systems that do one thing, and split a module before it grows into a monolith. Someone should be able to work on one system without reading the rest.
- **Shared state stays out of Bevy** (D28). Anything every player must agree on lives in a crate that does not depend on Bevy. Only the applications, `terra_game` and `terra_sandbox`, may. The check enforces this.
- **Use `libm` for trigonometry and exponentials** (D30), never `f32::sin` and friends. The system maths library differs between platforms in results and speed. The lints enforce this.
- **No `unsafe`.** It is forbidden workspace-wide.
- **Tests live next to the code** and need no GPU. A behaviour you rely on gets a test.
- **`cargo xtask check` must pass before you commit**, and a git hook runs it on every commit (any `cargo xtask` task switches the hook on; `--no-verify` skips it, for an emergency only). It runs formatting, lints, tests, the docs check (links, indexes, the decisions table), a check of the Blender scripts and the dependency rule. CI is off for now (D37), so this check is the gate.
- **`cargo xtask doctor`** tells you what a machine is missing. **In a new worktree, build after the seed** (D183): a hook copies the main checkout's compiled dependencies in when a session starts there; if you entered the worktree partway through a session, run `cargo xtask seed` first.
- **Make what you build checkable without a person** (D69): scripted runs (`--press`, `--shot`, `--vantage`, `--bench`), the workbench's sheets, `cargo xtask loop`, and driving a run live over Bevy's remote protocol (`cargo xtask remote`, [tools/sandbox](tools/sandbox/README.md)). A feature is not done until an agent can drive it and see that it worked.
- **Check the real API, don't recall it.** Bevy changes every release. Read the examples and source that ship with the pinned version under `~/.cargo/registry/src/` before writing against it. How far we change Bevy itself is D184.
- **Look at what you made.** `cargo xtask sandbox --experiment play --shot <file.png>` saves a screenshot of the game; under WSL the Linux build renders on the CPU, so this needs no GPU. On macOS add `--offscreen`: a covered window, or a locked screen, draws nothing. `cargo xtask workbench` shows models. Close every run you start. The rest of the commands are in [docs/development.md](docs/development.md).
- **Measure performance with `cargo xtask bench`** (D185): release, offscreen at 1920x1080, on the real GPU (under WSL, the Windows build), against the last recorded result. Report the integrated GPU as well as the fast one. Software rendering numbers mean nothing.
- **Don't add a dependency casually.** Check it supports the pinned Bevy version and is maintained, and say why in the commit.
