//! The cases of climbing (docs/controller.md, "Climbing"; D477, D505-D511), for a man and the
//! second kind: faces of each source walked into, climbed and pulled up onto, and walkable
//! ground, a step and another body never held (D505); a jump at a face grabbing it at once and
//! a walk into it after half a second's push (D507); a climb's top and a ledge in reach from a
//! jump stood on, one with no room to stand never (D508); the speed up a sheer face and its
//! order by steepness (D509); overhangs held to about 30 deg past vertical and ceilings never
//! (D510); the lunge's rise and the kick-off (D511).
//!
//! Every answer is known from geometry and the kind's numbers alone (movers.ron's `climb`,
//! `climb_push_s`, `reach`, `climb_jump`: for a man, the decisions'), and a mover that does
//! not climb fails every case that should climb.

use super::field::{Course, forward_at, local, side_at, world};
use super::runs::{Outcome, Run};
use super::sources::{CELL_M, cubes};
use super::{Body, Intent, Kind, Measure, Mover, Solid, TICK};
use crate::mover::Obstacle;
use glam::{Vec2, Vec3};
use terra_core::{Piece, Solid as Pieces};

/// Where a face stands along the way, m; bodies start at 0.
const FACE_U: f32 = 8.0;
/// A body touches a face within this of it beyond its radius, m.
const TOUCH_M: f32 = 0.05;
/// How far before a face a body jumping at it presses jump, beyond its radius, m.
const JUMP_FROM: f32 = 0.6;
/// How far a body's feet rose over the ground while climbing for it to have climbed, m.
const CLIMBED_M: f32 = 1.0;
/// Overhangs held, deg past vertical, and those never (D510: about 30).
const HELD: [f32; 3] = [0.0, 15.0, 25.0];
const NEVER: [f32; 3] = [35.0, 45.0, 60.0];
/// Faces timed for their order by steepness, deg from level (D509).
const STEEP: [f32; 3] = [70.0, 90.0, 110.0];
/// Ledges jumped at, their tops as shares of the kind's reach over its feet (D508).
const LEDGES: [f32; 2] = [0.7, 0.9];
/// The slowest climb a case waits for, as a share of the kind's up a sheer face (D509).
const SLOWEST: f32 = 0.5;

/// What a body walks into (D505).
#[derive(Clone, Copy, Debug)]
enum Face {
    /// The ground standing sheer, 8 m.
    Wall,
    /// A cliff dug in 0.5 m cells: risers of three cells, treads of one, to 6 m.
    Cliff,
    /// A base's wall of cubes, 4 m.
    Cubes,
    /// A rock, 5 m round and 5 m high.
    Rock,
    /// A trunk 0.8 m thick.
    Trunk,
    /// A walkable slope, 40 deg (never held).
    Slope,
    /// A face lower than a step (never held).
    Step,
    /// Another body standing (never held).
    Body,
}

impl Face {
    fn climbed(self) -> bool {
        !matches!(self, Face::Slope | Face::Step | Face::Body)
    }
}

/// A top a body ends on, or not (D508).
#[derive(Clone, Copy, Debug)]
enum Top {
    /// A sheer wall's, 3 m, climbed.
    Climbed,
    /// A ledge jumped at, its top a share of the kind's reach.
    Ledge(f32),
    /// A ledge in reach under a roof too low to stand under (never taken).
    Roofed,
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Faces(usize, Face),
    /// Jumping at the face (or walking into it), so many deg off straight.
    Start(usize, bool, f32),
    Mantle(usize, Top),
    Speed,
    Steep(usize, f32),
    /// An overhang so many deg past vertical, or a ceiling.
    Overhang(usize, Option<f32>),
    Lunge,
    Kickoff(usize),
}

fn kinds() -> [Kind; 2] {
    let man = Kind::man();
    [man, super::data::second(&man)]
}

