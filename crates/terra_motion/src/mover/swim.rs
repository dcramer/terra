//! Swimming (D74, D478, docs/controller.md criterion 17): a body in water deeper than its
//! kind's swim depth swims. Moving while looking further down than the kind's `dive_from_deg`
//! strokes it down along its look, at most at its `dive` rate looking straight down (or up
//! along its look, under the surface); holding jump strokes it up at that rate; with neither,
//! buoyancy floats it to rest with its `float_depth` under (Unreal's `PhysSwimming`, whose
//! swimmer goes along its control rotation and is held up by its `Buoyancy` share of
//! immersion; ours floats it to a depth, since a swimmer at rest bobs at the surface rather
//! than hanging where it stopped, as Unreal's neutral default does). Its breath (D74) is used
//! while its mouth is under and refills at the surface; out of breath, it drowns a second at a
//! time, as Minecraft's and Valheim's swimmers take harm in pulses.
//!
//! A swimmer at the surface swimming into a ledge no higher than its kind's `climb_out` over
//! the water climbs out onto it (D523): Unreal's water jump (`UCharacterMovementComponent::
//! CheckWaterJump`, its `MaxOutOfWaterStepHeight` and `OutofWaterZ`, after its API docs), a
//! sweep ahead for a wall and a line over it at the limit, then a launch up and on, falling.

use super::query::{Capsule, Query};
use super::slide::Shape;
use super::{Body, Intent, Kind, World};
use glam::Vec3;
use std::f32::consts::FRAC_PI_2;

/// How far under floating depth an upward stroke begins to ease off, m: so a swimmer rising
/// arrives at the surface rather than leaping out of it and bobbing.
const EASE_M: f32 = 0.5;
/// How often drowning takes its harm, s.
const DROWN_PULSE_S: f32 = 1.0;

/// The pitch a swimmer strokes along, rad, up positive: its look's, beyond the kind's
/// `dive_from_deg` either side of level and stretched so straight down is still straight down,
/// so a camera resting a little under level does not dive it.
pub(super) fn look(k: &Kind, pitch: f32) -> f32 {
    let from = k.dive_from_deg.to_radians().clamp(0.0, FRAC_PI_2 - 1.0e-3);
    let pitch = pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
    let beyond = (pitch.abs() - from).max(0.0) * FRAC_PI_2 / (FRAC_PI_2 - from);
    beyond.copysign(pitch)
}

/// The vertical speed a swimmer strokes toward this tick, m/s, if it strokes: holding jump,
/// up at its dive rate; moving, along its stroke's pitch (`look`). An upward stroke eases off
/// over the last [`EASE_M`] under floating depth and is none at it, so buoyancy holds it there.
/// `depth` is how deep its feet are under the surface, m.
pub(super) fn stroke(
    k: &Kind,
    intent: &Intent,
    moving: bool,
    look: f32,
    depth: f32,
) -> Option<f32> {
    let up = if intent.rise {
        k.dive
    } else if moving && look != 0.0 {
        k.dive * libm::sinf(look)
    } else {
        return None;
    };
    if up <= 0.0 {
        return Some(up);
    }
    let ease = ((depth - k.float_depth * k.height) / EASE_M).clamp(0.0, 1.0);
    (ease > 0.0).then_some(up * ease)
}

/// How far ahead a swimmer looks for a wall to climb out over, in its radii (Unreal's
/// `CheckWaterJump` sweeps its capsule 1.2 radii on), and how far past the wall it looks for
/// the ledge's top (Unreal's line, 3.2 radii from the capsule's axis).
const WALL_RADII: f32 = 1.2;
const OVER_RADII: f32 = 3.2;
/// The least a swimmer wishing into a wall goes toward it: within 75 deg of straight at it,
/// as a step is taken (Jolt's `mStepCosForward`), so swimming along a bank does not climb it.
const INTO_COS: f32 = 0.258_819;
/// How far over the ledge's top a climb out lifts the feet, m: clear of its edge's round.
const CLEAR_M: f32 = 0.15;
/// The thin probe's radius that finds the ledge's top, m.
const PROBE_M: f32 = 0.02;

