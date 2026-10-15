//! Project tasks, and the one place every tool is started from. Run as `cargo xtask <task>`;
//! `cargo xtask tools` lists them all, from `TASKS` below. A new tool gets a line there.

mod seed;
mod worktrees;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// A task: its name, what it takes, what it does, the group it is listed under, and how
/// it runs, given the arguments after its name.
struct Task {
    name: &'static str,
    takes: &'static str,
    does: &'static str,
    group: &'static str,
    run: fn(&[String]) -> bool,
}

/// Every task, in the order `tools` lists them. The TypeScript and Blender tools are started
/// here too, so nobody has to remember where each one lives; each has its own README.
const TASKS: &[Task] = &[
    Task {
        name: "play",
        takes: "[--fresh] [game args]",
        does: "play the tech demo on the real GPU, wherever you are; --fresh starts a new game",
        group: "Play and check",
        run: play,
    },
    Task {
        name: "yard",
        takes: "[game args]",
        does: "play the yard, a flat test level for building: nothing on it, building free and instant, nothing saved",
        group: "Play and check",
        run: yard,
    },
    Task {
        name: "loop",
        takes: "",
        does: "play the demo's loop by script and check every step happened (D69)",
        group: "Play and check",
        run: |_| run("node", &["tools/sandbox/loop.ts"]),
    },
    Task {
        name: "cabin",
        takes: "[--here] [--until STEP]",
        does: "build a cabin from plan to door by script, fit a door and shutters, open and shut them, walk in, and check every step by state (D69, D139, D201)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/cabin.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "demo",
        takes: "[--skip-cabin] [--hour 15] [--out shots/demo]",
        does: "the construction demo's pictures: the cabin built by script, then the village and the cabin from fixed views (tools/sandbox)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/demo.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "vantage",
        takes: "[--time 13,17]",
        does: "the vantage sheet: the lush forest from fixed views, side by side, to judge how the world looks (tools/sandbox)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/vantage.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "collide",
        takes: "[--count 20]",
        does: "walk the player into the lush forest's rocks, boulders, stumps, logs and trees, and check against their meshes that each stops them where it is drawn, or is stepped over if low (tools/sandbox)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/collide.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "lodcheck",
        takes: "[--views floor,path,clearing,village,town] [--time 17] [--limit 0.5]",
        does: "find what pops as detail changes: still views while every switch distance is swept; fails past the limit (tools/sandbox)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/lodcheck.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "pops",
        takes: "[--frames 1200-1320] [-- sandbox args]",
        does: "find what pops in or out, or flickers, in a stretch of a scripted walk, frame by frame (tools/sandbox)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/pops.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "film",
        takes: "[--frames 300-2700] [--time 15] [--out shots/film.mp4] [-- sandbox args]",
        does: "a video of a scripted run, by default the forest walk, at the game's own speed (tools/sandbox)",
        group: "Look at things",
        run: |args| {
            let mut all = vec!["tools/sandbox/film.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "qa",
        takes: "<scenario> [--set <kit>|all] [--part <stage>] [--no-trace] | --list | --index",
        does: "a QA film of building in a set: the same scripted film every time, a contact sheet and a real-time frame trace, in shots/qa/ (tools/sandbox)",
        group: "Look at things",
        run: |args| {
            let mut all = vec!["tools/sandbox/qa.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "bench",
        takes: "[scene…] [--runs N] [--record] [--debug] [--copy NAME]",
        does: "time the benchmark scenes: release build, offscreen at 1920x1080, against the last recorded; with every scene, also lodcheck",
        group: "Play and check",
        run: bench,
    },
    Task {
        name: "check",
        takes: "",
        does: "what every change must pass: formatting, lints, tests, the dependency rules",
        group: "Play and check",
        run: |_| check(),
    },
    Task {
        name: "kits",
        takes: "",
        does: "check every building kit file: the materials it needs, sizes and the style guide's budgets",
        group: "Play and check",
        run: |_| kits(),
    },
    Task {
        name: "contract",
        takes: "",
        does: "check every model the game places against the model contract: size, anchor, axes, sockets and, for doors and shutters, fit in their hole",
        group: "Play and check",
        run: |_| contract(),
    },
    Task {
        name: "rules",
        takes: "[--pure] [--here] [--debug]",
        does: "audit building: its rules by test and live, the village's drawing, edits' and frames' timings (research/building-audit.md)",
        group: "Play and check",
        run: rules,
    },
    Task {
        name: "sections",
        takes: "[building]",
        does: "time laying and merging the village's sections, as the game's workers do, against the 2 ms bar, and the block kinds' near triangles and MiB against their budget (D284)",
        group: "Play and check",
        run: sections,
    },
    Task {
        name: "artifacts",
        takes: "[building] [--all] [--quick] [--ratchet|--record] | --frames [--quality high,low] [--checks still,sun,glide,going-up]",
        does: "find what would draw as junk in building's meshes: z-fighting, sky through walls, gaps under roofs, pieces poking out; --frames, what only frames show, on the GPU: shimmer, acne, flashes during an edit, TAA trails, light probes popping",
        group: "Play and check",
        run: artifacts,
    },
    Task {
        name: "sandbox",
        takes: "[args]",
        does: "the sandbox, the test environment; --list for its experiments (crates/terra_sandbox)",
        group: "Look at things",
        run: |args| cargo_run("terra_sandbox", &[], args),
    },
    Task {
        name: "workbench",
        takes: "[args]",
        does: "models in the game's renderer: --model, --prop Forge|all, --hold, --sheet",
        group: "Look at things",
        run: |args| cargo_run("terra_sandbox", &["--experiment", "workbench"], args),
    },
    Task {
        name: "remote",
        takes: "<command> [args]",
        does: "drive a running sandbox: start, warp, give, look, shot… (tools/sandbox)",
        group: "Look at things",
        run: |args| node("tools/sandbox/remote.ts", args),
    },
    Task {
        name: "models",
        takes: "[--port 8765]",
        does: "the model gallery in a browser: every model, its size, problems and a page each (tools/models)",
        group: "Look at things",
        run: |args| node("tools/models/serve.ts", args),
    },
    Task {
        name: "props",
        takes: "<step> [prop…]",
        does: "make props: concept, model, refine, all; fronts to see every model from the front (tools/props)",
        group: "Make art",
        run: |args| node("tools/props/props.ts", args),
    },
    Task {
        name: "textures",
        takes: "<step> [args]",
        does: "make materials: fetch from Poly Haven, material from an image, stack for the ground (tools/textures)",
        group: "Make art",
        run: |args| node("tools/textures/textures.ts", args),
    },
    Task {
        name: "pack",
        takes: "<name> --from <source> [--pattern block|frame|coursed|roof]",
        does: "make a construction material in one command: its pack from a CC0 scan, an image or a reference model, checked and credited, and its file (tools/kits, D290)",
        group: "Make art",
        run: |args| node("tools/kits/pack.ts", args),
    },
    Task {
        name: "tripo",
        takes: "<command> [args]",
        does: "Tripo on our Studio subscription: image, model, rig, animate, balance… (tools/tripo)",
        group: "Make art",
        run: |args| node("tools/tripo/tripo.ts", args),
    },
    Task {
        name: "bodies",
        takes: "",
        does: "rebuild every character asset the game loads from art/ (tools/characters)",
        group: "Make art",
        run: |_| run("tools/characters/build_bodies.sh", &[]),
    },
    Task {
        name: "blender",
        takes: "<script.py> [args]",
        does: "run a script in Blender, headless (tools/blender)",
        group: "Make art",
        run: |args| run_owned("tools/blender/run.sh", args),
    },
    Task {
        name: "icons",
        takes: "[args]",
        does: "bake every item's icon from its look into assets/icons (D129)",
        group: "Make art",
        run: icons,
    },
    Task {
        name: "paint-icons",
        takes: "<item…> | --unpainted",
        does: "paint item icons in the art style (tools/icons)",
        group: "Make art",
        run: |args| node("tools/icons/paint.ts", args),
    },
    Task {
        name: "doctor",
        takes: "",
        does: "what this machine is missing",
        group: "This machine",
        run: |_| doctor(),
    },
    Task {
        name: "win",
        takes: "-p <package> [--release] [--copy <name>] [-- args]",
        does: "under WSL, build for Windows and run it there, on the real GPU",
        group: "This machine",
        run: win,
    },
    Task {
        name: "seed",
        takes: "",
        does: "in a new worktree, copy the main checkout's compiled dependencies in, so the first build is quick",
        group: "This machine",
        run: seed::seed,
    },
    Task {
        name: "worktrees",
        takes: "[--prune | --done]",
        does: "what each worktree holds; --prune clears away the ones agents are done with (a hook does it hourly); --done, once your work is pushed, deletes this worktree's build",
        group: "This machine",
        run: worktrees::worktrees,
    },
    Task {
        name: "tools",
        takes: "",
        does: "this list",
        group: "This machine",
        run: |_| {
            println!("{}", usage());
            true
        },
    },
];

/// A scene performance is judged in: a scripted run of the sandbox that comes out the same
/// on every machine. docs/development.md says how they are used.
struct Scene {
    name: &'static str,
    about: &'static str,
    args: &'static [&'static str],
    /// How many frames are timed.
    frames: &'static str,
}

/// Where the village scenes look from and at, east, up and south of the start.
const VILLAGE_VIEW: &str = "-85,16,10,-5,0,-50";

/// The scale scenes' world, as large as D13 leans toward, so the ground curves away as it
/// will and a view from a rise reaches as far as it will.
const SCALE_RADIUS: &str = "30000";

/// The scenes, at the hour whose long shadows cost the most.
const SCENES: &[Scene] = &[
    Scene {
        name: "yard",
        about: "the flat test level with nothing on it: what the engine and the player cost",
        args: &["--experiment", "play", "--level", "yard", "--time", "17"],
        frames: "600",
    },
    Scene {
        name: "world",
        about: "the game where a new one starts: ground, sea, trees, grass, creatures",
        args: &["--experiment", "play", "--time", "17"],
        frames: "600",
    },
    Scene {
        name: "forest",
        about: "the lush forest spike: thousands of trees of leaf cards, grass, bushes",
        args: &[
            "--experiment",
            "play",
            "--experiment",
            "lush",
            "--level",
            "yard",
            "--time",
            "17",
        ],
        frames: "600",
    },
    Scene {
        name: "forest-walk",
        about: "the same, walking down a path: grass built as the player goes, trees changing detail",
        args: &[
            "--experiment",
            "play",
            "--experiment",
            "lush",
            "--level",
            "yard",
            "--time",
            "17",
            "--at",
            "-5,-3.4",
            "--face",
            "-191,-130",
            "--press",
            "w@200-2000",
        ],
        frames: "600",
    },
    Scene {
        name: "village",
        about: "fifty buildings of every kind, roof and plan on the yard, seen from its corner (docs/building-plan.md)",
        args: &[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--time",
            "17",
            "--village",
            "--vantage",
            VILLAGE_VIEW,
        ],
        frames: "600",
    },
    Scene {
        name: "village-bare",
        about: "the yard from the village's corner with nothing built: what the village is measured against",
        args: &[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--time",
            "17",
            "--vantage",
            VILLAGE_VIEW,
        ],
        frames: "600",
    },
    Scene {
        name: "going-up",
        about: "a stone house built on the yard a step every tenth of a second, as the hammer builds, the camera on it: what building costs as it happens",
        args: &[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--time",
            "17",
            "--going-up",
        ],
        frames: "1800",
    },
    Scene {
        name: "town300",
        about: "three hundred buildings of every kit, plan and roof on a grid of streets, walked down a street through its middle at a run (docs/building-plan.md)",
        args: &[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--time",
            "17",
            "--radius",
            SCALE_RADIUS,
            "--town",
            "300",
            "--glide",
            "--vantage",
            "-130,1.7,-152,-110,2.5,-152",
            "--vantage",
            "-130,1.7,-152,-110,2.5,-152@240",
            "--vantage",
            "-10,1.7,-152,10,2.5,-152@1440",
        ],
        frames: "1200",
    },
    Scene {
        name: "town300-rise",
        about: "the same town seen across from a rise at its corner",
        args: &[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--time",
            "17",
            "--radius",
            SCALE_RADIUS,
            "--town",
            "300",
            "--vantage",
            "-170,30,-15,0,0,-170",
        ],
        frames: "600",
    },
    Scene {
        name: "town300-bare",
        about: "the same view from the rise with nothing built: what the town is measured against",
        args: &[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--time",
            "17",
            "--radius",
            SCALE_RADIUS,
            "--vantage",
            "-170,30,-15,0,0,-170",
        ],
        frames: "600",
    },
    Scene {
        name: "town300-fly",
        about: "the same town flown over sixty metres up, from one corner to the other",
        args: &[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--time",
            "17",
            "--radius",
            SCALE_RADIUS,
            "--town",
            "300",
            "--glide",
            "--vantage",
            "-200,60,-20,-130,0,-90",
            "--vantage",
            "-200,60,-20,-130,0,-90@240",
            "--vantage",
            "150,60,-320,220,0,-390@840",
        ],
        frames: "600",
    },
    Scene {
        name: "castle",
        about: "one base of a castle, some 140,000 cubes of thick stone walls, towers and roofs, walked through its gate toward its keep",
        args: &[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--time",
            "17",
            "--radius",
            SCALE_RADIUS,
            "--castle",
            "--glide",
            "--vantage",
            "0,1.7,-35,0,8,-140",
            "--vantage",
            "0,1.7,-35,0,8,-140@240",
            "--vantage",
            "0,1.7,-85,0,8,-140@840",
        ],
        frames: "600",
    },
    Scene {
        name: "castle-field",
        about: "the castle seen from across a field, 270 m off",
        args: &[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--time",
            "17",
            "--radius",
            SCALE_RADIUS,
            "--castle",
            "--vantage",
            "120,3,140,0,10,-100",
        ],
        frames: "600",
    },
    Scene {
        name: "far",
        about: "three hundred buildings in hamlets spread out to 2 km, looked round from sixty metres up",
        args: &[
            "--experiment",
            "play",
            "--level",
            "yard",
            "--time",
            "17",
            "--radius",
            SCALE_RADIUS,
            "--far",
            "300",
            "--glide",
            "--vantage",
            "0,60,0,1500,0,0",
            "--vantage",
            "0,60,0,1500,0,0@240",
            "--vantage",
            "0,60,0,0,0,-1500@540",
            "--vantage",
            "0,60,0,-1500,0,0@840",
        ],
        frames: "600",
    },
    Scene {
        name: "walk",
        about: "40 seconds' walk through the lush forest and its grass, out from the clearing and round, turning as a player would, looking up into the canopy once; the world and the walker alone",
        args: &[
            "--experiment",
            "walk",
            "--experiment",
            "lush",
            "--level",
            "yard",
            "--time",
            "17",
            "--route",
            "-58,-39",
            "--route",
            "-40,-69",
            "--route",
            "-6,-80",
            "--route",
            "30,-74",
            "--route",
            "59,-54",
            "--route",
            "77,-22",
            "--route",
            "79,14",
            // Halfway, a look up into the canopy and back down, the heaviest view (D185).
            "--press",
            "up@1500-1545",
            "--press",
            "down@1650-1695",
        ],
        frames: "2400",
    },
];

/// Frames run before any are timed, so shaders are ready and the scene has settled.
const BENCH_SETTLE: &str = "240";
/// A run that has not finished in this long is stuck, and is ended.
const BENCH_TIMEOUT_S: u64 = 300;
/// How much slower than the last recorded result counts as a regression to look into.
const SLOWER_BY: f32 = 0.10;
/// Where `--record` keeps results, one line per scene per run of the task.
const BENCHMARKS: &str = "research/benchmarks.csv";
const BENCHMARKS_HEADER: &str =
    "date,commit,gpu,size,build,scene,median_ms,p99_ms,worst_ms,runs,over_pct,hitches,drift_pct";
/// The frame budget (D25), and what a scene must hold to on the machine it runs on: at most
/// this share of frames over budget, no hitch (a frame over twice it), and its last third
/// no slower than its first by more than this.
const BUDGET_MS: f32 = 1000.0 / 60.0;
const MOST_OVER_PCT: f32 = 1.0;
const MOST_DRIFT_PCT: f32 = 15.0;

/// One timed run, as the sandbox prints it.
#[derive(Debug, Clone, PartialEq)]
struct Timing {
    gpu: String,
    size: String,
    median_ms: f32,
    p99_ms: f32,
    worst_ms: f32,
    /// How the run held to the budget, from its next line.
    over_pct: f32,
    hitches: u32,
    drift_pct: f32,
}

impl Timing {
    /// What the scene missed of the budget's gates, if anything.
    fn misses(&self) -> Vec<String> {
        let mut missed = Vec::new();
        if self.p99_ms > BUDGET_MS {
            missed.push(format!("99th percentile over {BUDGET_MS:.1} ms"));
        }
        if self.over_pct > MOST_OVER_PCT {
            missed.push(format!("{:.1}% of frames over budget", self.over_pct));
        }
        if self.hitches > 0 {
            missed.push(format!("{} hitches", self.hitches));
        }
        if self.drift_pct > MOST_DRIFT_PCT {
            missed.push(format!("{:+.0}% slower by the end", self.drift_pct));
        }
        missed
    }
}

/// Reads `SANDBOX against the 16.7 ms budget: 0.50% of frames over, 2 hitches over
/// 33.3 ms, the last third +3.0% on the first`.
fn parse_sustained(line: &str) -> Option<(f32, u32, f32)> {
    let rest = line.strip_prefix("SANDBOX against the ")?;
    let (_, rest) = rest.split_once(" budget: ")?;
    let (over, rest) = rest.split_once("% of frames over, ")?;
    let (hitches, rest) = rest.split_once(" hitches over ")?;
    let (_, drift) = rest.split_once("in the last third ")?;
    let drift = drift.strip_suffix("% on the first")?;
    Some((
        over.parse().ok()?,
        hitches.parse().ok()?,
        drift.parse().ok()?,
    ))
}

/// Reads `SANDBOX frames on <gpu> at <size>: median 1.00 ms, 99th percentile 2.00 ms,
/// worst 3.00 ms` (crates/terra_world/src/shell.rs).
/// What a scene's run says about itself that a benchmark reports beside its frames: how
/// long building's sections took to lay on the workers, and its operations to apply; and
/// where its frames went: the game, the render passes on the CPU and the GPU, and the GPU
/// by pass.
fn worth_saying(line: &str) -> Option<&str> {
    [
        "Builds drawn: ",
        "Builds changed: ",
        "Builds streamed in: ",
        "Village: ",
        "Going up: ",
        "SANDBOX slowest frames",
        "Scale: ",
        "Builds hold: ",
        "Builds hold by detail: ",
        "Builds drawn in view: ",
        "Builds' texture arrays: ",
        "frame parts, ",
        "GPU passes, ",
        "triangles a frame, ",
        "drawn, medians: ",
    ]
    .iter()
    .find_map(|key| line.find(key))
    .map(|at| line[at..].trim_end())
}

fn parse_timing(line: &str) -> Option<Timing> {
    let rest = line.strip_prefix("SANDBOX frames on ")?;
    let (who, numbers) = rest.split_once(": ")?;
    let (gpu, size) = who.rsplit_once(" at ")?;
    let ms: Vec<f32> = numbers
        .split(", ")
        .map(|part| {
            part.trim_end_matches(" ms")
                .rsplit(' ')
                .next()?
                .parse()
                .ok()
        })
        .collect::<Option<_>>()?;
    let [median_ms, p99_ms, worst_ms] = ms[..] else {
        return None;
    };
    Some(Timing {
        gpu: gpu.to_string(),
        size: size.to_string(),
        median_ms,
        p99_ms,
        worst_ms,
        over_pct: 0.0,
        hitches: 0,
        drift_pct: 0.0,
    })
}

/// Several runs of a scene as one: the middle run's median, 99th percentile, share over
/// budget and drift, and the worst frame and most hitches of any.
fn combine(runs: &[Timing]) -> Option<Timing> {
    let middle = |pick: fn(&Timing) -> f32| {
        let mut values: Vec<f32> = runs.iter().map(pick).collect();
        values.sort_by(f32::total_cmp);
        values[values.len() / 2]
    };
    let first = runs.first()?;
    Some(Timing {
        median_ms: middle(|t| t.median_ms),
        p99_ms: middle(|t| t.p99_ms),
        worst_ms: runs.iter().map(|t| t.worst_ms).fold(0.0, f32::max),
        over_pct: middle(|t| t.over_pct),
        hitches: runs.iter().map(|t| t.hitches).max().unwrap_or(0),
        drift_pct: middle(|t| t.drift_pct),
        ..first.clone()
    })
}

/// The last recorded median for a scene on the same GPU, size and build, and its commit.
fn last_recorded(csv: &str, timing: &Timing, build: &str, scene: &str) -> Option<(f32, String)> {
    csv.lines()
        .skip(1)
        .filter_map(|line| {
            let cells: Vec<&str> = line.split(',').collect();
            let [_, commit, gpu, size, built, named, median, ..] = cells[..] else {
                return None;
            };
            (gpu == timing.gpu && size == timing.size && built == build && named == scene)
                .then(|| Some((median.parse().ok()?, commit.to_string())))?
        })
        .last()
}

/// Times the benchmark scenes, each several times, and compares them with what was last
/// recorded on this kind of machine. A release build unless `--debug`, which is quicker
/// to build and only good for comparing changes with each other.
fn bench(args: &[String]) -> bool {
    let mut runs = 3usize;
    let mut record = false;
    let mut debug = false;
    // A copy of its own, so two agents' benchmarks do not overwrite one another's program.
    let mut copy = "bench".to_string();
    let mut chosen: Vec<&Scene> = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--record" => record = true,
            "--copy" => match args.next() {
                Some(name) => copy = format!("bench-{name}"),
                None => {
                    eprintln!("--copy needs a name");
                    return false;
                }
            },
            "--debug" => debug = true,
            "--runs" => match args.next().and_then(|n| n.parse().ok()) {
                Some(n) if n > 0 => runs = n,
                _ => {
                    eprintln!("--runs needs a number above nothing");
                    return false;
                }
            },
            name => match SCENES.iter().find(|scene| scene.name == name) {
                Some(scene) => chosen.push(scene),
                None => {
                    eprintln!("No scene `{name}`. The scenes:");
                    for scene in SCENES {
                        eprintln!("  {:8} {}", scene.name, scene.about);
                    }
                    return false;
                }
            },
        }
    }
    let all_scenes = chosen.is_empty();
    if all_scenes {
        chosen = SCENES.iter().collect();
    }
    let build = if debug { "debug" } else { "release" };
    let Some((program, folder)) = sandbox_program(!debug, &copy) else {
        return false;
    };
    let commit = output("git", &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    let dirty = output("git", &["status", "--porcelain", "--untracked-files=no"])
        .is_some_and(|changes| !changes.is_empty());
    let commit = if dirty { format!("{commit}+") } else { commit };
    let date = output("date", &["+%F"]).unwrap_or_default();
    let csv = std::fs::read_to_string(root().join(BENCHMARKS)).unwrap_or_default();

    let mut ok = true;
    let mut lines = Vec::new();
    let mut busiest: Option<(f32, usize)> = None;
    for scene in chosen {
        let mut timed = Vec::new();
        for run in 1..=runs {
            let load = machine_load();
            if let Some((now, cores)) = load
                && busiest.is_none_or(|(most, _)| now > most)
            {
                busiest = Some((now, cores));
            }
            let load = load.map_or(String::new(), |(load, cores)| {
                format!(", machine load {load:.1} on {cores} cores")
            });
            println!("\n> {} {}/{runs}{load}", scene.name, run);
            match time_scene(&program, &folder, scene) {
                Some(timing) => timed.push(timing),
                None => println!("  no frame times came out"),
            }
        }
        let Some(timing) = combine(&timed) else {
            ok = false;
            continue;
        };
        let against = match last_recorded(&csv, &timing, build, scene.name) {
            Some((before, then)) => {
                let change = timing.median_ms / before - 1.0;
                let note = if change > SLOWER_BY { "  SLOWER" } else { "" };
                format!("{:+.0}% on {then}{note}", change * 100.0)
            }
            None => "nothing recorded".to_string(),
        };
        lines.push((scene.name, timing.clone(), against));
        if record {
            let row = format!(
                "{date},{commit},{},{},{build},{},{:.2},{:.2},{:.2},{},{:.2},{},{:.1}",
                timing.gpu,
                timing.size,
                scene.name,
                timing.median_ms,
                timing.p99_ms,
                timing.worst_ms,
                timed.len(),
                timing.over_pct,
                timing.hitches,
                timing.drift_pct
            );
            ok &= append_line(&root().join(BENCHMARKS), BENCHMARKS_HEADER, &row);
        }
    }
    if let Some((_, first, _)) = lines.first() {
        println!(
            "\nFrame times on {} at {}, {build} build, commit {commit}, {runs} runs each:",
            first.gpu, first.size
        );
        println!(
            "  {:11} {:>8} {:>8} {:>8} {:>6} {:>7} {:>6}  budget  against the last recorded",
            "scene", "median", "99th", "worst", "over", "hitches", "drift"
        );
        for (name, timing, against) in &lines {
            let missed = timing.misses();
            println!(
                "  {name:11} {:>5.2} ms {:>5.2} ms {:>5.2} ms {:>5.1}% {:>7} {:>+5.0}%  {:6}  {against}",
                timing.median_ms,
                timing.p99_ms,
                timing.worst_ms,
                timing.over_pct,
                timing.hitches,
                timing.drift_pct,
                if missed.is_empty() { "holds" } else { "MISSED" }
            );
            for miss in &missed {
                println!("  {:11} {miss}", "");
            }
            ok &= missed.is_empty();
        }
        println!(
            "  The budget is {BUDGET_MS:.1} ms (D25): a scene holds to it with its 99th percentile under it, \
             at most {MOST_OVER_PCT}% of frames over, no hitch over twice it, and no more than \
             {MOST_DRIFT_PCT}% slower by the end. On this machine; D25's target is a mid-range GPU."
        );
        if let Some((load, cores)) = busiest
            && load > cores as f32 / 2.0
        {
            println!(
                "  The machine was busy with other work (load up to {load:.1} on {cores} cores): \
                 numbers come out slower and less even than on a quiet machine."
            );
        }
        if record {
            println!("  Recorded in {BENCHMARKS}.");
        }
    }
    // A full benchmark also checks that nothing pops as detail changes (D230): what is
    // measured is only worth having if it looks right.
    if all_scenes && !debug {
        println!("\n> lodcheck");
        ok &= run("node", &["tools/sandbox/lodcheck.ts"]);
    }
    ok
}

/// Adds a line to a file, starting it with a header if it is new.
fn append_line(path: &Path, header: &str, line: &str) -> bool {
    use std::io::Write;
    let new = !path.exists();
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| {
            if new {
                writeln!(file, "{header}")?;
            }
            writeln!(file, "{line}")
        })
        .is_ok()
}

/// Builds the sandbox and says where its program is and the folder to run it from. Under
/// WSL that is the Windows build, the only one with a real GPU.
fn sandbox_program(release: bool, copy: &str) -> Option<(PathBuf, PathBuf)> {
    if on_wsl() {
        return windows_program("terra_sandbox", release, Some(copy));
    }
    let mut build = vec!["build", "-p", "terra_sandbox"];
    if release {
        build.push("--release");
    }
    if !run("cargo", &build) {
        return None;
    }
    let profile = if release { "release" } else { "debug" };
    let program = root().join("target").join(profile).join("terra_sandbox");
    Some((program, root()))
}

/// How busy the machine is: its load over the last minute, against its cores. A benchmark
/// shares the machine, and other work on it makes frames slower and less even.
fn machine_load() -> Option<(f32, usize)> {
    let text = output("sysctl", &["-n", "vm.loadavg"])
        .or_else(|| std::fs::read_to_string("/proc/loadavg").ok())?;
    let load = text
        .split_whitespace()
        .find_map(|word| word.parse::<f32>().ok())?;
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    Some((load, cores))
}

/// Runs one scene once and reads its frame times, ending it if it takes too long.
fn time_scene(program: &Path, folder: &Path, scene: &Scene) -> Option<Timing> {
    use std::io::{BufRead, BufReader};
    let mut command = Command::new(program);
    command
        .args(scene.args)
        .args(["--offscreen", "--bench", "--settle", BENCH_SETTLE])
        .args(["--bench-frames", scene.frames])
        .current_dir(folder);
    // Run outside Cargo, a Bevy program looks for assets/ next to itself, which is where
    // the Windows copy has them; the native one is pointed at the root's.
    if !on_wsl() {
        command.env("BEVY_ASSET_ROOT", folder);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    // The log, read only for what is worth saying beside the frames.
    let stderr = child.stderr.take()?;
    let said = std::thread::spawn(move || {
        BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| worth_saying(&line).map(str::to_string))
            .collect::<Vec<String>>()
    });
    let reader = std::thread::spawn(move || {
        let mut timing: Option<Timing> = None;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(read) = parse_timing(&line) {
                timing = Some(read);
            } else if let Some(said) = worth_saying(&line) {
                println!("  {said}");
            } else if let (Some(timing), Some((over, hitches, drift))) =
                (&mut timing, parse_sustained(&line))
            {
                (timing.over_pct, timing.hitches, timing.drift_pct) = (over, hitches, drift);
            }
        }
        timing
    });
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed().as_secs() > BENCH_TIMEOUT_S => {
                println!("  stuck after {BENCH_TIMEOUT_S} s, so ended");
                child.kill().ok();
                child.wait().ok();
                break;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(200)),
            Err(_) => break,
        }
    }
    // Each time sections were laid again as the camera moved, said as one line: how many
    // times, and how long they took to be on screen.
    let mut streamed: Vec<f32> = Vec::new();
    for line in said.join().unwrap_or_default() {
        match line
            .strip_prefix("Builds streamed in: ")
            .and_then(|ms| ms.trim_end_matches(" ms").parse().ok())
        {
            Some(ms) => streamed.push(ms),
            None => println!("  {line}"),
        }
    }
    if !streamed.is_empty() {
        streamed.sort_by(f32::total_cmp);
        println!(
            "  Builds streamed in {} times as the camera moved, in a median {:.0} ms, the slowest {:.0} ms",
            streamed.len(),
            streamed[streamed.len() / 2],
            streamed[streamed.len() - 1]
        );
    }
    let timing = reader.join().ok()??;
    println!(
        "  median {:.2} ms, 99th percentile {:.2} ms, worst {:.2} ms; {:.2}% over budget, {} hitches, {:+.1}% by the end",
        timing.median_ms,
        timing.p99_ms,
        timing.worst_ms,
        timing.over_pct,
        timing.hitches,
        timing.drift_pct
    );
    Some(timing)
}

