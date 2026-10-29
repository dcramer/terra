//! What a body looks like (D112, docs/characters.md): every character stands on the one base
//! body, `models/character/base.glb`, shaped by how much of each of its morphs it has, and
//! wearing a skin. Male and female, heavy, thin and strong are morphs of the same mesh, so the
//! base's skeleton, weights and skin patches serve every body, and every worn look, made once,
//! carries the same morphs and follows (`tools/characters/build_bodies.sh` makes them).
//!
//! A morph changes the skeleton as well as the skin: a woman's joints and a heavier man's
//! sockets are not the base's. The body's file says, for each morph, how far each joint and
//! socket moves (`{"morphs": {M: {node: [x, y, z]}}}` in the skeleton node's glTF extras). For
//! a body's mix of morphs this module moves each joint's and socket's rest position, keeps it
//! there under animation (clips turn joints; they do not place them), and gives the body's
//! skin, and that of everything worn on it, bind poses for those joints. All of it is worked
//! out when a body appears or its appearance changes, and shared between bodies that look the
//! same; nothing is done per frame but restoring the moved joints after the animation.
//!
//! A template (D111) is a named appearance, from `models/character/templates.json`.

use crate::character::{Body, BodyReady, Covers, Held, Shifts, Skin, Worn};
use bevy::{
    gltf::GltfExtras,
    math::{Affine3A, Vec3A},
    mesh::{
        morph::MorphWeights,
        skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    },
    prelude::*,
    world_serialization::WorldInstanceReady,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The body every character stands on.
pub const BASE: &str = "models/character/base.glb";

/// How a body is shaped and skinned (D111): how much of each of the base's morphs (by name)
/// it has, its height, and which painted skin it wears (`models/character/skins/<skin>.png`;
/// none is the base's). On a body's root (with `Body`).
#[derive(Component, Clone, Debug, PartialEq, Deserialize)]
pub struct Appearance {
    #[serde(default)]
    pub morphs: Vec<(String, f32)>,
    #[serde(default)]
    pub skin: Option<String>,
    /// Its height against the base body's (1.80 m): a uniform scale of the whole character,
    /// so what it wears, its sockets and its clips scale with it and nothing is fitted per
    /// height. What it holds keeps its own size (`keep_held_size`).
    #[serde(default = "full_height")]
    pub height: f32,
}

fn full_height() -> f32 {
    1.0
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            morphs: Vec::new(),
            skin: None,
            height: full_height(),
        }
    }
}

impl Appearance {
    /// A template by name, from `models/character/templates.json`.
    pub fn template(name: &str) -> Option<Appearance> {
        templates().get(name).cloned()
    }

    /// How much of a morph it has.
    pub fn weight(&self, morph: &str) -> f32 {
        self.morphs
            .iter()
            .filter(|(name, _)| name == morph)
            .map(|(_, weight)| weight)
            .sum()
    }

    /// The same appearance, as a key bodies that look alike share things under.
    fn key(&self) -> (Vec<(String, u32)>, Option<String>) {
        let mut morphs: Vec<(String, u32)> = self
            .morphs
            .iter()
            .filter(|(_, weight)| *weight != 0.0)
            .map(|(name, weight)| (name.clone(), weight.to_bits()))
            .collect();
        morphs.sort();
        (morphs, self.skin.clone())
    }
}

#[derive(Deserialize)]
struct TemplateFile {
    #[serde(default)]
    morphs: HashMap<String, f32>,
    #[serde(default)]
    skin: Option<String>,
    #[serde(default = "full_height")]
    height: f32,
}

/// Every template, read once from the data file the game ships with.
pub fn templates() -> &'static HashMap<String, Appearance> {
    static TEMPLATES: OnceLock<HashMap<String, Appearance>> = OnceLock::new();
    TEMPLATES.get_or_init(|| {
        parse_templates(include_str!(
            "../../../assets/models/character/templates.json"
        ))
        .unwrap_or_else(|error| {
            error!("models/character/templates.json: {error}");
            HashMap::new()
        })
    })
}

