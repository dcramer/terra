//! The cases of wading (D523): a body jogs on level ground under still water of a depth, from
//! ankle deep to past its kind's swim depth, for a man and the second kind. Wading, its speed
//! falls linearly with the depth from its jog to its kind's `wade` share of it at its swim
//! depth; deeper, it swims at its kind's swim speed. Each answer is that rule on the kind's
//! numbers.

use super::field::{Course, Water, local, world};
use super::runs::{Outcome, Run, ground};
use super::{Intent, Kind, Measure, Mover, TICK};
use glam::{Vec2, Vec3};

/// Depths walked, as shares of the kind's swim depth: the middles of [`STEPS`] equal parts up
/// to it, and on past it by [`PAST`] more, so none is at the swim depth itself.
const STEPS: usize = 20;
const PAST: usize = 8;
/// How long the body jogs before its speed is read, and over how long it is read, s.
const SETTLE_S: f32 = 2.0;
const READ_S: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
struct Case {
    /// The man or the second kind.
    kind: usize,
    /// The depth's share of the kind's swim depth.
    share: f32,
}

fn kinds() -> [Kind; 2] {
    let man = Kind::man();
    [man, super::data::second(&man)]
}

fn cases() -> Vec<Case> {
    (0..2)
        .flat_map(|kind| {
            (0..STEPS + PAST).map(move |i| Case {
                kind,
                share: (i as f32 + 0.5) / STEPS as f32,
            })
        })
        .collect()
}

/// The speed the rule wants at a depth's share of the swim depth, m/s.
fn wanted(kind: &Kind, share: f32) -> f32 {
    if share > 1.0 {
        kind.swim
    } else {
        kind.jog * (1.0 - (1.0 - kind.wade) * share)
    }
}

/// Level ground at 0 under water `depth` deep, wide every way.
fn pond(depth: f32) -> Course {
    let mut course = ground(&[(-20.0, 0.0), (200.0, 0.0)]);
    course.water = Some(Water {
        min: Vec2::new(-20.0, -200.0),
        max: Vec2::new(200.0, 200.0),
        surface: depth,
    });
    course
}

/// The level speed it jogs at in the water, m/s.
fn run<M: Mover>(new: &impl Fn() -> M, case: Case) -> Outcome {
    let kind = kinds()[case.kind];
    let course = pond(case.share * kind.swim_depth * kind.height);
    let mut run = Run::new(new(), &course);
    run.add(0, kind, world(0.0, 0.0, 0.0));
    let jog = |run: &mut Run<M>, s: f32| {
        for _ in 0..(s / TICK) as usize {
            run.tick(|_, b| Intent {
                wish: Vec3::X.reject_from(b.feet.normalize()).normalize(),
                ..Intent::default()
            });
        }
    };
    jog(&mut run, SETTLE_S);
    let from = local(run.body(0).feet);
    jog(&mut run, READ_S);
    let to = local(run.body(0).feet);
    let level = Vec2::new(to.x - from.x, to.z - from.z).length() / READ_S;
    run.outcome(vec![level])
}

/// The wading measures, and the deepest any body went into the ground.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> (Vec<Measure>, f32) {
    let cases = cases();
    let outcomes = terra_gen::par::map(&cases, |case| run(new, *case));
    let kinds = kinds();
    let speeds: Vec<f32> = outcomes.iter().map(|o| o.values[0]).collect();
    let off = cases
        .iter()
        .zip(&speeds)
        .map(|(c, v)| (v - wanted(&kinds[c.kind], c.share)).abs())
        .fold(0.0, f32::max);
    // Consecutive depths of one kind: the largest change of speed between them.
    let step = cases
        .windows(2)
        .zip(speeds.windows(2))
        .filter(|(c, _)| c[0].kind == c[1].kind)
        .map(|(_, v)| (v[1] - v[0]).abs())
        .fold(0.0, f32::max);
    let deepest = outcomes
        .iter()
        .flat_map(|o| o.depths.iter().copied())
        .fold(0.0, f32::max);
    let m = |id, value: f32| Measure { id, value };
    (vec![m("wade.speed", off), m("wade.step", step)], deepest)
}
