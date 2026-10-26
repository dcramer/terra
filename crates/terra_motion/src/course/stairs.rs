//! The cases of flights of steps and a doorway's sill (docs/controller.md criteria 5 to 7): a
//! body walks up and down flights of the ground's steps and a base's (whole, half and stair
//! cubes), and its speed along the way over the flight is the kind's jog, as on level ground,
//! with no tick off the ground climbing; and it walks through a doorway onto a floor standing
//! a little over the ground, never rising above the higher of the two under it, and onto a
//! threshold a step high under a lintel 2 m over it, as a base's doorway of four cubes over its
//! floor stands (the cabin's, `cargo xtask cabin`). A step's rise is the step's alone: what is shown smooths it (`Body::stepped`), as in Unreal and Jolt.
//! And it walks over a lump of ground lower than its step whose foot is too steep to stand on,
//! as the filmed yard's (`cargo xtask controller --film`): a step climbs it, a slope it is not.

use super::field::{Course, Solid, local, world};
use super::runs::{Outcome, Run, ground, walk};
use super::sources::{CELL_M, DOOR_U, block, cubes, doorway};
use super::{Kind, Measure, Mover, TICK};
use crate::mover::Obstacle;
use glam::Vec3;

/// Where a flight's first riser stands along the way, and how many steps it has.
const FACE_U: f32 = 2.0;
const STEPS: usize = 6;
/// The rises of the ground's flights, m, each on a tread of half a metre.
const RISES: [f32; 5] = [0.1, 0.2, 0.3, 0.4, 0.5];
const TREAD: f32 = 0.5;
/// A base's flights (terra_build's cubes), rise and tread: whole cubes, half cubes, and stair
/// cubes (`Shape::Stair`: two steps a cube, a quarter of a metre each way).
const BUILT: [(f32, f32); 3] = [
    (CELL_M, CELL_M),
    (CELL_M * 0.5, CELL_M),
    (CELL_M * 0.5, CELL_M * 0.5),
];
/// A doorway's sills: how far the floor inside stands over the ground at its door, m.
const SILLS: [f32; 5] = [0.0, 0.006, 0.01, 0.02, 0.1];
/// A doorway's thresholds under its lintel: how far the floor inside stands over the ground at
/// its door, m, and how high the cube before it stands, its last step, if one does (as a
/// base's doorway steps down to the ground: the cabin's half cube 0.12 m proud of the ground
/// and a quarter under its floor, where `cargo xtask cabin` stopped the walk on the mover).
const THRESHOLDS: [(f32, f32); 12] = [
    (0.1, 0.0),
    (0.2, 0.0),
    (0.25, 0.0),
    (0.3, 0.0),
    (0.4, 0.0),
    (0.5, 0.0),
    (0.3, 0.05),
    (0.35, 0.1),
    (0.37, 0.12),
    (0.4, 0.15),
    (0.45, 0.2),
    (0.5, 0.25),
];
/// Where a body walking at a threshold starts, a little apart, so the tick that meets its
/// riser falls at each place along a tick's walk: whether a step is taken there can turn on it.
const PHASES: usize = 5;
const PHASE_M: f32 = 0.01;
/// How high a doorway's opening stands over its floor, m: four cubes.
const OPENING_M: f32 = 2.0;
/// How far past a doorway's wall a body walking through must get, m.
const THROUGH_M: f32 = 1.0;
/// How far the floor inside a doorway runs, m: past where the walk ends.
const FLOOR_M: f32 = 20.0;
/// How long a body walks a flight or through a doorway, s.
const WALK_S: f32 = 3.0;
/// Lumps of ground, balls filled into level ground: how far their middle is under it and
/// their radius, m. The yard's (0.35 m high, its foot 62 deg), a lower and a higher, each
/// lower than the kind's step and steeper at its foot than it can stand on.
const LUMPS: [(f32, f32); 3] = [(0.3, 0.65), (0.2, 0.5), (0.4, 0.8)];
/// Where a lump's middle is along the way, how far across from the walk's line each walk
/// passes it, m, and how far on the walk must get.
const LUMP_U: f32 = 3.0;
const LUMP_ACROSS: [f32; 4] = [0.0, 0.1, 0.2, 0.3];
const PAST_LUMP_U: f32 = 5.0;
/// How long a body walks at a lump, s.
const LUMP_S: f32 = 2.0;