fn parse_templates(text: &str) -> Result<HashMap<String, Appearance>, serde_json::Error> {
    let file: HashMap<String, TemplateFile> = serde_json::from_str(text)?;
    Ok(file
        .into_iter()
        .map(|(name, template)| {
            let mut morphs: Vec<(String, f32)> = template.morphs.into_iter().collect();
            morphs.sort_by(|a, b| a.0.cmp(&b.0));
            (
                name,
                Appearance {
                    morphs,
                    skin: template.skin,
                    height: template.height,
                },
            )
        })
        .collect())
}

/// Each morph's offsets, as the body's file gives them: morph, then joint or socket, then how
/// far it moves in the model's space.
type Offsets = HashMap<String, HashMap<String, Vec3>>;

fn parse_offsets(text: &str) -> Option<Offsets> {
    let raw: HashMap<String, HashMap<String, [f32; 3]>> = serde_json::from_str(text).ok()?;
    Some(
        raw.into_iter()
            .map(|(morph, nodes)| {
                (
                    morph,
                    nodes
                        .into_iter()
                        .map(|(node, at)| (node, Vec3::from_array(at)))
                        .collect(),
                )
            })
            .collect(),
    )
}

/// The morph offsets a node's glTF extras carry, as the body's file writes them: a JSON
/// string under `"morphs"`.
fn offsets_in(extras: &str) -> Option<Offsets> {
    let value: serde_json::Value = serde_json::from_str(extras).ok()?;
    parse_offsets(value.get("morphs")?.as_str()?)
}

/// What a mix of morphs (name and weight) makes of something each morph gives at full
/// weight: each morph's by its weight, summed. A morph that gives none adds nothing. How
/// far a joint moves, a hung look shifts and a body is lifted are all mixed so.
pub fn mix<T>(weights: &[(String, f32)], value: impl Fn(&str) -> Option<T>) -> T
where
    T: std::ops::Mul<f32, Output = T> + std::iter::Sum,
{
    weights
        .iter()
        .filter_map(|(morph, weight)| Some(value(morph)? * *weight))
        .sum()
}

/// How far each joint and socket moves, in the model's space, for a mix of morphs.
fn mixed(offsets: &Offsets, appearance: &Appearance) -> HashMap<String, Vec3> {
    let nodes: std::collections::HashSet<&String> = appearance
        .morphs
        .iter()
        .filter_map(|(morph, _)| offsets.get(morph))
        .flat_map(HashMap::keys)
        .collect();
    nodes
        .into_iter()
        .map(|node| {
            let by = mix(&appearance.morphs, |morph| {
                offsets.get(morph)?.get(node).copied()
            });
            (node.clone(), by)
        })
        .collect()
}

/// Where a painted skin is, under `assets/`, by its name.
fn skin_file(name: &str) -> String {
    format!("models/character/skins/{name}.png")
}

/// Each body as tall as its appearance says: the scale of its root, the whole character.
fn size_bodies(
    mut bodies: Query<(&Appearance, &mut Transform), (With<Body>, Changed<Appearance>)>,
) {
    for (appearance, mut transform) in &mut bodies {
        let scale = Vec3::splat(appearance.height.max(0.01));
        if transform.scale != scale {
            transform.scale = scale;
        }
    }
}

/// What a body holds keeps its own size whatever the body's height: a tall person's axe is
/// no bigger. Held where the hand's socket is, scaled back by the height of the body above it.
fn keep_held_size(
    mut held: Query<(Entity, &Held, &mut Transform)>,
    new: Query<(), Changed<Held>>,
    resized: Query<(), (With<Body>, Changed<Appearance>)>,
    bodies: Query<&Appearance, With<Body>>,
    parents: Query<&ChildOf>,
) {
    let any_resized = !resized.is_empty();
    for (entity, look, mut transform) in &mut held {
        if !any_resized && !new.contains(entity) {
            continue;
        }
        let height = parents
            .iter_ancestors(entity)
            .find_map(|each| bodies.get(each).ok())
            .map_or(1.0, |appearance| appearance.height);
        let scale = Vec3::splat(look.fit / height.max(0.01));
        if transform.scale != scale {
            transform.scale = scale;
        }
    }
}

