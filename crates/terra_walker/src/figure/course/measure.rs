//! The animation course's measures (`super`), on frames recorded from the figure as the game poses
//! it: plain numbers on plain data, so each is tested against an answer known another way.
//!
//! - **Pop**: a joint's angular velocity jumping between frames (its angular acceleration, as a
//!   change of angular velocity a frame), against the worst the clips on either side of the
//!   transition do on their own at full weight (their steady state). A crossfade from a pose
//!   moving one way to a pose moving another changes the velocity in a frame (Bollo, GDC 2018,
//!   "Inertialization": the discontinuity a blend leaves); a ratio over 1 is a frame rougher
//!   than either clip ever is.
//! - **Slide**: a foot's ball moving over the ground while its clips have it planted (their
//!   baked lock curve at 1), summed over each stay planted: a planted foot moves none.
//! - **Glide**: the body's speed over the ground against the speed the clips' feet push it at
//!   (each clip's stride pace by its rate and warp, by its weight): a body going faster or
//!   slower than its feet glides (Unreal's stride warping and play-rate matching, Lyra's).
//! - **Seam**: a loop's pop at the frames it wraps from its end to its start.

use bevy::math::{Quat, Vec3};

/// A ball within this over its standing height is on the ground (the duel's `PLANTED_M`), m.
pub const DOWN_M: f32 = 0.02;

/// A joint's steady change of angular velocity a frame is never taken as under this, deg/s:
/// a joint its clips hold still would otherwise make any turn a pop. One degree a frame of
/// speed gained over a frame at 60 fps (3600 deg/s²), under what the eye reads as a jerk.
pub const FLOOR_DEG_S: f32 = 60.0;

/// One frame of the figure as posed: the clip coming in (the one the game shows), how far
/// into it, the major joints' turns from their parents, the balls' places in the world, the
/// body's speed over the ground and the speed its clips' feet push at.
#[derive(Clone, Debug)]
pub struct Frame {
    /// The game's clock (kept for a reader of the frames: Debug, a failing test's message).
    #[allow(dead_code)]
    pub t: f32,
    pub clip: String,
    pub seek: f32,
    /// The clips playing on the legs at their whole weight alone (no blend going on).
    pub solo: Option<String>,
    pub turns: Vec<Quat>,
    pub balls: [Vec3; 2],
    pub speed: f32,
    pub stride: f32,
    pub aloft: bool,
    /// Each foot's lock curve over the clips coming in (`feet::Kept::locks`, the contact their
    /// files bake, ALS's FootLock curves): 1 where the clips have the foot planted.
    pub lock: [f32; 2],
}

/// Each frame's change of angular velocity, a joint each, deg/s (0 for the first two frames).
pub fn kicks(frames: &[Frame], dt: f32) -> Vec<Vec<f32>> {
    let omega = |a: &Frame, b: &Frame| -> Vec<Vec3> {
        a.turns
            .iter()
            .zip(&b.turns)
            .map(|(qa, qb)| {
                let d = (qa.inverse() * *qb).normalize();
                let d = if d.w < 0.0 { -d } else { d };
                let (axis, angle) = d.to_axis_angle();
                axis * angle.to_degrees() / dt
            })
            .collect()
    };
    let n = frames.first().map_or(0, |f| f.turns.len());
    let mut out = vec![vec![0.0; n]; frames.len()];
    for i in 2..frames.len() {
        let was = omega(&frames[i - 2], &frames[i - 1]);
        let now = omega(&frames[i - 1], &frames[i]);
        out[i] = was
            .iter()
            .zip(&now)
            .map(|(a, b)| (*b - *a).length())
            .collect();
    }
    out
}

/// Whether frame `i` is where its clip wrapped from its end to its start (a loop's seam).
pub fn wraps(frames: &[Frame], i: usize) -> bool {
    i > 0 && frames[i].clip == frames[i - 1].clip && frames[i].seek + 1e-4 < frames[i - 1].seek
}

