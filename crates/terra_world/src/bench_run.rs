//! What a benchmark's run says beside its frames (`shell::scripted_run`, `cargo xtask
//! bench`): how long its ticks took against their timestep, so a scene whose game cannot
//! keep up with the clock fails before a player meets the death spiral it leads to (D138;
//! the lake, 2026-11-02: ticks of 35-40 ms against 15.6, 4 frames a second); its GPU time by
//! thirds, so drawing that grows the longer a scene runs shows apart from the frame; and the
//! phases its budgets hold (D607, `assets/data/perf/budgets.ron`): the settle, the first
//! seconds after the loading screen lifts with the script standing still and the world
//! streaming in (`--bench-settle`), timed apart from play, and each phase's groups of systems
//! (`terra_gen::stat`) by their ms a frame.

use crate::{
    debug::{FrameParts, STUTTER_MS},
    loading::{self, Screen},
    recorder::FREEZE_MS,
    shell::Options,
    tick::FrameTicks,
};
use bevy::prelude::*;
use std::{collections::BTreeMap, time::Instant};

/// The timed frames' ticks and GPU times, gathered until the run ends; the settle phase's
/// frames and ticks; and each phase's groups.
#[derive(Default)]
pub struct Gathered {
    frame: u32,
    tick_ms: Vec<f32>,
    gpu_ms: Vec<f32>,
    said: bool,
    last: Option<Instant>,
    settle_ms: Vec<f32>,
    settle_tick_ms: Vec<f32>,
    settle_groups: Groups,
    play_groups: Groups,
}

/// Each group's ms in each frame of a phase, a frame it did not run in counting 0; and how
/// many scopes timed them, for what timing them cost.
#[derive(Default)]
struct Groups {
    frames: usize,
    ms: BTreeMap<&'static str, Vec<f32>>,
    scopes: u64,
}

impl Groups {
    fn add(&mut self, taken: &[terra_gen::stat::Taken]) {
        self.frames += 1;
        for group in taken {
            let ms = self
                .ms
                .entry(group.group)
                .or_insert_with(|| vec![0.0; self.frames - 1]);
            ms.push(group.ns as f32 / 1e6);
            self.scopes += group.scopes;
        }
        for ms in self.ms.values_mut() {
            ms.resize(self.frames, 0.0);
        }
    }
}

/// Starts and ends a bench's settle phase (`--bench-settle`): from the first frame the
/// loading screen is down, the script stands still (`loading::Hold::settling`) for so many
/// seconds while the world, its clock running, streams in round it; then the scene runs as
/// it was written. As a shipped game's perf capture waits for streaming to settle before its
/// fly-through, but timing the wait, where a load's leftovers land (D607). Before
/// `scripted_run`, so no frame of the script is counted inside it.
pub fn settle(
    options: Res<Options>,
    screen: Res<State<Screen>>,
    mut hold: ResMut<loading::Hold>,
    mut lifted: Local<Option<Instant>>,
) {
    if !options.bench || options.bench_settle_s <= 0.0 || *screen.get() == Screen::Loading {
        return;
    }
    let since = *lifted.get_or_insert_with(Instant::now);
    let settling = since.elapsed().as_secs_f32() < options.bench_settle_s;
    if hold.settling != settling {
        hold.settling = settling;
    }
}

