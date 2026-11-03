//! How busy the machine is, for measuring (D185 as amended): a benchmark shares the machine
//! with builds, tests and other agents' runs, and their work makes frames slower and less
//! even. Before each run the task waits, a bounded time, for a quiet machine; during it, it
//! samples the load; and it says which runs were taken on a busy one, so a number is never
//! argued with because of noise nobody saw. `tools/sandbox/load.ts` is the same for the QA
//! traces; the thresholds are the same in both, held so by a test here.
//!
//! The processor's busy share is read where it is whole: under WSL from Windows
//! (`typeperf`), whose total counts WSL's own virtual machine and the Windows programs the
//! benchmarks run; on Linux from `/proc/stat` over a second; on macOS from the load average
//! against the cores. While a run goes, the sandbox's own share is taken out (its program,
//! `terra_sandbox`, as Windows or `/proc` counts it), so what is left is everything else's;
//! a benchmark has the GPU to itself (D319), so no other sandbox runs then. The GPU's, where
//! `nvidia-smi` answers.

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Quiet: the processors less than this busy, and the GPU, before a run, less than this. The
/// run itself takes a few of the cores, so a quiet machine is well under half busy.
pub(crate) const QUIET_CPU_PCT: f32 = 35.0;
pub(crate) const QUIET_GPU_PCT: f32 = 15.0;
/// Quiet: the last minute's load average, against the logical processors, less than this. It
/// sees what a second of `/proc/stat` misses: threads queued for a core and tasks stalled on
/// the disk (a build swapping), and a build that paused between its crates (the worker's
/// "processors 2% busy" samples on a box at load 40, perf round 1).
pub(crate) const QUIET_LOAD_PCT: f32 = 50.0;
/// Busy during a run: everything but the run itself more than this busy on average, or more
/// than the second at once (a run of the yard with the processors half busy beside it had
/// three times its 99th percentile).
pub(crate) const BUSY_DURING_CPU_PCT: f32 = 30.0;
pub(crate) const BUSY_DURING_MOST_PCT: f32 = 70.0;
/// The program whose share a run's load leaves out.
const SANDBOX: &str = "terra_sandbox";
/// How long to wait for a quiet machine before a run, at most, and how often to look.
pub(crate) const WAIT: Duration = Duration::from_secs(90);
const LOOK_EVERY: Duration = Duration::from_secs(5);
/// The most a command waits for a quiet machine over all its runs, so a machine that never
/// quiets costs minutes, not hours: after it, runs go ahead and say they were busy.
pub(crate) const MOST_WAITING: Duration = Duration::from_secs(300);
/// The QA traces' side of this module, which must judge a machine quiet or busy as this one
/// does: `the_traces_judge_load_as_benchmarks_do` holds its numbers to these.
#[cfg(test)]
const TRACES: &str = "tools/sandbox/load.ts";

/// The machine's load at a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Load {
    /// The share of every processor busy, 0 to 100.
    pub cpu_pct: f32,
    /// The GPU's busy share, where it can be read.
    pub gpu_pct: Option<f32>,
    /// The last minute's load average against the logical processors, where it can be read.
    pub load_pct: Option<f32>,
}

impl Load {
    pub(crate) fn quiet(&self) -> bool {
        self.cpu_pct < QUIET_CPU_PCT
            && self.gpu_pct.is_none_or(|gpu| gpu < QUIET_GPU_PCT)
            && self.load_pct.is_none_or(|load| load < QUIET_LOAD_PCT)
    }

    pub(crate) fn describe(&self) -> String {
        let mut said = format!("processors {:.0}% busy", self.cpu_pct);
        if let Some(gpu) = self.gpu_pct {
            said += &format!(", GPU {gpu:.0}%");
        }
        if let Some(load) = self.load_pct {
            said += &format!(", load average {load:.0}% of the processors");
        }
        said
    }
}

/// The load now, if it can be read: everything on the machine.
pub(crate) fn sample() -> Option<Load> {
    Some(Load {
        cpu_pct: cpu().map(|(all, _)| all)?,
        gpu_pct: nvidia_gpu(),
        load_pct: if crate::windows::on_wsl() {
            // WSL's load average is its virtual machine's alone; Windows' total has the rest.
            None
        } else {
            load_average()
        },
    })
}

