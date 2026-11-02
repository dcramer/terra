//! What a benchmark's run says beside its frames (`shell::scripted_run`, `cargo xtask
//! bench`): how long its ticks took against their timestep, so a scene whose game cannot
//! keep up with the clock fails before a player meets the death spiral it leads to (D138;
//! the lake, 2026-11-02: ticks of 35-40 ms against 15.6, 4 frames a second); and its GPU
//! time by thirds, so drawing that grows the longer a scene runs shows apart from the frame.

use crate::{
    debug::FrameParts,
    loading::{self, Screen},
    shell::Options,
    tick::FrameTicks,
};
use bevy::prelude::*;

/// The timed frames' ticks and GPU times, gathered until the run ends.
#[derive(Default)]
pub struct Gathered {
    frame: u32,
    tick_ms: Vec<f32>,
    gpu_ms: Vec<f32>,
    said: bool,
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
    if !ended {
        if crate::shell::counting(&options, &screen, &hold) {
            run.frame += 1;
            if run.frame > options.settle_frames {
                run.tick_ms.extend(ticks.and_then(|t| t.tick_ms()));
                run.gpu_ms.extend(parts.and_then(|p| p.gpu_ms));
            }
        }
        return;
    }
    run.said = true;
    if let Some(line) = ticks_line(&mut run.tick_ms, tick.timestep().as_secs_f32() * 1000.0) {
        println!("{line}");
    }
    if let Some(line) = thirds_line(&run.gpu_ms) {
        println!("{line}");
    }
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
}
