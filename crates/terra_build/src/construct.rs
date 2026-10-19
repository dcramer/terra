//! A building going up as builders put it up (D139, D194, D241; D381): the plan of what it
//! will be laid first, a part a kind, its openings left open, and then a step at a time in
//! the order the hammer takes them ([`Base::next_steps`]), every step checked as a player's
//! is. A footing and a plinth go in before the walls on them, a log wall a course at a time
//! across its whole length, a stone one a block at a time beside the last, a roof a layer
//! at a time and a stage at a time, frame, battens, covering and trims. What a time-lapse of
//! a settlement shows, and anything else that wants a building built as a player builds it
//! rather than laid whole (`put_up`).

use crate::cost::Rules;
use crate::cubes::{Fill, Kind};
use crate::ops::{Context, Op};
use crate::order::Step;
use crate::{Base, CELL_M};
use glam::{IVec3, Vec3};

/// How far from where the work is the next part is looked for, in metres: the whole of a
/// building, so its builders work round it rather than stop where they stand.
pub const REACH_M: f32 = 1000.0;

/// The step builders take next on a base going up, working at `near` (metres, its own
/// space): the first of [`Base::next_steps`] the rules allow, with its operation. `None` once
/// everything planned is built, or nothing left can be (the rules refuse every step).
pub fn next_step(
    base: &Base,
    near: Vec3,
    cx: &Context,
    one_at_a_time: impl Fn(Kind) -> bool,
) -> Option<(Step, Op)> {
    base.next_steps(near, REACH_M, None, one_at_a_time)
        .into_iter()
        .map(|step| {
            let op = step.op();
            (step, op)
        })
        .find(|(_, op)| base.check(op, cx).is_ok())
}

/// Where the work is after a step: the middle of the cubes it put in, so the next block
/// goes in beside the last, as a mason works along a wall.
pub fn worked_at(step: &Step) -> Option<Vec3> {
    let Step::Put { cubes, .. } = step else {
        return None;
    };
    let sum: Vec3 = cubes
        .iter()
        .map(|&at| (IVec3::from_array(at).as_vec3() + 0.5) * CELL_M)
        .sum();
    (!cubes.is_empty()).then(|| sum / cubes.len() as f32)
}

/// A building put up from nothing as builders put it up: its plan, then every step in the
/// order they take them, starting from `near`, each with the base as it stands after it.
/// Free of any claim and with nothing in the way. Stops where the rules refuse every step.
pub fn construction(whole: &Base, rules: &Rules, near: Vec3) -> Vec<(Step, Base)> {
    let cx = Context::free(rules);
    let mut base = Base::default();
    for op in whole.plan_operations() {
        if base.act(&op, &cx).is_err() {
            return Vec::new();
        }
    }
    let one_at_a_time = |kind: Kind| rules.of(kind).step() == crate::cubes::FULL;
    let mut near = near;
    let mut steps = Vec::new();
    while let Some((step, op)) = next_step(&base, near, &cx, one_at_a_time) {
        if base.act(&op, &cx).is_err() {
            break;
        }
        near = worked_at(&step).unwrap_or(near);
        steps.push((step, base.clone()));
    }
    steps
}

/// Whether every cube of a base that is to be built is.
pub fn finished(base: &Base) -> bool {
    base.cubes()
        .all(|(_, cube)| cube.fill == Fill::Open || cube.is_built())
}
