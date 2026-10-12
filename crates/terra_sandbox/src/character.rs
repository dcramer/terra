//! What hangs on a character: things held and worn. A character's model carries sockets,
//! named points on its skeleton placed for that body when it was exported
//! (`spike/character_gear/export_character.py`).
//!
//! What is worn has a look for each base body, fitted to it once
//! (`spike/character_gear/fit_gear.py`) and found by name: `models/gear/<body>/<item>.glb`,
//! the item named as in the catalogue (`item_id`). Looks name their materials by role
//! (Leather, Metal, Wood...), so a `Tint` can recolour one look many ways without a new
//! file, and tinted materials are shared, one per colour. The file says how it goes on, so
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
    gltf::{GltfExtras, GltfMaterialName},
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

/// What a character wears: the looks its body shows (`worn` in the play experiment). The
/// player's follows its equipment; anyone else's is set by whatever stands them in the world.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Outfit(pub Vec<Clothing>);

/// One thing worn: the item, by id (`item_id`), and how it is coloured.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clothing {
    pub item: String,
    pub tint: Tint,
}

impl Clothing {
    pub fn new(item: impl Into<String>) -> Self {
        Clothing {
            item: item.into(),
            tint: Tint::default(),
        }
    }
}

/// Colours for a look's parts, by the role their material plays (its glTF material name
/// before any `.`): `[("Leather", blue)]` dyes every leather part blue. Over a texture, the
/// colour dyes it, keeping its light and dark (`dye`). On a look's root; applied once it has loaded.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Tint(pub Vec<(String, Color)>);

/// Tinted materials, shared: one per original material and colour, however many looks use it.
#[derive(Resource, Default)]
struct Tinted(HashMap<(AssetId<StandardMaterial>, [u32; 4]), Handle<StandardMaterial>>);

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

/// Where a held item's look is: the same for every body, since every hand socket is alike.
pub fn held_look(item: &str) -> String {
    format!("models/held/{item}.glb")
}

/// Where the look of an item that is neither held nor worn is, such as a material: the
/// thing on its own, as it lies, life size. Its icon is drawn from it (`icons`).
pub fn item_look(item: &str) -> String {
    format!("models/items/{item}.glb")
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Tinted>()
        .add_observer(put_on)
        .add_observer(body_ready)
        .add_observer(held_ready)
        .add_observer(tint)
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

/// Puts a held thing's look in `hand`, a hand socket. Once it has loaded it tips, when
/// carried, as its file says (`carry_tilt` in its glTF extras). Returns the look's root.
pub fn hold(commands: &mut Commands, model: Handle<WorldAsset>, hand: Entity) -> Entity {
    commands
        .spawn((
            WorldAssetRoot(model),
            Transform::default(),
            Held::new(0.0),
            ChildOf(hand),
        ))
        .id()
}

/// A value a look's file gives in its glTF extras.
fn extra(extras: &GltfExtras, key: &str) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(&extras.value).ok()?;
    value.get(key).cloned()
}

/// The socket a rigid look names in its glTF extras.
fn socket_in(extras: &GltfExtras) -> Option<String> {
    Some(extra(extras, "socket")?.as_str()?.to_string())
}

/// Once a held look has loaded, takes how far it tips when carried from its file.
fn held_ready(
    ready: On<WorldInstanceReady>,
    mut held: Query<&mut Held>,
    children: Query<&Children>,
    extras: Query<&GltfExtras>,
) {
    let Ok(mut held) = held.get_mut(ready.entity) else {
        return;
    };
    if let Some(tilt) = children
        .iter_descendants(ready.entity)
        .find_map(|each| extra(extras.get(each).ok()?, "carry_tilt")?.as_f64())
    {
        held.carry_tilt = tilt as f32;
    }
}

/// Once a tinted look has loaded, gives each part whose role has a colour a material in that
/// colour, shared with every other part tinted the same.
fn tint(
    ready: On<WorldInstanceReady>,
    tints: Query<&Tint>,
    children: Query<&Children>,
    parts: Query<(Entity, &MeshMaterial3d<StandardMaterial>, &GltfMaterialName)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tinted: ResMut<Tinted>,
    mut commands: Commands,
) {
    let Ok(tint) = tints.get(ready.entity) else {
        return;
    };
    for each in children.iter_descendants(ready.entity) {
        let Ok((part, material, name)) = parts.get(each) else {
            continue;
        };
        let role = name.0.split('.').next().unwrap_or_default();
        let Some((_, colour)) = tint.0.iter().find(|(wanted, _)| wanted == role) else {
            continue;
        };
        let key = (
            material.0.id(),
            colour.to_linear().to_f32_array().map(f32::to_bits),
        );
        let handle = match tinted.0.get(&key) {
            Some(handle) => handle.clone(),
            None => {
                let Some(mut recoloured) = materials.get(&material.0).cloned() else {
                    continue;
                };
                recoloured.base_color = if recoloured.base_color_texture.is_some() {
                    dye(*colour)
                } else {
                    *colour
                };
                let handle = materials.add(recoloured);
                tinted.0.insert(key, handle.clone());
                handle
            }
        };
        commands.entity(part).insert(MeshMaterial3d(handle));
    }
}

/// A colour to multiply a texture by, keeping its brightness: scaled so its brightest
/// channel is full, as dye takes the cloth's own light and dark.
fn dye(colour: Color) -> Color {
    let linear = colour.to_linear();
    let most = linear.red.max(linear.green).max(linear.blue).max(1e-3);
    Color::linear_rgb(linear.red / most, linear.green / most, linear.blue / most)
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
    fn a_dye_keeps_a_texture_bright() {
        let blue = dye(Color::linear_rgb(0.1, 0.2, 0.4)).to_linear();
        assert!((blue.blue - 1.0).abs() < 1e-5);
        assert!((blue.green - 0.5).abs() < 1e-5);
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
