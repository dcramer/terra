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
/// How long a recovery step takes: a foot caught where a crossfade swept it down (a body
/// stopped dead at a wall, Idle fading in over a stride) or mid-blend, short of the clip's
/// place, steps there at once and quickly, as ALS plays its transition steps faster than
/// their clips; at STEP_S the sprint's stop at the yard's mound settled in 0.32 s, over
/// stop.wall's 0.3.
pub const CAUGHT_STEP_S: f32 = 0.12;
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

/// A foot swung fast is taken as swept over the ground by a crossfade (and locked however fast
/// the pose moves it) only this near where it rests, m (`aloft`): on the yard's film the wall
/// stops' fades into Idle swept feet 0-2.5 cm over their rest, while a stride's touchdown still
/// coming down swung 3.6-4.9 cm over its rest in its last swinging frames (the duel's partner,
/// 2026-10-28), and taken at PLANT_M it locked a frame early, short, and slid as it let go.
pub const SWEPT_M: f32 = 0.03;

/// Whether a sole as placed hangs its ball over its ground (`Posed::over`) past `PLANT_M` more
/// than its heel: a sole straddling a step down's edge on its heel, the ball over the lower
/// tread. Such a foot is not down, however near its heel is: a foot stands on its ball with its
/// heel hanging (a stair climbed on the ball of the foot), never on its heel with its forefoot
/// over a drop, which rolls forward off the edge. A sole over its ground by heel and ball alike
/// (a landing's foot still coming down) is the lock's gate's (`Foot::aloft`), not this.
pub fn hangs(over: [f32; 2]) -> bool {
    over[1] - over[0] > PLANT_M
}

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
    /// The lock was taken where the foot was caught (swept down fast, or mid-blend), not
    /// where its clip came down: its step to the clip is a recovery step (`CAUGHT_STEP_S`).
    caught: bool,
    /// A lock taken this frame from a free or blending foot: the state, rest and catch it
    /// would have had without it, which `refuse` puts back.
    fresh: Option<(State, Option<[f32; 2]>, bool)>,
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
            caught: false,
            fresh: None,
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
        self.fresh = None;
        let shown = self.step(posed, dt, other_moving);
        self.shown = Some(shown);
        if self.planted() && posed.lock >= FULL {
            let rest = self.rest.get_or_insert(posed.sole);
            *rest = [rest[0].min(posed.sole[0]), rest[1].min(posed.sole[1])];
        }
        shown
    }

    /// Where the foot rests (`rest`) when this frame's `update` took a new lock from a free or
    /// blending foot, gated on last frame's ground (`PLANT_M`); else none, and none before its
    /// first lock. Placement checks such a lock against its own probe
    /// (`placement::Foot::fresh`).
    pub fn fresh(&self) -> Option<[f32; 2]> {
        self.fresh.and_then(|(_, rest, _)| rest)
    }

    /// This frame's new lock let go again (placement found the sole aloft under it,
    /// `placement::Grounded::refused`): the foot is free or blending as it would have been, and
    /// is left free as not yet down (`"high"`).
    pub fn refuse(&mut self) {
        if let Some((state, rest, caught)) = self.fresh.take() {
            (self.state, self.rest, self.caught) = (state, rest, caught);
            self.why = "high";
        }
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
        // A new lock starts only once the foot as placed is down (`PLANT_M`); a lock held, let
        // go or stepped is not a new one.
        let aloft = self.aloft(&posed);
        let down = aloft <= PLANT_M && !hangs(posed.over);
        // Only a foot the clip has low takes the weight on landing however it swings; one the
        // pose still has up (a fall's foot coming forward) is not locked in the air. A foot
        // known to be down (where it rests is known) under a full curve stands however fast
        // the pose moves it: that is a crossfade sweeping it over the ground (a stop at a
        // wall, Idle fading in over a stride), and it is locked where it comes down and stepped
        // to the clip by the step rule (ALS's foot lock, its dynamic transition a step), never
        // glided there. Only a foot on its ground (`SWEPT_M`): one still coming down in a stride
        // waits until its swing slows.
        let swept = self.rest.is_some() && aloft <= SWEPT_M;
        let swings = posed.swing_mps >= SWING_MPS && !(posed.landing && posed.low) && !swept;
        let stands = full && !gone && !swings;
        let lands = stands && down;
        // Stood on only because it is swept: caught where it is, not where its clip comes down.
        let caught = swept && posed.swing_mps >= SWING_MPS;
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
                    self.fresh = Some((State::Free, self.rest, self.caught));
                    self.state = State::Planted(clip);
                    self.caught = caught;
                }
                clip
            }
            State::Planted(lock) | State::Releasing(lock) => held(lock, clip, posed.up, posed.ball),
            State::Blending { from, s } => {
                let s = s + dt / BLEND_S;
                if s >= 1.0 {
                    self.state = if lands {
                        self.fresh = Some((State::Free, self.rest, self.caught));
                        State::Planted(clip)
                    } else {
                        State::Free
                    };
                    self.caught = caught;
                    return clip;
                }
                // Up as the clip lifts it at once, across in the blend's time: a foot that
                // went across while still on the ground would slide.
                let across = from.at.lerp(clip.at, ease(s));
                let shown = Pose {
                    at: across + posed.up * (clip.at - across).dot(posed.up),
                    turn: from.turn.slerp(clip.turn, ease(s)),
                };
                // Its clips have it down again and it is down: it stands where it is shown, and
                // the step rule takes it on to the clip (a cut step's blend meeting a stop).
                self.state = if lands {
                    self.fresh = Some((State::Blending { from, s }, self.rest, self.caught));
                    self.caught = true;
                    State::Planted(shown)
                } else {
                    State::Blending { from, s }
                };
                shown
            }
            State::Stepping { from, s, land } => {
                if gone || !full {
                    let from = self.shown.unwrap_or(from);
                    self.state = State::Blending { from, s: 0.0 };
                    return from;
                }
                let took = if self.caught { CAUGHT_STEP_S } else { STEP_S };
                self.stepping(from, s + dt / took, land, &posed)
            }
        }
    }

    /// A step from `from` to land under the clip's foot, `s` of the way, lifted over an arc;
    /// where it comes down once found (`land`).
    fn stepping(&mut self, from: Pose, s: f32, land: Option<Vec3>, posed: &Posed) -> Pose {
        let clip = posed.pose;
        let flat = (from.at - clip.at).reject_from(posed.up).length();
        let up = (flat * 0.5).clamp(STEP_UP_M.0, STEP_UP_M.1);
        let lift = posed.up * up * libm::sinf(core::f32::consts::PI * s.min(1.0));
        // Over the clip's foot by OVER of the way, then down onto where it was then, so the
        // foot lands still rather than gliding in with a driven body.
        let land = if s >= OVER {
            Some(land.unwrap_or(clip.at))
        } else {
            None
        };
        let turn = from.turn.slerp(clip.turn, ease(s / OVER));
        if s >= 1.0 {
            let at = land.unwrap_or(clip.at);
            self.state = State::Planted(Pose { at, turn });
            self.caught = false;
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
mod tests;
