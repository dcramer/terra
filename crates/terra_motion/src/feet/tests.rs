//! The feet's lock (`terra_motion::feet`) on made-up clips and bodies.

use super::*;

const DT: f32 = 1.0 / 60.0;

/// The film's check (tools/sandbox/duel/checks.ts, `sliding`) on one foot's path: a ball
/// within 2 cm of the ground 4 frames or more is a plant; how far each plant travelled.
fn plants(path: &[Vec3]) -> Vec<f32> {
    let mut out = Vec::new();
    let mut run: Vec<Vec3> = Vec::new();
    let flat = |a: Vec3, b: Vec3| Vec3::new(a.x - b.x, 0.0, a.z - b.z).length();
    let mut close = |run: &mut Vec<Vec3>| {
        if run.len() >= 4 {
            out.push(run.iter().map(|p| flat(*p, run[0])).fold(0.0, f32::max));
        }
        run.clear();
    };
    for p in path {
        if p.y <= 0.02 {
            run.push(*p);
        } else {
            close(&mut run);
        }
    }
    close(&mut run);
    out
}

/// A body driven along `way` by `speed(t)` and turned by `turn(t)` (radians a second) for
/// `secs`, its clip holding a foot still at `foot` in its own frame on the ground: the foot's
/// path as kept, and as the clip had it.
fn drive(
    secs: f32,
    foot: Vec3,
    speed: impl Fn(f32) -> f32,
    turn: impl Fn(f32) -> f32,
) -> (Vec<Vec3>, Vec<Vec3>) {
    let (mut at, mut yaw) = (Vec3::ZERO, 0.0f32);
    let mut kept = Foot::default();
    let (mut path, mut clip) = (Vec::new(), Vec::new());
    let mut t = 0.0;
    while t < secs {
        let facing = Quat::from_rotation_y(yaw);
        at += facing * Vec3::Z * speed(t) * DT;
        yaw += turn(t) * DT;
        let pose = Pose {
            at: at + Quat::from_rotation_y(yaw) * foot,
            turn: Quat::from_rotation_y(yaw),
        };
        let posed = Posed {
            pose,
            lock: 1.0,
            up: Vec3::Y,
            swing_mps: 0.0,
            strained: false,
            off: false,
            landing: false,
            low: true,
            ball: Vec3::ZERO,
            sole: [0.0; 2],
            over: [0.0; 2],
        };
        path.push(kept.update(posed, DT, false).at);
        clip.push(pose.at);
        t += DT;
    }
    (path, clip)
}

#[test]
fn a_body_driven_over_a_planted_foot_steps_rather_than_sliding_and_ends_on_its_clip() {
    // The partner's heavy: 2.6 m/s for a third of a second, then standing.
    let speed = |t: f32| if (0.2..0.55).contains(&t) { 2.6 } else { 0.0 };
    let (path, clip) = drive(1.6, Vec3::new(0.12, 0.0, 0.1), speed, |_| 0.0);
    let unkept = plants(&clip);
    assert!(
        unkept.iter().any(|travel| *travel > 0.5),
        "the clip alone slides: {unkept:?}"
    );
    let kept = plants(&path);
    assert!(
        kept.len() >= 2,
        "it stood, stepped and stood again: {kept:?}"
    );
    assert!(
        kept.iter().all(|travel| *travel <= 0.03),
        "no plant slid past 3 cm: {kept:?}"
    );
    let gap = (*path.last().unwrap() - *clip.last().unwrap()).length();
    assert!(
        gap < 0.01,
        "standing still, it settled under its clip's foot: {gap} m off"
    );
    let most = path
        .iter()
        .zip(&clip)
        .map(|(a, b)| (*a - *b).length())
        .fold(0.0, f32::max);
    assert!(most <= MOST_M + 0.05, "never left far behind: {most} m");
}

