//! The card's clocks while the probe measures: `nvidia-smi` polled four times a second, so
//! each phase says the clock, performance state and busy share it ran at. A GPU left idle
//! part of each frame lowers its clocks, and the same passes then take longer (play at the
//! lake j12: the passes read long while the card idled at low clocks), so a phase run at
//! lower clocks than another is not comparable with it. NVIDIA's Nsight Graphics locks the
//! clocks to base for its GPU traces for this reason; locking them takes root here
//! (`nvidia-smi --lock-gpu-clocks`), so the probe says the clocks beside its times and
//! warns of a phase that ran clocked down instead.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// What `nvidia-smi` is asked, in this order.
const QUERY: &str = "clocks.gr,clocks.max.gr,pstate,utilization.gpu,temperature.gpu";

/// A phase is clocked down below this share of the fastest phase's clock.
pub const CLOCKED_DOWN: f32 = 0.9;

/// One reading of the card.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub mhz: f32,
    pub max_mhz: f32,
    pub pstate: String,
    /// The share of the last sample period the GPU was busy, 0 to 100.
    pub busy: f32,
    pub celsius: f32,
}

/// One line of `nvidia-smi --query-gpu=<QUERY> --format=csv,noheader,nounits`.
pub fn parse(line: &str) -> Option<Reading> {
    let f: Vec<&str> = line.split(',').map(str::trim).collect();
    let [mhz, max, pstate, busy, celsius] = f[..] else {
        return None;
    };
    Some(Reading {
        mhz: mhz.parse().ok()?,
        max_mhz: max.parse().ok()?,
        pstate: pstate.to_string(),
        busy: busy.parse().ok()?,
        celsius: celsius.parse().ok()?,
    })
}

/// A phase's clocks: medians of its readings, and the performance state most read.
#[derive(Clone, Debug, PartialEq)]
pub struct Clocks {
    pub mhz: f32,
    pub max_mhz: f32,
    pub pstate: String,
    pub busy: f32,
    pub celsius: f32,
    pub readings: usize,
}

fn median(mut v: Vec<f32>) -> f32 {
    v.sort_by(f32::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(f32::NAN)
}

/// The phase's clocks from its readings, None without any.
pub fn summarise(readings: &[Reading]) -> Option<Clocks> {
    if readings.is_empty() {
        return None;
    }
    let mut states: Vec<(&str, usize)> = Vec::new();
    for r in readings {
        match states.iter_mut().find(|(s, _)| *s == r.pstate) {
            Some((_, n)) => *n += 1,
            None => states.push((&r.pstate, 1)),
        }
    }
    let pstate = states.iter().max_by_key(|(_, n)| *n).map(|(s, _)| *s);
    Some(Clocks {
        mhz: median(readings.iter().map(|r| r.mhz).collect()),
        max_mhz: median(readings.iter().map(|r| r.max_mhz).collect()),
        pstate: pstate.unwrap_or("?").to_string(),
        busy: median(readings.iter().map(|r| r.busy).collect()),
        celsius: median(readings.iter().map(|r| r.celsius).collect()),
        readings: readings.len(),
    })
}

impl std::fmt::Display for Clocks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GPU {:.0} of {:.0} MHz {} busy {:.0}% {:.0} C",
            self.mhz, self.max_mhz, self.pstate, self.busy, self.celsius
        )
    }
}

/// A warning for each phase that ran clocked down: under [`CLOCKED_DOWN`] of the fastest
/// phase's clock, or in another performance state than it. Its times read long for the
/// card's sake, not the work's.
pub fn clocked_down(phases: &[(String, Option<Clocks>)]) -> Vec<String> {
    let Some((best_name, best)) = phases
        .iter()
        .filter_map(|(n, c)| Some((n, c.as_ref()?)))
        .max_by(|a, b| a.1.mhz.total_cmp(&b.1.mhz))
    else {
        return Vec::new();
    };
    phases
        .iter()
        .filter_map(|(name, c)| {
            let c = c.as_ref()?;
            let slow = c.mhz < best.mhz * CLOCKED_DOWN;
            let state = c.pstate != best.pstate;
            (slow || state).then(|| {
                format!(
                    "{name} ran clocked down, {:.0} MHz {} against {best_name}'s {:.0} MHz {}: \
                     its times read long for the card's sake, not the work's; run it again \
                     with nothing else on the card, or lock the clocks \
                     (`sudo nvidia-smi --lock-gpu-clocks=<MHz>`, D609)",
                    c.mhz, c.pstate, best.mhz, best.pstate
                )
            })
        })
        .collect()
}

/// `nvidia-smi` polling in the background, its readings kept with when they came.
pub struct Sampler {
    child: Child,
    readings: Arc<Mutex<Vec<(Instant, Reading)>>>,
}

impl Sampler {
    /// Starts polling, or says why it cannot.
    pub fn start() -> Result<Sampler, String> {
        let mut child = Command::new("nvidia-smi")
            .args([
                &format!("--query-gpu={QUERY}"),
                "--format=csv,noheader,nounits",
                "--id=0",
                "-lms",
                "250",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("no nvidia-smi ({e}): the clocks are not known"))?;
        let stdout = child.stdout.take().ok_or("nvidia-smi gave no output")?;
        let readings = Arc::new(Mutex::new(Vec::new()));
        let kept = readings.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let (Some(r), Ok(mut kept)) = (parse(&line), kept.lock()) {
                    kept.push((Instant::now(), r));
                }
            }
        });
        Ok(Sampler { child, readings })
    }

    /// The readings that came between two instants.
    pub fn between(&self, from: Instant, to: Instant) -> Vec<Reading> {
        self.readings.lock().map_or(Vec::new(), |r| {
            r.iter()
                .filter(|(at, _)| *at >= from && *at <= to)
                .map(|(_, r)| r.clone())
                .collect()
        })
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}
