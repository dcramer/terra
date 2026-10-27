# Terra

First and foremost, we are building an extremely ambitious sandbox engine, with multitudes of procedural worlds and intricate systems for each component of the game (D187): a multiplayer survival sandbox with RPG in it, across realms reached through portals, toward a thousand players on one persistent server (D52, D89). Rust on Bevy, our own voxel terrain. Pre-production: everything is current thinking.

## Tenets

- **Realistic but fun** (D122): things work as in the world, so players can reason about them (arrows ride in a quiver on your belt; what you know you found out by looking, D93). Where realism would only be a chore, fun wins.
- **Hundreds of variants, not millions** (D182): things are made from parts by rules, a theme (realm, maker, people) choosing which, in variants a player tells apart and cares about. A generator says how many distinct variants it makes and what tells them apart.
- **Port from the best prior art, in any engine** (D196): always look for the best-in-class implementation, wherever it runs, and rewrite it for ours; where Bevy falls short, never our own invention or another engine: its code under MIT, BSD, Apache or zlib (Godot, Voxel Tools, O3DE, Jolt), otherwise (Unreal, GPL) only its docs, papers and talks. Name the source in the code, with any licence notice it requires. How deep: D184.
- **One art style for every realm** (D14): read [docs/art-style.md](docs/art-style.md) before making, generating or judging a model, concept, material or icon, and check the result against its checklist. A realm changes parts, materials and light, never the rules.

## Where things are

