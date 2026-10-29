//! The head kept on the aim (D535): after every clip, hold and fix-up has posed a body, its
//! neck and head are turned about the body's up so the head looks within a glance of the way
//! the body faces (its aim: the foe it is locked on, else the camera's ahead), however far the
//! clip turns the chest. A blow swings the chest and shoulders; the head stays on the target,
//! as a fighter's does (Enshrouded's through a chain, research/2026-10-24-enshrouded-combat-
//! measured.md).
//!
//! It is the look-at shipped engines lay after the clips (research/2026-10-26-head-look-at.md):
//! Godot's `LookAtModifier3D` (MIT; a bone turned toward a target within limit angles, after
//! the animation), Unreal's AnimGraph "Look At" node and Control Rig "Aim" (docs only), Unity's
//! `Animator.SetLookAtWeight` (the turn shared between the body and the head, clamped).
//! Here it is the turn about the up alone (yaw: what a swing throws off; the clip keeps its
//! nod), shared between the neck and the head, the clip's own glance kept up to `FREE`, at most
//! `MOST`, and let go while the head is far from upright (a roll), so a tumble is not wrung.

use bevy::{prelude::*, world_serialization::WorldInstanceReady};

use crate::bend::{Kept, find, in_body};
use crate::character::Body;

/// How far the clip may turn the head off the aim untouched, degrees: a glance (the duel check's
/// bar is 15, `tools/sandbox/duel/pose.ts`).
pub const FREE: f32 = 8.0;
/// The most it is turned back, degrees: a clip that looks over its shoulder on purpose keeps
/// the rest.
pub const MOST: f32 = 70.0;
/// The neck's share of the turn; the head takes the rest (a neck turns about half as far as the
/// head on it, Unity's body and head weights).
pub const NECK: f32 = 0.4;
/// The head's tilt off upright, degrees, over which the turn is let go: all of it to `HELD`,
/// none past `LET_GO`.
const HELD: f32 = 35.0;
const LET_GO: f32 = 60.0;

/// On a body whose model has a neck and a head: those bones, the head's ahead and up in its own
/// frame (from its rest), and what was written over each.
#[derive(Component)]
pub struct Gaze {
    neck: Entity,
    head: Entity,
    ahead: Vec3,
    up: Vec3,
    kept: [Kept; 2],
}

pub fn plugin(app: &mut App) {
    app.add_observer(find_head).add_systems(
        PostUpdate,
        // Last of the pose writers (the clips and hold layer, the fit and lift, the foot lock,
        // the arms off the hips), before propagation and the duel record's read in `Last`
        // (D530): the head is turned on the pose as drawn.
        keep_on_aim
            .after(bevy::app::AnimationSystems)
            .after(crate::Fitted)
            .after(crate::feet::keep)
            .after(crate::arms::keep_off_hips)
            .before(TransformSystems::Propagate),
    );
}

/// When a body's model is in, still at rest: its neck and head, and which way the head looks.
fn find_head(
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
    let Some([neck, head]) = find(root, ["neck_01", "Head"], &children, &names) else {
        return;
    };
    let rest = in_body(head, root, &parents, &transforms)
        .to_scale_rotation_translation()
        .1;
    commands.entity(root).insert(Gaze {
        neck,
        head,
        ahead: rest.inverse() * Vec3::Z,
        up: rest.inverse() * Vec3::Y,
        kept: [Kept::default(); 2],
    });
}

/// How far, degrees, to turn a head whose ahead turns `off` degrees about the up from the aim
/// (+ to the left), tilted `tilt` degrees off upright: back toward the aim past `FREE`, at most
/// `MOST`, less as it tilts.
pub fn turn_back(off: f32, tilt: f32) -> f32 {
    let past = (off.abs() - FREE).clamp(0.0, MOST);
    let held = terra_gen::smoothstep(LET_GO, HELD, tilt);
    -off.signum() * past * held
}

