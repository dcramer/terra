//! `cargo xtask bench` (D185): the scenes timed in release, offscreen at 1920x1080 on the
//! real GPU, each judged against the frame budget and the last recorded result.

use crate::load::{self, During, Load};
use crate::scenes::{SCENES, Scene};
use crate::windows::{on_wsl, profile_folder, windows_program};
use crate::{Task, node, output, root, run};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The tasks this module runs (D410): the benchmark scenes, and the rebuild times beside them.
pub(crate) const TASKS: &[Task] = &[
    Task {
        name: "bench",
        takes: "[scene…] [--runs N] [--record] [--debug] [--copy NAME]",
        does: "time the benchmark scenes: release build, offscreen at 1920x1080, against the last recorded; with every scene, also lodcheck",
        group: "Play and check",
        run: bench,
    },
    Task {
        name: "rebuilds",
        takes: "[--dev|--release] [probe…]",
        does: "how long the sandbox takes to rebuild after a file in each part of the code is touched, in dev and release, and which crates were compiled again, against the last run (D407, tools/rebuilds)",
        group: "Play and check",
        run: |args| node("tools/rebuilds/rebuilds.ts", args),
    },
];

/// Frames run before any are timed, so shaders are ready and the scene has settled.
const BENCH_SETTLE: &str = "240";
/// A run that has not finished in this long is stuck, and is ended; counted from when it has
/// its turn on the GPU (D319), not while it waits for it.
const BENCH_TIMEOUT_S: u64 = 300;
/// How much slower than the last recorded result counts as a regression to look into.
const SLOWER_BY: f32 = 0.10;
/// Where `--record` keeps results, one line per scene per run of the task.
const BENCHMARKS: &str = "research/benchmarks.csv";
const BENCHMARKS_HEADER: &str =
    "date,commit,gpu,size,build,scene,median_ms,p99_ms,worst_ms,runs,over_pct,hitches,drift_pct";
/// What a scene must hold to on the machine it runs on, against the frame budget (D25)
/// the sandbox judged its frames by and says (`terra_world::debug::FRAME_BUDGET_MS`, the
/// one place it is set): at most this share of frames over it, no hitch (a frame over
/// twice it), and its last third no slower than its first by more than this.
const MOST_OVER_PCT: f32 = 1.0;
const MOST_DRIFT_PCT: f32 = 15.0;
/// The integrated GPU's target at the low preset (D239): 30 frames a second, a median under
/// this.
const INTEGRATED_MEDIAN_MS: f32 = 1000.0 / 30.0;

/// One timed run, as the sandbox prints it.
#[derive(Debug, Clone, PartialEq)]
struct Timing {
    gpu: String,
    size: String,
    median_ms: f32,
    p99_ms: f32,
    worst_ms: f32,
    /// The budget the sandbox judged the run against, and how the run held to it, from
    /// its next line.
    budget_ms: f32,
    over_pct: f32,
    hitches: u32,
    drift_pct: f32,
    /// The kind of GPU it ran on as wgpu says it (`DiscreteGpu`, `IntegratedGpu`) and the
    /// preset it drew at, from the run's own line; empty if it did not say.
    device: String,
    preset: String,
}

impl Timing {
    /// Whether it is held to the integrated GPU's target (D239) rather than the budget (D25).
    fn integrated(&self) -> bool {
        self.device == "IntegratedGpu"
    }

