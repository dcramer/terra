//! A scene's runs and which of them its result is taken from (D185): those on a quiet
//! machine, a run that hitched while the machine's CPU clock collapsed counted as busy.

use super::Timing;
use crate::load::{During, Load};

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

    /// It hitched, settling or playing, and the machine's clock collapsed after it loaded.
    fn clock_hitched(&self) -> bool {
        self.timing.all_hitches() > 0 && self.collapses() > 0
    }

    /// How many times the machine's clock collapsed after the run loaded (a scene timed from
    /// the program's start, D578, loads for its `load_s`; the rest from their start).
    fn collapses(&self) -> usize {
        let from_s = self.timing.load_s.unwrap_or(0.0);
        self.during.as_ref().map_or(0, |d| d.collapses_from(from_s))
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
        let verdict = if self.clock_hitched() {
            ": its hitches are the machine's, so the run counts as busy and another stands in"
        } else if self.collapses() == 0 {
            ", all while it loaded"
        } else {
            ""
        };
        Some(format!(
            "the box's CPU clock fell to {:.0} MHz {times} time{} from its {:.0} MHz, at {} s (its power management, not the game){verdict}",
            clock.lowest_mhz,
            if times == 1 { "" } else { "s" },
            clock.median_mhz,
            at.join(", "),
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
