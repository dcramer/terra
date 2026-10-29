//! The frame recorder (D576, [launcher.md](../../../docs/launcher.md#the-hitch-log-in-every-build-d576)):
//! what ran long in a slow frame, said in the log of every build, so a freeze is explained
//! by the session that froze rather than by a trace taken later that never sees it.
//!
//! A `tracing` layer takes the enter and exit of Bevy's per-system spans (Bevy's `trace`
//! feature, compiled in by the `hitch-log` feature: in every build it cost `world` 9.7 to
//! 11.5 ms a frame, so it is not), of each world's update (`main app`, and the render thread's
//! `sub app` RenderApp) and of any span of ours (a `terra…` target), and keeps them in a
//! buffer a thread; a system drains them at each frame's start. A frame of 100 ms or more
//! (a freeze) gets a warning naming the world that held it, its five longest systems and
//! the time outside any of them; the hitches' warning (`debug`) names the system longest in
//! most hitches; the `Loaded in` line (`loading`) names the load's longest systems.
//!
//! After Unreal's hitch log (`stat DumpHitches`), which writes a frame over
//! `t.HitchFrameTimeThreshold` to the log with the stats that ran long in it. With
//! `--profile-report=<file>` it also writes the session report (`report`).

mod report;

use crate::debug::STUTTER_MS;
use bevy::{
    diagnostic::FrameCount,
    log::{
        BoxedLayer,
        tracing::{
            Subscriber,
            field::{Field, Visit},
            span::{Attributes, Id},
        },
        tracing_subscriber::{Layer, layer::Context, registry::LookupSpan},
    },
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::{RenderApp, renderer::RenderAdapterInfo},
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

/// A frame this long is a freeze, said on its own line: a player sees the game stop.
pub const FREEZE_MS: f32 = 100.0;
/// How many systems a freeze, or the load, names.
const SHOWN: usize = 5;
/// How long spans are kept to explain a frame: longer than any frame worth explaining
/// would leave the frame's start gone, so a freeze longer is explained by what is left.
const KEEP_NS: u64 = 10_000_000_000;
/// The most spans kept, should a few seconds hold more than they ought.
const KEEP_MOST: usize = 1 << 19;

/// The two worlds' updates, which bound their frames; names count from after them.
const GAME_FRAME: u32 = 0;
const RENDER_FRAME: u32 = 1;
/// How a system's commands, applied apart from it, are named.
const COMMANDS: &str = " (commands)";
/// How a schedule is named, after its label.
const SCHEDULE: &str = " schedule";

/// Which world ran a system, so a freeze names only the systems of the world that held it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Game,
    Render,
}

impl Held {
    fn said(self) -> &'static str {
        match self {
            Held::Game => "the game world",
            Held::Render => "the render world",
        }
    }
}

/// The recorder, for the systems that read it; the layer holds the same.
#[derive(Resource, Clone)]
pub struct Recorder(Arc<Shared>);

/// For the log's `custom_layer` (`record::layer`): the layer, with the recorder and its
/// systems put in the app.
pub fn layer(app: &mut App) -> BoxedLayer {
    let shared = Arc::new(Shared::new());
    app.insert_resource(Recorder(shared.clone()))
        .add_plugins(Worlds(shared.clone()))
        .add_systems(First, explain)
        .add_systems(Last, write_at_exit);
    if shared.report.lock().is_ok_and(|report| report.is_some()) && !cfg!(feature = "hitch-log") {
        warn!(
            "--profile-report: this build has no spans to report (Bevy's `trace`); build with the `hitch-log` feature, as `cargo xtask profile` does (D576)."
        );
    }
    Box::new(Spans(shared))
}

/// Whether the session report was asked for (`--profile-report=<file>`), which wants every
/// system's span (`record::filter`).
pub fn reporting() -> bool {
    report::asked().is_some()
}

pub use report::takes;

