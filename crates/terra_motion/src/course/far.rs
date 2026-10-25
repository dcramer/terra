//! The cases far from the world's origin (D482; Jolt's large worlds, Docs/BigWorlds.md): the
//! same walks on a world [`FAR_M`] across as on the course's own [`RADIUS_M`], where an f32
//! point is only about 2 mm fine. A body walking level ground, up and down a slope and up a
//! flight of the ground's steps keeps its feet on the ground every tick and its capsule's gap
//! to the ground as steady as near the origin, and asks the field no more often a tick.

use super::field::{Course, Field, RADIUS_M, forward_at};
use super::runs::ground;
use super::{Intent, Kind, Measure, Mover, TICK};
use crate::mover::Obstacle;
use glam::{DVec3, Vec3};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The far world's radius, m: the lake world is 30 km across (D482).
pub(super) const FAR_M: f32 = 30_000.0;
/// The slope walked up and down, deg, and how long each walk runs, s.
const SLOPE_DEG: f32 = 20.0;
const WALK_S: f32 = 3.0;
/// The flight's rise and tread, m: under the man's step, so it is walked up.
const RISE: f32 = 0.3;
const TREAD: f32 = 0.5;
/// How long a body settles before its gap is read, s.
const SETTLE_S: f32 = 0.25;
/// How far a far walk's feet may stray from the same walk's near the origin, m: a tick's move
/// at a sprint, so a riser climbed a tick earlier or later is not counted, a body stuck is.
/// (The two worlds' curves differ: 2 km's 20 deg slope walks 7 mm apart from 30 km's in 3 s.)
const WAY_M: f64 = 0.1;

#[derive(Clone, Copy, Debug)]
enum Ground {
    Level,
    Up,
    Down,
    Flight,
}

const GROUNDS: [Ground; 4] = [Ground::Level, Ground::Up, Ground::Down, Ground::Flight];

/// A fixture's ground, the same at any radius.
fn fixture(ground_of: Ground) -> Course {
    let (s, c) = (
        libm::sinf(SLOPE_DEG.to_radians()),
        libm::cosf(SLOPE_DEG.to_radians()),
    );
    match ground_of {
        Ground::Level => Course::flat(),
        Ground::Up => ground(&[(-5.0, 0.0), (1.0, 0.0), (1.0 + 40.0 * c, 40.0 * s)]),
        Ground::Down => ground(&[(-5.0, 0.0), (1.0, 0.0), (1.0 + 40.0 * c, -40.0 * s)]),
        Ground::Flight => {
            let mut corners = vec![(-5.0, 0.0)];
            for i in 0..8 {
                let u = 1.0 + i as f32 * TREAD;
                corners.extend([(u, i as f32 * RISE), (u, (i + 1) as f32 * RISE)]);
            }
            corners.push((20.0, 8.0 * RISE));
            ground(&corners)
        }
    }
}

/// A fixture that counts the field's calls.
struct Counted {
    course: Course,
    calls: AtomicUsize,
}

impl Field for Counted {
    fn distance(&self, at: Vec3) -> f32 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.course.distance(at)
    }

    fn distance_at(&self, at: DVec3) -> f32 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.course.distance_at(at)
    }

    fn water(&self, at: Vec3) -> Option<f32> {
        self.course.water(at)
    }

    fn obstacles(&self) -> &[Obstacle] {
        self.course.obstacles()
    }
}

/// A tick of a walk: on the ground or not, the feet in the fixture's level frame, and the gap
/// from the foot's sphere to the ground (m).
#[derive(Clone, Copy, Debug)]
struct Tick {
    grounded: bool,
    place: DVec3,
    gap: f32,
}

/// A walk's ticks, and its field calls a tick.
#[derive(Clone, Debug, Default)]
struct Walk {
    ticks: Vec<Tick>,
    calls: f32,
}

fn walk<M: Mover>(new: &impl Fn() -> M, ground_of: Ground, radius: f32) -> Walk {
    let kind = Kind::man();
    let field = Counted {
        course: fixture(ground_of).on(radius),
        calls: AtomicUsize::new(0),
    };
    let frame = field.course.frame();
    let start = frame.world(0.0, 0.0, 0.0).as_vec3();
    let mut mover = new();
    mover.add(0, &kind, start, forward_at(start), &field);
    field.calls.store(0, Ordering::Relaxed);
    let count = (WALK_S / TICK) as usize;
    let ticks = (0..count)
        .map(|_| {
            let intent = Intent {
                wish: forward_at(mover.body(0).feet),
                ..Intent::default()
            };
            mover.tick(&field, &[(0, intent)], TICK);
            let body = mover.body(0);
            // The foot's sphere's centre, its gap to the ground asked in f64.
            let centre = body.position + body.up.as_dvec3() * f64::from(kind.radius);
            Tick {
                grounded: body.grounded,
                place: frame.local(body.position),
                gap: field.course.distance_at(centre) - kind.radius,
            }
        })
        .collect();
    Walk {
        ticks,
        calls: field.calls.load(Ordering::Relaxed) as f32 / count as f32,
    }
}

/// The far rows: the ticks at [`FAR_M`] a body is off the ground or its feet more than
/// [`WAY_M`] from where the same walk's are near the origin; the most a foot's gap to the
/// ground there differs from near the origin's, tick by tick, once settled; and the field calls
/// a tick there over those near the origin.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> Vec<Measure> {
    let cases: Vec<(Ground, f32)> = GROUNDS
        .iter()
        .flat_map(|g| [(*g, RADIUS_M), (*g, FAR_M)])
        .collect();
    let walks = terra_gen::par::map(&cases, |(g, radius)| walk(new, *g, *radius));
    // Each ground's near walk, then its far one, tick by tick.
    let pairs = || walks.chunks(2).map(|pair| (&pair[0], &pair[1]));
    let ticks = || pairs().flat_map(|(near, far)| near.ticks.iter().zip(&far.ticks));
    let settle = (SETTLE_S / TICK) as usize;
    // How far apart along the ground (`u`, `w`): a riser climbed a tick apart is no stray.
    let level = |d: DVec3| d.x.hypot(d.z);
    let off = ticks()
        .filter(|(near, far)| !far.grounded || level(near.place - far.place) > WAY_M)
        .count();
    let float = pairs()
        .flat_map(|(near, far)| near.ticks.iter().zip(&far.ticks).skip(settle))
        .map(|(near, far)| (near.gap - far.gap).abs())
        .fold(0.0, f32::max);
    let calls = |far: bool| {
        pairs()
            .map(|(n, f)| if far { f.calls } else { n.calls })
            .sum::<f32>()
    };
    vec![
        Measure {
            id: "far.grounded",
            value: off as f32,
        },
        Measure {
            id: "far.float",
            value: float,
        },
        Measure {
            id: "far.queries",
            value: calls(true) / calls(false).max(1.0),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_fixture_is_the_same_ground_at_either_radius() {
        // So a far walk differs from its near one by precision alone.
        for g in GROUNDS {
            let (near, far) = (fixture(g), fixture(g).on(FAR_M));
            for (u, h) in [(0.0, 0.5), (1.6, 0.4), (3.2, -0.2), (6.0, 2.0)] {
                let a = near.distance_at(near.frame().world(u, h, 0.3));
                let b = far.distance_at(far.frame().world(u, h, 0.3));
                assert!((a - b).abs() < 1e-5, "{g:?} at {u}, {h}: {a} near, {b} far");
            }
        }
    }
}
