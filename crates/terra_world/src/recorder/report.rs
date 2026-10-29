//! The session report (D576, [launcher.md](../../../../docs/launcher.md#the-session-report-d576)):
//! with `--profile-report=<file>`, the frame recorder also keeps, in memory and bounded, each
//! name's time a frame as a histogram, the frames' lengths and the slowest frames, and writes
//! a report of about a hundred lines every ten seconds and when the game exits: the load's
//! longest steps, the frames while playing, the schedules, the slowest frames with what held
//! them, and the systems by 99th percentile and by total. It is what a Chrome trace read by a
//! script used to say, computed while the game runs, so no trace file is written at all.
//!
//! A frame is the time from one end of the game world's update to the next, as the player
//! sees it; a span counts in the frame it ends in, a system by its own time (what the spans
//! inside it on its thread did not take), a schedule by its wall clock. After Unreal's CSV
//! Profiler and PerfReportTool, which turn a session's cheap per-frame numbers into a short
//! report.

use super::{GAME_FRAME, Held, Names, RENDER_FRAME, SCHEDULE, Span};
use crate::debug::STUTTER_MS;
use crate::recorder::FREEZE_MS;
use bevy::platform::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;

/// How many of the load's steps, slowest frames and systems the report names.
const LOAD_STEPS: usize = 10;
const SLOWEST: usize = 10;
const SLOWEST_SYSTEMS: usize = 4;
const BY_P99: usize = 25;
const BY_TOTAL: usize = 15;
/// How often the report is written while the game runs, so a session ended by a crash or
/// a kill still leaves one.
pub(super) const EVERY_NS: u64 = 10_000_000_000;

/// The game's argument naming the report's file.
const FLAG: &str = "--profile-report=";

/// The game's argument ending a reported session that many seconds after the load.
const SECONDS: &str = "--profile-seconds=";

/// The file `--profile-report=<file>` names, if the game was given it.
pub fn asked() -> Option<PathBuf> {
    std::env::args()
        .find_map(|arg| arg.strip_prefix(FLAG).map(PathBuf::from))
        .filter(|path| !path.as_os_str().is_empty())
}

/// Whether `flag` is one of the report's, which the shell leaves to it.
pub fn takes(flag: &str) -> bool {
    flag.starts_with(FLAG) || flag.starts_with(SECONDS)
}

/// How long the session runs after its load, by the clock, if it was given
/// `--profile-seconds=<seconds>` (`cargo xtask profile walk`, whose frames are a sixtieth
/// of a second of play however long they take to draw).
pub(super) fn seconds() -> Option<f64> {
    std::env::args().find_map(|arg| arg.strip_prefix(SECONDS)?.parse().ok())
}

/// A histogram of times in nanoseconds: sixteen buckets a power of two (about 4% wide), from
/// a microsecond up; under a microsecond is the first bucket. Kept a name, so its median and
/// 99th percentile a frame are known without keeping every frame's.
#[derive(Default, Clone)]
pub(super) struct Hist {
    counts: Vec<u32>,
    /// How many frames it ran in.
    runs: u32,
    total: u64,
    max: u64,
}

const SUB: u32 = 16;
const FIRST_POWER: u32 = 10;

fn bucket(ns: u64) -> usize {
    if ns < 1 << FIRST_POWER {
        return 0;
    }
    let power = 63 - ns.leading_zeros();
    let sub = (ns >> (power - 4)) as u32 & (SUB - 1);
    ((power - FIRST_POWER) * SUB + sub + 1) as usize
}

/// The middle of a bucket.
fn middle(bucket: usize) -> u64 {
    if bucket == 0 {
        return 1 << (FIRST_POWER - 1);
    }
    let power = (bucket as u32 - 1) / SUB + FIRST_POWER;
    let sub = (bucket as u32 - 1) % SUB;
    let low = u64::from(SUB + sub) << (power - 4);
    low + (1 << (power - 4)) / 2
}

impl Hist {
    fn add(&mut self, ns: u64) {
        let at = bucket(ns);
        if self.counts.len() <= at {
            self.counts.resize(at + 1, 0);
        }
        self.counts[at] += 1;
        self.runs += 1;
        self.total += ns;
        self.max = self.max.max(ns);
    }

