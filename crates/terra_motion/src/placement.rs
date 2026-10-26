//! Foot placement (docs/movement.md stage 4, the fix-up layer; D455), raising feet onto the
//! ground under them: a clip strides on flat ground at its root, so on a step or a slope the
//! ground under a foot is higher than the root and the foot would sink into it. As Unreal's
//! Foot Placement node and ALS place feet (their documentation; no code taken): a foot is
//! probed once when it is locked, the ground found under its heel (the ankle) and its ball
//! frozen in the world for the lock's life, so a planted foot never shifts with what a probe
//! sees later (the next tread, the body moving over it); a swinging foot is probed under its
//! ankle once a frame (the ground's normal standing in for its ball's); the foot is pitched by
//! the heel's and ball's ground and rolled by the ground's normal (within `TILT_DEG`), turned
//! about its ball so a locked ball stays put, and raised until neither heel nor ball is under
//! its ground (a rigid sole rests on the highest ground under it: a foot locked across a
//! riser stands on the upper tread, its heel over the edge), each eased. A foot
//! is only raised here: raising never asks a leg for more than the clip did, so the pelvis is
//! not moved. A foot over lower ground (a drop, a tread's lower side) stays at the clip's
//! height, for the pelvis to lower (unit 2c). The leg is then bent to the foot by D455's
//! two-bone solve (`feet::two_bone`).
//!
//! Engine-free: places are world positions, so it tests with a field and no skeleton; the game
//! probes the walk's own world and writes the legs (`terra_walker::feet`).

use crate::feet::Pose;
use crate::mover::{Capsule, Query};
use glam::{DVec3, Quat, Vec3};

/// The probe starts this far over the root's level and goes this far under it, m: a step up as
/// high as a body steps (a man's 0.55 m, movers.ron) and the ankle over it.
pub const ABOVE_M: f32 = 0.7;
pub const BELOW_M: f32 = 0.6;
/// The probe's radius, m: a foot's narrowest, so a crack a voxel's tenth wide is no hole.
pub const PROBE_M: f32 = 0.02;
/// A foot pitches and rolls to its ground no further than this, deg.
pub const TILT_DEG: f32 = 30.0;
/// A foot's ground is eased to with this time constant, s (ALS eases its foot offsets at 15-30
/// a second).
pub const EASE_S: f32 = 0.04;
/// A planted ball of the foot stands about this high over its ground, m (a man's clips: 4-5
/// cm): a foot higher than this is clear of the ground, and its raise may lag.
pub const BALL_REST_M: f32 = 0.04;
/// Raised by less than this and tilted by less than `STILL_DEG`, the foot is left as the clip
/// has it (flat ground's numbers do not move a leg).
pub const STILL_M: f32 = 0.001;
pub const STILL_DEG: f32 = 0.5;

/// The ground under a point: how high it is over the root's level, m, and its normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Under {
    pub height: f32,
    pub normal: Vec3,
}

/// The ground under `at` (a world point), probed by `ground`, the mover's own query: a sphere
/// of `PROBE_M` swept down along `up` from `ABOVE_M` over `root`'s level to `BELOW_M` under it,
/// at `at`'s place across. None where nothing is met (a drop), or where the probe starts
/// inside the ground (a face higher than a step).
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
    let height = hit.contact.at.dot(up);
    // Met where it started: it started in the ground.
    if hit.fraction <= 0.0 || height > ABOVE_M {
        return None;
    }
    Some(Under {
        height,
        normal: hit.contact.normal.normalize_or(up),
    })
}

/// The ground a foot is placed on, in the world: the points under its heel and its ball, and
/// the normal it rolls to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    pub heel: Vec3,
    pub ball: Vec3,
    pub normal: Vec3,
}