/// The processors' busy share now, and the sandbox's own part of it, where it can be told.
fn cpu() -> Option<(f32, f32)> {
    if crate::windows::on_wsl() {
        windows_cpu().or_else(linux_cpu)
    } else if cfg!(target_os = "linux") {
        linux_cpu()
    } else {
        load_average().map(|all| (all.min(100.0), 0.0))
    }
}

/// Waits up to `most` for a quiet machine, looking every few seconds; says the load it ran
/// on and how long it waited.
pub(crate) fn wait_for_quiet(most: Duration) -> (Option<Load>, Duration) {
    let started = Instant::now();
    loop {
        let load = sample();
        let quiet = load.is_none_or(|load| load.quiet());
        if quiet || started.elapsed() + LOOK_EVERY > most {
            return (load, started.elapsed());
        }
        std::thread::sleep(LOOK_EVERY);
    }
}

/// The load sampled every few seconds while something runs, and the processors' clock every
/// `CLOCK_EVERY`.
pub(crate) struct Watch {
    stop: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<Load>>>,
    clocks: Arc<Mutex<Vec<(f32, f32)>>>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

/// What the load was during a run: its mean and its most; and the processors' clock, where
/// it can be read.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct During {
    pub mean_cpu_pct: f32,
    pub most_cpu_pct: f32,
    pub clock: Option<Clock>,
}

impl During {
    pub(crate) fn busy(&self) -> bool {
        self.mean_cpu_pct > BUSY_DURING_CPU_PCT || self.most_cpu_pct > BUSY_DURING_MOST_PCT
    }

    /// How many times the machine's clock collapsed from `from_s` seconds into the run on (a
    /// play scene's collapses while it loads slow no frame it is judged on).
    pub(crate) fn collapses_from(&self, from_s: f32) -> usize {
        self.clock.as_ref().map_or(0, |clock| {
            clock.at_s.iter().filter(|&&at| at >= from_s).count()
        })
    }
}

/// The watched processors' clock is read this often: the render worker's collapses fall in
/// one 20 ms step (perf-hitch j1).
const CLOCK_EVERY: Duration = Duration::from_millis(20);
/// A collapse (D185): the watched processors' mean clock under this share of the run's
/// median clock. The render worker's i9-14900K fell from 5.0-5.7 GHz to 0.8-1.4 GHz and
/// climbed back over ~600 ms, every frame of it slow; that is no work of the game's, as
/// Chromium's perf bots flag frequency throttling and Google Benchmark warns of CPU scaling.
pub(crate) const COLLAPSE_SHARE: f32 = 0.4;
/// A collapse is a fall, not a wind-down: within the `SUDDEN` readings before it the clock was
/// at least this share of the median (the worker's fell in one reading).
const SUDDEN_SHARE: f32 = 0.8;
const SUDDEN: usize = 5;

/// The watched processors' clock over a run: its median, when it collapsed (seconds from the
/// run's start) and the lowest it fell to in them (the lowest of all where it never did).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Clock {
    pub median_mhz: f32,
    pub lowest_mhz: f32,
    pub at_s: Vec<f32>,
}

/// The clock of a run from its readings, each its second from the run's start and its MHz,
/// one every `CLOCK_EVERY`; none for too few.
pub(crate) fn clock(series: &[(f32, f32)]) -> Option<Clock> {
    if series.len() < SUDDEN * 2 {
        return None;
    }
    let mut sorted: Vec<f32> = series.iter().map(|&(_, mhz)| mhz).collect();
    sorted.sort_by(f32::total_cmp);
    let median_mhz = sorted[sorted.len() / 2];
    if median_mhz <= 0.0 {
        return None;
    }
    let (low, high) = (median_mhz * COLLAPSE_SHARE, median_mhz * SUDDEN_SHARE);
    let (mut at_s, mut lowest, mut down) = (Vec::new(), f32::MAX, false);
    for (at, &(second, mhz)) in series.iter().enumerate() {
        if mhz >= low {
            down = false;
            continue;
        }
        let before = &series[at.saturating_sub(SUDDEN)..at];
        if !down && before.iter().any(|&(_, mhz)| mhz >= high) {
            at_s.push(second);
            down = true;
        }
        if down {
            lowest = lowest.min(mhz);
        }
    }
    Some(Clock {
        median_mhz,
        lowest_mhz: if at_s.is_empty() { sorted[0] } else { lowest },
        at_s,
    })
}