static OWNERS: AtomicUsize = AtomicUsize::new(0);

struct Shared {
    /// The thread buffers this recorder's (a test makes several recorders in one process).
    owner: usize,
    epoch: Instant,
    names: Mutex<Names>,
    threads: Mutex<Vec<Arc<Mutex<Vec<Span>>>>>,
    state: Mutex<State>,
    /// The session report, when asked for.
    report: Mutex<Option<report::Report>>,
}

/// A span's name as shown, and its world when known (ours and unknown ones are either).
#[derive(Default)]
struct Names {
    shown: Vec<String>,
    index: HashMap<String, u32>,
    world: Vec<Option<Held>>,
    render: HashSet<String>,
}

/// One run of a span, in nanoseconds from the recorder's start, and its own time: what
/// the spans run inside it on its thread (a schedule's inside `Main`) did not take.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Span {
    name: u32,
    start: u64,
    end: u64,
    own: u64,
}

#[derive(Default)]
struct State {
    recent: VecDeque<Span>,
    /// Each name's time so far, for the load's longest.
    totals: Vec<u64>,
    last_game_end: Option<u64>,
    /// Frames ended and not yet explained: start, end, and the game world's update.
    frames: Vec<(u64, u64, u64)>,
    /// Since the hitches were last said: how many, and how often each system was longest.
    hitches: u32,
    longest_in: HashMap<u32, u32>,
}

/// What held a slow frame.
#[derive(Debug, PartialEq)]
struct Slow {
    frame_ns: u64,
    held: Held,
    longest: Vec<(u32, u64)>,
    outside_ns: u64,
}

impl Shared {
    fn new() -> Self {
        let mut names = Names::default();
        for frame in ["the game world's update", "the render world's update"] {
            names.shown.push(frame.into());
            names.world.push(None);
        }
        Self {
            owner: OWNERS.fetch_add(1, Ordering::Relaxed),
            epoch: Instant::now(),
            names: Mutex::new(names),
            threads: Mutex::default(),
            state: Mutex::default(),
            report: Mutex::new(report::asked().map(report::Report::new)),
        }
    }

    fn report(&self) -> MutexGuard<'_, Option<report::Report>> {
        self.report
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Writes the session report, if asked for, when `due` says so; says once if it cannot.
    fn write_report(&self, about: &str, due: impl Fn(u64, u64) -> bool) {
        let now = self.now();
        let mut report = self.report();
        let Some(report) = report.as_mut().filter(|report| due(report.written, now)) else {
            return;
        };
        let first = report.written == 0;
        report.written = now;
        let text = report.text(about, &self.names());
        let part = report.path().with_extension("part");
        let written =
            std::fs::write(&part, text).and_then(|()| std::fs::rename(&part, report.path()));
        if let (Err(why), true) = (written, first) {
            warn!(
                "Could not write the session report to {} ({why}); give --profile-report= a folder that exists (D576).",
                report.path().display()
            );
        }
    }

    fn now(&self) -> u64 {
        self.epoch.elapsed().as_nanos() as u64
    }

