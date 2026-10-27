//! The cases of holding on to a face (docs/controller.md, "Climbing"; D505-D507, D510, D511),
//! for a man and the second kind: what holding, moving, an overhang and a climb jump cost in
//! stamina and letting go at none (D506); a face of no grip never held and one of less grip
//! costing more (D505); never taking hold in a fight stance (D507); a staggering blow knocking
//! a climber off (D506); how close a climber is held over a dug cliff's steps, and climbing
//! down onto the floor.
//!
//! Every answer is the kind's own numbers (movers.ron's `climb_stamina_s`,
//! `climb_jump_stamina`, `climb_gap`) and the decisions' shares: holding a quarter of moving,
//! an overhang twice, a face's cost over its grip.

use super::climb::{self, CLIMBED_M, Face};
use super::field::{Course, forward_at, local, world};
use super::runs::Run;
use super::{Body, Field, Intent, Kind, Measure, Mover, TICK};
use glam::Vec3;

/// The ticks a cost is measured over (a second).
const WINDOW: usize = 64;
/// The overhang a cost is measured on, deg past vertical (D510: twice the cost).
const LEAN: f32 = 15.0;
/// A smooth face's grip, and a face of none (D505: ice, glass, plaster).
const SMOOTH: f32 = 0.5;
/// Holding still costs this share of moving (D506).
const HOLD_SHARE: f32 = 0.25;
/// Feet within this of the floor have reached it, m (criterion 2's centimetre).
const FLOOR_M: f32 = 0.01;
/// How far a climber is held from the face is measured once it has held it this long, s.
const SETTLED_S: f32 = 0.25;

/// What a second on a face is spent on.
#[derive(Clone, Copy, Debug)]
enum Spend {
    /// Holding still on a sheer face, or on an overhang.
    Hold(bool),
    /// Moving up a sheer face, or an overhang.
    Move(bool),
    /// A climb jump up a sheer face, then holding.
    Lunge,
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Stamina(usize, Spend),
    /// Held still until out of stamina, then walked into the face again.
    Zero(usize),
    /// A face of no grip walked into, or jumped at.
    Ice(usize, bool),
    /// Held still on a face of [`SMOOTH`] grip.
    Smooth(usize),
    /// Walked into a face in a fight stance, or jumping at it.
    Stance(usize, bool),
    Knock(usize),
    /// Climbing the dug cliff, or a sheer wall.
    Hold(usize, bool),
    Down(usize),
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for k in 0..2 {
        cases.extend(
            [
                Spend::Hold(false),
                Spend::Hold(true),
                Spend::Move(false),
                Spend::Move(true),
                Spend::Lunge,
            ]
            .map(|s| Case::Stamina(k, s)),
        );
        cases.push(Case::Zero(k));
        cases.extend([false, true].map(|j| Case::Ice(k, j)));
        cases.push(Case::Smooth(k));
        cases.extend([false, true].map(|j| Case::Stance(k, j)));
        cases.push(Case::Knock(k));
        cases.extend([true, false].map(|c| Case::Hold(k, c)));
        cases.push(Case::Down(k));
    }
    cases
}

fn wrong(is: bool) -> f32 {
    f32::from(u8::from(is))
}

/// Into the face.
fn into(b: &Body) -> Intent {
    Intent {
        wish: forward_at(b.feet),
        ..Intent::default()
    }
}

/// A body walked into a face until it holds it [`CLIMBED_M`] up; `None` if it never does.
fn climbed<'a, M: Mover>(
    new: &impl Fn() -> M,
    kind: &Kind,
    course: &'a Course,
) -> Option<Run<'a, M>> {
    let mut run = Run::new(new(), course);
    run.add(0, *kind, world(0.0, 0.0, 0.0));
    let mut walk = climb::at_face(kind, 0.0, false);
    let seconds = climb::reach_s(kind) + kind.climb_push_s + CLIMBED_M / climb::slowest(kind) + 1.0;
    for _ in 0..(seconds / TICK) as usize {
        run.tick(|_, b| walk(b));
        if run.mover().climbing(0) && local(run.body(0).feet).y >= CLIMBED_M {
            return Some(run);
        }
    }
    None
}

/// Whether a body took hold of a face walked into (or jumped at) by a script.
fn takes_hold<M: Mover>(
    new: &impl Fn() -> M,
    kind: &Kind,
    course: &Course,
    jump: bool,
    with: impl Fn(Intent) -> Intent,
) -> bool {
    let mut run = Run::new(new(), course);
    run.add(0, *kind, world(0.0, 0.0, 0.0));
    let mut walk = climb::at_face(kind, 0.0, jump);
    let mut held = false;
    for _ in 0..((climb::reach_s(kind) + kind.climb_push_s + 1.0) / TICK) as usize {
        run.tick(|_, b| with(walk(b)));
        held |= run.mover().climbing(0);
    }
    held
}