fn cases() -> Vec<Case> {
    let mut cases = vec![Case::Speed, Case::Lunge];
    for k in 0..2 {
        for face in [
            Face::Wall,
            Face::Cliff,
            Face::Cubes,
            Face::Rock,
            Face::Trunk,
            Face::Slope,
            Face::Step,
            Face::Body,
        ] {
            cases.push(Case::Faces(k, face));
        }
        for jump in [false, true] {
            for angle in [0.0, 20.0] {
                cases.push(Case::Start(k, jump, angle));
            }
        }
        cases.push(Case::Mantle(k, Top::Climbed));
        cases.extend(LEDGES.map(|s| Case::Mantle(k, Top::Ledge(s))));
        cases.push(Case::Mantle(k, Top::Roofed));
        cases.extend(STEEP.map(|s| Case::Steep(k, s)));
        cases.extend(
            HELD.iter()
                .chain(&NEVER)
                .map(|&l| Case::Overhang(k, Some(l))),
        );
        cases.push(Case::Overhang(k, None));
        cases.push(Case::Kickoff(k));
    }
    cases
}

/// Level ground and a face at [`FACE_U`] standing `steep` deg from level (past 90 an
/// overhang) to a top `high` m up, the top running on 30 m.
fn face(steep: f32, high: f32) -> Course {
    let lean = libm::tanf((steep - 90.0).to_radians());
    let far = FACE_U + 30.0;
    Course::flat().with(Solid::Polygon(vec![
        Vec2::new(FACE_U + lean, -1.0),
        Vec2::new(far, -1.0),
        Vec2::new(far, high),
        Vec2::new(FACE_U - high * lean, high),
    ]))
}

/// A round solid (a rock, a trunk) of a radius and height, its near side at [`FACE_U`].
fn round(radius: f32, high: f32) -> Course {
    let mut course = Course::flat();
    let piece = Piece::round(Vec2::ZERO, radius, high);
    let middle = world(FACE_U + radius, 0.0, 0.0);
    course.obstacles = vec![Obstacle::Solid(Pieces::of_pieces(middle, vec![piece]))];
    course
}

fn faces(face_: Face, kind: &Kind) -> Course {
    let ground = |corners: &[(f32, f32)]| {
        Course::of(Solid::Profile(
            corners.iter().map(|&(u, h)| Vec2::new(u, h)).collect(),
        ))
    };
    match face_ {
        Face::Wall => face(90.0, 8.0),
        Face::Cliff => {
            let mut corners = vec![(-20.0, 0.0)];
            for i in 0..4 {
                let (u, h) = (FACE_U + i as f32 * CELL_M, i as f32 * 3.0 * CELL_M);
                corners.extend([(u, h), (u, h + 3.0 * CELL_M)]);
            }
            corners.push((60.0, 6.0));
            ground(&corners)
        }
        Face::Cubes => {
            let mut course = Course::flat();
            course.obstacles = cubes(
                Vec3::new(FACE_U, 0.0, 0.0),
                Vec3::new(FACE_U, -CELL_M, -3.0),
                Vec3::new(FACE_U + CELL_M, 4.0, 3.0),
            );
            course
        }
        Face::Rock => round(2.5, 5.0),
        Face::Trunk => round(0.4, 8.0),
        Face::Slope => {
            let rise = 20.0 * libm::tanf(40f32.to_radians());
            ground(&[(-20.0, 0.0), (FACE_U, 0.0), (FACE_U + 20.0, rise)])
        }
        Face::Step => {
            let h = 0.7 * kind.step_height;
            ground(&[(-20.0, 0.0), (FACE_U, 0.0), (FACE_U, h), (60.0, h)])
        }
        Face::Body => Course::flat(),
    }
}

/// A walk at the face, so many deg off straight, jumping once [`JUMP_FROM`] short of it if
/// told; then on into it.
fn at_face(kind: &Kind, angle: f32, jump: bool) -> impl FnMut(&Body) -> Intent {
    let (s, c) = (
        libm::sinf(angle.to_radians()),
        libm::cosf(angle.to_radians()),
    );
    let radius = kind.radius;
    let mut jumped = !jump;
    move |b: &Body| {
        let near = gap(b, radius) <= JUMP_FROM;
        let jump = !jumped && near;
        jumped |= jump;
        Intent {
            wish: forward_at(b.feet) * c + side_at(b.feet) * s,
            jump,
            ..Intent::default()
        }
    }
}

