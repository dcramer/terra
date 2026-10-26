//! Foot placement (docs/movement.md stage 4, the fix-up layer; D455): a foot put on the ground
//! under it, not on the level its clip was made on. A clip strides on flat ground at its root;
//! on a step or a slope the ground under a planted foot is higher or lower than the root, so
//! without this the foot floats over a lower tread or sinks into a higher one. Each foot's
//! ground is probed under its ankle and its ball, the mover's own query (`mover::Query`, the
//! field and the shapes it walks on, presentation only: nothing here goes back to the mover),
//! a small sphere swept down from above the root as Unreal's Foot Placement node and ALS trace
//! each foot (their documentation; no code taken); the foot is lifted or lowered by the ground
//! under its ankle, turned to the ground's normal within `TILT_DEG`, and lifted again if its
//! ball would be under the ground there; and the pelvis is lowered by the lowest planted foot's
//! drop so its leg still reaches (ALS's pelvis offset, Lyra's leg IK), each eased in and out.
//! The leg is then bent to the foot by D455's two-bone solve (`feet::two_bone`).
//!
//! Engine-free: places are world positions, so it tests with a field and no skeleton; the game
//! probes the walk's own world and writes the legs (`terra_walker::feet`).

use crate::feet::Pose;
use crate::mover::{Capsule, Query};
use glam::{DVec3, Quat, Vec3};

/// The probe starts this far over the root's level and goes this far under it, m: a step up or
/// down as high as a body steps (a man's 0.55 m, movers.ron) and a little.
pub const ABOVE_M: f32 = 0.6;
pub const BELOW_M: f32 = 0.6;
/// The probe's radius, m: a foot's narrowest, so a crack a voxel's tenth wide is no hole.
pub const PROBE_M: f32 = 0.02;
/// A foot turns to the ground's normal no further than this, deg (a step's edge's normal, met
/// by the sphere, leans the most).
pub const TILT_DEG: f32 = 30.0;
/// A foot's lift and tilt ease to the ground under it with this time constant, s (ALS eases
/// its foot offsets at 15-30 a second).
pub const EASE_S: f32 = 0.04;
/// The pelvis eases to its drop with this time constant, s, and drops no further than this, m:
/// a step down of 0.5 m and a slope at the slope limit under a planted foot.
pub const PELVIS_S: f32 = 0.06;
pub const MOST_DROP_M: f32 = 0.55;

/// The ground under a point: how high it is over the root's level, m, and its normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Under {
    pub height: f32,
    pub normal: Vec3,
}

/// The ground under `at` (a world point), probed by `ground`, the mover's own query: a sphere
/// of `PROBE_M` swept down along `up` from `ABOVE_M` over `root`'s level to `BELOW_M` under it,
/// at `at`'s place across. None where nothing is met (a drop), or where the probe starts
/// inside the ground (a wall the foot is beside).
pub fn probe<Q: Query + ?Sized>(ground: &Q, at: DVec3, root: DVec3, up: Vec3) -> Option<Under> {
    let level = at - up.as_dvec3() * (at - root).dot(up.as_dvec3());
    let top = up * (ABOVE_M + PROBE_M);
    let sphere = Capsule {
        a: top,
        b: top,
        radius: PROBE_M,
        origin: level,
    };
    let hit = ground.sweep(&sphere, -up * (ABOVE_M + BELOW_M))?;
    // Met over where it started: it started in the ground.
    let height = hit.contact.at.dot(up);
    if hit.fraction <= 0.0 || height > ABOVE_M {
        return None;
    }
    Some(Under {
        height,
        normal: hit.contact.normal.normalize_or(up),
    })
}

/// Where a foot goes on the ground under it: how far it is lifted along the up (negative,
/// lowered) and the turn that leans it to the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub lift: f32,
    pub tilt: Quat,
}

impl Default for Target {
    fn default() -> Self {
        Self {
            lift: 0.0,
            tilt: Quat::IDENTITY,
        }
    }
}

