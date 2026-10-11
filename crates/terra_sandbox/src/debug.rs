//! What we need in front of us to keep the game fast: a short readout of frame times
//! against the budget that is always on screen, and more of it on F3. The same numbers go
//! to the log, as a summary once a minute and a warning when frames stutter, so every
//! session leaves a record of how it ran.
//!
//! The budget is D25: 60 frames a second on a mid-range GPU. A frame's work is the most
//! of three things that run side by side: the game's own update on the CPU, the render
//! passes recorded on the CPU, and the GPU drawing them. With vsync on, the time between
//! frames only says how fast the screen refreshes, so headroom is judged on the work.

use bevy::{
    diagnostic::{
        DiagnosticsStore, EntityCountDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
    },
    prelude::*,
    render::{diagnostic::RenderDiagnosticsPlugin, renderer::RenderAdapterInfo},
    window::{PresentMode, PrimaryWindow},
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// How long a frame may take: 60 frames a second (D25).
pub const FRAME_BUDGET_MS: f32 = 1000.0 / 60.0;
/// A frame this long is a stutter a player can see.
const STUTTER_MS: f32 = FRAME_BUDGET_MS * 2.0;
/// How often the readout changes, so it can be read and costs little.
const REFRESH: Duration = Duration::from_millis(250);
/// The readout's figures cover this much of the recent past.
const RECENT: Duration = Duration::from_secs(2);
/// How often the log gets a summary.
const SUMMARY: Duration = Duration::from_secs(60);
/// How often, at most, the log is told about stutters.
const STUTTER_REPORT: Duration = Duration::from_secs(10);
/// Frames at the start that load and compile shaders, and say nothing about play.
const WARM_UP_FRAMES: u32 = 300;

/// One frame, timed on the real clock.
#[derive(Clone, Copy)]
struct Sample {
    at: Instant,
    /// From the end of the last frame to the end of this one.
    frame_ms: f32,
    /// The game's own update, on the CPU.
    game_ms: f32,
    /// The render passes, as recorded on the CPU, if they were timed.
    render_ms: Option<f32>,
    /// The GPU drawing them, if it can say.
    gpu_ms: Option<f32>,
}

impl Sample {
    /// The longest of the work running side by side: what has to fit in the budget.
    fn work_ms(&self) -> f32 {
        self.game_ms
            .max(self.render_ms.unwrap_or(0.0))
            .max(self.gpu_ms.unwrap_or(0.0))
    }
}

/// Figures over a stretch of frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spread {
    pub median: f32,
    pub high: f32,
    pub worst: f32,
}

impl Spread {
    /// The middle, the 99th percentile and the worst of some numbers.
    pub fn of(mut values: Vec<f32>) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| a.total_cmp(b));
        let last = values.len() - 1;
        Some(Self {
            median: values[last / 2],
            high: values[last * 99 / 100],
            worst: values[last],
        })
    }
}

/// How the work compares with the budget, for colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Within,
    Close,
    Over,
}

impl Standing {
    pub fn of(work_ms: f32) -> Self {
        if work_ms > FRAME_BUDGET_MS {
            Self::Over
        } else if work_ms > FRAME_BUDGET_MS * 0.75 {
            Self::Close
        } else {
            Self::Within
        }
    }

    fn colour(self) -> Color {
        match self {
            Self::Within => Color::srgb(0.55, 0.95, 0.55),
            Self::Close => Color::srgb(1.0, 0.8, 0.3),
            Self::Over => Color::srgb(1.0, 0.4, 0.35),
        }
    }
}

#[derive(Resource)]
struct Frames {
    samples: VecDeque<Sample>,
    count: u32,
    update_began: Option<Instant>,
    last_end: Option<Instant>,
    last_summary: Option<Instant>,
    last_refresh: Option<Instant>,
    stutters: Vec<f32>,
    last_stutter_report: Option<Instant>,
    /// Frame times mean nothing on a software renderer, so it does not warn about them.
    software: bool,
}

/// Whether the readout shows everything or one line.
#[derive(Resource, Default)]
pub struct Detail(pub bool);

#[derive(Component)]
struct Readout;

/// Adds the readout and the log of frame times. `on_screen` is off for screenshots.
pub fn plugin(on_screen: bool) -> impl Fn(&mut App) {
    move |app: &mut App| {
        app.add_plugins((
            RenderDiagnosticsPlugin,
            EntityCountDiagnosticsPlugin::default(),
            SystemInformationDiagnosticsPlugin,
        ))
        .insert_resource(Frames {
            samples: VecDeque::new(),
            count: 0,
            update_began: None,
            last_end: None,
            last_summary: None,
            last_refresh: None,
            stutters: Vec::new(),
            last_stutter_report: None,
            software: false,
        })
        .init_resource::<Detail>()
        .add_systems(Startup, describe_machine)
        .add_systems(First, begin_update)
        .add_systems(Last, (end_update, report).chain());
        if on_screen {
            app.add_systems(Startup, spawn_readout)
                .add_systems(Update, toggle_detail)
                .add_systems(Last, show.after(end_update));
        }
    }
}

