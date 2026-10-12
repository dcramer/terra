# Debugging and QA tools for our agents

2026-10-12. Question: what is the state of the art in tools and patterns that help a coding agent build, debug and check a game, and what should we build or adopt?

Mostly read, not run: engine documentation, papers, studio write-ups and crate sources, much of it through search summaries (marked below where it matters). The pinned Bevy 0.19.1 was read in source. One thing was built and run: the first of the recommendations, described at the end. The companion note on agents *playing* the game is [agents-playing-games.md](agents-playing-games.md); this one is about agents *testing* it.

## Where we started

Scripted runs (`--press` on given frames, `--shot`, `--bench`), workbench sheets, and a CLI over Bevy's remote protocol for placing things and framing shots ([tools/sandbox](../tools/sandbox/README.md)). The play experiment's loop is checked by [loop.sh](../crates/terra_sandbox/loop.sh), which starts the game afresh for every step, presses keys on fixed frame numbers, and searches the log afterwards. That works, but a change of timing breaks it, every step pays a cold start, and when a step fails all it says is that a line never appeared.

## What every engine's agent tooling converges on

Roblox Studio (built-in MCP, 25 tools), Unity (an MCP bridge, and in July 2026 a CLI), Unreal 5.8 (an experimental MCP plugin), Godot (community servers) and Bevy (`bevy_brp_mcp`) have arrived at the same core:

- query the scene, run code in the live process, start and stop play, read the log, take a screenshot, inject input;
- and in the newer ones: a clock the agent can stop and step, a digest of the game's state as JSON, and commands the game itself defines.

The details worth copying:

- **Unity treats MCP as one client of a CLI**, not the centre. Its CLI evaluates code in a live editor or player, turns methods marked `[CliCommand]` into commands an agent can discover, and prints JSON with exit codes. A user on Unreal's forum said it "lets an agent check its own work in minutes".
- **The best-designed Godot server** (satelliteoflove/godot-mcp) freezes the clock, steps it, or steps until a condition holds; nodes opt in to reporting their state; its README says to take screenshots only when looks matter, since state is cheaper.
- **OpenAI's web-game skill** asks the game for `render_game_to_text()`, JSON that must match the screen, and `advanceTime(ms)`: the same two ideas.
- **What goes wrong.** Hands-on reports of Unreal's MCP: large tool lists made models loop, a search tool silently stopped at 20 results, an agent claimed a fix for controls that stayed broken, and another faked procedural generation with hand-placed meshes without saying so. Anthropic's guidance on tools for agents says the same: few tools, concise output, errors that say how to fix the problem.

**MCP or a CLI.** MCP adds nothing a CLI over the same protocol lacks for Claude Code, which runs commands, pipes their output and reads images from files. A CLI costs no context until used, composes with scripts, and is what a person runs too. What matters is what the game answers, not how the agent reaches it. Our own reading: MCP is worth a thin wrapper only if a client that cannot run commands needs it.

## Bevy and Rust specifically

Read in the 0.19.1 source unless marked.

