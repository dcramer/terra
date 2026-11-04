//! A scene's runs and which of them its result is taken from (D185): those on a quiet
//! machine, a run whose every hitch fell in a collapse of the machine's CPU clock counted as
//! busy.

use super::Timing;
use crate::load::{During, Load};

/// A hitch is the machine's when it ended from this long before a collapse's fall to this
/// long after: the worker's slow frames ran ~600 ms as its clock climbed back (perf-hitch j1).
const BEFORE_FALL_S: f64 = 0.1;
const AFTER_FALL_S: f64 = 1.0;

/// The sandbox's `SANDBOX hitches ended at, seconds since the Unix epoch: 1762.10 1763.20`
/// (`none` for none; `terra_world::bench_run::hitches_line`).
pub(super) fn parse_hitch_times(line: &str) -> Option<Vec<f64>> {
    let rest = line.strip_prefix("SANDBOX hitches ended at, seconds since the Unix epoch: ")?;
    if rest.trim() == "none" {
        return Some(Vec::new());
    }
    rest.split_whitespace().map(|s| s.parse().ok()).collect()
}

/// One run of a scene: its frames, the load before it and how long it waited for a quiet
/// machine, and the load while it ran.
#[derive(Debug, Clone)]
pub(super) struct Run {
    pub timing: Timing,
    pub before: Option<Load>,
    pub during: Option<During>,
}

impl Run {
    /// Taken on a busy machine: it started busy, or was busy while it ran, or it hitched and
    /// the machine's clock collapsed meanwhile (D185), whose slow frames are the machine's.
    pub(super) fn busy(&self) -> bool {
        self.before.is_some_and(|load| !load.quiet())
            || self.during.as_ref().is_some_and(|d| d.busy())
            || self.clock_hitched()
    }

    /// It hitched, settling or playing, and every hitch fell in a collapse of the machine's
    /// clock: none is the game's.
    fn clock_hitched(&self) -> bool {
        self.hitches_split()
            .is_some_and(|(machine, game)| machine > 0 && game == 0)
    }

    /// Its hitches that fell in a collapse of the machine's clock, and those that did not;
    /// none where the clock was not read or the run did not say when its hitches fell.
    pub(super) fn hitches_split(&self) -> Option<(usize, usize)> {
        let clock = self.during.as_ref()?.clock.as_ref()?;
        let at = self.timing.hitch_at.as_ref()?;
        let machine = at
            .iter()
            .filter(|&&hitch| {
                clock.at_s.iter().any(|&fall| {
                    let fall = clock.start_epoch_s + f64::from(fall);
                    (fall - BEFORE_FALL_S..=fall + AFTER_FALL_S).contains(&hitch)
                })
            })
            .count();
        Some((machine, at.len() - machine))
    }

    /// What its line says of the machine's clock, where it collapsed (D556: whose the slow
    /// frames are, and what the bench did about them).
    pub(super) fn clock_said(&self) -> Option<String> {
        let clock = self.during.as_ref()?.clock.as_ref()?;
        let times = clock.at_s.len();
        if times == 0 {
            return None;
        }
        let at: Vec<String> = clock.at_s.iter().map(|s| format!("{s:.1}")).collect();
        let verdict = match self.hitches_split() {
            Some((0, 0)) => String::new(),
            Some((machine, 0)) => format!(
                "; its {machine} hitches fell in them, the machine's, so the run counts as busy and another stands in"
            ),
            Some((0, game)) => format!("; none of its {game} hitches fell in them: the game's"),
            Some((machine, game)) => format!(
                "; {machine} of its hitches fell in them, {game} outside them are the game's"
            ),
            None if self.timing.all_hitches() > 0 => {
                "; the run did not say when its hitches fell, so they count as the game's"
                    .to_string()
            }
            None => String::new(),
        };
        Some(format!(
            "the box's CPU clock fell to {:.0} MHz {times} time{} from its {:.0} MHz, at {} s ({}){verdict}",
            clock.lowest_mhz,
            if times == 1 { "" } else { "s" },
            clock.median_mhz,
            at.join(", "),
            clock.cause(),
        ))
    }
}

/// The runs a scene's result is taken from: the quiet ones when at least half are, so noise
/// nobody chose is left out; otherwise all of them, which the table says were busy.
pub(super) fn chosen_runs(runs: &[Run]) -> (Vec<Timing>, bool) {
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
