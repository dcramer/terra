//! What hangs on a character: things held and worn. A character's model carries sockets,
//! named points on its skeleton placed for that body when it was exported
//! (`tools/characters/export_character.py`).
//!
//! What is worn has one look, made once on the base body and carrying the base's morphs, so
//! it fits every body (`appearance`, D128; `tools/characters/build_bodies.sh`), found by name:
//! `models/gear/<item>.glb`, the item named as in the catalogue (`item_id`). Looks name their materials by role
//! (Leather, Metal, Wood...), so a `Tint` can recolour one look many ways without a new
//! file, and tinted materials are shared, one per colour. The file says how it goes on, so
//! nothing here knows any one item: a look with a skin bends with the body, and its joints
//! are swapped for the body's own; a rigid one names its socket in its glTF extras,
//! `{"socket": "socket.back"}`, and is hung there.
//!
//! A body hides the skin under what it wears: a look lists the patches of skin it covers
//! (`{"covers": [..]}`, worked out by `fit_gear.py`), and the body's material collapses those
//! patches while it is worn (`BodyMask`, `body_mask.wesl`). Bodies with the same mask share
//! one material.
//!
//! Something held is used at one angle and carried at another: swung, the grip holds it as
//! a hand would to strike; walking or standing, it tips forward and down (`Held`, in `held`).
//! A shield is strapped to the forearm instead, where its file says (`strap`).
//!
//! Shared by the experiments that show characters, so a tool in the workbench sits as it
//! does in play.

mod held;

pub(crate) use held::look_points;
pub use held::{CARRIED_IN, CARRY_DOWN, CARRY_REACH, Carrying, Held, carry_rule};

use bevy::{
    gltf::{GltfExtras, GltfMaterialName},
    mesh::{
        VertexAttributeValues,
        skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    },
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};
use std::collections::{HashMap, HashSet};

/// The hips' joint on every body's skeleton (Unreal's names, as Quaternius's animation libraries
/// use them: `tools/characters/clips.py`).
pub const HIPS: &str = "pelvis";

/// The right hand's socket: the middle of a closed fist, +Y along a held handle toward its
/// working end, +Z toward the wrist. The others (`socket.head`, `socket.back`,
/// `socket.belt.R` and `.L`) face as the body does.
pub const HAND_R: &str = "socket.hand.R";
/// The left hand's socket, alike: its +Y, too, runs along a held handle (a torch's), so one look
/// fits either hand (D128); a strapped look hangs from the forearm instead (`strap`).
pub const HAND_L: &str = "socket.hand.L";

/// A character's model: the root of the base body (`appearance::BASE`), shaped by its
/// `Appearance`. Its parent is the character, whose gear it wears.
#[derive(Component)]
pub struct Body;

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

/// A body's material: the standard one, collapsing the patches of skin its mask hides.
pub type BodyMaterial = ExtendedMaterial<StandardMaterial, BodyMask>;

/// How many patches a body is divided into (`tools/characters/patches.py`).
pub const PATCHES: u32 = 1024;

/// Which of a body's patches are hidden, one bit each. Field order must match
/// body_mask.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Copy, Default, PartialEq)]
pub struct BodyMask {
    #[uniform(100)]
    hidden: [UVec4; 8],
}

impl BodyMask {
    /// The mask hiding these patches.
    pub fn hiding(patches: impl IntoIterator<Item = u32>) -> Self {
        let mut hidden = [UVec4::ZERO; 8];
        for patch in patches.into_iter().filter(|patch| *patch < PATCHES) {
            let word = &mut hidden[(patch / 128) as usize][((patch / 32) % 4) as usize];
            *word |= 1 << (patch % 32);
        }
        BodyMask { hidden }
    }

    fn key(&self) -> [u32; 32] {
        let mut key = [0; 32];
        for (i, word) in self.hidden.iter().enumerate() {
            key[i * 4..i * 4 + 4].copy_from_slice(&word.to_array());
        }
        key
    }
}