/// Every task, by group, for `tools` and for a task that is not known.
fn usage() -> String {
    let mut text = String::from("usage: cargo xtask <task> [args]\n");
    let width = TASKS
        .iter()
        .map(|task| task.name.len() + task.takes.len() + 1)
        .max()
        .unwrap_or(0)
        .min(34);
    let mut group = "";
    for task in TASKS {
        if task.group != group {
            group = task.group;
            text.push_str(&format!("\n{group}\n"));
        }
        let call = format!("{} {}", task.name, task.takes);
        let call = call.trim_end();
        if call.len() > width {
            text.push_str(&format!("  {call}\n  {:width$}  {}\n", "", task.does));
        } else {
            text.push_str(&format!("  {call:width$}  {}\n", task.does));
        }
    }
    text
}

/// The only crates allowed to depend on Bevy (D28): the applications, and the world on
/// screen that the game stands on (D198).
const BEVY_CRATES: [&str; 3] = ["terra_game", "terra_sandbox", "terra_world"];

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_default();
    let rest: Vec<String> = args.collect();
    use_hooks();
    let ok = match TASKS
        .iter()
        .find(|task| task.name == name || (name == "help" && task.name == "tools"))
    {
        Some(task) => (task.run)(&rest),
        None => {
            eprintln!("{}", usage());
            false
        }
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Points git at the repository's hooks, so `cargo xtask check` runs before every commit (D37).
/// Any task does it, so a new clone has it as soon as anyone runs one; worktrees share it.
fn use_hooks() {
    if output("git", &["config", "--get", "core.hooksPath"]).as_deref() != Some(".githooks") {
        let set = output("git", &["config", "core.hooksPath", ".githooks"]).is_some();
        if set {
            eprintln!("git now runs `cargo xtask check` before each commit (.githooks)");
        }
    }
}

/// A TypeScript tool, run by Node from the root.
fn node(script: &str, args: &[String]) -> bool {
    let mut all = vec![script.to_string()];
    all.extend(args.iter().cloned());
    run_owned("node", &all)
}

/// A package's program, with some arguments first.
fn cargo_run(package: &str, first: &[&str], args: &[String]) -> bool {
    let mut all = vec!["run", "-p", package, "--"];
    all.extend(first);
    all.extend(args.iter().map(String::as_str));
    run("cargo", &all)
}

fn run_owned(program: &str, args: &[String]) -> bool {
    run(
        program,
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run(program: &str, args: &[&str]) -> bool {
    println!("\n> {program} {}", args.join(" "));
    Command::new(program)
        .args(args)
        .current_dir(root())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(root())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Renders every item's icon and writes them into assets/icons. It opens a window
/// for the few seconds it takes; under WSL the CPU renderer is enough.
fn icons(args: &[String]) -> bool {
    let mut all = vec!["run", "-p", "terra_sandbox", "--", "--experiment", "icons"];
    all.extend(args.iter().map(String::as_str));
    run("cargo", &all)
}

fn check() -> bool {
    // CI only needs to know that everything compiles and the tests pass, so it skips
    // optimising dependencies, which is most of the time spent building Bevy.
    let quick: &[&str] = if std::env::var_os("CI").is_some() {
        &["--config", "profile.dev.package.\"*\".opt-level=0"]
    } else {
        &[]
    };
    let cargo = |command: &'static str, rest: &[&'static str]| {
        run("cargo", &[&[command], quick, rest].concat())
    };
    run("cargo", &["fmt", "--all", "--check"])
        && cargo(
            "clippy",
            &["--workspace", "--all-targets", "--", "-D", "warnings"],
        )
        && cargo("test", &["--workspace"])
        && tools_tests()
        && kits()
        && artifacts(&["--quick".into(), "--ratchet".into()])
        && run("node", &["tools/docs/docs.ts"])
        && blender_scripts()
        && shared_crates_avoid_bevy()
        && shared_crates_are_repeatable()
}

/// The model contract's checker (docs/art-style.md, D303): every model the game places,
/// each on a line with what is wrong with it. The same test runs in `check`.
fn contract() -> bool {
    run(
        "cargo",
        &[
            "test",
            "-q",
            "-p",
            "terra_game",
            "--lib",
            "every_model_keeps_the_contract",
            "--",
            "--nocapture",
        ],
    )
}

/// The kit checker (docs/building-plan.md): every kit file in `assets/data/kits/` has the
/// materials its pattern needs, life-size numbers, and keeps to the style guide's budgets
/// (`terra_build::checker`).
fn kits() -> bool {
    run(
        "cargo",
        &["run", "-q", "-p", "terra_build", "--example", "kits"],
    )
}

/// How long the village's buildings take to lay whole and their sections to cut and merge,
/// on one thread, as the game's workers do (docs/building-plan.md, checking it); a name
/// times only the buildings whose names hold it.
fn sections(args: &[String]) -> bool {
    let mut cargo = vec![
        "run",
        "-q",
        "--release",
        "-p",
        "terra_build",
        "--example",
        "sections",
    ];
    cargo.extend(args.iter().map(String::as_str));
    run("cargo", &cargo)
}

/// The building audit (research/building-audit.md): terra_build's rules tests with the
/// known failures too, the random operations over many seeds, the drawing audit over the
/// village, and the rules checked live (tools/sandbox/rules.ts) unless `--pure`. Every part
/// runs whatever an earlier one says, and each says pass or fail.
fn rules(args: &[String]) -> bool {
    let pure = args.iter().any(|arg| arg == "--pure");
    let mut results = vec![
        (
            "rules tests, known failures included",
            run(
                "cargo",
                &[
                    "test",
                    "-q",
                    "-p",
                    "terra_build",
                    "--test",
                    "rules",
                    "--",
                    "--include-ignored",
                ],
            ),
        ),
        ("random operations over 64 seeds", {
            println!("\n> TERRA_SEEDS=64 cargo test -p terra_build random_operations");
            Command::new("cargo")
                .args([
                    "test",
                    "-q",
                    "--release",
                    "-p",
                    "terra_build",
                    "random_operations",
                ])
                .env("TERRA_SEEDS", "64")
                .current_dir(root())
                .status()
                .is_ok_and(|status| status.success())
        }),
        (
            "drawing audit",
            run(
                "cargo",
                &[
                    "run",
                    "-q",
                    "--release",
                    "-p",
                    "terra_build",
                    "--example",
                    "drawing",
                ],
            ),
        ),
    ];
    if !pure {
        let mut all = vec!["tools/sandbox/rules.ts"];
        all.extend(args.iter().map(String::as_str));
        results.push(("live rules (tools/sandbox/rules.ts)", run("node", &all)));
    }
    println!();
    for (what, passed) in &results {
        println!("{}  {what}", if *passed { "ok  " } else { "FAIL" });
    }
    results.iter().all(|(_, passed)| *passed)
}

/// What a renderer would show as junk in the meshes building draws, every roof over every
/// plan in each kind and every two kinds mixed, at every detail (`terra_build::inspect`); a
/// name keeps only the bases whose names hold it, and `--all` lists every fault found. With
/// `--frames`, what only frames show, from scripted runs on the GPU (`tools/sandbox/artifacts.ts`).
fn artifacts(args: &[String]) -> bool {
    if args.iter().any(|a| a == "--frames") {
        let mut all = vec!["tools/sandbox/artifacts.ts"];
        all.extend(args.iter().filter(|a| *a != "--frames").map(String::as_str));
        return run("node", &all);
    }
    let mut cargo = vec![
        "run",
        "-q",
        "--release",
        "-p",
        "terra_build",
        "--example",
        "artifacts",
        "--",
    ];
    cargo.extend(args.iter().map(String::as_str));
    run("cargo", &cargo)
}

/// The tests of the TypeScript tools (D121), which Node runs as they are.
fn tools_tests() -> bool {
    let Some(files) = output(
        "git",
        &["ls-files", "tools/**/*.test.ts", "tools/**/*.test.mjs"],
    ) else {
        println!("\n> tools tests: git cannot list them");
        return false;
    };
    let files: Vec<&str> = files.lines().collect();
    run("node", &[&["--test"], files.as_slice()].concat())
}

/// The Blender scripts (D121) compile and find what they import, checked inside the pinned
/// Blender without running them. A machine with no Blender skips this.
fn blender_scripts() -> bool {
    println!("\n> Blender scripts (tools/blender/check.py)");
    if output("tools/blender/run.sh", &["--find"]).is_none() {
        println!("  skipped: no Blender here (cargo xtask doctor)");
        return true;
    }
    let Some(files) = output("git", &["ls-files", "tools/**/*.py"]) else {
        println!("  git cannot list them");
        return false;
    };
    Command::new("tools/blender/run.sh")
        .arg("tools/blender/check.py")
        .args(files.lines())
        .current_dir(root())
        .status()
        .is_ok_and(|status| status.success())
}

/// The crates that must not depend on Bevy, by name and folder, or `None` when the crates
/// folder cannot be read. A check that cannot list them fails rather than passing.
fn shared_crates() -> Option<Vec<(String, PathBuf)>> {
    let entries = std::fs::read_dir(root().join("crates")).ok()?;
    let mut found: Vec<(String, PathBuf)> = entries
        .flatten()
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().to_string(),
                entry.path(),
            )
        })
        .filter(|(name, path)| {
            !BEVY_CRATES.contains(&name.as_str()) && path.join("Cargo.toml").exists()
        })
        .collect();
    found.sort();
    (!found.is_empty()).then_some(found)
}

