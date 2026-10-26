//! Feet that stay where they are put (docs/movement.md, the fix-up layer; D455): a foot the
//! clip has on the ground is locked to the world where it landed, and the leg is bent to it,
//! while the body is driven, warped or turned over it; when the body has gone too far from it
//! the foot steps, lifting and landing again under the clip's foot. A move's drive stands in
//! for root motion its clip does not carry, so without this its feet slide (`film.feet_sliding`).
//!
//! How shipped games do it: a move with root motion steps in its clip; a capsule driven by
//! code (locomotion, a warp) keeps its feet by locking each planted foot and releasing it past
//! an unplant radius, as Unreal's Foot Placement node does (its documentation; no code taken).
//! The leg's solve is Godot's TwoBoneIK3D (MIT, `scene/3d/two_bone_ik_3d.cpp`, Copyright (c)
//! 2014-present Godot Engine contributors): the knee where two circles meet, on the side of
//! the pole.
//!
//! Engine-free: poses are world positions and turns, so it tests without a skeleton; the game
//! reads the clip's pose and writes the leg (`terra_creatures::melee::feet`).

use glam::{Quat, Vec3};
use serde::Deserialize;

/// How high a body's clips stand a planted foot (D455): the clip's ball of the foot within
/// `low_m` of its ground is on it, the plant; past `lifted_m` the clip has lifted it, a margin
/// over `low_m` so a foot rolling off its toe is not let go and taken again frame by frame.
/// Data, by the clips a body plays (D136): a fighter's stance stands its ball about 3-4 cm over
/// the feet (its move set's), a jog's or a sprint's stride 3-7 cm, rolling heel to toe (the
/// body's kind's, `assets/data/movers.ron`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plant {
    pub low_m: f32,
    pub lifted_m: f32,
}
/// A locked foot this far from the clip's steps when the other is down; this far, at once.
pub const REACH_M: f32 = 0.22;
pub const MOST_M: f32 = 0.38;
/// A locked foot turned this far from the clip's steps when the other is down; this far, at once.
pub const TURN_DEG: f32 = 35.0;
pub const MOST_TURN_DEG: f32 = 70.0;
/// A body standing still this long with a foot this far from its clip's settles it (a settle
/// step), so a stance ends as its clip stands.
pub const SETTLE_M: f32 = 0.06;
pub const SETTLE_S: f32 = 0.25;
pub const STILL_MPS: f32 = 0.15;
/// The clip's foot moving this fast in the body's own frame is swinging, skimming the ground
/// in a stride, not standing on it: it is not locked. A drive or a turn moves the body, not the
/// foot in its frame, so a foot it carries is locked however fast the body goes.
pub const SWING_MPS: f32 = 3.2;
/// A move driving the body faster than this at its peak (a roll) is not on its feet: they go
/// as the clip has them.
pub const DRIVEN_MPS: f32 = 4.0;
/// How long a step takes, and how high it lifts the foot at most (half the way it goes).
pub const STEP_S: f32 = 0.16;
pub const STEP_UP_M: (f32, f32) = (0.05, 0.09);
/// Share of a step by which the foot is over where it lands; the rest it comes down.
const OVER: f32 = 0.7;
/// A blend back to the clip, when it lifts the foot or the lock is let go.
pub const BLEND_S: f32 = 0.12;

/// A foot's place and turn in the world (its ankle, as the skeleton's foot bone has it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub at: Vec3,
    pub turn: Quat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    /// As the clip has it.
    Free,
    /// Locked where it landed.
    Planted(Pose),
    /// Stepping from a lock to land under the clip's foot, `s` of the way.
    Stepping {
        from: Pose,
        s: f32,
        land: Option<Vec3>,
    },
    /// Going back to the clip's foot.
    Blending { from: Pose, s: f32 },
}

/// What the clip does with a foot this frame, as `Foot::update` reads it.
#[derive(Clone, Copy, Debug)]
pub struct Posed {
    /// The foot where the clip puts it, in the world.
    pub pose: Pose,
    /// How high the clip's ball of the foot is over the body's ground.
    pub ball_up: f32,
    /// How high the clips playing stand a planted foot.
    pub plant: Plant,
    /// The world's up.
    pub up: Vec3,
    /// How fast the clip moves the foot in the body's own frame, metres a second.
    pub swing_mps: f32,
    /// The leg cannot reach the lock (the solve straightened it).
    pub strained: bool,
    /// Feet not kept this frame (a body down, a roll).
    pub off: bool,
    /// The body came down this frame: a foot the clip has low takes its weight however the
    /// clip swings it (a landing at a run, its foot still blending from the fall).
    pub landing: bool,
}

/// One foot's lock.
#[derive(Clone, Copy, Debug)]
pub struct Foot {
    state: State,
    last: Option<Vec3>,
    shown: Option<Pose>,
    still_s: f32,
}

