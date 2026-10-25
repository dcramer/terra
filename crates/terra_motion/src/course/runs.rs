//! The course's cases: a fixture, a body or a crowd, a script of intents, and what the run
//! measures. Each answer is known another way: by the fixture's geometry, the kind's numbers
//! or a law (v²/2g, a mass ratio, the same state from the same inputs).

use super::field::{Ball, Course, Field, Solid, Water, forward_at, local, side_at, world};
use super::{Body, Intent, Kind, Measure, Mover, TICK};
use glam::{Vec2, Vec3};

/// The speeds a body is driven at a wall or another body, m/s, up to the bar's 60.
const SPEEDS: [f32; 8] = [2.0, 5.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0];
/// The step heights tried, m: 0.1 to 0.8 by 0.1, and either side of the kind's step.
const STEPS: [f32; 8] = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
/// How much wider than the body's diameter each gap is, m: narrower ones must stop it, those
/// 4 cm or more wider let it through.
const GAPS: [f32; 6] = [-0.1, -0.04, -0.01, 0.04, 0.07, 0.1];
/// The heights a body is dropped from, m.
const DROPS: [f32; 4] = [0.5, 1.0, 2.0, 4.0];
/// The crowd: its size, and how long it runs for the determinism criterion.
const CROWD: u32 = 24;
const CROWD_TICKS: usize = 10_000;
/// The latest tick after leaving a ledge, or before landing, a jump is tried at.
const LATE: usize = 24;
/// Where the round world's falls start: over the pole, the equator, between, and in the cave.
const FALLS: usize = 4;

#[derive(Clone, Copy, Debug)]
enum Order {
    Forward,
    Again,
    Reversed,
    Interleaved,
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Crowd(Order),
    Step(f32),
    Slope(f32),
    Gap(f32),
    Wall(f32),
    Through(f32),
    StickSlope,
    StickStair,
    Ceiling,
    Fall(usize),
    Apex,
    Coyote(usize),
    Buffer(usize),
    Drop(f32),
    Input,
    Pool,
    Share(bool),
    Fixed(bool),
}

/// What a case gave: its numbers, the capsule's depth into the ground after each tick, the
/// worst overlap of two bodies after a tick, and for the crowd its end state.
#[derive(Default)]
struct Outcome {
    values: Vec<f32>,
    depths: Vec<f32>,
    overlap: f32,
    end: Vec<u32>,
}

fn cases() -> Vec<Case> {
    let mut cases = vec![
        Case::Crowd(Order::Forward),
        Case::Crowd(Order::Again),
        Case::Crowd(Order::Reversed),
        Case::Crowd(Order::Interleaved),
    ];
    let kind = Kind::man();
    let steps = STEPS
        .into_iter()
        .chain([kind.step_height, kind.step_height + 0.01]);
    cases.extend(steps.map(Case::Step));
    // 0 to 70 deg by 5, and by half a degree within 10 of the kind's limit.
    let coarse = (0..=14).map(|i| i as f32 * 5.0);
    let fine = (0..=40).map(|i| kind.slope_limit_deg - 10.0 + i as f32 * 0.5);
    cases.extend(coarse.chain(fine).map(Case::Slope));
    // The narrowest is a voxel (0.5 m) for a man.
    cases.extend(GAPS.map(Case::Gap));
    cases.extend(SPEEDS.map(Case::Wall));
    cases.extend(SPEEDS.map(Case::Through));
    cases.extend([
        Case::StickSlope,
        Case::StickStair,
        Case::Ceiling,
        Case::Apex,
        Case::Input,
        Case::Pool,
    ]);
    cases.extend((0..FALLS).map(Case::Fall));
    cases.extend((0..=LATE).map(Case::Coyote));
    cases.extend((1..=LATE).map(Case::Buffer));
    cases.extend(DROPS.map(Case::Drop));
    cases.extend([
        Case::Share(false),
        Case::Share(true),
        Case::Fixed(false),
        Case::Fixed(true),
    ]);
    cases
}

pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> Vec<Measure> {
    let cases = cases();
    let outcomes = terra_gen::par::map(&cases, |case| run(new, *case));
    let kind = Kind::man();
    let all = || cases.iter().zip(&outcomes);
    let of = |want: fn(&Case) -> bool| all().filter(move |(case, _)| want(case));
    let sum = |want: fn(&Case) -> bool, i: usize| of(want).map(|(_, o)| o.values[i]).sum::<f32>();
    let worst =
        |want: fn(&Case) -> bool, i: usize| of(want).map(|(_, o)| o.values[i]).fold(0.0, f32::max);

    let mut depths: Vec<f32> = outcomes
        .iter()
        .flat_map(|o| o.depths.iter().copied())
        .collect();
    let deepest = depths.iter().copied().fold(0.0, f32::max);
    // The median over ticks touching something, within the padding.
    depths.retain(|d| *d > -0.02);
    depths.sort_by(f32::total_cmp);
    let median = depths.get(depths.len() / 2).map_or(0.0, |d| d.max(0.0));

    let slopes: Vec<(f32, bool)> = all()
        .filter_map(|(case, o)| match case {
            Case::Slope(deg) => Some((*deg, o.values[0] > 0.5)),
            _ => None,
        })
        .collect();
    let late = |buffer: bool| {
        let worked = all().filter_map(|(case, o)| match (case, buffer) {
            (Case::Coyote(k), false) if o.values[0] > 0.5 => Some(*k + 1),
            (Case::Buffer(k), true) if o.values[0] > 0.5 => Some(*k),
            _ => None,
        });
        worked.max().unwrap_or(0) as f32 * TICK
    };
    let crowd = |order: Order| {
        all().find_map(|(case, o)| match case {
            Case::Crowd(each) if std::mem::discriminant(each) == std::mem::discriminant(&order) => {
                Some(o)
            }
            _ => None,
        })
    };
    let first = crowd(Order::Forward).map(|o| o.end.clone());
    let differ = [Order::Again, Order::Reversed, Order::Interleaved]
        .into_iter()
        .filter(|order| crowd(*order).map(|o| &o.end) != first.as_ref())
        .count();
    let overlap = outcomes.iter().map(|o| o.overlap).fold(0.0, f32::max);

    let m = |id, value: f32| Measure { id, value };
    vec![
        m("tunnel.wall", sum(|c| matches!(c, Case::Wall(_)), 0)),
        m("tunnel.body", sum(|c| matches!(c, Case::Through(_)), 0)),
        m("penetration.max", deepest),
        m("penetration.median", median),
        m("slope.limit", (limit(&slopes) - kind.slope_limit_deg).abs()),
        m("step.ground", sum(|c| matches!(c, Case::Step(_)), 0)),
        m("stick.slope", sum(|c| matches!(c, Case::StickSlope), 0)),
        m("stick.stair", sum(|c| matches!(c, Case::StickStair), 0)),
        m("ceiling.depth", sum(|c| matches!(c, Case::Ceiling), 0)),
        m("ceiling.speed", sum(|c| matches!(c, Case::Ceiling), 1)),
        m("gravity.landing", worst(|c| matches!(c, Case::Fall(_)), 0)),
        m("gravity.upright", worst(|c| matches!(c, Case::Fall(_)), 1)),
        m("jump.apex", sum(|c| matches!(c, Case::Apex), 0)),
        m("jump.coyote", (late(false) - kind.coyote_s).abs()),
        m("jump.buffer", (late(true) - kind.buffer_s).abs()),
        m("landing.delay", worst(|c| matches!(c, Case::Drop(_)), 0)),
        m("landing.bounce", worst(|c| matches!(c, Case::Drop(_)), 1)),
        m("input.first_tick", sum(|c| matches!(c, Case::Input), 0)),
        m("push.overlap", overlap),
        m("push.share", worst(|c| matches!(c, Case::Share(_)), 0)),
        m("push.fixed", worst(|c| matches!(c, Case::Fixed(_)), 0)),
        m("fit.gap", sum(|c| matches!(c, Case::Gap(_)), 0)),
        m("swim.enters", sum(|c| matches!(c, Case::Pool), 0)),
        m("determinism.crowd", differ as f32),
    ]
}

