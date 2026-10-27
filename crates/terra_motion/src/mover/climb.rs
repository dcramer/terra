//! Climbing (D477, D505-D511, docs/controller.md "Climbing"): a body holds a face too steep to
//! walk and moves over it, as shipped free climbers do (research/2026-10-25-climbing.md). A
//! state of its own (`Ground::Climbing`), entered against a face; each tick the face is found
//! again by probes ahead, the body moves along the face's plane by its input with gravity off,
//! is collided and slid as every tick is, then is held to the face at its kind's gap and turned
//! to it.
//!
//! The approach is Vitor Cantão's recreation of Breath of the Wild's climbing on Unreal's custom
//! movement modes (<https://www.vitorcantao.com/post/climbing-system/>), with nbertoa's UE5
//! climbing system (<https://github.com/nbertoa/ue5-climbing-system>) the same shape, written
//! from their accounts on our mover's own query; no code of theirs is taken. From Cantão: the
//! face from sweeps ahead, their normals averaged; a start only within 25 deg of the facing and
//! with face where the hands are; a tick's order (refresh the face, let go if it is gone, land
//! moving down onto floor, accelerate toward the input laid on the face, move, turn to the face,
//! snap back to the gap by a sweep). Ours probes at the feet and at the hands (the kind's
//! `reach`) where his traces at the capsule and the eyes.
//!
//!
//! The top (D508) is a mantle as Unreal's Advanced Locomotion System (Caleb Longmire's ALS v4,
//! its mantle system) and Cantão's ledge climb find one, written from their accounts: when the
//! hands' probe finds no face, a probe dropped from over the hands' reach just past the lip finds
//! walkable ground, the standing capsule is swept up the face to it and over onto it, and when
//! both are clear the body is carried along that path (up, then forward) over the kind's
//! `mantle_s`, then stands. ALS plays its clip along such a path; ours is the mover's own, a
//! clip to be fitted to it. With no room to stand the body holds at the lip.
//!
//! Not yet (docs/controller.md "Climbing", units C4 and C6-C7): the speed by steepness, the
//! lunge and the kick-off, stamina, a material's grip, the fight stance.

use super::query::{Capsule, Contact, Query};
use super::slide::{self, Shape, Support};
use super::{Body, Ground, Intent, Kind, approach, turn};
use glam::{DVec3, Vec3};

/// The probes' radius, m.
const PROBE_M: f32 = 0.05;
/// How far past the kind's gap the probes and the hold look for the face, m: past a dug cliff's
/// cell (0.5 m) set back over the one held.
const AHEAD_M: f32 = 0.6;
/// A face is held only within 25 deg of the way the body faces (Cantão's start).
const FACING_COS: f32 = 0.906_307_8;
/// How far past the lip a mantle stands the body's back, m.
const LIP_M: f32 = 0.1;
/// How high over the top a mantle carries the feet, m: clear of it, the walk's stick to floor
/// takes the rest.
const SKIN_M: f32 = 0.03;

/// What a body holds, or has pushed into on the ground (D507).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Hold {
    /// The face held: its normal, out of it; zero when none.
    normal: Vec3,
    /// How long it has pushed into a face on the ground, s, from the tick it touched it.
    pushed: Option<f32>,
    /// A pull-up under way (D508).
    mantle: Option<Mantle>,
}

/// A pull-up onto a top: the feet's path in the world, up the face then over the lip onto
/// the top, and how long it has run, s.
#[derive(Clone, Copy, Debug)]
struct Mantle {
    from: DVec3,
    over: DVec3,
    to: DVec3,
    t: f32,
}

impl Mantle {
    /// Where the feet are a share of the way along the path, by its length.
    fn at(&self, share: f32) -> DVec3 {
        let up = (self.over - self.from).length();
        let on = (self.to - self.over).length();
        let along = f64::from(share.clamp(0.0, 1.0)) * (up + on);
        if along <= up {
            self.from + (self.over - self.from) * (along / up.max(1.0e-9))
        } else {
            self.over + (self.to - self.over) * ((along - up) / on.max(1.0e-9))
        }
    }
}

/// Whether a contact is a face to hold: no other body (D505), too steep to stand on, leaning
/// no further past vertical than the kind holds (D510), so never a ceiling.
fn holdable(k: &Kind, shape: &Shape, c: &Contact) -> bool {
    c.body.is_none()
        && shape.too_steep(c.normal)
        && c.normal.dot(shape.up) >= -libm::sinf(k.overhang_deg.to_radians())
}