#[derive(Clone, Copy, Debug)]
enum Source {
    /// The ground's field.
    Ground,
    /// A base's cubes standing on level ground.
    Building,
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Flight {
        source: Source,
        rise: f32,
        tread: f32,
        up: bool,
    },
    Sill(f32),
    Threshold(f32, f32, f32),
    /// A lump (its middle's depth and radius) walked at so far across and from so far back.
    Lump(f32, f32, f32, f32),
}

fn cases() -> Vec<Case> {
    let mut flights: Vec<(Source, f32, f32)> =
        RISES.map(|rise| (Source::Ground, rise, TREAD)).to_vec();
    flights.extend(BUILT.map(|(rise, tread)| (Source::Building, rise, tread)));
    let mut cases = Vec::new();
    for (source, rise, tread) in flights {
        for up in [true, false] {
            cases.push(Case::Flight {
                source,
                rise,
                tread,
                up,
            });
        }
    }
    cases.extend(SILLS.map(Case::Sill));
    for (height, before) in THRESHOLDS {
        for i in 0..PHASES {
            let from = i as f32 * PHASE_M;
            cases.push(Case::Threshold(height, before, from));
        }
    }
    for (under, radius) in LUMPS {
        for across in LUMP_ACROSS {
            for i in 0..PHASES {
                cases.push(Case::Lump(under, radius, across, i as f32 * PHASE_M));
            }
        }
    }
    cases
}

/// A flight of [`STEPS`] from [`FACE_U`] on, up or down: the height before riser `i` and its
/// place along the way.
fn risers(rise: f32, tread: f32, up: bool) -> Vec<(f32, f32)> {
    (0..=STEPS)
        .map(|i| {
            let climbed = if up { i } else { STEPS - i };
            (FACE_U + i as f32 * tread, climbed as f32 * rise)
        })
        .collect()
}

fn flight(source: Source, rise: f32, tread: f32, up: bool) -> Course {
    let risers = risers(rise, tread, up);
    let end = risers[STEPS].0 + 5.0;
    match source {
        Source::Ground => {
            let mut corners = vec![(-5.0, risers[0].1)];
            for pair in risers.windows(2) {
                corners.extend([(pair[0].0, pair[0].1), (pair[0].0, pair[1].1)]);
            }
            corners.push((end, risers[STEPS].1));
            ground(&corners)
        }
        Source::Building => {
            let at = Vec3::new(FACE_U, 0.0, 0.0);
            let mut course = Course::flat();
            course.obstacles = (0..STEPS)
                .map(|i| {
                    // Up, a box from each riser on as high as its step; down, a box to each
                    // riser from the landing as high as the floor before it.
                    let (from, to, top) = if up {
                        (risers[i].0, end, risers[i + 1].1)
                    } else {
                        (-5.0, risers[i].0, risers[i].1)
                    };
                    block(at, Vec3::new(from, -CELL_M, -2.0), Vec3::new(to, top, 2.0))
                })
                .collect();
            course
        }
    }
}

/// Level ground, a base's wall with a doorway, and its floor inside standing `sill` over the
/// ground at the door (laid level there, as a base is).
fn sill(sill: f32) -> Course {
    let mut course = Course::flat();
    let at = Vec3::new(DOOR_U, 0.0, 0.0);
    let mut obstacles: Vec<Obstacle> = doorway();
    if sill > 0.0 {
        obstacles.extend(cubes(
            at,
            Vec3::new(DOOR_U, -CELL_M, -2.0),
            Vec3::new(DOOR_U + FLOOR_M, sill, 2.0),
        ));
    } else {
        obstacles.push(block(
            at,
            Vec3::new(DOOR_U, -CELL_M, -2.0),
            Vec3::new(DOOR_U + FLOOR_M, 0.0, 2.0),
        ));
    }
    course.obstacles = obstacles;
    course
}

