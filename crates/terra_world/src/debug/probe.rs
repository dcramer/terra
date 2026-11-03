//! The probe (D609): what a change costs on the GPU, measured in one load, only on settled
//! frames. `--probe "old: fog.light_span=0" --probe "new: fog.light_span=1"` (`cargo xtask
//! probe`) names the variants, each a set of levers: tunables (D547) to set, materials to hide. Once
//! play begins it holds the clock, then runs the variants in rounds, A B A B, each phase
//! counting `probe.frames` frames that the settle gate passes ([`plan::Gate`]: no pipeline
//! compiling, every vertex drawn), and prints each phase's line with the card's clocks
//! ([`gpu`]), a table after each round, and warnings for a phase run clocked down. It fails
//! loudly, and ends the run, on a lever that matches nothing: a tunable no one declared, a
//! hide matching no mesh or none in view (play-lake-fps j12's `WindMaterial` that was
//! `WindExtension`), a phase that never settles, or a clock that moved.
//!
//! Shipped practice it follows: Unreal's CSV profiler and its PerfReportTool (a capture's
//! frames, a summary table a stat), Unreal's automated performance tests (Gauntlet), which
//! wait for shader and PSO compilation before they capture, Unity's Performance Testing
//! package (`Measure.Frames().WarmupCount(n).MeasurementCount(m)`), and Chromium's Pinpoint
//! and Mozilla's Perfherder, which alternate the two sides' repeats so drift falls on both
//! and judge a difference against the spread of each side's repeats. Built from the lake
//! jobs' `probe.rs` (play-lake-fps j8 to j13) and perf-growth j1's, kept until then in
//! patches.

use super::{CPU_PASS, pass_series, passes_timed};
use crate::loading::Screen;
use bevy::diagnostic::DiagnosticsStore;
use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld, RenderApp, render_resource::PipelineCache};
use std::time::Instant;
use terra_gen::tune::{self, SetBy, Tunable};

pub mod gpu;
pub mod plan;
#[cfg(test)]
mod tests;

pub use plan::Plan;
use plan::{Frame, Gate, Lever, Rules, Seen, TOTAL, Verdict};

static FRAMES: Tunable = Tunable::new(
    "probe.frames",
    400.0,
    50.0..=20000.0,
    "frames a probe's phase counts once settled (D609): more narrows the noise, costs time",
);
static ROUNDS: Tunable = Tunable::new(
    "probe.rounds",
    2.0,
    1.0..=10.0,
    "rounds of every variant a probe runs (D609): two give each variant its A/A pair",
);
static WAIT: Tunable = Tunable::new(
    "probe.wait",
    60.0,
    0.0..=5000.0,
    "frames after a probe's switch before its gate looks (D609)",
);
static IDLE: Tunable = Tunable::new(
    "probe.idle",
    30.0,
    1.0..=5000.0,
    "frames in a row with no pipeline compiling before a probe's phase counts (D609)",
);
static TIMEOUT: Tunable = Tunable::new(
    "probe.timeout",
    6000.0,
    100.0..=100000.0,
    "frames a probe's phase may wait to settle before the probe fails (D609)",
);

/// A counted frame draws at least this share of its variant's most vertices (j10's rule).
const DRAWN_SHARE: f64 = 0.995;
/// How many passes the table shows beside the total.
const TABLE_PASSES: usize = 14;
/// How far the clock may move over the run, in hours, before the probe fails: a minute.
const HOUR_DRIFT: f32 = 1.0 / 60.0;

/// The pipelines compiling, as the render world last said.
#[derive(Resource, Default)]
struct Compiling(usize);