    /// The `q` quantile over `frames` frames, those it did not run in counting as 0; the
    /// worst is exact.
    fn quantile(&self, q: f64, frames: u32) -> u64 {
        let frames = frames.max(self.runs);
        if frames == 0 {
            return 0;
        }
        let rank = ((q * f64::from(frames)).ceil() as u32).clamp(1, frames);
        let zeros = frames - self.runs;
        if rank <= zeros {
            return 0;
        }
        if rank == frames {
            return self.max;
        }
        let mut left = rank - zeros;
        for (at, &count) in self.counts.iter().enumerate() {
            if left <= count {
                return middle(at).min(self.max);
            }
            left -= count;
        }
        self.max
    }
}

/// A slow frame, kept with its longest systems.
#[derive(Clone, Debug, PartialEq)]
struct Slow {
    frame: u32,
    ns: u64,
    longest: Vec<(u32, u64)>,
}

/// The report being gathered.
pub(super) struct Report {
    path: PathBuf,
    /// The load: its seconds and longest steps, once loaded.
    load: Option<(f32, Vec<(u32, u64)>)>,
    /// Each name's time in the frame not yet ended, and the render world's update's.
    open: HashMap<u32, u64>,
    open_render: u64,
    last_end: Option<u64>,
    /// Game frames ended since the start, counted as `FrameCount` counts them.
    frame: u32,
    /// The frames while playing (since the load, or the start if nothing loads).
    frames: Hist,
    game: Hist,
    render: Hist,
    names: Vec<Hist>,
    slowest: Vec<Slow>,
    pub(super) written: u64,
    /// When the session ends, on the recorder's clock (`--profile-seconds`), once loaded.
    pub(super) ends: Option<u64>,
}