/// One foot's placing: the ground frozen for its lock, and the ground eased to.
#[derive(Clone, Copy, Debug, Default)]
struct Placed {
    /// The lock (its ankle where it was locked) the frozen ground is of.
    lock: Option<Vec3>,
    frozen: Option<Ground>,
    /// The heel's and ball's ground over the root's level and its normal as eased, and the
    /// root they were over.
    eased: Option<((f32, f32, Vec3), Vec3)>,
}

/// A body's two feet placed on the ground under them, frame by frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Placing {
    feet: [Placed; 2],
}

/// What `Placing::foot` reads of a foot this frame.
#[derive(Clone, Copy, Debug)]
pub struct Foot {
    /// The foot as the lock shows it (`feet::Foot::update`), at the clip's height.
    pub shown: Pose,
    /// The ball of the foot in the foot's own frame.
    pub ball: Vec3,
    /// Where it was locked, while it is (`feet::Foot::locked`).
    pub lock: Option<Pose>,
    /// The clip's flat ground: the figure's root as shown.
    pub root: Vec3,
    pub up: Vec3,
    /// Feet not kept this frame (a body down, aloft): it goes back to the clip.
    pub off: bool,
    /// The leg's hip, in the world: the foot is never put farther from it than the clip has it.
    pub hip: Vec3,
}

impl Placing {
    /// Foot `side` placed on the ground under it, `dt` seconds on: `under` probes the ground
    /// under a world point (`probe`). A locked foot is probed under its heel and ball once, at
    /// its lock, and keeps that ground while it is locked; a free, stepping or released foot
    /// once a frame under its ankle.
    pub fn foot(
        &mut self,
        side: usize,
        foot: Foot,
        dt: f32,
        mut under: impl FnMut(Vec3) -> Option<Under>,
    ) -> Pose {
        let Foot {
            shown,
            ball,
            lock,
            root,
            up,
            off,
            hip,
        } = foot;
        let placed = &mut self.feet[side];
        let ball_at = shown.at + shown.turn * ball;
        let at_level = |at: Vec3| at - up * (at - root).dot(up);
        let found = |at: Vec3, under: Option<Under>| {
            under.map(|under| (at_level(at) + up * under.height, under.normal))
        };
        let target = match lock {
            _ if off => None,
            Some(lock) => {
                if placed.lock != Some(lock.at) {
                    placed.lock = Some(lock.at);
                    let heel = found(shown.at, under(shown.at));
                    let toe = found(ball_at, under(ball_at));
                    placed.frozen = match (heel, toe) {
                        (Some(heel), Some(toe)) => Some(Ground {
                            heel: heel.0,
                            ball: toe.0,
                            normal: sole(heel.0, toe.0, (heel.1 + toe.1).normalize_or(up), up),
                        }),
                        (Some((at, normal)), None) => {
                            Some(plane(at, normal, ball_at - shown.at, up))
                        }
                        (None, Some((at, normal))) => {
                            let heel = plane(at, normal, shown.at - ball_at, up);
                            Some(Ground {
                                heel: heel.ball,
                                ball: heel.heel,
                                normal,
                            })
                        }
                        (None, None) => None,
                    };
                }
                placed.frozen
            }
            None => {
                placed.lock = None;
                found(shown.at, under(shown.at))
                    .map(|(at, normal)| plane(at, normal, ball_at - shown.at, up))
            }
        };
        // Heights over the root's level now; no ground found, or the foot let go: the clip's own
        // flat ground.
        let over = |at: Vec3| (at - root).dot(up);
        let target = target.map_or((0.0, 0.0, up), |ground| {
            (over(ground.heel), over(ground.ball), ground.normal)
        });
        // Eased as ALS eases its foot offsets: a swinging foot's over the root (so a slope it
        // runs down does not lag it), a locked foot's in the world (so the body moving over
        // its frozen ground does not shift it).
        let k = 1.0 - libm::expf(-dt.max(0.0) / EASE_S);
        let eased = match placed.eased {
            None => target,
            Some(((heel, ball, normal), was_root)) => {
                let moved = if lock.is_some() {
                    (root - was_root).dot(up)
                } else {
                    0.0
                };
                let (heel, ball) = (heel - moved, ball - moved);
                // Raised behind its ground no further than the clip's foot is clear of its flat
                // ground (its ball over its rest): a foot is lifted out of the ground at once
                // (a lock across a riser), and eased only where it is clear of it.
                let clear = ((ball_at - root).dot(up) - BALL_REST_M).max(0.0);
                let ease = |was: f32, to: f32| (was + (to - was) * k).max(to - clear);
                (
                    ease(heel, target.0),
                    ease(ball, target.1),
                    normal.lerp(target.2, k).normalize_or(up),
                )
            }
        };
        placed.eased = Some((eased, root));
        raised(shown, ball_at, eased, up, hip)
    }
}

