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

/// A clip's foot locks (D455), baked for every clip by `tools/characters/markers.ts` into its
/// file's glTF extras: each foot's lock curve, `rate` samples a second through the clip, 1
/// while the foot stands on the ground the clip strides on (its heel or toe tip still over the
/// ground passing at the clip's one pace, its height only a loose gate), falling to 0 as it
/// rolls off its toe; and its plants as `[down, up]` seconds (`up < down` over a loop's end),
/// named as Unreal's sync markers (foot down L/R), which sync groups will use. Data shipped with
/// the clips (D136), as ALS's FootLock curves and motion matching's contact labels are.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Markers {
    pub length_s: f32,
    pub rate: f32,
    pub lock_l: Vec<f32>,
    pub lock_r: Vec<f32>,
    pub foot_down_l: Vec<[f32; 2]>,
    pub foot_down_r: Vec<[f32; 2]>,
    /// A clip that keeps its root motion (a start, a stop): its pelvis's travel along the way it
    /// goes, m at `rate`, its distance curve (`crate::distance`); empty for one played in place.
    #[serde(default)]
    pub distance: Vec<f32>,
    /// That way in the pelvis's parent's space, so the travel is taken off the pelvis.
    #[serde(default)]
    pub distance_way: [f32; 3],
}

impl Markers {
    /// Foot `side`'s (0 left, 1 right) lock curve `t` seconds in, between its samples; a time
    /// past the clip's end (a clip played once, held at its end) is its end.
    pub fn lock(&self, side: usize, t: f32) -> f32 {
        let curve = if side == 0 {
            &self.lock_l
        } else {
            &self.lock_r
        };
        let Some(&last) = curve.last() else {
            return 0.0;
        };
        let at = t.clamp(0.0, self.length_s) * self.rate;
        let i = at as usize;
        if i + 1 >= curve.len() {
            return last;
        }
        let s = at - i as f32;
        curve[i] + (curve[i + 1] - curve[i]) * s
    }
}

/// A lock curve this near 1 is at 1: the foot stands and may be locked again.
pub const FULL: f32 = 0.99;

/// A locked foot this far from the clip's steps when the other is down; this far, at once.
pub const REACH_M: f32 = 0.22;
pub const MOST_M: f32 = 0.38;
/// A locked foot turned this far about the up from the clip's steps when the other is down
/// (turn in place: Lyra's and ALS's turn-in-place thresholds, about 45 deg); this far, at once.
/// Yaw alone: the clip's foot rolling heel to toe under a lock is no turn.
pub const TURN_DEG: f32 = 45.0;
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
/// The lock weight falls from 1 to 0 no faster than this (the bake's ramp, markers.ts RAMP_S).
pub const RELEASE_S: f32 = 0.1;
/// A blend back to the clip, when the lock is let go (a body down, a roll) or a step is cut.
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
    /// Locked where it landed while the lock weight falls with the clip's curve: the foot eased
    /// from the lock back to the clip by it (ALS's foot lock).
    Releasing(Pose),
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
    /// The clips' lock curve for the foot (`Markers::lock`), weighed by the weight of each
    /// clip playing on the legs, 0 to 1.
    pub lock: f32,
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
    /// The clip has the foot on the ground (its ankle within `LOW_ANKLE_M` of the body's
    /// ground), which a landing reads.
    pub low: bool,
    /// The ball of the foot in the foot's own frame, which a locked foot keeps still.
    pub ball: Vec3,
    /// The clip's heel (the ankle) and ball over its flat ground (the figure's root), m: a
    /// locked foot's lowest are where it rests (`PLANT_M`).
    pub sole: [f32; 2],
    /// How far the foot as placed on the ground last frame stood over its ground past where
    /// the clip has it over its flat ground, heel and ball, m (`placement::Grounded::over`: a
    /// foot over lower ground the pelvis has not yet come down to); 0 unplaced.
    pub over: [f32; 2],
}

/// A clip's ankle this near the body's ground has the foot on it (a flat foot's ankle stands
/// about 10 cm up).
pub const LOW_ANKLE_M: f32 = 0.15;

/// A lock starts only with the foot as placed this near where it rests on its ground, m: its
/// sole's lower point (heel or ball) over its ground, less where that point stands when locked
/// (as Unreal's Foot Placement plants a foot only within its PlantSettings' DistanceToGround of
/// the traced ground; its documentation, no code taken). The bake's height gate (markers.ts,
/// 7 cm over a point's lowest): a clip coming in whose curve is at 1 while the pose shown still
/// has the foot coming down (a stop's fade), or a foot over lower ground the pelvis has not yet
/// come down to, is locked once it is down, not in the air.
pub const PLANT_M: f32 = 0.07;