impl MaterialExtension for BodyMask {
    fn vertex_shader() -> ShaderRef {
        "shaders/body_mask.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/body_mask.wesl".into()
    }
}

/// The patches of skin a worn look covers, from its file; on a look once it is put on.
#[derive(Component)]
pub struct Covers(pub Vec<u32>);

/// A body's meshes and the standard materials they show now (their own, or a template's
/// painted skin, `appearance`), the materials they came with, and the mask they show now.
#[derive(Component)]
pub struct Skin {
    pub parts: Vec<(Entity, Handle<StandardMaterial>)>,
    pub original: Vec<Handle<StandardMaterial>>,
    pub showing: Option<BodyMask>,
}

/// Body materials, shared: one per original material and mask.
#[derive(Resource, Default)]
struct BodyMaterials(HashMap<(AssetId<StandardMaterial>, [u32; 32]), Handle<BodyMaterial>>);

/// A body whose model has loaded, so its sockets exist.
#[derive(Component)]
pub struct BodyReady;

/// How far a look hung on a socket moves, in its socket's space, for each of the body's morphs
/// at full weight (`{"morphs": {M: [x, y, z]}}` in its extras, by `carry_gear.py`): beyond its
/// socket's own move, so it keeps its clearance as the skin beneath it grows. `appearance`
/// mixes them by the body's morph weights.
#[derive(Component, Default, Debug, PartialEq)]
pub struct Shifts(pub HashMap<String, Vec3>);

impl Shifts {
    /// The shifts in a look's extras, if it has any.
    pub fn from_extras(value: serde_json::Value) -> Option<Shifts> {
        let raw: HashMap<String, [f32; 3]> = serde_json::from_value(value).ok()?;
        Some(Shifts(
            raw.into_iter()
                .map(|(m, v)| (m, Vec3::from_array(v)))
                .collect(),
        ))
    }

    /// The shift for a mix of morphs: each morph's by its weight.
    pub fn mixed(&self, weights: &[(String, f32)]) -> Vec3 {
        crate::appearance::mix(weights, |morph| self.0.get(morph).copied())
    }
}

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

/// Where an item's look is: one for every body (`appearance`).
pub fn look(item: &str) -> String {
    format!("models/gear/{item}.glb")
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
    // Bodies and what they wear skin with dual quaternions (`skinning`).
    app.add_plugins((
        MaterialPlugin::<BodyMaterial>::default(),
        crate::skinning::plugin,
        crate::swing::plugin,
        crate::lift::plugin,
    ))
    .init_resource::<Tinted>()
    .init_resource::<BodyMaterials>()
    .init_resource::<Trimmed>()
    .add_systems(Update, mask_bodies)
    .add_observer(put_on)
    .add_observer(body_ready)
    .add_observer(held::held_ready)
    .add_observer(crate::strap::strap)
    .add_observer(tint)
    .add_systems(Update, held::tilt)
    .add_systems(
        PostUpdate,
        crate::strap::follow_fist
            .after(bevy::app::AnimationSystems)
            .after(crate::Fitted)
            .before(bevy::transform::TransformSystems::Propagate),
    );
}

fn body_ready(
    ready: On<WorldInstanceReady>,
    bodies: Query<(), With<Body>>,
    children: Query<&Children>,
    looks: Query<(Entity, &MeshMaterial3d<StandardMaterial>)>,
    mut commands: Commands,
) {
    if !bodies.contains(ready.entity) {
        return;
    }
    // Its meshes now, before anything is worn: the skin.
    let parts: Vec<_> = children
        .iter_descendants(ready.entity)
        .filter_map(|each| looks.get(each).ok())
        .map(|(each, material)| (each, material.0.clone()))
        .collect();
    let original = parts.iter().map(|(_, material)| material.clone()).collect();
    commands.entity(ready.entity).insert((
        BodyReady,
        Skin {
            parts,
            original,
            showing: None,
        },
    ));
}