/// Runs after `scripted_run` each frame: gathers the timed frames as it counts them, and
/// says what they came to on the frame it ends the run.
#[allow(clippy::too_many_arguments)]
pub fn gather(
    options: Res<Options>,
    screen: Res<State<Screen>>,
    hold: Res<loading::Hold>,
    ticks: Option<Res<FrameTicks>>,
    parts: Option<Res<FrameParts>>,
    tick: Res<Time<Fixed>>,
    mut exits: MessageReader<AppExit>,
    mut run: Local<Gathered>,
) {
    let ended = exits.read().next().is_some();
    if !options.bench || run.said {
        return;
    }
    // Each frame's groups, taken every frame so a phase holds only its own.
    let taken = terra_gen::stat::take();
    let now = Instant::now();
    let frame_ms = run
        .last
        .replace(now)
        .map(|last| (now - last).as_secs_f32() * 1000.0);
    if !ended {
        if hold.settling {
            run.settle_ms.extend(frame_ms);
            run.settle_tick_ms
                .extend(ticks.as_ref().and_then(|t| t.tick_ms()));
            run.settle_groups.add(&taken);
        } else if crate::shell::counting(&options, &screen, &hold) {
            run.frame += 1;
            if run.frame > options.settle_frames {
                run.tick_ms.extend(ticks.and_then(|t| t.tick_ms()));
                run.gpu_ms.extend(parts.and_then(|p| p.gpu_ms));
                run.play_groups.add(&taken);
            }
        }
        return;
    }
    let run = &mut *run;
    run.said = true;
    if let Some(line) = ticks_line(&mut run.tick_ms, tick.timestep().as_secs_f32() * 1000.0) {
        println!("{line}");
    }
    if let Some(line) = thirds_line(&run.gpu_ms) {
        println!("{line}");
    }
    if let Some(line) = settle_line(
        options.bench_settle_s,
        &mut run.settle_ms,
        &mut run.settle_tick_ms,
    ) {
        println!("{line}");
    }
    for (phase, groups) in [("settle", &run.settle_groups), ("play", &run.play_groups)] {
        if let Some(line) = groups_line(phase, groups) {
            println!("{line}");
        }
    }
}

/// The settle phase's line: its frames' spread, hitches and freezes, and its ticks' 99th
/// percentile.
fn settle_line(seconds: f32, frame_ms: &mut [f32], tick_ms: &mut [f32]) -> Option<String> {
    if frame_ms.is_empty() {
        return None;
    }
    let hitches = frame_ms.iter().filter(|&&ms| ms > STUTTER_MS).count();
    let freezes = frame_ms.iter().filter(|&&ms| ms >= FREEZE_MS).count();
    frame_ms.sort_by(f32::total_cmp);
    tick_ms.sort_by(f32::total_cmp);
    let n = frame_ms.len();
    let ticks = tick_ms
        .get(tick_ms.len() * 99 / 100)
        .map_or("-".to_string(), |ms| format!("{ms:.2} ms"));
    Some(format!(
        "SANDBOX settle phase, the first {seconds:.0} s after the loading screen lifted: {n} frames, median {:.2} ms, 99th percentile {:.2} ms, worst {:.2} ms; {hitches} hitches over {STUTTER_MS:.1} ms, {freezes} freezes of {FREEZE_MS:.0} ms or more; ticks' 99th percentile {ticks}",
        frame_ms[n / 2],
        frame_ms[n * 99 / 100],
        frame_ms[n - 1],
    ))
}

/// A phase's groups' line: each group's median and 99th percentile ms a frame, and the scopes
/// a frame that timed them.
fn groups_line(phase: &str, groups: &Groups) -> Option<String> {
    if groups.frames == 0 {
        return None;
    }
    let each: Vec<String> = groups
        .ms
        .iter()
        .map(|(name, ms)| {
            let mut ms = ms.clone();
            ms.sort_by(f32::total_cmp);
            let n = ms.len();
            format!("{name} {:.2} ({:.2})", ms[n / 2], ms[n * 99 / 100])
        })
        .collect();
    Some(format!(
        "SANDBOX groups in the {phase} phase, ms a frame, median (99th): {}; {:.0} scopes a frame",
        each.join(", "),
        groups.scopes as f64 / groups.frames as f64
    ))
}