/// The face's normal where a probe finds one ahead along `into`: a thin sweep from the body's
/// axis so high over its feet.
fn probe(k: &Kind, query: &dyn Query, shape: &Shape, high: f32, into: Vec3) -> Option<Vec3> {
    probe_at(k, query, shape, Vec3::ZERO, high, into)
}

/// [`probe`] were the feet at `feet`.
fn probe_at(
    k: &Kind,
    query: &dyn Query,
    shape: &Shape,
    feet: Vec3,
    high: f32,
    into: Vec3,
) -> Option<Vec3> {
    let at = feet + shape.up * high;
    let probe = Capsule {
        a: at,
        b: at,
        radius: PROBE_M,
        origin: shape.origin,
    };
    let hit = query.sweep(&probe, into * (shape.radius + k.climb_gap + AHEAD_M))?;
    holdable(k, shape, &hit.contact).then_some(hit.contact.normal)
}

/// The face ahead: the normals the probes at the feet and at the hands find, averaged
/// (Unreal's climbers average a sweep's hits); and whether the hands found it.
fn face(k: &Kind, query: &dyn Query, shape: &Shape, into: Vec3) -> Option<(Vec3, bool)> {
    let feet = probe(k, query, shape, shape.radius, into);
    let hands = probe(k, query, shape, k.reach, into);
    let normal = (feet.unwrap_or(Vec3::ZERO) + hands.unwrap_or(Vec3::ZERO)).try_normalize()?;
    Some((normal, hands.is_some()))
}

/// The top past a face whose hands' probe found none (D508): walkable ground within the kind's
/// reach over the feet, found by a probe dropped from over the reach just past the lip, with
/// room to stand there reached by a clear path; the feet's corner over the lip and their place
/// on the top, offsets from the shape's origin. The hands' probe that missed already swept the
/// way out to where this one drops.
fn top(k: &Kind, query: &dyn Query, shape: &Shape, into: Vec3) -> Option<(Vec3, Vec3)> {
    let up = shape.up;
    let ahead = into * (2.0 * shape.radius + k.climb_gap + LIP_M);
    let at = up * (k.reach + PROBE_M) + ahead;
    let drop = Capsule {
        a: at,
        b: at,
        radius: PROBE_M,
        origin: shape.origin,
    };
    let hit = query.sweep(&drop, -up * k.reach)?;
    if hit.fraction <= 0.0 || shape.too_steep(hit.contact.normal) {
        return None;
    }
    let rise = k.reach * (1.0 - hit.fraction) + SKIN_M;
    let over = up * rise;
    let capsule = shape.capsule(Vec3::ZERO);
    let clear =
        query.sweep(&capsule, over).is_none() && query.sweep(&shape.capsule(over), ahead).is_none();
    clear.then_some((over, over + ahead))
}

/// The face a body touches, within its kind's gap, from its contacts: their normals averaged.
fn touched(body: &Body, shape: &Shape) -> Option<Vec3> {
    let k = &body.kind;
    body.active
        .iter()
        .map(|t| &t.contact)
        .filter(|c| c.distance <= k.climb_gap && holdable(k, shape, c))
        .map(|c| c.normal)
        .sum::<Vec3>()
        .try_normalize()
}

/// The level way into a face.
fn level_into(normal: Vec3, up: Vec3, or: Vec3) -> Vec3 {
    (-normal).reject_from(up).normalize_or(or)
}

/// After a tick off a face, whether the body takes hold of one it touches (D507): one that
/// rises past its step (lower is a step), within 25 deg of its facing, wished into; in the air at once, on the
/// ground after its kind's `climb_push_s` of pushing. A kind with no climb speed never does
/// (D136), nor a swimmer.
pub(super) fn start(body: &mut Body, intent: &Intent, query: &dyn Query, shape: &Shape, dt: f32) {
    let k = body.kind;
    let up = shape.up;
    let can = k.climb > 0.0 && body.ground != Ground::Swimming && !body.climbing_out;
    let Some(normal) = can.then(|| touched(body, shape)).flatten() else {
        body.hold.pushed = None;
        return;
    };
    let into = level_into(normal, up, body.facing);
    let wish = intent.wish.reject_from(up).normalize_or_zero();
    let toward = body.facing.dot(into) >= FACING_COS && wish.dot(into) > 0.0;
    let over_step = probe(&k, query, shape, k.step_height + PROBE_M, into).is_some();
    if !toward || !over_step {
        body.hold.pushed = None;
        return;
    }
    if body.ground == Ground::Grounded {
        let pushed = body.hold.pushed.map_or(0.0, |p| p + dt);
        body.hold.pushed = Some(pushed);
        if pushed + 1.0e-4 < k.climb_push_s {
            return;
        }
    }
    body.hold = Hold {
        normal,
        ..Hold::default()
    };
    (body.ground, body.velocity, body.normal) = (Ground::Climbing, Vec3::ZERO, Vec3::ZERO);
    // No coyote jump from a face.
    (body.air_s, body.left_by_jump) = (f32::INFINITY, true);
}

