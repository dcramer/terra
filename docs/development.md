# Development

How to set up a machine and work in the repository. The reasons behind these choices are in [tech.md](tech.md) and [decisions.md](decisions.md).

## Set up

Clone the repository, then from its root:

```bash
make setup
```

It installs what Cargo cannot install for itself, when it is missing: rustup and the pinned compiler, Xcode's command line tools on macOS, and Bevy's libraries on Debian or Ubuntu (with `sudo`; under WSL also MinGW and the Windows target). It then points git at the hooks, builds the game and runs `cargo xtask doctor` (D387). Node it only checks for. `make` lists the everyday tasks, each a `cargo xtask` task; the Makefile stays that thin, and anything with logic goes in `xtask`.

`cargo xtask doctor` reads the pinned compiler version from `rust-toolchain.toml`, installs nothing itself, and lists what is missing with the command that fixes each item. The first `cargo` command in the repository downloads the pinned compiler.

| Platform | What you need beyond rustup |
|---|---|
| Linux, including WSL | `pkg-config`, `libasound2-dev`, `libudev-dev`, `libwayland-dev`, `libxkbcommon-dev` |
| WSL, to use the Windows GPU | `mingw-w64`, and the `x86_64-pc-windows-gnu` Rust target |
| macOS | Xcode command line tools |

CI can build and test on Linux, macOS and Windows, run by hand for now. The game runs on macOS natively, and an Apple M4 Max is among the benchmark machines.

## Everyday commands

Every task and tool starts from `cargo xtask`, from anywhere in the repository; `cargo xtask tools` lists them all.