impl Report {
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            path,
            load: None,
            open: HashMap::default(),
            open_render: 0,
            last_end: None,
            frame: 0,
            frames: Hist::default(),
            game: Hist::default(),
            render: Hist::default(),
            names: Vec::new(),
            slowest: Vec::new(),
            written: 0,
            ends: None,
        }
    }

    pub(super) fn path(&self) -> &PathBuf {
        &self.path
    }

    /// The load has ended, after `seconds` from the start, its steps' times summed in
    /// `totals`: the frames counted so far were the load's, so they are let go.
    pub(super) fn loaded(&mut self, seconds: f32, totals: &[u64], names: &Names) {
        let mut steps: Vec<(u32, u64)> = (totals.iter().enumerate())
            .skip(2)
            .filter(|&(name, &ns)| ns > 0 && !is_schedule(names, name as u32))
            .map(|(name, &ns)| (name as u32, ns))
            .collect();
        steps.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        steps.truncate(LOAD_STEPS);
        self.load = Some((seconds, steps));
        self.frames = Hist::default();
        self.game = Hist::default();
        self.render = Hist::default();
        self.names.clear();
        self.slowest.clear();
    }

    /// Counts spans just drained, in the order they ended.
    pub(super) fn add(&mut self, spans: &mut [Span], names: &Names) {
        spans.sort_by_key(|span| span.end);
        for span in spans.iter() {
            match span.name {
                GAME_FRAME => self.end_frame(span.end, span.end - span.start, names),
                RENDER_FRAME => self.open_render += span.end - span.start,
                name => {
                    // A schedule by its wall clock: the systems it waits on run elsewhere.
                    let ns = if is_schedule(names, name) {
                        span.end - span.start
                    } else {
                        span.own
                    };
                    *self.open.entry(name).or_default() += ns;
                }
            }
        }
    }

    fn end_frame(&mut self, end: u64, game_ns: u64, names: &Names) {
        self.frame += 1;
        let render_ns = std::mem::take(&mut self.open_render);
        let Some(start) = self.last_end.replace(end) else {
            self.open.clear();
            return;
        };
        let ns = end - start;
        self.frames.add(ns);
        self.game.add(game_ns);
        self.render.add(render_ns);
        let keep =
            self.slowest.len() < SLOWEST || self.slowest.last().is_some_and(|slow| ns > slow.ns);
        if keep {
            // Its systems, or its schedules in a build that times only those (`marks`).
            let timed = |schedules: bool| -> Vec<(u32, u64)> {
                (self.open.iter())
                    .filter(|&(&name, _)| is_schedule(names, name) == schedules)
                    .map(|(&name, &ns)| (name, ns))
                    .collect()
            };
            let mut longest = timed(false);
            if longest.is_empty() {
                longest = timed(true);
            }
            longest.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            longest.truncate(SLOWEST_SYSTEMS);
            let slow = Slow {
                frame: self.frame,
                ns,
                longest,
            };
            let at = self.slowest.partition_point(|kept| kept.ns >= ns);
            self.slowest.insert(at, slow);
            self.slowest.truncate(SLOWEST);
        }
        for (name, ns) in self.open.drain() {
            let at = name as usize;
            if self.names.len() <= at {
                self.names.resize(at + 1, Hist::default());
            }
            self.names[at].add(ns);
        }
    }

    /// The report, as written: `about` its first line (the GPU, the build).
    pub(super) fn text(&self, about: &str, names: &Names) -> String {
        let mut out = String::new();
        let n = self.frames.runs;
        let shown = |name: u32| shown(names, name);
        let _ = writeln!(out, "Terra session report: {about}");
        let _ = writeln!(
            out,
            "Arguments: {}",
            std::env::args().skip(1).collect::<Vec<_>>().join(" ")
        );
        match &self.load {
            Some((seconds, steps)) => {
                let _ = writeln!(
                    out,
                    "\nLoad: {seconds:.1} s from the start to the first playable frame. Its longest steps, summed over every thread:"
                );
                for &(name, ns) in steps {
                    let _ = writeln!(out, "  {:>7.2} s  {}", ns as f64 / 1e9, shown(name));
                }
            }
            None => {
                let _ = writeln!(out, "\nLoad: not ended, or nothing loaded.");
            }
        }
        if n == 0 {
            let _ = writeln!(
                out,
                "\nNo frames timed while playing: was the game built with the frame recorder (`hitch-log`; `cargo xtask profile` builds it)?"
            );
            return out;
        }
        let over = |limit_ms: f32| {
            let from = bucket((limit_ms * 1e6) as u64);
            self.frames.counts.iter().skip(from).sum::<u32>()
        };
        let seconds = self.frames.total as f64 / 1e9;
        let _ = writeln!(
            out,
            "\nPlaying: {n} frames over {seconds:.1} s ({:.0} a second). Frame median {}, 99th {}, worst {}; {} hitches (over {STUTTER_MS:.1} ms), {} freezes ({FREEZE_MS:.0} ms or more).",
            f64::from(n) / seconds.max(1e-9),
            ms(self.frames.quantile(0.5, n)),
            ms(self.frames.quantile(0.99, n)),
            ms(self.frames.max),
            over(STUTTER_MS),
            over(FREEZE_MS),
        );
        for (what, hist) in [
            ("the game world's update", &self.game),
            ("the render world's, extraction included", &self.render),
        ] {
            let _ = writeln!(
                out,
                "  {what}: median {}, 99th {}, worst {}",
                ms(hist.quantile(0.5, n)),
                ms(hist.quantile(0.99, n)),
                ms(hist.max),
            );
        }
        self.schedules(&mut out, names);
        let _ = writeln!(
            out,
            "\nSlowest frames, each with its longest systems (both worlds; [render] the render world's):"
        );
        for slow in &self.slowest {
            let longest: Vec<String> = (slow.longest.iter())
                .map(|&(name, ns)| format!("{} {}", shown(name), ms(ns)))
                .collect();
            let _ = writeln!(
                out,
                "  frame {:>6} {:>9}  {}",
                slow.frame,
                ms(slow.ns),
                longest.join(", ")
            );
        }
        let systems: Vec<(u32, &Hist)> = (self.names.iter().enumerate())
            .filter(|&(name, hist)| hist.runs > 0 && !is_schedule(names, name as u32))
            .map(|(name, hist)| (name as u32, hist))
            .collect();
        let _ = writeln!(
            out,
            "\nSystems by 99th percentile a frame (a frame it did not run in counts 0):\n  {:>9} {:>9} {:>9}  system",
            "median", "99th", "worst"
        );
        let mut by_p99: Vec<(u32, u64, u64, u64)> = (systems.iter())
            .map(|&(name, h)| (name, h.quantile(0.5, n), h.quantile(0.99, n), h.max))
            .collect();
        by_p99.sort_by(|a, b| b.2.cmp(&a.2).then(b.3.cmp(&a.3)).then(a.0.cmp(&b.0)));
        for &(name, p50, p99, max) in by_p99.iter().take(BY_P99) {
            let _ = writeln!(
                out,
                "  {:>9} {:>9} {:>9}  {}",
                ms(p50),
                ms(p99),
                ms(max),
                shown(name)
            );
        }
        let _ = writeln!(
            out,
            "\nSystems by total while playing (the share of the frames' time; systems on other threads overlap):"
        );
        let mut by_total = systems;
        by_total.sort_by(|a, b| b.1.total.cmp(&a.1.total).then(a.0.cmp(&b.0)));
        for &(name, hist) in by_total.iter().take(BY_TOTAL) {
            let _ = writeln!(
                out,
                "  {:>7.2} s {:>5.1}%  {}",
                hist.total as f64 / 1e9,
                100.0 * hist.total as f64 / self.frames.total.max(1) as f64,
                shown(name)
            );
        }
        out
    }

    /// Each world's schedules, longest median first, on a line a world.
    fn schedules(&self, out: &mut String, names: &Names) {
        let n = self.frames.runs;
        let _ = writeln!(out, "\nSchedules, median (99th) a frame:");
        for world in [Held::Game, Held::Render] {
            let mut rows: Vec<(String, u64, u64)> = (self.names.iter().enumerate())
                .filter(|&(name, hist)| {
                    hist.runs > 0
                        && is_schedule(names, name as u32)
                        && names
                            .world
                            .get(name)
                            .copied()
                            .flatten()
                            .unwrap_or(Held::Game)
                            == world
                })
                .map(|(name, hist)| {
                    let label = names.shown.get(name).map_or("", String::as_str);
                    let label = label
                        .trim_end_matches(SCHEDULE)
                        .trim_matches('"')
                        .to_string();
                    (label, hist.quantile(0.5, n), hist.quantile(0.99, n))
                })
                .collect();
            rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            let rows: Vec<String> = (rows.iter().take(12))
                .map(|(label, p50, p99)| format!("{label} {} ({})", ms(*p50), ms(*p99)))
                .collect();
            let world = match world {
                Held::Game => "game",
                Held::Render => "render",
            };
            let _ = writeln!(out, "  {world}: {}", rows.join(", "));
        }
    }
}

