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
| `.agents/skills/` | Playbooks for repeated work: making characters and models, item icons, Tripo, Blender, how a place looks (environment), construction (building). Use the matching one. `.claude/skills` is a link to it, for Claude Code. |
| `spike/` | Throwaway experiments, each answering one question, deleted once its research note is written (D133). Nothing lasting (a skill, tool, doc or game code) may depend on a spike; move it to `tools/` or the workspace first. |

## Before doing anything

Read `docs/README.md`, then `docs/decisions.md`, the index of decisions; before building on one, read it in full (`docs/decisions/Dnnn.md`, or `cargo xtask decision D28`). Before building a component, read its doc in `docs/` and `docs/development.md`.

## Routine

Every agent and lead session follows it (D375; [development.md](docs/development.md#worktrees) has the details):

- **Start.** The session-start hook fast-forwards the main checkout to origin/main if it is clean, makes sure the shared seed build is current, and seeds your worktree from it (D183). If any of these fail, run `cargo xtask doctor` and fix it before building.
- **While working.** `git pull --rebase origin main` before each push. Commits run the quick check and pushes the full one, each on what the change touches; never use `--no-verify` (D37, D398). Decision log lines go in [docs/decisions-log.md](docs/decisions-log.md), and decision numbers come from `cargo xtask decision` or, when two collide, the check (D376).
- **Finish.** Confirm your commit is on origin/main (`git fetch`, then `git branch -r --contains <commit>`), then run `cargo xtask worktrees --done` (D259).
- **A lead session runs at most three or four agents at once**, so builds and measurements stay fast and trustworthy.

## Decisions

- `docs/decisions/`, a file per decision indexed by `docs/decisions.md`, is the source of truth. Don't contradict a live decision; raise it instead.
- **Decided** is our current answer. **Leaning** is a working assumption. **Open** is not chosen. **Superseded** rows are kept only so their numbers resolve.
- Design decisions (what the game is: gameplay, art, scope) belong to the developers. Give a recommendation, mark it Open, and don't build on it. When a developer thinks out loud, that is input, not a decision, unless they say it is settled.
- Technical decisions (engine, architecture, tooling, workflow) are yours: decide, record the reason, and keep going. The developers can overrule.
- **Say who decided.** By is Developers or Agent. An agent's decision is soft: another agent may change it with a reason and a log line, without asking; a developer's decision is changed only by the developers. A choice that other work will build on gets a decision, even a small one; one left only in chat or code didn't happen.
- **Revisit what proves wrong.** Nothing decided is fixed for good. When a decision, or something built on one, turns out wrong or inefficient, say so with the evidence and change it: an agent's decision directly, with a reason and a log line; a developer's by raising it with them and a recommendation.
- When a decision is made or changes, update its file and every doc it affects in the same change, run `node tools/docs/docs.ts --render` (it writes the index; never edit `docs/decisions.md`'s table by hand), and add a line at the end of [docs/decisions-log.md](docs/decisions-log.md). A new decision takes the next free number (`cargo xtask decision` prints it; when two agents take one at once, the rebase stops on the file both added and whoever pushes second renumbers) as `docs/decisions/Dnnn.md`, shaped like the others: its Area an index heading, Owner Design or Tech, By who made it, and the developers' own words under `## Why`. `cargo xtask check` checks each file's shape, that the index is their render, and that every decision cited exists.

## How we work

- **Design before building** (D49). A feature gets a design doc the developers have read before it gets code. Don't start an experiment because an idea came up in conversation; write it down and ask.
- **Don't overbuild.** Nothing is built on a Leaning or Open design decision except a spike, and a spike answers one stated question with the least code that can, then stops.
- **Build tools for the work you keep doing** (D71). When testing, debugging or checking something takes several manual steps, a restart, or squinting at a picture, and you will do it again, build the tool that makes it one step. Prefer general tools over ones for a single case. Tools must be easy to use, a requirement: a developer reads what one shows at a glance, without knowing how it works, and it explains itself where it is used.
  - **Would build:** setting a joint, a grip or a camera in the running game and seeing the result at once; a workbench sheet that puts two models side by side through the same animation; a checker that measures a rig against a known-good one.
  - **Wouldn't build:** a tool for something done once; a polished editor where a command-line flag does the job; a tool for a feature that is not decided yet.
  - **No options for one-offs** (D249). Don't add a command-line option, flag or experiment for one scene or one check. Do it with what exists: the existing options, the run driven live (`cargo xtask remote`), or a throwaway script left uncommitted. If nothing that exists can do it, ask first.
- **Do it the way shipped games do.** Before designing a system or a pipeline, find out how shipped games and other engines do it, use the industry's terms, and say so when we depart from it. When Bevy lacks something, port it from an engine that has it (D196).
- **Check third-party things are current.** Before building on, or debugging, an asset pack, library or tool, check we have its latest release and read its changelog. Prefer the original source to mirrors.
- **Step back when something keeps failing.** Before a second attempt at the same symptom, list the approaches (our own, an industry tool, a service, a known-good reference), pick one an agent can verify by measurement, and compare against a known-good reference to find the fault.
- **Correct before fast.** Make things look and behave right first. Measure performance so the numbers are known, but optimise later unless something is unusable.
- **Demos look finished.** Anything shown to the developers has real models, light, ground and props, small in scope but never placeholder in appearance. Check it with a screenshot first.
- **Research goes in `research/`**, as a dated note linked from `research/README.md`. Research informs decisions; it does not make them.
- **Keep docs short:** intent, constraints and reasons, not implementation detail or history. Git has the history.
- **Code is Rust or TypeScript** (D121). The game and anything it shares is Rust; tooling outside it is TypeScript run by Node 22.18 or later, with no build step and, where possible, no dependencies. A short shell script is fine as glue; once it has real logic, write it in TypeScript or Rust. No new Python or other languages, except code that must run inside a program that only speaks Python, such as Blender. Code nothing uses is deleted, not kept or ported.
- **This is a public repository.** Don't commit engine binaries, build output, logs, or anything personal.
- **Everything lands on `main`, with no pull requests.** Commit and push after every incremental change, so everyone is up to date between changes. In a worktree, commit on its branch, then `git pull --rebase origin main` and `git push origin HEAD:main`, whose hook runs the full check on what you push. Never force-push `main`.

## Working in the Rust workspace

- **Assume many of everything** (D127). There will be a large quantity of every kind of object: items, gear, creatures, characters, props, places. Build every system to perform with many of each and many kinds, change only what changed rather than recomputing every frame, and make a new kind of thing data (a file, a catalogue entry) rather than code (D136). If code names a specific item, ask whether it should be data. How many is worth making is D182.
- **Model the game in ECS** (D58). Everything in a world is an entity, its state is components, and behaviour is systems. Use a resource only for what is truly one of a kind. Things that differ by world (gravity, air, what grows) are data read by systems, not code written per world.
- **Keep systems apart.** Each feature is its own plugin in its own module, owning its components, resources and systems, and talking to others through components, events and small shared types. Prefer small systems that do one thing, and split a module before it grows into a monolith. Someone should be able to work on one system without reading the rest.
- **Layers, not a tangle** (D198). The game is three plugin layers, world, walker and gameplay ([docs/tech.md](docs/tech.md#layers-of-the-game-d198)). Put a feature in the lowest layer it can; a layer never needs a system or resource above it to run, and learns what it needs through system sets, small types the layer above fills in, observers and optional resources. Test a lower layer alone, as the sandbox's `walk` does. A shared type goes in the lowest layer that has to understand it, defined by the side that asks the question, never in a "common" crate ([docs/tech.md](docs/tech.md#where-shared-types-go)).
- **Shared state stays out of Bevy** (D28). Anything every player must agree on lives in a crate that does not depend on Bevy. Only the applications, `terra_game` and `terra_sandbox`, may, and `terra_world`, the world on screen they stand on (D198). The check enforces this.
- **Use `libm` for trigonometry and exponentials** (D30), never `f32::sin` and friends. The system maths library differs between platforms in results and speed. The lints enforce this.
- **No `unsafe`.** It is forbidden workspace-wide.
- **Tests live next to the code** and need no GPU. A behaviour you rely on gets a test, and a test must fail when that behaviour breaks: test the outcome, know the answer another way than the code does, and never pin tuned data ([docs/development.md](docs/development.md#writing-tests)).
- **Commits run the quick check, pushes the full one, each scoped to the change** (D37, D398). Before every commit git runs `cargo xtask check --quick`, linters and fast unit tests in a 30-second budget it reports against: formatting, the code-health gates, clippy and the tests but `SLOW_TESTS` of the crates changed only, the changed tools' tests and the docs check (links, indexes, the decisions table). Before every push it runs `cargo xtask check`: clippy and every test of the crates changed and every crate depending on them, the tools' tests, and the kit, foliage, artifacts, fuzz and Blender checks when what they read changed, minutes when building changed, so give a push a long timeout. A change to the build itself, or a file the scoping cannot place, runs everything; `--all` does too. Keep a test to a few seconds, or list it in `SLOW_TESTS`. Any `cargo xtask` task switches the hooks on. Never skip them with `--no-verify`. CI is off for now, so these are the gate.
- **`cargo xtask doctor`** tells you what a machine is missing, the shared seed build's state included. **In a new worktree, build after the seed** (D183): a hook links in the compiled dependencies of the shared seed build, kept current with origin/main, when a session starts there; if you entered the worktree partway through a session, or ran `--done` there, run `cargo xtask seed` first. **When your work in a worktree is pushed, run `cargo xtask worktrees --done`** before you finish (D259): it deletes the worktree's build, ten gigabytes or more, and a background prune removes the worktree itself two days later.
- **Make what you build checkable without a person** (D69): scripted runs (`--press`, `--shot`, `--vantage`, `--bench`), the workbench's sheets, `cargo xtask loop`, and driving a run live over Bevy's remote protocol (`cargo xtask remote`, [tools/sandbox](tools/sandbox/README.md)). A feature is not done until an agent can drive it and see that it worked.
- **Check the real API, don't recall it.** Bevy changes every release. Read the examples and source that ship with the pinned version under `~/.cargo/registry/src/` before writing against it. How far we change Bevy itself is D184.
- **Look at what you made.** `cargo xtask sandbox --experiment play --shot <file.png>` saves a screenshot of the game; under WSL the Linux build renders on the CPU, so this needs no GPU. On macOS add `--offscreen`: a covered window, or a locked screen, draws nothing. `cargo xtask workbench` shows models. Close every run you start. The rest of the commands are in [docs/development.md](docs/development.md).
- **Measure performance with `cargo xtask bench`** (D185): release, offscreen at 1920x1080, on the real GPU (under WSL, the Windows build), against the last recorded result. Report the integrated GPU as well as the fast one. Software rendering numbers mean nothing.
- **Don't add a dependency casually.** Check it supports the pinned Bevy version and is maintained, and say why in the commit.
