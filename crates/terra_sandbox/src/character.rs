//! What hangs on a character: things held and worn. A character's model carries sockets,
//! named points on its skeleton placed for that body when it was exported
//! (`spike/character_gear/export_character.py`).
//!
//! What is worn has a look for each base body, fitted to it once
//! (`spike/character_gear/fit_gear.py`) and found by name: `models/gear/<body>/<item>.glb`,
//! the item named as in the catalogue (`item_id`). The file says how it goes on, so
//! nothing here knows any one item: a look with a skin bends with the body, and its joints
//! are swapped for the body's own; a rigid one names its socket in its glTF extras,
//! `{"socket": "socket.back"}`, and is hung there.
//!
//! Something held is used at one angle and carried at another: swung, the grip holds it as
//! a hand would to strike; walking or standing, it tips forward and down (`Held`).
//!
//! Shared by the experiments that show characters, so a tool in the workbench sits as it
//! does in play.

use bevy::{
    gltf::GltfExtras,
    mesh::skinning::SkinnedMesh,
    prelude::*,
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};
use std::collections::{HashMap, HashSet};

/// The right hand's socket: the middle of a closed fist, +Y along a held handle toward its
/// working end, +Z toward the wrist. The others (`socket.head`, `socket.back`,
/// `socket.belt.R` and `.L`) face as the body does.
pub const HAND_R: &str = "socket.hand.R";

/// The clips in which something in hand is carried rather than used.
pub const CARRIED_IN: [&str; 10] = [
    "Idle_Loop",
    "Idle_Talking_Loop",
    "Walk_Loop",
    "Jog_Fwd_Loop",
    "Sprint_Loop",
    "Jump_Start",
    "Jump_Loop",
    "Jump_Land",
    "Crouch_Idle_Loop",
    "Crouch_Fwd_Loop",
];
/// How quickly something in hand turns between carried and used, in seconds.
const TURN_S: f32 = 0.12;

/// On a hand socket: whether what it holds is being carried (`CARRIED_IN`) or used.
#[derive(Component, Default)]
pub struct Carrying(pub bool);

/// Something held, a child of a hand socket. Carried, it turns `carry_tilt` degrees about
/// the socket's X: negative tips its working end forward and down when the arm hangs,
/// positive raises it.
#[derive(Component)]
pub struct Held {
    pub carry_tilt: f32,
    now: f32,
}

impl Held {
    pub fn new(carry_tilt: f32) -> Self {
        Held {
            carry_tilt,
            now: 0.0,
        }
    }
}

/// A character's model: the root of a base body, by the name its gear is fitted under. Its
/// parent is the character, whose gear it wears.
#[derive(Component)]
pub struct Body {
    pub name: &'static str,
}

/// A body whose model has loaded, so its sockets exist.
#[derive(Component)]
pub struct BodyReady;

/// A worn thing's look, on `body`, a character's model.
#[derive(Component)]
pub struct Worn {
    pub body: Entity,
}

/// An item's id in file names: its catalogue name in lower case, words joined by `_`,
/// as `--give` writes it: "Leather coat" is `leather_coat`.
pub fn item_id(name: &str) -> String {
    name.to_lowercase().replace(' ', "_")
}

/// Where an item's look for a base body is.
pub fn look(body: &str, item: &str) -> String {
    format!("models/gear/{body}/{item}.glb")
}

pub fn plugin(app: &mut App) {
    app.add_observer(put_on)
        .add_observer(body_ready)
        .add_systems(Update, tilt);
}

fn body_ready(
    ready: On<WorldInstanceReady>,
    bodies: Query<(), With<Body>>,
    mut commands: Commands,
) {
    if bodies.contains(ready.entity) {
        commands.entity(ready.entity).insert(BodyReady);
    }
}