/// The slope limit a sweep shows: halfway from the steepest slope walked up to the next one
/// tried that stopped the body (2.5 deg past the steepest when none did).
fn limit(slopes: &[(f32, bool)]) -> f32 {
    let steepest = slopes
        .iter()
        .filter(|s| s.1)
        .map(|s| s.0)
        .fold(0.0, f32::max);
    let stopped = slopes
        .iter()
        .filter(|s| !s.1 && s.0 > steepest)
        .map(|s| s.0)
        .fold(f32::INFINITY, f32::min);
    if stopped.is_finite() {
        (steepest + stopped) * 0.5
    } else {
        steepest + 2.5
    }
}

/// How far a body's capsule is into the ground: its radius less the least distance along its
/// axis, sampled every 15 cm or so; negative when clear.
fn depth(field: &dyn Field, kind: &Kind, body: &Body) -> f32 {
    let a = body.feet + body.up * kind.radius;
    let b = body.feet + body.up * (kind.height - kind.radius);
    let nearest = (0..=8)
        .map(|i| field.distance(a.lerp(b, i as f32 / 8.0)))
        .fold(f32::INFINITY, f32::min);
    kind.radius - nearest
}

/// A run of one or more bodies through a fixture, the trace of each kept.
struct Run<'a, M> {
    mover: M,
    field: &'a dyn Field,
    kinds: Vec<(u32, Kind)>,
    depths: Vec<f32>,
    overlap: f32,
}

impl<'a, M: Mover> Run<'a, M> {
    fn new(mover: M, field: &'a dyn Field) -> Self {
        Run {
            mover,
            field,
            kinds: Vec::new(),
            depths: Vec::new(),
            overlap: 0.0,
        }
    }

    fn add(&mut self, id: u32, kind: Kind, feet: Vec3) -> Body {
        let facing = forward_at(feet);
        self.mover.add(id, &kind, feet, facing, self.field);
        self.kinds.push((id, kind));
        self.mover.body(id)
    }

    fn body(&self, id: u32) -> Body {
        self.mover.body(id)
    }

    /// A tick of every body, each with the intent its script gives it; keeps each body's
    /// depth into the ground and the worst overlap of any two.
    fn tick(&mut self, mut intent: impl FnMut(u32, &Body) -> Intent) {
        let intents: Vec<(u32, Intent)> = self
            .kinds
            .iter()
            .map(|(id, _)| (*id, intent(*id, &self.mover.body(*id))))
            .collect();
        self.mover.tick(self.field, &intents, TICK);
        let bodies: Vec<(Body, Kind)> = self
            .kinds
            .iter()
            .map(|(id, kind)| (self.mover.body(*id), *kind))
            .collect();
        for (i, (a, ka)) in bodies.iter().enumerate() {
            self.depths.push(depth(self.field, ka, a));
            for (b, kb) in &bodies[i + 1..] {
                let apart = (a.feet - b.feet).reject_from(a.up).length();
                self.overlap = self.overlap.max(ka.radius + kb.radius - apart);
            }
        }
    }

    fn outcome(self, values: Vec<f32>) -> Outcome {
        Outcome {
            values,
            depths: self.depths,
            overlap: self.overlap,
            end: Vec::new(),
        }
    }
}

fn walk(body: &Body) -> Intent {
    Intent {
        wish: forward_at(body.feet),
        ..Intent::default()
    }
}

fn up_speed(body: &Body) -> f32 {
    body.velocity.dot(body.up)
}