#[derive(Resource)]
struct Probe {
    plan: Plan,
    rules: Rules,
    frames: usize,
    phases: Vec<(usize, usize)>,
    at: usize,
    started: bool,
    applied: bool,
    gate: Gate,
    began: Instant,
    counted: Vec<Frame>,
    by_variant: Vec<Vec<Vec<Frame>>>,
    most: Vec<f64>,
    last_timed: Option<Instant>,
    /// Each tunable's value when the probe began, for variants that do not set it.
    base: Vec<(&'static Tunable, f32)>,
    hidden: Vec<(Entity, Visibility)>,
    hour: Option<f32>,
    clocks: Vec<(String, Option<gpu::Clocks>)>,
    sampler: Result<gpu::Sampler, String>,
    /// The run is over: whether it passed.
    done: Option<bool>,
}

impl Probe {
    fn new(plan: Plan) -> Probe {
        let variants = plan.variants.len();
        Probe {
            plan,
            rules: Rules {
                wait: 0,
                idle: 1,
                timeout: 0,
                drawn_share: DRAWN_SHARE,
            },
            frames: 0,
            phases: Vec::new(),
            at: 0,
            started: false,
            applied: false,
            gate: Gate::default(),
            began: Instant::now(),
            counted: Vec::new(),
            by_variant: vec![Vec::new(); variants],
            most: vec![0.0; variants],
            last_timed: None,
            base: Vec::new(),
            hidden: Vec::new(),
            hour: None,
            clocks: Vec::new(),
            sampler: Err("not started".into()),
            done: None,
        }
    }
}

/// Adds the probe when `--probe` asked for one.
pub fn plugin(app: &mut App) {
    let Some(plan) = app
        .world()
        .get_resource::<crate::shell::Options>()
        .and_then(|o| o.probe.clone())
    else {
        return;
    };
    tune::register(&[&FRAMES, &ROUNDS, &WAIT, &IDLE, &TIMEOUT]);
    app.insert_resource(Probe::new(plan))
        .init_resource::<Compiling>()
        .add_systems(
            PostUpdate,
            step.run_if(in_state(Screen::Playing))
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        )
        .add_systems(Last, leave);
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(ExtractSchedule, count_compiling);
    }
}

fn count_compiling(mut main_world: ResMut<MainWorld>, cache: Res<PipelineCache>) {
    if let Some(mut compiling) = main_world.get_resource_mut::<Compiling>() {
        compiling.0 = cache.waiting_pipelines().count();
    }
}

fn leave(probe: Res<Probe>, mut exit: MessageWriter<AppExit>, mut left: Local<bool>) {
    if let (Some(passed), false) = (probe.done, *left) {
        *left = true;
        crate::shell::finish(
            &mut exit,
            if passed {
                AppExit::Success
            } else {
                AppExit::error()
            },
        );
    }
}

/// Ends the run with the reason, after `✗` so a gate's reader finds it (D388).
fn fail(probe: &mut Probe, why: String) {
    println!("PROBE ✗ {why}");
    probe.done = Some(false);
}

fn step(world: &mut World) {
    world.resource_scope(|world, mut probe: Mut<Probe>| {
        if probe.done.is_some() {
            return;
        }
        if !probe.started {
            probe.started = true;
            if let Err(why) = start(&mut probe) {
                return fail(&mut probe, why);
            }
        }
        if !probe.applied
            && let Err(why) = apply(world, &mut probe)
        {
            return fail(&mut probe, why);
        }
        hide(world, &mut probe);
        let store = world.resource::<DiagnosticsStore>();
        let Some(then) = passes_timed(store).filter(|&t| probe.last_timed != Some(t)) else {
            return;
        };
        probe.last_timed = Some(then);
        let frame = sample(store, then);
        let compiling = world.resource::<Compiling>().0;
        let hour = world
            .get_resource::<crate::daylight::Clock>()
            .map(|c| c.hour);
        judge(&mut probe, frame, compiling, hour);
    });
}

