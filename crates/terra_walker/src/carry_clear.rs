//! A carried look kept clear of the ground and the body by a constraint solved from the look
//! itself (held-constraint j1 and j2; to replace `carry_arm`'s give, whose amounts were
//! constants set by hand for every look and body, D560). `carry_arm::hold_in_body` lays it
//! beside the give behind `carry_arm::CLEAR_ON`.
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
//! hand, and the look in it, turn with the forearm, so the wrist is turned back (`back`) by
//! the forearm's turn, within a person's wrist (`WRIST_*`), and the look keeps the direction
//! the carry pose gave it: left to the forearm, a 6.2 cm lift tipped a 60 cm look 19.5 degrees
//! (`holds.ts`'s `carry.rise`, bar 10).

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

/// A person's wrist, degrees of flexion, extension, radial and ulnar deviation from the hand's
/// rest (AAOS ranges, adult averages): the one definition, which `tools/characters/wrist.ts`'s
/// `WRIST` reads from this file (`rust_const.ts`; `wrist.test.ts` holds it).
pub const WRIST_FLEX_DEG: f32 = 80.0;
pub const WRIST_EXT_DEG: f32 = 70.0;
pub const WRIST_RADIAL_DEG: f32 = 20.0;
pub const WRIST_ULNAR_DEG: f32 = 35.0;

/// The hand a look is held in: its bone's rotation in the body as the carry pose lays it, its
/// own axes in the bone's frame (`hand_axes`) and its bend at the body's rest, `[flex, dev]`.
#[derive(Clone, Copy, Debug)]
pub struct Hand {
    pub turn: Quat,
    pub axes: [Vec3; 3],
    pub rest: [f32; 2],
}

/// A hand's axes in its bone's frame from its rest pose, each unit and square to the others:
/// along it (wrist to middle knuckle), out of the palm (`palm_out`: the hand socket's -X) and
/// across it toward the thumb. `wrist.ts`'s `handAxes`.
pub fn hand_axes(hand: Quat, wrist: Vec3, knuckle: Vec3, thumb: Vec3, palm_out: Vec3) -> [Vec3; 3] {
    let back = hand.inverse();
    let along = (back * (knuckle - wrist)).normalize_or_zero();
    let palm = (back * palm_out).reject_from(along).normalize_or_zero();
    let across = (back * (thumb - wrist))
        .reject_from(along)
        .reject_from(palm)
        .normalize_or_zero();
    [along, palm, across]
}

/// The bend of a hand turned `hand` in the body off a forearm running along `fore` (elbow to
/// wrist), degrees: `[flex, dev]`, flex + toward the palm, dev + toward the thumb. `wrist.ts`'s
/// `bendOf`.
pub fn bend(fore: Vec3, hand: Quat, axes: [Vec3; 3]) -> [f32; 2] {
    let f = fore.normalize_or_zero();
    let [fh, fn_, fa] = axes.map(|axis| f.dot(hand * axis));
    [
        libm::atan2f(-fn_, fh).to_degrees(),
        libm::atan2f(-fa, libm::hypotf(fh, fn_)).to_degrees(),
    ]
}

/// The turn in the body that takes the hand back from where the arm laid by `laid` turned it,
/// so the look in it points as the carry pose had it: the whole of the forearm's turn undone
/// where the wrist's limits allow it, else the most of it (by halving, `PASSES` times) that
/// keeps the wrist inside them; none for no hand, and none where the pose is past them already.
pub fn back(arm: &Arm, (swing, elbow): (Quat, Quat), hand: Option<&Hand>) -> Quat {
    let Some(hand) = hand else {
        return Quat::IDENTITY;
    };
    let fore = swing * elbow;
    if fore == Quat::IDENTITY {
        return Quat::IDENTITY;
    }
    let along = fore * (arm.hand - arm.elbow);
    let within = |back: Quat| {
        let [flex, dev] = bend(along, back * fore * hand.turn, hand.axes);
        let (flex, dev) = (flex - hand.rest[0], dev - hand.rest[1]);
        (-WRIST_EXT_DEG..=WRIST_FLEX_DEG).contains(&flex)
            && (-WRIST_ULNAR_DEG..=WRIST_RADIAL_DEG).contains(&dev)
    };
    let whole = fore.inverse();
    if within(whole) {
        return whole;
    }
    if !within(Quat::IDENTITY) {
        return Quat::IDENTITY;
    }
    let (mut least, mut most) = (0.0_f32, 1.0_f32);
    for _ in 0..PASSES {
        let mid = (least + most) * 0.5;
        if within(Quat::IDENTITY.slerp(whole, mid)) {
            least = mid;
        } else {
            most = mid;
        }
    }
    Quat::IDENTITY.slerp(whole, least)
}

