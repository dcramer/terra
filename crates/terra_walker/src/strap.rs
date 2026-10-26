//! A held look strapped to the forearm rather than hung on the hand socket: a shield. Its file
//! says where, in its glTF extras, `{"strap": {"bone": "lowerarm_l", "at": [x, y, z], "face":
//! [x, y, z], "grip": [x, y, z]}}` (`tools/characters/fit_held.py`): the bone it rides on, where
//! its origin (its grip) sits in the bone's frame in metres, and the directions in that frame
//! its front (-Z) and its grip (+Y) turn to. It is held as anything is (`character::hold`) and
//! moved onto the bone once it has loaded, so it follows the forearm, not the hand's roll.
//! `tools/characters/moves.ts` hangs it the same way to measure it against the body.

use crate::character::{Body, Held, extra};
use bevy::{gltf::GltfExtras, prelude::*, world_serialization::WorldInstanceReady};
use serde::Deserialize;

/// Where a strapped look rides, as its file gives it.
#[derive(Deserialize, Debug, PartialEq)]
pub struct Strap {
    pub bone: String,
    pub at: [f32; 3],
    pub face: [f32; 3],
    pub grip: [f32; 3],
}

impl Strap {
    /// The look's turn in the bone's frame: its -Z along `face`, its +Y along `grip` (made
    /// square to the face if it is not quite).
    pub fn turn(&self) -> Quat {
        let back = -Vec3::from(self.face).normalize_or(Vec3::NEG_Z);
        let grip = Vec3::from(self.grip);
        let y = (grip - back * grip.dot(back)).normalize_or(back.any_orthonormal_vector());
        Quat::from_mat3(&Mat3::from_cols(y.cross(back), y, back)).normalize()
    }
}

/// A held look strapped to its bone (a shield), so a record or check can find it.
#[derive(Component)]
pub struct Strapped;

/// Once a held look has loaded, straps it on if its file says so (`put`).
pub(crate) fn strap(
    ready: On<WorldInstanceReady>,
    held: Query<(), With<Held>>,
    mut commands: Commands,
) {
    if held.contains(ready.entity) {
        commands.run_system_cached_with(put, ready.entity);
    }
}

/// Moves a held look onto the bone its file straps it to: placed by the strap, its size kept
/// against the bone's frame as it was against the hand socket's.
#[allow(clippy::too_many_arguments)]
fn put(
    In(look): In<Entity>,
    mut held: Query<(&mut Held, &mut Transform)>,
    frames: Query<&Transform, Without<Held>>,
    children: Query<&Children>,
    extras: Query<&GltfExtras>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    bodies: Query<(), With<Body>>,
    mut commands: Commands,
) {
    let Some(strap) = children.iter_descendants(look).find_map(|each| {
        serde_json::from_value::<Strap>(extra(extras.get(each).ok()?, "strap")?).ok()
    }) else {
        return;
    };
    let named = |each: &Entity| {
        names
            .get(*each)
            .is_ok_and(|name| name.as_str() == strap.bone)
    };
    // The bone is the hand's ancestor (the forearm) or, a twist bone, elsewhere in its body.
    let bone = parents.iter_ancestors(look).find(named).or_else(|| {
        let body = parents
            .iter_ancestors(look)
            .find(|each| bodies.contains(*each))?;
        children.iter_descendants(body).find(named)
    });
    let (Some(bone), Ok(socket)) = (bone, parents.get(look).map(ChildOf::parent)) else {
        warn!("Strap: no bone {} to strap a held look to.", strap.bone);
        return;
    };
    // How much smaller the socket's frame is than the bone's (the socket's scale undoes the
    // skeleton's), from their local scales up to the top.
    let scale = |from: Entity| -> f32 {
        std::iter::once(from)
            .chain(parents.iter_ancestors(from))
            .filter_map(|each| frames.get(each).ok())
            .map(|frame| frame.scale.x)
            .product()
    };
    let fit = scale(socket) / scale(bone).max(1e-6);
    let Ok((mut held, mut transform)) = held.get_mut(look) else {
        return;
    };
    held.fit = fit;
    held.turn = strap.turn();
    transform.translation = Vec3::from(strap.at) * fit;
    transform.rotation = held.turn;
    commands.entity(look).insert((ChildOf(bone), Strapped));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_files;

    #[test]
    fn every_strap_parses_and_names_a_bone_of_the_base_body() {
        let bones = model_files::nodes(&model_files::json(crate::appearance::BASE));
        let mut straps = 0;
        for file in model_files::glbs("models/held") {
            let Some(strap) = model_files::extra(&model_files::json(&file), "strap") else {
                continue;
            };
            let strap: Strap =
                serde_json::from_value(strap).unwrap_or_else(|e| panic!("{file}: {e}"));
            assert!(
                bones.contains(&strap.bone),
                "{file}: no bone {}",
                strap.bone
            );
            straps += 1;
        }
        assert!(straps > 0, "no held look is strapped");
    }

    #[test]
    fn a_strap_turns_the_front_to_its_face_and_the_grip_along_it() {
        let strap: Strap = serde_json::from_str(
            r#"{"bone": "lowerarm_l", "at": [0, 0.3, -0.06], "face": [0, 0, -1], "grip": [0, 1, 0]}"#,
        )
        .unwrap();
        assert_eq!(strap.bone, "lowerarm_l");
        for (face, grip) in [
            ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
            ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            ([0.0, 2.0, 0.0], [1.0, 0.0, 0.3]),
        ] {
            let turn = Strap {
                face,
                grip,
                bone: strap.bone.clone(),
                at: strap.at,
            }
            .turn();
            let face = Vec3::from(face).normalize();
            assert!((turn * Vec3::NEG_Z - face).length() < 1e-5, "{face}");
            let along = turn * Vec3::Y;
            assert!(along.dot(face).abs() < 1e-5);
            assert!(along.dot(Vec3::from(grip)) > 0.9);
        }
    }

    #[test]
    fn a_strapped_look_rides_its_bone_at_the_sockets_scale() {
        let mut world = World::new();
        let body = world.spawn((Body, Transform::default())).id();
        let arm = world
            .spawn((
                Name::new("lowerarm_l"),
                Transform::from_scale(Vec3::splat(1.25)),
                ChildOf(body),
            ))
            .id();
        let socket = world
            .spawn((
                Name::new("socket.hand.L"),
                Transform::from_scale(Vec3::splat(0.8)),
                ChildOf(arm),
            ))
            .id();
        let look = world
            .spawn((Held::new(0.0), Transform::default(), ChildOf(socket)))
            .id();
        world.spawn((
            GltfExtras {
                value: r#"{"strap": {"bone": "lowerarm_l", "at": [0, 0.3, -0.06], "face": [0, 0, -1], "grip": [0, 1, 0]}}"#.into(),
            },
            ChildOf(look),
        ));
        world.run_system_cached_with(put, look).unwrap();
        assert_eq!(world.get::<ChildOf>(look).unwrap().parent(), arm);
        let held = world.get::<Held>(look).unwrap();
        assert!((held.fit - 0.8).abs() < 1e-6);
        let placed = world.get::<Transform>(look).unwrap();
        // At the bone's scale, 0.3 m along it is 0.24 of its own units.
        assert!((placed.translation - Vec3::new(0.0, 0.24, -0.048)).length() < 1e-6);
        assert!((placed.rotation * Vec3::NEG_Z - Vec3::NEG_Z).length() < 1e-5);
    }
}