/// What `cargo tree` says of a crate's dependencies, or `None` when it gave no answer:
/// "ok" only when it answered and Bevy is not among them.
fn bevy_verdict(tree: Option<&str>) -> &'static str {
    match tree {
        None => "CARGO TREE FAILED",
        Some(tree) if tree.lines().any(|line| line.starts_with("bevy")) => "DEPENDS ON BEVY",
        Some(_) => "ok",
    }
}

/// Anything every player must agree on lives in crates that know nothing about Bevy.
fn shared_crates_avoid_bevy() -> bool {
    println!("\n> shared-state crates must not depend on Bevy (D28)");
    let Some(crates) = shared_crates() else {
        println!("  cannot list the crates");
        return false;
    };
    let mut ok = true;
    for (name, _) in crates {
        let tree = output(
            "cargo",
            &["tree", "-p", &name, "-e", "normal", "--prefix", "none"],
        );
        let verdict = bevy_verdict(tree.as_deref());
        println!("  {name}: {verdict}");
        ok &= verdict == "ok";
    }
    ok
}

/// What makes a run of shared code come out differently from the same start: the clock and
/// the operating system's randomness. Shared code takes time and seeds as arguments.
const UNREPEATABLE: [&str; 6] = [
    "Instant::now",
    "SystemTime::now",
    "thread_rng",
    "rand::random",
    "RandomState::new",
    "getrandom",
];