#[test]
fn a_body_turning_in_place_over_its_feet_pivots_them_and_steps_without_sliding() {
    // Turned to face the partner: 90 degrees at 180 a second.
    let turn = |t: f32| if t < 0.5 { core::f32::consts::PI } else { 0.0 };
    let (path, clip) = drive(1.4, Vec3::new(0.15, 0.0, 0.12), |_| 0.0, turn);
    assert!(plants(&clip).iter().any(|travel| *travel > 0.1));
    let kept = plants(&path);
    assert!(
        kept.iter().all(|travel| *travel <= 0.03),
        "no plant slid past 3 cm: {kept:?}"
    );
    assert!((*path.last().unwrap() - *clip.last().unwrap()).length() < 0.01);
}

#[test]
fn a_foot_the_clip_lifts_or_a_body_down_goes_with_the_clip_and_one_swept_down_steps() {
    let mut foot = Foot::default();
    let pose = |x: f32, y: f32| Pose {
        at: Vec3::new(x, y, 0.0),
        turn: Quat::IDENTITY,
    };
    // Down by the clip's markers while its ball is low.
    let on = |pose: Pose, ball_up: f32, off: bool| Posed {
        pose,
        lock: if ball_up < 0.05 { 1.0 } else { 0.0 },
        up: Vec3::Y,
        swing_mps: 0.0,
        strained: false,
        off,
        landing: false,
        low: true,
        ball: Vec3::ZERO,
        sole: [0.0; 2],
        over: [0.0; 2],
    };
    assert_eq!(
        foot.update(on(pose(0.0, 0.0), 0.03, false), DT, false),
        pose(0.0, 0.0)
    );
    assert!(foot.planted());
    // The clip lifts it: it eases to the clip's foot and is free by the release's end.
    let mut last = pose(0.0, 0.0);
    for _ in 0..(BLEND_S / DT) as usize + 2 {
        last = foot.update(on(pose(0.05, 0.2), 0.2, false), DT, false);
        // Lifted as the clip lifts it (and over an arc) before it goes across, so it never
        // slides.
        assert!(last.at.y >= 0.2 - 1e-5, "{last:?}");
    }
    assert_eq!(last, pose(0.05, 0.2));
    assert!(!foot.planted() && !foot.moving());
    // A body down never plants.
    for _ in 0..10 {
        assert_eq!(
            foot.update(on(pose(0.1, 0.0), 0.03, true), DT, false),
            pose(0.1, 0.0)
        );
    }
    assert!(!foot.planted());
    // A foot swept fast over the ground under a full curve (a crossfade's: Idle fading in
    // as a body stops dead at a wall) is stood on where it comes down and stepped on to
    // the clip, never slid there; before, it went with the pose, gliding 60 cm.
    let mut swept = Foot::default();
    swept.update(on(pose(0.0, 0.0), 0.03, false), DT, false);
    for _ in 0..(BLEND_S / DT) as usize + 2 {
        swept.update(on(pose(0.05, 0.2), 0.2, false), DT, false);
    }
    assert!(!swept.planted() && !swept.moving());
    let path: Vec<Vec3> = (0..40)
        .map(|step| {
            let posed = Posed {
                swing_mps: if step < 10 { 3.6 } else { 0.0 },
                ..on(pose(0.1 + 0.06 * step.min(10) as f32, 0.0), 0.04, false)
            };
            swept.update(posed, DT, false).at
        })
        .collect();
    let slid = plants(&path);
    assert!(slid.iter().all(|travel| *travel <= 0.03), "{slid:?}");
    assert!(
        swept.planted() && (path[39].x - 0.7).abs() < 0.01,
        "{:?}",
        path[39]
    );
    // Coming down, it takes the body's weight, however it swings.
    let mut swinging = Foot::default();
    let at = pose(0.6, 0.0);
    let landing = Posed {
        swing_mps: 3.6,
        landing: true,
        ..on(at, 0.04, false)
    };
    // Coming down with the foot still up in the pose (its ankle past LOW_ANKLE_M) and
    // swinging, it is not locked in the air, whatever its curve says.
    let mut high = Foot::default();
    let up_in_pose = Posed {
        low: false,
        ..landing
    };
    assert_eq!(high.update(up_in_pose, DT, false), at);
    assert!(!high.planted());
    assert_eq!(swinging.update(landing, DT, false), at);
    assert!(swinging.planted());
}

