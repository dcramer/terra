# The launcher and diagnosing performance

Status: the agents' technical design (D575 to D578) and the developers' title screen with its worlds (D579, Decided 2026-10-30); build plan steps 1 to 5 built, step 6 (the title screen) being built; the load's budget is Open for the developers (D580). Tools, so [development.md](development.md) is where each part is used once it lands.

The developer, 2026-10-29: `make play` should open a launcher a normal person can use, to continue, start a new game, play creative or the yard, and choose profiling, the build and the quality preset; it remembers the last choice, and scripts and agents drive it by flags (D69). And performance should be easy to diagnose: perf round 1 (`~/.cache/terra-handoff/perf-j1.md`) took a trace build, the Windows build launched by hand (`cargo xtask win --features trace`), a 17 GB Chrome trace grepped down before `tools/sandbox/spans.ts` could read it, and a raised Node heap, to learn that the first frame spent 5.1 s in `terra_world::mipmaps` and that 1 to 3 s freezes every 10 to 20 s in the developer's play never showed in a release run.

## Prior art

- **Unreal's Project Launcher** keeps **launch profiles**: a named set of build configuration (Debug, Development, Shipping), cook, deploy and launch command line, with **Quick Launch** for the usual one; **BuildCookRun** is the same from the command line. **UnrealGameSync**, Epic's developers' front end, remembers the build configuration and the editor's arguments as checkboxes between runs.
- **The Minecraft Launcher** keeps installations (a version and its arguments) and its Play button starts the last one.
- **Unreal's hitch log**: with `stat DumpHitches`, every frame over `t.HitchFrameTimeThreshold` is written to the log with the stats that ran long in it, from stats a development build gathers anyway. Its **CSV Profiler** writes cheap per-frame numbers in test builds and **PerfReportTool** turns a session into a short report; **Unreal Insights** captures the detail, its trace **channels** chosen at launch so a capture holds only what was asked for.
- **Bevy** offers `trace_chrome` (what we used) and `trace_tracy`; its profiling guide recommends Tracy.

We take: a **launch profile** (what to play, build configuration, profiling, quality preset), the last one the default and the same options as flags; a **hitch log** from what every build gathers; a **session report** written by the game, not a trace for a tool to digest; and Tracy for the detail.

## The launcher (D575)

**Where it lives: a menu in the terminal, in `cargo xtask play`.** `make play` already opens a terminal; the launcher is what `cargo xtask play` shows when it is run in one with no choices on its command line. Three reasons:

- **The build comes first.** Debug or optimised, and a Tracy build, are chosen before Cargo runs; a choice made inside the game would have to quit, rebuild and start again.
- **Builds run where the code is.** Under WSL the build runs in Linux and the game in Windows; xtask already builds, cooks, stages the copy and starts it (`xtask/src/windows.rs`). A window on the Windows side would have to call back into WSL to build; one in WSL would look foreign and is a second program for macOS and Linux. The terminal works everywhere `make play` does.
- **One path for people and scripts.** The menu only fills in the flags `cargo xtask play` already takes; whatever it can choose, a flag chooses (D69).

Rejected: a native launcher window (two programs, a WSL bridge, nothing it shows that a terminal cannot); the game's own front screen for the build's choices (too late, see above). The player's choices are the game's title screen's (D579, [below](#the-title-screen-d579)).

**What it shows.** One screen of the build's choices, the last ones preselected, so Enter alone plays again, on the game's title screen:

```
Terra   main 3f2a1c

  Build: optimised   Profiling: off      o  change these

Enter plays: the game opens on its title screen.  o options, q quit.
```