pub(super) fn is_schedule(names: &Names, name: u32) -> bool {
    names
        .shown
        .get(name as usize)
        .is_some_and(|shown| shown.ends_with(SCHEDULE))
}

/// A name as the report shows it: the paths in its generic parameters cut to their last
/// part, so each instance of a generic system shows apart and short; marked when the render
/// world's.
fn shown(names: &Names, name: u32) -> String {
    let full = names.shown.get(name as usize).map_or("", String::as_str);
    let render = names.world.get(name as usize).copied().flatten() == Some(Held::Render);
    format!("{}{}", short(full), if render { " [render]" } else { "" })
}

/// `a::B<c::D, e::F<g::H>>::run` as `a::B<D, F<H>>::run`.
fn short(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut depth = 0u32;
    // Where the path being read began in `out`.
    let mut path_from = 0;
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ':' && chars.peek() == Some(&':') && depth > 0 {
            chars.next();
            out.truncate(path_from);
            continue;
        }
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ => {}
        }
        out.push(c);
        if !(c.is_alphanumeric() || c == '_' || c == ':') {
            path_from = out.len();
        }
    }
    out
}

fn ms(ns: u64) -> String {
    format!("{:.2} ms", ns as f64 / 1e6)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    fn names(shown: &[(&str, Option<Held>)]) -> Names {
        let mut names = Names::default();
        for frame in ["game", "render"] {
            names.shown.push(frame.into());
            names.world.push(None);
        }
        for &(name, world) in shown {
            names.shown.push(name.into());
            names.world.push(world);
        }
        names
    }

    fn span(name: u32, start_ms: u64, end_ms: u64) -> Span {
        Span {
            name,
            start: start_ms * MS,
            end: end_ms * MS,
            own: (end_ms - start_ms) * MS,
        }
    }

    #[test]
    fn a_slow_frame_with_no_system_timed_names_its_schedules() {
        let names = names(&[("Update schedule", Some(Held::Game))]);
        let mut report = Report::new(PathBuf::new());
        let mut spans = vec![
            span(GAME_FRAME, 0, 1),
            span(2, 1, 40),
            span(GAME_FRAME, 1, 50),
        ];
        report.add(&mut spans, &names);
        assert_eq!(report.slowest[0].longest, vec![(2, 39 * MS)]);
    }

    #[test]
    fn the_shell_leaves_the_reports_arguments_to_it() {
        assert!(takes("--profile-report=report.txt"));
        assert!(takes("--profile-seconds=60"));
        assert!(!takes("--profile"));
    }

    #[test]
    fn a_histogram_knows_its_quantiles_within_its_buckets() {
        let mut hist = Hist::default();
        for ms in 1..=100u64 {
            hist.add(ms * MS);
        }
        let near = |got: u64, want: u64| got.abs_diff(want) * 100 <= want * 5;
        assert!(
            near(hist.quantile(0.5, 100), 50 * MS),
            "{}",
            hist.quantile(0.5, 100)
        );
        assert!(near(hist.quantile(0.99, 100), 99 * MS));
        assert_eq!(hist.quantile(1.0, 100), 100 * MS, "the worst is exact");
        // Over 200 frames, half without it: its median is 0, its 99th the 98th of its own.
        assert_eq!(hist.quantile(0.5, 200), 0);
        assert!(near(hist.quantile(0.99, 200), 98 * MS));
        for ns in [0, 1023, 1024, 5_000, 123_456_789, 90_000_000_000] {
            let mid = middle(bucket(ns));
            assert!(
                ns < 1024 || mid.abs_diff(ns) * 100 <= ns * 4,
                "{ns} in {mid}"
            );
        }
    }

    #[test]
    fn a_frame_counts_what_ended_in_it_and_the_report_says_so() {
        // 2 a game system, 3 a render system, 4 the Update schedule.
        let names = names(&[
            ("game::slow", Some(Held::Game)),
            ("render::prepare<Foo>", Some(Held::Render)),
            ("\"Update\" schedule", Some(Held::Game)),
        ]);
        let mut report = Report::new("r.txt".into());
        let mut frames = Vec::new();
        // Twenty frames of 10 ms, then one of 150 ms that game::slow held.
        for i in 0..21u64 {
            let start = i * 10;
            let long = if i == 20 { 140 } else { 0 };
            frames.extend([
                span(4, start, start + 6 + long),
                span(2, start + 1, start + 5 + long),
                span(3, start + 2, start + 4),
                span(RENDER_FRAME, start + 1, start + 8),
                span(GAME_FRAME, start, start + 9 + long),
            ]);
        }
        // Drained in two batches, out of order within each.
        frames.reverse();
        let (late, early) = frames.split_at_mut(40);
        report.add(early, &names);
        report.add(late, &names);
        // The first frame only starts the count.
        assert_eq!(report.frames.runs, 20);
        assert_eq!(report.frames.max, 150 * MS);
        assert_eq!(report.slowest[0].frame, 21);
        assert_eq!(report.slowest[0].longest[0], (2, 144 * MS));
        let text = report.text("a test", &names);
        assert!(text.contains("20 frames"), "{text}");
        assert!(text.contains("worst 150.00 ms; 1 hitches"), "{text}");
        assert!(text.contains("1 freezes"), "{text}");
        assert!(text.contains("game: Update"), "{text}");
        assert!(
            text.contains("frame     21 150.00 ms  game::slow 144.00 ms, render::prepare<Foo> [render] 2.00 ms"),
            "{text}"
        );
        assert_eq!(
            short("bevy_asset::Assets<bevy_pbr::Material<x::Y>, z::W>::track (commands)"),
            "bevy_asset::Assets<Material<Y>, W>::track (commands)"
        );
        assert!(
            !text.contains("Update\" schedule 1"),
            "schedules are not systems: {text}"
        );
    }

    #[test]
    fn the_load_lets_go_of_its_frames_and_names_its_steps() {
        let names = names(&[("terra::mipmaps", None), ("game::tick", Some(Held::Game))]);
        let mut report = Report::new("r.txt".into());
        let mut load = vec![
            span(GAME_FRAME, 0, 1),
            span(2, 1, 900),
            span(GAME_FRAME, 1, 950),
        ];
        report.add(&mut load, &names);
        report.loaded(1.0, &[0, 0, 899 * MS, 0], &names);
        let mut play = vec![span(3, 951, 952), span(GAME_FRAME, 950, 960)];
        report.add(&mut play, &names);
        assert_eq!(report.frames.runs, 1, "the load's frames let go");
        assert_eq!(report.frames.max, 10 * MS);
        let text = report.text("a test", &names);
        assert!(text.contains("Load: 1.0 s"), "{text}");
        assert!(text.contains("0.90 s  terra::mipmaps"), "{text}");
    }
}
