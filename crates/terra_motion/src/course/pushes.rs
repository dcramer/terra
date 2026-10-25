//! Pushing (D479, criterion 15): people push each other softly, monsters hard by their mass.
//! Each answer is known another way: two soft bodies' overlap eases by their kind's time
//! (`soft_s`), so the ticks it takes to fall from 20 cm under 1 cm are `ln 20` of them; a man
//! walking past a standing one is measured against the same walk with no one there; a hard
//! pair parts by its share of their mass, so a man walking into a monster moves it at that share
//! of his speed; and a wall is where the fixture says.
//!
//! The monster is the course's own (a kind four-odd times a man's mass, twice his width, hard):
//! no monster walks the mover in the game yet.

use super::field::{Course, local, world};
use super::runs::{Outcome, Run, overlap, walk, walls};
use super::{Intent, Kind, Measure, Mover, Push, TICK};

/// How far to the side of a walker's way a man stands as it passes, m: from all but head on to
/// a shoulder's brush.
const PASS: [f32; 6] = [0.05, 0.1, 0.2, 0.3, 0.4, 0.5];
/// How far the line of men the monster walks through is shifted to its side, m.
const LINE: [f32; 2] = [0.0, 0.3];
/// How far to the side of the monster's way a man stands as it walks him into a wall, m.
const PINNED: [f32; 2] = [0.0, 0.3];

/// A monster for the course: hard (D479), wider, taller and over five times a man's mass.
pub(super) fn monster(man: &Kind) -> Kind {
    Kind {
        radius: 0.6,
        height: 2.6,
        mass: 400.0,
        push: Push::Hard,
        core_m: 0.1,
        ..*man
    }
}

#[derive(Clone, Copy, Debug)]
enum Case {
    /// Two men standing 20 cm into each other.
    Soft,
    /// A man jogging past one standing so far to the side of his way, and the same walk with no
    /// one there.
    Pass(f32, bool),
    /// The monster walking through a line of men across its way, shifted so far, or alone.
    Monster(f32, bool),
    /// A man walking head on into the monster standing.
    Person,
    /// The monster walking a man standing so far to the side of its way into a wall.
    Wall(f32),
}

fn cases() -> Vec<Case> {
    let mut cases = vec![Case::Soft, Case::Person];
    cases.extend(
        PASS.iter()
            .flat_map(|&w| [Case::Pass(w, true), Case::Pass(w, false)]),
    );
    cases.extend(
        LINE.iter()
            .flat_map(|&w| [Case::Monster(w, true), Case::Monster(w, false)]),
    );
    cases.extend(PINNED.map(Case::Wall));
    cases
}

fn ticks(s: f32) -> usize {
    (s / TICK) as usize
}

/// How far ahead a body is in the level frame.
fn ahead(run: &Run<'_, impl Mover>, id: u32) -> f32 {
    local(run.body(id).feet).x
}

