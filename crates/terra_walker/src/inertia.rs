//! Inertialization: a figure taking up a clip at once, its whole weight from the first frame,
//! with the gap between the pose it showed and the clip's carried as an offset per bone that
//! decays to nothing (D631). A crossfade's weight ramps by a straight line, so at its first
//! and last frames each joint's angular velocity jumps by the pose gap over the blend time
//! (the roll's sword arm 13.6 times its clips' own worst, measured on the animation course);
//! the offset here starts at the gap moving as the old pose moved and eases out by a quintic,
//! so the pose and its velocity are continuous, and the clip's own motion shows whole from its
//! first frame rather than faded in.
//!
//! Ported from David Bollo, "Inertialization: High-Performance Animation Transitions in Gears
//! of War" (GDC 2018): the offset's axis fixed at the start, its angle (or a translation's
//! length) along it a quintic in time from the offset and its velocity to rest, the velocity
//! clamped to one that closes the gap and the time shortened so it does not overshoot.
//! Unreal's Inertialization anim node is the same method. Runs on the clips' output, after the
//! animation is evaluated and before the pose layers (feet, lift, arms, ...) change the body,
//! so they work on the inertialized pose as on any clip's.

use std::{collections::HashMap, time::Duration};

use bevy::{animation::AnimatedBy, prelude::*};

/// Below this a gap is no gap: rad or m.
const NONE: f32 = 1e-5;

/// On a figure's animation player: the pose its clips put out the last two frames, and the
/// offsets decaying since the clip it took up at once (`start`).
#[derive(Component, Default)]
pub struct Inertia {
    /// Each bone's output (rotation, translation) last frame and the frame before.
    seen: HashMap<Entity, [(Quat, Vec3); 2]>,
    /// Asked to start, over this time, s: the offsets are taken at the next evaluation.
    asked: Option<f32>,
    /// Taking the offsets this frame, to decay over this time, s.
    taking: Option<f32>,
    /// Each bone's offset and how long since it was taken, s.
    offsets: HashMap<Entity, Offset>,
    t: f32,
    /// Whether the clip playing is an act's, so the clip after it is taken up at once too.
    acting: bool,
}

impl Inertia {
    /// The clip played now is inertialized from what showed, over `over`: the crossfade it
    /// takes the place of, none, returned for the transitions' play.
    pub fn start(&mut self, over: Duration) -> Duration {
        self.asked = Some(over.as_secs_f32());
        Duration::ZERO
    }

    /// The crossfade of `over` a clip is played with: none, inertialized instead, for an act
    /// (`act`) and for whatever follows one, as an act's end is a gap as large as its start
    /// (Low_Guard to Idle_Loop popped 2.0 times its clips' worst as a crossfade); a stride
    /// loop after a stride loop crossfades, kept in step by its markers.
    pub fn play(&mut self, act: bool, over: Duration) -> Duration {
        let after_act = std::mem::replace(&mut self.acting, act);
        if act || after_act {
            self.start(over)
        } else {
            over
        }
    }
}

/// A bone's offset from its clip: a rotation about `axis` and a translation along `along`,
/// each its size in time.
struct Offset {
    axis: Vec3,
    turn: Quintic,
    along: Vec3,
    shift: Quintic,
}

/// A size decaying from `x0` (at least 0) moving at `v0` to rest at `t1`, by Bollo's quintic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quintic {
    c: [f32; 6],
    t1: f32,
}

impl Quintic {
    pub fn new(x0: f32, v0: f32, t1: f32) -> Self {
        if x0 <= NONE || t1 <= 0.0 {
            return Self {
                c: [0.0; 6],
                t1: 0.0,
            };
        }
        // Moving away from the clip it is taken as still; moving toward it, the time is
        // shortened so it comes to the clip at rest rather than past it.
        let v0 = v0.min(0.0);
        let t1 = if v0 < 0.0 { t1.min(-5.0 * x0 / v0) } else { t1 };
        let (t2, t3) = (t1 * t1, t1 * t1 * t1);
        let a0 = ((-8.0 * v0 * t1 - 20.0 * x0) / t2).max(0.0);
        let a = -(a0 * t2 + 6.0 * v0 * t1 + 12.0 * x0) / (2.0 * t3 * t2);
        let b = (3.0 * a0 * t2 + 16.0 * v0 * t1 + 30.0 * x0) / (2.0 * t2 * t2);
        let c = -(3.0 * a0 * t2 + 12.0 * v0 * t1 + 20.0 * x0) / (2.0 * t3);
        Self {
            c: [a, b, c, a0 / 2.0, v0, x0],
            t1,
        }
    }

    /// Its size at `t`: none once over.
    pub fn at(&self, t: f32) -> f32 {
        if t >= self.t1 {
            return 0.0;
        }
        self.c.iter().fold(0.0, |sum, k| sum * t + k)
    }
}

/// Each figure's time since its start goes on, and a start asked is taken now.
fn advance(time: Res<Time>, mut figures: Query<&mut Inertia>) {
    let dt = time.delta_secs();
    for mut inertia in &mut figures {
        inertia.taking = inertia.asked.take();
        if inertia.taking.is_some() {
            inertia.offsets.clear();
            inertia.t = 0.0;
        } else {
            inertia.t += dt;
        }
    }
}