- **Options** (`o`): build `debug` or `optimised`; profiling `off`, `report` (a session report, below) or `live` (a Tracy build with Tracy's profiler open beside the game, and the report too; shown `live (Tracy)`). A choice that needs a build not yet made says so and how long the first takes (a Tracy build compiles Bevy again, minutes; switching back is a link). The quality preset is the game's own setting (its options menu), so the launcher no longer asks; `--quality` still goes to the game.
- **Remembered** in `~/.config/terra/launch.json` (outside the repository, one per machine): the last options. No named profiles until someone asks for a second.
- **Flags**: `cargo xtask play [continue|new|creative|yard] [--build debug|optimised] [--profiling off|report|live] [--last] [-- game args]`. A word chooses what to play and goes straight in, no title screen: `continue` the world played last, `new` a new world, `creative` the creative world played last (a new one if there is none), `yard` the yard. With no word the game opens on its title screen in a terminal and plays `continue` with none (stdin not a TTY, as for agents and scripts). Any argument, or no terminal, skips the menu; an option left out is the default, not the remembered one, so a script's run never depends on what someone played last. `--last` plays the remembered options; `--menu` shows the menu anyway. `ARGS` passes through `make` as now.
- **Make**: `make play` is `cargo xtask play $(ARGS)` as today; `make fresh`, `make creative` and `make yard` stay as one-line shortcuts that pass their word (D387: the Makefile holds no logic).
- **Whatever `play` does, the launcher does.** Another job is changing how `make play` cooks textures (D188) and may make it an optimised build by default; the launcher is a menu over that path, so it is built after that lands and takes its default build from it.

## The title screen (D579)

The player's choices live in the game, as in Minecraft, Valheim and Enshrouded, which keep a list of worlds, each made with a name and its mode and deleted only after asking.

- **What it shows**: the world itself behind it, loaded round the start, seen from a fixed camera over the start's lake to its wooded shore at five in the afternoon, the clock held and the game's interface put away, as Valheim and Enshrouded open on their worlds; darkened at the edges and deeply down the left under a column of choices: **Continue** (the world played last, by name; greyed out when there is none), **Worlds** (the list, the last played first, each with its mode and when it was played: Enter or a click plays it, Delete or its ✕ asks "Delete <name> for good?" before it does), **New world** (a name, filled with the next free "World n", and Survival or Creative, D494, kept with the world; Create plays it), **The yard**, **Settings** (the options menu, D80) and **Quit**.
- **Input**: mouse and keyboard (arrows or W and S to move, Enter or Space to choose, Esc back, typing into the name). The game has no gamepad support yet; when it gets one, the title screen takes it with the rest.
- **Worlds on disk**: a folder each under `worlds/` in the player's settings folder, holding the save (`save.ron`, as before) and `world.ron` (its name, mode, when made and last played). The save from before worlds is moved, untouched, into the first world, "My world", the first time the game finds no worlds; no converter (D353).
- **How a choice plays**: the title screen is a `Screen` between the first load and play, the clock held. A world's save is read only once it is chosen; the game then loads again round where its player stands, as the loading screen does for any screen (D135). A Creative world builds free (`--free-build`'s rule). The yard is another level, made as the game starts, so the game ends asking to be started again on it (exit code 76), and `cargo xtask play` starts it in the same terminal with the same build and profiling, each game reported; a game started otherwise just ends. A world chosen is timed from the choice: the second `Loaded in` line says so.
- **When it shows**: only when asked, `--title`, which `cargo xtask play` passes when nothing is chosen in a terminal; a run with no `--title` plays as before, so no script, test, bench or agent's run meets it. `--worlds <dir>` keeps a run's worlds elsewhere (a scripted run keeps none without it), and `--world <name>`, `--fresh` (a new world) and `--creative` choose without the screen.
- **Quit to title**: the options menu offers it while a world is played; the game saves as it ends and asks to be started again on the title screen (exit code 75), as for the yard. Both start a fresh game because the yard is made as the game starts and a played world cannot yet be emptied in place for another; doing it in one process needs the world reset (the ground's edits, the base, props, people and items) or the level made again, a job of its own.
- **Not yet**: a seed: the land is one world for now, so a seed would choose nothing.

## Diagnosing performance

Four needs, each with one tool: what froze just now, in any session; what a session spends; the detail, live; and holding the line.

### The hitch log, in every build (D576)

A **frame recorder** in `terra_world` keeps timed spans in a buffer a thread, drained at each frame's start. In every build, **marks** between each world's top-level schedules (a one-system schedule reading the clock, put before the first and after each in `MainScheduleOrder` and the render thread's `RenderScheduleOrder`) time the schedules and the world's update. A `hitch-log` build (Bevy's `trace`, which costs about 15% of a frame compiled in, so only `cargo xtask profile` builds it) times every system instead, through a `tracing` layer over Bevy's per-system spans. Our own spans round heavy work (a load step, a mesher) are timed in either.

- **A freeze**, a frame of 100 ms or more, gets one `warn` line of its own when it ends: its length, which world held it (the game's or the render world's), and the five longest schedules (systems, in a `hitch-log` build) or spans in it, with the time outside anything timed (task-pool work, a wait on the GPU or a lock, the other world). Example from a release build: `freeze: frame 7 took 114 ms, held by the render world: Render schedule 105 ms, First schedule 0.0 ms; 9.5 ms outside anything timed (…)`.
- **Hitches**, frames over twice the budget (`STUTTER_MS`), keep the minute's counted warning (`terra_world::debug`), which now names the system longest in most of them.
- **The load** gets the same: the `Loaded in` line (`terra_world::loading`) names its five longest steps, so a slow load says why without a trace (round 1's 5.1 s of mipmaps).
- **Cost**: every build, so at most 1% of the median frame. The marks cost 3 to 10 µs an update (eight in the game world, three in the render world's), about 0.1% of a 10 ms frame, below what `cargo xtask bench` can see; Bevy's `trace` cost 15%. The numbers are in D576.
- **Threads**: the game and render worlds run side by side (pipelined rendering, kept: unlike the Chrome `trace` build, nothing here changes how frames run); a system is put in its world by the schedules read when the app starts.

### The session report (D576)

`--profile-report=<file>` (the launcher's `Profiling: report`; it also switches the system spans on, `record::filter`) has the recorder also keep, in memory and bounded, each frame's schedule times and its longest systems, and a histogram a system; when the game exits it writes a report of at most about a hundred lines and the task prints its head:

- the load's longest steps;
- the frame by schedule, median and 99th percentile, game and render world;
- the ten slowest frames, each with its longest systems;
- the systems by 99th percentile and by total.

This is what a script once read from a Chrome trace (`tools/sandbox/spans.ts`, deleted with the `trace` feature once the report agreed with it on one recorded run), computed while the game runs, so no trace file is written at all.

It is written every ten seconds as well as at exit, so a session that crashes or is killed still leaves one; a frame is the time from one end of the game world's update to the next, and a span counts in the frame it ends in (a system by its own time, a schedule by its wall clock). Each name's times a frame are kept as a histogram (sixteen buckets a power of two, about 4%), so a session of any length costs a few megabytes at most.

**One command** (built, step 4): `cargo xtask profile [walk|play] [--seconds 60] [--build optimised|debug] [-- game args]` builds with `hitch-log`, runs the play experiment on the real GPU (under WSL the Windows build, as `play` does, in a copy of its own), either walking `play-walk`'s new game and route offscreen for `--seconds` by the clock after the load (`walk`, the default; the game's `--profile-seconds`, as its scripted frames are each a sixtieth of a second of play however fast they are drawn) or played by a person until they quit (`play`, no cap: the report is bounded however long), and prints the report's head (each section's first lines) and the path of the whole in `logs/`, the newest ten of each kind kept. An agent hands the path on; the developer reads twenty lines.

### Live: Tracy (D577)

[Tracy](https://github.com/wolfpld/tracy) shows every span on a timeline, live, and keeps a capture to open later.

- **Versions**: the pinned Bevy 0.20's `trace_tracy` (bevy_log's `tracing-tracy` 0.12, `tracy-client` 0.19, `tracy-client-sys` 0.30.1) is Tracy 0.14.1's client, Tracy's latest release (2026-08-22). The profiler must be the same version as the client, so the first `--tracy` run fetches that release (`windows-0.14.1.zip`, or Linux's or macOS's) from GitHub, checked against the SHA-256 the release publishes, into `%LOCALAPPDATA%\terra\tools\tracy-0.14.1\` under WSL, as the developer allowed (`~/.cache/terra/tools/` elsewhere); nothing is installed system-wide. `cargo xtask doctor` reads the client's version from its source (`TracyVersion.hpp`) and the tools' from `tracy-csvexport -V`; a Bevy update moves both together (`xtask/src/tracy.rs`).
- **Licence**: Tracy is BSD 3-clause and `rust_tracy_client` MIT or Apache 2.0, both allowed (D196). Its client is compiled only into profiling builds, never into one we hand out; one that is would carry Tracy's notice.
- **The build**: a `tracy` feature on `terra_sandbox` (terra_world's: `bevy/trace_tracy` and `hitch-log`, so the recorder and the report run as in a report build). Every system's span is let through to Tracy, and the renderer's frame mark (kept out of the log). The client listens on 127.0.0.1 only (`tracy-client`'s `only-localhost`), so the game is not open to the network nor asks Windows' firewall. MinGW's libstdc++ is linked into the Windows build (`.cargo/config.toml`). Not `trace_tracy_memory`, whose allocation tracking slows every allocation.
- **Live**: `cargo xtask profile play --tracy`, the launcher's `Profiling: live (Tracy)`, builds it, opens `tracy-profiler.exe -a 127.0.0.1`, which connects as the game starts, and plays; the session report is written at quit as with `report`. Under WSL both run in Windows and meet on Windows' own localhost, so WSL's network is not in the way.
- **A capture**: `cargo xtask profile walk --tracy` starts `tracy-capture -s <walk + 120 s>` before the build; it waits for the game, records from its start (the client keeps everything until something connects), in Tracy's compressed format, and is kept in `logs/` (the newest three) with `tracy-csvexport`'s CSV of every zone's times beside it; the task prints the costliest zones by total and by longest run under the report's head. Context switches and sampling need the game run as administrator on Windows; spans do not.
- A Tracy build's times run a little long; judge a change by the benchmark, never by Tracy.

### The play benchmark (D578)

Bench scenes settle 240 frames before timing, so neither the load nor the first minute's streaming, where the developer's freezes are, is measured. A **play scene**, `play-walk`, is timed from the program's start:

- **Load**: the wall clock from start to the first playable frame (the `Loaded in` line), and its longest steps from the recorder.
- **The walk**: a new game (a scripted run has no save, so the same world on every machine), walked by `--route` for 60 s out from the start through ground not yet streamed, every frame timed: median, 99th percentile, worst, hitches and **freezes** (frames of 100 ms or more).
- **Warm**, as a player's second launch: cooked textures present (D188); the run says if the cook cache was empty, round 1's cause, and fails then, since a number from it means nothing.
- **Ratcheted** like the others: recorded in `research/benchmarks.csv` with two more columns, load seconds and freezes, compared with the last for the same GPU and build, marked `SLOWER` past the same margin; a freeze fails the scene, as a hitch fails a settled one. Lowering a baseline is nightly's (D414). How long a load may take a player is design, so the load is compared, not held to a budget, until the developers set one (D580).
- `--build debug` runs it in the debug build for the A/B round 1 left open; only the optimised build is ratcheted.

### Keeping files small

No trace file for the report; Tracy's captures capped by `-s` and compressed; the Chrome trace retired. Nothing a run writes for diagnosis should pass a few hundred megabytes.

### The entry point

[development.md's "Diagnosing performance"](development.md#diagnosing-performance) is where a person or agent starts: what you saw (a slow load, a freeze, a low frame rate, a regression) and the one command for each. It lands with this design, naming today's tools, and each step below rewrites its line.

## Build plan

Each step lands alone, checked, in order:

1. **The hitch log**: the frame recorder, `bevy/trace` in every build, freeze lines, the hitch warning's system and the load's steps; tests on the recorder's buffers and report without a GPU; the cost measured on `world` and `town300` and stated in the commit (D576 to Decided or to schedules only). Built: `bevy/trace` cost 15%, so systems are timed only in `hitch-log` builds, and every build names schedules by marks at about 0.1% (perf round 9, D576 Decided).
2. **The play benchmark** (done 2026-10-29): `play-walk`, load and freeze columns, the cook-cache check, a first baseline recorded (RTX 5090: load 13.3 s, no freeze); the debug-against-optimised A/B written to the perf handoff.
3. **The launcher** (done 2026-10-29, `xtask/src/launcher.rs`): the menu, remembered launch, flags, the confirmation before a new game; `make` shortcuts unchanged. The profiling option and `--profiling` come with steps 4 and 5, which give it something to choose. A new game is remembered as Continue, so `--last` or Enter after one plays the world it made rather than replacing it again.
4. **The session report and the `profile` task** (report, task and launcher option built 2026-10-29, `terra_world::recorder::report`, `xtask/src/profile.rs`; on one walk recorded both ways, the report agreed with spans.ts within 2% on each world's update and schedules and named the same slowest frames and systems; the Chrome `trace` feature, its recipe and spans.ts then deleted, so the report is the one way to profile, done): the launcher's `Profiling: report`.
5. **Tracy** (built 2026-10-29, perf round 10, `xtask/src/tracy.rs`): the `tracy` feature, the fetched profiler and its doctor check, `profile walk --tracy`'s capped capture summarised by `tracy-csvexport`, the launcher's `Profiling: live (Tracy)`.
6. **The title screen and its worlds** (D579): the worlds on disk and the save moved into the first; the `Title` screen in the play experiment (`terra_game::play::title`); the launcher down to the build's choices, its words going straight in; a scripted run pressing through the title screen into a world and into a new Creative one.

## Open questions for the developers (D580)

- How long may a load take, warm, on the developers' machines (D580)? A budget for the play benchmark, as 13.3 ms is for a frame; recommended 10 s.