/// Bind poses for joints moved by `moved` (in the model's space): a joint's bind pose maps
/// the mesh's space into the joint's, so a joint moved by `d` takes the mesh `d` further
/// before it, `inverse_bindpose * translation(-d)`, with `d` in the mesh's own space.
fn moved_bindposes(bindposes: &[Mat4], moved: &[Vec3], mesh_from_model: Mat3) -> Vec<Mat4> {
    bindposes
        .iter()
        .zip(moved)
        .map(|(pose, by)| *pose * Mat4::from_translation(-(mesh_from_model * *by)))
        .collect()
}

/// The rest transform of everything in a body or a look, from its root's children down,
/// recorded when it appears, before anything animates it.
#[derive(Component)]
struct Rest(HashMap<Entity, Transform>);

/// A body's joints and sockets that its appearance moves: each, its rest position, and how
/// far it moves in its parent's space. `held` is what was last written, to tell a frame the
/// animation set it from one it did not.
#[derive(Component, Default)]
pub(crate) struct Moved(pub(crate) Vec<MovedNode>);

pub(crate) struct MovedNode {
    pub(crate) node: Entity,
    pub(crate) rest: Vec3,
    pub(crate) by: Vec3,
    pub(crate) held: Option<Vec3>,
}

/// A body's appearance as worked out: how far each joint and socket moves (model space), and
/// its morph weights by name, which its looks take.
#[derive(Component, Default)]
pub struct Shape {
    moved: HashMap<String, Vec3>,
    weights_by_name: Vec<(String, f32)>,
}

/// Bind poses and skins made for an appearance, shared by every body that looks the same.
#[derive(Resource, Default)]
struct Shared {
    bindposes: HashMap<
        (AssetId<SkinnedMeshInverseBindposes>, String),
        Handle<SkinnedMeshInverseBindposes>,
    >,
    skins: HashMap<(AssetId<StandardMaterial>, String), Handle<StandardMaterial>>,
}

/// With this resource, each body prints where its joints and sockets stand at rest once it
/// is shaped (`JOINTS` lines, in Blender's axes as the export's `SOCKETS` lines are), to
/// check the game shapes a body as the tools do (the workbench's `--joints`).
#[derive(Resource)]
pub struct ReportJoints;

pub fn plugin(app: &mut App) {
    app.init_resource::<Shared>()
        .add_observer(record_rest)
        .add_systems(
            Update,
            (
                size_bodies,
                keep_held_size,
                shape_bodies,
                fit_worn,
                report_joints.run_if(resource_exists::<ReportJoints>),
            )
                .chain(),
        )
        .configure_sets(PostUpdate, crate::fitted())
        .add_systems(PostUpdate, hold_joints.in_set(crate::Fitted));
}

/// Records the rest pose of each body and look as it appears.
fn record_rest(
    ready: On<WorldInstanceReady>,
    wanted: Query<(), Or<(With<Body>, With<Worn>)>>,
    children: Query<&Children>,
    transforms: Query<&Transform>,
    mut commands: Commands,
) {
    if !wanted.contains(ready.entity) {
        return;
    }
    let rest = children
        .iter_descendants(ready.entity)
        .filter_map(|each| Some((each, *transforms.get(each).ok()?)))
        .collect();
    commands.entity(ready.entity).insert(Rest(rest));
}

