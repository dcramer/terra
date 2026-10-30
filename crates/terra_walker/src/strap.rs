//! A held look strapped to the forearm rather than hung on the hand socket: a shield (D504,
//! D554). Its file says how, in its glTF extras, `{"strap": {"bone": "lowerarm_l", "fist":
//! "socket.hand.L", "face": [x, y, z], "grip": [x, y, z]}}` (`tools/characters/fit_held.py`):
//! the forearm it rides on, the fist that closes on its hand strap, and the directions its front
//! (-Z) and its grip (+Y) turn to in the true forearm's frame (the bone's, turned the least way
//! to run from the elbow to the wrist on this body). The strap is physical and per body: the
//! look's origin, its hand strap, is wherever this body's fist is in the forearm's frame, and
//! its grip runs along this body's forearm, so a woman's shorter forearm puts the board nearer
//! her elbow and nothing is set per body. It is held as anything is (`character::hold`), moved
//! onto the bone once it has loaded, and kept at the fist after every clip and fit-up
//! (`follow_fist`), so it follows the forearm, not the hand's roll.
//! `tools/characters/held.ts` hangs it the same way for the move check; change both.

use crate::character::{Body, Held, extra};
use bevy::{gltf::GltfExtras, prelude::*, world_serialization::WorldInstanceReady};
use serde::Deserialize;

/// How a strapped look rides, as its file gives it.
#[derive(Deserialize, Debug, PartialEq)]
pub struct Strap {
    pub bone: String,
    pub fist: String,
    pub face: [f32; 3],
    pub grip: [f32; 3],
}

impl Strap {
    /// The look's turn in the true forearm's frame: its -Z along `face`, its +Y along `grip`
    /// (made square to the face if it is not quite).
    pub fn turn(&self) -> Quat {
        let back = -Vec3::from(self.face).normalize_or(Vec3::NEG_Z);
        let grip = Vec3::from(self.grip);
        let y = (grip - back * grip.dot(back)).normalize_or(back.any_orthonormal_vector());
        Quat::from_mat3(&Mat3::from_cols(y.cross(back), y, back)).normalize()
    }
}

/// Where a strapped look sits in its bone's frame: at the fist (`socket`, a child of `hand`,
/// both given in their parents' frames), turned by `turn` in the true forearm's frame, which is
/// the bone's turned the least way to take its +Y onto the wrist (`hand`'s joint).
pub fn at_fist(hand: &Transform, socket: &Transform, turn: Quat) -> (Vec3, Quat) {
    let along = Quat::from_rotation_arc(Vec3::Y, hand.translation.normalize_or(Vec3::Y));
    (hand.transform_point(socket.translation), along * turn)
}

/// A held look strapped to its bone (a shield): the hand and the fist's socket it is kept at,
/// and its turn in the true forearm's frame.
#[derive(Component)]
pub struct Strapped {
    pub(crate) hand: Entity,
    pub(crate) socket: Entity,
    pub(crate) turn: Quat,
}

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

/// Moves a held look onto the bone its file straps it to, at the fist: its size kept against
/// the bone's frame as it was against the hand socket's.
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
    let Some(bone) = bone else {
        warn!("Strap: no bone {} to strap a held look to.", strap.bone);
        return;
    };
    // The fist it closes on: the hand socket it is held in.
    let Ok(socket) = parents.get(look).map(ChildOf::parent) else {
        return;
    };
    if names.get(socket).map(Name::as_str) != Ok(strap.fist.as_str()) {
        warn!(
            "Strap: a look strapped at {} is held elsewhere.",
            strap.fist
        );
        return;
    }
    let Ok(hand) = parents.get(socket).map(ChildOf::parent) else {
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
    let (Ok(hand_at), Ok(socket_at)) = (frames.get(hand), frames.get(socket)) else {
        return;
    };
    let Ok((mut held, mut transform)) = held.get_mut(look) else {
        return;
    };
    let (at, turn) = at_fist(hand_at, socket_at, strap.turn());
    held.fit = fit;
    held.turn = turn;
    transform.translation = at;
    transform.rotation = held.turn;
    commands.entity(look).insert((
        ChildOf(bone),
        Strapped {
            hand,
            socket,
            turn: strap.turn(),
        },
    ));
}