/// One foot's lock.
#[derive(Clone, Copy, Debug)]
pub struct Foot {
    state: State,
    last: Option<Vec3>,
    shown: Option<Pose>,
    still_s: f32,
    /// The lock weight: falls with the clips' curve, and rises again only once it is back at 1
    /// (ALS's FootLock alpha), so a foot let go is not taken again mid-roll.
    alpha: f32,
    /// What last kept the foot free while the clip had it low, as the fight log writes it.
    why: &'static str,
    /// Where the clip has the heel and ball while the foot is locked and its own curve has it
    /// down, at their lowest (`sole`): where it rests, which a lock's start is gated on
    /// (`PLANT_M`); none before its first such lock. A landing's lock (the fall's clip, its
    /// curve not at 1, the ball under its flat ground) is not where the foot rests.
    rest: Option<[f32; 2]>,
}

impl Default for Foot {
    fn default() -> Self {
        Self {
            state: State::Free,
            last: None,
            shown: None,
            still_s: 0.0,
            alpha: 0.0,
            why: "",
            rest: None,
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

    /// How it is kept, in a word: free, planted, releasing, stepping or blending.
    pub fn state(&self) -> &'static str {
        match self.state {
            State::Free => "free",
            State::Planted(_) => "planted",
            State::Releasing(_) => "releasing",
            State::Stepping { .. } => "stepping",
            State::Blending { .. } => "blending",
        }
    }

    /// Locked where it landed.
    pub fn planted(&self) -> bool {
        matches!(self.state, State::Planted(_))
    }

    /// Where the foot was locked while it is locked or let go (planted or releasing), else
    /// none: each lock is its own (foot placement freezes the ground found under it for its
    /// life, `placement`).
    pub fn locked(&self) -> Option<Pose> {
        match self.state {
            State::Planted(lock) | State::Releasing(lock) => Some(lock),
            _ => None,
        }
    }

    /// The lock weight, 0 to 1.
    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    /// Why the foot was last left free while its clips' curve had it locked (`"off"`: the body
    /// down, aloft or let go; `"swing"`: the clip's foot too fast; `"high"`: the foot as placed
    /// not yet down, `PLANT_M`; `"rising"`: the curve not yet back at 1), or `""`.
    pub fn why(&self) -> &'static str {
        self.why
    }

    /// Where the foot is this frame, `dt` seconds on, from the clip's foot; `other_moving`
    /// when the other foot is mid-step.
    pub fn update(&mut self, posed: Posed, dt: f32, other_moving: bool) -> Pose {
        let shown = self.step(posed, dt, other_moving);
        self.shown = Some(shown);
        if self.planted() && posed.lock >= FULL {
            let rest = self.rest.get_or_insert(posed.sole);
            *rest = [rest[0].min(posed.sole[0]), rest[1].min(posed.sole[1])];
        }
        shown
    }