/// Each clip's steady state: the worst change of angular velocity a frame, a joint each, over
/// the frames it plays alone at full weight, its seams and the frames just after a blend left
/// out (`kicks` reads two frames back).
pub fn steady(frames: &[Frame], kicks: &[Vec<f32>]) -> std::collections::HashMap<String, Vec<f32>> {
    let mut out: std::collections::HashMap<String, Vec<f32>> = Default::default();
    for i in 3..frames.len() {
        let Some(clip) = &frames[i].solo else {
            continue;
        };
        let alone = (i - 2..i).all(|k| frames[k].solo.as_ref() == Some(clip));
        if !alone || (i - 2..=i).any(|k| wraps(frames, k)) {
            continue;
        }
        let worst = out
            .entry(clip.clone())
            .or_insert_with(|| vec![0.0; kicks[i].len()]);
        for (w, k) in worst.iter_mut().zip(&kicks[i]) {
            *w = w.max(*k);
        }
    }
    out
}

/// The worst pop over frames `from..to`: each joint's change of angular velocity over the
/// worst its clips `clips` do alone (`steady`), floored at `FLOOR_DEG_S`; with the joint and
/// frame it was at.
pub fn pop(
    kicks: &[Vec<f32>],
    steady: &std::collections::HashMap<String, Vec<f32>>,
    clips: &[&str],
    (from, to): (usize, usize),
) -> (f32, usize, usize) {
    let mut worst = (0.0, 0, from);
    for (i, kick) in kicks.iter().enumerate().take(to).skip(from) {
        for (j, k) in kick.iter().enumerate() {
            let own = clips
                .iter()
                .filter_map(|c| steady.get(*c).map(|s| s[j]))
                .fold(FLOOR_DEG_S, f32::max);
            let ratio = k / own;
            if ratio > worst.0 {
                worst = (ratio, j, i);
            }
        }
    }
    worst
}

/// How far each foot's ball went over the ground in its worst stay planted within frames
/// `from..to`, m: planted while the clips coming in have it down (`Frame::lock` at
/// `terra_motion::feet::FULL`, the contact their files bake, as the controller's feet glide
/// reads it; ALS's FootLock), the body on the ground and the ball within `DOWN_M` of `stand`
/// (its height standing still, over the course's ground at `Y` 0), as the duel's feet sliding
/// reads a plant from the ball over the ground: a foot lifted, as the lock's step lifts it to
/// its clip's place (ALS's dynamic transition), steps and does not slide. Contact is the
/// clips' first, the height only gating it: a mark set by the height alone, the run's lowest
/// ball, hid every slide once a roll dipped a ball under the ground, and one at the standing
/// ball took a sprint's swing skimming the ground for a slide (anim-blend j3). Along the
/// ground, its up `Y`; a foot rolling off as its curve ramps down is a release, not counted.
pub fn slide(frames: &[Frame], stand: [f32; 2], (from, to): (usize, usize)) -> f32 {
    let mut worst: f32 = 0.0;
    for (foot, stand) in stand.iter().enumerate() {
        let planted = |f: &Frame| {
            !f.aloft
                && f.lock[foot] >= terra_motion::feet::FULL
                && f.balls[foot].y <= stand + DOWN_M
        };
        let mut stay = 0.0;
        for i in from.max(1)..to.min(frames.len()) {
            if planted(&frames[i]) && planted(&frames[i - 1]) {
                let d = frames[i].balls[foot] - frames[i - 1].balls[foot];
                stay += Vec3::new(d.x, 0.0, d.z).length();
                worst = worst.max(stay);
            } else {
                stay = 0.0;
            }
        }
    }
    worst
}

