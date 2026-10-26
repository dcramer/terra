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
        let under = probe(&floor(h), DVec3::new(1.0, 0.2, 2.0), DVec3::ZERO, UP, 0.0).unwrap();
        assert!((under.height - h).abs() < 2e-3, "{h}: {under:?}");
        assert!(under.normal.dot(UP) > 0.999, "{h}: {under:?}");
    }
    // Further down than it looks, or a face it starts in: no ground.
    assert_eq!(probe(&floor(-0.8), DVec3::ZERO, DVec3::ZERO, UP, 0.0), None);
    assert_eq!(probe(&floor(0.9), DVec3::ZERO, DVec3::ZERO, UP, 0.0), None);
    // On a slope, the ground under the point, and its normal.
    let deg: f32 = 30.0;
    let under = probe(&slope(deg), DVec3::new(0.4, 0.0, 0.0), DVec3::ZERO, UP, 0.0).unwrap();
    let want = 0.4 * libm::tanf(deg.to_radians());
    assert!((under.height - want).abs() < 0.01, "{under:?} vs {want}");
    assert!((under.normal.angle_between(UP).to_degrees() - deg).abs() < 1.0);
}

/// On a steep slope the probe's sphere touches the ground uphill of the point, higher than the
/// ground straight under it (by 2.7 cm on the yard's lump's 58 deg foot): it reads the ground
/// under the point, as a line down would, not the height it touched.
#[test]
fn the_probe_reads_the_ground_straight_under_a_point_on_a_steep_slope() {
    for deg in [20.0_f32, 45.0, 58.0, 70.0] {
        let under = probe(&slope(deg), DVec3::new(0.1, 0.0, 0.0), DVec3::ZERO, UP, 0.0).unwrap();
        let want = 0.1 * libm::tanf(deg.to_radians());
        assert!(
            (under.height - want).abs() < 2e-3,
            "{deg}: {under:?} vs {want}"
        );
    }
    // A point on level ground a centimetre short of a 60 deg face's foot: the sphere touches
    // the face, whose plane runs under the floor there; the floor is read.
    let n = Vec3::new(-libm::sinf(1.047), libm::cosf(1.047), 0.0);
    let crease = Sdf {
        distance: move |p: DVec3| {
            let p = p.as_vec3();
            p.y.min((p - Vec3::X * 0.03).dot(n))
        },
        steepest: 1.0,
    };
    let under = probe(&crease, DVec3::new(0.02, 0.0, 0.0), DVec3::ZERO, UP, 0.0).unwrap();
    assert!(under.height.abs() < 2e-3, "{under:?}");
    // A crack narrower than the sphere in level ground is no hole.
    let crack = Sdf {
        distance: move |p: DVec3| {
            let p = p.as_vec3();
            p.y.max(0.005 - p.x.abs()).min(p.y + 0.3)
        },
        steepest: 1.0,
    };
    let under = probe(&crack, DVec3::ZERO, DVec3::ZERO, UP, 0.0).unwrap();
    assert!(under.height.abs() < 2e-3, "{under:?}");
}