/// Gives each body the material hiding what its looks cover, when that changes.
fn mask_bodies(
    mut commands: Commands,
    mut bodies: Query<(Entity, &mut Skin)>,
    looks: Query<(&Worn, &Covers)>,
    added: Query<(), Added<Covers>>,
    mut removed: RemovedComponents<Worn>,
    standard: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<BodyMaterial>>,
    mut shared: ResMut<BodyMaterials>,
) {
    let changed = !added.is_empty() || removed.read().count() > 0;
    for (body, mut skin) in &mut bodies {
        if !changed && !skin.is_changed() {
            continue;
        }
        let mask = BodyMask::hiding(
            looks
                .iter()
                .filter(|(worn, _)| worn.body == body)
                .flat_map(|(_, covers)| covers.0.iter().copied()),
        );
        if skin.showing == Some(mask) {
            continue;
        }
        skin.showing = Some(mask);
        for (part, base) in &skin.parts {
            let key = (base.id(), mask.key());
            let handle = match shared.0.get(&key) {
                Some(handle) => handle.clone(),
                None => {
                    let Some(base_material) = standard.get(base).cloned() else {
                        continue;
                    };
                    let handle = materials.add(BodyMaterial {
                        base: base_material,
                        extension: mask,
                    });
                    shared.0.insert(key, handle.clone());
                    handle
                }
            };
            commands
                .entity(*part)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(handle));
        }
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
pub(crate) fn extra(extras: &GltfExtras, key: &str) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(&extras.value).ok()?;
    value.get(key).cloned()
}

/// The socket a rigid look names in its glTF extras.
fn socket_in(extras: &GltfExtras) -> Option<String> {
    Some(extra(extras, "socket")?.as_str()?.to_string())
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

/// A body's own joints by name, leaving out everything under the looks worn on it.
fn body_joints<'a>(
    body: Entity,
    children: &Query<&Children>,
    names: &'a Query<&Name>,
    worn: &Query<&Worn>,
) -> HashMap<&'a str, Entity> {
    let mut joints = HashMap::new();
    let mut stack: Vec<Entity> = children.get(body).map(|c| c.to_vec()).unwrap_or_default();
    while let Some(each) = stack.pop() {
        if worn.contains(each) {
            continue;
        }
        if let Ok(name) = names.get(each) {
            joints.entry(name.as_str()).or_insert(each);
        }
        if let Ok(more) = children.get(each) {
            stack.extend(more.iter());
        }
    }
    joints
}

/// Once a worn look has loaded: a skinned one's joints become the body's; a rigid one moves
/// onto the socket it names.
fn put_on(ready: On<WorldInstanceReady>, mut commands: Commands, worn: Query<&Worn>) {
    if let Ok(Worn { body }) = worn.get(ready.entity) {
        commands.run_system_cached_with(dress, (ready.entity, *body));
    }
}

/// Each worn look's mesh cut down to the joints it uses, once, for every wearer: which of its
/// file's joints it keeps, in order, and their bind poses.
#[derive(Resource, Default)]
pub struct Trimmed(HashMap<AssetId<Mesh>, (Vec<usize>, Handle<SkinnedMeshInverseBindposes>)>);

/// Which of `count` joints the vertices use (any weight above zero), in order, and for each
/// joint its index among those kept: a look's file names every joint of the skeleton, and
/// each costs a matrix computed and sent to the GPU every frame for every wearer.
pub fn used_joints(
    indices: &[[u16; 4]],
    weights: &[[f32; 4]],
    count: usize,
) -> (Vec<usize>, Vec<u16>) {
    let mut used = vec![false; count];
    for (joints, ws) in indices.iter().zip(weights) {
        for (j, w) in joints.iter().zip(ws) {
            if *w > 0.0 && (*j as usize) < count {
                used[*j as usize] = true;
            }
        }
    }
    let kept: Vec<usize> = (0..count).filter(|i| used[*i]).collect();
    let mut index = vec![0u16; count];
    for (new, old) in kept.iter().enumerate() {
        index[*old] = new as u16;
    }
    (kept, index)
}

