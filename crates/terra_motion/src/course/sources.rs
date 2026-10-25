//! The cases of what stops a body besides the ground (docs/controller.md criteria 1 and 5): a
//! step of each source, a base's cubes, a prop's box and a rock, climbed up to the kind's step
//! height and refused 1 cm higher, as the ground's steps are; and a door's leaf in a base's
//! wall, driven at from 2 to 60 m/s. Each stands on level ground as the mover's obstacles, apart
//! from the ground's field, as the game's base, props and rocks stand on its terrain.

use super::field::{Course, RADIUS_M, forward_at, local, side_at, world};
use super::runs::{Outcome, Run, SPEEDS, STEPS, walk};
use super::{Intent, Kind, Measure, Mover, TICK};
use crate::mover::{Block, Obstacle};
use glam::{Vec2, Vec3};
use terra_core::{Piece, Solid};

/// A base's cube, m (`terra_build::CELL_M`).
pub(super) const CELL_M: f32 = 0.5;
/// Where each step's face is along the way, and how far on its top runs, m.
const FACE_U: f32 = 1.5;
const TOP_U: f32 = 10.0;
/// How long a body walks at a step, s.
const WALK_S: f32 = 1.5;

#[derive(Clone, Copy, Debug)]
enum Source {
    /// A base's cubes: a floor of them, the step's face and top built of whole cubes and the
    /// top layer's part of one.
    Building,
    /// A prop's box.
    Prop,
    /// A round rock.
    Rock,
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Step(Source, f32),
    Door(f32),
}

/// A box in the level frame from one corner to the other, turned to the frame at a place (its
/// top flat there, as a base or a prop is laid level).
pub(super) fn block(at: Vec3, min: Vec3, max: Vec3) -> Obstacle {
    let origin = world(at.x, at.y, at.z);
    let up = origin.normalize();
    let axes = [forward_at(origin), up, side_at(origin)];
    let middle = (min + max) * 0.5 - at;
    Obstacle::Block(Block {
        centre: origin + axes[0] * middle.x + axes[1] * middle.y + axes[2] * middle.z,
        axes,
        half: (max - min) * 0.5,
    })
}

/// A base's cubes filling a box of the base's frame (whose origin is at `at`), the top layer
/// filled as far as `max` reaches into it.
pub(super) fn cubes(at: Vec3, min: Vec3, max: Vec3) -> Vec<Obstacle> {
    let cells = ((max - min) / CELL_M).ceil();
    let mut out = Vec::new();
    for i in 0..cells.x as i32 {
        for j in 0..cells.y as i32 {
            for k in 0..cells.z as i32 {
                let low = min + Vec3::new(i as f32, j as f32, k as f32) * CELL_M;
                let high = (low + Vec3::splat(CELL_M)).min(max);
                out.push(block(at, low, high));
            }
        }
    }
    out
}

/// Level ground with a step of a source on it, its face at [`FACE_U`].
fn step(source: Source, height: f32) -> Course {
    let mut course = Course::flat();
    let (near, far) = (FACE_U, TOP_U);
    course.obstacles = match source {
        // Sunk a cube into the ground, as a base's floor stands in its dug footing.
        Source::Building => cubes(
            Vec3::new(near, 0.0, 0.0),
            Vec3::new(near, -CELL_M, -2.0),
            Vec3::new(far, height, 2.0),
        ),
        // Laid level at its face, so it is as high as named where it is climbed.
        Source::Prop => vec![block(
            Vec3::new(near, 0.0, 0.0),
            Vec3::new(near, -0.1, -2.0),
            Vec3::new(far, height, 2.0),
        )],
        Source::Rock => {
            let radius = (far - near) * 0.5;
            let middle = world(near + radius, 0.0, 0.0);
            // Its top is level at its middle, so its crest is lowered by the world's curve
            // there, to stand as high as named over the ground at its face, where it is climbed.
            let crest = libm::sqrtf((RADIUS_M + height).powi(2) - radius * radius) - RADIUS_M;
            let piece = Piece::round(Vec2::ZERO, radius, crest);
            vec![Obstacle::Solid(Solid::of_pieces(middle, vec![piece]))]
        }
    };
    course
}