/// Where `entity` stands at rest in its root's space: its rest transforms chained up to the
/// root (whose own transform places the model in the world and is left out).
fn rest_from_root(
    entity: Entity,
    root: Entity,
    rest: &Rest,
    parents: &Query<&ChildOf>,
) -> Affine3A {
    let mut at = Affine3A::IDENTITY;
    let mut each = entity;
    while each != root {
        let Some(local) = rest.0.get(&each) else {
            break;
        };
        at = local.compute_affine() * at;
        let Ok(parent) = parents.get(each) else {
            break;
        };
        each = parent.parent();
    }
    at
}

/// Works out each body's shape when it appears or its appearance changes.
#[allow(clippy::too_many_arguments)]
fn shape_bodies(
    mut commands: Commands,
    mut bodies: Query<
        (Entity, Option<&Appearance>, &Rest, &mut Skin),
        (
            With<BodyReady>,
            Or<(Added<BodyReady>, Changed<Appearance>, Added<Rest>)>,
        ),
    >,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    extras: Query<&GltfExtras>,
    mut weights: Query<&mut MorphWeights>,
    mut skins: Query<&mut SkinnedMesh>,
    transforms: Query<&Transform>,
    meshes: Res<Assets<Mesh>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
    mut shared: ResMut<Shared>,
) {
    for (root, appearance, rest, mut skin) in &mut bodies {
        let appearance = appearance.cloned().unwrap_or_default();
        let appearance = &appearance;
        let descendants: Vec<Entity> = children.iter_descendants(root).collect();
        let offsets = descendants
            .iter()
            .filter_map(|each| extras.get(*each).ok())
            .find_map(|extras| offsets_in(&extras.value))
            .unwrap_or_default();
        let moved = mixed(&offsets, appearance);
        let key = format!("{:?}", appearance.key());

        // The morph weights, by name, on the body's mesh.
        for each in &descendants {
            let Ok(mut node) = weights.get_mut(*each) else {
                continue;
            };
            let names_of = node
                .first_mesh()
                .and_then(|mesh| meshes.get(mesh))
                .and_then(|mesh| mesh.morph_target_names().map(<[String]>::to_vec))
                .unwrap_or_default();
            for (slot, name) in node.weights_mut().iter_mut().zip(&names_of) {
                *slot = appearance.weight(name);
            }
        }

        // Each moved joint and socket, in its parent's space.
        let mut nodes = Vec::new();
        for each in &descendants {
            let Ok(name) = names.get(*each) else {
                continue;
            };
            let Some(by) = moved.get(name.as_str()) else {
                continue;
            };
            let parent = parents.get(*each).map(ChildOf::parent).ok();
            let parent_by = parent
                .and_then(|p| names.get(p).ok())
                .and_then(|n| moved.get(n.as_str()))
                .copied()
                .unwrap_or(Vec3::ZERO);
            let parent_rest = parent.map_or(Affine3A::IDENTITY, |p| {
                rest_from_root(p, root, rest, &parents)
            });
            let local = parent_rest.matrix3.inverse() * Vec3A::from(*by - parent_by);
            nodes.push(MovedNode {
                node: *each,
                rest: rest.0.get(each).map_or(Vec3::ZERO, |t| t.translation),
                by: Vec3::from(local),
                // As it stands now: if the animation does not set it, it is put back at its
                // rest, not left where an earlier appearance moved it.
                held: transforms.get(*each).ok().map(|t| t.translation),
            });
        }

        // The body's skin, bound to its moved joints.
        for each in &descendants {
            let Ok(mut mesh) = skins.get_mut(*each) else {
                continue;
            };
            let mesh_from_model = Mat3::from(
                rest_from_root(*each, root, rest, &parents)
                    .matrix3
                    .inverse(),
            );
            rebind(
                &mut mesh,
                &moved,
                mesh_from_model,
                &names,
                &key,
                &mut bindposes,
                &mut shared,
            );
        }

        // Its painted skin, on the body's own materials, which the body mask is built from.
        let skin_image: Option<Handle<Image>> = appearance
            .skin
            .as_ref()
            .map(|name| assets.load(skin_file(name)));
        let mut parts = std::mem::take(&mut skin.parts);
        for ((_, material), original) in parts.iter_mut().zip(skin.original.clone()) {
            let Some(image) = &skin_image else {
                *material = original;
                continue;
            };
            let cache = (original.id(), appearance.skin.clone().unwrap_or_default());
            if let Some(handle) = shared.skins.get(&cache) {
                *material = handle.clone();
                continue;
            }
            let Some(mut painted) = materials.get(&original).cloned() else {
                continue;
            };
            painted.base_color_texture = Some(image.clone());
            let handle = materials.add(painted);
            shared.skins.insert(cache, handle.clone());
            *material = handle;
        }
        skin.parts = parts;
        skin.showing = None;

        commands.entity(root).insert((
            Moved(nodes),
            Shape {
                moved,
                weights_by_name: appearance.morphs.clone(),
            },
        ));
    }
}