/// The water's depth at a body's feet as a share of its kind's swim depth, 1 or more swum,
/// and the speed up it climbs out at this tick if it does ([`climb_out`]): a swimmer climbing
/// out (D523) rises as a jump does, not swimming, until it tops its arc.
pub(super) fn wading(
    body: &mut Body,
    intent: &Intent,
    depth: f32,
    gravity: f32,
    query: &dyn Query,
    shape: &Shape,
) -> (f32, Option<f32>) {
    let k = &body.kind;
    let wading = depth / (k.swim_depth * k.height).max(1e-3);
    body.climbing &= body.velocity.dot(shape.up) > 0.0;
    let climb = (wading > 1.0 && !body.climbing)
        .then(|| climb_out(body, intent, depth, gravity, query, shape))
        .flatten();
    body.climbing |= climb.is_some();
    let wading = if body.climbing {
        wading.min(1.0)
    } else {
        wading
    };
    (wading, climb)
}

/// The speed up a swimmer climbing out this tick launches at, if it does (Unreal's
/// `CheckWaterJump`): at the surface (its mouth out), wishing into a wall too steep to stand
/// on within [`WALL_RADII`] ahead, whose walkable top stands no higher than the kind's
/// `climb_out` over the water (`depth` is how deep its feet are, m) with room over it, and the
/// water over it too shallow to swim. Unreal launches at a fixed `OutofWaterZ`; this launches
/// just high enough to clear the ledge found, so a low bank is not leapt.
fn climb_out(
    body: &Body,
    intent: &Intent,
    depth: f32,
    gravity: f32,
    query: &dyn Query,
    shape: &Shape,
) -> Option<f32> {
    let (k, up) = (&body.kind, shape.up);
    let wish = intent.wish.reject_from(up).normalize_or_zero();
    if body.under || wish == Vec3::ZERO {
        return None;
    }
    let reach = k.radius * WALL_RADII;
    let wall = query.sweep(&shape.capsule(Vec3::ZERO), wish * reach)?;
    let into = -wall.contact.normal.reject_from(up).normalize_or_zero();
    if !shape.too_steep(wall.contact.normal) || wish.dot(into) < INTO_COS {
        return None;
    }
    // The line over the wall at the limit (Unreal's trace at `MaxOutOfWaterStepHeight`): open,
    // or ending on ground to stand on.
    let limit = depth + k.climb_out;
    let probe = |at: Vec3| Capsule {
        a: at,
        b: at,
        radius: PROBE_M,
        origin: shape.origin,
    };
    let over = up * limit;
    if let Some(hit) = query.sweep(&probe(over), into * k.radius * OVER_RADII)
        && shape.too_steep(hit.contact.normal)
    {
        return None;
    }
    // The top, straight down from the limit where the body would stand past the wall's face.
    let stand = over + into * (reach * wall.fraction + 2.0 * k.radius + shape.padding);
    let top = query.sweep(&probe(stand), -up * limit)?;
    let rise = limit * (1.0 - top.fraction) - PROBE_M;
    let swum = depth - rise > k.swim_depth * k.height;
    if shape.too_steep(top.contact.normal) || swum {
        return None;
    }
    Some(libm::sqrtf(2.0 * gravity * (rise + CLEAR_M).max(0.0)))
}

/// A body's breath after a tick, by how deep its mouth ends under the water over its feet:
/// used while under, refilled at the surface, and once gone, its health taken a share
/// at a time ([`Body::drown`]).
pub(super) fn breathe(body: &mut Body, world: &World, dt: f32) {
    let from_centre = (body.position - world.centre.as_dvec3()).length() as f32;
    let mouth_depth = mouth_depth(body, (world.water)(body.feet), from_centre);
    let k = &body.kind;
    body.under = mouth_depth > 0.0;
    body.drown = 0.0;
    if !body.under {
        let back = k.breath_s / k.breath_back_s.max(dt);
        body.breath = (body.breath + back * dt).min(k.breath_s);
        body.gasping = 0.0;
        return;
    }
    let left = body.breath - dt;
    body.breath = left.max(0.0);
    if left >= 0.0 {
        return;
    }
    // The part of the tick past running out counts toward the next pulse.
    body.gasping += (-left).min(dt);
    if body.gasping + 1.0e-6 >= DROWN_PULSE_S {
        body.gasping -= DROWN_PULSE_S;
        body.drown = DROWN_PULSE_S / k.drown_s.max(dt);
    }
}

/// How deep a body's mouth is under the water over its feet, m (negative out of it).
fn mouth_depth(body: &Body, surface: Option<f32>, from_centre: f32) -> f32 {
    surface.map_or(f32::NEG_INFINITY, |surface| {
        surface - from_centre - body.kind.mouth * body.kind.height
    })
}
