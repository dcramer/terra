//! Feet kept where they land (`terra_motion::feet`, D455), for every body with legs, whatever
//! it does or holds: after the clip has posed the skeleton and before the pose is carried to
//! the joints' world places, each leg the clip has on the ground is bent so its foot stays
//! where it was locked while the body moves over it (a stride, a move's drive or warp, a turn
//! to the foe), and steps when it has gone too far. A stride's clip plays in place at its pace
//! (`pace.rs`), so its feet are read against the ground it stands on (`Kept::ground`, which the
//! figure sets); a clip that stands is read in the body's frame. Only the thigh, calf and foot
//! are turned, so nothing else that writes the joints after the animation (the body's
//! proportions, its lift) is disturbed.

use bevy::{prelude::*, transform::TransformSystems};
use terra_motion::feet::{self, Foot, Pose, Posed};

use crate::{Downed, Motion, character};

/// Each leg's hip, knee, ankle and ball of the foot, left then right.
const LEGS: [[&str; 4]; 2] = [
    ["thigh_l", "calf_l", "foot_l", "ball_l"],
    ["thigh_r", "calf_r", "foot_r", "ball_r"],
];

/// A body's two feet as kept, and its legs' joints once found.
#[derive(Component, Default)]
pub struct Kept {
    feet: [Foot; 2],
    strained: [bool; 2],
    /// Each clip foot's place in the body's own frame last frame, for how fast it swings.
    was: [Option<Vec3>; 2],
    legs: Option<[[Entity; 4]; 2]>,
    /// How fast the ground the clip's feet stand on passes under the body, in the world, m/s:
    /// the body's velocity over the ground while a clip that travels plays at its pace, zero
    /// for a clip that stands. Whoever plays the clip sets it (`figure`).
    pub ground: Vec3,
    /// A layer above lets the feet go this frame (a fighter down, a roll): they go as the clip
    /// has them. Set each frame by whoever knows.
    pub let_go: bool,
    /// A layer above plays its own clip set this frame (a fighter's move set): how high its
    /// clips stand a planted foot, and its feet are read in the body's own frame whatever it
    /// plays, its walk too, as a stance's are (D455). Else the body's kind's heights
    /// (`movers.ron`) and a stride's feet read on the ground it passes. Set each frame by
    /// whoever knows, like `let_go`: both last the frame.
    pub stance: Option<feet::Plant>,
    /// Off the ground last frame, so the frame it comes down is known.
    was_off: bool,
}

impl Kept {
    /// The clip playing travels `pace` metres a second (scaled to the body) at `rate`, `way`:
    /// the ground its planted feet stand on passes under the body that fast (none for a clip
    /// that stands, of no pace).
    pub fn stride(&mut self, way: Vec3, pace: f32, rate: f32) {
        self.ground = way.normalize_or_zero() * pace * rate.abs();
    }

    /// How each foot is kept, left then right, as the fight log writes it.
    pub fn states(&self) -> [&'static str; 2] {
        self.feet.map(|foot| foot.state())
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        keep.after(bevy::app::AnimationSystems)
            .after(crate::Fitted)
            .before(TransformSystems::Propagate),
    );
}

/// An entity's place in the world from its own and its ancestors' transforms as they are now,
/// before this frame's propagation.
fn world(
    entity: Entity,
    transforms: &Query<&mut Transform>,
    parents: &Query<&ChildOf>,
) -> Option<GlobalTransform> {
    let mut place = GlobalTransform::from(*transforms.get(entity).ok()?);
    for each in parents.iter_ancestors(entity) {
        place = *transforms.get(each).ok()? * place;
    }
    Some(place)
}

