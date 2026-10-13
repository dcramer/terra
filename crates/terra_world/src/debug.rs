//! What we need in front of us to keep the game fast: a short readout of frame times
//! against the budget and a graph of the last few seconds, a bar a frame, always on screen,
//! and more of both on F3. The same numbers go
//! to the log, as a summary once a minute and a warning when frames stutter, so every
//! session leaves a record of how it ran.
//!
//! The budget is D25: 60 frames a second on a mid-range GPU. A frame's work is the most
//! of three things that run side by side: the game's own update on the CPU, the render
//! passes recorded on the CPU, and the GPU drawing them. With vsync on, the time between
//! frames only says how fast the screen refreshes, so headroom is judged on the work.
//!
//! The graph is the frame-time graph every engine has (Unreal's `stat unitgraph`, Bevy's
//! own `FrameTimeGraph`), drawn on the CPU into a small picture so it shows our work rather
//! than the time between frames, and so it can be tested.

use bevy::{
    asset::RenderAssetUsages,
    diagnostic::{
        DiagnosticsStore, EntityCountDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
    },
    image::ImageSampler,
    prelude::*,
    render::{
        diagnostic::RenderDiagnosticsPlugin,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        renderer::RenderAdapterInfo,
    },
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
/// How many frames the graph shows: four seconds at 60 a second.
const GRAPH_FRAMES: usize = 240;
/// How wide each frame's bar is, in pixels.
const BAR_PX: usize = 2;
/// How tall a graph is, in pixels. Its top is a stutter, so the budget line is halfway up.
const GRAPH_PX: usize = 40;

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
        let [r, g, b, _] = self.rgba();
        Color::srgb_u8(r, g, b)
    }

    /// The colour as sRGB bytes, for the graph's pixels.
    fn rgba(self) -> [u8; 4] {
        match self {
            Self::Within => [140, 242, 140, 255],
            Self::Close => [255, 204, 77, 255],
            Self::Over => [255, 102, 89, 255],
        }
    }
}

/// The budget line across a graph, drawn over the bars so it shows through them.
const BUDGET_LINE: [u8; 4] = [255, 255, 255, 200];
/// A graph's empty part, a little darker than the readout so its edges show.
const GRAPH_BACKGROUND: [u8; 4] = [0, 0, 0, 110];

/// Draws a graph of some frames' times into RGBA pixels `width` by `height`, oldest on the
/// left, a bar a frame [`BAR_PX`] wide, the newest at the right edge. The top is a stutter
/// (twice the budget) and taller bars stop there; the budget line is halfway up. A frame
/// with no time draws nothing.
fn draw_graph(times: &[Option<f32>], width: usize, height: usize, pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&GRAPH_BACKGROUND);
    }
    let bars = width / BAR_PX;
    let shown = &times[times.len().saturating_sub(bars)..];
    let first = bars - shown.len();
    for (i, time) in shown.iter().enumerate() {
        let Some(ms) = *time else { continue };
        let tall = ((ms / STUTTER_MS).clamp(0.0, 1.0) * height as f32).round() as usize;
        let colour = Standing::of(ms).rgba();
        for x in (first + i) * BAR_PX..(first + i + 1) * BAR_PX {
            for y in height - tall..height {
                let at = (y * width + x) * 4;
                pixels[at..at + 4].copy_from_slice(&colour);
            }
        }
    }
    let line = height - height / 2;
    for x in 0..width {
        let at = (line * width + x) * 4;
        pixels[at..at + 4].copy_from_slice(&BUDGET_LINE);
    }
}

/// What a graph shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    /// The longest of the three, against the budget. Always shown.
    Work,
    Game,
    Render,
    Gpu,
}

impl Part {
    const ALL: [Self; 4] = [Self::Work, Self::Game, Self::Render, Self::Gpu];

    fn label(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Game => "game",
            Self::Render => "render",
            Self::Gpu => "GPU",
        }
    }

    fn of(self, sample: &Sample) -> Option<f32> {
        match self {
            Self::Work => Some(sample.work_ms()),
            Self::Game => Some(sample.game_ms),
            Self::Render => sample.render_ms,
            Self::Gpu => sample.gpu_ms,
        }
    }
}