/// Reads the tunables, holds the clock and checks every lever's tunable is one.
fn start(probe: &mut Probe) -> Result<(), String> {
    probe.frames = FRAMES.get() as usize;
    probe.rules = Rules {
        wait: WAIT.get() as u32,
        idle: IDLE.get() as u32,
        timeout: TIMEOUT.get() as u32,
        drawn_share: DRAWN_SHARE,
    };
    probe.phases = probe.plan.phases(ROUNDS.get() as usize);
    for name in probe.plan.tunables() {
        let tunable = tune::named(name).map_err(|e| {
            format!("a lever names no tunable: {e}; a lever is a tunable its owner declares (D547, D609)")
        })?;
        probe.base.push((tunable, tunable.get()));
    }
    for variant in &probe.plan.variants {
        for lever in &variant.levers {
            if let Lever::Tune(name, value) = lever {
                let t = tune::named(name)?;
                if !(t.min..=t.max).contains(value) {
                    return Err(format!(
                        "{}'s lever {name}={value} is out of its range, {} to {} (D547)",
                        variant.name, t.min, t.max
                    ));
                }
            }
        }
    }
    // A running clock moves the sun, and the shade and sky with it: j11's fog saving was the
    // sun going behind the ridge.
    if !probe.plan.tunables().contains(&"daylight.pace") {
        crate::daylight::PACE.set(0.0, SetBy::CommandLine)?;
    }
    probe.sampler = gpu::Sampler::start();
    let names: Vec<&str> = probe
        .plan
        .variants
        .iter()
        .map(|v| v.name.as_str())
        .collect();
    println!(
        "PROBE {} phases ({} in rounds of {}), {} settled frames each, the clock held at pace 0; clocks: {}",
        probe.phases.len(),
        names.join(" "),
        names.len(),
        probe.frames,
        probe
            .sampler
            .as_ref()
            .map_or_else(|e| e.clone(), |_| "nvidia-smi".into()),
    );
    Ok(())
}

/// The phase's variant: its tunables set, the rest back to where they began, what the last
/// hid shown again, and its hides checked to match meshes in view.
fn apply(world: &mut World, probe: &mut Probe) -> Result<(), String> {
    let (v, round) = probe.phases[probe.at];
    let variant = probe.plan.variants[v].clone();
    for &(tunable, base) in &probe.base {
        let value = variant
            .levers
            .iter()
            .find_map(|l| match l {
                Lever::Tune(n, value) if n == tunable.name => Some(*value),
                _ => None,
            })
            .unwrap_or(base);
        tunable.set(value, SetBy::CommandLine)?;
    }
    for (e, was) in probe.hidden.drain(..) {
        if let Ok(mut entity) = world.get_entity_mut(e) {
            entity.insert(was);
        }
    }
    for part in hides(&variant) {
        let matched = matching(world, part);
        let in_view = matched
            .iter()
            .filter(|&&e| world.get::<ViewVisibility>(e).is_some_and(|v| v.get()))
            .count();
        if in_view == 0 {
            let what = if matched.is_empty() {
                "matched no mesh or light".to_string()
            } else {
                format!("matched {} meshes, none in view,", matched.len())
            };
            return Err(format!(
                "{}'s hide={part} {what} so it measures nothing: name a part of the type of a \
                 material or light drawn here (all/in view): {} (D609)",
                variant.name,
                census(world).join(", ")
            ));
        }
    }
    let levers: Vec<String> = variant
        .levers
        .iter()
        .map(|l| match l {
            Lever::Tune(n, v) => format!("{n}={v}"),
            Lever::Hide(p) => format!("hide={p}"),
        })
        .collect();
    println!(
        "PROBE phase {} #{}: {}",
        variant.name,
        round + 1,
        if levers.is_empty() {
            "the base".to_string()
        } else {
            levers.join(", ")
        }
    );
    probe.applied = true;
    probe.gate = Gate::default();
    probe.counted.clear();
    probe.began = Instant::now();
    Ok(())
}

