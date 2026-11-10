//! A carried look kept clear of the ground and the body by a constraint solved from the look
//! itself (held-constraint j1; to replace `carry_arm`'s give, whose amounts were constants set
//! by hand for every look and body, D560). Not wired into `carry_arm::hold_in_body` yet.
//!
//! All in the body's frame, +Y up. `clear` finds how far the hand must move for the look's
//! points to stand a margin over the ground and a margin off the legs' and trunk's capsules;
//! `turns` is the arm's change that moves the hand so; `ease` brings it in over time. The
//! authored carry pose is the rest: nothing over a margin, nothing moves.
//!
//! The method: projection along the constraints' gradients onto a corrected hand target (each
//! pass moves the target by the deepest breach of the ground, up, and of a capsule, along its
//! normal, then lays the arm again and reads the points where they are), the form of a
//! position-based constraint projection (Müller et al., "Position Based Dynamics", 2007) on
//! one effector; the arm reaches the target by the analytic two-bone IK every engine carries
//! (the law of cosines at the elbow, then the least swing at the shoulder, so the elbow's
//! swivel stays the authored one: Unreal's Two Bone IK node and Unity's TwoBoneIKConstraint,
//! docs only). The terms are RelaxedIK's (Rakita, Mutlu, Gleicher 2018): the authored pose as
//! the goal, clearance and smoothness as terms; here closed form, with no allocation. The
//! hand, and the look in it, turn with the forearm: the wrist is not turned back, so a deep
//! lift tips the look (what `holds.ts`'s `carry.rise` reads; not solved here).

use bevy::math::{Quat, Vec3};

/// The least a carried look's lowest point stands over the ground, metres.
pub const CLEAR_GROUND_M: f32 = 0.02;
/// The least a carried look's points stand off the legs' and trunk's capsules, metres.
pub const CLEAR_BODY_M: f32 = 0.01;
/// The passes `clear` takes at most: each lays the arm again, since a hand moved by the elbow
/// turns the look, and a point far from the hand does not move as the hand does.
pub const PASSES: usize = 6;
/// A breach under this, metres, is none.
const NEAR_M: f32 = 1.0e-4;

/// An arm's shoulder, elbow and hand (its socket) in the body.
#[derive(Clone, Copy, Debug)]
pub struct Arm {
    pub shoulder: Vec3,
    pub elbow: Vec3,
    pub hand: Vec3,
}