/// The foot `pose` (its ankle where the clip has it, over `root`'s level as the clip's flat
/// ground) set on the ground: lifted by the ground's height under its ankle (`heel`), leaned to
/// that ground's normal within `TILT_DEG`, and lifted again if its ball (`ball`, in the foot's
/// own frame) would be lower over the ground under it (`toe`) than the clip has it over its
/// flat ground. No ground under either: as the clip has it there.
pub fn target(
    pose: Pose,
    ball: Vec3,
    root: Vec3,
    up: Vec3,
    heel: Option<Under>,
    toe: Option<Under>,
) -> Target {
    let Some(under) = heel.or(toe) else {
        return Target::default();
    };
    let tilt = lean(up, under.normal, TILT_DEG.to_radians());
    let mut lift = under.height;
    if let Some(toe) = toe {
        let clip_ball = (pose.at + pose.turn * ball - root).dot(up);
        let shown_ball = (pose.at - root).dot(up) + lift + (tilt * (pose.turn * ball)).dot(up);
        lift += (toe.height + clip_ball - shown_ball).max(0.0);
    }
    Target { lift, tilt }
}

/// The turn from `up` toward `normal`, no further than `most` radians.
fn lean(up: Vec3, normal: Vec3, most: f32) -> Quat {
    let full = Quat::from_rotation_arc(up, normal.normalize_or(up));
    let (axis, angle) = full.to_axis_angle();
    if angle <= most || !axis.is_finite() {
        full
    } else {
        Quat::from_axis_angle(axis, most)
    }
}

/// The foot as shown on its ground: `pose` lifted and leaned by `target`, about its ankle.
pub fn placed(pose: Pose, target: Target, up: Vec3) -> Pose {
    Pose {
        at: pose.at + up * target.lift,
        turn: (target.tilt * pose.turn).normalize(),
    }
}

/// How far the pelvis drops, m, for feet lifted by `lifts` (negative, lowered) and weighing on
/// the ground by `weights` (a foot's lock weight, 1 planted, 0 swinging): by the lowest planted
/// foot's drop, so its leg reaches as the clip has it, and the other bends; never up, and no
/// more than `MOST_DROP_M`. A swinging foot over a drop does not pull the body down after it.
pub fn drop(lifts: [f32; 2], weights: [f32; 2]) -> f32 {
    let low = (0..2)
        .map(|i| lifts[i].min(0.0) * weights[i].clamp(0.0, 1.0))
        .fold(0.0, f32::min);
    (-low).min(MOST_DROP_M)
}

/// A body's feet and pelvis eased to their ground, frame by frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Placing {
    feet: [Option<Target>; 2],
    drop: f32,
}

impl Placing {
    /// Foot `side`'s target eased toward `to` over `dt` seconds (a first frame takes it at
    /// once).
    pub fn foot(&mut self, side: usize, to: Target, dt: f32) -> Target {
        let k = 1.0 - libm::expf(-dt.max(0.0) / EASE_S);
        let now = match self.feet[side] {
            None => to,
            Some(was) => Target {
                lift: was.lift + (to.lift - was.lift) * k,
                tilt: was.tilt.slerp(to.tilt, k).normalize(),
            },
        };
        self.feet[side] = Some(now);
        now
    }

    /// The pelvis's drop eased toward `to` over `dt` seconds.
    pub fn pelvis(&mut self, to: f32, dt: f32) -> f32 {
        let k = 1.0 - libm::expf(-dt.max(0.0) / PELVIS_S);
        self.drop += (to - self.drop) * k;
        self.drop
    }

    /// Let go of the ground (a body in the air, or one whose feet are not kept): the next
    /// frame on the ground starts from the clip again, eased.
    pub fn lift_off(&mut self, dt: f32) {
        for side in 0..2 {
            self.foot(side, Target::default(), dt);
        }
        self.pelvis(0.0, dt);
    }
}

#[cfg(test)]
#[path = "placement_tests.rs"]
mod tests;
