//! Stumps and logs (docs/controller.md criterion 5, `step.flora`): a thing of flora is fitted
//! as the game fits it (`terra_core::Solid::fit` of a relief: convex pieces, each as tall as
//! the highest of it, split where its outline is long or loose), so a stump with roots is
//! pieces as tall as its trunk over its roots and a log's thin end a lower piece than its
//! thick end. One taller than the kind's step is walked at from eight ways, on level ground
//! and on a walkable slope, and is never climbed: not by way of its thin end's lower piece, by
//! the round of a piece's edge a little over the one stood on, nor from the slope's high side
//! (11 of its 64 walks before the rule). One no taller than a step is stepped onto from every
//! way, as a rock is (`step.rock`). Asked of the walk alone ([`Kind::walking`]): climbing one
//! after half a second's push (D507) is climbing's to ask.

use super::field::{Course, Solid as Ground, forward_at, local, side_at, world};
use super::runs::{Outcome, Run};
use super::{Intent, Kind, Measure, Mover, TICK};
use crate::mover::Obstacle;
use glam::{Vec2, Vec3};
use terra_core::{Placement, Relief, Solid};

/// The relief's cell, m: as fine as flora's fitting takes a thing a metre or two across.
const CELL_M: f32 = 0.05;
/// How far from a thing's middle each walk begins, m, and how long it walks, s.
const START_M: f32 = 3.5;
const WALK_S: f32 = 2.0;
/// The slope a tall thing also stands on, deg: well within a man's 50.
const SLOPE_DEG: f32 = 20.0;

#[derive(Clone, Copy, Debug)]
enum Thing {
    /// A stump as high as given over level ground, 0.35 m round, its five roots 0.3 m high
    /// where they leave it, running out 0.5 m.
    Stump(f32),
    /// A log 2.4 m long lying on the ground across the way (along a slope's contour), as
    /// thick as given at one end and 0.22 m round at the other.
    Log(f32),
}

impl Thing {
    /// Its height at a point of its plan over level ground, if it stands there.
    fn height(self, at: Vec2) -> Option<f32> {
        match self {
            Thing::Stump(high) => {
                let d = at.length();
                if d <= 0.35 {
                    return Some(high);
                }
                (0..5)
                    .filter_map(|k| {
                        let angle = k as f32 * std::f32::consts::TAU / 5.0 + 0.3;
                        let way = Vec2::new(libm::cosf(angle), libm::sinf(angle));
                        let (along, off) = (at.dot(way), at.perp_dot(way).abs());
                        (off < 0.07 && (0.3..0.85).contains(&along))
                            .then(|| 0.3 * (1.0 - (along - 0.3) / 0.55))
                    })
                    .reduce(f32::max)
            }
            Thing::Log(thick) => {
                let (along, across) = (at.y, at.x);
                if along.abs() > 1.2 {
                    return None;
                }
                let t = along / 2.4 + 0.5;
                let r = 0.22 + (thick * 0.5 - 0.22) * t;
                (across.abs() < r).then(|| r + libm::sqrtf(r * r - across * across))
            }
        }
    }

    /// Its relief: its height at each cell over a grid round it.
    fn relief(self) -> Relief {
        let n = (1.4 / CELL_M) as i32;
        let cells = (-n..=n)
            .flat_map(|i| (-n..=n).map(move |j| Vec2::new(i as f32, j as f32) * CELL_M))
            .filter_map(|at| self.height(at).map(|h| Vec3::new(at.x, h, at.y)))
            .collect();
        Relief {
            cells,
            cell: CELL_M,
        }
    }