/// A bone of the body as a capsule: its two ends and its radius.
#[derive(Clone, Copy, Debug)]
pub struct Capsule {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

/// The arm laid so its hand is `by` from where it was: the swing at the shoulder and the bend
/// at the elbow, each a turn in the body about its own joint. The hand goes as far toward the
/// target as the arm's length lets it.
pub fn reach(arm: &Arm, by: Vec3) -> (Quat, Quat) {
    if by == Vec3::ZERO {
        return (Quat::IDENTITY, Quat::IDENTITY);
    }
    let (upper, lower) = (arm.elbow - arm.shoulder, arm.hand - arm.elbow);
    let (a, b) = (upper.length(), lower.length());
    let target = arm.hand + by - arm.shoulder;
    if a < NEAR_M || b < NEAR_M || target.length() < NEAR_M {
        return (Quat::IDENTITY, Quat::IDENTITY);
    }
    // The elbow's inner angle for a shoulder to hand distance (the law of cosines).
    let inner = |c: f32| libm::acosf(((a * a + b * b - c * c) / (2.0 * a * b)).clamp(-1.0, 1.0));
    let far = target
        .length()
        .clamp((a - b).abs() + NEAR_M, a + b - NEAR_M);
    // A straight arm has no plane to bend in: it bends as an arm hanging at the side does, the
    // forearm coming forward (the model faces +Z, so about -X).
    let axis = upper.cross(lower).try_normalize().unwrap_or(Vec3::NEG_X);
    let elbow = Quat::from_axis_angle(axis, inner((arm.hand - arm.shoulder).length()) - inner(far));
    let bent = upper + elbow * lower;
    let swing = match (bent.try_normalize(), target.try_normalize()) {
        (Some(from), Some(to)) => Quat::from_rotation_arc(from, to),
        _ => Quat::IDENTITY,
    };
    (swing, elbow)
}

/// Where a point carried with the forearm (the hand, the look in it) stands after `reach`.
pub fn moved(arm: &Arm, (swing, elbow): (Quat, Quat), point: Vec3) -> Vec3 {
    arm.shoulder + swing * (arm.elbow - arm.shoulder + elbow * (point - arm.elbow))
}

/// `reach` as the turns `carry_arm`'s `turned` writes: the upper arm's in the body, then the
/// forearm's in the body under its parent as it then stands.
pub fn turns(arm: &Arm, by: Vec3) -> (Quat, Quat) {
    let (swing, elbow) = reach(arm, by);
    (swing, swing * elbow * swing.inverse())
}

/// How far the hand must move for the look's `points` (in the body, as the carry pose holds
/// them) to stand `CLEAR_GROUND_M` over `ground` and `CLEAR_BODY_M` off every capsule: zero,
/// exactly, when they do already, after one pass over the points. `side` is the way out of a
/// capsule for a point on its axis (the arm's own side of the body).
pub fn clear(arm: &Arm, points: &[Vec3], ground: f32, capsules: &[Capsule], side: Vec3) -> Vec3 {
    let most = (arm.elbow - arm.shoulder).length() + (arm.hand - arm.elbow).length();
    // The move that mends the deepest breaches with the hand moved `by`; none if there is none.
    let mend = |by: Vec3| {
        let laid = reach(arm, by);
        let (mut under, mut deep, mut out) = (0.0_f32, 0.0_f32, Vec3::ZERO);
        for &point in points {
            let p = moved(arm, laid, point);
            under = under.max(ground + CLEAR_GROUND_M - p.y);
            for c in capsules {
                let along = c.b - c.a;
                let t = if along.length_squared() > 0.0 {
                    ((p - c.a).dot(along) / along.length_squared()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let off = p - (c.a + along * t);
                let over = c.radius + CLEAR_BODY_M - off.length();
                if over > deep {
                    deep = over;
                    out = off.try_normalize().unwrap_or(side) * over;
                }
            }
        }
        (under > NEAR_M || deep > NEAR_M).then(|| Vec3::Y * under.max(0.0) + out)
    };
    let (mut short, mut by) = (Vec3::ZERO, Vec3::ZERO);
    for pass in 0..PASSES {
        let Some(more) = mend(by) else {
            if pass == 0 {
                return Vec3::ZERO;
            }
            // A hand lifted by its elbow tips the look up too, so a pass moves a far point
            // further than it meant: the least move that clears lies between the last that did
            // not and this one, found by halving.
            for _ in 0..PASSES {
                let mid = (short + by) * 0.5;
                if mend(mid).is_some() {
                    short = mid;
                } else {
                    by = mid;
                }
            }
            return by;
        };
        short = by;
        by = (by + more).clamp_length_max(most);
    }
    by
}

/// A hand's move eased in time: where it is and how fast it goes, metres.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Eased {
    pub at: Vec3,
    pub speed: Vec3,
}

/// `eased` after `dt` seconds toward `to`: the exact critically damped spring of
/// `carry_arm::ease` (Holden, "Spring-It-On", 2021; its source is named there), on a vector.
/// Its greatest speed toward a step is the step's length times rate / e.
pub fn ease(eased: Eased, to: Vec3, rate: f32, dt: f32) -> Eased {
    let j = eased.speed + (eased.at - to) * rate;
    let decay = libm::expf(-rate * dt);
    Eased {
        at: to + decay * (eased.at - to + j * dt),
        speed: decay * (eased.speed - j * rate * dt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crouching body's right arm, its hand at knee height, and a look 60 cm long in it
    /// pointing down and ahead: five points along it.
    fn crouched() -> (Arm, [Vec3; 5]) {
        let arm = Arm {
            shoulder: Vec3::new(-0.2, 0.9, 0.0),
            elbow: Vec3::new(-0.2, 0.62, -0.03),
            hand: Vec3::new(-0.2, 0.35, 0.04),
        };
        let tip = Vec3::new(0.0, -0.52, 0.3);
        (arm, [0.0, 0.25, 0.5, 0.75, 1.0].map(|k| arm.hand + tip * k))
    }

    fn lowest(arm: &Arm, by: Vec3, points: &[Vec3]) -> f32 {
        let laid = reach(arm, by);
        points
            .iter()
            .map(|&p| moved(arm, laid, p).y)
            .fold(f32::MAX, f32::min)
    }

    #[test]
    fn a_long_look_in_a_deep_crouch_stays_over_the_ground() {
        let (arm, points) = crouched();
        // The fault planted: left as posed, the look's tip is 17 cm under the ground.
        assert!(lowest(&arm, Vec3::ZERO, &points) < -0.15);
        let by = clear(&arm, &points, 0.0, &[], Vec3::NEG_X);
        let low = lowest(&arm, by, &points);
        assert!(low >= CLEAR_GROUND_M - 2.0 * NEAR_M, "lowest {low}");
        // No further than it must: within a centimetre of the margin.
        assert!(low < CLEAR_GROUND_M + 0.01, "lowest {low}"); // The arm's bones keep their lengths, and the hand is where the turns put it.
        let laid = reach(&arm, by);
        let (elbow, hand) = (
            moved(&arm, (laid.0, Quat::IDENTITY), arm.elbow),
            moved(&arm, laid, arm.hand),
        );
        assert!(((elbow - arm.shoulder).length() - 0.2816).abs() < 1e-3);
        assert!(((hand - elbow).length() - (arm.hand - arm.elbow).length()).abs() < 1e-4);
    }

    #[test]
    fn a_look_swung_into_a_shin_is_pushed_clear() {
        let (arm, _) = crouched();
        let points = [arm.hand, arm.hand + Vec3::new(0.06, -0.1, 0.1)];
        let shin = Capsule {
            a: Vec3::new(-0.12, 0.3, 0.18),
            b: Vec3::new(-0.12, 0.05, 0.1),
            radius: 0.05,
        };
        let off = |by: Vec3| {
            let laid = reach(&arm, by);
            points
                .iter()
                .map(|&p| {
                    let p = moved(&arm, laid, p);
                    let along = shin.b - shin.a;
                    let t = ((p - shin.a).dot(along) / along.length_squared()).clamp(0.0, 1.0);
                    (p - (shin.a + along * t)).length() - shin.radius
                })
                .fold(f32::MAX, f32::min)
        };
        // The fault planted: as posed the look's point is inside the shin.
        assert!(off(Vec3::ZERO) < -0.01, "{}", off(Vec3::ZERO));
        let by = clear(&arm, &points, 0.0, &[shin], Vec3::NEG_X);
        assert!(off(by) >= CLEAR_BODY_M - 2.0 * NEAR_M, "off {}", off(by));
    }

    #[test]
    fn a_look_that_is_clear_and_a_hand_holding_nothing_move_nothing() {
        let (arm, _) = crouched();
        let shin = Capsule {
            a: Vec3::new(0.12, 0.3, 0.18),
            b: Vec3::new(0.12, 0.05, 0.1),
            radius: 0.05,
        };
        assert_eq!(
            clear(&arm, &[arm.hand], 0.0, &[shin], Vec3::NEG_X),
            Vec3::ZERO
        );
        assert_eq!(clear(&arm, &[], 0.0, &[shin], Vec3::NEG_X), Vec3::ZERO);
        assert_eq!(turns(&arm, Vec3::ZERO), (Quat::IDENTITY, Quat::IDENTITY));
    }

    #[test]
    fn a_frame_moves_the_hand_no_faster_than_the_spring_allows() {
        let (arm, points) = crouched();
        let by = clear(&arm, &points, 0.0, &[], Vec3::NEG_X);
        let (rate, dt) = (crate::carry_arm::GIVE_RATE, 1.0 / 60.0);
        let most = by.length() * rate / std::f32::consts::E;
        let mut eased = Eased::default();
        for _ in 0..60 {
            let next = ease(eased, by, rate, dt);
            let speed = (next.at - eased.at).length() / dt;
            assert!(speed <= most * 1.001, "{speed} over {most}");
            eased = next;
        }
        // And it arrives, without passing the target.
        assert!((eased.at - by).length() < 1e-3);
        // The fault planted: followed frame by frame, the first frame's step is the whole move.
        assert!(by.length() / dt > most * 4.0);
    }
}
