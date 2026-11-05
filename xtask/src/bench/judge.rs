//! A scene's run judged against its budgets (`budgets`, D607): each miss names what is over,
//! its budget, what was measured and where to look (D556).

use super::budgets::{FILE, Limits, Scene};
use super::{Timing, play};

/// What the scene missed of its budgets: its load past the most it may take; a play scene's
/// walk, through ground streaming in, any freeze (D578); the integrated GPU's median at the
/// low preset (D239); or every budget of its play phase.
pub(super) fn missed(timing: &Timing) -> Vec<String> {
    let budget = &timing.budget;
    let mut missed = play::load_missed(timing.load_s, budget.load_s);
    missed.extend(held_still(timing, budget));
    let limits = &budget.play;
    if timing.from_start {
        missed.extend(freezes(timing, limits));
        return missed;
    }
    if timing.integrated() {
        if let Some(most) = limits.integrated_median_ms
            && timing.median_ms > most
        {
            missed.push(format!(
                "median {:.1} ms over the {most:.1} ms of the integrated GPU's 30 fps (D239, {FILE})",
                timing.median_ms
            ));
        }
        return missed;
    }
    missed.extend(frames(timing, limits));
    missed.extend(timing.phases.misses(&budget.settle, limits));
    missed
}

/// What the play phase's frames missed.
fn frames(timing: &Timing, limits: &Limits) -> Vec<String> {
    let mut missed = Vec::new();
    let look = "the systems that ran long: `cargo xtask profile`'s report";
    match limits.frame_p99_ms {
        Some(_) if timing.budget_ms <= 0.0 => {
            missed.push("the run did not say how it held to the budget".to_string())
        }
        Some(most) if timing.p99_ms > most => missed.push(format!(
            "99th percentile {:.1} ms over the play phase's {most:.1} ms (D25, {FILE}); {look}",
            timing.p99_ms
        )),
        _ => {}
    }
    if let Some(most) = limits.over_pct
        && timing.over_pct > most
    {
        missed.push(format!(
            "{:.1}% of frames over budget, past the {most}% the play phase allows ({FILE}); {look}",
            timing.over_pct
        ));
    }
    if let Some(most) = limits.hitches
        && timing.hitches > most
    {
        missed.push(format!(
            "{} hitches, past the {most} the play phase allows ({FILE}): {}",
            timing.hitches,
            timing.longest.look()
        ));
    }
    missed.extend(freezes(timing, limits));
    missed.extend(
        timing
            .ticks
            .misses(limits.tick_p99_ms, limits.gpu_growth_pct),
    );
    if let Some(most) = limits.drift_pct
        && timing.drift_pct > most
    {
        missed.push(format!(
            "{:+.0}% slower by the end, past the {most}% the play phase allows ({FILE}): the game's work grew over the run",
            timing.drift_pct
        ));
    }
    missed
}

/// Freezes past the most a phase allows, with what held the worst as the run's freeze
/// warnings said, which the bench keeps (`freezes`, D576).
fn freezes(timing: &Timing, limits: &Limits) -> Option<String> {
    let most = limits.freezes?;
    (timing.freezes > most).then(|| {
        format!(
            "{} freezes of {:.0} ms or more, past the {most} allowed ({FILE}): {}",
            timing.freezes,
            play::FREEZE_MS,
            timing.froze.look()
        )
    })
}

/// A frame, its load's too, longer than the scene's `load_frame_ms`: the window drawn nothing
/// meanwhile, the loading screen's bar held still (crossing-freeze j1: 13.7 s as the world
/// was entered, every walk at the lake).
fn held_still(timing: &Timing, budget: &Scene) -> Option<String> {
    let most = budget.load_frame_ms?;
    let (ms, _) = timing.froze.worst.as_ref()?;
    (*ms > most).then(|| {
        format!(
            "a frame took {ms:.0} ms, past the {most:.0} ms any frame may hold the window, its load's too ({FILE}'s load_frame_ms): {}; move the work it names off the main thread or spread it over frames, never raise the budget to pass (D414)",
            timing.froze.look()
        )
    })
}

/// The table's closing words on what a scene is held to.
pub(super) fn said(budget: &Scene) -> String {
    let play = &budget.play;
    let or = |value: Option<f32>| value.map_or("-".to_string(), |v| format!("{v}"));
    let load = match (budget.load_target_s, budget.load_provisional) {
        (Some(target), true) => format!(
            " A load aims at {target:.0} s to playable, provisional while D580 is Open, so it fails none."
        ),
        (Some(target), false) => format!(" A load aims at {target:.0} s to playable (D580)."),
        (None, _) => String::new(),
    };
    format!(
        "The budgets are {FILE}'s (D607), by scene and phase; in play, every scene's: a 99th \
         percentile under {} ms (D25: 75 fps on the developers' RTX 3080 at the high preset), at \
         most {}% of frames over, no hitch over twice it and at most {}% slower by the end, ticks' \
         99th percentile under {} ms (D138), on the machine it runs on; on an integrated GPU, at \
         the low preset, a median under {} ms (D239).{load}",
        or(play.frame_p99_ms),
        or(play.over_pct),
        or(play.drift_pct),
        or(play.tick_p99_ms),
        or(play.integrated_median_ms),
    )
}
