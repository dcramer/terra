//! The cases measured against a kind's own numbers (docs/controller.md criteria 4, 11, 12 and
//! 14): the time to top speed and back to standing, the turn, steering in the air, the speed on
//! each walkable slope and the speed a landing is reported at. Each answer is known from the
//! kind's data or a law (`top / accel`, `sqrt(2gh)`), never from a tuned number. Every case is
//! run for two kinds, a man and a second body whose numbers all differ, so two kinds' files
//! must give two results.

use super::field::{Course, Field, forward_at, side_at, world};
use super::runs::{DROPS, Run, ground};
use super::{Body, Intent, Kind, Measure, Mover, TICK};
use glam::Vec3;

/// The turns tried from standing, deg.
const TURNS: [f32; 2] = [90.0, 150.0];
/// How long a body steers in the air, ticks.
const AIR_TICKS: usize = 16;
/// The walkable grades walked, deg: 45 down to 45 up by 5.
const GRADES: std::ops::RangeInclusive<i32> = -9..=9;
const GRADE_STEP: f32 = 5.0;

/// A second kind, a heavier, slower body, every number these cases read unlike the man's. It is
/// the course's alone: no second body walks the mover in the game yet.
pub(super) fn second(man: &Kind) -> Kind {
    Kind {
        radius: 0.5,
        height: 2.4,
        mass: 300.0,
        jog: 3.5,
        sprint: 6.0,
        accel: 12.0,
        decel: 16.0,
        air_control: 0.15,
        turn_deg_s: 240.0,
        slope_speed: [
            (-45.0, 0.8),
            (-20.0, 0.95),
            (0.0, 1.0),
            (20.0, 0.7),
            (45.0, 0.45),
        ],
        jump_speed: 4.5,
        fall_onset: 10.0,
        fall_fatal: 20.0,
        swim: 1.4,
        swim_accel: 5.0,
        wade: 0.5,
        climb_out: 1.3,
        dive: 1.0,
        dive_from_deg: 25.0,
        mouth: 0.85,
        breath_s: 45.0,
        breath_back_s: 6.0,
        drown_s: 15.0,
        climb: 0.7,
        climb_side: 0.55,
        climb_accel: 3.0,
        reach: 2.9,
        mantle_s: 1.0,
        climb_jump: 1.2,
        climb_stamina_s: 6.0,
        climb_jump_stamina: 25.0,
        ..*man
    }
}

/// A case, for the kind at an index of the two.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Case {
    /// From standing to top speed, jogging or sprinting.
    Accel(usize, bool),
    /// From jogging to standing.
    Decel(usize),
    /// Standing, turned toward a way so many degrees off its facing.
    Turn(usize, f32),
    /// Steering forward in the air after a jump straight up.
    Air(usize),
    /// Walking a slope of a grade, deg (downhill negative).
    Slope(usize, f32),
    /// Dropped from a height onto level ground.
    Landing(usize, f32),
}

impl Case {
    fn kind(self) -> usize {
        match self {
            Case::Accel(k, _)
            | Case::Decel(k)
            | Case::Turn(k, _)
            | Case::Air(k)
            | Case::Slope(k, _)
            | Case::Landing(k, _) => k,
        }
    }

    /// The criterion it is measured for.
    fn row(self) -> &'static str {
        match self {
            Case::Accel(..) | Case::Decel(_) => "accel.data",
            Case::Turn(..) => "turn.data",
            Case::Air(_) => "air.control",
            Case::Slope(..) => "slope.speed",
            Case::Landing(..) => "landing.speed",
        }
    }

    /// The same case for the other kind.
    fn other(self) -> Case {
        let o = 1 - self.kind();
        match self {
            Case::Accel(_, sprint) => Case::Accel(o, sprint),
            Case::Decel(_) => Case::Decel(o),
            Case::Turn(_, deg) => Case::Turn(o, deg),
            Case::Air(_) => Case::Air(o),
            Case::Slope(_, deg) => Case::Slope(o, deg),
            Case::Landing(_, h) => Case::Landing(o, h),
        }
    }
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for k in 0..2 {
        cases.extend([Case::Accel(k, false), Case::Accel(k, true), Case::Decel(k)]);
        cases.extend(TURNS.map(|deg| Case::Turn(k, deg)));
        cases.push(Case::Air(k));
        cases.extend(GRADES.map(|i| Case::Slope(k, i as f32 * GRADE_STEP)));
        cases.extend(DROPS.map(|h| Case::Landing(k, h)));
    }
    cases
}

