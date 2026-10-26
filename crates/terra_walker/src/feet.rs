//! Feet kept where they land (`terra_motion::feet`, D455), for every body with legs, whatever
//! it does or holds: after the clip has posed the skeleton and before the pose is carried to
//! the joints' world places, each leg the clip has on the ground is bent so its foot stays
//! where it was locked while the body moves over it (a stride, a move's drive or warp, a turn
//! to the foe), and steps when it has gone too far. A stride's clip plays in place at its pace
//! (`pace.rs`), so its feet are read against the ground it stands on (`Kept::ground`, which the
//! figure sets); a clip that stands is read in the body's frame. Which feet a clip has down is
//! its own data, the foot-down markers baked into its file (`markers`, `terra_motion::feet::
//! Markers`), read at the time each clip playing is at and weighed by its weight. Only the
//! thigh, calf and foot are turned, so nothing else that writes the joints after the animation
//! (the body's proportions, its lift) is disturbed.

use std::collections::HashMap;

use bevy::{
    gltf::{Gltf, GltfNode},
    prelude::*,
    transform::TransformSystems,
};
use terra_motion::feet::{self, Foot, Markers, Pose, Posed};

use crate::{Downed, Motion, character};

/// Every clip's foot-down markers, by the clip loaded: whoever loads a body's clips registers
/// them (`Baked::register`), so a foot is kept by the clip it plays whichever file it is from.
#[derive(Resource, Default)]
pub struct Baked(HashMap<AssetId<AnimationClip>, Markers>);

impl Baked {
    /// The markers a body file carries (`markers`), for each of its clips as loaded.
    pub fn register(&mut self, gltf: &Gltf, nodes: &Assets<GltfNode>) {
        let markers = markers(gltf, nodes);
        for (name, clip) in &gltf.named_animations {
            match markers.get(name.as_ref()) {
                Some(each) => {
                    self.0.insert(clip.id(), each.clone());
                }
                None => {
                    warn!("The clip {name} has no foot-down markers (tools/characters/markers.ts).")
                }
            }
        }
    }
}

/// The foot-down markers kept in a body file, by clip (`tools/characters/markers.ts`):
/// `{"markers": {"Jog_Fwd_Loop": {...}}}` in its glTF extras, beside the paces.
pub fn markers(gltf: &Gltf, nodes: &Assets<GltfNode>) -> HashMap<String, Markers> {
    gltf.nodes
        .iter()
        .filter_map(|node| nodes.get(node)?.extras.as_ref())
        .find_map(|extras| {
            let value: serde_json::Value = serde_json::from_str(&extras.value).ok()?;
            serde_json::from_value(value.get("markers")?.clone()).ok()
        })
        .unwrap_or_default()
}

/// What share of the weight playing on `animator` has each foot down by its clips' markers, at
/// the time each is at; a clip with none is left out.
fn down(animator: &AnimationPlayer, graph: &AnimationGraph, baked: &Baked) -> [f32; 2] {
    let (mut down, mut whole) = ([0.0; 2], 0.0);
    for (node, active) in animator.playing_animations() {
        let Some(AnimationNodeType::Clip(clip)) = graph.get(*node).map(|n| &n.node_type) else {
            continue;
        };
        let Some(markers) = baked.0.get(&clip.id()) else {
            continue;
        };
        let weight = active.weight();
        if weight <= 0.0 {
            continue;
        }
        whole += weight;
        for (side, share) in down.iter_mut().enumerate() {
            if markers.down(side, active.seek_time()) {
                *share += weight;
            }
        }
    }
    if whole > 0.0 {
        down.map(|share| share / whole)
    } else {
        [0.0; 2]
    }
}

/// Each leg's hip, knee and ankle, left then right.
const LEGS: [[&str; 3]; 2] = [
    ["thigh_l", "calf_l", "foot_l"],
    ["thigh_r", "calf_r", "foot_r"],
];