/// A base's wall of cubes across the way at `u` 3 to 3.5 m, 2.5 m high, with a doorway a metre
/// wide at `w` = 0 under a lintel.
pub(super) fn doorway() -> Vec<Obstacle> {
    let at = Vec3::new(DOOR_U, 0.0, 0.0);
    let (near, far, high) = (DOOR_U, DOOR_U + CELL_M, 2.5);
    let mut wall = cubes(at, Vec3::new(near, -CELL_M, 0.5), Vec3::new(far, high, 5.0));
    wall.extend(cubes(
        at,
        Vec3::new(near, -CELL_M, -5.0),
        Vec3::new(far, high, -0.5),
    ));
    wall.extend(cubes(
        at,
        Vec3::new(near, 2.0, -0.5),
        Vec3::new(far, high, 0.5),
    ));
    wall
}

/// Where a doorway's wall begins along the way, m.
pub(super) const DOOR_U: f32 = 3.0;

/// The doorway with its door's leaf, 4 cm thick, shut on the wall's face.
fn door() -> Course {
    let mut course = Course::flat();
    course.obstacles = doorway();
    course.obstacles.push(block(
        Vec3::new(DOOR_U, 0.0, 0.0),
        Vec3::new(DOOR_U - 0.04, 0.0, -0.5),
        Vec3::new(DOOR_U, 2.0, 0.5),
    ));
    course
}

fn cases(kind: &Kind) -> Vec<Case> {
    let heights = STEPS
        .into_iter()
        .chain([kind.step_height, kind.step_height + 0.01]);
    let mut cases = Vec::new();
    for source in [Source::Building, Source::Prop, Source::Rock] {
        cases.extend(heights.clone().map(|h| Case::Step(source, h)));
    }
    cases.extend(SPEEDS.map(Case::Door));
    cases
}

fn run<M: Mover>(new: &impl Fn() -> M, case: Case) -> Outcome {
    let kind = Kind::man();
    let start = world(0.0, 0.0, 0.0);
    let ticks = |s: f32| (s / TICK) as usize;
    match case {
        Case::Step(source, height) => {
            let course = step(source, height);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, start);
            for _ in 0..ticks(WALK_S) {
                run.tick(|_, b| walk(b));
            }
            let end = local(run.body(0).feet);
            let climbed = end.x > FACE_U + 0.5 && end.y > height - 0.05;
            let wrong = climbed != (height <= kind.step_height);
            run.outcome(vec![f32::from(u8::from(wrong))])
        }
        Case::Door(speed) => {
            let course = door();
            let mut run = Run::new(new(), &course);
            run.add(0, kind, start);
            let mut through = false;
            for _ in 0..ticks(1.0) {
                run.tick(|_, b| Intent {
                    drive: Some(forward_at(b.feet) * speed),
                    ..walk(b)
                });
                through |= local(run.body(0).feet).x > 2.96;
            }
            run.outcome(vec![f32::from(u8::from(through))])
        }
    }
}

/// The sources' measures, and the deepest any body went into what it met.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> (Vec<Measure>, f32) {
    let kind = Kind::man();
    let cases = cases(&kind);
    let outcomes = terra_gen::par::map(&cases, |case| run(new, *case));
    let all = || cases.iter().zip(&outcomes);
    let wrong = |want: &dyn Fn(&Case) -> bool| {
        all()
            .filter(|(case, _)| want(case))
            .map(|(_, o)| o.values[0])
            .sum::<f32>()
    };
    let deepest = outcomes
        .iter()
        .flat_map(|o| o.depths.iter().copied())
        .fold(0.0, f32::max);
    let m = |id, value: f32| Measure { id, value };
    let measures = vec![
        m(
            "step.building",
            wrong(&|c| matches!(c, Case::Step(Source::Building, _))),
        ),
        m(
            "step.prop",
            wrong(&|c| matches!(c, Case::Step(Source::Prop, _))),
        ),
        m(
            "step.rock",
            wrong(&|c| matches!(c, Case::Step(Source::Rock, _))),
        ),
        m("tunnel.door", wrong(&|c| matches!(c, Case::Door(_)))),
    ];
    (measures, deepest)
}
