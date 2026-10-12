//! What hangs on a character: things held and worn. A character's model carries sockets,
//! named points on its skeleton placed for that body when it was exported
//! (`spike/character_gear/export_character.py`); something rigid, a tool or a pack, is put
//! on one. Clothing that bends with the body is a model skinned to the same skeleton,
//! whose joints are swapped for the character's own once it has loaded.
//!
//! Something held is used at one angle and carried at another: swung, the grip holds it as
//! a hand would to strike; walking or standing, it tips forward and down (`Held`).
//!
//! Shared by the experiments that show characters, so a tool in the workbench sits as it
//! does in play.

use bevy::{
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

/// Clothing that bends with `body`, a character's model: its joints become the body's.
#[derive(Component)]
pub struct Fitted {
    pub body: Entity,
}

pub fn plugin(app: &mut App) {
    app.add_observer(fit).add_systems(Update, tilt);
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

/// Puts clothing that bends on `body`, a character's model.
pub fn dress(commands: &mut Commands, model: Handle<WorldAsset>, body: Entity) -> Entity {
    commands
        .spawn((
            WorldAssetRoot(model),
            Transform::default(),
            Fitted { body },
            ChildOf(body),
        ))
        .id()
}

/// Once fitted clothing has loaded, moves its skin onto the body's skeleton.
fn fit(
    ready: On<WorldInstanceReady>,
    fitted: Query<&Fitted>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut skins: Query<&mut SkinnedMesh>,
) {
    let Ok(Fitted { body }) = fitted.get(ready.entity) else {
        return;
    };
    let own: HashSet<Entity> = children.iter_descendants(ready.entity).collect();
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
                    "Clothing has a joint the body lacks: {:?}",
                    names.get(*joint).ok()
                ),
            }
        }
    }
}