fn hides(variant: &plan::Variant) -> Vec<&str> {
    variant
        .levers
        .iter()
        .filter_map(|l| match l {
            Lever::Hide(p) => Some(p.as_str()),
            Lever::Tune(..) => None,
        })
        .collect()
}

/// The name a hide matches an entity by: its material's type (`MeshMaterial3d<…>`) or its
/// light's, the short names.
fn kinds(world: &World) -> Vec<(String, Vec<Entity>)> {
    let mut by: Vec<(String, Vec<Entity>)> = Vec::new();
    let Some(visibility) = world.components().component_id::<Visibility>() else {
        return by;
    };
    for arch in world.archetypes().iter() {
        if !arch.contains(visibility) || arch.entities().is_empty() {
            continue;
        }
        let name = arch.components().iter().find_map(|&id| {
            let n = world
                .components()
                .get_info(id)?
                .name()
                .shortname()
                .to_string();
            (n.starts_with("MeshMaterial3d<") || n.ends_with("Light")).then_some(n)
        });
        let Some(name) = name else { continue };
        let entities = arch.entities().iter().map(|e| e.id());
        match by.iter_mut().find(|(n, _)| *n == name) {
            Some((_, es)) => es.extend(entities),
            None => by.push((name, entities.collect())),
        }
    }
    by
}

fn matching(world: &World, part: &str) -> Vec<Entity> {
    kinds(world)
        .into_iter()
        .filter(|(name, _)| name.contains(part))
        .flat_map(|(_, es)| es)
        .collect()
}

/// Every kind drawn, with how many and how many in view.
fn census(world: &World) -> Vec<String> {
    kinds(world)
        .iter()
        .map(|(name, es)| {
            let shown = es
                .iter()
                .filter(|&&e| world.get::<ViewVisibility>(e).is_some_and(|v| v.get()))
                .count();
            format!("{name} {}/{shown}", es.len())
        })
        .collect()
}

/// Hides what the variant hides, each frame, so what streams in is hidden too.
fn hide(world: &mut World, probe: &mut Probe) {
    let (v, _) = probe.phases[probe.at];
    let parts: Vec<String> = hides(&probe.plan.variants[v])
        .into_iter()
        .map(String::from)
        .collect();
    for part in parts {
        for e in matching(world, &part) {
            let Some(&was) = world.get::<Visibility>(e) else {
                continue;
            };
            if was != Visibility::Hidden {
                world.entity_mut(e).insert(Visibility::Hidden);
                probe.hidden.push((e, was));
            }
        }
    }
}

/// The frame last timed: each GPU pass's time, their total, and the vertices drawn.
fn sample(store: &DiagnosticsStore, then: Instant) -> Frame {
    let mut frame = Frame::default();
    let mut total = 0.0;
    let mut vertices = None;
    for d in store.iter() {
        let path = d.path().as_str();
        if let Some(pass) = pass_series(path)
            && !pass.starts_with(CPU_PASS)
            && let Some(ms) = super::timed_then(d, then)
        {
            if !pass.contains('/') {
                total += ms;
            }
            frame.passes.insert(pass, ms);
        }
        if path.ends_with("/vertex_shader_invocations")
            && path.matches('/').count() == 2
            && let Some(v) = d.value()
        {
            *vertices.get_or_insert(0.0) += v;
        }
    }
    frame.passes.insert(TOTAL.into(), total);
    frame.vertices = vertices;
    frame
}