/// Gives a skinned mesh bind poses for its joints as `moved` moves them, shared by every mesh
/// with the same bind poses and the same moves.
fn rebind(
    mesh: &mut SkinnedMesh,
    moved: &HashMap<String, Vec3>,
    mesh_from_model: Mat3,
    names: &Query<&Name>,
    key: &str,
    bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
    shared: &mut Shared,
) {
    let original = shared
        .bindposes
        .iter()
        .find(|(_, made)| made.id() == mesh.inverse_bindposes.id())
        .map(|((from, _), _)| *from)
        .unwrap_or(mesh.inverse_bindposes.id());
    let cache = (original, format!("{key}{mesh_from_model:?}"));
    if let Some(handle) = shared.bindposes.get(&cache) {
        mesh.inverse_bindposes = handle.clone();
        return;
    }
    let Some(poses) = bindposes.get(original) else {
        return;
    };
    let by: Vec<Vec3> = mesh
        .joints
        .iter()
        .map(|joint| {
            names
                .get(*joint)
                .ok()
                .and_then(|name| moved.get(name.as_str()))
                .copied()
                .unwrap_or(Vec3::ZERO)
        })
        .collect();
    let made = bindposes.add(SkinnedMeshInverseBindposes::from(moved_bindposes(
        poses,
        &by,
        mesh_from_model,
    )));
    shared.bindposes.insert(cache, made.clone());
    mesh.inverse_bindposes = made;
}

/// Each look worn on a shaped body takes the body's morph weights and bind poses for its moved
/// joints: when it is put on, and when the body's shape changes.
fn fit_worn(
    looks: Query<(Entity, &Worn, &Rest), With<Covers>>,
    new_looks: Query<(), Added<Covers>>,
    bodies: Query<(Entity, Ref<Shape>)>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    mut skins: Query<&mut SkinnedMesh>,
    mut own_weights: Query<&mut MorphWeights>,
    mut hung: Query<(&Shifts, &mut Transform)>,
    meshes: Res<Assets<Mesh>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut shared: ResMut<Shared>,
) {
    for (look, worn, rest) in &looks {
        let Ok((_, shape)) = bodies.get(worn.body) else {
            continue;
        };
        if !shape.is_changed() && !new_looks.contains(look) {
            continue;
        }
        // A look hung on a socket moves beyond its socket by its own shifts, mixed.
        if let Ok((shifts, mut transform)) = hung.get_mut(look) {
            transform.translation = shifts.mixed(&shape.weights_by_name);
        }
        let key = format!("{:?}", shape.weights_by_name);
        for each in children.iter_descendants(look) {
            if let Ok(mut mesh) = skins.get_mut(each) {
                let mesh_from_model =
                    Mat3::from(rest_from_root(each, look, rest, &parents).matrix3.inverse());
                rebind(
                    &mut mesh,
                    &shape.moved,
                    mesh_from_model,
                    &names,
                    &key,
                    &mut bindposes,
                    &mut shared,
                );
            }
            // Its morphs follow the body's, by name.
            if let Ok(mut weights) = own_weights.get_mut(each) {
                let names_of = weights
                    .first_mesh()
                    .and_then(|mesh| meshes.get(mesh))
                    .and_then(|mesh| mesh.morph_target_names().map(<[String]>::to_vec))
                    .unwrap_or_default();
                for (slot, name) in weights.weights_mut().iter_mut().zip(&names_of) {
                    *slot = shape
                        .weights_by_name
                        .iter()
                        .filter(|(n, _)| n == name)
                        .map(|(_, w)| w)
                        .sum();
                }
            }
        }
    }
}