/// A tick of a body holding a face, if it holds one and the face is still there; false
/// otherwise, the body let go (airborne) for the walk to move it.
pub(super) fn tick(
    body: &mut Body,
    intent: &Intent,
    query: &dyn Query,
    shape: &Shape,
    dt: f32,
) -> bool {
    if body.ground != Ground::Climbing {
        return false;
    }
    if let Some(mantle) = body.hold.mantle {
        pull_up(body, mantle, query, shape, dt);
        return true;
    }
    let (k, up) = (body.kind, shape.up);
    let Some((normal, hands)) = face(&k, query, shape, -body.hold.normal) else {
        body.ground = Ground::Airborne;
        body.hold = Hold::default();
        return false;
    };
    // The input laid on the face: forward (into it) up it, the sides along it.
    let into = level_into(normal, up, body.facing);
    let side = up.cross(into);
    let wish = intent.wish.reject_from(up).normalize_or_zero();
    let upward = wish.dot(into);
    if !hands && upward > 0.0 {
        if let Some((over, to)) = top(&k, query, shape, into) {
            let from = body.frame;
            let mantle = Mantle {
                from,
                over: from + over.as_dvec3(),
                to: from + to.as_dvec3(),
                t: 0.0,
            };
            body.hold.mantle = Some(mantle);
            pull_up(body, mantle, query, shape, dt);
            return true;
        }
    }
    let rise = up.reject_from(normal).normalize_or(up);
    let wanted = rise * (upward * k.climb) + side * (wish.dot(side) * k.climb_side);
    let mut velocity = approach(body.velocity, wanted, k.climb_accel * dt);
    let mut feet = slide::move_shape(Vec3::ZERO, velocity, dt, query, shape, &mut body.active);
    if !hands
        && feet.dot(up) > 0.0
        && probe_at(&k, query, shape, feet, shape.radius, -normal).is_none()
    {
        // At a lip with no room to stand on the top: held there (D508), not climbed off it.
        feet = Vec3::ZERO;
        velocity = Vec3::ZERO;
    }
    let down = velocity.dot(up) < 0.0;
    let (support, floor) = slide::support(&mut body.active, velocity, shape, feet);
    if down && support == Support::OnGround {
        // Climbed down onto walkable floor: standing.
        body.support = support;
        (body.ground, body.normal, body.velocity) = (Ground::Grounded, floor, Vec3::ZERO);
        body.hold = Hold::default();
    } else {
        feet = held(&k, query, shape, feet, -normal);
        slide::contacts_at(feet, query, shape, &mut body.active);
        body.support = Support::NotSupported;
        (body.normal, body.velocity) = (Vec3::ZERO, velocity);
        body.hold.normal = normal;
        turn(body, into, dt);
    }
    body.position = body.frame + feet.as_dvec3();
    body.feet = body.position.as_vec3();
    (body.landed, body.fall, body.stepped, body.jumped) = (None, 0.0, 0.0, false);
    body.buffer = 0.0;
    true
}

/// A tick of a pull-up: the feet carried along its path over the kind's `mantle_s`, then
/// standing on the top, handed back to the walk.
fn pull_up(body: &mut Body, mut mantle: Mantle, query: &dyn Query, shape: &Shape, dt: f32) {
    let k = body.kind;
    mantle.t += dt;
    let share = mantle.t / k.mantle_s.max(dt);
    let feet = (mantle.at(share) - body.frame).as_vec3();
    slide::contacts_at(feet, query, shape, &mut body.active);
    if share >= 1.0 {
        let (support, floor) = slide::support(&mut body.active, Vec3::ZERO, shape, feet);
        body.support = support;
        (body.ground, body.normal, body.velocity) = if support == Support::OnGround {
            (Ground::Grounded, floor, Vec3::ZERO)
        } else {
            (Ground::Airborne, Vec3::ZERO, Vec3::ZERO)
        };
        body.hold = Hold::default();
    } else {
        body.support = Support::NotSupported;
        (body.normal, body.velocity) = (Vec3::ZERO, feet / dt);
        body.hold.mantle = Some(mantle);
    }
    body.position = body.frame + feet.as_dvec3();
    body.feet = body.position.as_vec3();
    (body.landed, body.fall, body.stepped, body.jumped) = (None, 0.0, 0.0, false);
    body.buffer = 0.0;
}