    fn names(&self) -> MutexGuard<'_, Names> {
        self.names
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The number a name is kept under. `system` is the system it is (its commands are
    /// it too), whose world is looked up; a span of ours, not a system, is either world's.
    fn name(&self, shown: String, system: Option<&str>) -> u32 {
        let mut names = self.names();
        if let Some(&index) = names.index.get(&shown) {
            return index;
        }
        let index = names.shown.len() as u32;
        let world = system.map(|system| match names.render.contains(system) {
            true => Held::Render,
            false => Held::Game,
        });
        names.world.push(world);
        names.index.insert(shown.clone(), index);
        names.shown.push(shown);
        index
    }

    /// The render world's schedules and their systems, read once the app is built.
    fn mark_render(&self, render: HashSet<String>) {
        let mut names = self.names();
        let names = &mut *names;
        for (shown, world) in names.shown.iter().zip(names.world.iter_mut()) {
            let system = (shown.strip_suffix(COMMANDS))
                .or(shown.strip_suffix(SCHEDULE))
                .unwrap_or(shown);
            if world.is_some() && render.contains(system) {
                *world = Some(Held::Render);
            }
        }
        names.render = render;
    }

    /// Moves every thread's finished spans to the recent ones and counts them.
    fn drain(&self) {
        let threads: Vec<_> = self
            .threads
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let mut report = self.report();
        let mut batch = Vec::new();
        let mut state = self.state();
        let state = &mut *state;
        for thread in threads {
            let mut done = thread
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if report.is_some() {
                batch.extend_from_slice(&done);
            }
            for span in done.drain(..) {
                let name = span.name as usize;
                if state.totals.len() <= name {
                    state.totals.resize(name + 1, 0);
                }
                state.totals[name] += span.own;
                if span.name == GAME_FRAME {
                    if let Some(last) = state.last_game_end {
                        state.frames.push((last, span.end, span.end - span.start));
                    }
                    state.last_game_end = Some(span.end);
                }
                state.recent.push_back(span);
            }
        }
        let now = self.now();
        while state.recent.len() > KEEP_MOST
            || state
                .recent
                .front()
                .is_some_and(|oldest| now.saturating_sub(oldest.end) > KEEP_NS)
        {
            state.recent.pop_front();
        }
        if let Some(report) = report.as_mut() {
            report.add(&mut batch, &self.names());
        }
    }

    /// The frames ended since last asked that were over the hitch length, explained; the
    /// longest system of each counted for the hitches' warning.
    fn slow_frames(&self) -> Vec<Slow> {
        let worlds = self.names().world.clone();
        let mut state = self.state();
        let frames = std::mem::take(&mut state.frames);
        let mut slow = Vec::new();
        for (start, end, game_ns) in frames {
            if ((end - start) as f32) < STUTTER_MS * 1e6 {
                continue;
            }
            let explained = explain_frame(&state.recent, &worlds, start, end, game_ns);
            state.hitches += 1;
            if let Some(&(name, _)) = explained.longest.first() {
                *state.longest_in.entry(name).or_default() += 1;
            }
            slow.push(explained);
        }
        slow
    }

    fn shown(&self, name: u32) -> String {
        self.names()
            .shown
            .get(name as usize)
            .cloned()
            .unwrap_or_default()
    }
}

/// Which world held the frame from `start` to `end` (the game world's update took
/// `game_ns` of it), its longest systems in it and the time it spent in none of them.
fn explain_frame(
    recent: &VecDeque<Span>,
    worlds: &[Option<Held>],
    start: u64,
    end: u64,
    game_ns: u64,
) -> Slow {
    let within = |span: &Span| (span.start.max(start), span.end.min(end));
    let overlapping = || {
        recent
            .iter()
            .filter(move |span| span.end > start && span.start < end)
    };
    let render_ns = overlapping()
        .filter(|span| span.name == RENDER_FRAME)
        .map(|span| {
            let (from, to) = within(span);
            to - from
        })
        .max()
        .unwrap_or(0);
    let held = if render_ns > game_ns {
        Held::Render
    } else {
        Held::Game
    };
    let mut by_name: HashMap<u32, u64> = HashMap::default();
    let mut runs: Vec<(u64, u64)> = Vec::new();
    for span in overlapping() {
        let world = worlds.get(span.name as usize).copied().flatten();
        if span.name <= RENDER_FRAME || world.is_some_and(|world| world != held) {
            continue;
        }
        let (from, to) = within(span);
        // Its own time, as much of it as fell in the frame.
        *by_name.entry(span.name).or_default() += span.own.min(to - from);
        runs.push((from, to));
    }
    let mut longest: Vec<(u32, u64)> = by_name.into_iter().collect();
    longest.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    longest.truncate(SHOWN);
    Slow {
        frame_ns: end - start,
        held,
        longest,
        outside_ns: (end - start) - covered(runs),
    }
}

/// How much of the time the runs cover between them, counting time two share once.
fn covered(mut runs: Vec<(u64, u64)>) -> u64 {
    runs.sort_unstable();
    let mut total = 0;
    let mut reach = 0;
    for (from, to) in runs {
        let from = from.max(reach);
        if to > from {
            total += to - from;
            reach = to;
        }
    }
    total
}

fn ms(ns: u64) -> String {
    let ms = ns as f64 / 1e6;
    if ms >= 10.0 {
        format!("{ms:.0} ms")
    } else {
        format!("{ms:.1} ms")
    }
}

impl Recorder {
    /// The load's longest systems so far, said as `name 1.7 s, …`: their time summed over
    /// every frame and thread since the start.
    pub fn longest_so_far(&self) -> String {
        self.0.drain();
        let mut totals: Vec<(u32, u64)> = {
            let state = self.0.state();
            state
                .totals
                .iter()
                .enumerate()
                .skip(2)
                .map(|(name, &ns)| (name as u32, ns))
                .collect()
        };
        totals.sort_by_key(|&(_, ns)| std::cmp::Reverse(ns));
        totals
            .iter()
            .take(SHOWN)
            .filter(|(_, ns)| *ns > 0)
            .map(|&(name, ns)| format!("{} {:.2} s", self.0.shown(name), ns as f64 / 1e9))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The load ended `seconds` after the start: the session report names its longest steps
    /// and times the frames from here.
    pub fn loaded(&self, seconds: f32) {
        self.0.drain();
        let mut report = self.0.report();
        if let Some(report) = report.as_mut() {
            let totals = self.0.state().totals.clone();
            report.loaded(seconds, &totals, &self.0.names());
            report.ends = report::seconds().map(|after| self.0.now() + (after * 1e9) as u64);
        }
    }

    /// The system longest in most hitches since last asked, how many of them, and of how
    /// many; forgotten once asked.
    pub fn take_hitches(&self) -> Option<(String, u32, u32)> {
        let (name, count, of) = {
            let mut state = self.0.state();
            let of = std::mem::take(&mut state.hitches);
            let longest = std::mem::take(&mut state.longest_in);
            let (name, count) = longest
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))?;
            (name, count, of)
        };
        Some((self.0.shown(name), count, of))
    }
}