/// Cuts mesh `id`'s skin down to the joints it uses, once: its joint indices renumbered and its
/// bind poses (from its file, before any body reshapes them) kept for those alone.
fn trim(
    id: AssetId<Mesh>,
    poses: &Handle<SkinnedMeshInverseBindposes>,
    count: usize,
    meshes: &mut Assets<Mesh>,
    bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
) -> Option<(Vec<usize>, Handle<SkinnedMeshInverseBindposes>)> {
    let all: Vec<Mat4> = bindposes.get(poses)?.to_vec();
    let mut mesh = meshes.get_mut(id)?;
    let weights = match mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)? {
        VertexAttributeValues::Float32x4(w) => w.clone(),
        _ => return None,
    };
    let Some(VertexAttributeValues::Uint16x4(indices)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_JOINT_INDEX)
    else {
        return None;
    };
    let (kept, index) = used_joints(indices, &weights, count);
    if kept.is_empty() || kept.len() == count {
        return None;
    }
    for (joints, ws) in indices.iter_mut().zip(&weights) {
        for (j, w) in joints.iter_mut().zip(ws) {
            // An unused slot points at the first joint kept, with no weight.
            *j = if *w > 0.0 { index[*j as usize] } else { 0 };
        }
    }
    let handle = bindposes.add(SkinnedMeshInverseBindposes::from(
        kept.iter().map(|i| all[*i]).collect::<Vec<_>>(),
    ));
    Some((kept, handle))
}