/// Where a point of the look stands after `reach` with the hand turned `back` at the wrist.
pub fn held(arm: &Arm, laid: (Quat, Quat), back: Quat, point: Vec3) -> Vec3 {
    let wrist = moved(arm, laid, arm.hand);
    wrist + back * (moved(arm, laid, point) - wrist)
}

/// The nearest point to `p` on a capsule's axis.
fn on_axis(c: &Capsule, p: Vec3) -> Vec3 {
    let along = c.b - c.a;
    if along.length_squared() > 0.0 {
        c.a + along * ((p - c.a).dot(along) / along.length_squared()).clamp(0.0, 1.0)
    } else {
        c.a
    }
}

/// How far the hand (`arm.hand`, the wrist) must move for the look's `points` (in the body, as
/// the carry pose holds them) to stand `CLEAR_GROUND_M` over `ground` and `CLEAR_BODY_M` off
/// every capsule, the hand turned back at the wrist as `back` allows: zero, exactly, when they
/// do already, after one pass over the points. A point in a capsule goes out along its normal
/// when that is toward the side of the capsule the hand is on, else out through that side (a
/// point past a leg's axis is not pushed on into the body); `side` is the way out when the
/// hand is on the axis (the arm's own side of the body).
pub fn clear(
    arm: &Arm,
    hand: Option<&Hand>,
    points: &[Vec3],
    ground: f32,
    capsules: &[Capsule],
    side: Vec3,
) -> Vec3 {
    let most = (arm.elbow - arm.shoulder).length() + (arm.hand - arm.elbow).length();
    // The move that mends the deepest breaches with the hand moved `by`; none if there is none.
    let mend = |by: Vec3| {
        let laid = reach(arm, by);
        let back = back(arm, laid, hand);
        let wrist = moved(arm, laid, arm.hand);
        let (mut under, mut deep, mut out) = (0.0_f32, 0.0_f32, Vec3::ZERO);
        for &point in points {
            let p = held(arm, laid, back, point);
            under = under.max(ground + CLEAR_GROUND_M - p.y);
            for c in capsules {
                let off = p - on_axis(c, p);
                let far = c.radius + CLEAR_BODY_M;
                if off.length() >= far {
                    continue;
                }
                let way = (wrist - on_axis(c, wrist)).try_normalize().unwrap_or(side);
                let toward = off.dot(way);
                // Along `way` from `off` to the capsule's margin: a ray out of a circle.
                let through = -toward
                    + libm::sqrtf((toward * toward - off.length_squared() + far * far).max(0.0));
                let (over, to) = match off.try_normalize() {
                    Some(normal) if toward >= 0.0 => (far - off.length(), normal),
                    _ => (through, way),
                };
                if over > deep {
                    deep = over;
                    out = to * over;
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

/// `eased` a frame on, held to the move wanted now: never short of `want` along it, and eased
/// (`ease`) only where it is past it, so a breach is mended the frame it is found and only the
/// way back to the clip's pose takes time. A spring both ways trails a breach that grows over a
/// few frames (a jump's start, a landing: 7 to 9 cm under the ground, held-constraint j5). As
/// penetration is treated elsewhere: Unreal's Lyra third-person camera takes a hard block's
/// distance at once and blends only back out (its PenetrationBlendOutTime), and its
/// spring arm's lag never delays the collision test's pull in (how they behave; no code of
/// theirs is used). The speed along `want` is dropped with the shortfall, so the release starts
/// from rest.
pub fn follow(eased: Eased, want: Vec3, rate: f32, dt: f32) -> Eased {
    let next = ease(eased, want, rate, dt);
    let Some(way) = want.try_normalize() else {
        return next;
    };
    let short = want.length() - next.at.dot(way);
    if short <= 0.0 {
        return next;
    }
    Eased {
        at: next.at + way * short,
        speed: next.speed - way * next.speed.dot(way),
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
        let by = clear(&arm, None, &points, 0.0, &[], Vec3::NEG_X);
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
        let by = clear(&arm, None, &points, 0.0, &[shin], Vec3::NEG_X);
        assert!(off(by) >= CLEAR_BODY_M - 2.0 * NEAR_M, "off {}", off(by));
    }

    /// The crouched arm's hand, straight on its forearm, its palm ahead so a lift's turn back
    /// is the wrist's flexion or extension.
    fn straight(arm: &Arm) -> Hand {
        let along = (arm.hand - arm.elbow).normalize();
        let palm = along.cross(Vec3::X).normalize();
        let axes = [along, palm, Vec3::X];
        Hand {
            turn: Quat::IDENTITY,
            axes,
            rest: bend(along, Quat::IDENTITY, axes),
        }
    }

    /// How far the look points from where the carry pose had it, degrees.
    fn tipped(arm: &Arm, hand: Option<&Hand>, by: Vec3, points: &[Vec3; 5]) -> f32 {
        let laid = reach(arm, by);
        let at = |p| held(arm, laid, back(arm, laid, hand), p);
        (at(points[4]) - at(points[0]))
            .angle_between(points[4] - points[0])
            .to_degrees()
    }

    #[test]
    fn a_lifted_look_keeps_its_direction_by_the_wrist_turned_back() {
        let (arm, points) = crouched();
        let hand = straight(&arm);
        // The fault planted: with no hand to turn back, the lift tips the look past the bar.
        let by = clear(&arm, None, &points, 0.0, &[], Vec3::NEG_X);
        assert!(tipped(&arm, None, by, &points) > 10.0);
        let by = clear(&arm, Some(&hand), &points, 0.0, &[], Vec3::NEG_X);
        let laid = reach(&arm, by);
        let low = points
            .iter()
            .map(|&p| held(&arm, laid, back(&arm, laid, Some(&hand)), p).y)
            .fold(f32::MAX, f32::min);
        assert!(low >= CLEAR_GROUND_M - 2.0 * NEAR_M, "lowest {low}");
        let tip = tipped(&arm, Some(&hand), by, &points);
        assert!(tip < 3.0, "tipped {tip}");
    }

    #[test]
    fn a_wrist_is_turned_back_no_further_than_its_limits() {
        let (arm, points) = crouched();
        let free = straight(&arm);
        let by = clear(&arm, Some(&free), &points, 0.0, &[], Vec3::NEG_X);
        let bent = |hand: &Hand, by: Vec3| {
            let laid = reach(&arm, by);
            let fore = laid.0 * laid.1;
            let turn = back(&arm, laid, Some(hand)) * fore * hand.turn;
            bend(fore * (arm.hand - arm.elbow), turn, hand.axes)[0] - hand.rest[0]
        };
        let whole = bent(&free, by);
        assert!(whole.abs() > 10.0, "the turn back bends the wrist {whole}");
        // A hand whose rest leaves it 5 degrees that way: 75 of its 80 of flexion spent, or 65
        // of its 70 of extension.
        let spent = if whole > 0.0 { -75.0 } else { 65.0 };
        let tight = Hand {
            rest: [free.rest[0] + spent, free.rest[1]],
            ..free
        };
        let by = clear(&arm, Some(&tight), &points, 0.0, &[], Vec3::NEG_X);
        let now = bent(&tight, by) + spent;
        assert!(now.abs() <= 5.0 + 1e-3, "bent {now} of 5");
        // Short of the whole turn, the look tips, and is still over the ground.
        assert!(tipped(&arm, Some(&tight), by, &points) > 3.0);
        let laid = reach(&arm, by);
        let low = points
            .iter()
            .map(|&p| held(&arm, laid, back(&arm, laid, Some(&tight)), p).y)
            .fold(f32::MAX, f32::min);
        assert!(low >= CLEAR_GROUND_M - 2.0 * NEAR_M, "lowest {low}");
    }

    #[test]
    fn a_point_past_a_legs_axis_goes_out_on_the_hands_side() {
        let (arm, _) = crouched();
        let shin = Capsule {
            a: Vec3::new(-0.12, 0.4, 0.14),
            b: Vec3::new(-0.12, 0.05, 0.14),
            radius: 0.05,
        };
        // The hand is outside the shin (-X of it); the look's point is 2 cm past its axis.
        let points = [arm.hand, Vec3::new(-0.10, 0.25, 0.14)];
        let off = |by: Vec3| {
            let p = moved(&arm, reach(&arm, by), points[1]);
            p - on_axis(&shin, p)
        };
        let way = (arm.hand - on_axis(&shin, arm.hand)).normalize();
        // The fault planted: the nearest way out is on into the body.
        assert!(off(Vec3::ZERO).length() < shin.radius && off(Vec3::ZERO).dot(way) < 0.0);
        let by = clear(&arm, None, &points, 0.0, &[shin], Vec3::NEG_X);
        let out = off(by);
        assert!(out.length() >= shin.radius + CLEAR_BODY_M - 2.0 * NEAR_M);
        assert!(out.dot(way) > 0.0 && out.x < 0.0, "out {out}");
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
            clear(&arm, None, &[arm.hand], 0.0, &[shin], Vec3::NEG_X),
            Vec3::ZERO
        );
        assert_eq!(
            clear(&arm, None, &[], 0.0, &[shin], Vec3::NEG_X),
            Vec3::ZERO
        );
        assert_eq!(turns(&arm, Vec3::ZERO), (Quat::IDENTITY, Quat::IDENTITY));
    }

    #[test]
    fn a_frame_moves_the_hand_no_faster_than_the_spring_allows() {
        let (arm, points) = crouched();
        let by = clear(&arm, None, &points, 0.0, &[], Vec3::NEG_X);
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

    /// A breach growing a centimetre a frame (a landing) is mended every frame, and the move
    /// let go is eased away no faster than the spring. The fault planted: eased both ways, the
    /// move trails the breach by centimetres.
    #[test]
    fn a_growing_breach_is_followed_at_once_and_only_its_release_is_eased() {
        let (rate, dt) = (crate::carry_arm::GIVE_RATE, 1.0 / 60.0);
        let want = |frame: usize| Vec3::new(0.0, 0.01 * frame.min(8) as f32, 0.0);
        let (mut held, mut sprung) = (Eased::default(), Eased::default());
        let mut trailed = 0.0_f32;
        for frame in 1..=12 {
            held = follow(held, want(frame), rate, dt);
            sprung = ease(sprung, want(frame), rate, dt);
            assert!(held.at.y >= want(frame).y - 1e-6, "frame {frame}: {held:?}");
            trailed = trailed.max(want(frame).y - sprung.at.y);
        }
        assert!(trailed > 0.02, "the spring alone trailed only {trailed} m");
        let most = 0.08 * rate / std::f32::consts::E;
        for _ in 0..60 {
            let next = follow(held, Vec3::ZERO, rate, dt);
            let speed = (next.at - held.at).length() / dt;
            assert!(speed <= most * 1.001, "{speed} over {most}");
            held = next;
        }
        assert!(held.at.length() < 1e-3);
        // A move past what is wanted, across it too, is left to the spring: only the shortfall
        // along the want is made up.
        let across = Eased {
            at: Vec3::new(0.03, 0.05, 0.0),
            speed: Vec3::ZERO,
        };
        let next = follow(across, Vec3::new(0.0, 0.05, 0.0), rate, dt);
        assert!(next.at.y >= 0.05 - 1e-6 && next.at.x > 0.02 && next.at.x < 0.03);
    }
}
