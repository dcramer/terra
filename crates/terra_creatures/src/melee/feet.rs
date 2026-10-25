//! A fighter's feet kept where they land (`terra_motion::feet`, D455): after the clip has
//! posed the skeleton and before the pose is carried to the joints' world places, each leg the
//! clip has on the ground is bent so its foot stays where it was locked while the move's drive,
//! its warp or a turn to the foe moves the body over it, and steps when it has gone too far.
//! Only the thigh, calf and foot are turned, so nothing else that writes the joints after the
//! animation (the body's proportions, its lift) is disturbed.

use bevy::{prelude::*, transform::TransformSystems};
use terra_motion::feet::{self, Foot, Pose, Posed};
use terra_walker::character;

use super::Fighter;

/// Each leg's hip, knee, ankle and ball of the foot, left then right.
const LEGS: [[&str; 4]; 2] = [
    ["thigh_l", "calf_l", "foot_l", "ball_l"],
    ["thigh_r", "calf_r", "foot_r", "ball_r"],
];

/// A fighter's two feet as kept, and its legs' joints once found.
#[derive(Component, Default)]
pub struct Kept {
    feet: [Foot; 2],
    strained: [bool; 2],
    /// Each clip foot's place in the body's own frame last frame, for how fast it swings.
    was: [Option<Vec3>; 2],
    legs: Option<[[Entity; 4]; 2]>,
}

impl Kept {
    /// How each foot is kept, left then right, as the fight log writes it.
    pub fn states(&self) -> [&'static str; 2] {
        self.feet.map(|foot| foot.state())
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        keep.after(bevy::app::AnimationSystems)
            .after(terra_walker::Fitted)
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
    set: Res<super::Set>,
    mut fighters: Query<(Entity, &Fighter, &mut Kept)>,
    children: Query<&Children>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    let dt = time.delta_secs();
    for (body, fighter, mut kept) in &mut fighters {
        if kept.legs.is_none() {
            let found =
                LEGS.map(|leg| leg.map(|name| character::find(body, name, &children, &names)));
            if found.iter().flatten().any(Option::is_none) {
                continue;
            }
            kept.legs = Some(found.map(|leg| leg.map(Option::unwrap)));
        }
        let Some(legs) = kept.legs else { continue };
        let Some(base) = world(body, &transforms, &parents) else {
            continue;
        };
        let ground = base.translation();
        let up = ground.normalize_or(Vec3::Y);
        // Down, or in a move that drives the body faster than feet step (a roll).
        let driven = fighter
            .runner
            .now(&set.0)
            .is_some_and(|now| now.drive.iter().any(|&(_, mps)| mps > feet::DRIVEN_MPS));
        let off = fighter.down_s.is_some() || driven;
        let into_body = base.affine().inverse();
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
            let swing_mps = match kept.was[side] {
                Some(was) if dt > 0.0 => (in_body - was).length() / dt,
                _ => 0.0,
            };
            kept.was[side] = Some(in_body);
            let posed = Posed {
                pose: clip,
                swing_mps,
                ball_up: (at_ball.translation() - ground).dot(up),
                up,
                strained: kept.strained[side],
                off,
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