/// Each bone's pose the clips put out, offset as its figure's start left it, and kept for the
/// next start.
fn inertialize(
    time: Res<Time>,
    mut figures: Query<&mut Inertia>,
    mut bones: Query<(Entity, &AnimatedBy, &mut Transform)>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (bone, by, mut place) in &mut bones {
        let Ok(mut inertia) = figures.get_mut(by.0) else {
            continue;
        };
        let clip = (place.rotation, place.translation);
        if let Some(over) = inertia.taking
            && let Some(seen) = inertia.seen.get(&bone)
        {
            let offset = offset(*seen, clip, dt, over);
            inertia.offsets.insert(bone, offset);
        }
        let t = inertia.t;
        if let Some(offset) = inertia.offsets.get(&bone) {
            place.rotation = Quat::from_axis_angle(offset.axis, offset.turn.at(t)) * clip.0;
            place.translation = clip.1 + offset.along * offset.shift.at(t);
        }
        let out = (place.rotation, place.translation);
        let last = inertia.seen.get(&bone).map_or(out, |seen| seen[0]);
        inertia.seen.insert(bone, [out, last]);
    }
}

/// The offset of what showed (`seen`: last frame and the one before) from the clip's pose
/// now, the shown pose carried on a frame at its own velocity, decaying over `over` s.
fn offset(seen: [(Quat, Vec3); 2], clip: (Quat, Vec3), dt: f32, over: f32) -> Offset {
    let [(last, at), (before, was)] = seen;
    let mut step = last * before.inverse();
    if step.w < 0.0 {
        step = -step;
    }
    let shown = (step * last).normalize();
    let mut now = (shown * clip.0.inverse()).normalize();
    if now.w < 0.0 {
        now = -now;
    }
    let mut then = (last * clip.0.inverse()).normalize();
    if then.dot(now) < 0.0 {
        then = -then;
    }
    let (axis, angle) = now.to_axis_angle();
    let (axis, turn) = if angle > NONE && axis.is_finite() {
        let was = 2.0 * libm::atan2f(then.xyz().dot(axis), then.w);
        (axis, Quintic::new(angle, (angle - was) / dt, over))
    } else {
        (Vec3::X, Quintic::new(0.0, 0.0, over))
    };
    let gap = at + (at - was) - clip.1;
    let length = gap.length();
    let (along, shift) = if length > NONE {
        let along = gap / length;
        let was = (at - clip.1).dot(along);
        (along, Quintic::new(length, (length - was) / dt, over))
    } else {
        (Vec3::X, Quintic::new(0.0, 0.0, over))
    };
    Offset {
        axis,
        turn,
        along,
        shift,
    }
}

/// After the clips are evaluated, before any pose layer.
pub fn plugin(app: &mut App) {
    let mut set = (advance, inertialize)
        .chain()
        .after(bevy::animation::animate_targets);
    for layer in crate::pose::Layer::ALL {
        set = set.before(layer);
    }
    app.add_systems(PostUpdate, set);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quintic_starts_at_the_gap_and_its_velocity_and_comes_to_rest_at_its_end() {
        let dt = 1e-3;
        let q = Quintic::new(1.0, -2.0, 0.5);
        assert!((q.at(0.0) - 1.0).abs() < 1e-6);
        assert!(((q.at(dt) - q.at(0.0)) / dt + 2.0).abs() < 0.01);
        let end = q.t1;
        assert!(q.at(end - 1e-4).abs() < 1e-4);
        assert!(((q.at(end - dt) - q.at(end - 2.0 * dt)) / dt).abs() < 0.01);
        assert_eq!(q.at(end + 0.1), 0.0);
    }

    #[test]
    fn an_act_and_the_clip_after_it_are_inertialized_and_a_stride_change_crossfades() {
        let over = Duration::from_millis(200);
        let mut inertia = Inertia::default();
        assert_eq!(inertia.play(false, over), over, "jog to sprint crossfades");
        assert_eq!(inertia.play(true, over), Duration::ZERO, "into the act");
        assert_eq!(inertia.asked, Some(0.2));
        inertia.asked = None;
        assert_eq!(inertia.play(false, over), Duration::ZERO, "out of the act");
        assert_eq!(inertia.asked, Some(0.2));
        assert_eq!(inertia.play(false, over), over, "and on, crossfading again");
    }

    #[test]
    fn a_quintic_never_passes_its_clip() {
        for v0 in [0.0, -1.0, -5.0, -40.0, 3.0] {
            let q = Quintic::new(0.3, v0, 0.3);
            for i in 0..400 {
                assert!(q.at(i as f32 * 1e-3) >= -1e-5, "v0 {v0}: below at {i} ms");
            }
        }
    }

    #[test]
    fn an_offset_carries_the_shown_pose_on_at_its_velocity() {
        // Shown turning 0.1 rad a frame about y; the clip is still, 1 rad on.
        let turn = |a: f32| Quat::from_rotation_y(a);
        let seen = [(turn(0.2), Vec3::ZERO), (turn(0.1), Vec3::ZERO)];
        let o = offset(seen, (turn(1.0), Vec3::ZERO), 1.0 / 60.0, 0.3);
        let at = |t: f32| {
            (Quat::from_axis_angle(o.axis, o.turn.at(t)) * turn(1.0))
                .to_euler(EulerRot::YXZ)
                .0
        };
        assert!(
            (at(0.0) - 0.3).abs() < 1e-4,
            "starts a frame on: {}",
            at(0.0)
        );
        let v = (at(1e-3) - at(0.0)) / 1e-3;
        assert!((v - 6.0).abs() < 0.1, "at the shown velocity: {v}");
        assert!((at(0.3) - 1.0).abs() < 1e-4, "ends on the clip");
    }
}
