# Terra

First and foremost, we are building an extremely ambitious sandbox engine, with multitudes of procedural worlds and intricate systems for each component of the game (D187): a multiplayer survival sandbox with RPG in it, across realms reached through portals, toward a thousand players on one persistent server (D52, D89). Rust on Bevy, our own voxel terrain. Pre-production: everything is current thinking.

## Tenets

- **Realistic but fun** (D122): things work as in the world, so players can reason about them (arrows ride in a quiver on your belt; what you know you found out by looking, D93). Where realism would only be a chore, fun wins.
- **Hundreds of variants, not millions** (D182): things are made from parts by rules, a theme (realm, maker, people) choosing which, in variants a player tells apart and cares about. A generator says how many distinct variants it makes and what tells them apart.
- **Where Bevy falls short, port from the engine that does it best** (D196), never our own invention or another engine: its code under MIT, BSD, Apache or zlib (Godot, Voxel Tools, O3DE, Jolt), otherwise (Unreal, GPL) only its docs, papers and talks. Name the source in the code, with any licence notice it requires. How deep: D184.
- **One art style for every realm** (D14): read [docs/art-style.md](docs/art-style.md) before making, generating or judging a model, concept, material or icon, and check the result against its checklist. A realm changes parts, materials and light, never the rules.

## Where things are