/// The processors listed as `0-3,8,10-11` (`Cpus_allowed_list`, `/sys/.../cpu/online`).
pub(crate) fn parse_cpu_list(text: &str) -> Vec<usize> {
    let mut cpus = Vec::new();
    for part in text.trim().split(',').filter(|part| !part.is_empty()) {
        let (from, to) = part.split_once('-').unwrap_or((part, part));
        if let (Ok(from), Ok(to)) = (from.trim().parse(), to.trim().parse::<usize>()) {
            cpus.extend(from..=to);
        }
    }
    cpus
}

/// The `scaling_cur_freq` files of the processors a run goes on: those this task may use (its
/// runs inherit them), of the fastest kind where a hybrid chip has two (the worker's P-cores
/// top out at 5.7-6.0 GHz, its E-cores at 4.4, whose clock would hide a P-cores' collapse in
/// the mean). Linux only; none under WSL, whose virtual processors have no cpufreq.
fn watched_cpus() -> Vec<String> {
    let allowed = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("Cpus_allowed_list:").map(parse_cpu_list))
        })
        .unwrap_or_default();
    let top = |cpu: usize| -> Option<f32> {
        let path = format!("/sys/devices/system/cpu/cpu{cpu}/cpufreq/cpuinfo_max_freq");
        std::fs::read_to_string(path).ok()?.trim().parse().ok()
    };
    let tops: Vec<(usize, f32)> = allowed
        .into_iter()
        .filter_map(|cpu| Some((cpu, top(cpu)?)))
        .collect();
    let fastest = tops.iter().map(|&(_, top)| top).fold(0.0, f32::max);
    tops.into_iter()
        .filter(|&(_, top)| top >= fastest * 0.9)
        .map(|(cpu, _)| format!("/sys/devices/system/cpu/cpu{cpu}/cpufreq/scaling_cur_freq"))
        .collect()
}

/// The watched processors' mean clock now, in MHz.
fn mean_clock(files: &[String]) -> Option<f32> {
    let mut khz = 0.0;
    for file in files {
        khz += std::fs::read_to_string(file)
            .ok()?
            .trim()
            .parse::<f32>()
            .ok()?;
    }
    (!files.is_empty()).then(|| khz / files.len() as f32 / 1000.0)
}

impl Watch {
    pub(crate) fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let samples = Arc::new(Mutex::new(Vec::new()));
        let clocks = Arc::new(Mutex::new(Vec::new()));
        let mut threads = Vec::new();
        let files = watched_cpus();
        if !files.is_empty() {
            let (stopping, kept) = (stop.clone(), clocks.clone());
            let started = Instant::now();
            threads.push(std::thread::spawn(move || {
                while !stopping.load(Ordering::Relaxed) {
                    if let (Some(mhz), Ok(mut kept)) = (mean_clock(&files), kept.lock()) {
                        kept.push((started.elapsed().as_secs_f32(), mhz));
                    }
                    std::thread::sleep(CLOCK_EVERY);
                }
            }));
        }
        let (stopping, kept) = (stop.clone(), samples.clone());
        let thread = std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                // Everything but the run itself.
                if let (Some((all, ours)), Ok(mut kept)) = (cpu(), kept.lock()) {
                    kept.push(Load {
                        cpu_pct: (all - ours).max(0.0),
                        gpu_pct: None,
                        load_pct: None,
                    });
                }
                // In short steps, so stopping does not wait out a whole interval.
                for _ in 0..10 {
                    if stopping.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
        });
        threads.push(thread);
        Self {
            stop,
            samples,
            clocks,
            threads,
        }
    }

    pub(crate) fn stop(self) -> Option<During> {
        self.stop.store(true, Ordering::Relaxed);
        for thread in self.threads {
            thread.join().ok();
        }
        let samples = self.samples.lock().ok()?;
        let mut seen = during(&samples)?;
        seen.clock = self.clocks.lock().ok().and_then(|series| clock(&series));
        Some(seen)
    }
}

