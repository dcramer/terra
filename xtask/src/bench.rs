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
        takes: "[scene…] [--runs N] [--record] [--debug] [--copy NAME] [--quality PRESET] [--tune NAME=VALUE…]",
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
/// the preset, the tunables set and the scenes, every scene if none was named.
struct Asked {
    runs: usize,
    record: bool,
    debug: bool,
    copy: String,
    quality: Option<String>,
    /// Each `NAME=VALUE` passed on as `--tune` (D547), sorted by name so a key reads the same
    /// whichever order they were given in.
    tunes: Vec<String>,
    chosen: Vec<&'static Scene>,
    all_scenes: bool,
}

/// A `--tune` assignment as its key spells it: `NAME=VALUE`, a number written the shortest
/// way (`0.250` is `0.25`), so one value is one key. None if it is not an assignment.
fn tune_assignment(arg: &str) -> Option<(String, String)> {
    let (name, value) = arg.split_once('=')?;
    let (name, value) = (name.trim(), value.trim());
    if name.is_empty() || value.is_empty() || name.contains([',', '@', '=']) {
        return None;
    }
    let value = value
        .parse::<f64>()
        .map_or(value.to_string(), |n| n.to_string());
    (!value.contains([',', '@'])).then(|| (name.to_string(), value))
}

/// The name a scene's results are recorded and compared under: the scene, then any preset
/// (`@high`), then each tunable set (`@tune:NAME=VALUE`), so a run that changed either is
/// never compared with, or moves, the scene's plain baseline.
fn result_key(scene: &str, quality: Option<&str>, tunes: &[String]) -> String {
    let mut key = scene.to_string();
    if let Some(preset) = quality {
        key += &format!("@{preset}");
    }
    for tune in tunes {
        key += &format!("@tune:{tune}");
    }
    key
}

/// The keys already in the results, of those a tuned run would record: a tuned key is
/// recorded once, when it is new, as the baseline it compares against (lowering one is the
/// nightly's alone, D414, and the nightly runs no tuned scene).
fn recorded_already(csv: &str, keys: &[String]) -> Vec<String> {
    keys.iter()
        .filter(|key| {
            csv.lines()
                .skip(1)
                .any(|line| line.split(',').nth(5) == Some(key.as_str()))
        })
        .cloned()
        .collect()
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
    let mut tunes: Vec<(String, String)> = Vec::new();
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
            "--tune" => match args.next().map(|arg| (arg, tune_assignment(arg))) {
                Some((_, Some((name, value)))) if tunes.iter().all(|(set, _)| *set != name) => {
                    tunes.push((name, value));
                }
                Some((_, Some((name, _)))) => {
                    eprintln!("--tune sets {name} twice");
                    return None;
                }
                given => {
                    let given = given.map_or("nothing".into(), |(arg, _)| format!("`{arg}`"));
                    eprintln!("--tune needs NAME=VALUE (D547), not {given}");
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
    tunes.sort();
    Some(Asked {
        runs,
        record,
        debug,
        copy,
        quality,
        tunes: tunes
            .into_iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect(),
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
        tunes,
        chosen,
        all_scenes,
    }) = asked(args)
    else {
        return false;
    };
    let csv = std::fs::read_to_string(root().join(BENCHMARKS)).unwrap_or_default();
    let key = |scene: &Scene| result_key(scene.name, quality.as_deref(), &tunes);
    if record && !tunes.is_empty() {
        let keys: Vec<String> = chosen.iter().map(|scene| key(scene)).collect();
        let there = recorded_already(&csv, &keys);
        if !there.is_empty() {
            eprintln!(
                "--record with --tune records only a key new to {BENCHMARKS}, and {} is there \
                 already: a tuned run is compared with its first record, never re-recorded \
                 (lowering a baseline is the nightly's, D414). Run it without --record.",
                there.join(", ")
            );
            return false;
        }
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

    let mut ok = true;
    let mut lines = Vec::new();
    let mut waiting_left = load::MOST_WAITING;
    let held = quality.as_deref() != Some("ultra");
    for scene in chosen {
        let name = key(scene);
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
            let timing = time_scene(&program, &folder, scene, quality.as_deref(), &tunes);
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
        // The scene column as wide as its longest key, a tuned one's included.
        let w = lines
            .iter()
            .map(|(name, ..)| name.len())
            .max()
            .unwrap_or(0)
            .max(11);
        println!(
            "\nFrame times on {} at {}, {build} build, commit {commit}, {runs} runs each, the middle run's:",
            first.gpu, first.size
        );
        println!(
            "  {:w$} {:>8} {:>19} {:>8} {:>8} {:>6} {:>7} {:>6}  {:15}  against the last recorded",
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
                "  {name:w$} {:>5.2} ms {range:>19} {:>5.2} ms {:>5.2} ms {:>5.1}% {:>7} {:>+5.0}%  {verdict:15}  {against}",
                timing.median_ms,
                timing.p99_ms,
                timing.worst_ms,
                timing.over_pct,
                timing.hitches,
                timing.drift_pct,
            );
            for miss in &missed {
                println!("  {:w$} {miss}", "");
            }
            if note.holding != 0 && note.holding != note.runs {
                println!(
                    "  {:w$} near its target: {} of {} runs held",
                    "", note.holding, note.runs
                );
            }
            if note.busy {
                println!(
                    "  {:w$} taken on a busy machine (fewer than half of {} runs quiet): read it as an upper bound",
                    "", note.of
                );
            } else if note.runs < note.of {
                println!(
                    "  {:w$} from the {} quiet runs of {}; the busy ones left out",
                    "", note.runs, note.of
                );
            }
            if held {
                ok &= missed.is_empty();
            } else if !missed.is_empty() {
                println!("  {:w$} ultra is measured, not held to a budget (D428)", "");
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
    tunes: &[String],
) -> Option<Timing> {
    use std::io::{BufRead, BufReader};
    let mut command = Command::new(program);
    command.args(scene.args);
    if let Some(preset) = quality {
        command.args(["--quality", preset]);
    }
    for tune in tunes {
        command.args(["--tune", tune]);
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
#[path = "bench/tests.rs"]
mod bench_tests;