/// Each body's head turned back toward the way it faces.
pub fn keep_on_aim(
    mut bodies: Query<(Entity, &mut Gaze)>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    for (root, mut gaze) in &mut bodies {
        let gaze = &mut *gaze;
        let neck_parent = match parents.get(gaze.neck) {
            Ok(up) => in_body(up.parent(), root, &parents, &transforms),
            Err(_) => continue,
        };
        let neck_parent = neck_parent.to_scale_rotation_translation().1;
        let (Ok(neck_now), Ok(head_now)) = (transforms.get(gaze.neck), transforms.get(gaze.head))
        else {
            continue;
        };
        let neck = gaze.kept[0].clip(neck_now.rotation);
        let head = gaze.kept[1].clip(head_now.rotation);
        // The head's place in the body, through the neck (its parent).
        let head_in_body = neck_parent * neck * head;
        let ahead = head_in_body * gaze.ahead;
        let up = head_in_body * gaze.up;
        if ahead.x.hypot(ahead.z) < 0.2 {
            continue;
        }
        let off = libm::atan2f(ahead.x, ahead.z).to_degrees();
        let tilt = libm::acosf(up.y.clamp(-1.0, 1.0)).to_degrees();
        let by = turn_back(off, tilt).to_radians();
        let neck_turn = Quat::from_rotation_y(by * NECK);
        let head_turn = Quat::from_rotation_y(by * (1.0 - NECK));
        let neck_written = gaze.kept[0].turn(neck, neck_parent, neck_turn);
        let head_written = gaze.kept[1].turn(head, neck_parent * neck_written, head_turn);
        // Written only when it turns: a write marks the skeleton's tree dirty for propagation.
        for (joint, rotation) in [(gaze.neck, neck_written), (gaze.head, head_written)] {
            if let Ok(mut t) = transforms.get_mut(joint)
                && t.rotation != rotation
            {
                t.rotation = rotation;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// A body: its root turned to face its figure's way, a chest under it turned `chest`
    /// degrees about the up, and a neck and head on the chest looking where the chest does.
    fn body(world: &mut World, chest: f32) -> (Entity, Entity, Entity) {
        let root = world
            .spawn(Transform::from_rotation(Quat::from_rotation_y(
                std::f32::consts::PI,
            )))
            .id();
        let chest = world
            .spawn((
                Transform::from_rotation(Quat::from_rotation_y(chest.to_radians())),
                ChildOf(root),
            ))
            .id();
        let neck = world
            .spawn((
                Transform::from_xyz(0.0, 0.3, 0.0).with_rotation(Quat::from_rotation_x(0.2)),
                ChildOf(chest),
            ))
            .id();
        let head = world
            .spawn((
                Transform::from_xyz(0.0, 0.1, 0.0).with_rotation(Quat::from_rotation_x(-0.2)),
                ChildOf(neck),
            ))
            .id();
        world.entity_mut(root).insert(Gaze {
            neck,
            head,
            ahead: Vec3::Z,
            up: Vec3::Y,
            kept: [Kept::default(); 2],
        });
        (root, chest, head)
    }

    /// The head's ahead in the body, and its turn off the body's ahead about the up, degrees.
    fn head_off(world: &mut World, root: Entity, head: Entity) -> f32 {
        let at = world
            .run_system_once(
                move |parents: Query<&ChildOf>, transforms: Query<&mut Transform>| {
                    in_body(head, root, &parents, &transforms)
                },
            )
            .unwrap();
        let ahead = at.to_scale_rotation_translation().1 * Vec3::Z;
        libm::atan2f(ahead.x, ahead.z).to_degrees()
    }

    #[test]
    fn a_head_stays_within_a_glance_of_the_aim_while_the_chest_swings_and_comes_back_with_it() {
        for chest in [-60.0, -41.0, -20.0, 0.0, 5.0, 25.0, 41.0, 60.0_f32] {
            let mut world = World::new();
            let (root, _, head) = body(&mut world, chest);
            // The clip's head turns with the chest.
            assert!((head_off(&mut world, root, head) - chest).abs() < 0.5);
            // Three frames, no clip posing it again: turned once, not three times.
            for _ in 0..3 {
                world.run_system_once(keep_on_aim).unwrap();
            }
            let off = head_off(&mut world, root, head);
            assert!(off.abs() <= FREE + 0.1, "chest {chest}: head {off}");
            // A glance the clip gives is its own.
            if chest.abs() <= FREE {
                assert!((off - chest).abs() < 0.1, "chest {chest}: head {off}");
            }
            // Held where it is, a frame writes neither joint, so the propagation leaves the
            // skeleton alone.
            let changed = |world: &World| {
                world
                    .entity(head)
                    .get_ref::<Transform>()
                    .unwrap()
                    .last_changed()
            };
            let before = changed(&world);
            world.run_system_once(keep_on_aim).unwrap();
            assert_eq!(
                changed(&world),
                before,
                "chest {chest}: an unturned head was written"
            );
        }
    }

    /// The developer's complaint (2026-10-26): "my head turns like 45 degree to the left when
    /// aiming forward and swinging". A blow turns the chest 45° off the aim, each way; the head
    /// stays within the duel check's bar (`film.pose.head_yaw_blows`, 15°) of the aim, frame
    /// after frame as the clip poses it again.
    #[test]
    fn the_head_stays_on_the_aim_while_a_blow_turns_the_chest_45_degrees() {
        const BAR: f32 = 15.0;
        for chest in [45.0, -45.0_f32] {
            let mut world = World::new();
            let (root, chest_bone, head) = body(&mut world, chest);
            let neck = world.get::<ChildOf>(head).unwrap().parent();
            let clip = |world: &mut World| {
                world.get_mut::<Transform>(neck).unwrap().rotation = Quat::from_rotation_x(0.2);
                world.get_mut::<Transform>(head).unwrap().rotation = Quat::from_rotation_x(-0.2);
            };
            for frame in 0..4 {
                clip(&mut world);
                world.run_system_once(keep_on_aim).unwrap();
                let off = head_off(&mut world, root, head);
                assert!(
                    off.abs() <= BAR,
                    "chest {chest}, frame {frame}: head {off}° off the aim"
                );
            }
            // The chest is the clip's: the swing itself is left alone.
            let turned = world.get::<Transform>(chest_bone).unwrap().rotation;
            assert!(turned.angle_between(Quat::from_rotation_y(chest.to_radians())) < 1e-4);
        }
    }

    #[test]
    fn a_tumbling_head_is_let_go() {
        assert_eq!(turn_back(40.0, 90.0), 0.0);
        assert!((turn_back(40.0, 0.0) + (40.0 - FREE)).abs() < 1e-4);
        assert!(turn_back(-40.0, 0.0) > 0.0);
        assert!((turn_back(170.0, 0.0) + MOST).abs() < 1e-4);
    }
}