/// Level ground, a base's wall of cubes with a doorway a metre wide under a lintel
/// [`OPENING_M`] over its floor, the floor standing `height` over the ground from the wall's
/// face on, and before it a cube standing `before` over the ground, or none.
fn threshold(height: f32, before: f32) -> Course {
    let mut course = Course::flat();
    let at = Vec3::new(DOOR_U, 0.0, 0.0);
    let (near, far) = (DOOR_U, DOOR_U + CELL_M);
    let (lintel, top) = (height + OPENING_M, height + OPENING_M + CELL_M);
    let mut obstacles = cubes(at, Vec3::new(near, -CELL_M, 0.5), Vec3::new(far, top, 5.0));
    obstacles.extend(cubes(
        at,
        Vec3::new(near, -CELL_M, -5.0),
        Vec3::new(far, top, -0.5),
    ));
    obstacles.extend(cubes(
        at,
        Vec3::new(near, lintel, -0.5),
        Vec3::new(far, top, 0.5),
    ));
    obstacles.extend(cubes(
        at,
        Vec3::new(near, -CELL_M, -2.0),
        Vec3::new(near + FLOOR_M, height, 2.0),
    ));
    if before > 0.0 {
        obstacles.push(block(
            at,
            Vec3::new(near - CELL_M, -CELL_M, -2.0),
            Vec3::new(near, before, 2.0),
        ));
    }
    course.obstacles = obstacles;
    course
}

fn run<M: Mover>(new: &impl Fn() -> M, case: Case) -> Outcome {
    let kind = Kind::man();
    let ticks = (WALK_S / TICK) as usize;
    match case {
        Case::Flight {
            source,
            rise,
            tread,
            up,
        } => {
            let course = flight(source, rise, tread, up);
            let risers = risers(rise, tread, up);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, world(0.0, risers[0].1, 0.0));
            let (first, last) = (risers[0].0, risers[STEPS - 1].0);
            let (mut entered, mut left, mut off) = (None::<(usize, f32)>, None, 0);
            for tick in 0..ticks {
                run.tick(|_, b| walk(b));
                let body = run.body(0);
                let u = local(body.feet).x;
                if (first..=last).contains(&u) {
                    entered.get_or_insert((tick, u));
                    left = Some((tick, u));
                    off += usize::from(!body.grounded);
                }
            }
            // Its speed along the way from the first riser to the last, as a share of the
            // kind's jog off it; one stopped at the flight's foot is all short.
            let speed = match (entered, left) {
                (Some((t0, u0)), Some((t1, u1))) if t1 > t0 => {
                    (u1 - u0) / ((t1 - t0) as f32 * TICK)
                }
                _ => 0.0,
            };
            let off_by = (1.0 - speed / kind.jog).abs();
            run.outcome(vec![off_by, off as f32])
        }
        Case::Sill(height) => {
            let course = sill(height);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, world(0.0, 0.0, 0.0));
            let door = world(DOOR_U, 0.0, 0.0);
            let floor_up = door.normalize();
            let floor = door + floor_up * height;
            let mut over = 0.0_f32;
            for _ in 0..ticks {
                run.tick(|_, b| walk(b));
                let feet = run.body(0).feet;
                let at = local(feet);
                // Over the higher of the ground and the floor under its foot's sphere.
                let above = if at.x + kind.radius > DOOR_U {
                    at.y.min((feet - floor).dot(floor_up))
                } else {
                    at.y
                };
                over = over.max(above);
            }
            run.outcome(vec![over])
        }
        Case::Threshold(height, before, from) => {
            let course = threshold(height, before);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, world(from, 0.0, 0.0));
            for _ in 0..ticks {
                run.tick(|_, b| walk(b));
            }
            // How far short of a metre past the wall it ended.
            let through = DOOR_U + CELL_M + THROUGH_M;
            let short = (through - local(run.body(0).feet).x).max(0.0);
            run.outcome(vec![short])
        }
        Case::Lump(under, radius, across, from) => {
            let lump = Solid::Ball(Vec3::new(LUMP_U, -under, 0.0), radius);
            let course = Course::flat().with(lump);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, world(from, 0.0, across));
            for _ in 0..(LUMP_S / TICK) as usize {
                run.tick(|_, b| walk(b));
            }
            let stopped = local(run.body(0).feet).x < PAST_LUMP_U;
            run.outcome(vec![f32::from(u8::from(stopped))])
        }
    }
}

