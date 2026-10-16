//! How busy the machine is, for measuring (D185 as amended): a benchmark shares the machine
//! with builds, tests and other agents' runs, and their work makes frames slower and less
//! even. Before each run the task waits, a bounded time, for a quiet machine; during it, it
//! samples the load; and it says which runs were taken on a busy one, so a number is never
//! argued with because of noise nobody saw. `tools/sandbox/load.ts` is the same for the QA
//! traces; the thresholds are the same in both.
//!
//! The processor's busy share is read where it is whole: under WSL from Windows
//! (`typeperf`), whose total counts WSL's own virtual machine and the Windows programs the
//! benchmarks run; on Linux from `/proc/stat` over a second; on macOS from the load average
//! against the cores. The GPU's, where `nvidia-smi` answers.

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Quiet: the processors less than this busy, and the GPU, before a run, less than this. The
/// run itself takes a few of the cores, so a quiet machine is well under half busy.
pub(crate) const QUIET_CPU_PCT: f32 = 35.0;
pub(crate) const QUIET_GPU_PCT: f32 = 15.0;
/// Busy during a run: the processors more than this busy on average, the run's own share
/// counted in (a quiet run of the yard averaged 37%; one at 50% had three times its 99th
/// percentile), or near all of them at once.
pub(crate) const BUSY_DURING_CPU_PCT: f32 = 45.0;
pub(crate) const BUSY_DURING_MOST_PCT: f32 = 90.0;
/// How long to wait for a quiet machine before a run, at most, and how often to look.
pub(crate) const WAIT: Duration = Duration::from_secs(90);
const LOOK_EVERY: Duration = Duration::from_secs(5);

/// The machine's load at a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Load {
    /// The share of every processor busy, 0 to 100.
    pub cpu_pct: f32,
    /// The GPU's busy share, where it can be read.
    pub gpu_pct: Option<f32>,
}

impl Load {
    pub(crate) fn quiet(&self) -> bool {
        self.cpu_pct < QUIET_CPU_PCT && self.gpu_pct.is_none_or(|gpu| gpu < QUIET_GPU_PCT)
    }

    pub(crate) fn describe(&self) -> String {
        match self.gpu_pct {
            Some(gpu) => format!("processors {:.0}% busy, GPU {gpu:.0}%", self.cpu_pct),
            None => format!("processors {:.0}% busy", self.cpu_pct),
        }
    }
}

/// The load now, if it can be read.
pub(crate) fn sample() -> Option<Load> {
    let cpu_pct = if crate::windows::on_wsl() {
        windows_cpu().or_else(linux_cpu)
    } else if cfg!(target_os = "linux") {
        linux_cpu()
    } else {
        load_average()
    }?;
    Some(Load {
        cpu_pct,
        gpu_pct: nvidia_gpu(),
    })
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
                if let (Some(load), Ok(mut kept)) = (sample(), kept.lock()) {
                    kept.push(load);
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

/// Windows' own count of every processor's busy share, WSL's virtual machine in it.
fn windows_cpu() -> Option<f32> {
    let text = quiet_output(
        "typeperf.exe",
        &[r"\Processor(_Total)\% Processor Time", "-sc", "1"],
    )?;
    parse_typeperf(&text)
}

/// Reads typeperf's CSV: a header, then `"time","value"`.
pub(crate) fn parse_typeperf(text: &str) -> Option<f32> {
    text.lines()
        .filter(|line| line.starts_with('"'))
        .nth(1)?
        .rsplit(',')
        .next()?
        .trim()
        .trim_matches('"')
        .parse()
        .ok()
}

/// Linux's processors over a second, from the jiffies `/proc/stat` counts.
fn linux_cpu() -> Option<f32> {
    let read = || -> Option<(u64, u64)> {
        let text = std::fs::read_to_string("/proc/stat").ok()?;
        parse_proc_stat(&text)
    };
    let (busy_a, all_a) = read()?;
    std::thread::sleep(Duration::from_secs(1));
    let (busy_b, all_b) = read()?;
    let all = all_b.saturating_sub(all_a);
    (all > 0).then(|| 100.0 * busy_b.saturating_sub(busy_a) as f32 / all as f32)
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

/// The last minute's load against the cores, as a share: where there is nothing better.
fn load_average() -> Option<f32> {
    let text = quiet_output("sysctl", &["-n", "vm.loadavg"])
        .or_else(|| std::fs::read_to_string("/proc/loadavg").ok())?;
    let load = text
        .split_whitespace()
        .find_map(|word| word.trim_matches('{').parse::<f32>().ok())?;
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    Some((100.0 * load / cores as f32).min(100.0))
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
        assert_eq!(parse_typeperf(text), Some(93.75));
        assert_eq!(parse_typeperf("The command failed"), None);
    }

    #[test]
    fn reads_linux_jiffies_without_idle_or_waiting() {
        // user nice system idle iowait irq softirq steal
        let text = "cpu  60 0 40 800 100 0 0 0 0 0\ncpu0 1 2 3 4\n";
        assert_eq!(parse_proc_stat(text), Some((100, 1000)));
    }

    #[test]
    fn quiet_is_both_processors_and_gpu() {
        let load = |cpu_pct, gpu_pct| Load { cpu_pct, gpu_pct };
        assert!(load(10.0, Some(2.0)).quiet());
        assert!(load(10.0, None).quiet());
        assert!(!load(60.0, Some(2.0)).quiet());
        assert!(!load(10.0, Some(40.0)).quiet());
        let seen = during(&[load(30.0, None), load(90.0, None)]).unwrap();
        assert_eq!((seen.mean_cpu_pct, seen.most_cpu_pct), (60.0, 90.0));
        assert!(seen.busy());
        assert_eq!(during(&[]), None);
    }
}