- **The remote protocol has more than we used**: `world.query`, streaming watches over HTTP (`world.get_components+watch`, `world.observe+watch` for events), `world.trigger_event`, `world.write_message`, `registry.schema`, `schedule.list` and `schedule.graph`. A screenshot can be taken with core methods alone, by spawning a `Screenshot` and watching for its event; Bevy's own integration test does this.
- **`bevy_brp_extras`** (0.22.10 for Bevy 0.19, maintained) adds key, mouse and gamepad input, a screenshot cropped to an entity, and frame diagnostics as remote methods, with an MCP server beside it. We did not adopt it: input has to go through the same path as our scripted presses or a scripted run wipes it each frame; we already have screenshots and diagnostics; and every dependency on Bevy's version is another crate to wait for when we upgrade (D31). Its entity-cropped screenshot is worth copying if we want it.
- **`bevy_dev_tools::ci_testing`** runs a file of events on given frames (screenshots, camera moves, screen recording, custom events) with a fixed frame time. It overlaps our `--press` and `--shot`; the idea we did not have yet was the fixed frame time for every agent run.
- **`Stepping`** (feature `bevy_debug_stepping`) steps single systems and sets breakpoints in a schedule; it is not reflected, so it would need a method of our own. `TimeUpdateStrategy::ManualDuration` gives exact frame times, which is what we use.
- **Profiling for an agent**: `trace_tracy` with `tracy-capture -s N` on the Windows build, then `tracy-csvexport` gives per-system times as CSV, which an agent can read. `trace_chrome` writes a trace Perfetto can query with SQL.
- **Rerun** (0.38, very active) logs time series and space to a file an agent can query from Rust, and has its own MCP server for its viewer. `revy`, which logs a Bevy world's changes per frame into it, calls itself a proof of concept. Worth it later for questions over time; not needed now.
- **GPU debugging** is the weak spot: Bevy 0.19 switches wgpu's trace off in its source, and programmatic RenderDoc captures need `unsafe`, which we forbid. Launching the Windows build under `renderdoccmd` needs neither.
- **Bevy 0.20 shipped on 2026-10-08** with a seed that shuffles system order to flush out ordering bugs, and system panics turned into errors. Both are useful for testing when we upgrade.
- **Image comparison**: `dssim-core` is maintained; `image-compare` is quiet; the released `nv-flip` crate is from 2023. Bevy checks its own examples' screenshots on a hosted service, Pixel Eagle, not locally.

## Seeing bugs in pictures

Vision models are not yet reliable judges of screenshots:

- VideoGameQA-Bench (2025): the best models find about 83% of glitches in single images but only 45% in visual regression and around a third in a needle-in-a-haystack search; they invent clipping that is not there.
- A 2026 study of clipping found the best model at 85% on easy frames and 61% near walls, and one model's recall fell from 74% to 26% with a more detailed prompt.
- Giving a known-good reference frame beside the one under test raised F1 from about 0.49 to 0.74 (RESP, 2026). Glitches that only show across frames were near chance.

So pictures are for what only looks can answer, compared side by side with a known-good one, asked one specific question, with the game's own facts (bounds, overlaps, distances) doing the judging where it can. Our workbench sheets beside accepted models are already this shape.

## Determinism, replay and simulation testing