/// The lines of a source that use something in [`UNREPEATABLE`], numbered from 1.
fn unrepeatable(text: &str) -> Vec<(usize, &'static str)> {
    text.lines()
        .enumerate()
        .filter_map(|(number, line)| {
            let what = UNREPEATABLE.iter().find(|what| line.contains(*what))?;
            Some((number + 1, *what))
        })
        .collect()
}

fn shared_crates_are_repeatable() -> bool {
    println!("\n> shared-state crates must not read the clock or the system's randomness");
    let Some(crates) = shared_crates() else {
        println!("  cannot list the crates");
        return false;
    };
    let mut ok = true;
    for (name, path) in crates {
        let mut found = Vec::new();
        if !sources(&path.join("src"), &mut found) || found.is_empty() {
            println!("  {name}: cannot read its sources");
            ok = false;
            continue;
        }
        let mut clean = true;
        for file in found {
            // A benchmark's own program may time itself.
            if file.ends_with("main.rs") || file.components().any(|part| part.as_os_str() == "bin")
            {
                continue;
            }
            let at = file.strip_prefix(root()).unwrap_or(&file).display();
            let Ok(text) = std::fs::read_to_string(&file) else {
                println!("  {at}: cannot read it");
                clean = false;
                continue;
            };
            for (number, what) in unrepeatable(&text) {
                println!("  {at}:{number}: {what}");
                clean = false;
            }
        }
        if clean {
            println!("  {name}: ok");
        }
        ok &= clean;
    }
    ok
}

