//! The sheet's numbers: many seeds of a theme generated, timed and checked, in a line (D388).

use crate::dress::ModuleSet;
use crate::theme::{Space, Theme};
use crate::{Dungeon, dress, graph, place};
use std::collections::HashSet;
use std::time::Instant;
use terra_gen::Seed;

/// One seed's dungeon, measured.
#[derive(Clone, Debug)]
pub struct Run {
    pub seed: u64,
    pub rooms: usize,
    pub solvable: bool,
    /// Placed (after `place_tries`), dressed (after `dress_tries`, or by the fallback).
    pub placed: bool,
    pub place_tries: u32,
    pub dress_tries: u32,
    pub fallback: bool,
    /// Every cell not blocked is reached from the entrance after dressing.
    pub connected: bool,
    /// The layout keeps its promises (`Layout::verify`).
    pub verified: bool,
    /// Milliseconds for the graph, placement and dressing.
    pub ms: [f64; 3],
    /// The cells, for how alike two layouts are.
    pub cells: Vec<[i32; 3]>,
    /// By module: how many cells took it.
    pub modules: Vec<u32>,
}

/// Generates and measures one seed.
pub fn run(theme: &Theme, set: &ModuleSet, seed: u64) -> (Run, Option<Dungeon>) {
    let s = Seed(seed);
    let clock = Instant::now();
    let graph = graph::generate(theme, s);
    let graph_ms = clock.elapsed().as_secs_f64() * 1e3;
    let clock = Instant::now();
    let placed = place::place(&graph, theme, s);
    let place_ms = clock.elapsed().as_secs_f64() * 1e3;
    let mut run = Run {
        seed,
        rooms: graph.rooms.len(),
        solvable: graph.solvable(),
        placed: placed.is_ok(),
        place_tries: placed
            .as_ref()
            .map_or(place::ATTEMPTS as u32, |l| l.attempts),
        dress_tries: 0,
        fallback: false,
        connected: false,
        verified: false,
        ms: [graph_ms, place_ms, 0.0],
        cells: Vec::new(),
        modules: vec![0; theme.modules.len()],
    };
    let Ok(layout) = placed else {
        return (run, None);
    };
    let clock = Instant::now();
    let dressing = dress::dress(&layout, &graph, theme, set, s);
    run.ms[2] = clock.elapsed().as_secs_f64() * 1e3;
    run.dress_tries = dressing.tries;
    run.fallback = dressing.fallback;
    run.verified = layout.verify(&graph).is_ok();
    let reached = layout.reached(|c| !set.blocks(dressing.picks[c]));
    run.connected = reached
        .iter()
        .zip(&dressing.picks)
        .all(|(&r, &v)| r || set.blocks(v));
    run.cells = layout.cells.iter().map(|c| c.at.to_array()).collect();
    for &v in &dressing.picks {
        run.modules[set.variants[v].module] += 1;
    }
    (
        run,
        Some(Dungeon {
            graph,
            layout,
            dressing,
        }),
    )
}

fn share(runs: &[Run], f: impl Fn(&Run) -> bool) -> f64 {
    runs.iter().filter(|r| f(r)).count() as f64 / runs.len().max(1) as f64 * 100.0
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(0.0)
}

/// How unlike two seeds' layouts are: the Jaccard distance of their cell sets, over pairs of
/// neighbouring seeds; 0 the same, 1 nothing shared.
pub fn layout_variety(runs: &[Run]) -> f64 {
    let placed: Vec<&Run> = runs.iter().filter(|r| r.placed).take(101).collect();
    let distances: Vec<f64> = placed
        .windows(2)
        .map(|pair| {
            let a: HashSet<[i32; 3]> = pair[0].cells.iter().copied().collect();
            let b: HashSet<[i32; 3]> = pair[1].cells.iter().copied().collect();
            let union = a.union(&b).count().max(1);
            1.0 - a.intersection(&b).count() as f64 / union as f64
        })
        .collect();
    distances.iter().sum::<f64>() / distances.len().max(1) as f64
}

/// How evenly the free modules are used: the entropy of their counts over the most it could
/// be, 0 one module everywhere, 1 every module alike.
pub fn module_variety(theme: &Theme, runs: &[Run]) -> f64 {
    let free: Vec<usize> = (0..theme.modules.len())
        .filter(|&m| !theme.modules[m].fixed && !theme.modules[m].spaces.contains(&Space::Stair))
        .collect();
    let counts: Vec<f64> = free
        .iter()
        .map(|&m| runs.iter().map(|r| r.modules[m] as f64).sum())
        .collect();
    let total: f64 = counts.iter().sum();
    if total <= 0.0 || free.len() < 2 {
        return 0.0;
    }
    let entropy: f64 = counts
        .iter()
        .filter(|&&c| c > 0.0)
        .map(|&c| -(c / total) * libm::log2(c / total))
        .sum();
    entropy / libm::log2(free.len() as f64)
}

/// The theme's line.
pub fn line(theme: &Theme, runs: &[Run]) -> String {
    let placed: Vec<Run> = runs.iter().filter(|r| r.placed).cloned().collect();
    let mean = |f: &dyn Fn(&Run) -> f64, of: &[Run]| {
        of.iter().map(f).sum::<f64>() / of.len().max(1) as f64
    };
    let rooms = (
        runs.iter().map(|r| r.rooms).min().unwrap_or(0),
        runs.iter().map(|r| r.rooms).max().unwrap_or(0),
    );
    format!(
        "{}: {} seeds | solvable {:.1}% | placed {:.1}% ({:.2} tries) | dressed {:.1}% without the fallback ({:.2} tries) | \
         connected {:.1}% | verified {:.1}% | median ms: graph {:.2}, place {:.2}, dress {:.2} | rooms {}-{}, cells {:.0} | \
         variety: layout {:.2}, modules {:.2}",
        theme.name,
        runs.len(),
        share(runs, |r| r.solvable),
        share(runs, |r| r.placed),
        mean(&|r| r.place_tries as f64, &placed),
        share(&placed, |r| !r.fallback),
        mean(&|r| r.dress_tries as f64, &placed),
        share(&placed, |r| r.connected),
        share(&placed, |r| r.verified),
        median(runs.iter().map(|r| r.ms[0]).collect()),
        median(placed.iter().map(|r| r.ms[1]).collect()),
        median(placed.iter().map(|r| r.ms[2]).collect()),
        rooms.0,
        rooms.1,
        mean(&|r| r.cells.len() as f64, &placed),
        layout_variety(runs),
        module_variety(theme, &placed),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::assets;

    #[test]
    fn variety_is_measured_from_both_ends() {
        let theme = Theme::all(&assets()).unwrap().remove(0);
        let set = ModuleSet::new(&theme).unwrap();
        let runs: Vec<Run> = (0..20).map(|s| run(&theme, &set, s).0).collect();
        let same: Vec<Run> = (0..20).map(|_| runs[0].clone()).collect();
        assert_eq!(layout_variety(&same), 0.0, "one layout is not varied");
        assert!(
            layout_variety(&runs) > 0.3,
            "different seeds, different layouts"
        );
        let v = module_variety(&theme, &runs);
        assert!(v > 0.0 && v <= 1.0, "{v}");
        assert!(line(&theme, &runs).starts_with(&theme.name));
    }
}