fn keep(
    time: Res<Time>,
    movers: Option<Res<crate::movement::Movers>>,
    mut bodies: Query<(Entity, &mut Kept, Option<&Motion>, Has<Downed>)>,
    children: Query<&Children>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    let dt = time.delta_secs();
    for (body, mut kept, motion, downed) in &mut bodies {
        if kept.legs.is_none() {
            let found =
                LEGS.map(|leg| leg.map(|name| character::find(body, name, &children, &names)));
            if found.iter().flatten().any(Option::is_none) {
                continue;
            }
            kept.legs = Some(found.map(|leg| leg.map(Option::unwrap)));
        }
        let Some(legs) = kept.legs else { continue };
        // What a layer above set this frame lasts the frame: a body no longer a fighter's
        // goes back to its kind's.
        let (let_go, stance) = (std::mem::take(&mut kept.let_go), kept.stance.take());
        let Some(plant) = stance.or_else(|| movers.as_ref().map(|movers| movers.kind().plant))
        else {
            continue;
        };
        let Some(base) = world(body, &transforms, &parents) else {
            continue;
        };
        let ground = base.translation();
        let up = ground.normalize_or(Vec3::Y);
        // Down, off the ground (a jump, a fall, swimming), or let go by a layer above.
        let aloft = motion.is_some_and(|motion| !motion.on_ground);
        let off = let_go || downed || aloft;
        let landing = kept.was_off && !aloft;
        kept.was_off = aloft;
        let into_body = base.affine().inverse();
        let passing = stance
            .is_none()
            .then(|| into_body.transform_vector3(kept.ground));
        for (side, &[thigh, calf, foot, ball]) in legs.iter().enumerate() {
            let Some(hips) = parents
                .get(thigh)
                .ok()
                .and_then(|p| world(p.parent(), &transforms, &parents))
            else {
                continue;
            };
            let (Ok(thigh_local), Ok(calf_local), Ok(foot_local), Ok(ball_local)) = (
                transforms.get(thigh),
                transforms.get(calf),
                transforms.get(foot),
                transforms.get(ball),
            ) else {
                continue;
            };
            let at_thigh = hips * *thigh_local;
            let at_calf = at_thigh * *calf_local;
            let at_foot = at_calf * *foot_local;
            let at_ball = at_foot * *ball_local;
            let (hip, knee, ankle) = (
                at_thigh.translation(),
                at_calf.translation(),
                at_foot.translation(),
            );
            let clip = Pose {
                at: ankle,
                turn: at_foot.rotation(),
            };
            let in_body = into_body.transform_point3(ankle);
            let swing_mps = kept.was[side].map_or(0.0, |was| match passing {
                Some(passing) => feet::swing_mps(was, in_body, passing, Vec3::Y, dt),
                None if dt > 0.0 => (in_body - was).length() / dt,
                None => 0.0,
            });
            kept.was[side] = Some(in_body);
            let posed = Posed {
                pose: clip,
                swing_mps,
                ball_up: (at_ball.translation() - ground).dot(up),
                plant,
                up,
                strained: kept.strained[side],
                off,
                landing,
            };
            let other = kept.feet[1 - side].moving();
            let kept_at = kept.feet[side].update(posed, dt, other);
            if kept_at == clip {
                kept.strained[side] = false;
                continue;
            }
            let (upper, lower) = ((knee - hip).length(), (ankle - knee).length());
            let ahead = *base.forward();
            let pole = knee + (knee - (hip + ankle) * 0.5).normalize_or(ahead) * 0.5;
            let (to_knee, to_ankle) = feet::two_bone(hip, upper, lower, kept_at.at, pole);
            kept.strained[side] = (to_ankle - kept_at.at).length() > 0.01;
            let turn_thigh =
                Quat::from_rotation_arc((knee - hip).normalize(), (to_knee - hip).normalize());
            let swung = to_knee + turn_thigh * (ankle - knee);
            let turn_calf = Quat::from_rotation_arc(
                (swung - to_knee).normalize(),
                (to_ankle - to_knee).normalize(),
            );
            let thigh_turn = turn_thigh * at_thigh.rotation();
            let calf_turn = turn_calf * turn_thigh * at_calf.rotation();
            let set = |entity: Entity, rotation: Quat, transforms: &mut Query<&mut Transform>| {
                if let Ok(mut transform) = transforms.get_mut(entity) {
                    transform.rotation = rotation.normalize();
                }
            };
            set(
                thigh,
                hips.rotation().inverse() * thigh_turn,
                &mut transforms,
            );
            set(calf, thigh_turn.inverse() * calf_turn, &mut transforms);
            set(foot, calf_turn.inverse() * kept_at.turn, &mut transforms);
        }
    }
}