/// A body's two feet as kept, and its legs' joints once found.
#[derive(Component, Default)]
pub struct Kept {
    feet: [Foot; 2],
    strained: [bool; 2],
    /// Each clip foot's place in the body's own frame last frame, for how fast it swings.
    was: [Option<Vec3>; 2],
    legs: Option<[[Entity; 3]; 2]>,
    /// The animation player posing its skeleton, once found.
    animator: Option<Entity>,
    /// How fast the ground the clip's feet stand on passes under the body, in the world, m/s:
    /// the body's velocity over the ground while a clip that travels plays at its pace, zero
    /// for a clip that stands. Whoever plays the clip sets it (`figure`).
    pub ground: Vec3,
    /// A layer above lets the feet go this frame (a fighter down, a roll): they go as the clip
    /// has them. Set each frame by whoever knows.
    pub let_go: bool,
    /// A layer above plays its own clip set this frame (a fighter's move set): its feet are
    /// read in the body's own frame whatever it plays, its walk too, as a stance's are (D455).
    /// Else a stride's feet are read on the ground it passes. Set each frame by whoever knows,
    /// like `let_go`: both last the frame.
    pub stance: bool,
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
    app.init_resource::<Baked>().add_systems(
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

#[allow(clippy::too_many_arguments)]
fn keep(
    time: Res<Time>,
    baked: Res<Baked>,
    graphs: Res<Assets<AnimationGraph>>,
    animators: Query<(&AnimationPlayer, &AnimationGraphHandle)>,
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
        if kept.animator.is_none() {
            kept.animator = children
                .iter_descendants(body)
                .find(|&each| animators.contains(each));
        }
        // Which feet its clips have down, by their markers.
        let Some(clips_down) = kept
            .animator
            .and_then(|each| animators.get(each).ok())
            .and_then(|(animator, graph)| Some(down(animator, graphs.get(graph)?, &baked)))
        else {
            continue;
        };
        // What a layer above set this frame lasts the frame: a body no longer a fighter's
        // reads a stride's feet on the ground it passes again.
        let let_go = std::mem::take(&mut kept.let_go);
        let stance = std::mem::take(&mut kept.stance);
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
        let passing = (!stance).then(|| into_body.transform_vector3(kept.ground));
        for (side, &[thigh, calf, foot]) in legs.iter().enumerate() {
            let Some(hips) = parents
                .get(thigh)
                .ok()
                .and_then(|p| world(p.parent(), &transforms, &parents))
            else {
                continue;
            };
            let (Ok(thigh_local), Ok(calf_local), Ok(foot_local)) = (
                transforms.get(thigh),
                transforms.get(calf),
                transforms.get(foot),
            ) else {
                continue;
            };
            let at_thigh = hips * *thigh_local;
            let at_calf = at_thigh * *calf_local;
            let at_foot = at_calf * *foot_local;
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
                down: clips_down[side],
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The body files whose clips the game plays: every clip carries its foot-down markers
    /// (`tools/characters/markers.ts`; a re-export without them leaves every foot free), and a
    /// stride has each foot down for part of its cycle, not all of it.
    #[test]
    fn every_clip_played_carries_its_foot_down_markers() {
        for file in [crate::appearance::BASE, crate::figure::CLIPS] {
            let json = crate::model_files::json(file);
            let markers: HashMap<String, Markers> = serde_json::from_value(
                crate::model_files::extra(&json, "markers")
                    .unwrap_or_else(|| panic!("{file} carries no markers")),
            )
            .unwrap();
            let paces: HashMap<String, f32> =
                serde_json::from_value(crate::model_files::extra(&json, "paces").unwrap()).unwrap();
            for clip in crate::model_files::animations(&json) {
                let each = markers
                    .get(&clip)
                    .unwrap_or_else(|| panic!("{file}: {clip} has no markers"));
                if !paces.contains_key(&clip) {
                    continue;
                }
                for side in 0..2 {
                    let down = (0..100)
                        .filter(|i| each.down(side, each.length_s * *i as f32 / 100.0))
                        .count();
                    assert!(
                        (5..95).contains(&down),
                        "{file}: {clip}'s foot {side} is down {down}% of its stride"
                    );
                }
            }
        }
    }
}