/// A row of the readout holding one part's graph; only work's shows until F3.
#[derive(Component)]
struct Graph {
    part: Part,
    image: Handle<Image>,
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

/// The last frame's parts, for scripted runs to time: the game's own systems on the CPU, the
/// render passes on the CPU, and the GPU (when the backend has timestamp queries).
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct FrameParts {
    pub game_ms: f32,
    pub render_ms: Option<f32>,
    pub gpu_ms: Option<f32>,
}

/// Whether the readout shows everything or one line.
#[derive(Resource, Default)]
pub struct Detail(pub bool);

/// The readout's first line, coloured by how the work stands.
#[derive(Component)]
struct Readout;

/// What F3 adds under the graphs.
#[derive(Component)]
struct ReadoutDetail;

/// Adds the readout and the log of frame times. `on_screen` is off for screenshots.
pub fn plugin(on_screen: bool) -> impl Fn(&mut App) {
    move |app: &mut App| {
        // Timing render passes takes GPU counters, which Metal cannot time anyway and runs
        // out of as new pipelines appear, losing the device and quitting the game. On macOS
        // the render and GPU times show as "-".
        if !cfg!(target_os = "macos") {
            app.add_plugins(RenderDiagnosticsPlugin);
        }
        app.add_plugins((
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
        .init_resource::<FrameParts>()
        .add_systems(Last, (end_update, report).chain());
        if on_screen {
            app.add_systems(Startup, spawn_readout)
                .add_systems(Update, toggle_detail)
                .add_systems(Last, (show, draw_graphs).after(end_update));
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

fn end_update(
    mut frames: ResMut<Frames>,
    store: Res<DiagnosticsStore>,
    mut parts: ResMut<FrameParts>,
) {
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
    *parts = FrameParts {
        game_ms: sample.game_ms,
        render_ms: sample.render_ms,
        gpu_ms: sample.gpu_ms,
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

fn spawn_readout(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let (width, height) = (GRAPH_FRAMES * BAR_PX, GRAPH_PX);
    let readout = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(8),
                top: px(8),
                padding: UiRect::axes(px(8), px(4)),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            // Above everything else on screen.
            GlobalZIndex(i32::MAX - 1),
        ))
        .id();
    commands.spawn((
        Readout,
        Text::new(""),
        TextFont::from_font_size(14.0),
        TextColor(Standing::Within.colour()),
        ChildOf(readout),
    ));
    for part in Part::ALL {
        let mut image = Image::new_fill(
            Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
        image.sampler = ImageSampler::nearest();
        let image = images.add(image);
        let row = commands
            .spawn((
                Graph {
                    part,
                    image: image.clone(),
                },
                Node {
                    column_gap: px(6),
                    align_items: AlignItems::Center,
                    display: if part == Part::Work {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
                ChildOf(readout),
            ))
            .id();
        commands.spawn((
            Text::new(part.label()),
            TextFont::from_font_size(12.0),
            TextColor(Color::srgb(0.8, 0.8, 0.8)),
            Node {
                width: px(44),
                ..default()
            },
            ChildOf(row),
        ));
        commands.spawn((
            ImageNode::new(image),
            Node {
                width: px(width as f32),
                height: px(height as f32),
                ..default()
            },
            ChildOf(row),
        ));
    }
    commands.spawn((
        ReadoutDetail,
        Text::new(""),
        TextFont::from_font_size(14.0),
        TextColor(Color::srgb(0.85, 0.85, 0.85)),
        Node {
            display: Display::None,
            ..default()
        },
        ChildOf(readout),
    ));
}

fn toggle_detail(
    keys: Res<ButtonInput<KeyCode>>,
    mut detail: ResMut<Detail>,
    mut graphs: Query<(&Graph, &mut Node)>,
) {
    if keys.just_pressed(KeyCode::F3) {
        detail.0 = !detail.0;
        for (graph, mut node) in &mut graphs {
            if graph.part != Part::Work {
                node.display = if detail.0 {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }
    }
}

/// Redraws every graph on screen with the newest frame.
fn draw_graphs(
    frames: Res<Frames>,
    detail: Res<Detail>,
    graphs: Query<&Graph>,
    mut images: ResMut<Assets<Image>>,
) {
    let start = frames.samples.len().saturating_sub(GRAPH_FRAMES);
    for graph in &graphs {
        if graph.part != Part::Work && !detail.0 {
            continue;
        }
        let Some(mut image) = images.get_mut(&graph.image) else {
            continue;
        };
        let (width, height) = (image.width() as usize, image.height() as usize);
        let times: Vec<Option<f32>> = frames
            .samples
            .range(start..)
            .map(|sample| graph.part.of(sample))
            .collect();
        if let Some(pixels) = image.data.as_mut() {
            draw_graph(&times, width, height, pixels);
        }
    }
}

fn show(
    mut frames: ResMut<Frames>,
    detail: Res<Detail>,
    store: Res<DiagnosticsStore>,
    adapter: Res<RenderAdapterInfo>,
    window: Single<&Window, With<PrimaryWindow>>,
    readout: Single<(&mut Text, &mut TextColor), With<Readout>>,
    more: Single<(&mut Text, &mut Node), (With<ReadoutDetail>, Without<Readout>)>,
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
    text.0 = format!(
        "{:.1} ms of {FRAME_BUDGET_MS:.1}  p99 {:.1}  worst {:.1}   game {}  render {}  GPU {}   {:.0} fps{}",
        work.median,
        work.high,
        work.worst,
        figure(spread(&recent, |s| Some(s.game_ms))),
        figure(spread(&recent, |s| s.render_ms)),
        figure(spread(&recent, |s| s.gpu_ms)),
        1000.0 / frame.median.max(0.001),
        if detail.0 { "" } else { "   F3 more" },
    );
    let (mut more, mut node) = more.into_inner();
    node.display = if detail.0 {
        Display::Flex
    } else {
        Display::None
    };
    if detail.0 {
        let mut lines = vec![
            "the line's colour is its p99: green under 12.5 ms, amber to 16.7, red over the budget"
                .to_string(),
            format!(
                "graphs: the last {GRAPH_FRAMES} frames, newest right; the line is the budget, the top a stutter ({STUTTER_MS:.0} ms)"
            ),
        ];
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
        more.0 = lines.join("\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreads_numbers_in_any_order() {
        // 1 to 100, shuffled: stepping by 37, which shares no factor with 100, visits each.
        let shuffled: Vec<f32> = (0..100).map(|n| (n * 37 % 100 + 1) as f32).collect();
        assert_ne!(shuffled[99], 100.0);
        let spread = Spread::of(shuffled).unwrap();
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
    fn graphs_a_bar_a_frame_against_the_budget() {
        let (width, height) = (4 * BAR_PX, 40);
        let mut pixels = vec![0; width * height * 4];
        let pixel = |pixels: &[u8], bar: usize, y: usize| {
            let at = (y * width + bar * BAR_PX) * 4;
            <[u8; 4]>::try_from(&pixels[at..at + 4]).unwrap()
        };
        let quarter = FRAME_BUDGET_MS / 2.0;
        // Five frames for four bars: the oldest is left off.
        let times = [Some(50.0), None, Some(quarter), Some(14.0), Some(100.0)];
        draw_graph(&times, width, height, &mut pixels);
        // A frame with no time is empty.
        assert_eq!(pixel(&pixels, 0, height - 1), GRAPH_BACKGROUND);
        // Half the budget is a quarter of the way up, the top being twice the budget.
        assert_eq!(pixel(&pixels, 1, 30), Standing::Within.rgba());
        assert_eq!(pixel(&pixels, 1, 29), GRAPH_BACKGROUND);
        assert_eq!(pixel(&pixels, 2, 39), Standing::Close.rgba());
        // A stutter fills the bar to the top and no further.
        assert_eq!(pixel(&pixels, 3, 0), Standing::Over.rgba());
        // The budget line runs halfway up, over the bars.
        for bar in 0..4 {
            assert_eq!(pixel(&pixels, bar, 20), BUDGET_LINE);
        }
        // Fewer frames than bars sit at the right.
        draw_graph(&[Some(100.0)], width, height, &mut pixels);
        assert_eq!(pixel(&pixels, 2, 0), GRAPH_BACKGROUND);
        assert_eq!(pixel(&pixels, 3, 0), Standing::Over.rgba());
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