/// The feet moved to hold the body at its kind's gap from the face along `into` (Cantão's
/// snap back, the stick-to-floor step turned toward the face): swept, so nothing is passed
/// into; kept where they are when the face is out of the sweep's reach.
fn held(k: &Kind, query: &dyn Query, shape: &Shape, feet: Vec3, into: Vec3) -> Vec3 {
    let look = k.climb_gap + AHEAD_M;
    let capsule = shape.capsule(feet);
    let Some(hit) = query.sweep(&capsule, into * look) else {
        return feet;
    };
    let by = hit.fraction * look - k.climb_gap;
    if by >= 0.0 {
        return feet + into * by;
    }
    let back = query.sweep(&capsule, into * by).map_or(1.0, |h| h.fraction);
    feet + into * by * back
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mover::{Block, Bodies, Obstacle, Obstacles, Sdf, World};
    use glam::DVec3;

    const DT: f32 = 1.0 / 64.0;

    /// Level ground at y = 0 and, if told, a sheer wall at x = 1 as high as told.
    fn world_with<R>(wall: Option<f32>, then: impl FnOnce(&World) -> R) -> R {
        let ground = Sdf {
            distance: |at: DVec3| at.y as f32,
            steepest: 1.0,
        };
        let walls: Vec<Obstacle> = wall
            .map(|high| {
                vec![Obstacle::Block(Block {
                    centre: Vec3::new(11.0, high * 0.5, 0.0),
                    axes: [Vec3::X, Vec3::Y, Vec3::Z],
                    half: Vec3::new(10.0, high * 0.5, 5.0),
                })]
            })
            .unwrap_or_default();
        let obstacles = Obstacles { obstacles: &walls };
        let water = |_: Vec3| None;
        let world = World {
            ground: &(&ground, &obstacles),
            water: &water,
            gravity: 16.0,
            centre: Vec3::new(0.0, -1.0e6, 0.0),
        };
        then(&world)
    }

    /// A man walked into a wall for so long: his feet and ground state at the end.
    fn walk_in(kind: Kind, high: f32, seconds: f32) -> (Vec3, Ground) {
        world_with(Some(high), |world| {
            let mut bodies = Bodies::default();
            bodies.add(0, kind, Vec3::ZERO, Vec3::X, world);
            let intent = Intent {
                wish: Vec3::X,
                ..Intent::default()
            };
            for _ in 0..(seconds / DT) as usize {
                bodies.tick(world, &[(0, intent)], DT);
            }
            let b = bodies.get(0).expect("the body");
            (b.feet, b.ground)
        })
    }

    #[test]
    fn a_man_walked_into_a_wall_holds_it_and_climbs_at_his_speed() {
        let man = crate::course::Kind::man();
        // 0.5 s to reach the wall, half a second's push, then 3 s up at 1 m/s less the
        // quarter second it takes to reach that speed.
        let (feet, ground) = walk_in(man, 8.0, 0.4 + man.climb_push_s + 3.0);
        assert_eq!(ground, Ground::Climbing);
        let gap = 1.0 - man.radius - feet.x;
        assert!(
            (gap - man.climb_gap).abs() < 0.005,
            "held {gap} m off the wall"
        );
        assert!(feet.y > 2.0 && feet.y < 3.2, "climbed to {}", feet.y);
    }

    #[test]
    fn a_man_who_climbs_a_wall_to_its_top_pulls_up_and_stands_on_it() {
        let man = crate::course::Kind::man();
        // To the wall, the push, 3 m up at 1 m/s, the pull-up and a second's slack.
        let (feet, ground) = walk_in(man, 3.0, 0.4 + man.climb_push_s + 3.0 + man.mantle_s + 1.0);
        assert_eq!(ground, Ground::Grounded);
        assert!((feet.y - 3.0).abs() < 0.01, "stands at {}", feet.y);
        assert!(feet.x > 1.0 + man.radius, "on the top at x {}", feet.x);
    }

    #[test]
    fn a_kind_with_no_climb_speed_never_holds_a_wall() {
        let kind = Kind {
            climb: 0.0,
            ..crate::course::Kind::man()
        };
        let (feet, ground) = walk_in(kind, 8.0, 3.0);
        assert_eq!(ground, Ground::Grounded);
        assert!(feet.y.abs() < 0.01);
    }

    #[test]
    fn a_face_lower_than_a_step_is_stepped_onto_not_held() {
        let man = crate::course::Kind::man();
        let (feet, ground) = walk_in(man, 0.7 * man.step_height, 2.0);
        assert_eq!(ground, Ground::Grounded);
        assert!(
            (feet.y - 0.7 * man.step_height).abs() < 0.01,
            "on it at {}",
            feet.y
        );
    }
}
