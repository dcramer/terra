use super::*;
use crate::feet::two_bone;
use crate::mover::Sdf;

/// Level ground at `h` m.
fn floor(h: f32) -> Sdf<impl Fn(DVec3) -> f32> {
    Sdf {
        distance: move |p: DVec3| p.y as f32 - h,
        steepest: 1.0,
    }
}

/// Level ground at 0 with a tread `rise` m higher for x past 0 (a box's exact distance,
/// unioned with the floor's).
fn step(rise: f32) -> Sdf<impl Fn(DVec3) -> f32> {
    Sdf {
        distance: move |p: DVec3| {
            let p = p.as_vec3();
            let centre = Vec3::new(5.0, rise - 5.0, 0.0);
            let q = (p - centre).abs() - Vec3::splat(5.0);
            let boxed = q.max(Vec3::ZERO).length() + q.max_element().min(0.0);
            boxed.min(p.y)
        },
        steepest: 1.0,
    }
}

/// Ground rising `deg` toward +x through the origin.
fn slope(deg: f32) -> Sdf<impl Fn(DVec3) -> f32> {
    let n = Vec3::new(
        -libm::sinf(deg.to_radians()),
        libm::cosf(deg.to_radians()),
        0.0,
    );
    Sdf {
        distance: move |p: DVec3| p.as_vec3().dot(n),
        steepest: 1.0,
    }
}

const UP: Vec3 = Vec3::Y;

#[test]
fn the_probe_finds_the_ground_under_a_point_over_or_under_the_roots_level() {
    for h in [-0.5, -0.2, 0.0, 0.3, 0.55] {
        let under = probe(&floor(h), DVec3::new(1.0, 0.2, 2.0), DVec3::ZERO, UP).unwrap();
        assert!((under.height - h).abs() < 2e-3, "{h}: {under:?}");
        assert!(under.normal.dot(UP) > 0.999, "{h}: {under:?}");
    }
    // Further down than it looks, or a wall it starts in: no ground.
    assert_eq!(probe(&floor(-0.8), DVec3::ZERO, DVec3::ZERO, UP), None);
    assert_eq!(probe(&floor(0.9), DVec3::ZERO, DVec3::ZERO, UP), None);
    // On a slope, the ground under the point, and its normal.
    let deg: f32 = 30.0;
    let under = probe(&slope(deg), DVec3::new(0.4, 0.0, 0.0), DVec3::ZERO, UP).unwrap();
    let want = 0.4 * libm::tanf(deg.to_radians());
    assert!((under.height - want).abs() < 0.01, "{under:?} vs {want}");
    assert!((under.normal.angle_between(UP).to_degrees() - deg).abs() < 1.0);
}

/// A flat foot on the root's level: its ankle 8 cm up, and in its own frame its ball 5 cm under
/// the ankle and 12 cm ahead.
const ANKLE: Vec3 = Vec3::new(0.0, 0.08, 0.0);
const BALL: Vec3 = Vec3::new(0.0, -0.05, 0.12);

fn flat(x: f32) -> Pose {
    Pose {
        at: ANKLE + Vec3::X * x,
        turn: Quat::IDENTITY,
    }
}

#[test]
fn a_foot_is_put_on_the_ground_under_it_and_leaned_to_it_within_its_limit() {
    let at = |pose: Pose| pose.at.as_dvec3();
    let ball = |pose: Pose| (pose.at + pose.turn * BALL).as_dvec3();
    let place = |field: &dyn Query, pose: Pose| {
        let (heel, toe) = (
            probe(field, at(pose), DVec3::ZERO, UP),
            probe(field, ball(pose), DVec3::ZERO, UP),
        );
        placed(pose, target(pose, BALL, Vec3::ZERO, UP, heel, toe), UP)
    };
    // A tread 0.3 m under the root: the foot as much lower, flat.
    let down = place(&floor(-0.3), flat(0.0));
    assert!((down.at.y - (ANKLE.y - 0.3)).abs() < 2e-3, "{down:?}");
    assert!(down.turn.angle_between(Quat::IDENTITY) < 1e-3);
    // A slope of 20 deg across the foot: leaned to it, the ankle as high over the ground
    // under it as on the flat.
    let field = Sdf {
        distance: |p: DVec3| {
            let n = Vec3::new(
                -libm::sinf(20f32.to_radians()),
                libm::cosf(20f32.to_radians()),
                0.0,
            );
            p.as_vec3().dot(n)
        },
        steepest: 1.0,
    };
    let on = place(&field, flat(0.3));
    let ground = 0.3 * libm::tanf(20f32.to_radians());
    assert!((on.at.y - (ANKLE.y + ground)).abs() < 0.01, "{on:?}");
    let leaned = (on.turn * UP).angle_between(UP).to_degrees();
    assert!((leaned - 20.0).abs() < 1.0, "{leaned}");
    // Steeper than the limit: leaned no further.
    let steep = place(&slope(45.0), flat(0.3));
    let leaned = (steep.turn * UP).angle_between(UP).to_degrees();
    assert!((leaned - TILT_DEG).abs() < 0.5, "{leaned}");
    // Heel over the lower tread, ball over a riser's top 0.2 m up: lifted so the ball is not
    // in the tread, never under the clip's height over its own ground.
    // The foot headed +x, across the riser.
    let across = Pose {
        turn: Quat::from_rotation_y(core::f32::consts::FRAC_PI_2),
        ..flat(-0.06)
    };
    let edge = place(&step(0.2), across);
    let shown_ball = (edge.at + edge.turn * BALL).y;
    assert!(
        shown_ball >= 0.2 + (ANKLE + BALL).y - 2e-3,
        "{edge:?} {shown_ball}"
    );
    // No ground under it: as the clip has it.
    assert_eq!(place(&floor(-2.0), flat(0.0)), flat(0.0));
}