/// A body jogging ahead at `speed` for `secs` with an in-place stride clip of `pace`
/// (stance and swing a half cycle each, `stride` long in the clip's frame): each foot's
/// path as kept with the clip's ground passing at `ground` (or not), and as the clip had it.
fn jog(secs: f32, speed: f32, pace: f32, stride: f32) -> [(Vec<Vec3>, Vec<Vec3>); 2] {
    let cycle = 2.0 * stride / pace;
    let mut feet = [Foot::default(), Foot::default()];
    let mut was: [Option<Vec3>; 2] = [None, None];
    let mut out: [(Vec<Vec3>, Vec<Vec3>); 2] = Default::default();
    let mut t = 0.0;
    while t < secs {
        let body = Vec3::Z * speed * t;
        let moving = [feet[0].moving(), feet[1].moving()];
        for side in 0..2 {
            // The clip's foot in the body's frame: back along the ground at its pace, then
            // forward over it, lifted, leaving and landing moving back with the ground as a
            // stride does (a Hermite curve, its ends' speed the pace's).
            let phase = (t / cycle + side as f32 * 0.5).fract();
            let (z, lift) = if phase < 0.5 {
                (stride * (0.5 - 2.0 * phase), 0.0)
            } else {
                let s = 2.0 * phase - 1.0;
                let back = -pace * cycle * 0.5;
                let (s2, s3) = (s * s, s * s * s);
                let z = (2.0 * s3 - 3.0 * s2 + 1.0) * -stride * 0.5
                    + (s3 - 2.0 * s2 + s) * back
                    + (-2.0 * s3 + 3.0 * s2) * stride * 0.5
                    + (s3 - s2) * back;
                (z, 0.12 * libm::sinf(core::f32::consts::PI * s))
            };
            let in_body = Vec3::new(if side == 0 { 0.1 } else { -0.1 }, lift, z);
            let swing = was[side].map_or(0.0, |was| {
                swing_mps(was, in_body, Vec3::Z * speed, Vec3::Y, DT)
            });
            was[side] = Some(in_body);
            let pose = Pose {
                at: body + in_body,
                turn: Quat::IDENTITY,
            };
            let posed = Posed {
                pose,
                // The bake's curve: 1 in the stance, falling to 0 over 0.1 s as it lifts.
                lock: (1.0 - (phase - 0.5).max(0.0) * cycle / 0.1).max(0.0),
                up: Vec3::Y,
                swing_mps: swing,
                strained: false,
                off: false,
                landing: false,
                low: true,
                ball: Vec3::ZERO,
                sole: [0.0; 2],
                over: [0.0; 2],
            };
            out[side]
                .0
                .push(feet[side].update(posed, DT, moving[1 - side]).at);
            out[side].1.push(pose.at);
        }
        t += DT;
    }
    out
}

