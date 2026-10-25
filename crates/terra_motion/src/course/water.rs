//! The cases of water (docs/controller.md criterion 17, D74): water is a medium, not a
//! contact. A body walks at a jog down a shelving shore into a lake, wades until the water is
//! deeper than its kind's swim depth, swims across and walks out up the far shore, straight or
//! at an angle to it; and one put deep in the water floats up to rest with its kind's float
//! depth under the surface. Each answer is the fixture's geometry or the kind's numbers.

use super::field::{Course, Water, local, world};
use super::runs::{Outcome, Run, ground};
use super::{Intent, Kind, Measure, Mover, TICK};
use glam::{Vec2, Vec3};
use terra_gen::smoothstep;

/// The water's surface over the lake, m: under the dry ground at 0, as a lake's is.
pub(super) const SURFACE: f32 = -0.2;
/// The lake's bed under the dry ground, m, and how long its flat middle is.
const BED: f32 = -2.5;
const MIDDLE_M: f32 = 6.0;
/// Where the near shore starts down, and how high the far shore's dry ground stands, m.
const NEAR_U: f32 = 2.0;
const FAR_H: f32 = 0.3;
/// The shores' steepest grades, deg: each a smooth curve (a smoothstep), sampled every
/// [`SAMPLE_M`], so the ground bends by less than a degree from sample to sample, as the
/// planet's does.
const GRADES: [f32; 2] = [12.0, 25.0];
const SAMPLE_M: f32 = 0.25;
/// The ways walked across, deg off straight across the shores.
const ANGLES: [f32; 3] = [0.0, 25.0, 45.0];
/// How much longer than the walk takes at the kind's speeds (its jog where it wades, its swim
/// where it swims) a body is given to get out on the far side.
const SLACK: f32 = 1.5;
/// How deep under the surface a floated body is put, m, and how long it is given to come up.
const PUT_M: f32 = 3.0;
const FLOAT_S: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
enum Case {
    /// Across the lake whose shores are at most so steep, at an angle off straight across.
    Across(f32, f32),
    Float,
}

fn cases() -> Vec<Case> {
    let mut cases: Vec<Case> = GRADES
        .iter()
        .flat_map(|&grade| ANGLES.map(|angle| Case::Across(grade, angle)))
        .collect();
    cases.push(Case::Float);
    cases
}

/// How long a shore of a grade runs, m: a smoothstep's steepest grade is 1.5 its mean.
fn shore_m(grade: f32, drop: f32) -> f32 {
    1.5 * drop / libm::tanf(grade.to_radians())
}

/// The lake's ground along the way: dry, down the near shore, the bed, up the far shore,
/// dry; and where the far shore's dry ground starts.
pub(super) fn lake(grade: f32) -> (Course, f32) {
    let (down, up) = (shore_m(grade, -BED), shore_m(grade, FAR_H - BED));
    let bed_u = NEAR_U + down;
    let rise_u = bed_u + MIDDLE_M;
    let dry_u = rise_u + up;
    let height = |u: f32| {
        if u < bed_u {
            BED * smoothstep(0.0, 1.0, (u - NEAR_U) / down)
        } else if u < rise_u {
            BED
        } else {
            BED + (FAR_H - BED) * smoothstep(0.0, 1.0, (u - rise_u) / up)
        }
    };
    let samples = ((dry_u - NEAR_U) / SAMPLE_M).ceil() as usize;
    let mut corners = vec![(-10.0, 0.0)];
    corners.extend((0..=samples).map(|i| {
        let u = NEAR_U + i as f32 * SAMPLE_M;
        (u, height(u))
    }));
    corners.push((dry_u + 40.0, FAR_H));
    let mut course = ground(&corners);
    course.water = Some(Water {
        min: Vec2::new(NEAR_U, -200.0),
        max: Vec2::new(dry_u, 200.0),
        surface: SURFACE,
    });
    (course, dry_u)
}

