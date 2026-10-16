//! `cargo xtask bench` (D185): the scenes timed in release, offscreen at 1920x1080 on the
//! real GPU, each judged against the frame budget and the last recorded result.

use crate::scenes::{SCENES, Scene};
use crate::windows::{on_wsl, windows_program};
use crate::{output, root, run};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
}

impl Timing {
    /// What the scene missed of the budget's gates, if anything.
    fn misses(&self) -> Vec<String> {
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
pub(crate) fn bench(args: &[String]) -> bool {
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
            "  The budget is {:.1} ms (D25: 75 fps on the developers' RTX 3080 at the high preset): \
             a scene holds to it with its 99th percentile under it, at most {MOST_OVER_PCT}% of \
             frames over, no hitch over twice it, and no more than {MOST_DRIFT_PCT}% slower by \
             the end, on the machine it runs on.",
            first.budget_ms
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
}