/// Explains each freeze that has ended, as a warning (D576).
fn explain(
    recorder: Res<Recorder>,
    frame: Res<FrameCount>,
    adapter: Option<Res<RenderAdapterInfo>>,
    mut exits: MessageWriter<AppExit>,
) {
    recorder.0.drain();
    let slow = recorder.0.slow_frames();
    recorder
        .0
        .write_report(&about(adapter.as_deref()), |written, now| {
            now.saturating_sub(written) >= report::EVERY_NS
        });
    let ends = recorder.0.report().as_ref().and_then(|report| report.ends);
    if ends.is_some_and(|ends| recorder.0.now() >= ends) {
        exits.write(AppExit::Success);
    }
    // Frame times mean nothing drawn on the CPU, as `debug` says.
    if adapter.is_some_and(|adapter| format!("{:?}", adapter.device_type) == "Cpu") {
        return;
    }
    for slow in slow {
        if (slow.frame_ns as f32) < FREEZE_MS * 1e6 {
            continue;
        }
        warn!(
            "{}",
            freeze_line(&recorder.0, frame.0.saturating_sub(1), &slow)
        );
    }
}

/// The session report's first line: the GPU and the build.
fn about(adapter: Option<&RenderAdapterInfo>) -> String {
    let gpu = adapter.map_or("no GPU yet".to_string(), |adapter| {
        format!("{} ({:?})", adapter.name, adapter.backend)
    });
    let build = if cfg!(debug_assertions) {
        "debug"
    } else {
        "optimised"
    };
    format!("{gpu}, the {build} build")
}