/// Ticks to cover a span at a rate a tick, the last part tick counted whole.
fn ticks_for(span: f32, per_tick: f32) -> f32 {
    libm::ceilf(span / per_tick - 1e-3)
}

/// What the kind's data says a case measures, and whether the two are told apart by ticks
/// (accel, turn: one tick off allowed) or as a fraction (air, slope).
fn expected(case: Case, kind: &Kind, gravity: f32) -> (f32, bool) {
    match case {
        Case::Accel(_, sprint) => {
            let top = if sprint { kind.sprint } else { kind.jog };
            (ticks_for(top, kind.accel * TICK), true)
        }
        Case::Decel(_) => (ticks_for(kind.jog, kind.decel * TICK), true),
        Case::Turn(_, deg) => (ticks_for(deg, kind.turn_deg_s * TICK), true),
        Case::Air(_) => (kind.accel * kind.air_control, false),
        Case::Slope(_, deg) => (kind.jog * kind.slope_factor(deg), false),
        Case::Landing(_, h) => (libm::sqrtf(2.0 * gravity * h), false),
    }
}

/// How far a measure is off what was expected: ticks, or a fraction of it.
fn off(measured: f32, (want, in_ticks): (f32, bool)) -> f32 {
    if in_ticks {
        (measured - want).abs()
    } else {
        (measured / want.max(1e-6) - 1.0).abs()
    }
}

pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> Vec<Measure> {
    let man = Kind::man();
    let kinds = [man, second(&man)];
    let gravity = Course::flat().gravity();
    let cases = cases();
    let got = terra_gen::par::map(&cases, |case| run(new, &kinds[case.kind()], *case));
    let all = || cases.iter().zip(&got);
    let want = |case: &Case| expected(*case, &kinds[case.kind()], gravity);
    let worst = |id: &str| -> Option<f32> {
        all()
            .filter(|(case, _)| case.row() == id)
            .map(|(case, m)| m.map(|m| off(m, want(case))))
            .try_fold(0.0_f32, |w, m| m.map(|m| w.max(m)))
    };
    // A landing reported at the speed of the fall, to within a tick of gravity.
    let landings = all()
        .filter(|(case, _)| matches!(case, Case::Landing(..)))
        .all(|(case, m)| m.is_some_and(|m| (m - want(case).0).abs() <= gravity * TICK));
    // The two kinds' results alike where their data sets them apart, by a tick or 2%.
    let alike = all()
        .filter(|(case, _)| case.kind() == 0 && !matches!(case, Case::Landing(..)))
        .filter_map(|(case, m)| {
            let (a, b) = (want(case), want(&case.other()));
            let apart = |x: f32, y: f32| {
                if a.1 {
                    (x - y).abs() > 1.5
                } else {
                    (x / y.max(1e-6) - 1.0).abs() > 0.02
                }
            };
            let theirs = (*all().find(|(c, _)| **c == case.other())?.1)?;
            let ours = (*m)?;
            apart(a.0, b.0).then(|| !apart(ours, theirs))
        })
        .filter(|same| *same)
        .count();

    let m = |id, value: f32| Measure { id, value };
    let mut measures = vec![
        m("landing.speed", f32::from(u8::from(!landings))),
        m("data.kinds", alike as f32),
    ];
    // A mover that keeps no facing leaves the turn unmeasured.
    measures.extend(
        ["accel.data", "turn.data", "air.control", "slope.speed"]
            .into_iter()
            .filter_map(|id| worst(id).map(|w| m(id, w))),
    );
    measures
}