- `docs/` designs and decisions: [README](docs/README.md) the map, [tech](docs/tech.md) the stack, [engine](docs/engine.md) the parts, [prototypes](docs/prototypes.md) the next milestone, [development](docs/development.md) every command.
- `research/` a dated note per question, linked from its README · `assets/` what every binary loads (Bevy's asset root, set in `.cargo/config.toml`) · `art/` source art · `crates/` the game · `xtask/` tasks (`cargo xtask tools`) · `tools/` tooling outside the game, each with a README · `patches/` Bevy crates we carry a change to (D184).
- `.agents/skills/` (linked as `.claude/skills`) playbooks for characters, models, icons, Tripo, Blender, environment, building, combat: use the matching one.
- `spike/` throwaway experiments, one question each, deleted once the research note is written; nothing lasting (a skill, tool, doc or game code) may depend on one; move it to `tools/` or the workspace first (D133).

## Start

- Read `docs/README.md`. Find decisions in the index `docs/decisions.md` by grep or its area's section, never whole; read one in full (`cargo xtask decision D28`) before building on it.
- Before building a component, read its doc and the sections of `docs/development.md` you need.

## Routine (D375)

- **Start**: the session hook fast-forwards a clean main checkout, updates the shared seed and seeds your worktree (D183). On failure run `cargo xtask doctor` and fix it before building. Entered a worktree mid-session, or ran `--done` there: `cargo xtask seed` first.
- **Land on `main` after each incremental change**, no pull requests: commit, `git pull --rebase origin main`, `git push origin HEAD:main`; again if main moved. Never force-push `main`, never `--no-verify`.
- **Small commits, often** (the developers): land each step that works and is checked on its own (a test, a fix, a measured change, a tool), as it's done; never squash a job's work into one commit or hold it to the end. Unchecked work stays in the worktree.
- **Hooks only format and lint** (D37): commit runs `cargo fmt --check`, push adds the size and one-definition lints and clippy on the changed crates, in seconds. Run the tests of what you changed yourself. There is no CI while we are pre-release; the full check is `cargo xtask check --all`, by hand.
- **Baselines**: lowering one is `cargo xtask nightly`'s alone (D414, D411).
- **Finish, then clean up at once**: `git fetch`, `git branch -r --contains <commit>`, then `cargo xtask worktrees --done` (D259) the moment the work is on main, never left for the prune; a coordinator removes each subagent's worktree as its report arrives, once its commits are on main (`git worktree remove`; a worktree with unlanded work keeps its branch).
- At most three or four agents build or check at once, subagents included.
- **Every task is bounded** (D400): a brief says when the agent stops (a score bar, a number of jobs or two without a gain, a clock time, a context of about 250k at most). Do one unit of work, write a handoff file, end; the next job is a fresh agent from it. Wait on a long run started in the background by its completion notice, never a polling loop, one wait per thing. An agent you start gets the same bounds, one helper at a time.

## Gates and tool output

- **Heavy tests run on main** (D398, D37): GPU renders, the fire sheet, look checks and benches are continuous checks (`cargo xtask continuous`, a row of `tools/continuous/checks.json`), never a hook. Keep a test to a few seconds; a sweep over many cases runs on every core (`terra_gen::par::map`, D406).
- **Gates are quiet** (D388): a line a stage. On failure read only what follows `✗`, then `target/check/<stage>.log`. `TERRA_VERBOSE=1` shows everything.
- **A new tool or task prints the same way** (D388): a line a stage on success, only what fixes a failure, the rest in a log it names.
- **An error says what to do** (D556): what went wrong with the thing named, the rule it breaks and its decision, the fix wanted or the next command (and what not to do where the tempting fix is wrong); never a bare complaint.
- **Code health is gated** (D396): split a file over 1,000 lines or a function over 150 (`cargo xtask sizes`); call a shared primitive from its home (`terra_gen::hash`), never copy it.

## Context

Every byte an agent reads is paid again each turn.

- **Keep context small** (D390): read by range or grep; send long output to a file and read its tail; when the context nudge fires, finish the unit of work and hand off with a summary.
- **Delegate mechanical work** (D391): finding to `finder` (Haiku), running a check, test or benchmark to `runner` (Sonnet). Judgement, design, debugging and fixes stay on the default model.
- **A job is a `job` agent** (D538), isolation "worktree": the brief says only the task, the skill, the bound and the handoff files (`~/.cache/terra-handoff/<topic>-j<n>.md`); the agent's file holds the routine.
- **A subagent's cache lasts an hour** (D397): a wait of minutes is cheap; past an hour the whole context is written again, so hand off instead.
- **Docs short and precise**: intent, constraints and reasons, not implementation or history. Here and in skills, one line a rule with its decision; the explanation lives in the doc it cites.

## Decisions (D389)

- Each is `docs/decisions/Dnnn.md`; edit those, never `docs/decisions.md` or research/README.md's table, then `node tools/docs/docs.ts --render`; git merges both indexes row by row (D409). The shape is in the index's preamble.
- They are the source of truth: don't contradict a live one, raise it. Decided is our answer, Leaning a working assumption, Open unchosen, Superseded kept so its number resolves.
- **Design** (gameplay, art, scope) is the developers': recommend, mark it Open, don't build on it; only they mark one Decided (D412, the docs check holds it). A developer thinking out loud is input, not a decision, unless they say it is settled.
- **Technical** (engine, architecture, tooling, workflow) is yours: decide, record why, keep going; the developers can overrule.
- **By** says who. An Agent decision is soft: any agent changes it with a reason and a log line. A Developers decision only they change. A choice other work builds on gets a decision; one left in chat or code didn't happen.
- **Revisit what proves wrong**, with evidence: change an agent's decision; raise a developer's with a recommendation.
- A change updates the decision's file and every doc it affects in one commit, with a line at the end of `docs/decisions-log.md` (D376). New numbers come from `cargo xtask decision`; on a collision whoever pushes second renumbers. The Why quotes the developers.

## How we work

- **Design before building** (D49): code waits for a design doc the developers have read. An idea from conversation is written down and asked about, not started as an experiment.
- **Don't overbuild**: nothing on a Leaning or Open design decision but a spike, which answers one stated question with the least code, then stops.
- **No new sessions** (the developers): never suggest or open a separate session (a task chip, `spawn_task`) and never ask about one; the coordinator spawns an agent or subagent for found work, and a subagent names out-of-scope work in its report instead.
- **The review desk** (D422): a question, or a still or film for the developer's eye, goes on their desk, never into chat; the coordinator keeps it and subagents hand items over in a `FOR REVIEW` line (`.agents/skills/review-desk`).
- **Tools for repeated work** (D71): when a check you will do again takes several manual steps, a restart or squinting at a picture, make it one step. General over single-case; its output reads at a glance and explains itself ([examples](docs/development.md#developer-tools)).
- **No options for one-offs** (D249): one scene or check uses existing options, `cargo xtask remote` or an uncommitted script; if none can, ask.
- **Do it as shipped games do**: find how they and other engines do it first, use the industry's terms, say where we depart.
- **Paid when it is much better** (D526): look for paid tools, libraries, assets and services too; propose one when it beats the free ones or solves what we'd find very hard, explaining on the desk what it is, what it costs and why we need it; the developers buy.
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
- **Tests** sit beside the code and need no GPU. A behaviour you rely on gets a test, and it fails when the behaviour breaks: test the outcome, know the answer another way, never pin tuned data ([how](docs/development.md#writing-tests)).
- **Check the real API**: read the pinned Bevy's source and examples under `~/.cargo/registry/src/`; never recall it. How far we change Bevy: D184.

## Checking your work

- **Checkable without a person** (D69): scripted runs (`--press`, `--shot`, `--vantage`, `--bench`), workbench sheets, `cargo xtask loop`, `cargo xtask remote`. Not done until an agent can drive it and see it worked.
- **Look at it**: `cargo xtask sandbox --offscreen --experiment play --shot <file.png>` (CPU-rendered under WSL; offscreen opens no window, so no "not responding" dialog, D417), `cargo xtask workbench` for models. Close every run you start.
- **Every unattended GPU run** goes through `cargo xtask gpu <still|sheet|film|bake|bench> -- <command>` started in the background (D319): renders start at once, side by side, with no queue; only a bench waits, for the card alone, holding back new runs meanwhile; `--status` shows what runs.
- **After a panic** (D399), read the `summary.txt` the run's last line names before any backtrace; see what an action changed with `r snap` and `r diff` ([tools/sandbox](tools/sandbox/README.md)).
- **Measure with `cargo xtask bench`** (D185): release, offscreen at 1920x1080, real GPU (under WSL the Windows build), against the last result; report the integrated GPU too. Software rendering's numbers mean nothing.