/// How far a body is from a sheer face at [`FACE_U`] beyond its radius.
fn gap(b: &Body, radius: f32) -> f32 {
    FACE_U - local(b.feet).x - radius
}

/// The slowest climb a case waits for, m/s.
fn slowest(kind: &Kind) -> f32 {
    SLOWEST * kind.climb
}

/// How long a body takes to reach the face at its jog, with slack.
fn reach_s(kind: &Kind) -> f32 {
    FACE_U / kind.jog * 1.5 + 0.5
}

/// What a run saw: when the body first touched the sheer face, first held a face and was
/// first in the air, and the most its feet rose over the ground while holding.
#[derive(Default)]
struct Trace {
    touch: Option<f32>,
    grab: Option<f32>,
    touch_aloft: bool,
    rise: f32,
    t: f32,
}

impl Trace {
    fn tick<M: Mover>(&mut self, run: &mut Run<'_, M>, kind: &Kind, intent: Intent) {
        run.tick(|_, _| intent);
        self.t += TICK;
        let b = run.body(0);
        if self.touch.is_none() && gap(&b, kind.radius) <= TOUCH_M {
            self.touch = Some(self.t);
            self.touch_aloft = !b.grounded;
        }
        if run.mover().climbing(0) {
            self.grab.get_or_insert(self.t);
            self.rise = self.rise.max(local(b.feet).y);
        }
    }

    fn held(&self, rise: f32) -> bool {
        self.grab.is_some() && self.rise >= rise
    }
}

/// Ticks a body on for so long by a script, keeping its trace.
fn drive<M: Mover>(
    run: &mut Run<'_, M>,
    kind: &Kind,
    trace: &mut Trace,
    seconds: f32,
    mut intent: impl FnMut(&Body) -> Intent,
) {
    for _ in 0..(seconds / TICK) as usize {
        let i = intent(&run.body(0));
        trace.tick(run, kind, i);
    }
}

/// Whether a body stands on a top `high` m up past the face.
fn on_top(b: &Body, kind: &Kind, high: f32) -> bool {
    let at = local(b.feet);
    b.grounded && at.x > FACE_U + kind.radius * 0.5 && (at.y - high).abs() < 0.05
}

fn wrong(is: bool) -> Vec<f32> {
    vec![f32::from(u8::from(is))]
}

fn run<M: Mover>(new: &impl Fn() -> M, case: Case) -> Outcome {
    let kinds = kinds();
    match case {
        Case::Faces(k, face_) => {
            let kind = kinds[k];
            let course = faces(face_, &kind);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, world(0.0, 0.0, 0.0));
            if let Face::Body = face_ {
                run.add(1, kind, world(FACE_U + kind.radius, 0.0, 0.0));
            }
            let mut trace = Trace::default();
            let seconds = reach_s(&kind) + kind.climb_push_s + CLIMBED_M / slowest(&kind) + 1.0;
            let mut walk = at_face(&kind, 0.0, false);
            for _ in 0..(seconds / TICK) as usize {
                run.tick(|id, b| if id == 0 { walk(b) } else { Intent::default() });
                trace.t += TICK;
                if run.mover().climbing(0) {
                    trace.grab.get_or_insert(trace.t);
                    trace.rise = trace.rise.max(local(run.body(0).feet).y);
                }
            }
            let is = if face_.climbed() {
                !trace.held(CLIMBED_M)
            } else {
                trace.grab.is_some()
            };
            run.outcome(wrong(is))
        }
        Case::Start(k, jump, angle) => {
            let kind = kinds[k];
            let course = face(90.0, 8.0);
            let mut run = Run::new(new(), &course);
            run.add(0, kind, world(0.0, 0.0, 0.0));
            let mut trace = Trace::default();
            drive(
                &mut run,
                &kind,
                &mut trace,
                reach_s(&kind) + kind.climb_push_s + 1.0,
                at_face(&kind, angle, jump),
            );
            let late = match (trace.touch, trace.grab) {
                (Some(touch), Some(grab)) => {
                    grab - touch - if jump { 0.0 } else { kind.climb_push_s }
                }
                _ => f32::INFINITY,
            };
            run.outcome(wrong(late.abs() > 1.5 * TICK))
        }
        Case::Mantle(k, top) => mantle(new, &kinds[k], top),
        Case::Speed => climb_speed(new, &kinds[0], &face(90.0, 12.0), 2.0, 5.0),
        Case::Steep(k, steep) => climb_speed(new, &kinds[k], &face(steep, 6.0), 1.0, 3.0),
        Case::Overhang(k, lean) => overhang(new, &kinds[k], lean),
        Case::Lunge | Case::Kickoff(_) => {
            let (kind, lunge) = match case {
                Case::Kickoff(k) => (kinds[k], false),
                _ => (kinds[0], true),
            };
            leave(new, &kind, lunge)
        }
    }
}