/// Writes the session report as the game exits.
fn write_at_exit(
    recorder: Res<Recorder>,
    mut exits: MessageReader<AppExit>,
    adapter: Option<Res<RenderAdapterInfo>>,
) {
    if exits.read().next().is_some() {
        recorder.0.drain();
        recorder
            .0
            .write_report(&about(adapter.as_deref()), |_, _| true);
    }
}

fn freeze_line(shared: &Shared, frame: u32, slow: &Slow) -> String {
    let longest = slow
        .longest
        .iter()
        .map(|&(name, ns)| format!("{} {}", shared.shown(name), ms(ns)))
        .collect::<Vec<_>>();
    format!(
        "freeze: frame {frame} took {}, held by {}: {}; {} outside its systems (a wait on the GPU, a lock or the other world, or work on the task pools).",
        ms(slow.frame_ns),
        slow.held.said(),
        if longest.is_empty() {
            "no system timed".to_string()
        } else {
            longest.join(", ")
        },
        ms(slow.outside_ns),
    )
}

/// Reads which systems are the render world's once every plugin has added its own.
struct Worlds(Arc<Shared>);

impl Plugin for Worlds {
    fn build(&self, _: &mut App) {}

    fn cleanup(&self, app: &mut App) {
        let Some(render) = app.get_sub_app(RenderApp) else {
            return;
        };
        let Some(schedules) = render.world().get_resource::<Schedules>() else {
            return;
        };
        let mut names = HashSet::default();
        for (_, schedule) in schedules.iter() {
            names.insert(format!("{:?}", schedule.label()));
            for (_, system, _) in schedule.graph().systems.iter() {
                names.insert(system.name().to_string());
            }
            if let Ok(systems) = schedule.systems() {
                names.extend(systems.map(|(_, system)| system.name().to_string()));
            }
        }
        self.0.mark_render(names);
    }
}

/// The layer: each span of interest tagged when made, timed on its thread when run.
struct Spans(Arc<Shared>);

#[derive(Clone, Copy)]
struct Tag(u32);

struct Open {
    id: u64,
    name: u32,
    start: u64,
    /// The time spans run inside it on its thread took.
    inner: u64,
}

/// One thread's spans for one recorder: those open, and those done since the last drain.
struct Local {
    owner: usize,
    open: Vec<Open>,
    done: Arc<Mutex<Vec<Span>>>,
}

thread_local! {
    static LOCAL: RefCell<Vec<Local>> = const { RefCell::new(Vec::new()) };
}

impl Spans {
    fn on_thread(&self, f: impl FnOnce(&mut Local)) {
        LOCAL.with(|local| {
            let Ok(mut locals) = local.try_borrow_mut() else {
                return;
            };
            let at = match locals.iter().position(|local| local.owner == self.0.owner) {
                Some(at) => at,
                None => {
                    let done: Arc<Mutex<Vec<Span>>> = Arc::default();
                    self.0
                        .threads
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .push(done.clone());
                    locals.push(Local {
                        owner: self.0.owner,
                        open: Vec::new(),
                        done,
                    });
                    locals.len() - 1
                }
            };
            f(&mut locals[at]);
        });
    }
}

/// A span's `name` field, as text.
#[derive(Default)]
struct NameField(Option<String>);

