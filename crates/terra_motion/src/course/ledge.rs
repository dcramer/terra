//! The cases of climbing out (D523): a swimmer at the surface of deep water swims into a
//! bank standing straight up out of it, straight at it and 30 deg off, for a man and the
//! second kind. A bank no higher over the water than the kind's `climb_out` it climbs out
//! onto; a higher one it never does. The banks are shares of the kind's limit either side of
//! it, so each answer is known from the kind's number alone.

use super::field::{Course, Water, forward_at, local, side_at, world};
use super::runs::{Outcome, Run};
use super::{Intent, Kind, Measure, Mover, Solid, TICK};
use glam::Vec2;

/// Banks' heights over the water as shares of the kind's `climb_out`: those under 1 climbed
/// onto, those over it not.
const BANKS: [f32; 5] = [0.35, 0.7, 0.9, 1.2, 1.5];
/// Ways swum at the bank, deg off straight at it.
const ANGLES: [f32; 2] = [0.0, 30.0];
/// Where the bank's face stands, m along the way from where the swimmer starts.
const FACE_U: f32 = 6.0;
/// How deep the pool is, m.
const POOL_M: f32 = 4.0;
/// How long the swimmer floats before it swims, s.
const SETTLE_S: f32 = 1.0;
/// How much longer than the swim to the face takes at the kind's swim speed it is given, and
/// a time on top of it for climbing out.
const SLACK: f32 = 1.5;
const CLIMB_S: f32 = 3.0;

#[derive(Clone, Copy, Debug)]
struct Case {
    /// The man or the second kind.
    kind: usize,
    /// The bank's height over the water, a share of the kind's `climb_out`.
    bank: f32,
    angle: f32,
}

fn kinds() -> [Kind; 2] {
    let man = Kind::man();
    [man, super::data::second(&man)]
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for kind in 0..2 {
        for bank in BANKS {
            for angle in ANGLES {
                cases.push(Case { kind, bank, angle });
            }
        }
    }
    cases
}

/// Deep water to a bank at `FACE_U` standing `bank` m over its surface (at 0).
fn bank(bank: f32) -> Course {
    let corners = [
        (-20.0, -POOL_M),
        (FACE_U, -POOL_M),
        (FACE_U, bank),
        (60.0, bank),
    ];
    let mut course = Course::of(Solid::Profile(
        corners.iter().map(|&(u, h)| Vec2::new(u, h)).collect(),
    ));
    course.water = Some(Water {
        min: Vec2::new(-20.0, -50.0),
        max: Vec2::new(FACE_U, 50.0),
        surface: 0.0,
    });
    course
}

/// 1 when the swimmer ended standing on the bank's top, else 0.
fn run<M: Mover>(new: &impl Fn() -> M, case: Case) -> Outcome {
    let kind = kinds()[case.kind];
    let height = case.bank * kind.climb_out;
    let course = bank(height);
    let mut run = Run::new(new(), &course);
    run.add(0, kind, world(0.0, -kind.float_depth * kind.height, 0.0));
    for _ in 0..(SETTLE_S / TICK) as usize {
        run.tick(|_, _| Intent::default());
    }
    let (s, c) = (
        libm::sinf(case.angle.to_radians()),
        libm::cosf(case.angle.to_radians()),
    );
    let way = FACE_U / c;
    let swim_s = way / kind.swim * SLACK + CLIMB_S;
    for _ in 0..(swim_s / TICK) as usize {
        run.tick(|_, b| Intent {
            wish: forward_at(b.feet) * c + side_at(b.feet) * s,
            ..Intent::default()
        });
    }
    let body = run.body(0);
    let at = local(body.feet);
    let on_top = body.grounded && at.x > FACE_U + kind.radius * 0.5 && at.y > height - 0.05;
    run.outcome(vec![f32::from(u8::from(on_top))])
}

/// The climbing-out measures, and the deepest any body went into the ground.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> (Vec<Measure>, f32) {
    let cases = cases();
    let outcomes = terra_gen::par::map(&cases, |case| run(new, *case));
    let share = |high: bool, wrong: f32| {
        let of: Vec<f32> = cases
            .iter()
            .zip(&outcomes)
            .filter(|(c, _)| (c.bank > 1.0) == high)
            .map(|(_, o)| (o.values[0] - wrong).abs())
            .collect();
        of.iter().sum::<f32>() / of.len() as f32
    };
    let deepest = outcomes
        .iter()
        .flat_map(|o| o.depths.iter().copied())
        .fold(0.0, f32::max);
    let m = |id, value: f32| Measure { id, value };
    (
        vec![
            m("swim.ledge", share(false, 1.0)),
            m("swim.ledge.high", share(true, 0.0)),
        ],
        deepest,
    )
}