#[test]
fn a_stride_clips_planted_feet_stand_on_the_ground_it_passes_and_do_not_slide() {
    // A jog at 4.3 m/s whose clip strides a tenth too fast: its planted feet skate.
    let (speed, pace, stride) = (4.3, 4.73, 0.8);
    let paths = jog(2.0, speed, pace, stride);
    assert!(
        paths
            .iter()
            .flat_map(|(_, clip)| plants(clip))
            .any(|travel| travel > 0.03),
        "the clip alone slides"
    );
    for (path, _) in &paths {
        let kept = plants(path);
        assert!(kept.len() >= 3, "a plant a stride: {kept:?}");
        assert!(
            kept.iter().all(|travel| *travel <= 0.03),
            "no plant slid past 3 cm: {kept:?}"
        );
    }
    // Read in the body's frame alone, a stride's planted foot moves back as fast as the
    // body goes, past what a foot stood on moves; on the ground the clip's feet stand on it
    // stands still. Standing ground (a stance, a blow) is the body's frame.
    let (was, now) = (Vec3::new(0.0, 0.0, 0.1), Vec3::new(0.0, 0.0, 0.05));
    assert!(swing_mps(was, now, Vec3::ZERO, Vec3::Y, 0.01) > SWING_MPS);
    assert!(swing_mps(was, now, Vec3::Z * 5.0, Vec3::Y, 0.01) < 1e-4);
    // A foot carried up with the figure (a step smoothed) has not moved over the ground.
    assert!(swing_mps(was, was + Vec3::Y * 0.04, Vec3::ZERO, Vec3::Y, 0.01) < 1e-4);
}

/// A foot locked as its heel touches flattens and rolls as the clip does, its ball kept
/// where it was locked over the ground and its heading kept.
#[test]
fn a_locked_foot_takes_the_clips_height_and_roll_and_keeps_its_ball_and_heading() {
    let ball = Vec3::new(0.0, -0.07, 0.14);
    let lock = Pose {
        at: Vec3::new(0.0, 0.14, 0.0),
        turn: Quat::from_rotation_y(0.5) * Quat::from_rotation_x(-0.3),
    };
    let clip = Pose {
        at: Vec3::new(0.08, 0.10, 0.03),
        turn: Quat::from_rotation_y(0.2) * Quat::from_rotation_x(0.3),
    };
    let shown = held(lock, clip, Vec3::Y, ball);
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let (was, now) = (lock.at + lock.turn * ball, shown.at + shown.turn * ball);
    assert!((flat(now) - flat(was)).length() < 1e-5, "{shown:?}");
    assert!((now.y - (clip.at + clip.turn * ball).y).abs() < 1e-5);
    assert!(yaw_deg(lock.turn, shown.turn, Vec3::Y) < 0.01);
    let pitch = |q: Quat| (q * Vec3::Z).y;
    assert!((pitch(shown.turn) - pitch(clip.turn)).abs() < 1e-4);
}

#[test]
fn a_feet_turn_is_its_yaw_about_the_up_not_its_roll() {
    let yaw = Quat::from_rotation_y(50f32.to_radians());
    let roll = Quat::from_rotation_x(40f32.to_radians());
    assert!((yaw_deg(Quat::IDENTITY, yaw, Vec3::Y) - 50.0).abs() < 0.01);
    assert!(yaw_deg(Quat::IDENTITY, roll, Vec3::Y) < 0.01);
    assert!((yaw_deg(roll, yaw * roll, Vec3::Y) - 50.0).abs() < 0.5);
    let back = Quat::from_rotation_y(-170f32.to_radians());
    assert!((yaw_deg(yaw, back, Vec3::Y) - 140.0).abs() < 0.01);
}

#[test]
fn a_clips_lock_curve_is_read_between_its_samples_and_held_at_its_end() {
    let markers = Markers {
        length_s: 0.1,
        rate: 30.0,
        lock_l: vec![1.0, 1.0, 0.5, 0.0],
        lock_r: vec![0.0, 0.0, 0.0, 1.0],
        ..Default::default()
    };
    assert!((markers.lock(0, 0.05) - 0.75).abs() < 1e-4);
    assert_eq!(markers.lock(0, 0.0), 1.0);
    // A clip played once and held at its end stays as its end has it.
    assert_eq!(markers.lock(0, 3.0), 0.0);
    assert_eq!(markers.lock(1, 3.0), 1.0);
    assert_eq!(Markers::default().lock(0, 0.3), 0.0);
}

