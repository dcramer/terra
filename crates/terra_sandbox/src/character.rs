//! What hangs on a character: things held and worn. A character's model carries sockets,
//! named points on its skeleton placed for that body when it was exported
//! (`spike/character_gear/export_character.py`); something rigid, a tool or a pack, is put
//! on one. Clothing that bends with the body is a model skinned to the same skeleton,
//! whose joints are swapped for the character's own once it has loaded.
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

/// Clothing that bends with `body`, a character's model: its joints become the body's.
#[derive(Component)]
pub struct Fitted {
    pub body: Entity,
}

pub fn plugin(app: &mut App) {
    app.add_observer(fit);
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