fn run<M: Mover>(new: &impl Fn() -> M, case: Case) -> Outcome {
    let man = Kind::man();
    let flat = Course::flat();
    match case {
        Case::Soft => {
            let mut run = Run::new(new(), &flat);
            run.add(0, man, world(0.0, 0.0, 0.0));
            run.add(1, man, world(2.0 * man.radius - 0.2, 0.0, 0.0));
            let parted = (1..=ticks(2.0)).find(|_| {
                run.tick(|_, _| Intent::default());
                overlap(&run.body(0), &man, &run.body(1), &man) < 0.01
            });
            let want = (libm::logf(20.0) * man.soft_s / TICK).ceil();
            let took = parted.unwrap_or(ticks(2.0)) as f32;
            run.outcome(vec![(took - want).abs()])
        }
        Case::Pass(side, someone) => {
            let mut run = Run::new(new(), &flat);
            run.add(0, man, world(0.0, 0.0, 0.0));
            let stands = world(3.0, 0.0, side);
            if someone {
                run.add(1, man, stands);
            }
            let mut moved = 0.0_f32;
            for _ in 0..ticks(1.5) {
                run.tick(|id, b| if id == 0 { walk(b) } else { Intent::default() });
                if someone {
                    moved = moved.max((run.body(1).feet - stands).length());
                }
            }
            let went = ahead(&run, 0);
            run.outcome(vec![went, moved])
        }
        Case::Monster(shift, line) => {
            let brute = monster(&man);
            let mut run = Run::new(new(), &flat);
            run.add(0, brute, world(0.0, 0.0, 0.0));
            if line {
                for k in 0..5u32 {
                    let w = (k as f32 - 2.0) * 0.8 + shift;
                    run.add(1 + k, man, world(3.0, 0.0, w));
                }
            }
            for _ in 0..ticks(2.0) {
                run.tick(|id, b| if id == 0 { walk(b) } else { Intent::default() });
            }
            let went = ahead(&run, 0);
            run.outcome(vec![went])
        }
        Case::Person => {
            let brute = monster(&man);
            let mut run = Run::new(new(), &flat);
            run.add(0, man, world(0.0, 0.0, 0.0));
            run.add(1, brute, world(2.0, 0.0, 0.0));
            let walking = |run: &mut Run<'_, M>, s: f32| {
                for _ in 0..ticks(s) {
                    run.tick(|id, b| if id == 0 { walk(b) } else { Intent::default() });
                }
            };
            // Up to it and pressing, then a second of pressing measured.
            walking(&mut run, 1.0);
            let from = ahead(&run, 1);
            walking(&mut run, 1.0);
            let speed = ahead(&run, 1) - from;
            let share = man.mass / (man.mass + brute.mass);
            run.outcome(vec![(speed / man.jog - share).abs() / share])
        }
        Case::Wall(side) => {
            let brute = monster(&man);
            let course = walls(0.0);
            let mut run = Run::new(new(), &course);
            run.add(0, brute, world(0.5, 0.0, 0.0));
            run.add(1, man, world(2.2, 0.0, side));
            let mut through = false;
            for _ in 0..ticks(2.0) {
                run.tick(|id, b| if id == 0 { walk(b) } else { Intent::default() });
                through |= ahead(&run, 1) > 3.0 || ahead(&run, 0) > 3.0;
            }
            run.outcome(vec![f32::from(u8::from(through))])
        }
    }
}

/// The pushes' measures, the deepest any body went into the ground or a wall, and the worst
/// overlap of a hard pair after a tick.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> (Vec<Measure>, f32, f32) {
    let cases = cases();
    let outcomes = terra_gen::par::map(&cases, |case| run(new, *case));
    let all = || cases.iter().zip(&outcomes);
    let found = |want: &dyn Fn(&Case) -> bool| all().find(|(c, _)| want(c)).map(|(_, o)| o);
    let soft = found(&|c| matches!(c, Case::Soft)).map_or(0.0, |o| o.values[0]);
    // A walk past one standing: the share of the walk with no one there it lost, and how far
    // the one standing was moved, as a share of how far out of the walker's way is.
    let (mut lost, mut moved) = (0.0_f32, 0.0_f32);
    for side in PASS {
        let with = found(&|c| matches!(c, Case::Pass(w, true) if *w == side));
        let alone = found(&|c| matches!(c, Case::Pass(w, false) if *w == side));
        if let (Some(with), Some(alone)) = (with, alone) {
            lost = lost.max(1.0 - with.values[0] / alone.values[0].max(1e-3));
            // Out of the walker's way is both radii less how far aside it stood.
            let way = 2.0 * Kind::man().radius - side;
            moved = moved.max(with.values[1] / way);
        }
    }
    let mut slowed = 0.0_f32;
    for shift in LINE {
        let with = found(&|c| matches!(c, Case::Monster(w, true) if *w == shift));
        let alone = found(&|c| matches!(c, Case::Monster(w, false) if *w == shift));
        if let (Some(with), Some(alone)) = (with, alone) {
            slowed = slowed.max(1.0 - with.values[0] / alone.values[0].max(1e-3));
        }
    }
    let person = found(&|c| matches!(c, Case::Person)).map_or(0.0, |o| o.values[0]);
    // Into the wall: as deep as any body of the cases went into it, or 1 if one went through.
    let walled = all().filter(|(c, _)| matches!(c, Case::Wall(_)));
    let wall = walled
        .map(|(_, o)| {
            let deepest = o.depths.iter().copied().fold(0.0, f32::max);
            o.values[0].max(deepest)
        })
        .fold(0.0, f32::max);
    let deepest = outcomes
        .iter()
        .flat_map(|o| o.depths.iter().copied())
        .fold(0.0, f32::max);
    let overlap = outcomes.iter().map(|o| o.overlap).fold(0.0, f32::max);
    let m = |id, value: f32| Measure { id, value };
    let measures = vec![
        m("push.soft", soft),
        m("push.pass", lost.max(0.0)),
        m("push.stander", moved),
        m("push.monster", slowed.max(0.0)),
        m("push.person", person),
        m("push.wall", wall),
    ];
    (measures, deepest, overlap)
}