/// Every Rust file under a folder. False if any folder in it could not be read.
fn sources(folder: &Path, found: &mut Vec<PathBuf>) -> bool {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return false;
    };
    let mut ok = true;
    for entry in entries {
        let Ok(entry) = entry else {
            ok = false;
            continue;
        };
        let path = entry.path();
        if path.is_dir() {
            ok &= sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    ok
}

fn on_wsl() -> bool {
    std::fs::read_to_string("/proc/version").is_ok_and(|v| v.to_lowercase().contains("microsoft"))
}

const WINDOWS_TARGET: &str = "x86_64-pc-windows-gnu";

/// WSL has no hardware Vulkan, so performance is measured on a Windows build launched
/// from WSL. Windows cannot use a WSL folder as a working directory, so the program is
/// copied to the Windows temp folder and run from there.
fn win(args: &[String]) -> bool {
    if !on_wsl() {
        eprintln!(
            "`cargo xtask win` is only for WSL. Elsewhere, `cargo run` already uses the GPU."
        );
        return false;
    }
    let (ours, program_args) = match args.iter().position(|a| a == "--") {
        Some(i) => (&args[..i], &args[i + 1..]),
        None => (args, &[][..]),
    };
    let release = ours.iter().any(|a| a == "--release");
    // A named copy runs from its own folder, so test runs never collide with a copy
    // someone is playing: Windows will not replace a program that is running.
    let copy = ours
        .iter()
        .position(|a| a == "--copy")
        .and_then(|i| ours.get(i + 1));
    let Some(package) = ours
        .iter()
        .position(|a| a == "-p")
        .and_then(|i| ours.get(i + 1))
    else {
        eprintln!(
            "usage: cargo xtask win -p <package> [--release] [--copy <name>] [-- program args]"
        );
        return false;
    };

    let Some((program, run_dir)) = windows_program(package, release, copy.map(String::as_str))
    else {
        return false;
    };
    let exe_name = format!("{package}.exe");
    println!(
        "\n> {exe_name} {}   (in {})",
        program_args.join(" "),
        run_dir.display()
    );
    // The kits' files are read from the source's assets, through WSL's share, so one saved
    // while the copy runs is seen (WSLENV's `/p` gives Windows the path as it names it).
    let wslenv = std::env::var("WSLENV").unwrap_or_default();
    let wslenv = [wslenv.as_str(), "TERRA_SOURCE_ASSETS/p"]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(":");
    Command::new(program)
        .args(program_args)
        .current_dir(&run_dir)
        .env("TERRA_SOURCE_ASSETS", root().join("assets"))
        .env("WSLENV", wslenv)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Builds a package for Windows and copies it, with the assets, to a folder of its own in
/// the Windows temp folder. Windows cannot use a WSL folder as a working directory. Gives
/// the program and its folder.
fn windows_program(package: &str, release: bool, copy: Option<&str>) -> Option<(PathBuf, PathBuf)> {
    let mut build = vec!["build", "--target", WINDOWS_TARGET, "-p", package];
    if release {
        build.push("--release");
    }
    if !run("cargo", &build) {
        return None;
    }

    let profile = if release { "release" } else { "debug" };
    let exe_name = format!("{package}.exe");
    let built = root()
        .join("target")
        .join(WINDOWS_TARGET)
        .join(profile)
        .join(&exe_name);
    let folder = match copy {
        Some(name) => format!("{package}-{name}"),
        None => package.to_string(),
    };
    let Some(run_dir) = windows_temp().map(|temp| temp.join("terra").join(folder)) else {
        eprintln!("Could not find the Windows temp folder. Is Windows interop enabled?");
        return None;
    };
    let staged = std::fs::create_dir_all(&run_dir)
        .and_then(|()| std::fs::copy(&built, run_dir.join(&exe_name)).map(|_| ()))
        // A Bevy program looks for its assets next to itself.
        .and_then(|()| copy_dir(&root().join("assets"), &run_dir.join("assets")));
    if let Err(error) = staged {
        eprintln!(
            "Could not copy the program to {}: {error}",
            run_dir.display()
        );
        return None;
    }
    Some((run_dir.join(&exe_name), run_dir))
}

/// Plays the tech demo as a person would: the sandbox's play experiment, with its save
/// kept between runs. Under WSL the Windows build runs, in a copy of its own, so a
/// scripted test running at the same time does not get in the way.
fn play(args: &[String]) -> bool {
    let mut game: Vec<String> = vec!["--experiment".into(), "play".into()];
    game.extend(args.iter().cloned());
    if on_wsl() {
        let mut ours: Vec<String> = ["-p", "terra_sandbox", "--copy", "play", "--"]
            .map(String::from)
            .to_vec();
        ours.extend(game);
        return win(&ours);
    }
    let mut cargo = vec!["run", "-p", "terra_sandbox", "--"];
    cargo.extend(game.iter().map(String::as_str));
    run("cargo", &cargo)
}

/// Plays the yard, the flat test level for working on building (`--level yard`).
fn yard(args: &[String]) -> bool {
    let mut game: Vec<String> = vec!["--level".into(), "yard".into()];
    game.extend(args.iter().cloned());
    play(&game)
}

/// The Windows temp folder, as a path WSL can use.
fn windows_temp() -> Option<PathBuf> {
    let run_from = |program: &str, args: &[&str]| {
        let out = Command::new(program)
            .args(args)
            // Windows programs complain when started from a WSL folder.
            .current_dir("/mnt/c")
            .stderr(Stdio::null())
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let windows_path = run_from("cmd.exe", &["/c", "echo %TEMP%"])?;
    run_from("wslpath", &["-u", &windows_path]).map(PathBuf::from)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    if !from.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

struct Report {
    missing_required: bool,
}

impl Report {
    fn item(&mut self, label: &str, found: bool, required: bool, fix: &str) {
        let state = match (found, required) {
            (true, _) => "ok     ",
            (false, true) => "MISSING",
            (false, false) => "absent ",
        };
        println!("  [{state}] {label}");
        if !found {
            println!("            {fix}");
            self.missing_required |= required;
        }
    }
}

fn doctor() -> bool {
    let mut report = Report {
        missing_required: false,
    };
    let on_wsl = on_wsl();
    let os = std::env::consts::OS;
    println!("Platform: {os}{}", if on_wsl { " under WSL" } else { "" });

    println!("\nRust");
    let pinned = std::fs::read_to_string(root().join("rust-toolchain.toml")).unwrap_or_default();
    let pinned = pinned
        .lines()
        .find_map(|l| l.strip_prefix("channel = "))
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_default();
    let rustc = output("rustc", &["--version"]).unwrap_or_default();
    report.item(
        &format!("compiler {pinned} (found: {rustc})"),
        !pinned.is_empty() && rustc.contains(&pinned),
        true,
        "rustup toolchain install   (run in the repo; it reads rust-toolchain.toml)",
    );
    let components = output("rustup", &["component", "list", "--installed"]).unwrap_or_default();
    for component in ["rustfmt", "clippy"] {
        report.item(
            component,
            components.contains(component),
            true,
            "rustup toolchain install",
        );
    }

    if os == "linux" {
        println!("\nLinux libraries Bevy needs to build");
        let pkg_config = output("pkg-config", &["--version"]).is_some();
        report.item(
            "pkg-config",
            pkg_config,
            true,
            "sudo apt install pkg-config",
        );
        for (library, package) in [
            ("alsa", "libasound2-dev"),
            ("libudev", "libudev-dev"),
            ("wayland-client", "libwayland-dev"),
            ("xkbcommon", "libxkbcommon-dev"),
        ] {
            let found = pkg_config && output("pkg-config", &["--exists", library]).is_some();
            report.item(library, found, true, &format!("sudo apt install {package}"));
        }
    }

    if os == "macos" {
        println!("\nmacOS");
        report.item(
            "Xcode command line tools",
            output("xcode-select", &["-p"]).is_some(),
            true,
            "xcode-select --install",
        );
    }

    if on_wsl {
        println!("\nWSL: building and running the Windows version (needed to use the real GPU)");
        let targets = output("rustup", &["target", "list", "--installed"]).unwrap_or_default();
        report.item(
            "Rust target x86_64-pc-windows-gnu",
            targets.contains(WINDOWS_TARGET),
            true,
            "rustup target add x86_64-pc-windows-gnu",
        );
        report.item(
            "MinGW cross-compiler",
            output("x86_64-w64-mingw32-gcc", &["--version"]).is_some(),
            true,
            "sudo apt install mingw-w64",
        );
        report.item(
            "Windows programs can be launched from WSL",
            output("cmd.exe", &["/c", "ver"]).is_some(),
            true,
            "enable Windows interop in /etc/wsl.conf ([interop] enabled=true)",
        );
        println!(
            "\n  WSL has no hardware Vulkan. The Linux build renders on the CPU, which is right\n  \
             for screenshots and wrong for performance. Measure on the Windows build."
        );
    }

    println!("\nTools");
    report.item(
        "git runs `cargo xtask check` before each commit",
        output("git", &["config", "--get", "core.hooksPath"]).as_deref() == Some(".githooks"),
        true,
        "git config core.hooksPath .githooks   (any cargo xtask task sets it)",
    );
    let node = output("node", &["--version"]).unwrap_or_default();
    report.item(
        &format!("Node 22.18 or later, which runs the TypeScript tools (found: {node})"),
        node_runs_typescript(&node),
        true,
        "install Node 22.18 or later (https://nodejs.org)",
    );

    println!("\nArt tools");
    let pinned = std::fs::read_to_string(root().join("tools/blender/version")).unwrap_or_default();
    let pinned = pinned.trim();
    let blender = output("tools/blender/run.sh", &["--find"])
        .and_then(|path| output(&path, &["--version"]))
        .and_then(|v| v.lines().next().map(str::to_string))
        .unwrap_or_default();
    report.item(
        &format!("Blender {pinned}, for the model pipeline and its check (found: {blender})"),
        blender.starts_with(&format!("Blender {pinned}.")),
        false,
        &format!("install Blender {pinned} (https://www.blender.org/download/lts/); under WSL, the Windows one"),
    );
    // Tripo's models are meshopt-compressed; some Linux distributions' Blender, such as
    // Arch's, leave out the library its glTF importer decodes them with.
    if !blender.is_empty() {
        let meshopt = output("tools/blender/run.sh", &["--find"]).and_then(|path| {
            output(
                &path,
                &[
                    "--background",
                    "--factory-startup",
                    "--python-expr",
                    "from io_scene_gltf2.io.imp.gltf2_io_binary_meshopt import MeshoptDecoder as M\n\
                     try:\n    print('MESHOPT', M.find_library())\nexcept Exception as e:\n    print('NOMESHOPT', e)",
                ],
            )
        });
        let found = meshopt.as_deref().unwrap_or_default();
        let missing = found
            .lines()
            .find_map(|line| line.strip_prefix("NOMESHOPT Meshopt decoder library not found at "))
            .unwrap_or("its glTF add-on folder");
        report.item(
            "Blender opens Tripo's compressed models (meshopt)",
            found.lines().any(|line| line.starts_with("MESHOPT ")),
            false,
            &format!(
                "this Blender was built without libbf_intern_meshopt_bridge.so: copy it from the official \
                 Blender {pinned} for Linux (its lib folder) to {missing}, or install that build and set BLENDER"
            ),
        );
    }
    report.item(
        "ImageMagick, for painting item icons (tools/icons)",
        output("convert", &["-version"]).is_some(),
        false,
        "sudo apt install imagemagick (or brew install imagemagick)",
    );

    println!();
    if report.missing_required {
        println!("Some required items are missing. Fix them and run `cargo xtask doctor` again.");
    } else {
        println!("This machine is ready.");
    }
    !report.missing_required
}

/// Whether a `node --version` is 22.18 or later, the first to run TypeScript as it is.
fn node_runs_typescript(version: &str) -> bool {
    let mut parts = version
        .trim_start_matches('v')
        .split('.')
        .map(|p| p.parse::<u32>().unwrap_or(0));
    let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    major > 22 || (major == 22 && minor >= 18)
}

#[cfg(test)]
mod tests {
    use super::node_runs_typescript;

    #[test]
    fn node_runs_typescript_from_22_18() {
        assert!(node_runs_typescript("v22.18.0"));
        assert!(node_runs_typescript("v24.14.0"));
        assert!(!node_runs_typescript("v22.17.1"));
        assert!(!node_runs_typescript("v20.19.0"));
        assert!(!node_runs_typescript(""));
    }
}

#[cfg(test)]
mod bench_tests {
    use super::*;

    fn timing(median_ms: f32, p99_ms: f32, worst_ms: f32) -> Timing {
        Timing {
            gpu: "Apple M4 Max".into(),
            size: "1920x1080 offscreen".into(),
            median_ms,
            p99_ms,
            worst_ms,
            over_pct: 0.0,
            hitches: 0,
            drift_pct: 0.0,
        }
    }

    #[test]
    fn reads_how_a_run_held_to_the_budget() {
        let line = "SANDBOX against the 16.7 ms budget: 0.50% of frames over, 2 hitches over \
                    33.3 ms, the game's work in the last third -3.0% on the first";
        assert_eq!(parse_sustained(line), Some((0.5, 2, -3.0)));
        assert_eq!(parse_sustained("SANDBOX saved shot.png"), None);
    }

    #[test]
    fn a_scene_holds_to_the_budget_unless_it_misses_a_gate() {
        assert!(timing(5.0, 9.0, 20.0).misses().is_empty());
        assert_eq!(timing(5.0, 18.0, 20.0).misses().len(), 1);
        let stutters = Timing {
            hitches: 1,
            drift_pct: 20.0,
            over_pct: 2.0,
            ..timing(5.0, 9.0, 40.0)
        };
        assert_eq!(stutters.misses().len(), 3);
    }

    #[test]
    fn reads_the_sandboxs_frame_times() {
        let line = "SANDBOX frames on Apple M4 Max at 1920x1080 offscreen: median 7.51 ms, \
                    99th percentile 15.39 ms, worst 16.40 ms";
        assert_eq!(parse_timing(line), Some(timing(7.51, 15.39, 16.40)));
        assert_eq!(parse_timing("SANDBOX saved shot.png"), None);
    }

    #[test]
    fn runs_combine_as_the_middle_one_and_the_worst_frame() {
        let runs = [
            timing(9.0, 12.0, 30.0),
            timing(7.0, 10.0, 11.0),
            timing(8.0, 11.0, 12.0),
        ];
        assert_eq!(combine(&runs), Some(timing(8.0, 11.0, 30.0)));
        assert_eq!(combine(&[]), None);
    }

    #[test]
    fn compares_with_the_last_result_on_the_same_kind_of_run() {
        let csv = format!(
            "{BENCHMARKS_HEADER}\n\
             2026-10-12,aaa,Apple M4 Max,1920x1080 offscreen,release,forest,6.00,7,8,3\n\
             2026-10-12,bbb,Apple M4 Max,1920x1080 offscreen,debug,forest,14.00,15,16,3\n\
             2026-10-13,ccc,Apple M4 Max,1920x1080 offscreen,release,forest,5.00,6,7,3\n\
             2026-10-13,ddd,Radeon,1920x1080 offscreen,release,forest,20.00,21,22,3\n"
        );
        let now = timing(5.5, 6.0, 7.0);
        assert_eq!(
            last_recorded(&csv, &now, "release", "forest"),
            Some((5.0, "ccc".to_string()))
        );
        assert_eq!(last_recorded(&csv, &now, "release", "yard"), None);
    }
}

#[cfg(test)]
mod task_tests {
    use super::*;

    #[test]
    fn every_task_has_its_own_name() {
        // The first task of a name is the one that runs, so a second would never run.
        for task in TASKS {
            assert_eq!(
                TASKS.iter().filter(|other| other.name == task.name).count(),
                1,
                "{} twice",
                task.name
            );
        }
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    #[test]
    fn only_an_answer_without_bevy_passes() {
        let tree = "terra_gen v0.1.0\nglam v0.30.0\nlibm v0.2.15";
        assert_eq!(bevy_verdict(Some(tree)), "ok");
        let with = "terra_gen v0.1.0\nglam v0.30.0\nbevy_math v0.17.0";
        assert_ne!(bevy_verdict(Some(with)), "ok");
        // No answer from cargo is a failure, not a pass.
        assert_ne!(bevy_verdict(None), "ok");
    }

    #[test]
    fn the_clock_and_the_systems_randomness_are_found_by_line() {
        let text = "let a = 1;\nlet t = Instant::now();\nlet b = 2;\nlet r = rand::random::<u8>();";
        assert_eq!(
            unrepeatable(text),
            vec![(2, "Instant::now"), (4, "rand::random")]
        );
        assert!(unrepeatable("let seed = Seed(1).rng();").is_empty());
    }

    #[test]
    fn the_shared_crates_are_found_and_the_bevy_ones_are_not() {
        let crates = shared_crates().expect("the crates folder is readable");
        let names: Vec<&str> = crates.iter().map(|(name, _)| name.as_str()).collect();
        assert!(names.contains(&"terra_core") && names.contains(&"terra_voxel"));
        assert!(BEVY_CRATES.iter().all(|bevy| !names.contains(bevy)));
    }
}