/// The ticks' line: their median, 99th percentile and worst against a tick's time.
fn ticks_line(tick_ms: &mut [f32], timestep_ms: f32) -> Option<String> {
    if tick_ms.is_empty() {
        return None;
    }
    tick_ms.sort_by(f32::total_cmp);
    let n = tick_ms.len();
    Some(format!(
        "SANDBOX ticks, in ms a tick: median {:.2}, 99th percentile {:.2}, worst {:.2}, of {n} frames' ticks, against the {timestep_ms:.1} ms of a tick",
        tick_ms[n / 2],
        tick_ms[n * 99 / 100],
        tick_ms[n - 1],
    ))
}

/// The GPU's line: the medians of the run's thirds, and the last on the first.
fn thirds_line(gpu_ms: &[f32]) -> Option<String> {
    let third = gpu_ms.len() / 3;
    if third == 0 {
        return None;
    }
    let median = |part: &[f32]| {
        let mut part = part.to_vec();
        part.sort_by(f32::total_cmp);
        part[part.len() / 2]
    };
    let [a, b, c] = [
        median(&gpu_ms[..third]),
        median(&gpu_ms[third..2 * third]),
        median(&gpu_ms[gpu_ms.len() - third..]),
    ];
    let grew = if a > 0.0 { 100.0 * (c / a - 1.0) } else { 0.0 };
    Some(format!(
        "SANDBOX GPU by thirds of the run, medians in ms: {a:.2} {b:.2} {c:.2}; the last third {grew:+.1}% on the first"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ticks_line_says_the_spread_of_a_tick() {
        let mut ms: Vec<f32> = (1..=100).map(|i| i as f32 / 10.0).collect();
        let line = ticks_line(&mut ms, 15.625).unwrap();
        assert!(line.contains("median 5.10, 99th percentile 10.00, worst 10.00"));
        assert!(line.ends_with("against the 15.6 ms of a tick"));
        assert_eq!(ticks_line(&mut [], 15.625), None);
    }

    #[test]
    fn the_gpu_line_says_how_much_the_last_third_grew_on_the_first() {
        let ms: Vec<f32> = [10.0; 30].into_iter().chain([12.0; 30]).collect();
        let line = thirds_line(&ms).unwrap();
        assert!(line.contains("medians in ms: 10.00 12.00 12.00;"), "{line}");
        assert!(line.ends_with("+20.0% on the first"), "{line}");
        assert_eq!(thirds_line(&[1.0, 2.0]), None);
    }

    #[test]
    fn the_settle_line_counts_its_hitches_and_freezes() {
        let mut ms: Vec<f32> = [10.0; 97].into_iter().chain([30.0, 30.0, 120.0]).collect();
        let line = settle_line(30.0, &mut ms, &mut [1.0, 2.0]).unwrap();
        assert!(
            line.contains("first 30 s after the loading screen lifted: 100 frames"),
            "{line}"
        );
        assert!(
            line.contains("worst 120.00 ms; 3 hitches over 26.7 ms, 1 freezes"),
            "{line}"
        );
        assert!(line.ends_with("ticks' 99th percentile 2.00 ms"), "{line}");
        assert_eq!(settle_line(30.0, &mut [], &mut []), None);
    }

    #[test]
    fn a_group_that_joins_late_counts_its_earlier_frames_as_none() {
        let taken = |group, ms: f32| terra_gen::stat::Taken {
            group,
            ns: (ms * 1e6) as u64,
            scopes: 2,
        };
        let mut groups = Groups::default();
        groups.add(&[taken("flora", 1.0)]);
        groups.add(&[taken("flora", 3.0), taken("creatures", 2.0)]);
        groups.add(&[taken("flora", 2.0), taken("creatures", 2.0)]);
        assert_eq!(groups.ms["creatures"], vec![0.0, 2.0, 2.0]);
        let line = groups_line("play", &groups).unwrap();
        assert_eq!(
            line,
            "SANDBOX groups in the play phase, ms a frame, median (99th): creatures 2.00 (2.00), flora 2.00 (3.00); 3 scopes a frame"
        );
        assert_eq!(groups_line("settle", &Groups::default()), None);
    }
}
