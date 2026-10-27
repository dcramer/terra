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
//! snap back to the gap by a sweep). Ours probes at the feet, the chest and the hands (the
//! kind's `reach`) where his traces at the capsule and the eyes, level into the face so a lip's
//! rounded corner never tilts the next tick's probes down; the snap keeps the gap by distance,
//! so a body rounds a lip onto a face set back over it (a dug cliff's next riser).
//!
//! The top (D508) is a mantle as Unreal's Advanced Locomotion System (Caleb Longmire's ALS v4,
//! its mantle system) and Cantão's ledge climb find one, written from their accounts: when the
//! hands' probe finds no face, a probe dropped from over the hands' reach just past the lip finds
//! walkable ground, the standing capsule is swept up the face to it and over onto it, and when
//! both are clear the body is carried along that path (up, then forward) over the kind's
//! `mantle_s`, then stands. ALS plays its clip along such a path; ours is the mover's own, a
//! clip to be fitted to it. With no room to stand the body holds at the lip.
//!
//! The climb jump (D511) is Breath of the Wild's and Genshin's, as Cantão's recreation and
//! nbertoa's system's dash describe it: jump while holding, the stick toward the face or
//! along it, lunges the body along the face by its kind's `climb_jump`, still holding, then
//! it climbs on; jump with the stick away from the face kicks it off backwards into the air (a
//! wall jump, Unreal's `LaunchCharacter` off a face's normal). The lunge rises as a hop
//! under gravity does, fast then slowing to a stop where the hands take hold again, so its
//! time is the world's gravity's, not a number of its own.
//!
//! Stamina (D506, D510, D511) drains as Breath of the Wild's and Genshin's climbing stamina
//! does, and as Conan Exiles' a held climber's: each tick of moving over a face costs the
//! kind's `climb_stamina_s` a second, holding still a quarter of it, a climb jump its lump
//! (the lunge's ticks then as holding); twice on an overhang and over the face's grip (a
//! material's, `Query::grip`, read where the probes touch it; 0 is never held, D505). At none
//! the climber lets go, as at a staggering blow (`Intent::knocked`), and holds no face again
//! until it stands or swims. The pool is the caller's (`Body::stamina`): the game's stamina
//! (D61, `terra_walker::Stamina`) is written in before a tick and the spend taken back after.
//! Never started in a fight stance (`Intent::fighting`, D507).

use super::query::{Capsule, Contact, Query, TOLERANCE};
use super::slide::{self, Shape, Support};
use super::{Body, Ground, Intent, Kind, approach, turn};
use glam::{DVec3, Vec3};

/// The probes' radius, m.
const PROBE_M: f32 = 0.05;
/// How far past the kind's gap the probes and the hold look for the face, m: past a dug cliff's
/// cell (0.5 m) set back over the one held.
const AHEAD_M: f32 = 0.6;
/// How far past its gap a face may be and still be held at it, m (a snap's slack).
const AT_GAP_M: f32 = 0.02;
/// A face is held only within 25 deg of the way the body faces (Cantão's start).
const FACING_COS: f32 = 0.906_307_8;
/// A jump with the stick more than this share of it away from the face kicks off (D511):
/// within 60 deg of straight away.
const AWAY: f32 = 0.5;
/// How far past the lip a mantle stands the body's back, m.
const LIP_M: f32 = 0.1;
/// How high over the top a mantle carries the feet, m: clear of it, the walk's stick to floor
/// takes the rest.
const SKIN_M: f32 = 0.03;
/// How much faster a body climbs a face by the up part of its normal (D509): up a sheer face at
/// its kind's speed, 1.34 times that up a slab 70 deg from level, 0.66 up an overhang 20 deg past
/// vertical, half at the kind's furthest (30 deg) past it. A placeholder to tune.
const LEAN_PACE: f32 = 1.0;
/// Holding still costs this share of moving's stamina (D506).
const HOLD_SHARE: f32 = 0.25;
/// A face leaning further past vertical than this (the sine of 5 deg) is an overhang, costing
/// [`OVERHANG_COST`] times as much (D510). A placeholder.
const OVERHANG_SIN: f32 = 0.087_155_74;
const OVERHANG_COST: f32 = 2.0;