impl Visit for NameField {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "name" {
            self.0 = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "name" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Spans {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let metadata = attrs.metadata();
        let target = metadata.target();
        let named = || {
            let mut name = NameField::default();
            attrs.record(&mut name);
            name.0
        };
        let name = match metadata.name() {
            "system" if target.starts_with("bevy_ecs") => {
                named().map(|name| self.0.name(name.clone(), Some(&name)))
            }
            "system_commands" if target.starts_with("bevy_ecs") => {
                named().map(|name| self.0.name(format!("{name}{COMMANDS}"), Some(&name)))
            }
            // A schedule is said by its label, and is its world's as a system is.
            "schedule" if target.starts_with("bevy_ecs") => {
                named().map(|label| self.0.name(format!("{label}{SCHEDULE}"), Some(&label)))
            }
            "main app" if target.starts_with("bevy_app") => Some(GAME_FRAME),
            "sub app" if named().as_deref() == Some("RenderApp") => Some(RENDER_FRAME),
            span if target.starts_with("terra") => Some(match named() {
                Some(name) => self.0.name(format!("{span} {name}"), None),
                None => self.0.name(span.to_string(), None),
            }),
            _ => None,
        };
        if let (Some(name), Some(span)) = (name, ctx.span(id)) {
            span.extensions_mut().insert(Tag(name));
        }
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        let Some(Tag(name)) = ctx
            .span(id)
            .and_then(|span| span.extensions().get::<Tag>().copied())
        else {
            return;
        };
        let start = self.0.now();
        self.on_thread(|local| {
            local.open.push(Open {
                id: id.into_u64(),
                name,
                start,
                inner: 0,
            });
        });
    }

