//! What a run says of its ticks and its GPU over time (`terra_world::bench_run`): the
//! ticks' 99th percentile against a tick's time, which every scene holds to (D138: a tick
//! slower than its timestep makes each frame run more of them, the death spiral the lake's
//! play fell into on 2026-11-02 with ticks of 35-40 ms against 15.6); and how much its GPU
//! time grew from the run's first third to its last, which a standing scene holds flat (its
//! play phase's `gpu_growth_pct` in the budgets, D607): only a scene whose view stays put can
//! be, since a walk's GPU follows what it looks at.

use crate::scenes::Scene;

/// The scenes a bench runs when none is named: all but those whose GPU is held flat, standing
/// ten minutes each, run only when named (their continuous row, D398).
pub(super) fn every_scene() -> Vec<&'static Scene> {
    crate::scenes::SCENES
        .iter()
        .filter(|s| super::budgets::of(s.name).is_ok_and(|b| b.play.gpu_growth_pct.is_none()))
        .collect()
}

/// The run's ticks and GPU thirds, as its lines said them, None where it did not say.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Said {
    pub tick_p99_ms: Option<f32>,
    pub tick_step_ms: f32,
    pub gpu_growth_pct: Option<f32>,
}

impl Said {
    /// Reads one of the two lines into what it says; false for any other line.
    pub(super) fn read(&mut self, line: &str) -> bool {
        if let Some((p99, step)) = parse_ticks(line) {
            (self.tick_p99_ms, self.tick_step_ms) = (Some(p99), step);
            true
        } else if let Some(growth) = parse_thirds(line) {
            self.gpu_growth_pct = Some(growth);
            true
        } else {
            false
        }
    }

    /// What the run missed: ticks whose 99th percentile is over the most a tick may take
    /// (D138: its timestep); and for a scene held flat, its GPU grown past the most it may,
    /// or not said.
    pub(super) fn misses(
        &self,
        tick_most_ms: Option<f32>,
        growth_most_pct: Option<f32>,
    ) -> Vec<String> {
        let mut missed = Vec::new();
        if let (Some(p99), Some(most)) = (self.tick_p99_ms, tick_most_ms)
            && p99 > most
        {
            missed.push(format!(
                "ticks' 99th percentile {p99:.1} ms over the {most:.1} ms of a tick (D138, {}): \
                 the fixed loop's systems, by `cargo xtask profile`, are what each tick runs",
                super::budgets::FILE
            ));
        }
        match (self.gpu_growth_pct, growth_most_pct) {
            (_, None) => {}
            (None, Some(most)) => missed.push(format!(
                "the GPU's thirds not said, where it is held within +{most:.0}%: no `SANDBOX GPU by \
                 thirds` line in the run's output (terra_world::bench_run says it)"
            )),
            (Some(growth), Some(most)) if growth > most => missed.push(format!(
                "GPU {growth:+.1}% from the first third to the last, over the +{most:.0}% a \
                 standing scene is held to: read the GPU minute by minute in the run's `Last \
                 minute` log lines against the card's clocks (`nvidia-smi --query-gpu=clocks.gr,\
                 pstate --format=csv -l 5`): GPU time up with the clocks down is the card idling, \
                 at the same clocks it is work that grew"
            )),
            _ => {}
        }
        missed
    }

    /// Several runs as one: the middle run's of each.
    pub(super) fn middle<'a>(runs: impl Iterator<Item = &'a Said>) -> Said {
        let runs: Vec<&Said> = runs.collect();
        let middle = |pick: fn(&Said) -> Option<f32>| {
            let mut values: Vec<f32> = runs.iter().filter_map(|s| pick(s)).collect();
            values.sort_by(f32::total_cmp);
            values.get(values.len() / 2).copied()
        };
        Said {
            tick_p99_ms: middle(|s| s.tick_p99_ms),
            tick_step_ms: runs.first().map_or(0.0, |s| s.tick_step_ms),
            gpu_growth_pct: middle(|s| s.gpu_growth_pct),
        }
    }
}