/// How fast a body climbing a face moves along it between two heights its feet pass, m/s; 0
/// if it never passes them climbing.
fn climb_speed<M: Mover>(
    new: &impl Fn() -> M,
    kind: &Kind,
    course: &Course,
    low: f32,
    high: f32,
) -> Outcome {
    let mut run = Run::new(new(), course);
    run.add(0, *kind, world(0.0, 0.0, 0.0));
    let mut walk = at_face(kind, 0.0, false);
    let mut passed: [Option<(f32, Vec3)>; 2] = [None, None];
    let seconds = reach_s(kind) + kind.climb_push_s + high / slowest(kind);
    for i in 0..(seconds / TICK) as usize {
        run.tick(|_, b| walk(b));
        let b = run.body(0);
        let at = local(b.feet);
        if !run.mover().climbing(0) {
            continue;
        }
        for (slot, h) in passed.iter_mut().zip([low, high]) {
            if slot.is_none() && at.y >= h {
                *slot = Some((i as f32 * TICK, at));
            }
        }
    }
    let speed = match passed {
        [Some((t0, a)), Some((t1, b))] if t1 > t0 => {
            Vec2::new(b.x - a.x, b.y - a.y).length() / (t1 - t0)
        }
        _ => 0.0,
    };
    run.outcome(vec![speed])
}

/// 1 when the body did not do as D508 says: stand on a climbed top or a ledge in reach, never
/// on one with no room to stand.
fn mantle<M: Mover>(new: &impl Fn() -> M, kind: &Kind, top: Top) -> Outcome {
    let reach = kind.reach;
    let (course, high, jump, after) = match top {
        Top::Climbed => {
            let after = kind.climb_push_s + 3.0 / slowest(kind) + 2.0;
            (face(90.0, 3.0), 3.0, false, after)
        }
        Top::Ledge(share) => (face(90.0, share * reach), share * reach, true, 2.0),
        Top::Roofed => {
            let high = LEDGES[0] * reach;
            let roof = high + 0.6 * kind.height;
            let course = face(90.0, high).with(Solid::Block(
                Vec3::new(FACE_U, roof, -10.0),
                Vec3::new(FACE_U + 10.0, roof + 0.5, 10.0),
            ));
            (course, high, true, 4.0)
        }
    };
    let mut run = Run::new(new(), &course);
    run.add(0, *kind, world(0.0, 0.0, 0.0));
    let mut trace = Trace::default();
    let mut walk = at_face(kind, 0.0, jump);
    let mut stood = false;
    let seconds = reach_s(kind) + after;
    for _ in 0..(seconds / TICK) as usize {
        // Once it stands on the top the stick is let go, so it ends there: the case asks that it
        // stays, not that it touched the top and fell back.
        let i = if stood {
            Intent::default()
        } else {
            walk(&run.body(0))
        };
        trace.tick(&mut run, kind, i);
        stood |= on_top(&run.body(0), kind, high);
        if trace.touch.is_some_and(|t| trace.t > t + after) {
            break;
        }
    }
    let is = match top {
        Top::Roofed => stood,
        _ => !on_top(&run.body(0), kind, high),
    };
    run.outcome(wrong(is))
}

