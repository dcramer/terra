//! Building by hand, a cube a click (D226, D260): thousands of cubes laid one at a time in
//! walks across the edges of sections never run out of parts, and every rule still holds.

use super::*;
use crate::support::FACES;
use terra_gen::Seed;

/// Whether every part is in one piece, and no two cubes of a kind face to face are in
/// different parts laid on the same terms.
fn parts_touch_and_agree(base: &Base, at: &str) {
    let mut by_part: BTreeMap<PartId, BTreeSet<[i32; 3]>> = BTreeMap::new();
    for (cube, found) in base.cubes() {
        let Some(part) = found.part else { continue };
        by_part.entry(part).or_default().insert(cube.to_array());
        for step in FACES {
            let Some(beside) = base.cube(cube + step) else {
                continue;
            };
            if let Some(other) = beside.part
                && other != part
                && beside.kind == found.kind
            {
                assert_ne!(
                    base.part(part).map(|p| &p.terms),
                    base.part(other).map(|p| &p.terms),
                    "{at}: parts {part:?} and {other:?} touch at {cube} and agree"
                );
            }
        }
    }
    for (part, cubes) in by_part {
        let first = *cubes.first().expect("a part has cubes");
        let mut reached = BTreeSet::from([first]);
        let mut frontier = vec![first];
        while let Some(at) = frontier.pop() {
            for step in FACES {
                let next = (IVec3::from_array(at) + step).to_array();
                if cubes.contains(&next) && reached.insert(next) {
                    frontier.push(next);
                }
            }
        }
        assert_eq!(
            reached.len(),
            cubes.len(),
            "{at}: part {part:?} is in pieces"
        );
    }
}

/// What went in and what came out, kept as operations are done.
#[derive(Default)]
struct Books {
    paid: Bill,
    refunded: Bill,
}

impl Books {
    /// Does operations as one act, and, if `undo_too`, undoes them and does them again,
    /// each time to the same state.
    fn act(
        &mut self,
        base: &mut Base,
        ops: &[Op],
        cx: &Context,
        at: &str,
        undo_too: bool,
    ) -> Result<(), Refusal> {
        let before = base.clone();
        let done = base.act_all(ops, cx)?;
        self.paid.add(&done.took);
        self.refunded.add(&done.gave);
        if undo_too {
            let after = base.clone();
            base.act_all(&done.undo, cx)
                .unwrap_or_else(|why| panic!("{at}: undo refused: {why:?}"));
            assert_eq!(*base, before, "{at}: undone is not as it was");
            base.act_all(ops, cx)
                .unwrap_or_else(|why| panic!("{at}: redo refused: {why:?}"));
            assert_eq!(*base, after, "{at}: redone is not as it was");
        }
        Ok(())
    }
}

#[test]
fn thousands_of_cubes_built_by_hand_never_run_out_of_parts() {
    // Cubes laid one at a time against what is built, as a player clicks, in walks that
    // wander across the edges of sections, in two kinds; now and then one is taken out and
    // then out of the plan, and now and then what was done is undone and done again.
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    let mut books = Books::default();
    let mut rng = Seed(7).derive("by hand").rng();
    let (low, high) = (IVec3::new(4, 0, 4), IVec3::new(27, 21, 27));
    let mut built: Vec<IVec3> = Vec::new();
    let mut walker = IVec3::new(15, 0, 15);
    let (mut laid, mut split) = (0, 0);
    for step in 0..3200 {
        let at = format!("step {step}");
        let kind = if (step / 400) % 2 == 0 { LOGS } else { STONE };
        if step % 9 == 8 && !built.is_empty() {
            // A cube built a while ago taken apart, if nothing rests on it: its last step
            // takes it out with its plan (D193), and splits its part if that leaves it in
            // pieces (D260).
            let i = rng.below(built.len() as u32) as usize;
            let cube = built[i];
            let mut out = true;
            while base.cube(cube).is_some() {
                let ops = base.taking_apart(&[cube]);
                let last = !base.emptying(&[cube]).is_empty();
                split += ops
                    .iter()
                    .filter(|op| matches!(op, Op::Regroup { .. }))
                    .count();
                let undo_too = last && rng.below(4) == 0;
                if books.act(&mut base, &ops, &cx, &at, undo_too).is_err() {
                    out = false;
                    break;
                }
            }
            // Part out and no further: put back, as a player finishes what they began.
            while !out && !base.cube(cube).is_some_and(|c| c.is_built()) {
                let fill = [Op::Fill {
                    cubes: vec![cube.to_array()],
                }];
                books
                    .act(&mut base, &fill, &cx, &at, false)
                    .unwrap_or_else(|why| panic!("{at}: putting {cube} back refused: {why:?}"));
            }
            if out {
                built.swap_remove(i);
            }
        } else {
            // Against a face of what is built, or on the ground, near where the walk is.
            let target = (0..40).find_map(|_| {
                let from = if built.is_empty() || rng.below(5) == 0 {
                    walker
                } else {
                    built[built.len() - 1 - rng.below(built.len().min(30) as u32) as usize]
                };
                let next = from + FACES[rng.below(6) as usize];
                // Not under what is in, which a kind of two courses cannot be built under.
                let free = base.cube(next).is_none()
                    && !base.is_in(next + IVec3::Y)
                    && next.cmpge(low).all()
                    && next.cmple(high).all()
                    && (next.y == 0 || FACES.iter().any(|&s| base.is_in(next + s)));
                free.then_some(next)
            });
            let Some(target) = target else {
                walker = IVec3::new(
                    low.x + rng.below(24) as i32,
                    0,
                    low.z + rng.below(24) as i32,
                );
                continue;
            };
            walker = target;
            let terms = rules.of(kind);
            let (part, mut ops) = base
                .joining(&[target], kind, &terms)
                .unwrap_or_else(|| (base.next_part(), Vec::new()));
            ops.push(Op::LayPlan {
                part: Some(part),
                kind,
                cubes: vec![Planned {
                    at: target.to_array(),
                    shape: Shape::Whole,
                }],
                ground: Vec::new(),
            });
            books
                .act(&mut base, &ops, &cx, &at, rng.below(10) == 0)
                .unwrap_or_else(|why| panic!("{at}: laying at {target} refused: {why:?}"));
            while !base.cube(target).is_some_and(|cube| cube.is_built()) {
                let fill = [Op::Fill {
                    cubes: vec![target.to_array()],
                }];
                books
                    .act(&mut base, &fill, &cx, &at, false)
                    .unwrap_or_else(|why| panic!("{at}: building {target} refused: {why:?}"));
            }
            built.push(target);
            laid += 1;
        }
        if step % 50 == 0 {
            check_rules(&base, &at);
            parts_touch_and_agree(&base, &at);
        }
        if step % 10 == 0 {
            let mut held = value(&base, &rules);
            held.add(&books.refunded);
            assert_eq!(
                held, books.paid,
                "{at}: what is in and what came out is not what went in"
            );
        }
    }
    check_rules(&base, "the end");
    parts_touch_and_agree(&base, "the end");
    let sections: BTreeSet<[i32; 3]> = base
        .cubes()
        .map(|(at, _)| crate::cubes::section_of(at).to_array())
        .collect();
    assert!(laid > 2500, "only {laid} laid");
    assert!(split > 0, "nothing taken out ever split a part");
    assert!(sections.len() >= 8, "the walks stayed in {sections:?}");
    // A part a click would have needed far more than a section's palette of 255.
    let cubes = base.cubes().count();
    assert!(cubes > 255 * 4, "only {cubes} stand");
}