/// Prints where a newly shaped body's sockets and main joints stand at rest, its appearance
/// applied: each node's rest transform, moved as the appearance moves it, chained to the root.
fn report_joints(
    bodies: Query<(Entity, &Rest, &Moved, Option<&Appearance>, Option<&Name>), Added<Shape>>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
) {
    for (root, rest, moved, appearance, label) in &bodies {
        let by: HashMap<Entity, Vec3> = moved.0.iter().map(|n| (n.node, n.by)).collect();
        let at = |entity: Entity| {
            let mut affine = Affine3A::IDENTITY;
            let mut each = entity;
            while each != root {
                let Some(local) = rest.0.get(&each) else {
                    break;
                };
                let mut local = *local;
                local.translation += by.get(&each).copied().unwrap_or(Vec3::ZERO);
                affine = local.compute_affine() * affine;
                let Ok(parent) = parents.get(each) else {
                    break;
                };
                each = parent.parent();
            }
            affine
        };
        let shape = appearance
            .map(|a| format!("{:?}", a.morphs))
            .unwrap_or_default();
        for each in children.iter_descendants(root) {
            let Ok(name) = names.get(each) else {
                continue;
            };
            let wanted = name.starts_with("socket.")
                || matches!(name.as_str(), "pelvis" | "Head" | "hand_r" | "foot_r");
            if !wanted {
                continue;
            }
            let affine = at(each);
            let p = affine.translation;
            // Blender's axes, as the export prints them: glTF's (x, y, z) is Blender's (x, -z, y).
            // A socket's turn too, as where its own +Y and +Z point: what hangs on it hangs by
            // both, so a turn wrong in either swings it round the socket.
            let (y, z) = (affine.matrix3.y_axis, affine.matrix3.z_axis);
            info!(
                "JOINTS {} {shape} {} ({:.3}, {:.3}, {:.3}) +Y ({:.2}, {:.2}, {:.2}) +Z ({:.2}, {:.2}, {:.2})",
                label.map_or("body", |n| n.as_str()),
                name.as_str(),
                p.x,
                -p.z,
                p.y,
                y.x,
                -y.z,
                y.y,
                z.x,
                -z.z,
                z.y
            );
        }
    }
}

/// Puts each moved joint and socket back where the body's appearance places it, after the
/// animation has posed the skeleton: clips turn joints and move the hips, but carry the
/// base's joint positions, which would undo the body's own.
fn hold_joints(mut bodies: Query<&mut Moved>, mut transforms: Query<&mut Transform>) {
    for mut moved in &mut bodies {
        for node in &mut moved.0 {
            let Ok(mut transform) = transforms.get_mut(node.node) else {
                continue;
            };
            // Unchanged since it was last written, the animation did not set it this frame:
            // it goes from its rest; otherwise from where the animation put it.
            let from = if node.held == Some(transform.translation) {
                node.rest
            } else {
                transform.translation
            };
            // Written only when it moves: a write marks the skeleton's tree dirty, and the
            // propagation then walks every bone of it (a town's still bodies, 4,900 a frame).
            let wanted = from + node.by;
            if transform.translation != wanted {
                transform.translation = wanted;
            }
            node.held = Some(wanted);
        }
    }
}

#[cfg(test)]
mod tests;