    fn tall(self) -> f32 {
        match self {
            Thing::Stump(high) | Thing::Log(high) => high,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Case {
    thing: Thing,
    /// The ground's grade along the way, rise over run.
    grade: f32,
    /// Which way it is walked at, deg round from the way.
    toward: f32,
}

fn cases(kind: &Kind) -> Vec<Case> {
    let grade = libm::tanf(SLOPE_DEG.to_radians());
    let tall = [Thing::Stump(0.65), Thing::Stump(0.75), Thing::Log(0.68)];
    let low = [Thing::Stump(kind.step_height - 0.15), Thing::Log(0.45)];
    let ways = (0..8).map(|k| k as f32 * 45.0);
    let mut cases = Vec::new();
    for toward in ways {
        for thing in tall {
            for grade in [0.0, grade] {
                cases.push(Case {
                    thing,
                    grade,
                    toward,
                });
            }
        }
        for thing in low {
            cases.push(Case {
                thing,
                grade: 0.0,
                toward,
            });
        }
    }
    cases
}

/// The ground along the way at a grade, and the thing fitted on it at its middle.
fn course(case: Case) -> Course {
    let g = case.grade;
    let mut course = Course::of(Ground::Profile(vec![
        Vec2::new(-50.0, -50.0 * g),
        Vec2::new(50.0, 50.0 * g),
    ]));
    let ground = world(0.0, 0.0, 0.0);
    let place = Placement {
        ground,
        x: forward_at(ground),
        z: side_at(ground),
        up: ground.normalize(),
        scale: 1.0,
        lift: 0.0,
    };
    let solid = Solid::fit(&case.thing.relief(), place, |at| at.x * g, Vec::new())
        .expect("a stump or log shows above the ground");
    course.obstacles = vec![Obstacle::Solid(solid)];
    course
}

/// Whether the walk went wrong: a tall thing climbed, its feet more than a step over the
/// ground under them; or a low one not stepped onto, never lifted most of its height.
fn run<M: Mover>(new: &impl Fn() -> M, case: Case) -> Outcome {
    let kind = Kind::man().walking();
    let course = course(case);
    let way = Vec2::from_angle(case.toward.to_radians());
    let from = -way * START_M;
    let start = world(from.x, from.x * case.grade, from.y);
    let mut run = Run::new(new(), &course);
    run.add(0, kind, start);
    let mut lift = 0.0f32;
    for _ in 0..(WALK_S / TICK) as usize {
        run.tick(|_, b| Intent {
            wish: forward_at(b.feet) * way.x + side_at(b.feet) * way.y,
            ..Intent::default()
        });
        let at = local(run.body(0).feet);
        lift = lift.max(at.y - at.x * case.grade);
    }
    let tall = case.thing.tall() > kind.step_height;
    let wrong = if tall {
        lift > kind.step_height + 0.01
    } else {
        lift < case.thing.tall() * 0.8
    };
    run.outcome(vec![f32::from(u8::from(wrong))])
}

/// `step.flora`: how many of the walks at stumps and logs went wrong.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> (Vec<Measure>, f32) {
    let cases = cases(&Kind::man());
    let outcomes = terra_gen::par::map(&cases, |case| run(new, *case));
    let wrong = outcomes.iter().map(|o| o.values[0]).sum();
    let deepest = outcomes
        .iter()
        .flat_map(|o| o.depths.iter().copied())
        .fold(0.0, f32::max);
    (
        vec![Measure {
            id: "step.flora",
            value: wrong,
        }],
        deepest,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture is what it says: the fitter makes a log's thin end a piece under a step
    /// and its thick end one over, so a body can step onto the one and walk on to the other.
    #[test]
    fn a_log_fits_to_a_thin_end_under_a_step_and_a_thick_end_over() {
        let course = course(Case {
            thing: Thing::Log(0.68),
            grade: 0.0,
            toward: 0.0,
        });
        let Obstacle::Solid(solid) = &course.obstacles[0] else {
            panic!("a solid");
        };
        let tops: Vec<f32> = solid.pieces.iter().map(|p| p.top).collect();
        let step = Kind::man().step_height;
        assert!(tops.iter().any(|t| *t < step), "{tops:?}");
        assert!(tops.iter().any(|t| (t - 0.68).abs() < 0.02), "{tops:?}");
    }
}