/// The flights' and the sill's measures, and the deepest any body went into what it met.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> (Vec<Measure>, f32) {
    let cases = cases();
    let outcomes = terra_gen::par::map(&cases, |case| run(new, *case));
    let all = || cases.iter().zip(&outcomes);
    let worst = |want: &dyn Fn(&Case) -> bool, i: usize| {
        all()
            .filter(|(case, _)| want(case))
            .map(|(_, o)| o.values[i])
            .fold(0.0, f32::max)
    };
    let off = all()
        .filter(|(case, _)| matches!(case, Case::Flight { up: true, .. }))
        .map(|(_, o)| o.values[1])
        .sum::<f32>();
    let stopped = all()
        .filter(|(case, _)| matches!(case, Case::Lump(..)))
        .map(|(_, o)| o.values[0])
        .sum::<f32>();
    let deepest = outcomes
        .iter()
        .flat_map(|o| o.depths.iter().copied())
        .fold(0.0, f32::max);
    let m = |id, value: f32| Measure { id, value };
    let measures = vec![
        m(
            "stair.up",
            worst(&|c| matches!(c, Case::Flight { up: true, .. }), 0),
        ),
        m("stair.hop", off),
        m(
            "stair.down",
            worst(&|c| matches!(c, Case::Flight { up: false, .. }), 0),
        ),
        m("door.hop", worst(&|c| matches!(c, Case::Sill(_)), 0)),
        m(
            "door.lintel",
            worst(&|c| matches!(c, Case::Threshold(..)), 0),
        ),
        m("step.lump", stopped),
    ];
    (measures, deepest)
}

#[cfg(test)]
mod tests {
    use super::super::Field;
    use super::*;

    #[test]
    fn a_flight_rises_a_step_a_tread_to_its_landing() {
        let up = risers(0.3, 0.5, true);
        assert_eq!(up[0], (FACE_U, 0.0));
        assert!((up[STEPS].1 - 1.8).abs() < 1e-6);
        let down = risers(0.3, 0.5, false);
        assert!((down[0].1 - 1.8).abs() < 1e-6 && down[STEPS].1 == 0.0);
        // The ground's flight stands as high as its step just past each riser.
        let course = flight(Source::Ground, 0.3, 0.5, true);
        let at = world(FACE_U + 2.0 * 0.5 + 0.25, 0.9 + 0.1, 0.0);
        assert!((course.distance(at) - 0.1).abs() < 1e-2);
    }

    #[test]
    fn a_lump_is_lower_than_a_step_and_too_steep_to_stand_on_at_its_foot() {
        let kind = Kind::man();
        for (under, radius) in LUMPS {
            let course = Course::flat().with(Solid::Ball(Vec3::new(LUMP_U, -under, 0.0), radius));
            let top = world(LUMP_U, radius - under + 0.01, 0.0);
            assert!((course.distance(top) - 0.01).abs() < 1e-3);
            assert!(radius - under < kind.step_height);
            // Its foot, where it meets the ground, leans from upright by more than the limit.
            let foot = libm::acosf(under / radius).to_degrees();
            assert!(foot > kind.slope_limit_deg, "{foot}");
        }
    }
}