| Command | What it does |
|---|---|
| `cargo xtask tools` | Every task and tool, with what it does |
| `cargo xtask play` | Play the MVP (D105) on the real GPU, wherever you are; your game is kept between runs. `cargo xtask play --fresh` starts a new one. |
| `cargo xtask yard` | Play the yard: a flat test level for working on building. Smooth ground with nothing on it, building free and instant without a claim stone, the sun at noon, nothing saved. The same as `--level yard` on the play experiment, so the sandbox remote can drive it. |
| `cargo xtask demo` | The construction demo's pictures: the cabin built by `cargo xtask cabin`, then the village (street, green, from a rise and above, each building close up), a stone house going up as ghosts and the cabin, from fixed views, in `shots/demo/` ([tools/sandbox](../tools/sandbox/README.md#scripts)) |
| `cargo xtask vantage` | The vantage sheet: the lush forest from fixed views at 13:00 and 17:00, side by side in `shots/vantage/sheet.jpg` ([tools/sandbox](../tools/sandbox/README.md#scripts)) |
| `cargo xtask looks [--place lake]` | The looks check: each place's views held to the art style's rules for places, a table of each number against its reference, pass or fail ([art-style.md](art-style.md#places), D356) |
| `cargo xtask film` | A video of the forest walk, or any scripted run, at the game's own speed, in `shots/film.mp4` ([tools/sandbox](../tools/sandbox/README.md#scripts)) |
| `cargo xtask trailer` | The trailer ([trailer.md](trailer.md)): a run through each place, the game's player running and its camera following, filmed with a held exposure, motion blur and a letterbox (`--cine`), cut with crossfades over the game's own sounds (`--sound-bed`), in `~/.cache/terra-review/trailer/`; `--stills` for a quick look at each run's framing |
| `--timelapse settlement.ron@600` | A settlement rising (D381): with `--experiment town`, the town's buildings put up from their plans by the hammer's own steps as a time-lapse, its builders at their sites and the sun through the day, as `assets/data/timelapse/settlement.ron` says; filmed with `cargo xtask film -- <options> --cine tools/sandbox/timelapse/settlement.json@600` 
| `cargo xtask qa clips --set <kit>` | QA clips for people to watch, the same every time: 10 to 30 s each of one stage of building a cabin (walls, openings, roof, a material swap, carving, a storey, a deck) and a house brought down, in a set of kits (`--set all` for the building baseline's, D333), each with its frames timed in real time against 13.3 ms; `shots/qa/index.html` has a row a set. Other scenarios (the whole cabin as one film, harm, materials, shapes, the castle) by name ([tools/sandbox](../tools/sandbox/README.md#scripts)) |
| `cargo xtask collide` | Walk the player into the lush forest's rocks, boulders, stumps, logs and trees and check each stops them ([tools/sandbox](../tools/sandbox/README.md#scripts)) |
| `cargo xtask lodcheck` | What pops as detail changes (a crown clipping out, a shadow clipping in), from still views while every switch distance is swept; fails past a limit ([tools/sandbox](../tools/sandbox/README.md#scripts)) |
| `cargo xtask pops` | What pops in or out, or flickers, frame by frame through a stretch of the forest walk, worst frames marked in `shots/pops/pops.jpg` ([tools/sandbox](../tools/sandbox/README.md#scripts)) |
| `cargo xtask loop` | Play the MVP's loop by script and check every step happened ([QA for agents](#checking-the-game-without-a-person-qa-for-agents)) |
| `cargo xtask bench` | Time the benchmark scenes ([Measuring performance](#measuring-performance)) |
| `cargo xtask check` | The full check, run by git before every push (`.githooks/pre-push`, D37): formatting, lints, every test (the TypeScript tools' too), the kit checker (`cargo xtask kits`), the foliage check (`cargo xtask foliage`), the artifacts and fuzz ratchets (`cargo xtask artifacts --quick --ratchet`, `cargo xtask fuzz --ratchet`), the docs check ([tools/docs](../tools/docs/README.md)), the Blender scripts' check (skipped with no Blender) and the dependency rule. Must pass before a push. |
| `cargo xtask check --quick` | The quick check, run by git before every commit (`.githooks/pre-commit`): the full check but the slow sweeps (`SLOW_TESTS` in `xtask/src/check.rs`, each marked at the test), the artifacts and fuzz ratchets and the Blender scripts' check; under two minutes once built. `cargo test` still runs every test. |
| `cargo xtask decision [D28 D198 …]` | The next free decision number, here and on origin/main after a fetch; given numbers, those decisions in full, each with the log's lines that name it (D389) |
| `cargo xtask sections` | How long the bench's village takes to lay whole and its sections to cut and merge, on one thread as the game's workers do, against building's 2 ms bar, with the triangles of each detail and the slowest sections; a name times only the buildings whose names hold it, and `BY_KIND=1` splits the far details' triangles by kind of piece (D238); it also gives near detail's triangles and MiB by what they are drawn in, and fails when the block kinds' surface and dress take over 120 triangles a cube of them or 4 MiB a building (D284) |
| `cargo xtask artifacts` | What a renderer would draw as junk in building's meshes, found from the meshes with no GPU: z-fighting (two surfaces in one plane), a piece drawn twice, sky through a wall or a roof, a gap between a wall's top and the roof, a piece poking through a roof or out of a wall, and corners that are not numbers. Every scene of the scene registry (`terra_build::scenes::registry`, D287) in the baseline's materials (D333, the settlement's), at every detail; prints a table of counts and the first of each, `--all` for every one; a name keeps only the scenes whose names hold it, in any material; `--quick` the scenes held before a push, `--ratchet` against the baseline, `--record` to lower it |
| `cargo xtask fuzz` | Small bases built at random from seeds as a player builds, through the same mesh checker; a base with a fault shrunk to its fewest elements and printed as a test to paste. `--seeds N`, `--from S`; the full check holds seeds 0 to 200 to the baseline (`--ratchet`, `--record`) |
| `cargo xtask look <material>` | The material bench (D386): the way to judge a change to a material ([Judging a material](#judging-a-material)) |
| `cargo xtask texels` | Where building's textures are stretched or smeared, found from the meshes with no GPU: for every triangle drawn near, how much longer its texture is drawn one way than the other against the metres its kit names, as each kit's share of area past 1.5:1 and 2.5:1 and the worst places in a base's own metres. A plank's side taking one column of its board, or a texture smeared round a rounded edge, shows here; a name keeps only the bases whose names hold it, `--worst N` lists more places |
| `cargo xtask fields` | The field maps (D326): a region of the world's land on a date as maps of its rules, on the CPU in seconds, no GPU: height, steepness, which way the ground leans, the growing season's mean temperature with the tree line's isotherm, snow lying on the date, shelter from the prevailing wind; `fields.json` with their numbers and a table of the tree line and snow line at each latitude; `index.html` to look at them, in `shots/fields/`. `--at E,N` kilometres from the planet's start, `--size`, `--cell`, `--date` as the sandbox takes it, `--radius` (30 km by default, the starting world's), `--site lake` for the region round the place a site query finds, with the walk round its shore a route can follow. Run it before drawing a place: a tree line or a snow line at the wrong height shows here first |
| `cargo xtask kits` | The kit checker: every building kit file in `assets/data/kits/` has the materials its pattern needs with all their maps, life-size numbers, and keeps to the style guide's budgets, measured on a cabin laid in it (`terra_build::checker`). |
| `cargo xtask foliage` | The foliage check (D368, [art-style.md](art-style.md#places), rule 12): every leaf, needle and frond card the plants wear, measured from its atlas and normal map on the CPU and held to what reads as leaves: leaves a card, texels a leaf and a pixel at 2 m, relief and tilt, a thickness map, detail, colour; a table a card, known failures listed with why in `tools/textures/foliage.json`. `--card <name>` one card, `--draw <dir>` the leaves it counted. `check` runs it. |
| `cargo xtask contract` | The model contract's checker (D303, [art-style.md](art-style.md#the-model-contract)): every model the game places, its size against its catalogue `dims`, its anchor, its sockets, and a door's or shutters' fit in its hole, one line each. `check` runs the same test. |
| `cargo test -p terra_items` | One crate's tests |
| `cargo fmt --all` | Format everything |
| `cargo xtask sandbox` | Open the sandbox, the test environment for graphics and utilities; anything after it goes to the sandbox, and `--list` lists its experiments. See its [README](../crates/terra_sandbox/README.md). |
| `cargo xtask workbench` | Models in the game's renderer ([Developer tools](#developer-tools)) |
| `cargo xtask remote <command>` | Drive a running sandbox live ([tools/sandbox](../tools/sandbox/README.md)) |
| `cargo xtask contract` | The model gallery in a browser, at `http://localhost:8765/` ([tools/models](../tools/models/README.md)) |
| `cargo xtask doctor` | Check this machine |
| `cargo xtask win -p <package>` | Under WSL, build for Windows and run on the real GPU |
| `cargo xtask seed` | Bring the shared seed build up to date with origin/main and, in a new worktree, link its compiled dependencies in; fast-forwards the main checkout when it is clean. `--refresh` runs the seed's builds again ([Worktrees](#worktrees)) |
| `cargo xtask worktrees` | What each worktree holds; `--prune` clears away finished ones; `--done`, in a worktree whose work is pushed, deletes its build ([Worktrees](#worktrees)) |

The tasks that make art, `props`, `tripo`, `bodies`, `blender`, `icons` and `paint-icons`, are under [Developer tools](#developer-tools).

A clean build of a Bevy application takes about two and a half minutes; a rebuild after a small change takes a few seconds.

## Worktrees

Agents each work in a git worktree of their own, and each worktree has its own `target/`. To spare a new one from building all of Bevy, `cargo xtask seed` puts compiled dependencies into it from **the shared seed**, leaving out our own crates, which it then builds in about a minute (D183). The libraries are hard links, so seeding costs about 0.2 GB of disk rather than 10 GB; what the worktree then builds of our own crates is its own. Why it links rather than sharing a cache is in [research/worktree-builds.md](../research/worktree-builds.md).

The shared seed is a build of origin/main kept for this alone, in the repository's git folder (`.git/terra-seed/`, its source in `src/`, its build in `target/`), so it never depends on anyone's checkout having built. It is built by the cargo commands agents run: the check's (clippy, the tests, the kit and artifact examples), the sandbox, and under WSL the sandbox for Windows in debug and release. `cargo xtask seed` fetches and, when origin/main's manifests (every `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml`) differ from those it was built from, checks origin/main out there and builds it again before linking. Only what changed is built; from nothing it took 17 minutes. It holds `.git/terra-seed/lock` while it does, so sessions starting together wait for one build instead of each making their own. `cargo xtask seed --refresh` runs its builds again; deleting `.git/terra-seed/` builds it from nothing. `cargo xtask doctor` says whether it is current. No cleanup ever touches it (D259).

It runs as routine. A Claude Code hook starts it in the background when a session or subagent starts in a worktree with no `target/`; a `cargo` command started meanwhile prints "Blocking waiting for file lock" and waits for it, seconds unless the seed is being built again. The same hook also runs it from the main checkout at most once every 15 minutes, logging to the main checkout's `target/seed.log`, so the seed follows origin/main before anyone needs it. Each run also **keeps the main checkout up to date**: it fast-forwards it to origin/main (`git merge --ff-only`) when it is on `main` with no changed files and no commits origin/main lacks, and otherwise leaves it as it is and says what is in the way. It never stashes, resets or rebases there. In a worktree entered partway through a session, run `cargo xtask seed` before the first build. It never touches a worktree that has built already.

Worktree cleanup separates build files from source files (D259). On session or subagent start, the same hook runs `cargo xtask worktrees --prune` in the background at most once every 15 minutes, logging to the main checkout's `target/worktrees.log`. It deletes a worktree's build after one idle hour. Source worktrees still wait two days: only a subagent's whose commits are all on `main` and that has no changes or untracked files is removed. Locked worktrees, running sessions and unreadable Git states are kept. Process inspection uses `lsof`; if it is unavailable, automatic cleanup keeps the worktrees. Cargo locks also protect running builds. Idle counts from the last edit to source files, commit, checkout or any build file. An agent need not wait: once its work is pushed, `cargo xtask worktrees --done` deletes its worktree's build at once, and `cargo xtask seed` makes a later build there quick again. `cargo xtask worktrees` shows every worktree, its state and size, and what a prune would do to it. Only worktrees under `.claude/worktrees/` are ever cleared; the shared seed, the main checkout and its build are not.

## Seeing and measuring

Sound is on in the sandbox except in scripted runs, which print what they would have played instead, and under WSL, which usually has no sound device: pass `--sound` to try it there, or `--mute` anywhere ([audio.md](audio.md)).

WSL has no hardware Vulkan, which splits the work in two:

- **To see output**, run the Linux build. It renders on the CPU, slowly but correctly, and can save screenshots. This works anywhere, including for agents.
- **To run on the real GPU**, build for Windows and run that build there. `cargo xtask play` and `cargo xtask bench` do this for you; for anything else:

```bash
cargo xtask win -p <package> -- <program arguments>
```

It cross-compiles the package, copies the program and its assets to the Windows temp folder, runs it there and passes the output back. Windows will not replace a program that is running, so `--copy <name>` runs a separate copy from its own folder: an agent tests with `--copy test` while a person plays the default copy, and neither blocks the other. The sandbox's scripts (`tools/sandbox`) name each copy for the checkout too, so runs from two worktrees never share one. It runs from the temp folder because a Windows program started from a WSL folder gets an unusable working directory, so anything the program writes, such as screenshots, lands there; the command prints the path. Add `--release` for an optimised build.

Keep debug builds on the workspace's default profile: full debug information produces a file Windows refuses to load.

### Measuring performance

Performance is judged one way, so numbers compare between changes, machines and days (D185):

```bash
cargo xtask bench                    # every scene, three runs each
cargo xtask bench forest --runs 5    # one scene
cargo xtask bench --record           # and add the results to research/benchmarks.csv
```

It builds the sandbox for release and runs each **scene**, a scripted run that comes out the same on every machine, at 17:00, whose long shadows cost the most:

| Scene | What it is |
|---|---|
| `yard` | The flat test level with nothing on it: what the engine and the player cost |
| `world` | The game where a new one starts: ground, sea, trees, grass, creatures |
| `forest` | The lush forest spike, standing still ([research/lush-forest-spike.md](../research/lush-forest-spike.md)) |
| `forest-walk` | The same, walking down a path: grass built as the player goes, trees changing detail |
| `village` | Fifty buildings on the yard, every kind, roof and plan (`--village`), seen from the village's corner: what building costs to draw ([building-plan.md](building-plan.md#checking-it)). It also says how long its sections took to lay and merge on the workers, and its operations to apply |
| `village-bare` | The yard from the same place with nothing built, which `village` is measured against |
| `going-up` | A stone house built on the yard a step every tenth of a second, as holding the hammer builds, the camera on it (`--going-up`): what building costs as it happens, and how long a change takes to reach the screen |
| `rising` | A settlement of some sixty buildings rising as a time-lapse at the town's place (`--timelapse`, `--site settlement`, D381), each built by the hammer's steps and its builders at their sites, from its bird's-eye drift: what building at a settlement's scale costs as it happens |
| `town300` | Building at scale: three hundred buildings of every kit, plan and roof on a grid of streets (`--town 300`, `terra_build::village::town`), walked down a street through its middle at a run (`--glide` between `--vantage`s). Says how many cubes, how long the town took to lay cold, how often sections streamed in as the camera moved, and what was drawn: triangles a frame by pass, meshes, and memory |
| `town300-rise`, `town300-fly`, `town300-bare` | The same town seen across from a rise at its corner, and flown over sixty metres up; and the view from the rise with nothing built, which the town is measured against |
| `castle`, `castle-field` | One base of a castle, some 140,000 cubes (`--castle`, `terra_build::castle`), walked through its gate, and seen from 270 m across a field |
| `far` | Three hundred buildings in hamlets out to 2 km (`--far 300`), looked round from sixty metres up |
| `walk` | 40 seconds' walk through the lush forest, out from the clearing and round through the trees, steered by `--route` as a player would; the world and the walker alone (`--experiment walk`), so it is the environment's cost |
| `lake`, `lake-walk` | The mountain lake (docs/demos.md §1) on the 30 km world, its place found by `--site lake`, in early June: from up its shore across the water to the woods, the tree line and the snowy peaks, still; and the shore walked for 30 seconds. The world and the walker alone |

Each run draws offscreen at 1920x1080 (`--offscreen`), with one frame in flight as a window has, with vsync off and no frame-rate limit, so it measures how long a frame really takes; skips 240 frames for shaders and streaming to settle; and times the scene's frames against the real clock (600, 2,400 for `walk`; `--bench-frames`). A scene's result is the middle run's median and 99th percentile and the worst frame of any.

**Nothing popping.** `cargo xtask bench` with every scene also runs `cargo xtask lodcheck` (D230), under a minute: a frame rate is only worth having if what it draws does not pop.

**The frame budget is 13.3 ms**: 75 frames a second, comfortably and always, at the 99th percentile, at 1080p on the developers' RTX 3080 at the high preset (D25). It is set once, `terra_world::debug::FRAME_BUDGET_MS`, and the readout, the log and the benchmark all judge by it; what a player sees on screen is their own choice of vsync or a frame-rate limit (D332).

**Holding to the budget.** Each scene must hold to it on the machine it runs on, or the task fails:

- its 99th percentile under the budget, and at most 1% of frames over it;
- no **hitch**, a frame over twice the budget (26.7 ms), a dropped frame a player sees;
- no **drift**: the game's own work in the last third of the run no more than 15% slower than in the first. Leaks, heat and a throttled machine show here; it is measured on the game's work, not the frame, so walking past more trees does not count.

**A quiet machine** (D185). A benchmark shares the machine, and others' builds, tests and runs make it slower and less even, so the task measures on a quiet one or says it could not. Before each run it waits, at most 90 s (five minutes over the whole task), for the processors under 35% busy and the GPU under 15% (under WSL as Windows counts them, which includes WSL's own machine), and samples the load while the run goes; a run is busy if it started busy or everything but the run itself was over 30% busy on average while it ran (or 70% at once). While the machine is quiet it makes up to as many runs again to stand in for busy ones; the result is the middle of the quiet runs when at least half are, and otherwise of all of them, marked as taken on a busy machine, an upper bound. Each scene's line gives the median of the middle run, the runs' medians' range, and a verdict, `holds` or `MISSED`, against D25's budget or, on an integrated GPU, D239's median, and says when runs disagree. Nobody needs to rerun it by hand: read the verdict.

The table also compares each median with the last one recorded for the same GPU, size and build, and marks it `SLOWER` past 10% or the runs' own spread, whichever is more: look into it before committing. Record a baseline when a change makes something faster or slower on purpose, and when a new machine first runs it. Scenes stay short, under a minute each, since a benchmark is something run often. Two agents benchmarking at once each pass `--copy <name>`, so neither overwrites the program the other is running; they still share the GPU. Benchmarks have the GPU to themselves (D319): a benchmark waits for `~/.cache/terra/gpu.lock` and says so; every other unattended run shares the lock and one of three slots (`TERRA_GPU_SLOTS`), so up to three shots and films run side by side, the rest wait, and none runs during a benchmark; take the turn for a measurement of your own with `flock ~/.cache/terra/gpu.lock <command>`; a benchmark is taken to be stuck after five minutes counted from when it has the GPU, not from when it started waiting.

For each run the sandbox also prints the medians of the game's work, the render passes on the CPU and the GPU, and of each render pass on the GPU, costliest first, which the task shows beside the frames (a pass counts in the frames it ran in, so one that ran once, as a light probe's filter does after the sky is painted, is not counted in every frame); how long the render world worked on each frame before it waited for the GPU, and how long it waited, so a frame that waits long is held up by the GPU and one that waits little by the CPU (the sun's shadow maps are missing from the passes' times, which Bevy gives as 0.03 ms where drawing them costs some 2.4 ms in the forest, but they show in the wait, [research/2026-10-15-75fps-budget.md](../research/2026-10-15-75fps-budget.md)); its frames and the game's own work by tenths of the run, so a climb or a jump shows where it starts, and what the world held at the first and last timed frame (entities, meshes, images), so something that keeps growing shows. On macOS a scripted run tells the system it is latency-critical: a hidden window, as a scripted run's is, is put into App Nap after about 40 s, which made every frame three times as long from then on.

Read the 99th percentile and worst frame as well as the median: a stutter hides in a good median. Each run also prints `SANDBOX slowest frames`, which frames were slowest, counted as `--press` counts them; slow frames at a steady interval point at something periodic. Benchmarks stream the ground as play does, so they include its cost; screenshot runs wait for every chunk instead, so they see the same ground every time.

Under WSL the task builds and runs the Windows program, the only one with a real GPU. On macOS `--offscreen` is what makes numbers trustworthy: the window server draws nothing of a window that is covered, or when the screen is asleep or locked, and a Retina screen draws a 1600x900 window at 3200x1800. Report the integrated GPU too (D25) when a machine has one. Software rendering's numbers mean nothing.

**Quality presets.** Each run draws at the preset chosen for its GPU, an integrated one low and any other high (D239); `--quality low|medium|high` asks for another. The integrated GPU is measured at the low preset, whose target is 30 fps (a median under 33.3 ms, D239), which the task judges it by, knowing it from the run's own line. To measure it, under WSL: `WGPU_POWER_PREF=low WSLENV=WGPU_POWER_PREF cargo xtask bench`.

To find what a scene spends its time on, switch things off and run it again. A temporary environment variable read by the code under test is enough, removed before committing.

## Judging a material

A change to a construction material, its scan, pack, tint, pattern, shader or the light it is seen in, is judged on the material bench (D386), not on ad hoc stills:

```bash
cargo xtask look stone            # one material, about two minutes on the GPU
cargo xtask look logs slate       # several
cargo xtask look --all            # the building baseline's (D333)
```

The kit sheet lays the material's preview alone and two scripted runs shoot fixed vantages of it: at noon its wall (a roof's covering) square on from 1, 2 and 5 m, a corner, an opening, its roof and the whole from 12 m; the same wall in the shade, and again in the `light` view; and in the evening its sunlit face again. An 18% grey card stands beside each face measured, in the same light and in the 2 m shot's frame (`sandbox.card`): the eye adapts to each view, so a card shot in a view of its own was up to a stop and a quarter off the wall it was held against. It prints, against its pattern's targets (`look` in `assets/data/kits/patterns/*.ron`), and fails on any it misses:

- **texel density**: its surface's colour texels a metre at the high preset;
- **sharpness**: the finest octave's share of contrast in the middle of the 1, 2 and 5 m shots (a sharp scan a quarter or more; TAA's blur and a stretched or upscaled texture take it down);
- **grey-card balance**: the card's red over blue in linear light (held near 1 at noon, D370; blue in shade and amber in the evening are the light's), and the surface's brightness against the card, lit and in shade (a wall at 0.2 of the card in shade reads near black);
- **the sky's light**: in shade in the `light` view, every surface an 18% grey lit as it is, the surface's red over blue against the card's, held to 0.75 to 1.2 for every material (a surface that took a neutral light where the card takes the blue sky would read 2 or more), and its brightness against the card's, what its occlusion leaves it;
- **value range**: its 5th to 95th percentile of brightness at 2 m at noon;
- **stretch**: each texture set's share of area drawn past 1.5:1 and 2.5:1 (`cargo xtask texels`).

`shots/look/<material>.jpg` (under 2 MB) puts every shot beside the pack's reference photograph (`art/references/`), the 1 and 2 m shots as 1:1 crops, with the table under them; `shots/look/<material>.txt` is the table. A change that makes a material better on purpose may raise its pattern's targets; one that lowers a target says why in its commit.

## Logs and the frame-time readout

Every build shows frame times in the top-left corner, all the time (D77). The line reads: how long a typical frame's work takes against the [frame budget](#measuring-performance), its 99th percentile (`p99`) and the worst over the last two seconds; then the game's CPU time, the render passes' CPU time, GPU time, and frames a second. Work is the longest of the three, since they run side by side. The line's colour is its `p99`: green under three quarters of the budget (10 ms), amber up to it, red over, so a good median with red means hitches. Under it, a graph of work over the last 240 frames, a bar a frame, newest on the right, each bar coloured the same way: the white line is the budget and the top twice it, a stutter. A flat row under the line is smooth; tall bars at a steady rhythm point at something periodic, and bars when you move or turn at streaming. F3 adds a graph each for game, render and GPU, so a spike shows which part caused it, and the GPU, resolution and build type, entities, memory, and the costliest render passes. GPU times need timestamp queries (Vulkan and DirectX 12); on Metal they show as `-`. A debug build is several times slower on the CPU than a release build, so judge budgets on `--release`.

The log is for what is worth reading: the engine's routine messages are filtered out (`RUST_LOG` overrides the filter), and once a minute it records frame times as median, 99th percentile and worst, with a warning whenever frames take over twice the budget. Software rendering skips the warning, since its times mean nothing.

## Building's debug views

F4 cycles what every building's surfaces are coloured by, and Shift+F4 goes back (D385, `crates/terra_game/src/build/views.rs`), as Unreal's view modes and Unity's draw modes do; a legend in the top right says what the view shows and how to read it. Look before measuring: most questions of how a building looks or where it is wrong are answered by one of these.

| View | Shows |
|---|---|
| `texels` | Screen pixels a texel covers seen from a metre off, against a texel a pixel (D362): a texture's density wherever the camera is, stretched mapping as blue; hatched where it is blurred as seen now |
| `roles` | Each cube's edges by their role (arris, jamb, crest, foot, sill and the rest), roof lines dashed by their part (eave, verge, ridge, hip, valley, flashing), faces tinted by what is beyond them; within 40 m |
| `detail` | Each mesh by the detail it draws (near, middle, far, hull, cell), each section's or group's box by the detail it is laid for, and the base being built's parts |
| `damage` | Each cube's health lost, where the damage drawing breaks a surface, and damaged cubes boxed |
| `shade` | How much light a surface keeps after texture occlusion, SSAO and its corners' baked shade, tinted by what takes the most |
| `normals` | The normal with its normal map in world axes, and each texture's tangent frame as ticks |
| `plan` | What is built, a plan's ghosts and what goes in next, in three colours |
| `light` | The light alone, as Unreal's Lighting Only: every surface an 18% grey lit as it is, its normal map and occlusion kept, its colours (and so its corners' baked shade) left out, to read against a grey card beside it (`sandbox.card`): in shade as blue as the card. A warm material in the blue shade reads near neutral; this view says whether that is its colour or its light |

Over the remote, `node tools/sandbox/remote.ts debug.view <name>` sets one (`off`, `next` and `back` too) and answers with its legend, so a view is shot headlessly (`shot` after it). Off, the views cost nothing: their shader code is compiled in only while one is on, and their systems do not run; so the first time one is on its shaders compile in the background, a few seconds in which buildings are not drawn, and a scripted run turns a view on early to compile it (as the material bench does). Every preset draws them.

## Debug menu

<a id="debug-menu"></a>Wanted (D81): an in-game menu, present only when the game is started with a flag, opened with the backtick key. It does what developers need while playing: warp to a place or planet, give items, change the time of day or the weather, show colliders and chunk borders, switch systems off to see what they cost. Each entry should be a command that QA for agents (D69) can send too, so the menu and the agents' tools are the same list. Not built.

## Developer tools

The developers want tools that help refine each system (D71), and as many of them as possible reachable from a browser by a link (research/web-build.md). Tools that look at the game's own world are the sandbox's experiments, each a small plugin switched on from the command line ([sandbox README](../crates/terra_sandbox/README.md)); tools that make or inspect files live in `tools/`. Every one starts from `cargo xtask`:

| Tool | Start it with | What it is for |
|---|---|---|
| [Model gallery](../tools/models/README.md) | `cargo xtask contract` | Every model in a browser, drawn the same way beside the 1.8 m figure, with its size, triangles and problems; a page per model with materials, animations, bounds, skeleton, wireframe. Every view is a link. Shape, size and orientation, not the game's exact look |
| [Model workbench](../crates/terra_sandbox/README.md#the-workbench) | `cargo xtask workbench --model … --prop …` | Models in the game's own renderer: bodies animated, holding and wearing things (`--hold`, `--wear`), catalogue items as placed in play with their sizes (`--prop Forge`, `--prop all`), contact sheets for agents (`--sheet`) |
| [Sandbox remote](../tools/sandbox/README.md) | `cargo xtask remote start …` | Drive a running sandbox live: move things, hold the camera, pose, give, warp, screenshot, without restarting |
| [Prop maker](../tools/props/README.md) | `cargo xtask props concept\|model\|refine <prop>` | A prop from a description to the game: concept, Tripo model, clean-up into our palette, levelled and facing front; `props fronts` shows every model from the front on one sheet |
| [Kit pieces](../tools/kits/README.md) | `cargo xtask blender tools/kits/pieces.py <source> <out>` | A building kit's dressed pieces, the profiles swept along each edge by its role and the corner stones up it, made in Blender from their source under `art/kits/` (D215) |
| [Tripo CLI](../tools/tripo/README.md) | `cargo xtask tripo <command>` | Everything Tripo's Studio does, on our subscription's credits (below) |
| [Character pipeline](../tools/characters/README.md) | `cargo xtask bodies` | Bodies, morphs, skins, worn and held looks, from `art/`; the `character` skill has the steps |
| [Blender runner](../tools/blender/README.md) | `cargo xtask blender <script.py>` | Any Blender script, headless, on the Windows Blender from WSL (below) |
| [Icon painter](../tools/icons/README.md) | `cargo xtask icons`, `cargo xtask paint-icons <item>` | Item icons: baked from each item's look, then painted in the art style |
| [Docs check](../tools/docs/README.md) | `cargo xtask check` | Links and anchors resolve, every doc, note, spike and script is in its index, every decision file is well formed and the decisions index is their render |
| [Token stats](../tools/token-stats/README.md) | a Claude Code hook | Claude tokens and Tripo credits per developer, on a public page |

A new tool gets a line in `TASKS` in `xtask/src/main.rs` and a row here, so it is found the same way as the rest.

Checks that run with `cargo xtask check` keep models honest without anyone looking: every catalogue item states how tall a real one stands, and a test holds its model to that (`play::props`); icons must be made from their item's current look.

The workbench grew from this wish list, kept for what is not built yet:

- **A model workbench**: load any model against a simple patch of terrain, scale it, recolour it or put textures on it, look at it from set angles and in the game's light, and save the result. It is also where models are tested and reviewed:
  - **Checks an agent runs**: triangle count, size against a reference figure, parts that float, every material named and in the palette, and a sheet of renders from set angles beside models already accepted. Built: the gallery and the prop maker's checks give all but the palette check.
  - **Review by a person**: step through candidates in the game's light, accept or reject each, and leave a note on it, or on a spot on it, that the agent reads on its next pass. Not built.

Each tool should be scriptable as well as usable by hand, so an agent can use it too.

### Blender

For making and cleaning up models ([research/ai-models-for-art.md](../research/ai-models-for-art.md)). Install the Blender version in [tools/blender/version](../tools/blender/version) (5.2 LTS); `run.sh` refuses any other, and `cargo xtask doctor` says what you have; under WSL, the Windows install is the one to use, since it renders on the real GPU.

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
- **It costs credits**, listed command by command in [its README](../tools/tripo/README.md); images and preset animations are free. `balance` says what is left. Credits count in the token stats beside Claude's tokens ([tools/token-stats](../tools/token-stats/README.md)), read from Tripo's own history, so work done by hand in the web app counts too.
- **Use the CLI, not the web app's API by hand.** When Tripo changes its requests and a command breaks, fix the CLI; [research/tripo-studio-api.md](../research/tripo-studio-api.md) says how to capture what the web app sends now.

## Checking the game without a person (QA for agents)

The developers want the game's QA reachable by our coding agents (D69): an agent should be able to run the game, drive it, look at it and ask it what is true, the way a tester would.

What exists: scripted runs of the sandbox. Every run starts on the loading screen (D135), and a script's frames are counted from when it goes, so a script does the same thing however long loading took. `--press` holds keys on given frames, `--shot` saves a screenshot, `--shots` a sequence of them from one run (so flicker shows between consecutive frames), `--vantage` fixes the camera, `--bench` measures frame time, and `--offscreen` draws into a picture rather than a window, so all of it works with the screen locked and without covering anyone's work. In the play experiment, `--at`, `--face`, `--warp` and `--give` set a scene up, and every on-screen notice is logged, which is how features were checked until the live commands below.

Live, in one running game (D69): any sandbox run started with `--remote` answers Bevy's remote protocol, and [tools/sandbox](../tools/sandbox/README.md) drives it from the command line. Besides moving things and taking screenshots, a script can press keys, point and click in menus (`point`, `click`), stop and step the clock exactly, read everything the run has said and wait for words to be said, and in the play experiment read a digest of the player's state (`state`) and set a scene up (`warp`, `give`). A check acts, then waits for what the game says or reads what is true, rather than counting frames or reading the screen. When a check needs a fact the game does not yet answer, add it to the game's methods; a command line over them is the interface, and an MCP server would only be a thin layer over the same methods if some client needed one.

Built on it:

- **The loop, played live**: `cargo xtask loop` plays the MVP's loop in one running game and checks each step, restarting once to load what it saved ([tools/sandbox](../tools/sandbox/README.md#scripts)).
- **Same start, same game**: with `--trace FILE.jsonl` the play experiment writes its whole state every frame, and `tools/sandbox/diverge.ts --twice -- <a scripted run>` runs it twice and names the first frame and the values where the runs part. Two runs of the hunt agree on every frame.
- **Shared code stays repeatable**: `cargo xtask check` refuses a shared-state crate that reads the clock or the system's randomness; time and seeds are passed in.
- **Property tests over action sequences** in `terra_items`: random moves, and random stowing, paying and wearing, over many seeds, checked against a simple tally after every step. A failure names its seed and step.

Off in release builds, since it lets anything on the machine control the game.

Playing the game as an agent, inside its rules rather than as a tester, is a separate want: see [multiplayer.md](multiplayer.md#agents-as-players) (D73).

## Writing tests

A test earns its place by failing when the behaviour breaks, and only then. Before keeping one, name a plausible bug in the code it covers and check that the test catches it; for a new test, make that bug for a moment and watch it fail.

- **Test the outcome, through the code that makes it.** A system is tested by running it on a small `World` (`run_system_once`) and reading what changed, not by calling the helper it uses and trusting the glue. A save is tested by saving a world and loading it into another, not by round-tripping a struct through serde.
- **Know the answer another way.** Never work out the expected value with the function under test, or with the same formula, predicate or constant copied out of it. Use a value worked out by hand, a law the result must obey (volume of a sphere, nothing made or lost, undo gives back the same state), or a second, simpler implementation.
- **Logic on small made-up data, real data checked for its shape.** A rule is tested on a fixture written in the test. The shipped files under `assets/data/` get integrity checks: everything they name exists, values are in range, every kind has what it needs. Neither pins tuned values (a pack is 6x5, the Hammer is in slot 0, a pine has 46 triangles), so retuning never breaks a test. Write a number relative to the constant or data it follows from.
- **Same on every machine is a pinned value.** Running something twice in one process and comparing proves nothing. Determinism that matters (D30, D41) is checked against values written into the test, as `terra_gen` does.
- **Inputs that reach the bug.** Unsorted input to what sorts, a turned frame for what rotates, two entities for what staggers, a cost naming a kind twice. A check that holds on empty output also asserts the output is not empty, and a random test asserts its operations were accepted often enough to mean something.
- **Quick by default.** The quick check before every commit runs every test but a few slow sweeps; keep a test to a few seconds. A sweep over every case that cannot be made quicker goes in `SLOW_TESTS` in `xtask/src/check.rs`, with a comment above the test naming it there, so it runs before each push instead (D37).
- **Not worth a test:** that a getter returns what was set, that a constant equals its literal, that serde or `format!` works, that something does not panic, or what another test already covers.

## Conventions

- The rules for working in the workspace are in [AGENTS.md](../AGENTS.md). They apply to people as well as agents.
- How crates are split, and how to add one, is in [crates/README.md](../crates/README.md).
- Dependencies are pinned through `Cargo.lock`, which is committed.
- Bevy is upgraded deliberately, once the crates we depend on support the new release (D31).

## Continuous integration

Off until we need it (D37): it runs only when started by hand from GitHub's Actions tab. It runs `cargo xtask check` on Linux, macOS and Windows, and a compile check for the Windows target we cross-compile from WSL. Nothing in CI uses a GPU.

To run the tests on that cross-compiled Windows build yourself, from WSL: `cargo test --workspace --target x86_64-pc-windows-gnu`.