/// 1 when an overhang was held against D510 (or not held within it), or a ceiling held.
fn overhang<M: Mover>(new: &impl Fn() -> M, kind: &Kind, lean: Option<f32>) -> Outcome {
    let course = match lean {
        Some(lean) => face(90.0 + lean, 3.0),
        None => {
            let roof = 1.3 * kind.height;
            Course::flat().with(Solid::Block(
                Vec3::new(FACE_U - 10.0, roof, -10.0),
                Vec3::new(FACE_U + 10.0, roof + 0.5, 10.0),
            ))
        }
    };
    let mut run = Run::new(new(), &course);
    run.add(0, *kind, world(0.0, 0.0, 0.0));
    let mut trace = Trace::default();
    let seconds = reach_s(kind) + kind.climb_push_s + 2.0;
    // Under the ceiling the jump is pressed near the face's place, under the ceiling's middle.
    let walk = at_face(kind, 0.0, lean.is_none());
    drive(&mut run, kind, &mut trace, seconds, walk);
    let is = match lean {
        Some(l) if HELD.contains(&l) => !trace.held(0.5),
        _ => trace.grab.is_some(),
    };
    run.outcome(wrong(is))
}

/// Climbs a sheer face until [`CLIMBED_M`] up, then jumps: with the stick up, the lunge's
/// rise (m, 0 if it never climbed); with it away, 1 unless the body left the face airborne,
/// moving away, 0.2 s on (D511).
fn leave<M: Mover>(new: &impl Fn() -> M, kind: &Kind, lunge: bool) -> Outcome {
    let course = face(90.0, 12.0);
    let mut run = Run::new(new(), &course);
    run.add(0, *kind, world(0.0, 0.0, 0.0));
    let mut trace = Trace::default();
    let mut walk = at_face(kind, 0.0, false);
    let seconds = reach_s(kind) + kind.climb_push_s + CLIMBED_M / slowest(kind);
    for _ in 0..(seconds / TICK) as usize {
        if trace.held(CLIMBED_M) {
            break;
        }
        let i = walk(&run.body(0));
        trace.tick(&mut run, kind, i);
    }
    if !trace.held(CLIMBED_M) {
        let value = if lunge { 0.0 } else { 1.0 };
        return run.outcome(vec![value]);
    }
    let from = local(run.body(0).feet).y;
    let away = -forward_at(run.body(0).feet);
    let stick = if lunge { -away } else { away };
    run.tick(|_, b| Intent {
        wish: stick.reject_from(b.feet.normalize()).normalize_or_zero(),
        jump: true,
        ..Intent::default()
    });
    if lunge {
        let mut top = local(run.body(0).feet).y;
        for _ in 0..(1.5 / TICK) as usize {
            run.tick(|_, _| Intent::default());
            top = top.max(local(run.body(0).feet).y);
        }
        return run.outcome(vec![top - from]);
    }
    for _ in 0..(0.2 / TICK) as usize {
        run.tick(|_, b| Intent {
            wish: stick.reject_from(b.feet.normalize()).normalize_or_zero(),
            ..Intent::default()
        });
    }
    let b = run.body(0);
    let left = !run.mover().climbing(0) && !b.grounded && b.velocity.dot(away) > 1.0;
    run.outcome(wrong(!left))
}

