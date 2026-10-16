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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::{Bill, Rules, Terms};
    use crate::village::{Kinds, town};
    use std::collections::BTreeMap;

    const KINDS: Kinds = Kinds {
        timber: Kind(1),
        masonry: Kind(2),
        beams: Kind(3),
        cob: Kind(4),
        framed: Kind(5),
        timber_roof: Kind(0x800),
        masonry_roof: Kind(0x801),
        beams_roof: Kind(0x802),
    };

    /// Timber two courses a cube, blocks one step, roofs four stages, as the kits give them.
    fn rules() -> Rules {
        let steps = [
            (1, 2),
            (2, 1),
            (3, 2),
            (4, 1),
            (5, 1),
            (0x800, 4),
            (0x801, 4),
            (0x802, 4),
        ];
        Rules {
            kinds: steps
                .iter()
                .map(|&(kind, steps)| (Kind(kind), Terms::new(steps, Bill::default())))
                .collect::<BTreeMap<_, _>>(),
            props: BTreeMap::new(),
            toughness: BTreeMap::new(),
        }
    }

    #[test]
    fn a_town_built_step_by_step_ends_as_the_shortcuts_say_from_the_bottom_up() {
        // D381: each building of a town, put up a step at a time in the hammer's order,
        // ends with every cube built and the same cubes as laid whole; no step puts a part's
        // cube in over one of the same part still to go in below it; a log wall goes in a
        // whole course at a time and a stone one a block at a time; and a roof's stage is
        // never put on before the one under it is on across its layer.
        for building in town(3, 12, KINDS) {
            let whole = Base::from_shortcuts(&building.ops);
            let steps = construction(&whole, &rules(), Vec3::ZERO);
            let name = &building.name;
            let (_, last) = steps.last().unwrap_or_else(|| panic!("{name}: no steps"));
            assert!(finished(last), "{name}: not all built");
            assert!(last.same_cubes(&whole), "{name}: not the same cubes");
            let mut before = Base::default();
            let rules = rules();
            let cx = Context::free(&rules);
            for op in whole.plan_operations() {
                before.act(&op, &cx).unwrap();
            }
            for (step, after) in &steps {
                if let Step::Put {
                    part, kind, cubes, ..
                } = step
                {
                    let lowest_waiting = before
                        .cubes_of(*part)
                        .filter(|(_, cube)| cube.fill == Fill::Solid && !cube.is_built())
                        .map(|(at, _)| at.y)
                        .min();
                    assert_eq!(
                        Some(cubes[0][1]),
                        lowest_waiting,
                        "{name}: a cube of part {} put in over one still to go in",
                        part.0
                    );
                    if *kind == KINDS.masonry {
                        assert_eq!(cubes.len(), 1, "{name}: stone goes in a block at a time");
                    }
                    if *kind == KINDS.timber {
                        let layer = before
                            .cubes_of(*part)
                            .filter(|(at, cube)| {
                                at.y == cubes[0][1] && cube.fill == Fill::Solid && !cube.is_built()
                            })
                            .count();
                        assert!(cubes.len() == layer, "{name}: a log course spans its wall");
                    }
                }
                before = after.clone();
            }
        }
    }

    #[test]
    fn the_built_share_rises_with_every_step_to_one() {
        // D381: a time-lapse paces a building by how far it is built, so that share must rise
        // with each step and be whole at the end.
        let building = &town(5, 3, KINDS)[0];
        let whole = Base::from_shortcuts(&building.ops);
        let steps = construction(&whole, &rules(), Vec3::ZERO);
        let mut last = 0.0;
        for (_, base) in &steps {
            let share = base.built_share();
            assert!(share > last, "{share} after {last}");
            last = share;
        }
        assert!((last - 1.0).abs() < 1e-6);
    }
}