    /// The decision it is judged by.
    fn judged_by(&self) -> &'static str {
        if self.integrated() { "D239" } else { "D25" }
    }

    /// What the scene missed of its target, if anything: the integrated GPU's median at the
    /// low preset (D239), or every gate of the frame budget (D25).
    fn misses(&self) -> Vec<String> {
        if self.integrated() {
            return if self.median_ms > INTEGRATED_MEDIAN_MS {
                vec![format!(
                    "median over {INTEGRATED_MEDIAN_MS:.1} ms, the integrated GPU's 30 fps"
                )]
            } else {
                Vec::new()
            };
        }
        let mut missed = Vec::new();
        if self.budget_ms <= 0.0 {
            missed.push("the run did not say how it held to the budget".to_string());
        } else if self.p99_ms > self.budget_ms {
            missed.push(format!("99th percentile over {:.1} ms", self.budget_ms));
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

/// Reads `SANDBOX against the 13.3 ms budget: 0.50% of frames over, 2 hitches over
/// 26.7 ms, the last third +3.0% on the first`, as the budget, the share over, hitches and
/// drift.
fn parse_sustained(line: &str) -> Option<(f32, f32, u32, f32)> {
    let rest = line.strip_prefix("SANDBOX against the ")?;
    let (budget, rest) = rest.split_once(" ms budget: ")?;
    let (over, rest) = rest.split_once("% of frames over, ")?;
    let (hitches, rest) = rest.split_once(" hitches over ")?;
    let (_, drift) = rest.split_once("in the last third ")?;
    let drift = drift.strip_suffix("% on the first")?;
    Some((
        budget.parse().ok()?,
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
        "render world, median: ",
        "GPU passes, ",
        "CPU passes, ",
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
        budget_ms: 0.0,
        over_pct: 0.0,
        hitches: 0,
        drift_pct: 0.0,
        device: String::new(),
        preset: String::new(),
    })
}

/// Reads `SANDBOX drawn at the low preset on a IntegratedGpu GPU` as the preset and the kind
/// of GPU.
fn parse_drawn(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("SANDBOX drawn at the ")?;
    let (preset, rest) = rest.split_once(" preset on a ")?;
    let device = rest.strip_suffix(" GPU")?;
    Some((preset.to_string(), device.to_string()))
}

/// One run of a scene: its frames, the load before it and how long it waited for a quiet
/// machine, and the load while it ran.
#[derive(Debug, Clone)]
struct Run {
    timing: Timing,
    before: Option<Load>,
    during: Option<During>,
}

impl Run {
    /// Taken on a busy machine: it started busy, or was busy while it ran.
    fn busy(&self) -> bool {
        self.before.is_some_and(|load| !load.quiet()) || self.during.is_some_and(|d| d.busy())
    }
}

/// How a scene's runs went, for its line in the table.
struct Note {
    /// The runs its result is taken from, of all it made.
    runs: usize,
    of: usize,
    /// Their medians' lowest and highest, and that range as a percentage of the middle one.
    low: f32,
    high: f32,
    spread_pct: f32,
    /// How many of them held to the target.
    holding: usize,
    /// Whether they were taken on a busy machine.
    busy: bool,
}

/// The runs a scene's result is taken from: the quiet ones when at least half are, so noise
/// nobody chose is left out; otherwise all of them, which the table says were busy.
fn chosen_runs(runs: &[Run]) -> (Vec<Timing>, bool) {
    let quiet: Vec<Timing> = runs
        .iter()
        .filter(|run| !run.busy())
        .map(|run| run.timing.clone())
        .collect();
    if !quiet.is_empty() && quiet.len() * 2 >= runs.len() {
        (quiet, false)
    } else {
        (runs.iter().map(|run| run.timing.clone()).collect(), true)
    }
}

/// How far apart the runs' medians are: their lowest and highest, and that range as a share
/// of their middle one, in percent.
fn spread(runs: &[Timing]) -> (f32, f32, f32) {
    let medians: Vec<f32> = runs.iter().map(|t| t.median_ms).collect();
    let low = medians.iter().copied().fold(f32::MAX, f32::min);
    let high = medians.iter().copied().fold(0.0, f32::max);
    let mut sorted = medians.clone();
    sorted.sort_by(f32::total_cmp);
    let middle = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
    let pct = if middle > 0.0 {
        100.0 * (high - low) / middle
    } else {
        0.0
    };
    (low, high, pct)
}

/// Whether a median is slower than the last recorded by more than chance: more than
/// `SLOWER_BY`, and more than the runs' own spread.
fn slower(change: f32, spread_pct: f32) -> bool {
    change > SLOWER_BY.max(spread_pct / 100.0)
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

/// What `cargo xtask bench` was asked: its runs, whether to record, the build, the copy,
/// the preset and the scenes, every scene if none was named.
struct Asked {
    runs: usize,
    record: bool,
    debug: bool,
    copy: String,
    quality: Option<String>,
    chosen: Vec<&'static Scene>,
    all_scenes: bool,
}

fn asked(args: &[String]) -> Option<Asked> {
    let mut runs = 3usize;
    let mut record = false;
    let mut debug = false;
    // A copy of its own, so two agents' benchmarks do not overwrite one another's program.
    let mut copy = "bench".to_string();
    // A preset other than the GPU's, measured and recorded as `<scene>@<preset>`; ultra, the
    // films' (D428), is measured for its cost and held to no budget.
    let mut quality: Option<String> = None;
    let mut chosen: Vec<&'static Scene> = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--record" => record = true,
            "--copy" => match args.next() {
                Some(name) => copy = format!("bench-{name}"),
                None => {
                    eprintln!("--copy needs a name");
                    return None;
                }
            },
            "--debug" => debug = true,
            "--quality" => match args.next() {
                Some(name) => quality = Some(name.clone()),
                None => {
                    eprintln!("--quality needs a preset: low, medium, high or ultra");
                    return None;
                }
            },
            "--runs" => match args.next().and_then(|n| n.parse().ok()) {
                Some(n) if n > 0 => runs = n,
                _ => {
                    eprintln!("--runs needs a number above nothing");
                    return None;
                }
            },
            name => match SCENES.iter().find(|scene| scene.name == name) {
                Some(scene) => chosen.push(scene),
                None => {
                    eprintln!("No scene `{name}`. The scenes:");
                    for scene in SCENES {
                        eprintln!("  {:8} {}", scene.name, scene.about);
                    }
                    return None;
                }
            },
        }
    }
    let all_scenes = chosen.is_empty();
    if all_scenes {
        chosen = SCENES.iter().collect();
    }
    Some(Asked {
        runs,
        record,
        debug,
        copy,
        quality,
        chosen,
        all_scenes,
    })
}

/// Times the benchmark scenes, each several times, and compares them with what was last
/// recorded on this kind of machine. A release build unless `--debug`, which is quicker
/// to build and only good for comparing changes with each other.
pub(crate) fn bench(args: &[String]) -> bool {
    let Some(Asked {
        runs,
        record,
        debug,
        copy,
        quality,
        chosen,
        all_scenes,
    }) = asked(args)
    else {
        return false;
    };
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
    let mut waiting_left = load::MOST_WAITING;
    let held = quality.as_deref() != Some("ultra");
    for scene in chosen {
        let name = match &quality {
            Some(preset) => format!("{}@{preset}", scene.name),
            None => scene.name.to_string(),
        };
        let mut timed: Vec<Run> = Vec::new();
        // Its runs, each after waiting (a bounded time) for a quiet machine; then, while the
        // machine is quiet, up to as many again to stand in for runs taken while it was busy.
        let mut attempt = 0;
        while attempt < runs * 2 && timed.iter().filter(|run| !run.busy()).count() < runs {
            let (before, waited) = load::wait_for_quiet(load::WAIT.min(waiting_left));
            waiting_left = waiting_left.saturating_sub(waited);
            let quiet = before.is_none_or(|load| load.quiet());
            if attempt >= runs && !quiet {
                println!("  the machine is still busy, so no more runs to stand in for busy ones");
                break;
            }
            attempt += 1;
            let said = before.map_or(String::new(), |load| {
                let waited = if waited.as_secs() > 0 {
                    format!(", after waiting {} s for it to quieten", waited.as_secs())
                } else {
                    String::new()
                };
                format!(
                    ", {}{}{waited}",
                    load.describe(),
                    if quiet { "" } else { ", busy" }
                )
            });
            let extra = if attempt > runs {
                " (standing in for a busy run)"
            } else {
                ""
            };
            println!("\n> {name} {attempt}/{runs}{extra}{said}");
            let watch = load::Watch::start();
            let timing = time_scene(&program, &folder, scene, quality.as_deref());
            let during = watch.stop();
            if let Some(during) = during {
                println!(
                    "  while it ran, everything but it: processors {:.0}% busy on average, {:.0}% at most{}",
                    during.mean_cpu_pct,
                    during.most_cpu_pct,
                    if during.busy() { ", busy" } else { "" }
                );
            }
            match timing {
                Some(timing) => timed.push(Run {
                    timing,
                    before,
                    during,
                }),
                None => println!("  no frame times came out"),
            }
        }
        let (used, busy) = chosen_runs(&timed);
        let Some(timing) = combine(&used) else {
            ok = false;
            continue;
        };
        let (low, high, spread_pct) = spread(&used);
        // Whether the runs agree on the verdict: one that held and one that missed say the
        // scene is near its target, whichever the middle run says.
        let holding = used.iter().filter(|t| t.misses().is_empty()).count();
        let against = match last_recorded(&csv, &timing, build, &name) {
            Some((before, then)) => {
                let change = timing.median_ms / before - 1.0;
                let note = if slower(change, spread_pct) {
                    "  SLOWER"
                } else {
                    ""
                };
                format!("{:+.0}% on {then}{note}", change * 100.0)
            }
            None => "nothing recorded".to_string(),
        };
        let note = Note {
            runs: used.len(),
            of: timed.len(),
            low,
            high,
            spread_pct,
            holding,
            busy,
        };
        lines.push((name.clone(), timing.clone(), against, note));
        if record {
            let row = format!(
                "{date},{commit},{},{},{build},{},{:.2},{:.2},{:.2},{},{:.2},{},{:.1}",
                timing.gpu,
                timing.size,
                name,
                timing.median_ms,
                timing.p99_ms,
                timing.worst_ms,
                used.len(),
                timing.over_pct,
                timing.hitches,
                timing.drift_pct
            );
            ok &= append_line(&root().join(BENCHMARKS), BENCHMARKS_HEADER, &row);
        }
    }
    if let Some((_, first, _, _)) = lines.first() {
        println!(
            "\nFrame times on {} at {}, {build} build, commit {commit}, {runs} runs each, the middle run's:",
            first.gpu, first.size
        );
        println!(
            "  {:11} {:>8} {:>19} {:>8} {:>8} {:>6} {:>7} {:>6}  {:15}  against the last recorded",
            "scene",
            "median",
            "runs' medians",
            "99th",
            "worst",
            "over",
            "hitches",
            "drift",
            "verdict"
        );
        for (name, timing, against, note) in &lines {
            let missed = timing.misses();
            let verdict = format!(
                "{} {}",
                if missed.is_empty() { "holds" } else { "MISSED" },
                timing.judged_by()
            );
            let range = format!(
                "{:.2}-{:.2} ±{:.0}%",
                note.low,
                note.high,
                note.spread_pct / 2.0
            );
            println!(
                "  {name:11} {:>5.2} ms {range:>19} {:>5.2} ms {:>5.2} ms {:>5.1}% {:>7} {:>+5.0}%  {verdict:15}  {against}",
                timing.median_ms,
                timing.p99_ms,
                timing.worst_ms,
                timing.over_pct,
                timing.hitches,
                timing.drift_pct,
            );
            for miss in &missed {
                println!("  {:11} {miss}", "");
            }
            if note.holding != 0 && note.holding != note.runs {
                println!(
                    "  {:11} near its target: {} of {} runs held",
                    "", note.holding, note.runs
                );
            }
            if note.busy {
                println!(
                    "  {:11} taken on a busy machine (fewer than half of {} runs quiet): read it as an upper bound",
                    "", note.of
                );
            } else if note.runs < note.of {
                println!(
                    "  {:11} from the {} quiet runs of {}; the busy ones left out",
                    "", note.runs, note.of
                );
            }
            if held {
                ok &= missed.is_empty();
            } else if !missed.is_empty() {
                println!("  {:11} ultra is measured, not held to a budget (D428)", "");
            }
        }
        println!(
            "  The budget is {:.1} ms (D25: 75 fps on the developers' RTX 3080 at the high preset): \
             a scene holds to it with its 99th percentile under it, at most {MOST_OVER_PCT}% of \
             frames over, no hitch over twice it, and no more than {MOST_DRIFT_PCT}% slower by \
             the end, on the machine it runs on. On an integrated GPU, at the low preset, a \
             median under {INTEGRATED_MEDIAN_MS:.1} ms (D239). A median is SLOWER than the last \
             recorded past {:.0}% or the runs' own spread, whichever is more; a run is busy \
             when the processors were over {:.0}% busy (or the GPU {:.0}%) before it, after \
             waiting for them, or everything but the run over {:.0}% on average ({:.0}% at once) while it ran.",
            first.budget_ms,
            SLOWER_BY * 100.0,
            load::QUIET_CPU_PCT,
            load::QUIET_GPU_PCT,
            load::BUSY_DURING_CPU_PCT,
            load::BUSY_DURING_MOST_PCT,
        );
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
/// WSL that is the Windows build, the only one with a real GPU. Optimised, it is the `bench`
/// profile: a release with nothing built incrementally, as a player's build is, where the
/// release that shots and films build has our crates incremental (Cargo.toml).
fn sandbox_program(release: bool, copy: &str) -> Option<(PathBuf, PathBuf)> {
    let profile = if release { "bench" } else { "dev" };
    if on_wsl() {
        return windows_program("terra_sandbox", profile, Some(copy));
    }
    if !crate::stage::build(&["build", "--profile", profile, "-p", "terra_sandbox"]) {
        return None;
    }
    let program = root()
        .join("target")
        .join(profile_folder(profile))
        .join("terra_sandbox");
    Some((program, root()))
}

/// Runs one scene once and reads its frame times, ending it if it takes too long.
fn time_scene(
    program: &Path,
    folder: &Path,
    scene: &Scene,
    quality: Option<&str>,
) -> Option<Timing> {
    use std::io::{BufRead, BufReader};
    let mut command = Command::new(program);
    command.args(scene.args);
    if let Some(preset) = quality {
        command.args(["--quality", preset]);
    }
    command
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
    // A run waiting its turn on the GPU (D319) is not stuck: its time starts once it has it.
    let turn = std::sync::Arc::new(std::sync::Mutex::new((false, std::time::Instant::now())));
    let turn_seen = turn.clone();
    let reader = std::thread::spawn(move || {
        let mut timing: Option<Timing> = None;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if line.starts_with("SANDBOX waiting for the GPU") {
                if let Ok(mut turn) = turn_seen.lock() {
                    turn.0 = true;
                }
            } else if line.starts_with("SANDBOX got the GPU")
                && let Ok(mut turn) = turn_seen.lock()
            {
                *turn = (false, std::time::Instant::now());
            }
            if let Some(read) = parse_timing(&line) {
                timing = Some(read);
            } else if let (Some(timing), Some((preset, device))) = (&mut timing, parse_drawn(&line))
            {
                (timing.preset, timing.device) = (preset, device);
            } else if let Some(said) = worth_saying(&line) {
                println!("  {said}");
            } else if let (Some(timing), Some((budget, over, hitches, drift))) =
                (&mut timing, parse_sustained(&line))
            {
                (
                    timing.budget_ms,
                    timing.over_pct,
                    timing.hitches,
                    timing.drift_pct,
                ) = (budget, over, hitches, drift);
            }
        }
        timing
    });
    let stuck = || {
        turn.lock()
            .is_ok_and(|turn| !turn.0 && turn.1.elapsed().as_secs() > BENCH_TIMEOUT_S)
    };
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if stuck() => {
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
            budget_ms: 13.3,
            over_pct: 0.0,
            hitches: 0,
            drift_pct: 0.0,
            device: "DiscreteGpu".into(),
            preset: "high".into(),
        }
    }

    #[test]
    fn reads_how_a_run_held_to_the_budget() {
        let line = "SANDBOX against the 13.3 ms budget: 0.50% of frames over, 2 hitches over \
                    26.7 ms, the game's work in the last third -3.0% on the first";
        assert_eq!(parse_sustained(line), Some((13.3, 0.5, 2, -3.0)));
        assert_eq!(parse_sustained("SANDBOX saved shot.png"), None);
    }

    #[test]
    fn a_scene_holds_to_the_budget_unless_it_misses_a_gate() {
        assert!(timing(5.0, 9.0, 20.0).misses().is_empty());
        assert_eq!(timing(5.0, 14.0, 20.0).misses().len(), 1);
        // Judged against the budget the run said, not one of its own.
        let at_60 = Timing {
            budget_ms: 16.7,
            ..timing(5.0, 14.0, 20.0)
        };
        assert!(at_60.misses().is_empty());
        // A run that never said how it held is not taken to have held.
        let unsaid = Timing {
            budget_ms: 0.0,
            ..timing(5.0, 9.0, 20.0)
        };
        assert_eq!(unsaid.misses().len(), 1);
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
        let read = Timing {
            budget_ms: 0.0,
            device: String::new(),
            preset: String::new(),
            ..timing(7.51, 15.39, 16.40)
        };
        assert_eq!(parse_timing(line), Some(read));
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

    #[test]
    fn the_integrated_gpu_is_held_to_its_median_not_the_budget() {
        let line = "SANDBOX drawn at the low preset on a IntegratedGpu GPU";
        assert_eq!(
            parse_drawn(line),
            Some(("low".to_string(), "IntegratedGpu".to_string()))
        );
        let integrated = |median_ms| Timing {
            device: "IntegratedGpu".into(),
            preset: "low".into(),
            hitches: 4,
            ..timing(median_ms, 45.0, 90.0)
        };
        // Over the 13.3 ms budget at every gate, but under 33.3 ms at the median: it holds.
        assert!(integrated(30.0).misses().is_empty());
        assert_eq!(integrated(34.0).misses().len(), 1);
        assert_eq!(integrated(30.0).judged_by(), "D239");
        assert_eq!(timing(5.0, 9.0, 20.0).judged_by(), "D25");
    }

    #[test]
    fn busy_runs_are_left_out_when_enough_are_quiet() {
        let quiet = Load {
            cpu_pct: 10.0,
            gpu_pct: Some(1.0),
        };
        let busy = Load {
            cpu_pct: 90.0,
            gpu_pct: Some(1.0),
        };
        let run = |median, before| Run {
            timing: timing(median, 9.0, 12.0),
            before: Some(before),
            during: None,
        };
        let (used, was_busy) = chosen_runs(&[run(5.0, quiet), run(9.0, busy), run(5.2, quiet)]);
        assert_eq!(used.len(), 2);
        assert!(!was_busy);
        // Mostly busy: every run is used, and said to be busy.
        let (used, was_busy) = chosen_runs(&[run(5.0, quiet), run(9.0, busy), run(8.0, busy)]);
        assert_eq!(used.len(), 3);
        assert!(was_busy);
        // A run quiet before it but busy while it ran is busy.
        let during = Run {
            during: Some(During {
                mean_cpu_pct: 80.0,
                most_cpu_pct: 95.0,
            }),
            ..run(5.0, quiet)
        };
        assert!(during.busy());
    }

    #[test]
    fn slower_is_past_the_runs_own_spread() {
        let runs = [
            timing(5.0, 9.0, 9.0),
            timing(6.0, 9.0, 9.0),
            timing(5.5, 9.0, 9.0),
        ];
        let (low, high, pct) = spread(&runs);
        assert_eq!((low, high), (5.0, 6.0));
        assert!((pct - 100.0 / 5.5).abs() < 0.01);
        // 12% slower than recorded: past 10%, but within an 18% spread, so not SLOWER.
        assert!(!slower(0.12, pct));
        assert!(slower(0.20, pct));
        assert!(slower(0.12, 4.0));
    }
}