/// The climbing measures, and the deepest any body went into the ground.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> (Vec<Measure>, f32) {
    let cases = cases();
    let outcomes = terra_gen::par::map(&cases, |case| run(new, *case));
    let of = |pick: &dyn Fn(&Case) -> bool| -> Vec<f32> {
        cases
            .iter()
            .zip(&outcomes)
            .filter(|(c, _)| pick(c))
            .map(|(_, o)| o.values[0])
            .collect()
    };
    let count = |pick: &dyn Fn(&Case) -> bool| of(pick).iter().sum::<f32>();
    let one = |pick: &dyn Fn(&Case) -> bool| of(pick)[0];
    // Each kind's speeds up the faces by steepness, a pair out of order where the steeper is
    // not slower.
    let steep = (0..2)
        .map(|k| {
            let speeds = of(&|c| matches!(c, Case::Steep(j, _) if *j == k));
            speeds.windows(2).filter(|p| p[1] >= p[0]).count() as f32
        })
        .sum();
    let deepest = outcomes
        .iter()
        .flat_map(|o| o.depths.iter().copied())
        .fold(0.0, f32::max);
    let man = &kinds()[0];
    let m = |id, value: f32| Measure { id, value };
    (
        vec![
            m("climb.faces", count(&|c| matches!(c, Case::Faces(..)))),
            m("climb.start", count(&|c| matches!(c, Case::Start(..)))),
            m("climb.mantle", count(&|c| matches!(c, Case::Mantle(..)))),
            m(
                "climb.speed",
                (one(&|c| matches!(c, Case::Speed)) - man.climb).abs() / man.climb,
            ),
            m("climb.steep", steep),
            m(
                "climb.overhang",
                count(&|c| matches!(c, Case::Overhang(..))),
            ),
            m(
                "climb.lunge",
                (one(&|c| matches!(c, Case::Lunge)) - man.climb_jump).abs(),
            ),
            m("climb.kickoff", count(&|c| matches!(c, Case::Kickoff(..)))),
        ],
        deepest,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::Field;
    use crate::mover::course::Ours;

    #[test]
    fn an_overhang_is_air_under_it_and_solid_behind_its_face() {
        // 30 deg past vertical, 3 m: the face at 2 m up stands 2 tan 30 = 1.155 m nearer.
        let course = face(120.0, 3.0);
        let lean = libm::tanf(30f32.to_radians());
        let under = Vec3::new(FACE_U - 1.2, 1.5, 0.0);
        assert!(
            course.local_distance(under) > 0.0,
            "under the overhang is air"
        );
        let behind = Vec3::new(FACE_U - 2.0 * lean + 0.2, 2.0, 0.0);
        assert!(
            course.local_distance(behind) < 0.0,
            "behind its face is solid"
        );
        // Straight out from the face at 2 m up, 0.3 m off it.
        let normal = Vec2::new(
            -libm::cosf(30f32.to_radians()),
            -lean * libm::cosf(30f32.to_radians()),
        );
        let on = Vec2::new(FACE_U - 2.0 * lean, 2.0) + normal * 0.3;
        let d = course.local_distance(Vec3::new(on.x, on.y, 0.0));
        assert!((d - 0.3).abs() < 1e-4, "{d}");
        assert!(course.distance(world(0.0, 0.5, 0.0)) > 0.4);
    }

    #[test]
    fn a_jump_at_the_face_touches_it_in_the_air_and_a_walk_on_the_ground() {
        // The fixtures do what the cases say on today's mover, which does not climb: the start
        // cases are told apart only by whether the body was aloft when it touched.
        for kind in kinds() {
            for jump in [false, true] {
                let course = face(90.0, 8.0);
                let mut run = Run::new(Ours::default(), &course);
                run.add(0, kind, world(0.0, 0.0, 0.0));
                let mut trace = Trace::default();
                drive(
                    &mut run,
                    &kind,
                    &mut trace,
                    reach_s(&kind),
                    at_face(&kind, 0.0, jump),
                );
                assert!(trace.touch.is_some(), "it reached the face");
                assert_eq!(trace.touch_aloft, jump, "jumping {jump}");
            }
        }
    }
}