/// Turns what is held toward its carried or used angle.
fn tilt(
    time: Res<Time>,
    hands: Query<&Carrying>,
    mut held: Query<(&mut Held, &mut Transform, &ChildOf)>,
) {
    let ease = 1.0 - libm::expf(-time.delta_secs() / TURN_S);
    for (mut held, mut transform, parent) in &mut held {
        let carried = hands.get(parent.parent()).is_ok_and(|hand| hand.0);
        let goal = if carried { held.carry_tilt } else { 0.0 };
        held.now += (goal - held.now) * ease;
        transform.rotation = Quat::from_rotation_x(held.now.to_radians());
    }
}

/// The first of `root`'s descendants called `name`.
pub fn find(
    root: Entity,
    name: &str,
    children: &Query<&Children>,
    names: &Query<&Name>,
) -> Option<Entity> {
    children
        .iter_descendants(root)
        .find(|each| names.get(*each).is_ok_and(|each| each.as_str() == name))
}

/// Puts a model on `to`: a socket, for something rigid. Returns the model's root.
pub fn hang(commands: &mut Commands, model: Handle<WorldAsset>, to: Entity) -> Entity {
    commands
        .spawn((WorldAssetRoot(model), Transform::default(), ChildOf(to)))
        .id()
}

/// Puts a worn thing's look on `body`, a character's model; it goes where its file says
/// once it has loaded (`put_on`). Returns the look's root.
pub fn wear(commands: &mut Commands, model: Handle<WorldAsset>, body: Entity) -> Entity {
    commands
        .spawn((
            WorldAssetRoot(model),
            Transform::default(),
            Worn { body },
            ChildOf(body),
        ))
        .id()
}

/// The socket a rigid look names in its glTF extras.
fn socket_in(extras: &GltfExtras) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(&extras.value).ok()?;
    Some(value.get("socket")?.as_str()?.to_string())
}

/// Once a worn look has loaded: a skinned one's joints become the body's; a rigid one moves
/// onto the socket it names.
fn put_on(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    worn: Query<&Worn>,
    children: Query<&Children>,
    names: Query<&Name>,
    extras: Query<&GltfExtras>,
    mut skins: Query<&mut SkinnedMesh>,
) {
    let Ok(Worn { body }) = worn.get(ready.entity) else {
        return;
    };
    let own: HashSet<Entity> = children.iter_descendants(ready.entity).collect();
    if !own.iter().any(|each| skins.contains(*each)) {
        let socket = own
            .iter()
            .find_map(|each| socket_in(extras.get(*each).ok()?));
        match socket.and_then(|socket| Some((find(*body, &socket, &children, &names)?, socket))) {
            Some((at, _)) => {
                commands.entity(ready.entity).insert(ChildOf(at));
            }
            None => warn!("A worn look names no socket this body has."),
        }
        return;
    }
    let joints: HashMap<&str, Entity> = children
        .iter_descendants(*body)
        .filter(|each| !own.contains(each))
        .filter_map(|each| Some((names.get(each).ok()?.as_str(), each)))
        .collect();
    for entity in &own {
        let Ok(mut skin) = skins.get_mut(*entity) else {
            continue;
        };
        for joint in skin.joints.iter_mut() {
            match names
                .get(*joint)
                .ok()
                .and_then(|name| joints.get(name.as_str()))
            {
                Some(theirs) => *joint = *theirs,
                None => warn!(
                    "A worn look has a joint the body lacks: {:?}",
                    names.get(*joint).ok()
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_look_is_found_by_body_and_item_name() {
        assert_eq!(item_id("Leather coat"), "leather_coat");
        assert_eq!(item_id("Quiver"), "quiver");
        assert_eq!(
            look("base-female", &item_id("Small pack")),
            "models/gear/base-female/small_pack.glb"
        );
    }

    #[test]
    fn a_rigid_look_names_its_socket() {
        let extras = GltfExtras {
            value: r#"{"socket": "socket.back"}"#.to_string(),
        };
        assert_eq!(socket_in(&extras).as_deref(), Some("socket.back"));
        let none = GltfExtras {
            value: "{}".to_string(),
        };
        assert_eq!(socket_in(&none), None);
    }
}