    fn on_exit(&self, id: &Id, _: Context<'_, S>) {
        let id = id.into_u64();
        self.on_thread(|local| {
            let Some(at) = local.open.iter().rposition(|open| open.id == id) else {
                return;
            };
            let open = local.open.remove(at);
            let end = self.0.now();
            let took = end.saturating_sub(open.start);
            // The span it ran inside on this thread did not spend this time itself.
            if let Some(outer) = local.open.get_mut(at.wrapping_sub(1)) {
                outer.inner += took;
            }
            let span = Span {
                name: open.name,
                start: open.start,
                end,
                own: took.saturating_sub(open.inner),
            };
            local
                .done
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(span);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::log::{
        tracing::{self, info_span},
        tracing_subscriber::{layer::SubscriberExt, registry::Registry},
    };

    const MS: u64 = 1_000_000;

    fn span(name: u32, start_ms: u64, end_ms: u64) -> Span {
        Span {
            name,
            start: start_ms * MS,
            end: end_ms * MS,
            own: (end_ms - start_ms) * MS,
        }
    }

    #[test]
    fn a_frame_names_its_longest_systems_of_the_world_that_held_it() {
        // Names 2 to 4 the game's, 5 the render world's, 6 ours (either world).
        let worlds = [
            None,
            None,
            Some(Held::Game),
            Some(Held::Game),
            Some(Held::Game),
            Some(Held::Render),
            None,
        ];
        let recent: VecDeque<Span> = [
            span(GAME_FRAME, 1000, 2900),
            // Begun in the frame before: only its part in this frame counts.
            span(2, 950, 1010),
            span(3, 1100, 2800),
            // Two runs of one system on two threads at once, summed.
            span(4, 1100, 1130),
            span(4, 1110, 1140),
            span(6, 1200, 1300),
            span(5, 1000, 1008),
            span(RENDER_FRAME, 995, 1010),
            // After the frame: not in it.
            span(2, 3100, 3200),
        ]
        .into();
        let slow = explain_frame(&recent, &worlds, 1000 * MS, 3000 * MS, 1900 * MS);
        assert_eq!(slow.held, Held::Game);
        assert_eq!(slow.frame_ns, 2_000_000_000);
        let names: Vec<u32> = slow.longest.iter().map(|&(name, _)| name).collect();
        assert_eq!(
            names,
            [3, 6, 4, 2],
            "longest first, the render world's left out"
        );
        assert_eq!(slow.longest[2].1, 60_000_000);
        // Covered: 1000-1010, 1100-2800 (the rest inside it): 1710 ms of 2000.
        assert_eq!(slow.outside_ns, 290_000_000);
    }

    #[test]
    fn a_frame_the_render_world_held_names_the_render_worlds_systems() {
        let worlds = [None, None, Some(Held::Game), Some(Held::Render)];
        let recent: VecDeque<Span> = [
            span(GAME_FRAME, 0, 5),
            span(2, 1, 4),
            span(RENDER_FRAME, 2, 400),
            span(3, 10, 390),
        ]
        .into();
        let slow = explain_frame(&recent, &worlds, 0, 400 * MS, 5 * MS);
        assert_eq!(slow.held, Held::Render);
        assert_eq!(slow.longest, vec![(3, 380_000_000)]);
    }

    #[test]
    fn a_schedule_inside_another_counts_only_once() {
        let shared = Arc::new(Shared::new());
        let subscriber = Registry::default().with(Spans(shared.clone()));
        let schedule = |name: &str| info_span!(target: "bevy_ecs::schedule::schedule", "schedule", name = ?name);
        tracing::subscriber::with_default(subscriber, || {
            let _main = schedule("Main").entered();
            let _update = schedule("Update").entered();
            std::thread::sleep(std::time::Duration::from_millis(4));
        });
        shared.drain();
        let state = shared.state();
        let own: Vec<(String, u64)> = state
            .recent
            .iter()
            .map(|span| (shared.shown(span.name), span.own))
            .collect();
        assert_eq!(own[0].0, "\"Update\" schedule");
        assert_eq!(own[1].0, "\"Main\" schedule");
        assert!(own[0].1 >= 4_000_000, "{own:?}");
        assert!(own[1].1 < 1_000_000, "Main only ran Update: {own:?}");
    }

    #[test]
    fn the_layer_times_systems_and_frames_on_every_thread() {
        let shared = Arc::new(Shared::new());
        let subscriber = Registry::default().with(Spans(shared.clone()));
        tracing::subscriber::with_default(subscriber, || {
            let frame = info_span!(target: "bevy_app::sub_app", "main app").entered();
            let system = info_span!(target: "bevy_ecs::system", "system", name = "game::slow");
            let other = info_span!(target: "bevy_ecs::system", "system", name = "game::quick");
            let ignored =
                info_span!(target: "bevy_ecs::schedule::executor", "multithreaded executor");
            {
                let _system = system.enter();
                let _ignored = ignored.enter();
                std::thread::sleep(std::time::Duration::from_millis(3));
            }
            let dispatch = tracing::dispatcher::get_default(Clone::clone);
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    tracing::dispatcher::with_default(&dispatch, || {
                        let _other = other.enter();
                    });
                });
            });
            drop(frame);
        });
        shared.drain();
        let state = shared.state();
        let names: Vec<String> = state.recent.iter().map(|s| shared.shown(s.name)).collect();
        assert_eq!(
            names,
            ["game::slow", "the game world's update", "game::quick"],
            "a thread's spans in the order they ended; the executor's not kept"
        );
        let slow = state.recent[0];
        assert!(slow.end - slow.start >= 3_000_000, "{slow:?}");
        assert!(state.recent[1].start <= slow.start && state.recent[1].end >= slow.end);
    }

    #[test]
    fn a_freeze_line_says_what_held_it() {
        let shared = Shared::new();
        let slow = shared.name("terra_folk::settlement::found".into(), None);
        let quick = shared.name("terra_world::flora::plant".into(), None);
        let line = freeze_line(
            &shared,
            41230,
            &Slow {
                frame_ns: 1_840_000_000,
                held: Held::Game,
                longest: vec![(slow, 1_710_000_000), (quick, 4_100_000)],
                outside_ns: 60_000_000,
            },
        );
        assert!(
            line.starts_with(
                "freeze: frame 41230 took 1840 ms, held by the game world: terra_folk::settlement::found 1710 ms, terra_world::flora::plant 4.1 ms; 60 ms outside its systems"
            ),
            "{line}"
        );
    }
}