/// Puts worn look `look` on `body`, as its file says: hung on its socket, or its skins
/// re-bound to the body's own joints and its copy of the skeleton dropped.
fn dress(
    In((look, body)): In<(Entity, Entity)>,
    mut commands: Commands,
    children: Query<&Children>,
    names: Query<&Name>,
    extras: Query<&GltfExtras>,
    mut skins: Query<&mut SkinnedMesh>,
    parents: Query<&ChildOf>,
    meshes: Query<(), With<Mesh3d>>,
    worn: Query<&Worn>,
    drawn: Query<&Mesh3d>,
    mut assets: Option<ResMut<Assets<Mesh>>>,
    mut bindposes: Option<ResMut<Assets<SkinnedMeshInverseBindposes>>>,
    mut trimmed: Option<ResMut<Trimmed>>,
) {
    let body = &body;
    let own: HashSet<Entity> = children.iter_descendants(look).collect();
    let covered: Vec<u32> = own
        .iter()
        .find_map(|each| extra(extras.get(*each).ok()?, "covers"))
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    commands.entity(look).insert(Covers(covered));

    if !own.iter().any(|each| skins.contains(*each)) {
        let socket = own
            .iter()
            .find_map(|each| socket_in(extras.get(*each).ok()?));
        let shifts = own
            .iter()
            .find_map(|each| extra(extras.get(*each).ok()?, "morphs"))
            .and_then(Shifts::from_extras)
            .unwrap_or_default();
        // A loose look swings about where it hangs (D189): the part that says so, about its
        // own origin.
        let swing = own.iter().find_map(|each| {
            let spring =
                crate::swing::Springiness::from_extras(extra(extras.get(*each).ok()?, "swing")?)?;
            Some((*each, spring))
        });
        match socket.and_then(|socket| Some((find(*body, &socket, &children, &names)?, socket))) {
            Some((at, _)) => {
                commands.entity(look).insert((ChildOf(at), shifts));
                if let Some((part, spring)) = swing {
                    commands
                        .entity(part)
                        .insert(crate::swing::Swing::new(spring));
                }
            }
            None => warn!("A worn look names no socket this body has."),
        }
        return;
    }
    // The body's own joints: not those of any look worn on it, which carry a copy of the
    // skeleton standing at rest.
    let joints = body_joints(*body, &children, &names, &worn);
    let mut copied: HashSet<Entity> = HashSet::new();
    for entity in &own {
        let Ok(mut skin) = skins.get_mut(*entity) else {
            continue;
        };
        copied.extend(skin.joints.iter().copied());
        // Only the joints it uses, cut once per mesh for every wearer.
        if let (Ok(mesh), Some(assets), Some(bindposes), Some(trimmed)) = (
            drawn.get(*entity),
            assets.as_deref_mut(),
            bindposes.as_deref_mut(),
            trimmed.as_deref_mut(),
        ) {
            let id = mesh.0.id();
            if !trimmed.0.contains_key(&id)
                && let Some(cut) = trim(
                    id,
                    &skin.inverse_bindposes,
                    skin.joints.len(),
                    assets,
                    bindposes,
                )
            {
                trimmed.0.insert(id, cut);
            }
            if let Some((kept, poses)) = trimmed.0.get(&id) {
                skin.joints = kept.iter().map(|i| skin.joints[*i]).collect();
                skin.inverse_bindposes = poses.clone();
            }
        }
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
    // Its own copy of the skeleton, now unused (each look's file brings one, and only that
    // look's skins pointed at it): dropped, so a dressed character does not carry a dead
    // skeleton per look, some 70 entities each, every one a transform kept up. Only the tops
    // of the copy, and only where nothing drawn hangs beneath them.
    for top in copied.iter().filter(|joint| {
        !parents
            .get(**joint)
            .is_ok_and(|p| copied.contains(&p.parent()))
    }) {
        if children
            .iter_descendants(*top)
            .any(|each| meshes.contains(each))
        {
            continue;
        }
        commands.entity(*top).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dye_keeps_a_texture_bright() {
        let blue = dye(Color::linear_rgb(0.1, 0.2, 0.4)).to_linear();
        assert!((blue.blue - 1.0).abs() < 1e-5);
        assert!((blue.green - 0.5).abs() < 1e-5);
    }

    #[test]
    fn a_mask_sets_one_bit_per_patch() {
        let mask = BodyMask::hiding([0, 33, 1023]);
        let key = mask.key();
        assert_eq!((key[0], key[1], key[31]), (1, 2, 1 << 31));
        assert_eq!(key[2..31].iter().sum::<u32>(), 0);
    }

    #[test]
    fn a_worn_look_takes_the_bodys_joints_and_drops_its_own() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let joint = |world: &mut World, name: &str, parent: Entity| {
            world
                .spawn((Name::new(name.to_string()), ChildOf(parent)))
                .id()
        };
        let body = world.spawn(Name::new("body")).id();
        let hips = joint(&mut world, "pelvis", body);
        let spine = joint(&mut world, "spine_01", hips);
        let look = world.spawn(Worn { body }).id();
        let armature = joint(&mut world, "Armature", look);
        let their_hips = joint(&mut world, "pelvis", armature);
        let their_spine = joint(&mut world, "spine_01", their_hips);
        let mesh = world
            .spawn((
                Mesh3d::default(),
                SkinnedMesh {
                    inverse_bindposes: Handle::default(),
                    joints: vec![their_hips, their_spine],
                },
                ChildOf(armature),
            ))
            .id();
        world.run_system_once_with(dress, (look, body)).unwrap();
        assert_eq!(
            world.get::<SkinnedMesh>(mesh).unwrap().joints,
            vec![hips, spine]
        );
        // The copy is gone, its mesh and the body's own joints are not.
        assert!(world.get_entity(their_hips).is_err());
        assert!(world.get_entity(their_spine).is_err());
        assert!(world.get_entity(mesh).is_ok());
        assert!(world.get_entity(spine).is_ok());
    }

    #[test]
    fn a_skinned_look_is_cut_to_the_joints_it_uses_once_for_every_wearer() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.insert_resource(Assets::<Mesh>::default());
        world.insert_resource(Assets::<SkinnedMeshInverseBindposes>::default());
        world.init_resource::<Trimmed>();
        let names = ["j0", "j1", "j2", "j3", "j4"];
        let body = world.spawn(Name::new("body")).id();
        let ours: Vec<Entity> = names
            .iter()
            .map(|name| world.spawn((Name::new(*name), ChildOf(body))).id())
            .collect();
        // Five joints, each bind pose told apart by its own offset. Joints 1 and 3 carry
        // weight; joint 4, and one past the last, are named by slots with none.
        let poses: Vec<Mat4> = (0..5)
            .map(|i| Mat4::from_translation(Vec3::X * i as f32))
            .collect();
        let posed = world
            .resource_mut::<Assets<SkinnedMeshInverseBindposes>>()
            .add(SkinnedMeshInverseBindposes::from(poses.clone()));
        let mesh = world.resource_mut::<Assets<Mesh>>().add(
            Mesh::new(
                bevy::mesh::PrimitiveTopology::TriangleList,
                bevy::asset::RenderAssetUsages::default(),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; 2])
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_JOINT_INDEX,
                VertexAttributeValues::Uint16x4(vec![[1, 3, 4, 9], [3, 0, 0, 0]]),
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_JOINT_WEIGHT,
                vec![[0.7f32, 0.3, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]],
            ),
        );
        // Two wearers of the same look, each with its file's copy of the skeleton.
        let mut skins = vec![];
        for _ in 0..2 {
            let look = world.spawn(Worn { body }).id();
            let armature = world.spawn(ChildOf(look)).id();
            let copy: Vec<Entity> = names
                .iter()
                .map(|name| world.spawn((Name::new(*name), ChildOf(armature))).id())
                .collect();
            let skin = world
                .spawn((
                    Mesh3d(mesh.clone()),
                    SkinnedMesh {
                        inverse_bindposes: posed.clone(),
                        joints: copy,
                    },
                    ChildOf(armature),
                ))
                .id();
            world.run_system_once_with(dress, (look, body)).unwrap();
            skins.push(skin);
        }
        for skin in skins {
            let skin = world.get::<SkinnedMesh>(skin).unwrap();
            assert_eq!(skin.joints, vec![ours[1], ours[3]]);
            let kept = world
                .resource::<Assets<SkinnedMeshInverseBindposes>>()
                .get(&skin.inverse_bindposes)
                .unwrap()
                .to_vec();
            assert_eq!(kept, vec![poses[1], poses[3]]);
        }
        // Numbered among the joints kept, an unused slot at the first; cut once, not twice.
        let indices = world
            .resource::<Assets<Mesh>>()
            .get(&mesh)
            .unwrap()
            .attribute(Mesh::ATTRIBUTE_JOINT_INDEX)
            .cloned();
        assert_eq!(
            indices,
            Some(VertexAttributeValues::Uint16x4(vec![
                [0, 1, 0, 0],
                [1, 0, 0, 0]
            ]))
        );
    }

    #[test]
    fn a_rigid_look_hangs_on_the_socket_it_names_and_swings_as_it_says() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let body = world.spawn(Name::new("body")).id();
        let hips = world.spawn((Name::new(HIPS), ChildOf(body))).id();
        world.spawn((Name::new("socket.back"), ChildOf(hips)));
        let belt = world
            .spawn((Name::new("socket.belt.R"), ChildOf(hips)))
            .id();
        let look = world.spawn((Worn { body }, ChildOf(body))).id();
        let holder = world.spawn((Name::new("quiver"), ChildOf(look))).id();
        // As a rigid look's file carries it (`carry_gear.py`): on the part that swings.
        let part = world
            .spawn((
                GltfExtras {
                    value: serde_json::json!({
                        "socket": "socket.belt.R",
                        "covers": [3, 98],
                        "swing": {"tail": [0.0, -0.2, 0.0], "stiffness": 0.2, "drag": 0.1, "gravity": 0.6},
                        "morphs": {"heavy": [0.0, 0.0, -0.04], "thin": [0.02, 0.0, 0.0]}
                    })
                    .to_string(),
                },
                ChildOf(holder),
            ))
            .id();
        world.run_system_once_with(dress, (look, body)).unwrap();
        assert_eq!(world.get::<ChildOf>(look).unwrap().parent(), belt);
        assert_eq!(world.get::<Covers>(look).unwrap().0, vec![3, 98]);
        let swing = world.get::<crate::swing::Swing>(part).unwrap();
        assert_eq!(swing.spring.tail, [0.0, -0.2, 0.0]);
        assert_eq!(swing.spring.stiffness, 0.2);
        assert!(world.get::<crate::swing::Swing>(holder).is_none());
        let shifts = world.get::<Shifts>(look).unwrap();
        assert_eq!(shifts.0.get("thin"), Some(&Vec3::new(0.02, 0.0, 0.0)));
        assert_eq!(shifts.0.len(), 2);
    }

    /// The base body's file, read without Bevy.
    fn base() -> serde_json::Value {
        crate::model_files::json(crate::appearance::BASE)
    }

    #[test]
    fn what_this_module_names_is_on_the_base_body() {
        let base = base();
        let nodes = crate::model_files::nodes(&base);
        for node in [HIPS, HAND_R] {
            assert!(nodes.iter().any(|n| n == node), "no {node}");
        }
        // A clip named wrong is never carried in, and nothing says so.
        let clips = crate::model_files::animations(&base);
        for clip in CARRIED_IN {
            assert!(clips.iter().any(|c| c == clip), "no clip {clip}");
        }
    }

    #[test]
    fn every_worn_look_fits_the_base_body() {
        let base = base();
        let nodes = crate::model_files::nodes(&base);
        let morphs = crate::model_files::morphs(&base);
        let looks = crate::model_files::glbs("models/gear");
        assert!(!looks.is_empty());
        for file in &looks {
            let json = crate::model_files::json(file);
            // Its morphs follow the body's by name; one the body lacks stays at nothing.
            for morph in crate::model_files::morphs(&json) {
                assert!(morphs.contains(&morph), "{file}: the body has no {morph}");
            }
            let skinned = json["skins"].as_array().is_some_and(|s| !s.is_empty());
            if skinned {
                // Its joints are swapped for the body's by name.
                for joint in crate::model_files::joints(&json) {
                    assert!(nodes.contains(&joint), "{file}: the body has no {joint}");
                }
            } else {
                // A rigid look hangs on the socket it names, or nowhere.
                let socket = crate::model_files::extra(&json, "socket");
                let socket = socket.as_ref().and_then(|s| s.as_str());
                assert!(
                    socket.is_some_and(|s| nodes.iter().any(|n| n == s)),
                    "{file}: names no socket the body has: {socket:?}"
                );
            }
            // What its extras say, as `dress` reads them, where they would otherwise be
            // dropped without a word.
            if let Some(shifts) = crate::model_files::extra(&json, "morphs") {
                let shifts =
                    Shifts::from_extras(shifts).unwrap_or_else(|| panic!("{file}: morphs"));
                for morph in shifts.0.keys() {
                    assert!(
                        morphs.contains(morph),
                        "{file}: shifts for no morph {morph}"
                    );
                }
            }
            if let Some(swing) = crate::model_files::extra(&json, "swing") {
                let spring = crate::swing::Springiness::from_extras(swing)
                    .unwrap_or_else(|| panic!("{file}: swing"));
                assert!(Vec3::from(spring.tail).length() > 0.0, "{file}: no tail");
            }
            if let Some(covers) = crate::model_files::extra(&json, "covers") {
                let covers: Vec<u32> =
                    serde_json::from_value(covers).unwrap_or_else(|e| panic!("{file}: {e}"));
                assert!(covers.iter().all(|patch| *patch < PATCHES), "{file}");
            }
        }
    }
}
