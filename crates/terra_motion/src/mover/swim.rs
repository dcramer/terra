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

use super::{Body, Intent, Kind, World};
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