impl Default for Foot {
    fn default() -> Self {
        Self {
            state: State::Free,
            last: None,
            shown: None,
            still_s: 0.0,
        }
    }
}

fn ease(s: f32) -> f32 {
    let s = s.clamp(0.0, 1.0);
    s * s * (3.0 - 2.0 * s)
}

impl Foot {
    /// Mid-step or blending: the other foot waits to step until this one is down.
    pub fn moving(&self) -> bool {
        matches!(self.state, State::Stepping { .. } | State::Blending { .. })
    }

    /// How it is kept, in a word: free, planted, stepping or blending.
    pub fn state(&self) -> &'static str {
        match self.state {
            State::Free => "free",
            State::Planted(_) => "planted",
            State::Stepping { .. } => "stepping",
            State::Blending { .. } => "blending",
        }
    }

    /// Locked where it landed.
    pub fn planted(&self) -> bool {
        matches!(self.state, State::Planted(_))
    }

    /// Where the foot is this frame, `dt` seconds on, from the clip's foot; `other_moving`
    /// when the other foot is mid-step.
    pub fn update(&mut self, posed: Posed, dt: f32, other_moving: bool) -> Pose {
        let shown = self.step(posed, dt, other_moving);
        self.shown = Some(shown);
        shown
    }

    fn step(&mut self, posed: Posed, dt: f32, other_moving: bool) -> Pose {
        let clip = posed.pose;
        let speed = match self.last {
            Some(last) if dt > 0.0 => (clip.at - last).length() / dt,
            _ => 0.0,
        };
        self.last = Some(clip.at);
        if dt > 0.0 {
            self.still_s = if speed < STILL_MPS {
                self.still_s + dt
            } else {
                0.0
            };
        }
        let flat = |a: Vec3, b: Vec3| (a - b).reject_from(posed.up).length();
        let low = posed.ball_up <= posed.plant.low_m;
        let lifted = posed.ball_up > posed.plant.lifted_m;
        let gone = posed.off;
        let stands = low && !gone && (posed.swing_mps < SWING_MPS || posed.landing);
        if let State::Planted(lock) = self.state {
            let off_by = flat(lock.at, clip.at);
            let turned = lock.turn.angle_between(clip.turn).to_degrees();
            let settle = self.still_s >= SETTLE_S && off_by > SETTLE_M;
            let urgent = posed.strained || off_by > MOST_M || turned > MOST_TURN_DEG;
            let soft = off_by > REACH_M || turned > TURN_DEG || settle;
            if gone || lifted {
                self.state = State::Blending { from: lock, s: 0.0 };
            } else if urgent || (soft && !other_moving) {
                self.state = State::Stepping {
                    from: lock,
                    s: 0.0,
                    land: None,
                };
            } else {
                return lock;
            }
        }
        match self.state {
            State::Free => {
                if stands {
                    self.state = State::Planted(clip);
                }
                clip
            }
            State::Planted(lock) => lock,
            State::Blending { from, s } => {
                let s = s + dt / BLEND_S;
                if s >= 1.0 {
                    self.state = if stands {
                        State::Planted(clip)
                    } else {
                        State::Free
                    };
                    return clip;
                }
                self.state = State::Blending { from, s };
                // Up as the clip lifts it at once, across in the blend's time: a foot that
                // went across while still on the ground would slide.
                let across = from.at.lerp(clip.at, ease(s));
                Pose {
                    at: across + posed.up * (clip.at - across).dot(posed.up),
                    turn: from.turn.slerp(clip.turn, ease(s)),
                }
            }
            State::Stepping { from, s, land } => {
                if gone || lifted {
                    let from = self.shown.unwrap_or(from);
                    self.state = State::Blending { from, s: 0.0 };
                    return from;
                }
                let s = s + dt / STEP_S;
                let up = (flat(from.at, clip.at) * 0.5).clamp(STEP_UP_M.0, STEP_UP_M.1);
                let lift = posed.up * up * libm::sinf(core::f32::consts::PI * s.min(1.0));
                // Over the clip's foot by OVER of the way, then down onto where it was then,
                // so the foot lands still rather than gliding in with a driven body.
                let land = if s >= OVER {
                    Some(land.unwrap_or(clip.at))
                } else {
                    None
                };
                let turn = from.turn.slerp(clip.turn, ease(s / OVER));
                if s >= 1.0 {
                    let at = land.unwrap_or(clip.at);
                    self.state = State::Planted(Pose { at, turn });
                    return Pose { at, turn };
                }
                self.state = State::Stepping { from, s, land };
                let at = land.unwrap_or_else(|| from.at.lerp(clip.at, ease(s / OVER)));
                Pose {
                    at: at + lift,
                    turn,
                }
            }
        }
    }
}