/// One line at the start saying what the game is running on.
fn describe_machine(adapter: Res<RenderAdapterInfo>, mut frames: ResMut<Frames>) {
    // Bevy does not re-export wgpu's device types, and this is not worth a dependency.
    frames.software = format!("{:?}", adapter.device_type) == "Cpu";
    info!(
        "Running on {} ({:?}, {} {}), {} build. Frame budget {FRAME_BUDGET_MS:.1} ms (D25).",
        adapter.name,
        adapter.backend,
        adapter.driver,
        adapter.driver_info,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
    );
}

fn begin_update(mut frames: ResMut<Frames>) {
    frames.update_began = Some(Instant::now());
}

/// The render passes' times, summed over the outermost ones so nothing counts twice.
fn render_times(store: &DiagnosticsStore, field: &str) -> Option<f32> {
    let mut total = None;
    for diagnostic in store.iter() {
        let mut parts = diagnostic.path().components();
        if parts.next() != Some("render") {
            continue;
        }
        let (Some(_pass), Some(last), None) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        if last == field
            && let Some(value) = diagnostic.value()
        {
            *total.get_or_insert(0.0) += value as f32;
        }
    }
    total
}

fn end_update(mut frames: ResMut<Frames>, store: Res<DiagnosticsStore>) {
    let now = Instant::now();
    let game_ms = frames
        .update_began
        .map_or(0.0, |began| (now - began).as_secs_f32() * 1000.0);
    let Some(last_end) = frames.last_end.replace(now) else {
        return;
    };
    frames.count += 1;
    let sample = Sample {
        at: now,
        frame_ms: (now - last_end).as_secs_f32() * 1000.0,
        game_ms,
        render_ms: render_times(&store, "elapsed_cpu"),
        gpu_ms: render_times(&store, "elapsed_gpu"),
    };
    if frames.count > WARM_UP_FRAMES && sample.frame_ms > STUTTER_MS {
        frames.stutters.push(sample.frame_ms);
    }
    frames.samples.push_back(sample);
    while frames
        .samples
        .front()
        .is_some_and(|oldest| now - oldest.at > SUMMARY)
    {
        frames.samples.pop_front();
    }
}

fn spread(samples: &[Sample], value: impl Fn(&Sample) -> Option<f32>) -> Option<Spread> {
    Spread::of(samples.iter().filter_map(value).collect())
}

/// Once a minute, how the last minute went; and, now and then, how often it stuttered.
fn report(mut frames: ResMut<Frames>) {
    let Some(&Sample { at: now, .. }) = frames.samples.back() else {
        return;
    };
    if frames.count <= WARM_UP_FRAMES {
        frames.last_summary = Some(now);
        return;
    }
    if !frames.software
        && !frames.stutters.is_empty()
        && frames
            .last_stutter_report
            .is_none_or(|last| now - last > STUTTER_REPORT)
    {
        let worst = frames.stutters.iter().fold(0.0f32, |a, &b| a.max(b));
        let count = frames.stutters.len();
        warn!(
            "{count} {} took over {STUTTER_MS:.0} ms since the last report, the worst {worst:.1} ms.",
            if count == 1 { "frame" } else { "frames" }
        );
        frames.stutters.clear();
        frames.last_stutter_report = Some(now);
    }
    if frames.last_summary.is_some_and(|last| now - last < SUMMARY) {
        return;
    }
    frames.last_summary = Some(now);
    let samples: Vec<Sample> = frames.samples.iter().copied().collect();
    let line = |name: &str, spread: Option<Spread>| {
        spread.map_or(format!("{name} not timed"), |s| {
            format!("{name} {:.1}/{:.1}/{:.1}", s.median, s.high, s.worst)
        })
    };
    let over = samples
        .iter()
        .filter(|sample| sample.work_ms() > FRAME_BUDGET_MS)
        .count();
    info!(
        "Last minute, {} frames, in ms as median/99th/worst: {}; {}; {}; {}. {over} over budget.",
        samples.len(),
        line("work", spread(&samples, |s| Some(s.work_ms()))),
        line("game", spread(&samples, |s| Some(s.game_ms))),
        line("render", spread(&samples, |s| s.render_ms)),
        line("GPU", spread(&samples, |s| s.gpu_ms)),
    );
}