    /// How far the foot as placed stands over where it rests (`PLANT_M`): its sole's lower
    /// point over its ground, less where that point stands locked; 0 before its first lock.
    pub fn aloft(&self, posed: &Posed) -> f32 {
        self.rest.map_or(0.0, |rest| {
            (0..2)
                .map(|i| posed.sole[i] + posed.over[i] - rest[i])
                .fold(f32::INFINITY, f32::min)
        })
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
        // The lock weight falls with the curve, no faster than RELEASE_S takes from 1 to 0 (a
        // curve that drops at once, a fade's clips coming in, still eases the foot back), and
        // rises only once it is back at 1. A body coming down stands a low foot at once.
        let curve = posed.lock.clamp(0.0, 1.0);
        if curve >= FULL {
            self.alpha = curve;
        } else if curve < self.alpha {
            self.alpha = curve.max(self.alpha - dt / RELEASE_S);
        }
        if posed.landing && posed.low && !posed.off {
            self.alpha = 1.0;
        }
        let full = self.alpha >= FULL;
        let gone = posed.off;
        // Only a foot the clip has low takes the weight on landing however it swings; one the
        // pose still has up (a fall's foot coming forward) is not locked in the air.
        let swings = posed.swing_mps >= SWING_MPS && !(posed.landing && posed.low);
        let stands = full && !gone && !swings;
        // A new lock starts only once the foot as placed is down (`PLANT_M`); a lock held, let
        // go or stepped is not a new one.
        let lands = stands && self.aloft(&posed) <= PLANT_M;
        let starts = matches!(self.state, State::Free | State::Blending { .. });
        self.why = match () {
            _ if curve < FULL || (stands && (lands || !starts)) => "",
            _ if gone => "off",
            _ if swings => "swing",
            _ if stands => "high",
            _ => "rising",
        };
        if let State::Releasing(lock) = self.state {
            if gone {
                let from = self.shown.unwrap_or(lock);
                self.state = State::Blending { from, s: 0.0 };
            } else if stands {
                // Back at 1 before it was let go: locked again where it is shown.
                self.state = State::Planted(self.shown.unwrap_or(lock));
            } else if self.alpha <= 0.0 {
                self.state = State::Free;
                return clip;
            } else {
                // Across by the lock weight, up as the clip lifts it at once and over an arc as
                // high as half the way it goes (a step's, at most): a foot that went across
                // while still on the ground would slide.
                let across = clip.at.lerp(lock.at, self.alpha);
                let arc = (flat(lock.at, clip.at) * 0.5).min(STEP_UP_M.1)
                    * libm::sinf(core::f32::consts::PI * (1.0 - self.alpha));
                return Pose {
                    at: across + posed.up * ((clip.at - across).dot(posed.up) + arc),
                    turn: clip.turn.slerp(lock.turn, self.alpha),
                };
            }
        }
        if let State::Planted(lock) = self.state {
            let off_by = flat(lock.at, clip.at);
            let turned = yaw_deg(lock.turn, clip.turn, posed.up);
            let settle = self.still_s >= SETTLE_S && off_by > SETTLE_M;
            let urgent = posed.strained || off_by > MOST_M || turned > MOST_TURN_DEG;
            let soft = off_by > REACH_M || turned > TURN_DEG || settle;
            if gone {
                self.state = State::Blending { from: lock, s: 0.0 };
            } else if !full {
                self.state = State::Releasing(lock);
                return self.step(posed, 0.0, other_moving);
            } else if urgent || (soft && !other_moving) {
                self.state = State::Stepping {
                    from: lock,
                    s: 0.0,
                    land: None,
                };
            } else {
                return held(lock, clip, posed.up, posed.ball);
            }
        }
        match self.state {
            State::Free => {
                if lands {
                    self.state = State::Planted(clip);
                }
                clip
            }
            State::Planted(lock) | State::Releasing(lock) => held(lock, clip, posed.up, posed.ball),
            State::Blending { from, s } => {
                let s = s + dt / BLEND_S;
                if s >= 1.0 {
                    self.state = if lands {
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
                if gone || !full {
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

/// A locked foot as shown: its ball of the foot where it was locked over the ground, headed as
/// it was, at the height and with the roll the clip has it, the ankle placed from the ball
/// (`ball`, the ball in the foot's frame): a foot locked as its heel touches flattens and rolls
/// off its toe as the clip does, its ball never moving across, rather than hanging where it
/// touched. The ground itself is foot IK's (movement.md stage 4).
fn held(lock: Pose, clip: Pose, up: Vec3, ball: Vec3) -> Pose {
    let q = lock.turn * clip.turn.inverse();
    let along = up * Vec3::new(q.x, q.y, q.z).dot(up);
    let heading = Quat::from_xyzw(along.x, along.y, along.z, q.w);
    let heading = if heading.length_squared() > 1e-8 {
        heading.normalize()
    } else {
        Quat::IDENTITY
    };
    let turn = (heading * clip.turn).normalize();
    let (locked, clipped) = (lock.at + lock.turn * ball, clip.at + clip.turn * ball);
    let shown = locked + up * (clipped - locked).dot(up);
    Pose {
        at: shown - turn * ball,
        turn,
    }
}

/// How far `to` is turned from `from` about `up`, degrees (the twist of the turn between them).
pub fn yaw_deg(from: Quat, to: Quat, up: Vec3) -> f32 {
    let q = to * from.inverse();
    let twist = 2.0 * libm::atan2f(Vec3::new(q.x, q.y, q.z).dot(up), q.w);
    let wrapped = libm::remainderf(twist, core::f32::consts::TAU);
    wrapped.abs().to_degrees()
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
    fn a_foot_the_clip_lifts_or_swings_low_or_a_body_down_goes_with_the_clip() {
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
            (end - hip).normalize().dot(axis) > 0.9999
                && (knee - hip).normalize().dot(axis) > 0.9999
        );
    }
}