/// Up a stair, the figure is eased under its body (`terra_walker::Stepping`): a swinging foot's
/// next tread, 0.9 m over the figure's root and 0.4 m over the body's ground, is found from the
/// body's ground; a face higher than a step over the body is still none.
#[test]
fn the_probe_starts_over_the_bodys_ground_not_its_figures() {
    let under = probe(&floor(0.9), DVec3::ZERO, DVec3::ZERO, UP, 0.5).unwrap();
    assert!((under.height - 0.9).abs() < 2e-3, "{under:?}");
    assert_eq!(probe(&floor(1.3), DVec3::ZERO, DVec3::ZERO, UP, 0.5), None);
    // Still as deep under the root.
    assert_eq!(probe(&floor(-0.8), DVec3::ZERO, DVec3::ZERO, UP, 0.5), None);
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
        reach: 0.88,
        extent: None,
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
fn a_swinging_foot_is_raised_by_the_ground_under_its_heel_and_ball_and_eased() {
    let mut placing = Placing::default();
    let ground = tread(0.3);
    let under = under_of(&ground, Vec3::ZERO);
    // Over the lower side first, then over the tread: two probes a frame, heel and ball.
    let mut probes = 0;
    let mut count = |p: Vec3| {
        probes += 1;
        under(p)
    };
    assert_eq!(
        placing.foot(0, foot(flat(-0.3), None, Vec3::ZERO), DT, &mut count),
        flat(-0.3)
    );
    // Swung 0.25 m up, clear of the tread: eased, part of the way the first frame (never more
    // than half the clip's ball's clearance behind), all of it in a few tenths of a second.
    let up = |pose: Pose| Pose {
        at: pose.at + UP * 0.25,
        ..pose
    };
    let first = placing.foot(0, foot(up(flat(0.2)), None, Vec3::ZERO), DT, &mut count);
    assert!(
        first.at.y > ANKLE.y + 0.55 - 0.5 * (0.25 + ANKLE.y + BALL.y - BALL_REST_M) - 1e-4
            && first.at.y < ANKLE.y + 0.55,
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
    assert_eq!(probes, 64);
    // Over no ground (a drop past the probe), it stays as the clip has it.
    let mut low = Placing::default();
    for _ in 0..30 {
        assert_eq!(
            low.foot(0, foot(flat(0.0), None, Vec3::ZERO), DT, |_| None),
            flat(0.0)
        );
    }
}

#[test]
fn a_swinging_foot_whose_ball_is_over_a_higher_tread_is_raised_onto_it() {
    // The ankle over the lower side, the ball over a tread 0.3 m up: the ball is not left in it.
    let ground = tread(0.3);
    let shown = flat(-0.06);
    let placed = Placing::default().foot(
        0,
        foot(shown, None, Vec3::ZERO),
        DT,
        under_of(&ground, Vec3::ZERO),
    );
    let [heel, ball] = off_ground(placed, shown, Vec3::ZERO, &ground);
    assert!(ball >= -1e-3 && heel >= -1e-3, "{heel} {ball}");
    assert!(heel.min(ball) < 1e-3, "{heel} {ball}");
}

#[test]
fn a_swinging_foot_stays_put_when_the_root_steps_down_under_it() {
    // The body steps down 0.4 m at once off the tread its swinging foot is still over, the foot
    // 0.15 m clear of it as the clip has it: the foot's place in the world does not move (its
    // ground did not), nor does it sink toward the tread while it catches up.
    let ground = |_: f32| (0.0, UP);
    let mut placing = Placing::default();
    let clear = UP * 0.15;
    let mut settled = None;
    for frame in 0..40 {
        let root = if frame < 20 { Vec3::ZERO } else { -UP * 0.4 };
        let shown = Pose {
            at: flat(0.3).at + root + clear,
            ..flat(0.3)
        };
        let placed = placing.foot(0, foot(shown, None, root), DT, under_of(&ground, root));
        let [heel, ball] = off_ground(placed, shown, root, &ground);
        assert!(
            heel.abs() < 1e-3 && ball.abs() < 1e-3,
            "{frame}: {heel} {ball}"
        );
        let at = *settled.get_or_insert(placed.at);
        assert!(placed.at.distance(at) < 1e-3, "{frame}: {placed:?} vs {at}");
    }
}

#[test]
fn a_foot_swinging_down_onto_a_higher_tread_touches_it_only_as_the_clip_touches_down() {
    // The clip's foot comes down to its flat ground over 12 frames as it crosses onto a tread
    // 0.3 m up: it is raised onto the tread at once by at least half its clip's clearance, so it
    // never rests on the tread (its ball within 2 cm of its rest) before the clip's foot does.
    let ground = tread(0.3);
    let mut placing = Placing::default();
    for frame in 0..=12 {
        let high = 0.24 * (1.0 - frame as f32 / 12.0);
        let shown = Pose {
            at: flat(-0.3 + 0.04 * frame as f32).at + UP * high,
            ..flat(0.0)
        };
        let placed = placing.foot(
            0,
            foot(shown, None, Vec3::ZERO),
            DT,
            under_of(&ground, Vec3::ZERO),
        );
        // The ball's clearance over its ground, less the clip's ball's own on flat ground.
        let ball = placed.at + placed.turn * BALL;
        let over = ball.y - ground(ball.x).0 - (ANKLE.y + BALL.y);
        assert!(
            over >= LAG_SHARE * high - 1e-4,
            "{frame}: {over} vs clip {high}"
        );
        if high > 0.04 {
            assert!(over > 0.02, "{frame}: {over}");
        }
    }
}

/// A man's leg: the hip 0.9 m over the root's level, the leg 0.88 m long (the clip's ankle
/// 0.82 m under the hip, its knee a little bent).
const HIP: Vec3 = Vec3::new(0.0, 0.9, 0.0);
const LEG: f32 = 0.88;

fn leg(shown: Pose, lock: Option<Pose>, root: Vec3) -> Foot {
    Foot {
        hip: HIP + root + (shown.at - ANKLE - root).reject_from(UP),
        reach: LEG,
        ..foot(shown, lock, root)
    }
}

/// Both feet placed and the pelvis dropped, `frames` frames at the root `root(frame)`: each
/// frame's drop and the feet placed, their hips lowered by it.
fn walk(
    feet: [Pose; 2],
    locked: [bool; 2],
    ground: &impl Fn(f32) -> (f32, Vec3),
    root: impl Fn(usize) -> Vec3,
    frames: usize,
) -> Vec<(f32, [Pose; 2], [Vec3; 2])> {
    let mut placing = Placing::default();
    (0..frames)
        .map(|frame| {
            let root = root(frame);
            let under = under_of(ground, root);
            let shown = [0, 1].map(|side| Pose {
                at: feet[side].at + root,
                ..feet[side]
            });
            let grounded = [0, 1].map(|side| {
                let lock = locked[side].then_some(shown[side]);
                placing.ground(side, leg(shown[side], lock, root), DT, &under)
            });
            let drop = placing.pelvis([&grounded[0], &grounded[1]], root, DT);
            let placed = grounded.map(|g| g.placed(drop));
            let hips = shown.map(|shown| leg(shown, None, root).hip - UP * drop);
            (drop, placed, hips)
        })
        .collect()
}

#[test]
fn a_foot_over_lower_ground_is_lowered_onto_it_and_the_pelvis_with_it_springing_and_settling() {
    // The left foot locked over a tread 0.3 m under the root's, the right on the root's level.
    let ground = |x: f32| (if x >= 0.2 { -0.3 } else { 0.0 }, UP);
    let feet = [flat(0.3), flat(-0.2)];
    for locked in [[true, true], [false, true], [true, false], [false, false]] {
        let frames = walk(feet, locked, &ground, |_| Vec3::ZERO, 30);
        // The pelvis drops by the lower foot's need whether that foot is locked or swinging
        // (not weighted by the lock), never overshooting, most of the way in a tenth of a
        // second and all of it in a fifth.
        let drops: Vec<f32> = frames.iter().map(|(drop, ..)| *drop).collect();
        assert!(
            drops.windows(2).all(|d| d[1] >= d[0] - 1e-6),
            "{locked:?} {drops:?}"
        );
        assert!(
            drops.iter().all(|d| *d <= 0.3 + 1e-4),
            "{locked:?} {drops:?}"
        );
        assert!(drops[6] > 0.25 && drops[12] > 0.299, "{locked:?} {drops:?}");
        let (_, placed, _) = frames[29];
        // The lowered foot rests on its tread as the clip has it on flat ground; the other
        // stays on its own; neither leg reaches farther than the clip's.
        let [heel, ball] = off_ground(placed[0], feet[0], Vec3::ZERO, &ground);
        assert!(
            heel.abs() < 2e-3 && ball.abs() < 2e-3,
            "{locked:?} {heel} {ball}"
        );
        assert!(placed[1].at.distance(feet[1].at) < 1e-4, "{locked:?}");
        for (frame, (_, placed, hips)) in frames.iter().enumerate() {
            for side in 0..2 {
                let clip = feet[side]
                    .at
                    .distance(leg(feet[side], None, Vec3::ZERO).hip);
                let reach = placed[side].at.distance(hips[side]);
                assert!(
                    reach <= clip.max(REACH_RATIO * LEG) + 1e-4,
                    "{frame} {side}: {reach}"
                );
            }
        }
    }
}

#[test]
fn a_warped_strides_planted_leg_is_reached_no_farther_than_its_clips_by_lowering_the_pelvis() {
    // On flat ground, the left foot locked 0.5 m behind its hip where the warp put it, the
    // clip's own 0.4 m behind (its leg reaching `clip` from the hip): the pelvis comes down
    // until the leg reaches the warped foot no farther than the clip's reaches its own.
    let flat_ground = |_: f32| (0.0, UP);
    let hip = HIP + Vec3::X * 0.5;
    let clip = (hip - (ANKLE + Vec3::X * 0.1)).length();
    let mut placing = Placing::default();
    let under = under_of(flat_ground, Vec3::ZERO);
    let warped = |placing: &mut Placing, extent: Option<f32>, locked: bool| {
        let feet = [flat(0.0), flat(0.6)].map(|shown| Foot {
            hip,
            reach: LEG,
            extent,
            ..foot(shown, locked.then_some(shown), Vec3::ZERO)
        });
        let grounded = [0, 1].map(|side| placing.ground(side, feet[side], DT, &under));
        let mut drop = 0.0;
        for _ in 0..30 {
            drop = placing.pelvis([&grounded[0], &grounded[1]], Vec3::ZERO, DT);
        }
        (drop, grounded[0].placed(drop))
    };
    let (drop, placed) = warped(&mut placing, Some(clip), true);
    let reach = placed.at.distance(hip - UP * drop);
    assert!(
        drop > 0.01 && (reach - clip).abs() < 2e-3,
        "{drop} {reach} {clip}"
    );
    // Unwarped (no extent asked), or with neither foot locked, it does not come down.
    assert_eq!(warped(&mut Placing::default(), None, true).0, 0.0);
    assert_eq!(warped(&mut Placing::default(), Some(clip), false).0, 0.0);
    // Its own extent reached as the clip has it asks nothing.
    let own = (hip - ANKLE).length();
    assert!(warped(&mut Placing::default(), Some(own), true).0 < 1e-5);
}

#[test]
fn the_pelvis_does_not_drop_for_a_foot_over_a_drop_past_what_the_planted_leg_folds_to() {
    // The left foot over ground 0.58 m down, the right planted on the root's level: dropping
    // the pelvis that far would fold the right leg past `FOLD_RATIO`, so it does not drop, and
    // the left foot is lowered only as far as its own leg reaches (it stays short).
    let ground = |x: f32| (if x >= 0.2 { -0.58 } else { 0.0 }, UP);
    let feet = [flat(0.3), flat(-0.2)];
    let frames = walk(feet, [false, true], &ground, |_| Vec3::ZERO, 30);
    let (drop, placed, hips) = frames[29];
    assert_eq!(drop, 0.0);
    let reach = placed[0].at.distance(hips[0]);
    assert!(reach <= REACH_RATIO * LEG + 1e-4, "{reach}");
    assert!(placed[0].at.y < feet[0].at.y - 0.01, "{placed:?}");
    // Swinging over it with the other foot swinging too, nothing is stood on: it drops.
    let frames = walk(feet, [false, false], &ground, |_| Vec3::ZERO, 30);
    assert!(frames[29].0 > 0.57, "{}", frames[29].0);
}

#[test]
fn a_body_stepping_down_onto_ground_its_pelvis_went_to_does_not_dip_again() {
    // The pelvis drops 0.3 m for the left foot over the lower tread; then the body steps down
    // onto it (its root 0.3 m lower at once), both feet with it: the pelvis's place in the
    // world never goes under where it settled, and the drop is gone at once.
    let ground = |x: f32| (if x >= 0.2 { -0.3 } else { 0.0 }, UP);
    let feet = [flat(0.3), flat(0.25)];
    let root = |frame: usize| {
        if frame < 30 {
            Vec3::ZERO
        } else {
            Vec3::new(0.0, -0.3, 0.0)
        }
    };
    let frames = walk(feet, [true, false], &ground, root, 60);
    let settled = -frames[29].0;
    for (frame, (drop, ..)) in frames.iter().enumerate().skip(30) {
        let pelvis = root(frame).y - drop;
        assert!(pelvis >= settled - 1e-4, "{frame}: {pelvis} vs {settled}");
        assert!(*drop < 1e-4, "{frame}: {drop}");
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

/// A foot over lower ground stands over it, past where its clip has it, by what the pelvis has
/// not yet come down to (`Grounded::over`), and by nothing once it has: what a lock's start
/// reads (`feet::PLANT_M`). A foot on its ground as the clip has it is over by nothing.
#[test]
fn a_foot_over_lower_ground_is_over_it_until_the_pelvis_comes_down() {
    let ground = |x: f32| (if x >= 0.2 { -0.3 } else { 0.0 }, UP);
    let feet = [flat(0.3), flat(-0.2)];
    let under = under_of(ground, Vec3::ZERO);
    let mut placing = Placing::default();
    let overs: Vec<[[f32; 2]; 2]> = (0..30)
        .map(|_| {
            let grounded = [0, 1].map(|side| {
                let lock = (side == 1).then_some(feet[side]);
                placing.ground(side, leg(feet[side], lock, Vec3::ZERO), DT, &under)
            });
            let drop = placing.pelvis([&grounded[0], &grounded[1]], Vec3::ZERO, DT);
            grounded.map(|g| g.over(g.placed(drop)))
        })
        .collect();
    assert!(overs[0][0].iter().all(|&o| o > 0.1), "{:?}", overs[0]);
    assert!(
        overs.windows(2).all(|o| o[1][0][0] <= o[0][0][0] + 1e-5),
        "{overs:?}"
    );
    let last = overs[overs.len() - 1];
    assert!(last[0].iter().all(|o| o.abs() < 0.005), "{last:?}");
    assert!(
        overs.iter().all(|o| o[1].iter().all(|v| v.abs() < 1e-3)),
        "{overs:?}"
    );
}

/// A swinging foot raised onto a higher face that then swings off it over level ground (the
/// yard's stop at the mound) eases down; until it is down it reads as over its ground, as
/// probed, not as eased, so a lock's start (`feet::PLANT_M`) waits for it to land.
#[test]
fn a_foot_swung_off_a_higher_face_is_over_its_ground_until_it_has_eased_down() {
    let face = |x: f32| (if x >= 0.0 { 0.6 } else { 0.0 }, UP);
    let level = |_: f32| (0.0, UP);
    let shown = flat(-0.3);
    let mut placing = Placing::default();
    // Over the face for a few frames: raised onto it.
    for _ in 0..10 {
        let g = placing.ground(
            0,
            foot(flat(0.1), None, Vec3::ZERO),
            DT,
            under_of(face, Vec3::ZERO),
        );
        assert!(g.over(g.placed(0.0)).iter().all(|o| o.abs() < 1e-3));
    }
    // Swung back over level ground: eased down over frames, over it until then.
    let overs: Vec<f32> = (0..20)
        .map(|_| {
            let g = placing.ground(
                0,
                foot(shown, None, Vec3::ZERO),
                DT,
                under_of(level, Vec3::ZERO),
            );
            let placed = g.placed(0.0);
            let over = g.over(placed);
            let off = off_ground(placed, shown, Vec3::ZERO, &level);
            assert!((over[0] - off[0]).abs() < 1e-4 && (over[1] - off[1]).abs() < 1e-4);
            over[0].min(over[1])
        })
        .collect();
    assert!(overs[0] > crate::feet::PLANT_M, "{overs:?}");
    assert!(overs[overs.len() - 1] < 0.005, "{overs:?}");
}

/// A swinging foot raised onto a higher tread lags its ground by up to `LAG_SHARE` of its
/// clip's clearance; once locked (a heel strike, the clip's ball still 6 cm up) it is raised at
/// once, neither heel nor ball under the clip's height over the tread.
#[test]
fn a_locked_foot_is_raised_onto_its_ground_at_once() {
    let ground = tread(0.3);
    let shown = Pose {
        at: flat(0.1).at + UP * 0.06,
        ..flat(0.0)
    };
    let mut placing = Placing::default();
    // Swinging over the lower side, then over the tread: lagging.
    placing.foot(
        0,
        foot(flat(-0.3), None, Vec3::ZERO),
        DT,
        under_of(&ground, Vec3::ZERO),
    );
    let free = placing.foot(
        0,
        foot(shown, None, Vec3::ZERO),
        DT,
        under_of(&ground, Vec3::ZERO),
    );
    assert!(off_ground(free, shown, Vec3::ZERO, &ground)[0] < -0.01);
    // Lagging under its ground it is not read as down past where the clip has it.
    let g = placing.ground(
        0,
        foot(shown, None, Vec3::ZERO),
        DT,
        under_of(&ground, Vec3::ZERO),
    );
    assert_eq!(g.over(g.placed(0.0)), [0.0; 2]);
    let locked = placing.foot(
        0,
        foot(shown, Some(shown), Vec3::ZERO),
        DT,
        under_of(&ground, Vec3::ZERO),
    );
    let off = off_ground(locked, shown, Vec3::ZERO, &ground);
    assert!(off.iter().all(|o| o.abs() < 1e-3), "{off:?}");
}

/// A sole straddling a riser (its heel's and ball's grounds steeper apart than it pitches to)
/// rests flat on the higher tread as the clip has it, heel or ball over the edge at the clip's
/// height over that tread, never pitched down toward the lower one; ground no steeper than
/// that still pitches it.
#[test]
fn a_sole_straddling_a_riser_keeps_the_clips_pitch_on_the_higher_tread() {
    let span = BALL.x;
    for rise in [0.09_f32, 0.2, 0.5, -0.09, -0.3] {
        // Up a riser: the heel over the lower tread, the ball over the higher; down: the reverse.
        let ground = move |x: f32| {
            let high = if rise > 0.0 { x >= 0.06 } else { x < 0.06 };
            (if high { rise.abs() } else { 0.0 }, UP)
        };
        let shown = flat(0.0);
        for lock in [None, Some(shown)] {
            let mut placing = Placing::default();
            let placed = placing.foot(
                0,
                foot(shown, lock, Vec3::ZERO),
                DT,
                under_of(ground, Vec3::ZERO),
            );
            assert!(
                placed.turn.angle_between(shown.turn) < 1e-3,
                "{rise} {lock:?}: pitched {}",
                placed.turn.angle_between(shown.turn).to_degrees()
            );
            let over_higher = placed.at.y - rise.abs() - shown.at.y;
            assert!(over_higher.abs() < 1e-3, "{rise} {lock:?}: {over_higher}");
        }
    }
    // Ground 25 deg apart over the sole (a slope's, not a riser's) still pitches it.
    let rise = span * libm::tanf(25.0_f32.to_radians());
    let ground = move |x: f32| (if x >= 0.06 { rise } else { 0.0 }, UP);
    let placed = Placing::default().foot(
        0,
        foot(flat(0.0), None, Vec3::ZERO),
        DT,
        under_of(ground, Vec3::ZERO),
    );
    assert!(placed.turn.angle_between(Quat::IDENTITY).to_degrees() > 20.0);
}

/// A foot locked across a step down (its heel on the higher tread, its ball over the lower)
/// rests on the higher tread; once its toe-off has rolled the heel over the edge too, it is set
/// on the lower tread, not left there at the higher's height. A lock wholly on one tread keeps
/// the ground it was locked on whatever is probed later.
#[test]
fn a_foot_locked_across_a_step_down_comes_down_once_its_heel_leaves_the_edge() {
    let ground = |x: f32| (if x >= 0.06 { -0.09 } else { 0.0 }, UP);
    let mut placing = Placing::default();
    let lock = flat(0.0);
    let under = under_of(ground, Vec3::ZERO);
    let at = |placing: &mut Placing, shown: Pose| {
        let g = placing.ground(0, leg(shown, Some(lock), Vec3::ZERO), DT, &under);
        g.placed(g.need()).at.y
    };
    // Straddling: on the higher tread, as the clip has it.
    for _ in 0..10 {
        assert!((at(&mut placing, lock) - ANKLE.y).abs() < 1e-4);
    }
    // The heel rolled over the edge (the same lock, its ankle shown 7 cm on): it comes down
    // onto the lower tread, eased (the pelvis lowered by its need).
    let rolled = flat(0.07);
    let mut last = 0.0;
    for _ in 0..30 {
        last = at(&mut placing, rolled);
    }
    assert!((last - (ANKLE.y - 0.09)).abs() < 1e-3, "{last}");
    // Locked wholly on the higher tread, ground probed lower later does not move it.
    let mut placing = Placing::default();
    let high = flat(-0.2);
    let held = |placing: &mut Placing, shown: Pose| {
        let g = placing.ground(0, leg(shown, Some(high), Vec3::ZERO), DT, &under);
        g.placed(g.need()).at.y
    };
    held(&mut placing, high);
    for _ in 0..30 {
        last = held(&mut placing, flat(0.07));
    }
    assert!((last - ANKLE.y).abs() < 1e-4, "{last}");
}

/// A swinging foot over a riser rests on the higher tread, but its point over the edge hangs
/// over the lower one and is read as over its ground there (`Grounded::over`): a heel strike
/// over the edge is not down while its ball is still up.
#[test]
fn a_foot_over_a_riser_reads_its_point_over_the_edge_as_over_the_lower_tread() {
    let ground = |x: f32| (if x >= 0.06 { 0.2 } else { 0.0 }, UP);
    let mut placing = Placing::default();
    let g = placing.ground(
        0,
        foot(flat(0.0), None, Vec3::ZERO),
        DT,
        under_of(ground, Vec3::ZERO),
    );
    let over = g.over(g.placed(0.0));
    assert!(
        (over[0] - 0.2).abs() < 1e-3 && over[1].abs() < 1e-3,
        "{over:?}"
    );
}