/// What a body holds, or has pushed into on the ground (D507).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Hold {
    /// The face held: its normal, out of it; zero when none.
    normal: Vec3,
    /// How long it has pushed into a face on the ground, s, from the tick it touched it.
    pushed: Option<f32>,
    /// A pull-up under way (D508).
    mantle: Option<Mantle>,
    /// A climb jump under way (D511).
    lunge: Option<Lunge>,
    /// Knocked off a face by a blow (D506): it holds none until it stands or swims.
    knocked: bool,
}

/// A climb jump's lunge along the face: its way as a share up the face and a share along it
/// (laid on the face each tick, so it follows a face that turns), and how long it has run, s.
#[derive(Clone, Copy, Debug)]
struct Lunge {
    up: f32,
    side: f32,
    t: f32,
}

impl Lunge {
    /// The share of its length a lunge has covered by `t` of its `time`: a hop under constant
    /// gravity, from its launch speed to a stop at the end.
    fn share(t: f32, time: f32) -> f32 {
        let left = 1.0 - (t / time).clamp(0.0, 1.0);
        1.0 - left * left
    }
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

/// Whether a contact is a face to hold: no other body, of a material with grip (D505), too
/// steep to stand on, leaning no further past vertical than the kind holds (D510), so never a
/// ceiling. Its grip, where it is one.
fn holdable(k: &Kind, query: &dyn Query, shape: &Shape, c: &Contact) -> Option<f32> {
    let face = c.body.is_none()
        && shape.too_steep(c.normal)
        && c.normal.dot(shape.up) >= -libm::sinf(k.overhang_deg.to_radians());
    let grip = if face {
        query.grip(shape.origin + c.at.as_dvec3())
    } else {
        0.0
    };
    (grip > 0.0).then_some(grip)
}

/// The face's normal and grip where a probe finds one ahead along `into`, and how far ahead of
/// the body's capsule it is: a thin sweep from the body's axis so high over its feet.
fn probe(
    k: &Kind,
    query: &dyn Query,
    shape: &Shape,
    high: f32,
    into: Vec3,
) -> Option<(Vec3, f32, f32)> {
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
) -> Option<(Vec3, f32, f32)> {
    let at = feet + shape.up * high;
    let probe = Capsule {
        a: at,
        b: at,
        radius: PROBE_M,
        origin: shape.origin,
    };
    let look = shape.radius + k.climb_gap + AHEAD_M;
    let hit = query.sweep(&probe, into * look)?;
    let grip = holdable(k, query, shape, &hit.contact)?;
    let off = hit.fraction * look + PROBE_M - shape.radius;
    Some((hit.contact.normal, grip, off))
}

/// The face ahead as it is found along the level way `into` it, by probes at the feet, the
/// chest (midway to the hands) and the hands.
struct Found {
    /// The probes' normals averaged (Unreal's climbers average a sweep's hits).
    normal: Vec3,
    /// Whether the hands found it.
    hands: bool,
    /// The probes' grips averaged.
    grip: f32,
    /// How far ahead of the capsule the nearest probe found it, m.
    off: f32,
}

/// The face ahead (see [`Found`]). The chest's probe holds a face the feet have climbed past
/// the lip of and the hands are over, as a dug cliff's next riser set back over a tread.
fn face(k: &Kind, query: &dyn Query, shape: &Shape, into: Vec3) -> Option<Found> {
    let probes = [
        probe(k, query, shape, shape.radius, into),
        probe(k, query, shape, 0.5 * (shape.radius + k.reach), into),
        probe(k, query, shape, k.reach, into),
    ];
    let found = probes.iter().flatten();
    let normal = found.clone().map(|p| p.0).sum::<Vec3>().try_normalize()?;
    let count = found.clone().count() as f32;
    Some(Found {
        normal,
        hands: probes[2].is_some(),
        grip: found.clone().map(|p| p.1).sum::<f32>() / count,
        off: found.map(|p| p.2).fold(f32::INFINITY, f32::min),
    })
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
    // Over onto the top first: a face set back over a narrow top (a dug cliff's next riser)
    // blocks that way, and the way up the face is then never asked.
    let clear = passes(query, &shape.capsule(over), ahead) && query.sweep(&capsule, over).is_none();
    clear.then_some((over, over + ahead))
}

/// Whether a capsule sweeps all of a motion: as [`Query::sweep`], told at once by two distances
/// where it starts clear and ends a tolerance inside something, which a sweep would have met.
fn passes(query: &dyn Query, capsule: &Capsule, motion: Vec3) -> bool {
    let blocked = query.apart(capsule, TOLERANCE).is_none()
        && query.apart(&capsule.moved(motion), -TOLERANCE).is_some();
    !blocked && query.sweep(capsule, motion).is_none()
}

/// The face a body touches, within its kind's gap, from its contacts: their normals averaged.
fn touched(body: &Body, query: &dyn Query, shape: &Shape) -> Option<Vec3> {
    let k = &body.kind;
    body.active
        .iter()
        .map(|t| &t.contact)
        .filter(|c| c.distance <= k.climb_gap && holdable(k, query, shape, c).is_some())
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
/// (D136), nor a swimmer, a body with no stamina (D506), in a fight stance (D507) or knocked
/// off a face and not yet standing.
pub(super) fn start(body: &mut Body, intent: &Intent, query: &dyn Query, shape: &Shape, dt: f32) {
    let k = body.kind;
    let up = shape.up;
    if matches!(body.ground, Ground::Grounded | Ground::Swimming) {
        body.hold.knocked = false;
    }
    let can = k.climb > 0.0
        && body.ground != Ground::Swimming
        && !body.climbing_out
        && body.stamina > 0.0
        && !intent.fighting
        && !intent.knocked
        && !body.hold.knocked;
    let Some(normal) = can.then(|| touched(body, query, shape)).flatten() else {
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
    gravity: f32,
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
    if body.stamina <= 0.0 || intent.knocked {
        // Out of stamina or knocked off (D506): let go, airborne this tick.
        body.ground = Ground::Airborne;
        body.hold = Hold {
            knocked: intent.knocked,
            ..Hold::default()
        };
        return false;
    }
    let ahead = level_into(body.hold.normal, up, body.facing);
    let Some(Found {
        normal,
        hands,
        grip,
        off,
    }) = face(&k, query, shape, ahead)
    else {
        body.ground = Ground::Airborne;
        body.hold = Hold::default();
        return false;
    };
    // The input laid on the face: forward (into it) up it, the sides along it, faster on a
    // face less steep and slower on an overhang (D509).
    let into = level_into(normal, up, body.facing);
    let side = up.cross(into);
    let wish = intent.wish.reject_from(up).normalize_or_zero();
    let upward = wish.dot(into);
    if intent.jump && body.hold.lunge.is_none() {
        if upward < -AWAY {
            kick_off(body, normal, up);
            return false;
        }
        // Toward the face or along it, up it if the stick is let go.
        let way = Vec3::new(upward.max(0.0), wish.dot(side), 0.0).normalize_or(Vec3::X);
        body.hold.lunge = Some(Lunge {
            up: way.x,
            side: way.y,
            t: 0.0,
        });
    }
    let lunge = body.hold.lunge;
    let rising = upward > 0.0 || lunge.is_some_and(|l| l.up > 0.0);
    // A top is pulled up onto from its face held at the gap, not from one past it.
    let at_gap = off <= k.climb_gap + AT_GAP_M;
    if !hands
        && rising
        && at_gap
        && let Some((over, to)) = top(&k, query, shape, into)
    {
        let from = body.frame;
        let mantle = Mantle {
            from,
            over: from + over.as_dvec3(),
            to: from + to.as_dvec3(),
            t: 0.0,
        };
        body.hold.mantle = Some(mantle);
        body.hold.lunge = None;
        pull_up(body, mantle, query, shape, dt);
        return true;
    }
    spend(
        body,
        wish,
        normal.dot(up),
        grip,
        lunge.is_some_and(|l| l.t == 0.0),
        dt,
    );
    let rise = up.reject_from(normal).normalize_or(up);
    let mut velocity = if let Some(mut lunge) = lunge {
        // The lunge's tick: its length's share this tick covers, over a time a hop of that
        // height under the world's gravity takes.
        let time = libm::sqrtf(2.0 * k.climb_jump / gravity.max(1.0e-3));
        let by = Lunge::share(lunge.t + dt, time) - Lunge::share(lunge.t, time);
        lunge.t += dt;
        body.hold.lunge = (lunge.t < time).then_some(lunge);
        (rise * lunge.up + side * lunge.side) * (k.climb_jump * by / dt)
    } else {
        let pace = (1.0 + LEAN_PACE * normal.dot(up)).max(0.0);
        let wanted = (rise * (upward * k.climb) + side * (wish.dot(side) * k.climb_side)) * pace;
        approach(body.velocity, wanted, k.climb_accel * dt)
    };
    // The move gathers contacts only nearer than half the gap: the face held off at the gap is
    // no constraint on a move along it, and what the move goes into its sweep meets (Jolt's
    // predictive contacts are a saving, the sweep the guarantee).
    let moving = Shape {
        predictive: (0.5 * k.climb_gap).max(shape.padding + TOLERANCE),
        ..*shape
    };
    let mut feet = slide::move_shape(Vec3::ZERO, velocity, dt, query, &moving, &mut body.active);
    let below_hands = [shape.radius, 0.5 * (shape.radius + k.reach)];
    if !hands
        && feet.dot(up) > 0.0
        && below_hands
            .iter()
            .all(|&high| probe_at(&k, query, shape, feet, high, into).is_none())
    {
        // At a lip with no room to stand on the top: held there (D508), not climbed off it.
        feet = Vec3::ZERO;
        velocity = Vec3::ZERO;
        body.hold.lunge = None;
    }
    if lunge.is_some() && body.hold.lunge.is_none() {
        // The lunge's end: held again, still.
        velocity = Vec3::ZERO;
    }
    let down = velocity.dot(up) < 0.0;
    let (support, floor) = slide::support(&mut body.active, velocity, shape, feet);
    if down && support == Support::OnGround {
        // Climbed down onto walkable floor: standing.
        body.support = support;
        (body.ground, body.normal, body.velocity) = (Ground::Grounded, floor, Vec3::ZERO);
        body.hold = Hold::default();
        body.stale = false;
    } else {
        feet = held(&k, query, shape, feet, into);
        // The move's contacts are kept: the next climbing tick's move asks its own, and a walk
        // asks them again where the body is (`stale`).
        body.stale = true;
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

/// A tick's stamina on a face (D506, D510, D511): moving by the stick its kind's rate, holding
/// still or lunging a quarter of it, a lunge begun its lump; twice on an overhang (`lean` the up
/// part of the face's normal), over the face's grip.
fn spend(body: &mut Body, wish: Vec3, lean: f32, grip: f32, lunged: bool, dt: f32) {
    let k = body.kind;
    let moving = wish != Vec3::ZERO && body.hold.lunge.is_none();
    let share = if moving { 1.0 } else { HOLD_SHARE };
    let lump = if lunged { k.climb_jump_stamina } else { 0.0 };
    let overhang = if lean < -OVERHANG_SIN {
        OVERHANG_COST
    } else {
        1.0
    };
    let cost = (k.climb_stamina_s * share * dt + lump) * overhang / grip;
    body.stamina = (body.stamina - cost).max(0.0);
}

/// A jump off the face with the stick away (D511): let go, launched up at the kind's jump and
/// out from the face at its jog, as a jogging jump off the ground; the walk carries it from
/// this tick on.
fn kick_off(body: &mut Body, normal: Vec3, up: Vec3) {
    let k = body.kind;
    let away = normal.reject_from(up).normalize_or(normal);
    body.velocity = away * k.jog + up * k.jump_speed;
    body.ground = Ground::Airborne;
    body.support = Support::NotSupported;
    body.normal = Vec3::ZERO;
    body.hold = Hold::default();
}

/// A tick of a pull-up: the feet carried along its path over the kind's `mantle_s`, then
/// standing on the top, handed back to the walk.
fn pull_up(body: &mut Body, mut mantle: Mantle, query: &dyn Query, shape: &Shape, dt: f32) {
    let k = body.kind;
    mantle.t += dt;
    let share = mantle.t / k.mantle_s.max(dt);
    let feet = (mantle.at(share) - body.frame).as_vec3();
    slide::contacts_at(feet, query, shape, &mut body.active);
    body.stale = false;
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
/// into, the gap from the nearest point of what it meets (a lip's corner as well as a face);
/// kept where they are when the face is out of the sweep's reach.
fn held(k: &Kind, query: &dyn Query, shape: &Shape, feet: Vec3, into: Vec3) -> Vec3 {
    if in_place(k, query, shape, feet, into) {
        return feet;
    }
    // Backed off by up to the gap, clear of what is behind, a capsule grown by the gap is swept
    // in: it stops where the body is the gap from the first thing ahead by distance, a lip's
    // corner rounded over as well as a face met square.
    let capsule = shape.capsule(feet);
    let behind = query
        .sweep(&capsule, -into * k.climb_gap)
        .map_or(1.0, |h| h.fraction)
        * k.climb_gap;
    let from = feet - into * behind;
    let grown = Capsule {
        radius: shape.radius + k.climb_gap,
        ..shape.capsule(from)
    };
    let look = behind + k.climb_gap + AHEAD_M;
    let Some(hit) = query.sweep(&grown, into * look) else {
        return feet;
    };
    from + into * (hit.fraction * look)
}

/// How far into the face [`in_place`] looks for the distance to fall, m.
const IN_PLACE_M: f32 = 0.01;

/// Whether [`held`]'s snap would leave the feet where they are, to within its sweep's own
/// tolerance: the nearest thing is the gap less between a tolerance and half of one away, as
/// the snap leaves it, and it is the face ahead (nearer by nearly the whole of a step into it,
/// not a floor beside). Two distances, where the snap is two sweeps of a march each.
fn in_place(k: &Kind, query: &dyn Query, shape: &Shape, feet: Vec3, into: Vec3) -> bool {
    let within = k.climb_gap + TOLERANCE;
    let Some(here) = query.apart(&shape.capsule(feet), within) else {
        return false;
    };
    let least = k.climb_gap - TOLERANCE;
    if !(least..=least + 0.5 * TOLERANCE).contains(&here) {
        return false;
    }
    query
        .apart(&shape.capsule(feet + into * IN_PLACE_M), within)
        .is_some_and(|ahead| here - ahead > 0.9 * IN_PLACE_M)
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

    /// A man climbing an 8 m wall 2 s, then jumping with the stick along `stick` and letting
    /// it go: his feet before the jump, and his feet, ground state and velocity so long after.
    fn jump_off(stick: Vec3, after: f32) -> (Vec3, Vec3, Ground, Vec3) {
        let man = crate::course::Kind::man();
        world_with(Some(8.0), |world| {
            let mut bodies = Bodies::default();
            bodies.add(0, man, Vec3::ZERO, Vec3::X, world);
            let into = Intent {
                wish: Vec3::X,
                ..Intent::default()
            };
            for _ in 0..((0.4 + man.climb_push_s + 2.0) / DT) as usize {
                bodies.tick(world, &[(0, into)], DT);
            }
            let before = bodies.get(0).expect("the body").feet;
            let jump = Intent {
                wish: stick,
                jump: true,
                ..Intent::default()
            };
            bodies.tick(world, &[(0, jump)], DT);
            for _ in 0..(after / DT) as usize {
                bodies.tick(world, &[(0, Intent::default())], DT);
            }
            let b = bodies.get(0).expect("the body");
            (before, b.feet, b.ground, b.velocity)
        })
    }

    #[test]
    fn a_climb_jump_lunges_the_kinds_length_up_the_face_and_holds_on() {
        let man = crate::course::Kind::man();
        for stick in [Vec3::X, Vec3::ZERO] {
            let (before, feet, ground, velocity) = jump_off(stick, 1.0);
            assert_eq!(ground, Ground::Climbing, "holds after the lunge");
            let rise = feet.y - before.y;
            assert!(
                (rise - man.climb_jump).abs() < 0.01,
                "lunged {rise} m, stick {stick}"
            );
            assert!(velocity.length() < 1.0e-3, "still after it: {velocity}");
        }
    }

    #[test]
    fn a_jump_with_the_stick_away_kicks_off_the_face() {
        let (_, feet, ground, velocity) = jump_off(-Vec3::X, 0.1);
        assert_eq!(ground, Ground::Airborne);
        assert!(velocity.x < -1.0, "moving away at {velocity}");
        assert!(feet.x < 1.0 - 0.5, "off the face at {}", feet.x);
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