- **Riot** logs each frame's changed state as nested key-values and reports only the *first* divergence between two runs, having rejected checksums because they say that runs differ but not where. They replay 2,000 to 3,000 real games a day; making the server deterministic took about a year, mostly unifying six clocks into one.
- **Factorio** checks a cheap checksum every tick in multiplayer, and in a debug mode saves every tick and replays to the first mismatch, then diffs the two saves; the difference is usually one variable.
- **Deterministic simulation testing** (FoundationDB, TigerBeetle's VOPR, S2, Polar Signals in Rust): the real code in one process, with time, randomness and the network driven from one seed, so any failure replays from the seed. Polar Signals' shape is the one that fits a game tick: state machines with `receive(message)` and `tick(now)`, time only ever passed in. S2 runs every seed twice and diffs the logs to catch hidden nondeterminism. Antithesis has beaten Super Mario Bros., Zelda and Metroid this way, guided by "sometimes" assertions that prove a state is reachable.
- **Property tests over sequences** (`proptest-state-machine`, mature): generate sequences of actions against a simple model and shrink a failure to the shortest that fails. Inventory, crafting and trade fit: nothing duplicated, nothing lost, limits held. Antithesis's `hegeltest` is a preview and needs Python.
- **Procedural worlds**: Dwarf Fortress rejects worlds that fail hard checks and logs why; Spelunky builds a path through every level by construction; Minecraft's bug tracker has worlds that came out differently depending on the order chunks loaded. Sweeping many seeds with checks per world, plus a few golden seeds kept as snapshots, is the common practice.
- **Bevy** can refuse to build a schedule where two systems that touch the same data have no fixed order (`ambiguity_detection`), the commonest source of nondeterminism in an ECS.

## What we should do

Our recommendations, in order of value for effort. Technical, so ours to decide (D69), except where noted.

1. **Live, step-by-step testing in one running game.** Live key presses, a clock that steps exactly, a record of what the run said that a script can wait on, and a digest of the player's state. **Built today**, below.
2. **Move `loop.sh` onto it.** One start, then each step sets itself up with `warp` and `give`, acts, and waits for words or reads `state`. Snapshot and restore between steps (`play.load`) would make steps independent without a restart; the save system already writes most of what is needed.
3. **Determinism of the shared simulation from the start.** Time and randomness passed in, never read; `ambiguity_detection` set to fail on the simulation's schedules; a test that runs the same seed twice and compares. Cheap now, very costly later (Riot's year).
4. **A state dump per tick and a tool that names the first difference** between two runs, which serves replays, desyncs and simulation testing alike.
5. **Property tests over action sequences** for the inventory and crafting crates (`proptest-state-machine`), with invariants checked every tick in test builds.
6. **Seed sweeps for world generation**: many seeds, a few checks per world (the spawn and portal reachable, what must be found within reach), and golden seeds kept as snapshots. Waits on world generation being decided.
7. **Screenshots compared with a known-good one**, with a measured tolerance, for the workbench and the item icons (D129), using `dssim-core`.
8. **Later**: a bot swarm with a simulated network for the server (lightyear's link conditioner, `tc netem`); a Tracy capture exported to CSV for performance questions; Rerun for questions over time; Sentry with minidumps for crashes once others play.

Not recommended now: an MCP server of our own, `bevy_brp_extras`, Antithesis (costly, aimed at distributed systems; worth a look once there are several servers), time-travel debuggers (rr needs hardware counters that WSL usually lacks; vendors' claims for agents are anecdote apart from one benchmark, where a debugger raised a model's score from 37% to 48%).

## What was built

The sandbox's remote protocol gained, in [remote.rs](../crates/terra_sandbox/src/remote.rs), [record.rs](../crates/terra_sandbox/src/record.rs) and the play experiment's [probe.rs](../crates/terra_sandbox/src/experiments/play/probe.rs):

- `sandbox.press`: a key held for so many frames in a live run.
- `sandbox.step`: the clock run for so many frames, then stopped.
- `sandbox.said` and `sandbox.status`: every line the log keeps, numbered, with its frame, kept in memory.
- `play.state`, `play.warp`, `play.give`: what is true of the player and what is near, and the same setting up `--warp` and `--give` do, through the same code.
- `--steady`, a fixed sixtieth of a second a frame, on by default from the CLI.

The CLI gained `press`, `step`, `status`, `said`, `wait`, `mark`, `state`, `warp` and `give` ([README](../tools/sandbox/README.md)). Tried on the Windows GPU: one start, then a bow, quiver and arrows given, the player warped beside the deer, the bow drawn and loosed, all checked by waiting for the game's own words, with `state` saying where the deer were. Stepping 30 frames moved the game's clock by exactly half a second. The arrow went over the deer, which is a question of aim for the game, not the tools.

## Built since

On the developers' go-ahead the same day, the next four of the recommendations, kept small:

- **The loop moved onto the live commands** ([loop.ts](../tools/sandbox/loop.ts), `cargo xtask loop`): two starts instead of eleven, each step setting itself up and asking the game where things are. Moving it found two faults the scripted loop had hidden: quitting over the remote protocol skipped the save, since remote requests are answered after the frame's last systems; and arrows aimed by turning the player toward a target miss, since the bow shoots along the camera's line, which sits off the shoulder. The first is fixed; for the second, `play.aim` turns the view until the crosshair is on a spot. Playing on in one game also showed what a fresh start per step had hidden: the body and the pack carry over, so a step must set up what it relies on (full health before a fight, room in a pack where every raw item takes a cell), and a give that did not fit only warned. Now it fails, and `play.take` clears a pack.
- **A trace and a first-difference tool** ([diverge.ts](../tools/sandbox/diverge.ts)). Its first use showed the trace itself had to name creatures and people by who they are, since the world lists them in a different order each run; after that, two runs of the scripted hunt agreed on all 422 frames with every number exact.
- **Repeatability of shared code** as a rule in `cargo xtask check`, not a test per crate: no clock, no system randomness. The shared crates passed already; the chunk tree builds from a hash set but sorts what it returns.
- **Counts under random actions** in `terra_items`, beside the existing test for moves, using the crate's own seeded streams rather than a new dependency.

Not done: Bevy's ambiguity detection on the sandbox's schedules, since nothing that runs on a server lives there yet; it belongs on the server's schedules when they exist.

## Limits

- Most claims about other studios and engines are from write-ups and search summaries, not from using the tools; vendor claims are the vendors'. The vision benchmarks are on other games' pictures, not ours.
- Nothing here measures how much faster an agent checks a feature with the new commands; that will show when `loop.sh` moves onto them.
- One sandbox stopped without a word in its log during this work. Most likely another session's `stop` reached it, since every checkout then listened on the same port; each checkout now has its own (`shots/remote.port`).

## Sources

- Engines: [Roblox Studio MCP](https://create.roblox.com/docs/en-us/studio/mcp.md), [Unity MCP](https://docs.unity3d.com/Packages/com.unity.ai.assistant@2.0/manual/unity-mcp-overview.html), [the Unity CLI](https://activation.unity3d.com/blog/meet-the-unity-cli), [Unreal 5.8](https://gamefromscratch.com/unreal-engine-5-8-released/), [Unreal MCP hands-on](https://www.pugetsystems.com/blog/2026/07/09/unreal-engine-mcp-hands-on-testing-ai-inside-the-editor/), [Unreal forum thread](https://forums.unrealengine.com/t/how-are-people-making-local-llms-reliably-control-unreal-engine-5-8-through-mcp/2746458), [godot-mcp (satelliteoflove)](https://github.com/satelliteoflove/godot-mcp), [godot-mcp (Coding-Solo)](https://github.com/Coding-Solo/godot-mcp), [OpenAI develop-web-game skill](https://skills.sh/openai/skills/develop-web-game) (summary), [Writing tools for agents](https://www.anthropic.com/engineering/writing-tools-for-agents)
- Bevy: [bevy_brp](https://github.com/natepiano/bevy_brp), [remote integration test](https://github.com/bevyengine/bevy/blob/main/examples/remote/integration_test.rs), [example showcase](https://github.com/bevyengine/bevy/blob/main/tools/example-showcase/src/main.rs), [profiling](https://github.com/bevyengine/bevy/blob/v0.19.1/docs/profiling.md), [Bevy 0.20](https://bevy.org/news/bevy-0-20/), [Tracy](https://github.com/wolfpld/tracy), [Rerun data out](https://rerun.io/docs/howto/get-data-out), [Rerun viewer MCP](https://rerun.io/docs/reference/viewer/mcp), [revy](https://github.com/rerun-io/revy), [sentry-rust](https://github.com/getsentry/sentry-rust), [crash-handling](https://github.com/EmbarkStudios/crash-handling)
- Pictures: [GlitchBench](https://arxiv.org/html/2312.05291v2), [VideoGameQA-Bench](https://arxiv.org/html/2505.15952v2), [clipping during exploration](https://www.alphaxiv.org/abs/2607.25921.md), [RESP](https://www.alphaxiv.org/abs/2604.11082.md), [TempGlitch](https://papers.cool/arxiv/2605.21443)
- Determinism and testing: [Riot on determinism](https://www.riotgames.com/en/news/determinism-league-legends-fixing-divergences), [Factorio FFF 47](https://factorio.com/blog/post/fff-47), [FoundationDB](https://apple.github.io/foundationdb/testing.html), [TigerBeetle VOPR](https://docs.tigerbeetle.com/about/vopr), [Polar Signals](https://blog.polarsignals.com/blog/posts/2025/07/08/dst-rust), [S2](https://s2.dev/blog/dst), [Antithesis and Metroid](https://antithesis.com/blog/2025/metroid/), [Antithesis and Zelda](https://antithesis.com/blog/zelda/), [Hegel](https://antithesis.com/blog/2026/hegel/), [proptest state machines](https://tzemanovic.codeberg.page/posts/state-machine-testing-with-proptest/), [Dwarf Fortress world rejection](https://dwarffortresswiki.org/index.php/40d:World_rejection), [Minecraft MC-55596](https://bugs.mojang.com/browse/MC-55596), [Metaplay bot clients](https://docs.metaplay.io/feature-cookbooks/automated-testing/botclient-testing)
- Debuggers: [Undo AI](https://docs.undo.io/HowUndoAIWorks.html), [debug-gym](https://simonwillison.net/2025/Mar/31/debug-gym/) (summary)
