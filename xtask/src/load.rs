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

/// The load sampled every few seconds while something runs.
pub(crate) struct Watch {
    stop: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<Load>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// What the load was during a run: its mean and its most.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct During {
    pub mean_cpu_pct: f32,
    pub most_cpu_pct: f32,
}

impl During {
    pub(crate) fn busy(&self) -> bool {
        self.mean_cpu_pct > BUSY_DURING_CPU_PCT || self.most_cpu_pct > BUSY_DURING_MOST_PCT
    }
}

impl Watch {
    pub(crate) fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let samples = Arc::new(Mutex::new(Vec::new()));
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
        Self {
            stop,
            samples,
            thread: Some(thread),
        }
    }

    pub(crate) fn stop(mut self) -> Option<During> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
        let samples = self.samples.lock().ok()?;
        during(&samples)
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