/// Ground under a profile of (`u`, `h`) corners.
fn ground(corners: &[(f32, f32)]) -> Course {
    Course::of(Solid::Profile(
        corners.iter().map(|&(u, h)| Vec2::new(u, h)).collect(),
    ))
}

/// Walls across the way at `u` 3 to 3.5 m, 3 m high, with a gap so wide at `w` = 0 (none at 0).
fn walls(gap: f32) -> Course {
    let (near, far) = (3.0, 3.5);
    let mut course = Course::flat();
    if gap <= 0.0 {
        return course.with(Solid::Block(
            Vec3::new(near, -1.0, -5.0),
            Vec3::new(far, 3.0, 5.0),
        ));
    }
    course = course.with(Solid::Block(
        Vec3::new(near, -1.0, gap * 0.5),
        Vec3::new(far, 3.0, 5.0),
    ));
    course.with(Solid::Block(
        Vec3::new(near, -1.0, -5.0),
        Vec3::new(far, 3.0, -gap * 0.5),
    ))
}

fn run<M: Mover>(new: &impl Fn() -> M, case: Case) -> Outcome {
    let kind = Kind::man();
    let flat = Course::flat();
    let start = world(0.0, 0.0, 0.0);
    let ticks = |s: f32| (s / TICK) as usize;
    match case {
        Case::Crowd(order) => crowd(new, order),
        Case::Step(height) => {
            let course = ground(&[(0.0, 0.0), (1.5, 0.0), (1.5, height), (6.0, height)]);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, start);
            for _ in 0..ticks(2.0) {
                run.tick(|_, b| walk(b));
            }
            let end = local(run.body(0).feet);
            let climbed = end.x > 2.0 && end.y > height - 0.05;
            let wrong = if height <= kind.step_height {
                !climbed
            } else {
                climbed
            };
            run.outcome(vec![f32::from(u8::from(wrong))])
        }
        Case::Slope(deg) => {
            let (s, c) = (libm::sinf(deg.to_radians()), libm::cosf(deg.to_radians()));
            let course = ground(&[(0.0, 0.0), (1.0, 0.0), (1.0 + 40.0 * c, 40.0 * s)]);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, start);
            for _ in 0..ticks(3.0) {
                run.tick(|_, b| walk(b));
            }
            let climbed = local(run.body(0).feet).x > 3.0;
            run.outcome(vec![f32::from(u8::from(climbed))])
        }
        Case::Gap(wider) => {
            let gap = 2.0 * kind.radius + wider;
            let course = walls(gap);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, start);
            for _ in 0..ticks(2.0) {
                run.tick(|_, b| walk(b));
            }
            let passed = local(run.body(0).feet).x > 3.5 + kind.radius;
            let wrong = if wider >= 0.04 {
                !passed
            } else {
                wider < 0.0 && passed
            };
            run.outcome(vec![f32::from(u8::from(wrong))])
        }
        Case::Wall(speed) => {
            let course = walls(0.0);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, start);
            let mut through = false;
            for _ in 0..ticks(1.0) {
                run.tick(|_, b| Intent {
                    drive: Some(forward_at(b.feet) * speed),
                    ..walk(b)
                });
                through |= local(run.body(0).feet).x > 3.0;
            }
            run.outcome(vec![f32::from(u8::from(through))])
        }
        Case::Through(speed) => {
            let mut run = Run::new(new(), &flat);
            run.add(0, kind, start);
            run.add(1, kind, world(3.0, 0.0, 0.0));
            let mut through = false;
            for _ in 0..ticks(0.5) {
                run.tick(|id, b| match id {
                    0 => Intent {
                        drive: Some(forward_at(b.feet) * speed),
                        ..walk(b)
                    },
                    _ => Intent::default(),
                });
                through |= local(run.body(0).feet).x > local(run.body(1).feet).x;
            }
            run.outcome(vec![f32::from(u8::from(through))])
        }
        Case::StickSlope | Case::StickStair => {
            let mut corners = vec![(-5.0, 0.0), (1.0, 0.0)];
            if matches!(case, Case::StickSlope) {
                corners.extend([(7.0, -6.0), (30.0, -6.0)]);
            } else {
                for i in 1..=10 {
                    let u = 1.0 + 0.3 * (i - 1) as f32;
                    corners.extend([(u, -0.3 * i as f32), (u + 0.3, -0.3 * i as f32)]);
                }
                corners.push((30.0, -3.0));
            }
            let bottom = corners[corners.len() - 2].0;
            let course = ground(&corners);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, start);
            let mut airborne = 0;
            for _ in 0..ticks(3.0) {
                run.tick(|_, b| Intent {
                    sprint: true,
                    ..walk(b)
                });
                let body = run.body(0);
                let u = local(body.feet).x;
                airborne += usize::from(!body.grounded && u > 1.0 && u < bottom);
            }
            run.outcome(vec![airborne as f32])
        }
        Case::Ceiling => {
            let roof = 2.0;
            let mut tunnel = Course::of(Solid::Rock);
            tunnel
                .dug
                .push((Vec3::new(-5.0, 0.0, -1.5), Vec3::new(20.0, roof, 1.5)));
            let mut run = Run::new(new(), &tunnel);
            run.add(0, kind, start);
            let (mut into, mut speed, mut touched) = (0.0_f32, 0.0, false);
            for tick in 0..ticks(1.5) {
                run.tick(|_, _| Intent {
                    jump: tick == 4,
                    ..Intent::default()
                });
                let body = run.body(0);
                let head = local(body.feet).y + kind.height;
                into = into.max(head - roof);
                if !touched && head >= roof - 0.01 {
                    touched = true;
                    speed = up_speed(&body).max(0.0);
                }
            }
            run.outcome(vec![into, speed])
        }
        Case::Fall(place) => {
            let ball = Ball { radius: 40.0 };
            let (dir, floor) = match place {
                0 => (Vec3::Y, ball.radius),
                1 => (Vec3::Z, ball.radius),
                2 => (Vec3::new(-1.0, 1.0, 1.0).normalize(), ball.radius),
                _ => (Vec3::X, Ball::CAVE_FLOOR),
            };
            let mut run = Run::new(new(), &ball);
            run.add(0, kind, dir * (floor + 0.5));
            for _ in 0..ticks(1.5) {
                run.tick(|_, _| Intent::default());
            }
            let body = run.body(0);
            let off = (body.feet - dir * floor).length();
            let tilt = libm::acosf(body.up.normalize().dot(dir).min(1.0)).to_degrees();
            run.outcome(vec![off, tilt])
        }
        Case::Apex => {
            let mut run = Run::new(new(), &flat);
            run.add(0, kind, start);
            let mut apex = 0.0_f32;
            for tick in 0..ticks(1.5) {
                run.tick(|_, _| Intent {
                    jump: tick == 4,
                    ..Intent::default()
                });
                apex = apex.max(local(run.body(0).feet).y);
            }
            let expected = kind.jump_speed * kind.jump_speed / (2.0 * flat.gravity());
            run.outcome(vec![(apex - expected).abs()])
        }
        Case::Coyote(late) => {
            let course = ground(&[(-5.0, 0.0), (2.0, 0.0), (2.0, -3.0), (20.0, -3.0)]);
            // The tick the body leaves the ledge, walking off it.
            let left = {
                let mut run = Run::new(new(), &course);
                run.add(0, kind, start);
                (1..ticks(2.0)).find(|_| {
                    run.tick(|_, b| walk(b));
                    !run.body(0).grounded
                })
            };
            let Some(left) = left else {
                return Outcome {
                    values: vec![0.0],
                    ..Outcome::default()
                };
            };
            let mut run = Run::new(new(), &course);
            run.add(0, kind, start);
            let mut jumped = false;
            for tick in 1..=left + late + 3 {
                run.tick(|_, b| Intent {
                    jump: tick == left + late,
                    ..walk(b)
                });
                jumped |= tick >= left + late && up_speed(&run.body(0)) > 0.5 * kind.jump_speed;
            }
            Outcome {
                values: vec![f32::from(u8::from(jumped))],
                ..Outcome::default()
            }
        }
        Case::Buffer(early) => {
            let drop_from = world(0.0, 1.5, 0.0);
            let landed = {
                let mut run = Run::new(new(), &flat);
                run.add(0, kind, drop_from);
                (1..ticks(2.0)).find(|_| {
                    run.tick(|_, _| Intent::default());
                    run.body(0).grounded
                })
            };
            let Some(landed) = landed.filter(|landed| *landed > early) else {
                return Outcome {
                    values: vec![0.0],
                    ..Outcome::default()
                };
            };
            let mut run = Run::new(new(), &flat);
            run.add(0, kind, drop_from);
            let mut jumped = false;
            for tick in 1..=landed + 3 {
                run.tick(|_, _| Intent {
                    jump: tick == landed - early,
                    ..Intent::default()
                });
                jumped |= tick >= landed && up_speed(&run.body(0)) > 0.5 * kind.jump_speed;
            }
            Outcome {
                values: vec![f32::from(u8::from(jumped))],
                ..Outcome::default()
            }
        }
        Case::Drop(height) => {
            let mut run = Run::new(new(), &flat);
            run.add(0, kind, world(0.0, height, 0.0));
            let (mut touched, mut grounded, mut bounce) = (None, None, 0.0_f32);
            for tick in 1..=ticks(2.0) {
                run.tick(|_, _| Intent::default());
                let body = run.body(0);
                let h = local(body.feet).y;
                if touched.is_none() && h <= 0.005 {
                    touched = Some(tick);
                }
                if grounded.is_none() && body.grounded {
                    grounded = Some(tick);
                }
                if grounded.is_some() {
                    bounce = bounce.max(h);
                }
            }
            let delay = match (touched, grounded) {
                (Some(t), Some(g)) => g.saturating_sub(t) as f32,
                _ => ticks(2.0) as f32,
            };
            run.outcome(vec![delay, bounce])
        }
        Case::Input => {
            let mut run = Run::new(new(), &flat);
            run.add(0, kind, start);
            for _ in 0..4 {
                run.tick(|_, _| Intent::default());
            }
            let moved = (1..=ticks(1.0)).find(|_| {
                run.tick(|_, b| walk(b));
                let body = run.body(0);
                body.velocity.dot(forward_at(body.feet)) > 0.0
            });
            run.outcome(vec![moved.unwrap_or(ticks(1.0)) as f32])
        }
        Case::Pool => {
            let mut pool = Course::flat();
            pool.dug
                .push((Vec3::new(2.0, -3.0, -4.0), Vec3::new(8.0, 1.0, 4.0)));
            pool.water = Some(Water {
                min: Vec2::new(2.0, -4.0),
                max: Vec2::new(8.0, 4.0),
                surface: -0.2,
            });
            let mut run = Run::new(new(), &pool);
            run.add(0, kind, start);
            for _ in 0..ticks(3.0) {
                run.tick(|_, b| walk(b));
            }
            let walled = local(run.body(0).feet).x < 3.0;
            run.outcome(vec![f32::from(u8::from(walled))])
        }
        Case::Share(reversed) | Case::Fixed(reversed) => {
            // A man and a body four times his mass, or a fixed one, overlapping by 20 cm.
            let heavy = Kind {
                mass: if matches!(case, Case::Share(_)) {
                    kind.mass * 4.0
                } else {
                    f32::INFINITY
                },
                ..kind
            };
            let (a, b) = (start, world(2.0 * kind.radius - 0.2, 0.0, 0.0));
            let mut run = Run::new(new(), &flat);
            let ids = if reversed { [1, 0] } else { [0, 1] };
            for id in ids {
                if id == 0 {
                    run.add(0, kind, a)
                } else {
                    run.add(1, heavy, b)
                };
            }
            run.tick(|_, _| Intent::default());
            let (da, db) = (
                (run.body(0).feet - a).length(),
                (run.body(1).feet - b).length(),
            );
            let value = match case {
                Case::Fixed(_) => db,
                _ if da + db < 1e-6 => 1.0,
                _ => (da / (da + db) - heavy.mass / (kind.mass + heavy.mass)).abs(),
            };
            run.outcome(vec![value])
        }
    }
}