/// How long the walk across takes at the kind's speeds, s: its jog where the water is no
/// deeper than its swim depth, its swim where it is.
fn crossing_s(kind: &Kind, course: &Course, dry_u: f32, angle: f32) -> f32 {
    let swim_below = SURFACE - kind.swim_depth * kind.height;
    let steps = 400;
    let du = dry_u / steps as f32;
    let along = 1.0 / libm::cosf(angle.to_radians());
    (0..steps)
        .map(|i| {
            let u = (i as f32 + 0.5) * du;
            let bed = -course.local_distance(Vec3::new(u, 0.0, 0.0));
            let speed = if bed < swim_below {
                kind.swim
            } else {
                kind.jog
            };
            du * along / speed
        })
        .sum()
}

fn run<M: Mover>(new: &impl Fn() -> M, case: Case) -> Outcome {
    let kind = Kind::man();
    let ticks = |s: f32| (s / TICK) as usize;
    match case {
        Case::Across(grade, angle) => {
            let (course, dry_u) = lake(grade);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, world(0.0, 0.0, 0.0));
            let (s, c) = (
                libm::sinf(angle.to_radians()),
                libm::cosf(angle.to_radians()),
            );
            let way = |feet: Vec3| {
                let up = feet.normalize();
                let forward = Vec3::X.reject_from(up).normalize();
                let side = Vec3::Z.reject_from(up).normalize();
                forward * c + side * s
            };
            // The depth it first leaves its feet at, going in: where it swims (a smooth shore
            // walked down keeps it on the ground otherwise), as deep as the mover judges it,
            // at the tick's start; and how far short of the far dry ground it ends.
            let mut swam_at = None;
            let allowed = crossing_s(&kind, &course, dry_u, angle) * SLACK;
            for _ in 0..ticks(allowed) {
                let before = run.body(0);
                run.tick(|_, b| Intent {
                    wish: way(b.feet),
                    ..Intent::default()
                });
                let after = run.body(0);
                if swam_at.is_none() && before.grounded && !after.grounded {
                    swam_at = Some(SURFACE - local(before.feet).y);
                }
            }
            let end = local(run.body(0).feet);
            let short = (dry_u + 0.5 - end.x).max(0.0);
            let wade = swam_at.map_or(1.0, |depth| (depth - kind.swim_depth * kind.height).abs());
            run.outcome(vec![short, wade])
        }
        Case::Float => {
            let (course, dry_u) = lake(GRADES[0]);
            let middle = (dry_u - MIDDLE_M * 0.5 - shore_m(GRADES[0], FAR_H - BED)).max(0.0);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, world(middle, SURFACE - PUT_M, 0.0));
            for _ in 0..ticks(FLOAT_S) {
                run.tick(|_, _| Intent::default());
            }
            let under = SURFACE - local(run.body(0).feet).y;
            run.outcome(vec![(under - kind.float_depth * kind.height).abs()])
        }
    }
}

/// The water's measures, and the deepest any body went into the ground.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> (Vec<Measure>, f32) {
    let cases = cases();
    let outcomes = terra_gen::par::map(&cases, |case| run(new, *case));
    let all = || cases.iter().zip(&outcomes);
    let worst = |want: fn(&Case) -> bool, i: usize| {
        all()
            .filter(|(case, _)| want(case))
            .map(|(_, o)| o.values[i])
            .fold(0.0, f32::max)
    };
    let deepest = outcomes
        .iter()
        .flat_map(|o| o.depths.iter().copied())
        .fold(0.0, f32::max);
    let m = |id, value: f32| Measure { id, value };
    let measures = vec![
        m("swim.shore", worst(|c| matches!(c, Case::Across(..)), 0)),
        m("swim.wade", worst(|c| matches!(c, Case::Across(..)), 1)),
        m("swim.float", worst(|c| matches!(c, Case::Float), 0)),
    ];
    (measures, deepest)
}
