use super::*;
use crate::mover::Sdf;

/// Level ground at `h` m.
fn floor(h: f32) -> Sdf<impl Fn(DVec3) -> f32> {
    Sdf {
        distance: move |p: DVec3| p.y as f32 - h,
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
const DT: f32 = 1.0 / 60.0;

#[test]
fn the_probe_finds_the_ground_under_a_point_over_or_under_the_roots_level() {
    for h in [-0.5, -0.2, 0.0, 0.3, 0.55, 0.65] {
        let under = probe(&floor(h), DVec3::new(1.0, 0.2, 2.0), DVec3::ZERO, UP).unwrap();
        assert!((under.height - h).abs() < 2e-3, "{h}: {under:?}");
        assert!(under.normal.dot(UP) > 0.999, "{h}: {under:?}");
    }
    // Further down than it looks, or a face it starts in: no ground.
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
/// the ankle and 12 cm ahead (+x).
const ANKLE: Vec3 = Vec3::new(0.0, 0.08, 0.0);
const BALL: Vec3 = Vec3::new(0.12, -0.05, 0.0);

fn flat(x: f32) -> Pose {
    Pose {
        at: ANKLE + Vec3::X * x,
        turn: Quat::IDENTITY,
    }
}

fn foot(shown: Pose, lock: Option<Pose>, root: Vec3) -> Foot {
    Foot {
        shown,
        ball: BALL,
        lock,
        root,
        up: UP,
        off: false,
        hip: Vec3::new(0.0, 10.0, 0.0),
    }
}

/// The ground `ground(x)` high (over y 0) with its normal, as `probe` answers it over `root`.
fn under_of(ground: impl Fn(f32) -> (f32, Vec3), root: Vec3) -> impl Fn(Vec3) -> Option<Under> {
    move |at: Vec3| {
        let (height, normal) = ground(at.x);
        Some(Under {
            height: height - root.y,
            normal,
        })
    }
}

/// Level ground with a tread `rise` higher past x 0.
fn tread(rise: f32) -> impl Fn(f32) -> (f32, Vec3) {
    move |x| (if x >= 0.0 { rise } else { 0.0 }, UP)
}

/// Each sole point's height over the ground under it, heel (the ankle) then ball, less the
/// clip's own over its flat ground: 0 where the foot stands as the clip has it.
fn off_ground(
    placed: Pose,
    shown: Pose,
    root: Vec3,
    ground: &impl Fn(f32) -> (f32, Vec3),
) -> [f32; 2] {
    let ball = |pose: Pose| pose.at + pose.turn * BALL;
    [
        (placed.at.y - ground(placed.at.x).0) - (shown.at.y - root.y),
        (ball(placed).y - ground(ball(placed).x).0) - (ball(shown).y - root.y),
    ]
}

#[test]
fn a_foot_locked_over_a_higher_tread_is_raised_onto_it_and_keeps_that_ground_for_its_lock() {
    let ground = tread(0.3);
    let mut placing = Placing::default();
    let at = flat(0.1);
    let mut probes = 0;
    let mut root = Vec3::ZERO;
    let mut placed = at;
    for frame in 0..60 {
        // The body steps up onto the tread under the planted foot: its root rises.
        root.y = (frame as f32 / 30.0).min(1.0) * 0.3;
        let shown = Pose {
            at: at.at + root,
            ..at
        };
        let under = under_of(&ground, root);
        placed = placing.foot(0, foot(shown, Some(at), root), DT, |p| {
            probes += 1;
            under(p)
        });
        if frame == 0 {
            // Placed at once on its first frame.
            let [heel, ball] = off_ground(placed, shown, root, &ground);
            assert!(heel.abs() < 1e-3 && ball.abs() < 1e-3, "{heel} {ball}");
        }
        // Never lower than the clip has it, nor than the tread it stands on.
        assert!(placed.at.y >= shown.at.y - 1e-5);
        assert!(placed.at.y >= 0.3 + ANKLE.y - 1e-3, "{frame}: {placed:?}");
    }
    // Probed once at its lock, heel and ball, and not again while locked.
    assert_eq!(probes, 2);
    assert!((placed.at.y - (0.3 + ANKLE.y)).abs() < 1e-3, "{placed:?}");
    // A new lock is probed again.
    let next = flat(0.5);
    placing.foot(0, foot(next, Some(next), Vec3::ZERO), DT, |p| {
        probes += 1;
        under_of(&ground, Vec3::ZERO)(p)
    });
    assert_eq!(probes, 4);
}

#[test]
fn a_swinging_foot_is_raised_by_the_ground_under_its_ankle_eased_and_never_lowered() {
    let mut placing = Placing::default();
    let ground = tread(0.3);
    let under = under_of(&ground, Vec3::ZERO);
    // Over the lower side first, then over the tread: one probe a frame.
    let mut probes = 0;
    let mut count = |p: Vec3| {
        probes += 1;
        under(p)
    };
    assert_eq!(
        placing.foot(0, foot(flat(-0.3), None, Vec3::ZERO), DT, &mut count),
        flat(-0.3)
    );
    // Swung 0.25 m up, clear of the tread: eased, part of the way the first frame, all of it in
    // a few tenths of a second.
    let up = |pose: Pose| Pose {
        at: pose.at + UP * 0.25,
        ..pose
    };
    let first = placing.foot(0, foot(up(flat(0.2)), None, Vec3::ZERO), DT, &mut count);
    assert!(
        first.at.y > ANKLE.y + 0.25 && first.at.y < ANKLE.y + 0.55,
        "{first:?}"
    );
    let mut last = first;
    for _ in 0..29 {
        last = placing.foot(0, foot(up(flat(0.2)), None, Vec3::ZERO), DT, &mut count);
    }
    assert!((last.at.y - (ANKLE.y + 0.55)).abs() < 1e-3, "{last:?}");
    // Low over the tread, it is out of the ground at once.
    let low = placing.foot(0, foot(flat(0.2), None, Vec3::ZERO), DT, &mut count);
    assert!(low.at.y >= ANKLE.y + 0.3 - 1e-4, "{low:?}");
    assert_eq!(probes, 32);
    // Over ground lower than the root's, or none, it stays as the clip has it (the pelvis lowers
    // it, unit 2c).
    let mut low = Placing::default();
    let down = under_of(|_| (-0.3, UP), Vec3::ZERO);
    for _ in 0..30 {
        assert_eq!(
            low.foot(1, foot(flat(0.0), None, Vec3::ZERO), DT, &down),
            flat(0.0)
        );
        assert_eq!(
            low.foot(0, foot(flat(0.0), None, Vec3::ZERO), DT, |_| None),
            flat(0.0)
        );
    }
}

#[test]
fn a_foot_is_pitched_and_rolled_to_its_ground_within_its_limit() {
    for (pitch, roll) in [(15.0_f32, 0.0_f32), (0.0, 20.0), (-12.0, 10.0), (45.0, 0.0)] {
        // Ground rising `pitch` toward +x (the foot's ahead) and `roll` toward -z.
        let (tp, tr) = (
            libm::tanf(pitch.to_radians()),
            libm::tanf(roll.to_radians()),
        );
        let normal = Vec3::new(-tp, 1.0, tr).normalize();
        let ground = move |x: f32| (0.1 + tp * x, normal);
        let mut placing = Placing::default();
        let shown = flat(0.0);
        let placed = placing.foot(
            0,
            foot(shown, Some(shown), Vec3::ZERO),
            DT,
            under_of(ground, Vec3::ZERO),
        );
        let tilt = placed.turn * Vec3::Y;
        let along_pitch = libm::atan2f(-tilt.x, tilt.y).to_degrees();
        let along_roll = libm::atan2f(tilt.z, tilt.y).to_degrees();
        let want = pitch.clamp(-TILT_DEG, TILT_DEG);
        assert!(
            (along_pitch - want).abs() < 0.5,
            "{pitch} {roll}: pitched {along_pitch}"
        );
        if pitch.abs() < TILT_DEG {
            assert!(
                (along_roll - roll).abs() < 0.5,
                "{pitch} {roll}: rolled {along_roll}"
            );
            // Heel and ball both on their ground as the clip has them on its flat ground, or
            // over it where the heel's ground is below the root's level (unit 2c lowers it).
            let [heel, ball] = off_ground(placed, shown, Vec3::ZERO, &ground);
            assert!(
                heel.abs() < 5e-3 && (ball - heel).abs() < 5e-3,
                "{heel} {ball}"
            );
        }
    }
}

#[test]
fn raising_a_foot_never_asks_its_leg_for_more_than_the_clip_did() {
    let hip = Vec3::new(0.0, 0.95, 0.0);
    let t = libm::tanf(25.0_f32.to_radians());
    let slope = move |x: f32| (t * x, Vec3::new(-t, 1.0, 0.0).normalize());
    for (x, roll) in [
        (0.3, 0.0),
        (-0.3, 0.0),
        (-0.45, -0.6),
        (0.45, 0.4),
        (-0.5, 0.8),
    ] {
        // A foot ahead of or behind the hip, flat or rolled heel up or toe up, on a slope
        // rising ahead: turned to it and raised, never farther from the hip.
        let shown = Pose {
            turn: Quat::from_rotation_z(roll),
            ..flat(x)
        };
        let mut placing = Placing::default();
        let placed = placing.foot(
            0,
            Foot {
                hip,
                ..foot(shown, Some(shown), Vec3::ZERO)
            },
            DT,
            under_of(slope, Vec3::ZERO),
        );
        assert!(
            placed.at.distance(hip) <= shown.at.distance(hip) + 1e-5,
            "{x} {roll}"
        );
    }
}

#[test]
fn a_foot_let_go_goes_back_to_the_clip() {
    let mut placing = Placing::default();
    let under = under_of(tread(0.3), Vec3::ZERO);
    let shown = flat(0.2);
    assert!(
        placing
            .foot(0, foot(shown, Some(shown), Vec3::ZERO), DT, &under)
            .at
            .y
            > 0.3
    );
    let mut last = shown;
    for _ in 0..30 {
        let off = Foot {
            off: true,
            ..foot(shown, None, Vec3::ZERO)
        };
        last = placing.foot(0, off, DT, &under);
    }
    assert_eq!(last, shown);
}

#[test]
fn a_foot_locked_across_a_riser_rests_on_the_higher_tread_and_its_ball_stays_put() {
    // Heel over the lower tread, ball over one 0.45 m higher: the sole rests on the higher, its
    // ball not sunk however far it can pitch, the heel over the edge.
    let ground = tread(0.45);
    let mut placing = Placing::default();
    let shown = flat(-0.06);
    let under = under_of(&ground, Vec3::ZERO);
    let placed = placing.foot(0, foot(shown, Some(shown), Vec3::ZERO), DT, &under);
    let [heel, ball] = off_ground(placed, shown, Vec3::ZERO, &ground);
    assert!(ball >= -1e-3 && heel > 0.0, "{heel} {ball}");
    // On a slope, the clip rolling the locked foot onto its toe: its ball never moves across
    // (the tilt turns it about its ball, not its ankle).
    let t = libm::tanf(20.0_f32.to_radians());
    let normal = Vec3::new(-t, 1.0, 0.0).normalize();
    let slope = under_of(move |x: f32| (t * x, normal), Vec3::ZERO);
    let mut placing = Placing::default();
    let lock = flat(0.0);
    for frame in 0..20 {
        let shown = Pose {
            at: lock.at + Vec3::Y * frame as f32 * 0.005,
            turn: Quat::from_rotation_z(-(frame as f32) * 0.03),
        };
        // The hip back from the foot, as at a heel strike, so the leg reaches the turn.
        let locked = Foot {
            hip: Vec3::new(-0.3, 0.95, 0.0),
            ..foot(shown, Some(lock), Vec3::ZERO)
        };
        let placed = placing.foot(0, locked, DT, &slope);
        let ball = (placed.at + placed.turn * BALL).reject_from(UP);
        let shown_ball = (shown.at + shown.turn * BALL).reject_from(UP);
        assert!(
            (ball - shown_ball).length() < 1e-4,
            "{frame}: {ball} vs {shown_ball}"
        );
    }
}