/// The share a second's stamina was off what the kind's numbers say it costs, held
/// [`CLIMBED_M`] up a face of a grip; 1 if it never held or let go.
fn cost<M: Mover>(new: &impl Fn() -> M, kind: &Kind, spend: Spend, grip: f32) -> f32 {
    let over = matches!(spend, Spend::Hold(true) | Spend::Move(true));
    let mut course = climb::face(if over { 90.0 + LEAN } else { 90.0 }, 8.0);
    course.grip = grip;
    let Some(mut run) = climbed(new, kind, &course) else {
        return 1.0;
    };
    let before = run.mover().stamina(0);
    for i in 0..WINDOW {
        run.tick(|_, b| match spend {
            Spend::Move(_) => into(b),
            Spend::Lunge if i == 0 => Intent {
                jump: true,
                ..into(b)
            },
            _ => Intent::default(),
        });
    }
    if !run.mover().climbing(0) {
        return 1.0;
    }
    let spent = before - run.mover().stamina(0);
    let second = kind.climb_stamina_s * WINDOW as f32 * TICK;
    let expected = match spend {
        Spend::Move(_) => second,
        Spend::Hold(_) => HOLD_SHARE * second,
        Spend::Lunge => kind.climb_jump_stamina + HOLD_SHARE * second,
    } * if over { 2.0 } else { 1.0 }
        / grip;
    (spent / expected - 1.0).abs()
}

/// Held still until out of stamina: the ticks past one from none to letting go, and 1 more if
/// it took hold again walked back into the face.
fn zero<M: Mover>(new: &impl Fn() -> M, kind: &Kind) -> f32 {
    let course = climb::face(90.0, 8.0);
    let Some(mut run) = climbed(new, kind, &course) else {
        return 1.0;
    };
    let longest = run.mover().stamina(0) / (HOLD_SHARE * kind.climb_stamina_s) + 2.0;
    let mut none = None;
    for i in 0..(longest / TICK) as usize {
        run.tick(|_, _| Intent::default());
        if !run.mover().climbing(0) {
            let Some(at) = none else {
                return 1.0;
            };
            let late = (i - at) as f32 - 1.0;
            let mut again = false;
            for _ in 0..((kind.climb_push_s + 2.0) / TICK) as usize {
                run.tick(|_, b| into(b));
                again |= run.mover().climbing(0);
            }
            return late.max(0.0) + wrong(again);
        }
        if none.is_none() && run.mover().stamina(0) <= 0.0 {
            none = Some(i);
        }
    }
    1.0
}

/// 1 unless a blow knocks a climber off within a tick and it holds no face again before it
/// stands, the stick still into the face.
fn knock<M: Mover>(new: &impl Fn() -> M, kind: &Kind) -> f32 {
    let course = climb::face(90.0, 8.0);
    let Some(mut run) = climbed(new, kind, &course) else {
        return 1.0;
    };
    run.tick(|_, b| Intent {
        knocked: true,
        ..into(b)
    });
    let mut held = run.mover().climbing(0);
    for _ in 0..(0.3 / TICK) as usize {
        run.tick(|_, b| into(b));
        held |= run.mover().climbing(0) && !run.body(0).grounded;
    }
    wrong(held)
}

/// The capsule's gap to the ground and what stands on it, sampled every 2.5 cm along its axis.
fn gap(field: &dyn Field, kind: &Kind, body: &Body) -> f32 {
    let a = body.feet + body.up * kind.radius;
    let b = body.feet + body.up * (kind.height - kind.radius);
    let distance = |at: Vec3| {
        let obstacles = field.obstacles().iter().map(|o| o.distance(at));
        obstacles.fold(field.distance(at), f32::min)
    };
    let n = ((kind.height - 2.0 * kind.radius) / 0.025) as usize;
    let nearest = (0..=n)
        .map(|i| distance(a.lerp(b, i as f32 / n as f32)))
        .fold(f32::INFINITY, f32::min);
    nearest - kind.radius
}