/// How fast the clip moves a foot over the ground its feet stand on, metres a second, which
/// `Posed::swing_mps` takes: the foot's speed in the body's own frame, from `was` to `now`
/// over `dt` along the ground (`up` the body's up: a step's smoothing lifts the figure under the body, not its foot over the ground), with that ground passing under the body at `ground`, in the body's frame too.
/// A clip that travels (a walk, a jog, a strafe) is played in place at its pace (`pace.rs` in
/// the walker), so the ground its feet stand on goes by at the body's speed over the world:
/// its planted foot moves back under the body at that speed and stands still on that ground,
/// however fast the body goes, while its swinging foot passes over it. For a clip that stands
/// (a stance, a blow) `ground` is zero: the body's frame, which a drive or a turn moves and
/// the clip's feet do not. One rule for both, so a foot is planted by the clip whatever the
/// body does or holds (stride matching's foot locking, as Unreal's Foot Placement node reads
/// its plant speed against the motion the animation carries; its documentation, no code).
pub fn swing_mps(was: Vec3, now: Vec3, ground: Vec3, up: Vec3, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    ((now - was) / dt + ground).reject_from(up).length()
}

/// A two-bone limb from `root` (a hip) with bones `upper` and `lower` long, bent to reach
/// `target`, its middle joint (a knee) on the side of `pole`: where the middle joint and the
/// end go. A target out of reach gets the limb straight toward it; one nearer than the bones
/// allow, the limb folded at its least. Godot's TwoBoneIK3D's solve (see the module).
pub fn two_bone(root: Vec3, upper: f32, lower: f32, target: Vec3, pole: Vec3) -> (Vec3, Vec3) {
    let to = target - root;
    let side = (pole - root).normalize_or(Vec3::Z);
    let u = to.normalize_or(side.any_orthonormal_vector());
    let d = to
        .length()
        .clamp((upper - lower).abs() + 1e-4, upper + lower);
    if d >= upper + lower {
        return (root + u * upper, root + u * (upper + lower));
    }
    let a = (d * d + upper * upper - lower * lower) / (2.0 * d);
    let h = libm::sqrtf((upper * upper - a * a).max(0.0));
    let bend = (pole - root)
        .reject_from_normalized(u)
        .normalize_or(u.any_orthonormal_vector());
    (root + u * a + bend * h, root + u * d)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    /// A stance's plant, as the sword-and-shield set has it.
    const STANCE: Plant = Plant {
        low_m: 0.055,
        lifted_m: 0.07,
    };
    /// A stride's, as a man's kind has it.
    const STRIDE: Plant = Plant {
        low_m: 0.075,
        lifted_m: 0.09,
    };

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
                ball_up: 0.035,
                plant: STANCE,
                up: Vec3::Y,
                swing_mps: 0.0,
                strained: false,
                off: false,
                landing: false,
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
    fn a_foot_the_clip_lifts_or_swings_low_or_a_body_down_goes_with_the_clip() {
        let mut foot = Foot::default();
        let pose = |x: f32, y: f32| Pose {
            at: Vec3::new(x, y, 0.0),
            turn: Quat::IDENTITY,
        };
        let on = |pose: Pose, ball_up: f32, off: bool| Posed {
            pose,
            ball_up,
            plant: STANCE,
            up: Vec3::Y,
            swing_mps: 0.0,
            strained: false,
            off,
            landing: false,
        };
        assert_eq!(
            foot.update(on(pose(0.0, 0.0), 0.03, false), DT, false),
            pose(0.0, 0.0)
        );
        assert!(foot.planted());
        // The clip lifts it: it blends to the clip's foot and is free by the blend's end.
        let mut last = pose(0.0, 0.0);
        for _ in 0..(BLEND_S / DT) as usize + 2 {
            last = foot.update(on(pose(0.05, 0.2), 0.2, false), DT, false);
            // Lifted as the clip lifts it before it goes across, so it never slides.
            assert!((last.at.y - 0.2).abs() < 1e-5, "{last:?}");
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
        // A stride's foot skimming the ground fast in the body's frame is not stood on.
        let mut swinging = Foot::default();
        for step in 0..10 {
            let at = pose(0.06 * step as f32, 0.0);
            let posed = Posed {
                swing_mps: 3.6,
                ..on(at, 0.04, false)
            };
            assert_eq!(swinging.update(posed, DT, false), at);
        }
        assert!(!swinging.planted());
        // Coming down, it takes the body's weight, however it swings.
        let at = pose(0.6, 0.0);
        let landing = Posed {
            swing_mps: 3.6,
            landing: true,
            ..on(at, 0.04, false)
        };
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
                    ball_up: 0.035 + lift,
                    plant: STRIDE,
                    up: Vec3::Y,
                    swing_mps: swing,
                    strained: false,
                    off: false,
                    landing: false,
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
            (end - hip).normalize().dot(axis) > 0.9999
                && (knee - hip).normalize().dot(axis) > 0.9999
        );
    }
}