/// The crowd course: bodies on a grid walking and sprinting across to the far side of the
/// middle and back, through each other, for [`CROWD_TICKS`]; added and ticked in the order
/// given. Its end state is every body's numbers' bits, by ID.
fn crowd<M: Mover>(new: &impl Fn() -> M, order: Order) -> Outcome {
    let flat = Course::flat();
    let kind = Kind::man();
    let place = |id: u32| {
        let (col, row) = ((id % 6) as f32, (id / 6) as f32);
        Vec2::new(col * 1.5 - 3.75, row * 1.5 - 2.25)
    };
    let mut ids: Vec<u32> = (0..CROWD).collect();
    match order {
        Order::Forward | Order::Again => {}
        Order::Reversed => ids.reverse(),
        Order::Interleaved => ids.sort_by_key(|id| (id % 2, *id)),
    }
    let mut run = Run::new(new(), &flat);
    for &id in &ids {
        let at = place(id);
        run.add(id, kind, world(at.x, 0.0, at.y));
    }
    // Each body heads for its place mirrored through the middle, and back again.
    let mut back = vec![false; CROWD as usize];
    for _ in 0..CROWD_TICKS {
        run.tick(|id, body| {
            let home = place(id);
            let goal = if back[id as usize] { home } else { -home };
            let here = local(body.feet);
            let to = goal - Vec2::new(here.x, here.z);
            if to.length() < 0.5 {
                back[id as usize] = !back[id as usize];
            }
            let wish =
                (forward_at(body.feet) * to.x + side_at(body.feet) * to.y).normalize_or_zero();
            Intent {
                wish,
                sprint: id % 3 == 0,
                ..Intent::default()
            }
        });
    }
    let end = (0..CROWD)
        .flat_map(|id| {
            let body = run.body(id);
            [body.feet, body.velocity]
                .into_iter()
                .flat_map(|v| v.to_array())
                .map(f32::to_bits)
        })
        .collect();
    Outcome {
        end,
        overlap: run.overlap,
        ..Outcome::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slope_sweep_reads_its_limit_halfway_between_walked_and_stopped() {
        let sweep = |limit: f32| -> Vec<(f32, bool)> {
            (0..=140)
                .map(|i| i as f32 * 0.5)
                .map(|deg| (deg, deg <= limit))
                .collect()
        };
        assert!((super::limit(&sweep(50.0)) - 50.25).abs() < 1e-6);
        // Unsorted, and walked up everything tried.
        let mut all = sweep(90.0);
        all.reverse();
        assert!((super::limit(&all) - 72.5).abs() < 1e-6);
    }

    #[test]
    fn a_capsule_on_level_ground_is_clear_and_one_sunk_is_in_by_as_much() {
        let (flat, kind) = (Course::flat(), Kind::man());
        let at = |h: f32| {
            let feet = world(1.0, h, 0.0);
            let body = Body {
                feet,
                up: feet.normalize(),
                velocity: Vec3::ZERO,
                grounded: true,
            };
            depth(&flat, &kind, &body)
        };
        assert!(at(0.0).abs() < 1e-3);
        assert!((at(-0.1) - 0.1).abs() < 1e-3);
        assert!((at(0.5) + 0.5).abs() < 1e-3);
    }
}