fn spawn_readout(mut commands: Commands) {
    commands.spawn((
        Readout,
        Text::new(""),
        TextFont::from_font_size(14.0),
        TextColor(Standing::Within.colour()),
        Node {
            position_type: PositionType::Absolute,
            left: px(8),
            top: px(8),
            padding: UiRect::axes(px(8), px(4)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
        // Above everything else on screen.
        GlobalZIndex(i32::MAX - 1),
    ));
}

fn toggle_detail(keys: Res<ButtonInput<KeyCode>>, mut detail: ResMut<Detail>) {
    if keys.just_pressed(KeyCode::F3) {
        detail.0 = !detail.0;
    }
}

fn show(
    mut frames: ResMut<Frames>,
    detail: Res<Detail>,
    store: Res<DiagnosticsStore>,
    adapter: Res<RenderAdapterInfo>,
    window: Single<&Window, With<PrimaryWindow>>,
    readout: Single<(&mut Text, &mut TextColor), With<Readout>>,
) {
    let Some(&Sample { at: now, .. }) = frames.samples.back() else {
        return;
    };
    if frames.last_refresh.is_some_and(|last| now - last < REFRESH) {
        return;
    }
    frames.last_refresh = Some(now);
    let recent: Vec<Sample> = frames
        .samples
        .iter()
        .filter(|sample| now - sample.at <= RECENT)
        .copied()
        .collect();
    let Some(work) = spread(&recent, |s| Some(s.work_ms())) else {
        return;
    };
    let frame = spread(&recent, |s| Some(s.frame_ms)).unwrap_or(work);
    let figure =
        |spread: Option<Spread>| spread.map_or("-".to_string(), |s| format!("{:.1}", s.median));
    let (mut text, mut colour) = readout.into_inner();
    colour.0 = Standing::of(work.high).colour();
    let mut lines = vec![format!(
        "{:.1} ms of {FRAME_BUDGET_MS:.1}  worst {:.1}   game {}  render {}  GPU {}   {:.0} fps",
        work.median,
        work.worst,
        figure(spread(&recent, |s| Some(s.game_ms))),
        figure(spread(&recent, |s| s.render_ms)),
        figure(spread(&recent, |s| s.gpu_ms)),
        1000.0 / frame.median.max(0.001),
    )];
    if detail.0 {
        let value = |path: &str| {
            store
                .iter()
                .find(|d| d.path().as_str() == path)
                .and_then(|d| d.smoothed())
        };
        lines.push(format!(
            "frame {:.1}/{:.1}/{:.1} ms (median/99th/worst over {}s)   {:?}",
            frame.median,
            frame.high,
            frame.worst,
            RECENT.as_secs(),
            window.present_mode,
        ));
        lines.push(format!(
            "{}  {:?}   {}x{}   {} build",
            adapter.name,
            adapter.backend,
            window.physical_width(),
            window.physical_height(),
            // An unoptimised build is several times slower on the CPU.
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
        ));
        lines.push(format!(
            "entities {:.0}   process CPU {:.0}%  memory {:.2} GiB",
            value("entity_count").unwrap_or(0.0),
            value("process/cpu_usage").unwrap_or(0.0),
            value("process/mem_usage").unwrap_or(0.0),
        ));
        // The costliest passes on the GPU, or on the CPU when the GPU cannot say.
        let field = if recent.iter().any(|s| s.gpu_ms.is_some()) {
            "elapsed_gpu"
        } else {
            "elapsed_cpu"
        };
        let mut passes: Vec<(String, f64)> = store
            .iter()
            .filter_map(|d| {
                let path = d.path().as_str();
                let pass = path.strip_prefix("render/")?.strip_suffix(field)?;
                Some((pass.trim_end_matches('/').to_string(), d.smoothed()?))
            })
            .collect();
        passes.sort_by(|a, b| b.1.total_cmp(&a.1));
        lines.push(format!(
            "passes by {}:",
            if field == "elapsed_gpu" { "GPU" } else { "CPU" }
        ));
        for (pass, ms) in passes.iter().take(8) {
            lines.push(format!("  {ms:5.2} ms  {pass}"));
        }
        if window.present_mode != PresentMode::AutoNoVsync {
            lines.push("vsync is on, so frame time follows the screen; work is what counts".into());
        }
        lines.push("F3 less".into());
    }
    text.0 = lines.join("\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreads_numbers() {
        let spread = Spread::of((1..=100).map(|n| n as f32).collect()).unwrap();
        assert_eq!(spread.median, 50.0);
        assert_eq!(spread.high, 99.0);
        assert_eq!(spread.worst, 100.0);
        assert_eq!(Spread::of(Vec::new()), None);
    }

    #[test]
    fn judges_work_against_the_budget() {
        assert_eq!(Standing::of(5.0), Standing::Within);
        assert_eq!(Standing::of(14.0), Standing::Close);
        assert_eq!(Standing::of(20.0), Standing::Over);
    }

    #[test]
    fn work_is_the_longest_of_what_runs_side_by_side() {
        let sample = Sample {
            at: Instant::now(),
            frame_ms: 16.7,
            game_ms: 3.0,
            render_ms: Some(2.0),
            gpu_ms: Some(5.0),
        };
        assert_eq!(sample.work_ms(), 5.0);
        let untimed = Sample {
            gpu_ms: None,
            render_ms: None,
            ..sample
        };
        assert_eq!(untimed.work_ms(), 3.0);
    }
}
