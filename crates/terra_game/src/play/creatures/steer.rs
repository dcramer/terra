//! What a creature on the ground steers round: the trunks the woods and the flora know
//! (`flora::Trunks`, the land's woods among them, which no walker collides with) and the
//! rocks, stumps and logs too tall to step onto (`flora::Obstacles`), D267.

use bevy::prelude::*;
use terra_core::Avoidance;
use terra_world::flora::{Obstacles, Trunks};

/// How a creature weighs its way round them: Detour's crowd defaults, looking a second and a
/// half ahead (docs/scene-rules.md: "for a second ahead"; a walk of 1.4 m/s sees 2 m).
pub(super) const AVOIDS: Avoidance = Avoidance {
    horizon_s: 1.5,
    ..Avoidance::DETOUR
};

/// The rounds standing within `reach` of a point: each trunk's foot and radius, and each
/// tall piece of a solid.
pub(super) fn rounds_near(
    trunks: &Trunks,
    obstacles: &Obstacles,
    at: Vec3,
    reach: f32,
) -> Vec<(Vec3, f32)> {
    trunks
        .near(at, reach, obstacles)
        .map(|trunk| (trunk.foot, trunk.radius_m))
        .chain(obstacles.near(at, reach))
        .collect()
}

/// A body of a radius at a point moved out of every round it is in, across the ground.
pub(super) fn clear_of(rounds: Vec<(Vec3, f32)>, at: Vec3, radius: f32) -> Vec3 {
    let up = at.normalize_or(Vec3::Y);
    let mut at = at;
    for (middle, round) in rounds {
        let off = (at - middle).reject_from(up);
        let least = round + radius;
        let apart = off.length();
        if apart < least {
            at += off.normalize_or(up.any_orthonormal_vector()) * (least - apart);
        }
    }
    at
}

/// The way and speed a creature at a point takes round what stands near, facing one way and
/// wanting another at a speed: the velocity nearest it that keeps it clear (the lake film's
/// fox ran through a tree).
pub(super) fn round(
    (trunks, obstacles): (&Trunks, &Obstacles),
    here: Vec3,
    radius: f32,
    (facing, want, speed): (Vec3, Vec3, f32),
) -> (Vec3, f32) {
    if speed <= 0.0 || want == Vec3::ZERO {
        return (want, speed);
    }
    let reach = speed * AVOIDS.horizon_s + radius + 1.0;
    let rounds = rounds_near(trunks, obstacles, here, reach);
    let v = AVOIDS.steer_on_ground(here, radius, (facing * speed, want * speed), rounds);
    (v.normalize_or_zero(), v.length())
}

/// Where a creature moving to a point stands, pushed out of any round steering left it in.
pub(super) fn out_of(
    (trunks, obstacles): (&Trunks, &Obstacles),
    at: Vec3,
    radius: f32,
    speed: f32,
) -> Vec3 {
    if speed <= 0.0 {
        return at;
    }
    clear_of(rounds_near(trunks, obstacles, at, radius + 1.0), at, radius)
}
