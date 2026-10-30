//! The carried arm held in the body's frame (D560): an arm the hold layer lays a carry pose on
//! (a look's `carry_hold`, `figure`'s one-arm hold, `layer::Nodes::over_one`) keeps that pose's
//! turn against the body, not against the chest, so a torch held flame up stays up through a
//! sprint's lean, a crouch's hunch and a jump's landing, where the layer alone, laying the arm
//! in its parent's frame, tipped it up to 83 degrees off with the chest.
//!
//! It is the mesh space rotation blend shipped overlay layers lay an arm with (Unreal's Layered
//! Blend per Bone, "Mesh Space Rotation Blend", which ALS's overlay layers use for the arms;
//! docs only): the shoulder blade's turn in the body is the carry pose's on the chest as it
//! stands at rest, whatever the clip does to the chest, and the shoulder still goes where the
//! chest takes it. Its twin is `tools/characters/carry_arm.ts`, which the move check lays over
//! the carried clips (`moves.ts --carry`); change both.

use bevy::{prelude::*, world_serialization::WorldInstanceReady};

use crate::bend::{Kept, find, in_body};
use crate::character::{Body, CarryPosed, HAND_L, HAND_R};

/// On a body whose model has both arms: each side's (left, right) hand socket, shoulder blade
/// and the chest it hangs from, the chest's turn in the body at rest, and what was written.
#[derive(Component)]
pub struct CarryArms {
    sides: [Side; 2],
}

struct Side {
    socket: Entity,
    clavicle: Entity,
    chest: Entity,
    rest: Quat,
    kept: Kept,
}

pub fn plugin(app: &mut App) {
    app.add_observer(find_arms).add_systems(
        PostUpdate,
        // On the pose the clips and holds gave, before the fit and every fix-up after it (the
        // fists, the arms kept off the hips, the shield arm, the head), which read it as held.
        hold_in_body
            .in_set(crate::pose::Layer::CarryArm)
            .after(bevy::app::AnimationSystems)
            .before(crate::Fitted),
    );
}

/// When a body's model is in, still at rest: each arm's bones and its chest's turn.
fn find_arms(
    ready: On<WorldInstanceReady>,
    bodies: Query<(), With<Body>>,
    children: Query<&Children>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    transforms: Query<&mut Transform>,
    mut commands: Commands,
) {
    let root = ready.entity;
    if !bodies.contains(root) {
        return;
    }
    let Some(bones) = find(
        root,
        [HAND_L, "clavicle_l", HAND_R, "clavicle_r"],
        &children,
        &names,
    ) else {
        return;
    };
    let side = |socket: Entity, clavicle: Entity| {
        let chest = parents.get(clavicle).ok()?.parent();
        let rest = in_body(chest, root, &parents, &transforms)
            .to_scale_rotation_translation()
            .1;
        Some(Side {
            socket,
            clavicle,
            chest,
            rest,
            kept: Kept::default(),
        })
    };
    let (Some(left), Some(right)) = (side(bones[0], bones[1]), side(bones[2], bones[3])) else {
        return;
    };
    commands.entity(root).insert(CarryArms {
        sides: [left, right],
    });
}

/// Each arm in a carry pose turned at its shoulder blade by the chest's turn from its rest,
/// undone.
pub fn hold_in_body(
    mut bodies: Query<(Entity, &mut CarryArms)>,
    posed: Query<&CarryPosed>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    for (root, mut arms) in &mut bodies {
        for side in &mut arms.sides {
            if !posed.get(side.socket).is_ok_and(|p| p.0) {
                continue;
            }
            let chest = in_body(side.chest, root, &parents, &transforms)
                .to_scale_rotation_translation()
                .1;
            let Ok(now) = transforms.get(side.clavicle).map(|t| t.rotation) else {
                continue;
            };
            let clip = side.kept.clip(now);
            let written = side.kept.turn(clip, chest, side.rest * chest.inverse());
            if let Ok(mut t) = transforms.get_mut(side.clavicle) {
                t.rotation = written;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// A body with a chest leaning `lean` degrees forward from its rest, a shoulder blade on it
    /// as the carry pose turns it, and a hand socket below, carrying in a pose or not.
    fn body(world: &mut World, lean: f32, carried: bool) -> (Entity, Entity) {
        let root = world
            .spawn(Transform::from_rotation(Quat::from_rotation_y(
                std::f32::consts::PI,
            )))
            .id();
        let rest = Quat::from_rotation_x(0.1);
        let chest = world
            .spawn((
                Transform::from_xyz(0.0, 1.3, 0.0)
                    .with_rotation(Quat::from_rotation_x(lean.to_radians()) * rest),
                ChildOf(root),
            ))
            .id();
        let clavicle = world
            .spawn((
                Transform::from_xyz(-0.05, 0.2, 0.0).with_rotation(Quat::from_rotation_z(1.2)),
                ChildOf(chest),
            ))
            .id();
        let socket = world
            .spawn((
                Transform::from_xyz(0.0, 0.6, 0.0),
                ChildOf(clavicle),
                CarryPosed(carried),
            ))
            .id();
        let side = |socket| Side {
            socket,
            clavicle,
            chest,
            rest,
            kept: Kept::default(),
        };
        let other = world.spawn_empty().id();
        world.entity_mut(root).insert(CarryArms {
            sides: [side(other), side(socket)],
        });
        (root, clavicle)
    }

    fn turn_in_body(world: &mut World, root: Entity, bone: Entity) -> Quat {
        world
            .run_system_once(
                move |parents: Query<&ChildOf>, transforms: Query<&mut Transform>| {
                    in_body(bone, root, &parents, &transforms)
                },
            )
            .unwrap()
            .to_scale_rotation_translation()
            .1
    }

    /// The torch's complaint the layer alone had (D560): the chest leans, a crouch's 80 degrees
    /// or a sprint's 30, and the carried arm keeps its turn in the body, frame after frame.
    #[test]
    fn a_carried_arm_keeps_its_turn_in_the_body_however_the_chest_leans() {
        let mut upright = World::new();
        let (root, clavicle) = body(&mut upright, 0.0, true);
        upright.run_system_once(hold_in_body).unwrap();
        let held = turn_in_body(&mut upright, root, clavicle);
        for lean in [-20.0, 30.0, 60.0, 80.0_f32] {
            let mut world = World::new();
            let (root, clavicle) = body(&mut world, lean, true);
            for _ in 0..3 {
                world.run_system_once(hold_in_body).unwrap();
            }
            let now = turn_in_body(&mut world, root, clavicle);
            let off = now.angle_between(held).to_degrees();
            assert!(
                off < 0.1,
                "lean {lean}: the arm {off} degrees off its carried turn"
            );
        }
    }

    /// An arm not in a carry pose (a stance's hold, a used look) leans with the chest.
    #[test]
    fn an_arm_not_carried_in_a_pose_is_left_as_the_clip_has_it() {
        let mut world = World::new();
        let (root, clavicle) = body(&mut world, 40.0, false);
        let before = turn_in_body(&mut world, root, clavicle);
        world.run_system_once(hold_in_body).unwrap();
        let after = turn_in_body(&mut world, root, clavicle);
        assert_eq!(after, before);
    }
}