#[test]
fn the_pelvis_drops_by_the_lowest_planted_foot_and_no_swinging_one() {
    assert_eq!(drop([-0.3, 0.0], [1.0, 1.0]), 0.3);
    assert_eq!(
        drop([-0.3, -0.1], [0.0, 1.0]),
        0.1,
        "a swinging foot is no weight"
    );
    assert_eq!(drop([0.2, 0.1], [1.0, 1.0]), 0.0, "never up");
    assert_eq!(drop([-1.2, 0.0], [1.0, 1.0]), MOST_DROP_M);
    assert!((drop([-0.4, 0.0], [0.5, 1.0]) - 0.2).abs() < 1e-6);
}

#[test]
fn a_feet_lift_and_the_pelvis_ease_to_their_ground_and_settle() {
    let mut placing = Placing::default();
    let first = Target {
        lift: -0.3,
        tilt: Quat::IDENTITY,
    };
    assert_eq!(
        placing.foot(0, first, 1.0 / 60.0),
        first,
        "a first frame at once"
    );
    let to = Target {
        lift: 0.2,
        tilt: Quat::from_rotation_x(0.3),
    };
    let mut shown = Vec::new();
    for _ in 0..12 {
        shown.push(placing.foot(0, to, 1.0 / 60.0).lift);
    }
    assert!(
        shown.windows(2).all(|w| w[1] >= w[0]),
        "eased, never back: {shown:?}"
    );
    assert!(shown[0] < 0.0, "not at once: {shown:?}");
    assert!(
        (shown[11] - 0.2).abs() < 0.03,
        "near it within 0.2 s: {shown:?}"
    );
    let mut drop = 0.0;
    for _ in 0..30 {
        drop = placing.pelvis(0.3, 1.0 / 60.0);
    }
    assert!((drop - 0.3).abs() < 1e-3, "{drop}");
}

/// A leg: thigh and shin 0.45 m each, the hip 0.93 m up so the knee is a little bent with its
/// ankle where the clip has it, and the ground under it `under` lower than the root's. The ankle's shortfall
/// from its target when the leg is bent to it, with the pelvis dropped or not.
fn shortfall(under: f32, dropped: bool) -> f32 {
    let foot = flat(0.05);
    let target = target(
        foot,
        BALL,
        Vec3::ZERO,
        UP,
        Some(Under {
            height: -under,
            normal: UP,
        }),
        None,
    );
    let goal = placed(foot, target, UP);
    let hip = Vec3::new(0.0, 0.93, 0.0)
        - UP * if dropped {
            drop([target.lift, 0.0], [1.0, 1.0])
        } else {
            0.0
        };
    let (_, reached) = two_bone(hip, 0.45, 0.45, goal.at, hip + Vec3::Z);
    (reached - goal.at).length()
}

#[test]
fn a_planted_foot_on_a_lower_tread_is_reached_with_the_pelvis_dropped() {
    for under in [0.1, 0.2, 0.3, 0.5] {
        assert!(
            shortfall(under, false) > 0.01,
            "{under}: out of reach without the drop"
        );
        assert!(shortfall(under, true) < 1e-3, "{under}: reached with it");
    }
}