/// The most a climber's gap to the face was off its kind's `climb_gap`, climbing the dug
/// cliff's steps (or a sheer wall) from holding it until its hands are near the top; 1 if it
/// never climbed that far.
fn hold<M: Mover>(new: &impl Fn() -> M, kind: &Kind, cliff: bool) -> f32 {
    let (course, high) = if cliff {
        (climb::faces(Face::Cliff, kind), 6.0)
    } else {
        (climb::face(90.0, 6.0), 6.0)
    };
    let below = high - kind.reach - 0.1;
    let mut run = Run::new(new(), &course);
    run.add(0, *kind, world(0.0, 0.0, 0.0));
    let mut walk = climb::at_face(kind, 0.0, false);
    let seconds = climb::reach_s(kind) + kind.climb_push_s + high / climb::slowest(kind);
    let (mut held_s, mut worst) = (0.0, 0.0_f32);
    for _ in 0..(seconds / TICK) as usize {
        run.tick(|_, b| walk(b));
        if !run.mover().climbing(0) {
            continue;
        }
        held_s += TICK;
        let b = run.body(0);
        if local(b.feet).y >= below {
            return worst;
        }
        if held_s >= SETTLED_S {
            worst = worst.max((gap(&course, kind, &b) - kind.climb_gap).abs());
        }
    }
    1.0
}

/// Climbing down from [`CLIMBED_M`] up a sheer wall: the ticks past one from the feet reaching
/// the floor to standing on it, and 1 more if the landing hurt; 1 if it never stood.
fn down<M: Mover>(new: &impl Fn() -> M, kind: &Kind) -> f32 {
    let course = climb::face(90.0, 8.0);
    let Some(mut run) = climbed(new, kind, &course) else {
        return 1.0;
    };
    let (mut floor, mut hurt) = (None, 0.0_f32);
    for i in 0..((CLIMBED_M / climb::slowest(kind) + 1.0) / TICK) as usize {
        run.tick(|_, b| Intent {
            wish: -forward_at(b.feet),
            ..Intent::default()
        });
        let b = run.body(0);
        hurt = hurt.max(run.mover().fall(0));
        if floor.is_none() && local(b.feet).y <= FLOOR_M {
            floor = Some(i);
        }
        if b.grounded {
            let late = floor.map_or(0.0, |at| (i - at) as f32 - 1.0);
            return late.max(0.0) + wrong(hurt > 0.0);
        }
    }
    1.0
}

fn run<M: Mover>(new: &impl Fn() -> M, case: Case) -> f32 {
    let kinds = climb::kinds();
    let wall = climb::face(90.0, 8.0);
    match case {
        Case::Stamina(k, spend) => cost(new, &kinds[k], spend, 1.0),
        Case::Zero(k) => zero(new, &kinds[k]),
        Case::Ice(k, jump) => {
            let ice = Course { grip: 0.0, ..wall };
            wrong(takes_hold(new, &kinds[k], &ice, jump, |i| i))
        }
        Case::Smooth(k) => wrong(cost(new, &kinds[k], Spend::Hold(false), SMOOTH) > 0.01),
        Case::Stance(k, jump) => wrong(takes_hold(new, &kinds[k], &wall, jump, |i| Intent {
            fighting: true,
            ..i
        })),
        Case::Knock(k) => knock(new, &kinds[k]),
        Case::Hold(k, cliff) => hold(new, &kinds[k], cliff),
        Case::Down(k) => down(new, &kinds[k]),
    }
}

/// The measures of holding on.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> Vec<Measure> {
    let cases = cases();
    let values = terra_gen::par::map(&cases, |case| run(new, *case));
    let of = |pick: &dyn Fn(&Case) -> bool| -> Vec<f32> {
        cases
            .iter()
            .zip(&values)
            .filter(|(c, _)| pick(c))
            .map(|(_, v)| *v)
            .collect()
    };
    let worst = |pick: &dyn Fn(&Case) -> bool| of(pick).into_iter().fold(0.0, f32::max);
    let count = |pick: &dyn Fn(&Case) -> bool| of(pick).iter().sum::<f32>();
    let m = |id, value: f32| Measure { id, value };
    vec![
        m(
            "climb.stamina",
            worst(&|c| matches!(c, Case::Stamina(..) | Case::Zero(..))),
        ),
        m(
            "climb.grip",
            count(&|c| matches!(c, Case::Ice(..) | Case::Smooth(..))),
        ),
        m("climb.stance", count(&|c| matches!(c, Case::Stance(..)))),
        m("climb.knock", count(&|c| matches!(c, Case::Knock(..)))),
        m("climb.hold", worst(&|c| matches!(c, Case::Hold(..)))),
        m("climb.down", count(&|c| matches!(c, Case::Down(..)))),
    ]
}