fn level_speed(body: &Body) -> f32 {
    body.velocity.reject_from(body.up).length()
}

/// Ticks a run takes until a body is so, at most so many.
fn until<M: Mover>(
    run: &mut Run<M>,
    most: usize,
    intent: Intent,
    done: impl Fn(&Run<M>) -> bool,
) -> f32 {
    (1..=most)
        .find(|_| {
            run.tick(|_, _| intent);
            done(run)
        })
        .unwrap_or(most + 1) as f32
}

/// A case's number as measured: ticks, an acceleration or a speed; `None` where the mover does
/// not report what it needs.
fn run<M: Mover>(new: &impl Fn() -> M, kind: &Kind, case: Case) -> Option<f32> {
    let flat = Course::flat();
    let start = world(0.0, 0.0, 0.0);
    let (forward, side) = (forward_at(start), side_at(start));
    let walk = |sprint| Intent {
        wish: forward,
        sprint,
        ..Intent::default()
    };
    let ticks = |s: f32| (s / TICK) as usize;
    let mut run = Run::new(new(), &flat);
    match case {
        Case::Accel(_, sprint) => {
            let top = if sprint { kind.sprint } else { kind.jog };
            run.add(0, *kind, start);
            Some(until(&mut run, ticks(2.0), walk(sprint), |r| {
                level_speed(&r.body(0)) >= top * 0.999
            }))
        }
        Case::Decel(_) => {
            run.add(0, *kind, start);
            for _ in 0..ticks(1.0) {
                run.tick(|_, _| walk(false));
            }
            Some(until(&mut run, ticks(2.0), Intent::default(), |r| {
                level_speed(&r.body(0)) <= 1e-3
            }))
        }
        Case::Turn(_, deg) => {
            run.add(0, *kind, start);
            run.mover().facing(0)?;
            let (s, c) = (libm::sinf(deg.to_radians()), libm::cosf(deg.to_radians()));
            let wish = forward * c + side * s;
            let intent = Intent {
                wish,
                ..Intent::default()
            };
            let near = libm::cosf(0.25_f32.to_radians());
            Some(until(&mut run, ticks(3.0), intent, |r| {
                let up = r.body(0).up;
                let facing = r.mover().facing(0).unwrap_or(Vec3::ZERO);
                facing.dot(wish.reject_from(up).normalize()) >= near
            }))
        }
        Case::Air(_) => {
            run.add(0, *kind, start);
            for tick in 0..=4 {
                run.tick(|_, _| Intent {
                    jump: tick == 4,
                    ..Intent::default()
                });
            }
            let along = |r: &Run<M>| r.body(0).velocity.dot(forward);
            let from = along(&run);
            for _ in 0..AIR_TICKS {
                run.tick(|_, _| walk(false));
            }
            Some((along(&run) - from) / (AIR_TICKS as f32 * TICK))
        }
        Case::Slope(_, deg) => {
            let (s, c) = (libm::sinf(deg.to_radians()), libm::cosf(deg.to_radians()));
            let course = ground(&[(-5.0, 0.0), (1.0, 0.0), (1.0 + 40.0 * c, 40.0 * s)]);
            let mut run = Run::new(new(), &course);
            run.add(0, *kind, start);
            let mut at = Vec3::ZERO;
            for tick in 1..=ticks(2.5) {
                run.tick(|_, _| walk(false));
                if tick == ticks(1.5) {
                    at = run.body(0).feet;
                }
            }
            Some((run.body(0).feet - at).length() / 1.0)
        }
        Case::Landing(_, h) => {
            run.add(0, *kind, world(0.0, h, 0.0));
            (0..ticks(2.0)).find_map(|_| {
                run.tick(|_, _| Intent::default());
                run.mover().landed(0)
            })
        }
    }
}