- `docs/` designs and decisions: [README](docs/README.md) the map, [tech](docs/tech.md) the stack, [engine](docs/engine.md) the parts, [prototypes](docs/prototypes.md) the next milestone, [development](docs/development.md) every command.
- `research/` a dated note per question, linked from its README · `assets/` what every binary loads (Bevy's asset root, set in `.cargo/config.toml`) · `art/` source art · `crates/` the game · `xtask/` tasks (`cargo xtask tools`) · `tools/` tooling outside the game, each with a README.
- `.agents/skills/` (linked as `.claude/skills`) playbooks for characters, models, icons, Tripo, Blender, environment, building: use the matching one.
- `spike/` throwaway experiments, one question each, deleted once the research note is written; nothing lasting depends on one (D133).

## Start

- Read `docs/README.md`. Find decisions in the index `docs/decisions.md` by grep or its area's section, never whole; read one in full (`cargo xtask decision D28`) before building on it.
- Before building a component, read its doc and the sections of `docs/development.md` you need.

## Routine (D375)

- **Start**: the session hook fast-forwards a clean main checkout, updates the shared seed and seeds your worktree (D183). On failure run `cargo xtask doctor` and fix it before building. Entered a worktree mid-session, or ran `--done` there: `cargo xtask seed` first.
- **Land on `main` after each incremental change**, no pull requests: commit on the branch, `git pull --rebase origin main`, `git push origin HEAD:main`. Never force-push `main`.
- **Hooks are the gate** (D37): commit runs `cargo xtask check --quick`, push `cargo xtask check`. Never `--no-verify`. Any `cargo xtask` task switches them on; CI is off.
- **Finish**: `git fetch`, `git branch -r --contains <commit>`, then `cargo xtask worktrees --done` (D259).
- A lead session runs at most three or four agents at once.
- **Every task is bounded** (D400): a brief says when the agent stops (a score bar, a number of rounds or two without a gain, a clock time, a context of about 250k at most). Do one unit of work, write a handoff file, end; the next round is a fresh agent from it. Wait on a long run started in the background by its completion notice, never a polling loop, one wait per thing. An agent you start gets the same bounds, one helper at a time.

## Gates and tool output

- **Gates scale with the change** (D398): a commit lints and unit-tests the changed crates in 30 s; a push tests them and what reads or depends on them, and runs the slow checks only when what they read changed. A push touching building takes minutes: give it a long timeout. `--all` runs everything. Keep a test to a few seconds or mark it `SLOW_TESTS`.
- **Gates are quiet** (D388): a line a stage. On failure read only what follows `✗`, then `target/check/<stage>.log`. `TERRA_VERBOSE=1` shows everything.
- **A new tool or task prints the same way** (D388): a line a stage on success, only what fixes a failure, the rest in a log it names.
- **Code health is gated** (D396): split a file over 1,000 lines or a function over 150 (`cargo xtask sizes`); call a shared primitive from its home (`terra_gen::hash`), never copy it.

## Context

Every byte an agent reads is paid again each turn.

- **Keep context small** (D390): read by range or grep; send long output to a file and read its tail; when the context nudge fires, finish the unit of work and hand off with a summary.
- **Delegate mechanical work** (D391): finding to `finder` (Haiku), running a check, test or benchmark to `runner` (Sonnet). Judgement, design, debugging and fixes stay on the default model.
- **A subagent's cache lasts an hour** (D397): a wait of minutes is cheap; past an hour the whole context is written again, so hand off instead.
- **Docs short and precise**: intent, constraints and reasons, not implementation or history. Here and in skills, one line a rule with its decision; the explanation lives in the doc it cites.

## Decisions (D389)

- Each is `docs/decisions/Dnnn.md`; edit those, never `docs/decisions.md`, then `node tools/docs/docs.ts --render`. The shape is in the index's preamble.
- They are the source of truth: don't contradict a live one, raise it. Decided is our answer, Leaning a working assumption, Open unchosen, Superseded kept so its number resolves.
- **Design** (gameplay, art, scope) is the developers': recommend, mark it Open, don't build on it. A developer thinking out loud is input, not a decision, unless they say it is settled.
- **Technical** (engine, architecture, tooling, workflow) is yours: decide, record why, keep going; the developers can overrule.
- **By** says who. An Agent decision is soft: any agent changes it with a reason and a log line. A Developers decision only they change. A choice other work builds on gets a decision; one left in chat or code didn't happen.
- **Revisit what proves wrong**, with evidence: change an agent's decision; raise a developer's with a recommendation.
- A change updates the decision's file and every doc it affects in one commit, with a line at the end of `docs/decisions-log.md` (D376). New numbers come from `cargo xtask decision`; on a collision whoever pushes second renumbers. The Why quotes the developers.

## How we work

- **Design before building** (D49): code waits for a design doc the developers have read. An idea from conversation is written down and asked about, not started as an experiment.
- **Don't overbuild**: nothing on a Leaning or Open design decision but a spike, which answers one stated question with the least code, then stops.
- **Tools for repeated work** (D71): when a check you will do again takes several manual steps, a restart or squinting at a picture, make it one step. General over single-case; its output reads at a glance and explains itself ([examples](docs/development.md#developer-tools)).
- **No options for one-offs** (D249): one scene or check uses existing options, `cargo xtask remote` or an uncommitted script; if none can, ask.
- **Do it as shipped games do**: find how they and other engines do it first, use the industry's terms, say where we depart.
- **Third-party current**: before building on or debugging a pack, library or tool, get its latest release from the original source and read its changelog.
- **Step back on a second attempt** at one failure: list the approaches (ours, an industry tool, a service, a known-good reference), pick one an agent can verify by measurement, and compare against the known-good reference.
- **Correct before fast**: right first; measure so the numbers are known; optimise later unless unusable.
- **Demos look finished**: real models, light, ground and props, small but never placeholder. Screenshot first.
- **Research** goes in `research/`; it informs decisions, never makes them.
- **Rust or TypeScript** (D121): the game and what it shares in Rust; tooling in TypeScript on Node 22.18+, no build step, no dependencies where possible; shell only as glue; Python only inside a program that speaks nothing else (Blender). Delete code nothing uses.
- **Public repository**: never commit engine binaries, build output, logs or anything personal.
- **A dependency** must support the pinned Bevy and be maintained; say why in the commit.

## Code

- **Many of everything** (D127): build for many of each kind and many kinds; change only what changed, not every frame; a new kind is data, not code (D136), so code naming one item should be questioned. How many: D182.
- **ECS** (D58): entities, components, systems; a resource only for what is truly one of a kind; what differs by world is data systems read.
- **Systems apart**: a feature is its own plugin and module, owning its components, resources and systems, talking through components, events and small shared types. Small systems; split a module before it is a monolith, so one system can be worked on without reading the rest.
- **Layers** (D198): world, walker, gameplay. A feature goes in the lowest it can; a layer never needs one above it and is tested alone; a shared type goes in the lowest layer that must understand it, defined by the side that asks, never in a "common" crate ([how](docs/tech.md#layers-of-the-game-d198)).
- **Shared state out of Bevy** (D28): only `terra_game`, `terra_sandbox` and `terra_world` depend on Bevy. The check enforces it.
- **`libm`** for trigonometry and exponentials, never `f32::sin` and friends (D30). The lints enforce it.
- **No `unsafe`.**
- **Tests** sit beside the code, need no GPU and fail when the behaviour breaks: test the outcome, know the answer another way, never pin tuned data ([how](docs/development.md#writing-tests)).
- **Check the real API**: read the pinned Bevy's source and examples under `~/.cargo/registry/src/`; never recall it. How far we change Bevy: D184.

## Checking your work

- **Checkable without a person** (D69): scripted runs (`--press`, `--shot`, `--vantage`, `--bench`), workbench sheets, `cargo xtask loop`, `cargo xtask remote`. Not done until an agent can drive it and see it worked.
- **Look at it**: `cargo xtask sandbox --experiment play --shot <file.png>` (CPU-rendered under WSL; add `--offscreen` on macOS), `cargo xtask workbench` for models. Close every run you start.
- **After a panic** (D399), read the `summary.txt` the run's last line names before any backtrace; see what an action changed with `r snap` and `r diff` ([tools/sandbox](tools/sandbox/README.md)).
- **Measure with `cargo xtask bench`** (D185): release, offscreen at 1920x1080, real GPU (under WSL the Windows build), against the last result; report the integrated GPU too. Software rendering's numbers mean nothing.