/// Keeps each strapped look at its fist, as the clips and fit-ups have posed the hand: the
/// fist in the forearm's frame moves only if the hand's pose or the body's shape does.
pub(crate) fn follow_fist(
    mut looks: Query<(&Strapped, &mut Held, &mut Transform)>,
    frames: Query<&Transform, Without<Held>>,
) {
    for (strapped, mut held, mut transform) in &mut looks {
        let (Ok(hand), Ok(socket)) = (frames.get(strapped.hand), frames.get(strapped.socket))
        else {
            continue;
        };
        let (at, turn) = at_fist(hand, socket, strapped.turn);
        if held.turn != turn {
            held.turn = turn;
        }
        let rotation = held.turn;
        if transform.translation != at || transform.rotation != rotation {
            transform.translation = at;
            transform.rotation = rotation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_files;

    const STRAP: &str = r#"{"strap": {"bone": "lowerarm_l", "fist": "socket.hand.L", "face": [0, 0, -1], "grip": [0, 1, 0]}}"#;

    #[test]
    fn every_strap_parses_and_names_a_bone_and_fist_of_the_base_body() {
        let bones = model_files::nodes(&model_files::json(crate::appearance::BASE));
        let mut straps = 0;
        for file in model_files::glbs("models/held") {
            let Some(strap) = model_files::extra(&model_files::json(&file), "strap") else {
                continue;
            };
            let strap: Strap =
                serde_json::from_value(strap).unwrap_or_else(|e| panic!("{file}: {e}"));
            for name in [&strap.bone, &strap.fist] {
                assert!(bones.contains(name), "{file}: no node {name}");
            }
            straps += 1;
        }
        assert!(straps > 0, "no held look is strapped");
    }

    #[test]
    fn a_strap_turns_the_front_to_its_face_and_the_grip_along_it() {
        for (face, grip) in [
            ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
            ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            ([0.0, 2.0, 0.0], [1.0, 0.0, 0.3]),
        ] {
            let turn = Strap {
                face,
                grip,
                bone: "lowerarm_l".into(),
                fist: "socket.hand.L".into(),
            }
            .turn();
            let face = Vec3::from(face).normalize();
            assert!((turn * Vec3::NEG_Z - face).length() < 1e-5, "{face}");
            let along = turn * Vec3::Y;
            assert!(along.dot(face).abs() < 1e-5);
            assert!(along.dot(Vec3::from(grip)) > 0.9);
        }
    }

    /// A forearm with a hand and its socket, and a look held in the socket: (world, bone,
    /// hand, socket, look).
    fn arm(wrist: Vec3, fist: Vec3) -> (World, Entity, Entity, Entity, Entity) {
        let mut world = World::new();
        let body = world.spawn((Body, Transform::default())).id();
        let bone = world
            .spawn((
                Name::new("lowerarm_l"),
                Transform::from_scale(Vec3::splat(1.25)),
                ChildOf(body),
            ))
            .id();
        let hand = world
            .spawn((
                Name::new("hand_l"),
                Transform::from_translation(wrist),
                ChildOf(bone),
            ))
            .id();
        let socket = world
            .spawn((
                Name::new("socket.hand.L"),
                Transform::from_translation(fist).with_scale(Vec3::splat(0.8)),
                ChildOf(hand),
            ))
            .id();
        let look = world
            .spawn((Held::default(), Transform::default(), ChildOf(socket)))
            .id();
        world.spawn((
            GltfExtras {
                value: STRAP.into(),
            },
            ChildOf(look),
        ));
        (world, bone, hand, socket, look)
    }

    #[test]
    fn a_strapped_look_rides_its_bone_at_the_fist_at_the_sockets_scale() {
        let (mut world, bone, _, _, look) =
            arm(Vec3::new(0.0, 0.2, 0.0), Vec3::new(0.04, 0.09, -0.01));
        world.run_system_cached_with(put, look).unwrap();
        assert_eq!(world.get::<ChildOf>(look).unwrap().parent(), bone);
        let held = world.get::<Held>(look).unwrap();
        assert!((held.fit - 0.8).abs() < 1e-6);
        let placed = world.get::<Transform>(look).unwrap();
        // The fist, 9 cm past a wrist 20 cm along the bone, in the bone's own units.
        assert!((placed.translation - Vec3::new(0.04, 0.29, -0.01)).length() < 1e-6);
        assert!((placed.rotation * Vec3::NEG_Z - Vec3::NEG_Z).length() < 1e-5);
    }

    #[test]
    fn the_strap_follows_the_fist_and_the_true_forearm() {
        // A forearm whose wrist lies off the bone's +Y, as a morph's moved joints leave it.
        let wrist = Vec3::new(-0.01, 0.18, 0.004);
        let (mut world, _, hand, socket, look) = arm(wrist, Vec3::new(0.04, 0.09, -0.01));
        world.run_system_cached_with(put, look).unwrap();
        world.get_mut::<Transform>(socket).unwrap().translation = Vec3::new(0.03, 0.1, 0.0);
        world.get_mut::<Transform>(hand).unwrap().rotation = Quat::from_rotation_z(0.3);
        world.run_system_cached(follow_fist).unwrap();
        let placed = *world.get::<Transform>(look).unwrap();
        let hand = *world.get::<Transform>(hand).unwrap();
        let fist = hand.transform_point(Vec3::new(0.03, 0.1, 0.0));
        assert!((placed.translation - fist).length() < 1e-6);
        // Its grip (+Y) runs from the elbow to the wrist, whatever the hand's turn.
        assert!((placed.rotation * Vec3::Y - wrist.normalize()).length() < 1e-5);
    }
}