/// A lock starts only once the foot as placed is down (`PLANT_M`, Unreal's Foot Placement
/// DistanceToGround): a clip coming in at 1 while the pose still has the foot coming down
/// (a stop's fade), or a foot the pelvis has not yet lowered onto lower ground, is left
/// free, and is locked the frame it is down; a foot already locked is not let go for it.
#[test]
fn a_lock_starts_only_once_the_foot_as_placed_is_down() {
    let at = |x: f32, y: f32| Pose {
        at: Vec3::new(x, y, 0.0),
        turn: Quat::IDENTITY,
    };
    // The clip's ankle `y` up with the ball 6 cm under it, placed `over` past the clip.
    let posed = |x: f32, y: f32, lock: f32, over: f32| Posed {
        pose: at(x, y),
        lock,
        up: Vec3::Y,
        swing_mps: 0.0,
        strained: false,
        off: false,
        landing: false,
        low: true,
        ball: Vec3::ZERO,
        sole: [y, y - 0.06],
        over: [over; 2],
    };
    let mut foot = Foot::default();
    // A landing's lock, its clip's curve not at 1 and the fall's ball under its flat
    // ground, says nothing of where the foot rests.
    let landing = Posed {
        landing: true,
        ..posed(0.0, 0.0, 0.0, 0.0)
    };
    foot.update(landing, DT, false);
    assert!(foot.planted());
    assert_eq!(foot.aloft(&posed(0.0, 0.40, 1.0, 0.0)), 0.0);
    // Before its first lock nothing says where it rests: locked as the curve has it.
    foot.update(posed(0.0, 0.10, 1.0, 0.0), DT, false);
    assert!(foot.planted());
    // Let go and lifted.
    for _ in 0..30 {
        foot.update(posed(0.2, 0.40, 0.0, 0.0), DT, false);
    }
    assert!(!foot.planted() && !foot.moving(), "{}", foot.state());
    // A clip coming in at 1 with the foot still 30 cm up and coming down: free, "high",
    // following the clip, until its sole is within PLANT_M of where it rests.
    let mut y = 0.40;
    let mut locked_at = None;
    while y > 0.10 {
        let shown = foot.update(posed(0.3, y, 1.0, 0.0), DT, false);
        if foot.planted() {
            locked_at = Some(y);
            break;
        }
        assert_eq!(shown, at(0.3, y));
        assert_eq!(foot.why(), "high");
        y -= 0.02;
    }
    let locked_at = locked_at.expect("locked once down");
    assert!(locked_at - 0.10 <= PLANT_M + 1e-5, "{locked_at}");
    assert!(locked_at - 0.10 > PLANT_M - 0.02 - 1e-5, "{locked_at}");
    // Locked, the clip lifting it under a full curve does not let it go (a lock held is
    // not a lock's start).
    foot.update(posed(0.3, 0.30, 1.0, 0.0), DT, false);
    assert!(foot.planted());
    // Let go, then the clip down at its rest but the foot placed 30 cm over lower ground
    // (the pelvis not yet down): free until placement has it down.
    for _ in 0..30 {
        foot.update(posed(0.5, 0.40, 0.0, 0.0), DT, false);
    }
    for over in [0.30, 0.2, 0.1] {
        foot.update(posed(0.6, 0.10, 1.0, over), DT, false);
        assert!(!foot.planted(), "{over}");
    }
    foot.update(posed(0.6, 0.10, 1.0, 0.03), DT, false);
    assert!(foot.planted());
}