/// The worst gap between the body's speed and its feet's over frames `from..to` on the
/// ground, m/s.
pub fn glide(frames: &[Frame], (from, to): (usize, usize)) -> f32 {
    frames[from..to.min(frames.len())]
        .iter()
        .filter(|f| !f.aloft)
        .map(|f| (f.speed - f.stride).abs())
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn turning(at: &[f32], clip: &str) -> Vec<Frame> {
        at.iter()
            .enumerate()
            .map(|(i, deg)| Frame {
                t: i as f32 * DT,
                clip: clip.into(),
                seek: i as f32 * DT,
                solo: Some(clip.into()),
                turns: vec![Quat::from_rotation_y(deg.to_radians())],
                balls: [Vec3::ZERO; 2],
                speed: 0.0,
                stride: 0.0,
                aloft: false,
                lock: [1.0; 2],
            })
            .collect()
    }

    /// A joint turning steadily changes its angular velocity none; one that starts turning at
    /// 120 deg/s in a frame changes it by 120.
    #[test]
    fn a_kick_is_the_change_of_angular_velocity_in_a_frame() {
        let steady: Vec<f32> = (0..10).map(|i| i as f32 * 2.0).collect();
        let k = kicks(&turning(&steady, "a"), DT);
        assert!(k[2..].iter().all(|f| f[0] < 1e-2), "{k:?}");
        let start: Vec<f32> = (0..10).map(|i| (i.max(4) - 4) as f32 * 2.0).collect();
        let k = kicks(&turning(&start, "a"), DT);
        assert!((k[5][0] - 120.0).abs() < 0.5, "{:?}", k[5]);
        assert!(k[6][0] < 0.5);
    }

    /// The pop is a frame's kick over the worst its clips do alone, floored: a kick twice the
    /// steady worst is 2, and a kick of a still clip is read against the floor.
    #[test]
    fn a_pop_is_a_kick_over_the_clips_own_worst() {
        let kicks = vec![vec![0.0], vec![100.0], vec![400.0]];
        let mut steady = std::collections::HashMap::new();
        steady.insert("jog".to_string(), vec![200.0]);
        assert_eq!(pop(&kicks, &steady, &["jog"], (0, 3)), (2.0, 0, 2));
        let (still, ..) = pop(&kicks, &steady, &["idle"], (0, 2));
        assert!((still - 100.0 / FLOOR_DEG_S).abs() < 1e-5);
    }

    /// A ball its clips have planted that moves 1 cm a frame for 5 frames slid 5 cm; one its
    /// clips lift between the moves, or moving while the body is aloft, slid none.
    #[test]
    fn a_slide_is_how_far_a_ball_went_while_planted() {
        let mut f = turning(&[0.0; 8], "a");
        for (i, frame) in f.iter_mut().enumerate() {
            frame.balls[0] = Vec3::new(i.min(5) as f32 * 0.01, 0.0, 0.0);
            frame.balls[1] = Vec3::new(i as f32 * 0.01, 0.0, 0.0);
            frame.lock[1] = if i % 2 == 0 { 0.0 } else { 1.0 };
        }
        let s = slide(&f, [0.0; 2], (0, 8));
        assert!((s - 0.05).abs() < 1e-5, "{s}");
        for frame in &mut f {
            frame.aloft = true;
        }
        assert_eq!(slide(&f, [0.0; 2], (0, 8)), 0.0);
    }

    /// The slide's contact is the clips', not the ball's height: a ball dipped under the ground
    /// once in the run (a roll's) hides no slide later, and a swing skimming the ground at a
    /// planted ball's height is no slide (anim-blend j3's two faults of a height mark).
    #[test]
    fn a_dip_elsewhere_hides_no_slide_and_a_low_swing_is_none() {
        let mut f = turning(&[0.0; 12], "a");
        for (i, frame) in f.iter_mut().enumerate() {
            frame.balls[0] = Vec3::new(0.0, 0.022, 0.0);
            if i >= 6 {
                frame.balls[0].x = (i - 6) as f32 * 0.008;
            }
            // The right foot swings 12 cm a frame at the standing height, its clips' curve 0.
            frame.balls[1] = Vec3::new(i as f32 * 0.12, 0.022, 0.0);
            frame.lock[1] = 0.0;
        }
        f[1].balls[0].y = -0.034;
        let s = slide(&f, [0.022; 2], (0, 12));
        assert!((s - 0.04).abs() < 1e-5, "{s}");
    }

    /// A loop's seam is where its time goes back.
    #[test]
    fn a_seam_is_where_the_clip_goes_back_to_its_start() {
        let mut f = turning(&[0.0; 4], "a");
        f[2].seek = 0.0;
        assert!(!wraps(&f, 1) && wraps(&f, 2) && !wraps(&f, 3));
    }
}