/// Reads `SANDBOX ticks, in ms a tick: median 2.10, 99th percentile 4.00, worst 9.00, of 1800
/// frames' ticks, against the 15.6 ms of a tick` as the 99th percentile and a tick's time.
fn parse_ticks(line: &str) -> Option<(f32, f32)> {
    let rest = line.strip_prefix("SANDBOX ticks, in ms a tick: ")?;
    let (_, rest) = rest.split_once("99th percentile ")?;
    let (p99, rest) = rest.split_once(',')?;
    let (_, step) = rest.split_once("against the ")?;
    let step = step.strip_suffix(" ms of a tick")?;
    Some((p99.parse().ok()?, step.parse().ok()?))
}

/// Reads `SANDBOX GPU by thirds of the run, medians in ms: 10.00 11.00 12.00; the last third
/// +20.0% on the first` as the growth.
fn parse_thirds(line: &str) -> Option<f32> {
    let rest = line.strip_prefix("SANDBOX GPU by thirds of the run, ")?;
    let (_, growth) = rest.split_once("the last third ")?;
    growth.strip_suffix("% on the first")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_is_held_to_ticks_within_their_time() {
        let mut said = Said::default();
        assert!(said.read(
            "SANDBOX ticks, in ms a tick: median 2.10, 99th percentile 16.20, worst 30.00, of 1800 frames' ticks, against the 15.6 ms of a tick"
        ));
        assert!(said.read(
            "SANDBOX GPU by thirds of the run, medians in ms: 10.00 11.00 12.00; the last third +20.0% on the first"
        ));
        assert!(!said.read("SANDBOX frames on X at 1x1: median 1.00 ms"));
        assert_eq!(said.tick_p99_ms, Some(16.2));
        assert_eq!(said.gpu_growth_pct, Some(20.0));
        let tick = Some(15.6);
        assert!(said.misses(tick, None)[0].contains("16.2 ms over the 15.6 ms"));
        said.tick_p99_ms = Some(15.0);
        assert!(said.misses(tick, None).is_empty());
        // A run that did not say (a sandbox before the ticks were timed) misses nothing.
        assert!(Said::default().misses(tick, None).is_empty());
    }

    #[test]
    fn a_standing_scene_misses_with_its_gpu_grown_and_a_walk_never() {
        let run = |growth| Said {
            tick_p99_ms: Some(5.0),
            tick_step_ms: 15.6,
            gpu_growth_pct: growth,
        };
        let flat = |growth, most| run(growth).misses(Some(15.6), most);
        // The developer's lake (+100% by the fourth minute) against the stand's 5%.
        assert!(flat(Some(100.0), Some(5.0))[0].contains("+100.0% from the first"));
        assert!(flat(Some(0.7), Some(5.0)).is_empty());
        // Faster by the end is no growth.
        assert!(flat(Some(-12.0), Some(5.0)).is_empty());
        // Held flat, the run must say its thirds.
        assert!(flat(None, Some(5.0))[0].contains("not said"));
        // A walk's GPU follows its view, and is held to nothing.
        assert!(flat(Some(48.2), None).is_empty());
    }

    #[test]
    fn every_scene_but_those_held_flat_runs_when_none_is_named() {
        let every = every_scene();
        assert!(every.iter().all(|s| s.name != "lake-stand"));
        assert!(every.iter().any(|s| s.name == "lake-play"));
        assert!(every.len() < crate::scenes::SCENES.len());
    }

    #[test]
    fn several_runs_take_the_middle_one() {
        let run = |p99| Said {
            tick_p99_ms: Some(p99),
            tick_step_ms: 15.6,
            gpu_growth_pct: None,
        };
        let runs = [run(9.0), run(20.0), run(10.0)];
        let middle = Said::middle(runs.iter());
        assert_eq!(middle.tick_p99_ms, Some(10.0));
        assert_eq!(middle.gpu_growth_pct, None);
    }
}