/// The ground under a foot from one point of it, `at` with its `normal`: its other point
/// `reach` away across, on the plane the normal gives.
fn plane(at: Vec3, normal: Vec3, reach: Vec3, up: Vec3) -> Ground {
    let across = reach.reject_from(up);
    let rise = if normal.dot(up) > 1e-3 {
        -normal.dot(across) / normal.dot(up)
    } else {
        0.0
    };
    Ground {
        heel: at,
        ball: at + across + up * rise,
        normal,
    }
}

/// The normal a sole stands to on ground found under its heel and ball (`heel`, `ball`, world
/// points): pitched by the slope from one to the other, rolled by the ground's normal there
/// (`normal`) about that line.
fn sole(heel: Vec3, ball: Vec3, normal: Vec3, up: Vec3) -> Vec3 {
    let span = (ball - heel).reject_from(up);
    let length = span.length();
    if length < 1e-3 || normal.dot(up) <= 1e-3 {
        return normal;
    }
    let ahead = span / length;
    let side = up.cross(ahead);
    let pitch = (ball - heel).dot(up) / length;
    let roll = -normal.dot(side) / normal.dot(up);
    (up - ahead * pitch - side * roll).normalize_or(up)
}

/// The foot `shown` (its ball at `ball_at`) raised onto its ground, `heel` and `ball` its
/// heights over the root's level under them and `normal` the normal it stands to: turned to
/// the normal within `TILT_DEG` of `up`, about its ball (so a locked ball stays where the lock
/// holds it), and raised until neither heel nor ball is lower over its ground than the clip
/// has it over its flat ground, as a rigid sole rests on the highest ground under it; never
/// lowered. A turn that would put the ankle farther from `hip` than the clip has it (a foot
/// behind the body at its stance's end, turned about its ball) is made only as far as the leg
/// reaches: a leg is never asked for more than the clip did.
fn raised(
    shown: Pose,
    ball_at: Vec3,
    (heel, ball, normal): (f32, f32, Vec3),
    up: Vec3,
    hip: Vec3,
) -> Pose {
    let full = lean(up, normal, TILT_DEG.to_radians());
    let reach = ball_at - shown.at;
    // The foot turned `s` of the way to the ground's normal.
    let turned = |s: f32| {
        let tilt = Quat::IDENTITY.slerp(full, s);
        let moved = tilt * reach - reach;
        let lift = heel.max(ball - moved.dot(up)).max(0.0);
        let at = shown.at + up * lift - moved.reject_from(up);
        (
            Pose {
                at,
                turn: (tilt * shown.turn).normalize(),
            },
            lift,
        )
    };
    let (placed, lift) = turned(1.0);
    if lift < STILL_M && full.angle_between(Quat::IDENTITY).to_degrees() < STILL_DEG {
        return shown;
    }
    let most = shown.at.distance(hip);
    if placed.at.distance(hip) <= most {
        return placed;
    }
    // Unturned it is only raised, which brings it no farther (a foot under its hip).
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..10 {
        let mid = (lo + hi) * 0.5;
        if turned(mid).0.at.distance(hip) > most {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    turned(lo).0
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

#[cfg(test)]
#[path = "placement_tests.rs"]
mod tests;