/// Passes the frame through the gate, and ends the phase once it has its frames.
fn judge(probe: &mut Probe, frame: Frame, compiling: usize, hour: Option<f32>) {
    let (v, round) = probe.phases[probe.at];
    let seen = Seen {
        compiling,
        vertices: frame.vertices,
    };
    let most = probe.most[v];
    let rules = probe.rules;
    let verdict = probe.gate.frame(&rules, seen, most);
    if probe.gate.past_pipelines(&rules)
        && let Some(vertices) = frame.vertices
    {
        probe.most[v] = probe.most[v].max(vertices);
    }
    match verdict {
        Verdict::Count => probe.counted.push(frame),
        Verdict::Wait(_) => {}
        Verdict::TimedOut(why) => {
            let name = probe.plan.variants[v].name.clone();
            return fail(
                probe,
                format!(
                    "{name} #{} never settled in {} timed frames ({} counted): at the last, {why}; \
                     raise probe.timeout if the scene is still loading, or look at what keeps \
                     compiling (D609)",
                    round + 1,
                    rules.timeout,
                    probe.counted.len()
                ),
            );
        }
    }
    if probe.counted.len() >= probe.frames {
        end_phase(probe, hour);
    }
}

fn end_phase(probe: &mut Probe, hour: Option<f32>) {
    let (v, round) = probe.phases[probe.at];
    let name = format!("{} #{}", probe.plan.variants[v].name, round + 1);
    let first = *probe.hour.get_or_insert(hour.unwrap_or(0.0));
    if let Some(hour) = hour
        && (hour - first).abs() > HOUR_DRIFT
    {
        return fail(
            probe,
            format!(
                "the clock moved from {first:.2} h to {hour:.2} h by {name}: something set the \
                 hour or the pace; a probe holds it at daylight.pace=0 (D609)"
            ),
        );
    }
    let clocks = probe
        .sampler
        .as_ref()
        .ok()
        .and_then(|s| gpu::summarise(&s.between(probe.began, Instant::now())));
    let mut totals: Vec<f64> = probe.counted.iter().map(|f| f.passes[TOTAL]).collect();
    println!(
        "PROBE {name}: TOTAL {:.3} ms over {} frames, {:.2}M vertices; {}; hour {first:.2}",
        plan::median(&mut totals),
        probe.counted.len(),
        plan::most_vertices(&probe.counted) / 1e6,
        clocks
            .as_ref()
            .map_or("clocks unknown".into(), |c| c.to_string()),
    );
    probe.clocks.push((name, clocks));
    let counted = std::mem::take(&mut probe.counted);
    probe.by_variant[v].push(counted);
    probe.at += 1;
    probe.applied = false;
    let round_done = probe.phases.get(probe.at).is_none_or(|&(_, r)| r != round);
    if round_done {
        print_table(probe, round);
    }
    if probe.at == probe.phases.len() {
        finish(probe);
    }
}

fn print_table(probe: &Probe, round: usize) {
    let names: Vec<&str> = probe
        .plan
        .variants
        .iter()
        .map(|v| v.name.as_str())
        .collect();
    let rows = plan::rows(&probe.by_variant, DRAWN_SHARE, TABLE_PASSES);
    println!(
        "PROBE table after round {}, GPU ms, medians of every settled frame; A/A the spread of a \
         variant's rounds, the noise floor:",
        round + 1
    );
    for line in plan::table(&names, &rows) {
        println!("PROBE | {line}");
    }
}

/// The run's warnings, then its end.
fn finish(probe: &mut Probe) {
    for warning in gpu::clocked_down(&probe.clocks) {
        println!("PROBE warning: {warning}");
    }
    // A variant that hides nothing drawing fewer vertices than another measured a scene with
    // something missing, unless its lever draws less.
    let most = probe
        .plan
        .variants
        .iter()
        .zip(&probe.most)
        .filter(|(v, _)| hides(v).is_empty())
        .map(|(_, m)| *m)
        .fold(0.0, f64::max);
    for (variant, m) in probe.plan.variants.iter().zip(&probe.most) {
        if hides(variant).is_empty() && *m < most * DRAWN_SHARE {
            println!(
                "PROBE warning: {} drew {:.2}M vertices at its most against {:.2}M: unless its \
                 levers draw less, something was missing from its frames (D609)",
                variant.name,
                m / 1e6,
                most / 1e6
            );
        }
    }
    println!("PROBE done");
    probe.done = Some(true);
}