/// The mean and most of some samples.
pub(crate) fn during(samples: &[Load]) -> Option<During> {
    if samples.is_empty() {
        return None;
    }
    let cpu: Vec<f32> = samples.iter().map(|load| load.cpu_pct).collect();
    Some(During {
        mean_cpu_pct: cpu.iter().sum::<f32>() / cpu.len() as f32,
        most_cpu_pct: cpu.iter().copied().fold(0.0, f32::max),
        clock: None,
    })
}

fn quiet_output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

/// Windows' own count of every processor's busy share, WSL's virtual machine in it, and the
/// sandbox programs' share of it.
fn windows_cpu() -> Option<(f32, f32)> {
    let ours = format!(r"\Process({SANDBOX}*)\% Processor Time");
    let text = quiet_output(
        "typeperf.exe",
        &[r"\Processor(_Total)\% Processor Time", &ours, "-sc", "1"],
    )?;
    let values = parse_typeperf(&text);
    let (all, programs) = values.split_first()?;
    // A process's time is counted a core at a time: 100% is one core.
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get()) as f32;
    Some((*all, programs.iter().sum::<f32>() / cores))
}

/// Reads typeperf's CSV, a header and then `"time","value",...`, as its values.
pub(crate) fn parse_typeperf(text: &str) -> Vec<f32> {
    text.lines()
        .filter(|line| line.starts_with('"'))
        .nth(1)
        .map(|line| {
            line.split(',')
                .skip(1)
                .filter_map(|cell| cell.trim().trim_matches('"').parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Linux's processors over a second, from the jiffies `/proc/stat` counts, and the sandbox
/// processes' part, from theirs.
fn linux_cpu() -> Option<(f32, f32)> {
    let read = || -> Option<(u64, u64)> {
        let text = std::fs::read_to_string("/proc/stat").ok()?;
        parse_proc_stat(&text)
    };
    let (busy_a, all_a) = read()?;
    let ours_a = sandbox_jiffies();
    std::thread::sleep(Duration::from_secs(1));
    let (busy_b, all_b) = read()?;
    let ours_b = sandbox_jiffies();
    let all = all_b.saturating_sub(all_a);
    (all > 0).then(|| {
        (
            100.0 * busy_b.saturating_sub(busy_a) as f32 / all as f32,
            100.0 * ours_b.saturating_sub(ours_a) as f32 / all as f32,
        )
    })
}

/// The jiffies every process called `terra_sandbox` has run, user and system.
fn sandbox_jiffies() -> u64 {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .bytes()
                .all(|b| b.is_ascii_digit())
        })
        .filter(|entry| {
            std::fs::read_to_string(entry.path().join("comm"))
                .is_ok_and(|name| name.trim().starts_with(SANDBOX))
        })
        .filter_map(|entry| std::fs::read_to_string(entry.path().join("stat")).ok())
        .filter_map(|stat| {
            // After the name in brackets: its state, then fields 4 to 13, then utime and stime.
            let rest = stat.rsplit_once(')')?.1;
            let fields: Vec<u64> = rest
                .split_whitespace()
                .skip(1)
                .map(|f| f.parse().unwrap_or(0))
                .collect();
            Some(fields.get(10)? + fields.get(11)?)
        })
        .sum()
}

/// The first line of `/proc/stat` as busy and all jiffies: idle and waiting on the disk are
/// not busy.
pub(crate) fn parse_proc_stat(text: &str) -> Option<(u64, u64)> {
    let line = text.lines().find(|line| line.starts_with("cpu "))?;
    let numbers: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|n| n.parse().ok())
        .collect();
    let all: u64 = numbers.iter().take(8).sum();
    let idle = numbers.get(3)? + numbers.get(4).copied().unwrap_or(0);
    Some((all - idle, all))
}

/// The last minute's load against the logical processors, as a share (over 100 when more
/// wait than there are processors): `/proc/loadavg` on Linux, `sysctl` on macOS.
fn load_average() -> Option<f32> {
    let text = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .or_else(|| quiet_output("sysctl", &["-n", "vm.loadavg"]))?;
    parse_loadavg(
        &text,
        std::thread::available_parallelism().map_or(1, |n| n.get()),
    )
}

/// The first number of `/proc/loadavg` or macOS's `{ 1.50 1.20 1.00 }`, against `cores`.
pub(crate) fn parse_loadavg(text: &str, cores: usize) -> Option<f32> {
    let load = text
        .split_whitespace()
        .find_map(|word| word.trim_matches('{').parse::<f32>().ok())?;
    Some(100.0 * load / cores.max(1) as f32)
}

/// The busiest NVIDIA GPU's share, if `nvidia-smi` answers.
fn nvidia_gpu() -> Option<f32> {
    let text = quiet_output(
        "nvidia-smi",
        &[
            "--query-gpu=utilization.gpu",
            "--format=csv,noheader,nounits",
        ],
    )?;
    text.lines()
        .filter_map(|line| line.trim().parse::<f32>().ok())
        .reduce(f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_windows_processor_time() {
        let text = "\"(PDH-CSV 4.0)\",\"\\\\BOX\\Processor(_Total)\\% Processor Time\"\n\
                    \"10/16/2026 11:05:03.021\",\"93.75\"\nExiting, please wait...\n";
        assert_eq!(parse_typeperf(text), vec![93.75]);
        assert!(parse_typeperf("The command failed").is_empty());
        // With the sandbox's process beside the total, its share counted a core at a time.
        let both = "\"(PDH-CSV 4.0)\",\"total\",\"sandbox\"\n\"10/16/2026\",\"40.0\",\"320.0\"\n";
        assert_eq!(parse_typeperf(both), vec![40.0, 320.0]);
    }

    #[test]
    fn reads_linux_jiffies_without_idle_or_waiting() {
        // user nice system idle iowait irq softirq steal
        let text = "cpu  60 0 40 800 100 0 0 0 0 0\ncpu0 1 2 3 4\n";
        assert_eq!(parse_proc_stat(text), Some((100, 1000)));
    }

    #[test]
    fn quiet_is_both_processors_and_gpu() {
        let load = |cpu_pct, gpu_pct| Load {
            cpu_pct,
            gpu_pct,
            load_pct: None,
        };
        assert!(load(10.0, Some(2.0)).quiet());
        // A second of idle processors on a box whose run queue is longer than its cores.
        let queued = Load {
            load_pct: Some(125.0),
            ..load(2.0, Some(0.0))
        };
        assert!(!queued.quiet());
        assert!(
            Load {
                load_pct: Some(10.0),
                ..queued
            }
            .quiet()
        );
        assert_eq!(
            parse_loadavg("40.00 38.50 30.01 3/2100 999\n", 32),
            Some(125.0)
        );
        assert_eq!(parse_loadavg("{ 1.50 1.20 1.00 }\n", 4), Some(37.5));
        assert!(load(10.0, None).quiet());
        assert!(!load(60.0, Some(2.0)).quiet());
        assert!(!load(10.0, Some(40.0)).quiet());
        let seen = during(&[load(30.0, None), load(90.0, None)]).unwrap();
        assert_eq!((seen.mean_cpu_pct, seen.most_cpu_pct), (60.0, 90.0));
        assert!(seen.busy());
        assert_eq!(during(&[]), None);
    }

    /// The worker's collapse as the probe saw it (perf-hitch j1): 5.0-5.7 GHz, down to
    /// ~1 GHz in one reading, back up over ~600 ms; twice in a run. A wind-down to idle and a
    /// dip to 60% are no collapse.
    #[test]
    fn a_collapse_is_a_sudden_fall_under_two_fifths_of_the_median() {
        let steady =
            |n: usize| -> Vec<f32> { (0..n).map(|i| 5000.0 + (i % 7) as f32 * 100.0).collect() };
        let ramp: Vec<f32> = (0..30).map(|i| 900.0 + i as f32 * 140.0).collect();
        // Each reading's second, one every 20 ms.
        let timed = |mhz: Vec<f32>| -> Vec<(f32, f32)> {
            mhz.into_iter()
                .enumerate()
                .map(|(i, mhz)| (i as f32 * 0.02, mhz))
                .collect()
        };
        let mut series = steady(200);
        series.extend(&ramp);
        series.extend(steady(200));
        series.push(1300.0);
        series.extend(&ramp[3..]);
        series.extend(steady(100));
        let seen = clock(&timed(series)).unwrap();
        assert_eq!(seen.at_s.len(), 2);
        assert!((seen.at_s[0] - 4.0).abs() < 1e-3 && (seen.at_s[1] - 8.6).abs() < 1e-3);
        assert_eq!(seen.lowest_mhz, 900.0);
        assert!((5000.0..=5600.0).contains(&seen.median_mhz));
        // The run's collapses from a second on: one after its load of 6 s, none after 9 s.
        let during = During {
            mean_cpu_pct: 2.0,
            most_cpu_pct: 3.0,
            clock: Some(seen),
        };
        assert_eq!(
            [0.0, 6.0, 9.0].map(|from| during.collapses_from(from)),
            [2, 1, 0]
        );

        // Nothing but steady, and a dip to 60% of it: none, the lowest of all said.
        let mut dipped = steady(300);
        dipped.extend([3100.0; 20]);
        dipped.extend(steady(100));
        let seen = clock(&timed(dipped)).unwrap();
        assert_eq!((seen.at_s.len(), seen.lowest_mhz), (0, 3100.0));

        // A slow wind-down (the last readings as the program exits and the cores idle).
        let mut idle = steady(300);
        idle.extend((0..40).map(|i| 5000.0 - i as f32 * 110.0));
        assert!(clock(&timed(idle)).unwrap().at_s.is_empty());

        // Too few readings to say, or none.
        assert_eq!(clock(&timed(vec![5000.0, 900.0])), None);
        assert_eq!(clock(&[]), None);
    }

    #[test]
    fn reads_cpu_lists() {
        assert_eq!(parse_cpu_list("0-3,8,10-11\n"), vec![0, 1, 2, 3, 8, 10, 11]);
        assert_eq!(parse_cpu_list("\t0-31").len(), 32);
        assert!(parse_cpu_list("").is_empty());
    }

    /// The QA traces wait for and judge a quiet machine by the same numbers as the
    /// benchmarks, so a trace and a benchmark taken side by side agree on which ran busy.
    #[test]
    fn the_traces_judge_load_as_benchmarks_do() {
        let text = std::fs::read_to_string(crate::root().join(TRACES)).expect(TRACES);
        let value = |name: &str| -> String {
            let line = text
                .lines()
                .find_map(|line| {
                    line.trim_start()
                        .strip_prefix("export ")
                        .unwrap_or(line.trim_start())
                        .strip_prefix(&format!("const {name} = "))
                })
                .unwrap_or_else(|| panic!("{TRACES} has no const {name}"));
            line.trim_end_matches(';')
                .trim()
                .trim_matches('"')
                .to_string()
        };
        let number = |name: &str| -> f32 {
            value(name)
                .parse()
                .unwrap_or_else(|_| panic!("{TRACES}'s {name} is not a number"))
        };
        assert_eq!(number("QUIET_CPU_PCT"), QUIET_CPU_PCT);
        assert_eq!(number("QUIET_GPU_PCT"), QUIET_GPU_PCT);
        assert_eq!(number("QUIET_LOAD_PCT"), QUIET_LOAD_PCT);
        assert_eq!(number("BUSY_DURING_CPU_PCT"), BUSY_DURING_CPU_PCT);
        assert_eq!(number("BUSY_DURING_MOST_PCT"), BUSY_DURING_MOST_PCT);
        assert_eq!(number("WAIT_S"), WAIT.as_secs_f32());
        assert_eq!(number("MOST_WAITING_S"), MOST_WAITING.as_secs_f32());
        assert_eq!(value("SANDBOX"), SANDBOX);
    }
}