/// A planted foot is eased back to the clip by the lock weight as the curve falls; the
/// weight does not rise with the curve until it is back at 1, and then the foot is locked
/// again where it is shown, not snapped to the clip (ALS's foot lock).
#[test]
fn a_lock_lets_go_by_its_curve_and_takes_the_foot_again_only_at_one() {
    let at = |x: f32| Pose {
        at: Vec3::new(x, 0.1, 0.0),
        turn: Quat::IDENTITY,
    };
    let posed = |x: f32, lock: f32| Posed {
        pose: at(x),
        lock,
        up: Vec3::Y,
        swing_mps: 0.0,
        strained: false,
        off: false,
        landing: false,
        low: true,
        ball: Vec3::ZERO,
        sole: [0.0; 2],
        over: [0.0; 2],
    };
    // A frame as long as the lock takes to fall all the way, so it follows the curve.
    const STEP: f32 = RELEASE_S;
    let mut foot = Foot::default();
    assert_eq!(foot.update(posed(0.0, 1.0), STEP, false).at.x, 0.0);
    assert!(foot.planted());
    // The body carries the clip's foot 10 cm on: the lock holds.
    assert_eq!(foot.update(posed(0.1, 1.0), STEP, false).at.x, 0.0);
    // The curve falls to 0.5: halfway back to the clip.
    let half = foot.update(posed(0.1, 0.5), STEP, false);
    assert!((half.at.x - 0.05).abs() < 1e-4, "{half:?}");
    // Lifted over an arc as it goes: half the way it goes, at its middle.
    assert!((half.at.y - 0.15).abs() < 1e-4, "{half:?}");
    // It rises to 0.8 mid-roll: the weight stays, the foot goes no further back.
    let held = foot.update(posed(0.1, 0.8), STEP, false).at.x;
    assert!((held - 0.05).abs() < 1e-4, "{held}");
    assert!((foot.alpha() - 0.5).abs() < 1e-4);
    // Back at 1: locked where it is shown.
    assert!((foot.update(posed(0.12, 1.0), STEP, false).at.x - 0.05).abs() < 1e-4);
    assert!(foot.planted());
    // Falling to 0 lets it go to the clip.
    foot.update(posed(0.12, 0.3), STEP, false);
    assert_eq!(foot.state(), "releasing");
    assert_eq!(foot.update(posed(0.12, 0.0), STEP, false).at.x, 0.12);
    assert_eq!(foot.state(), "free");
    // A curve rising from 0 does not lock it before 1, and says so while the foot is low.
    foot.update(posed(0.12, 0.995), STEP, false);
    assert!(foot.planted());
    // A curve dropping to 0 at once (a fade's clips coming in) still eases it back over
    // RELEASE_S: half a frame's time, halfway.
    foot.update(posed(0.22, 1.0), DT, false);
    let eased = foot.update(posed(0.22, 0.0), RELEASE_S / 2.0, false).at.x;
    assert!((eased - 0.17).abs() < 1e-3, "{eased}");
}

#[test]
fn a_leg_reaches_what_it_can_with_its_bones_kept_and_the_knee_toward_the_pole() {
    let (hip, upper, lower) = (Vec3::new(0.0, 0.9, 0.0), 0.45, 0.42);
    let pole = Vec3::new(0.0, 0.5, 1.0);
    for target in [
        Vec3::new(0.0, 0.15, 0.1),
        Vec3::new(0.2, 0.1, -0.2),
        Vec3::new(0.0, 0.5, 0.3),
    ] {
        let (knee, end) = two_bone(hip, upper, lower, target, pole);
        assert!((end - target).length() < 1e-4, "reached {target}: {end}");
        assert!(((knee - hip).length() - upper).abs() < 1e-4);
        assert!(((end - knee).length() - lower).abs() < 1e-4);
        let axis = (target - hip).normalize();
        let bend = (knee - hip).reject_from_normalized(axis);
        assert!(bend.dot(pole - hip) > 0.0, "the knee bends toward the pole");
    }
    // Out of reach: straight toward it, as far as the bones go.
    let far = Vec3::new(0.0, -1.0, 0.5);
    let (knee, end) = two_bone(hip, upper, lower, far, pole);
    let axis = (far - hip).normalize();
    assert!(((end - hip).length() - (upper + lower)).abs() < 1e-4);
    assert!(
        (end - hip).normalize().dot(axis) > 0.9999 && (knee - hip).normalize().dot(axis) > 0.9999
    );
}
