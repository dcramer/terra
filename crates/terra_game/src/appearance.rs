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
    transform::TransformSystems,
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
    mut held: Query<(Entity, &mut Transform), With<Held>>,
    new: Query<(), Added<Held>>,
    resized: Query<(), (With<Body>, Changed<Appearance>)>,
    bodies: Query<&Appearance, With<Body>>,
    parents: Query<&ChildOf>,
) {
    let any_resized = !resized.is_empty();
    for (entity, mut transform) in &mut held {
        if !any_resized && !new.contains(entity) {
            continue;
        }
        let height = parents
            .iter_ancestors(entity)
            .find_map(|each| bodies.get(each).ok())
            .map_or(1.0, |appearance| appearance.height);
        let scale = Vec3::splat(1.0 / height.max(0.01));
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
struct Moved(Vec<MovedNode>);

struct MovedNode {
    node: Entity,
    rest: Vec3,
    by: Vec3,
    held: Option<Vec3>,
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
        .add_systems(
            PostUpdate,
            hold_joints
                .after(bevy::app::AnimationSystems)
                .before(TransformSystems::Propagate),
        );
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
            transform.translation = from + node.by;
            node.held = Some(transform.translation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_files;

    #[test]
    fn a_body_is_as_tall_as_it_says_and_holds_things_at_their_own_size() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let body = world
            .spawn((
                Body,
                Appearance {
                    height: 1.2,
                    ..Appearance::default()
                },
                Transform::default(),
            ))
            .id();
        let hand = world.spawn((Transform::default(), ChildOf(body))).id();
        let held = world
            .spawn((Held::new(0.0), Transform::default(), ChildOf(hand)))
            .id();
        world.run_system_once(size_bodies).unwrap();
        world.run_system_once(keep_held_size).unwrap();
        assert_eq!(
            world.get::<Transform>(body).unwrap().scale,
            Vec3::splat(1.2)
        );
        assert!(
            (world.get::<Transform>(held).unwrap().scale - Vec3::splat(1.0 / 1.2)).length() < 1e-6
        );
    }

    #[test]
    fn templates_parse_and_name_their_morphs() {
        let templates = parse_templates(
            r#"{"male": {}, "female": {"morphs": {"female": 1.0}, "skin": "female"}}"#,
        )
        .unwrap();
        assert_eq!(templates["male"], Appearance::default());
        assert_eq!(templates["male"].height, 1.0);
        assert_eq!(templates["female"].weight("female"), 1.0);
        assert_eq!(templates["female"].skin.as_deref(), Some("female"));
    }

    #[test]
    fn the_shipped_templates_name_morphs_and_skins_there_are() {
        // Read as the game reads it, without the fallback that would leave no templates.
        let templates = parse_templates(include_str!(
            "../../../assets/models/character/templates.json"
        ))
        .unwrap();
        let morphs = model_files::morphs(&model_files::json(BASE));
        assert!(!morphs.is_empty());
        for (name, template) in &templates {
            // A morph the body lacks would be left out of its shape without a word.
            for (morph, weight) in &template.morphs {
                assert!(
                    morphs.contains(morph),
                    "{name}: the base body has no {morph}"
                );
                assert!(weight.is_finite(), "{name}: {morph}");
            }
            if let Some(skin) = &template.skin {
                let file = skin_file(skin);
                assert!(model_files::asset(&file).exists(), "{name}: no {file}");
            }
            assert!(template.height > 0.0, "{name}");
        }
        // The templates the game asks for by name: the player's (`play::figure::TEMPLATE`)
        // and the people stood in the world (`play::settlement`, `play::pooling`).
        for wanted in ["male", "female", "heavy", "thin", "strong"] {
            assert!(templates.contains_key(wanted), "no template {wanted}");
        }
    }

    #[test]
    fn the_base_body_says_how_each_of_its_morphs_moves_its_joints() {
        let base = model_files::json(BASE);
        let offsets = model_files::extras(&base)
            .iter()
            .find_map(|(_, extras)| offsets_in(&extras.to_string()))
            .expect("the base body's file carries no morph offsets that read");
        let morphs = model_files::morphs(&base);
        let nodes = model_files::nodes(&base);
        for morph in &morphs {
            assert!(offsets.contains_key(morph), "nothing moves for {morph}");
        }
        for (morph, moved) in &offsets {
            assert!(morphs.contains(morph), "offsets for {morph}, not a morph");
            for node in moved.keys() {
                assert!(nodes.contains(node), "{morph} moves {node}, not a node");
            }
        }
    }

    #[test]
    fn a_mix_is_each_morphs_value_by_its_weight() {
        let value = |morph: &str| match morph {
            "heavy" => Some(Vec3::new(0.0, 0.0, -0.04)),
            "strong" => Some(Vec3::new(0.0, 0.02, -0.02)),
            _ => None,
        };
        let mix = |weights: &[(&str, f32)]| {
            let weights: Vec<(String, f32)> =
                weights.iter().map(|(m, w)| (m.to_string(), *w)).collect();
            mix(&weights, value)
        };
        // Half heavy and fully strong: -0.02 and -0.02 back, 0.02 up; thin gives nothing.
        let both = mix(&[("heavy", 0.5), ("strong", 1.0), ("thin", 1.0)]);
        assert!(
            (both - Vec3::new(0.0, 0.02, -0.04)).length() < 1e-6,
            "{both}"
        );
        // A morph named twice counts twice, as `Appearance::weight` adds them.
        let twice = mix(&[("heavy", 0.25), ("heavy", 0.25)]);
        assert!(
            (twice - Vec3::new(0.0, 0.0, -0.02)).length() < 1e-6,
            "{twice}"
        );
        assert_eq!(mix(&[]), Vec3::ZERO);
    }

    #[test]
    fn morphs_mix_by_weight() {
        let offsets = parse_offsets(
            r#"{"heavy": {"socket.back": [0.0, 0.0, -0.04]}, "female": {"socket.back": [0.0, -0.1, 0.0], "Head": [0.0, -0.1, 0.0]}}"#,
        )
        .unwrap();
        let appearance = Appearance {
            morphs: vec![("heavy".into(), 0.5), ("female".into(), 1.0)],
            ..Appearance::default()
        };
        let moved = mixed(&offsets, &appearance);
        assert!((moved["socket.back"] - Vec3::new(0.0, -0.1, -0.02)).length() < 1e-6);
        assert!((moved["Head"] - Vec3::new(0.0, -0.1, 0.0)).length() < 1e-6);
    }

    /// Where skinning draws `vertex` (in the mesh's space) of a mesh whose space is turned
    /// `model_from_mesh` in the model's, bound to a joint standing at `joint` in the model,
    /// before and after the joint moves `by` and the mesh is bound again for it.
    fn drawn_before_and_after(
        model_from_mesh: Quat,
        joint: Mat4,
        by: Vec3,
        rebound: impl Fn(Mat4) -> Mat4,
        vertex: Vec3,
    ) -> (Vec3, Vec3) {
        // A bind pose takes the mesh's space into the joint's at rest.
        let pose = (Mat4::from_quat(model_from_mesh).inverse() * joint).inverse();
        let moved = Mat4::from_translation(by) * joint;
        (
            (joint * pose).transform_point3(vertex),
            (moved * rebound(pose)).transform_point3(vertex),
        )
    }

    #[test]
    fn a_moved_joint_bound_again_leaves_its_skin_where_it_was() {
        // Skinning draws a vertex at joint times bind pose times vertex. Moved and bound again,
        // a joint must draw the skin where it was: the morph, not the joint, moves the skin.
        // So in a mesh as the model has it, and in meshes turned in the model.
        let joint = Mat4::from_translation(Vec3::new(0.2, 1.0, -0.1)) * Mat4::from_rotation_z(0.3);
        let by = Vec3::new(0.05, -0.1, 0.02);
        for model_from_mesh in [
            Quat::IDENTITY,
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Quat::from_rotation_x(0.7),
        ] {
            let mesh_from_model = Mat3::from_quat(model_from_mesh.inverse());
            for vertex in [
                Vec3::ZERO,
                Vec3::new(0.2, 1.3, 0.0),
                Vec3::new(-0.4, 0.1, 0.6),
            ] {
                let (before, after) = drawn_before_and_after(
                    model_from_mesh,
                    joint,
                    by,
                    |pose| moved_bindposes(&[pose], &[by], mesh_from_model)[0],
                    vertex,
                );
                assert!(
                    before.distance(after) < 1e-5,
                    "{model_from_mesh}: {before} became {after}"
                );
            }
        }
    }

    #[test]
    fn a_joint_held_by_its_appearance_moves_from_where_the_animation_put_it_or_its_rest() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let rest = Vec3::new(0.0, 1.0, 0.0);
        let by = Vec3::new(0.0, -0.1, 0.0);
        let joint = world.spawn(Transform::from_translation(rest)).id();
        world.spawn(Moved(vec![MovedNode {
            node: joint,
            rest,
            by,
            held: Some(rest),
        }]));
        let frame = |world: &mut World, animated: Option<Vec3>| {
            if let Some(at) = animated {
                world.get_mut::<Transform>(joint).unwrap().translation = at;
            }
            world.run_system_once(hold_joints).unwrap();
            world.get::<Transform>(joint).unwrap().translation
        };
        // Frames no clip sets it: moved from its rest, and never further each frame.
        assert!(frame(&mut world, None).distance(rest + by) < 1e-6);
        assert!(frame(&mut world, None).distance(rest + by) < 1e-6);
        // A clip sets it: moved from there.
        let animated = Vec3::new(0.0, 1.2, 0.05);
        assert!(frame(&mut world, Some(animated)).distance(animated + by) < 1e-6);
        assert!(frame(&mut world, None).distance(rest + by) < 1e-6);
    }

    #[test]
    fn a_rest_is_chained_up_to_the_root_and_not_past_it() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let arm = world.spawn(ChildOf(root)).id();
        let hand = world.spawn(ChildOf(arm)).id();
        let rest = Rest(HashMap::from([
            // The root's own rest places the model in the world, and is left out.
            (root, Transform::from_xyz(100.0, 0.0, 0.0)),
            (
                arm,
                Transform::from_xyz(0.0, 1.0, 0.0)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ),
            (hand, Transform::from_xyz(1.0, 0.0, 0.0)),
        ]));
        let at = world
            .run_system_once(move |parents: Query<&ChildOf>| {
                rest_from_root(hand, root, &rest, &parents)
            })
            .unwrap();
        // The hand is 1 along the arm's X, which the quarter turn points up: 1 above the
        // arm, 2 above the root, and turned as the arm is.
        assert!(
            Vec3::from(at.translation).distance(Vec3::Y * 2.0) < 1e-6,
            "{at:?}"
        );
        assert!(Vec3::from(at.matrix3 * Vec3A::X).distance(Vec3::Y) < 1e-6);
    }

    #[test]
    fn a_worn_look_takes_its_bodys_shape() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<Shared>();
        world.insert_resource(Assets::<Mesh>::default());
        world.insert_resource(Assets::<SkinnedMeshInverseBindposes>::default());
        let weights = vec![("heavy".to_string(), 0.5), ("thin".to_string(), 0.25)];
        let by = Vec3::new(0.0, -0.1, 0.05);
        let body = world
            .spawn(Shape {
                moved: HashMap::from([("Head".to_string(), by)]),
                weights_by_name: weights,
            })
            .id();
        let head = world.spawn((Name::new("Head"), ChildOf(body))).id();
        let head_at = Mat4::from_translation(Vec3::new(0.0, 1.6, 0.05));

        // A rigid look hung on a socket, shifted beyond it by each morph.
        let hung = world
            .spawn((
                Worn { body },
                Covers(vec![]),
                Rest(HashMap::new()),
                Shifts(HashMap::from([
                    ("heavy".to_string(), Vec3::new(0.0, 0.0, -0.04)),
                    ("thin".to_string(), Vec3::new(0.02, 0.0, 0.0)),
                ])),
                Transform::default(),
            ))
            .id();

        // A skinned look whose mesh is turned a quarter in the look, with morphs of its own
        // in an order of its own.
        let turned = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let pose = (Mat4::from_quat(turned).inverse() * head_at).inverse();
        let poses = world
            .resource_mut::<Assets<SkinnedMeshInverseBindposes>>()
            .add(SkinnedMeshInverseBindposes::from(vec![pose]));
        let shape = world.resource_mut::<Assets<Mesh>>().add(
            Mesh::new(
                bevy::mesh::PrimitiveTopology::TriangleList,
                bevy::asset::RenderAssetUsages::default(),
            )
            .with_morph_target_names(vec![
                "thin".into(),
                "female".into(),
                "heavy".into(),
            ]),
        );
        let skinned = world.spawn(Worn { body }).id();
        let mesh = world
            .spawn((
                SkinnedMesh {
                    inverse_bindposes: poses.clone(),
                    joints: vec![head],
                },
                MorphWeights::new(vec![0.0; 3], Some(shape)).unwrap(),
                ChildOf(skinned),
            ))
            .id();
        world.entity_mut(skinned).insert((
            Covers(vec![]),
            Rest(HashMap::from([(mesh, Transform::from_rotation(turned))])),
        ));

        world.run_system_once(fit_worn).unwrap();

        // Half the heavy shift and a quarter of the thin.
        let shifted = world.get::<Transform>(hung).unwrap().translation;
        assert!(
            shifted.distance(Vec3::new(0.005, 0.0, -0.02)) < 1e-6,
            "{shifted}"
        );
        // Its morphs by name, whatever their order.
        let morphs = world.get::<MorphWeights>(mesh).unwrap().weights().to_vec();
        assert_eq!(morphs, vec![0.25, 0.0, 0.5]);
        // Bound again for the moved head, in the mesh's own turned space: the skin is drawn
        // where it was, before the morph moves it.
        let bound = world
            .get::<SkinnedMesh>(mesh)
            .unwrap()
            .inverse_bindposes
            .clone();
        assert_ne!(bound.id(), poses.id());
        let rebound = world
            .resource::<Assets<SkinnedMeshInverseBindposes>>()
            .get(&bound)
            .unwrap()[0];
        for vertex in [Vec3::ZERO, Vec3::new(0.1, 1.5, -0.2)] {
            let (before, after) = drawn_before_and_after(turned, head_at, by, |_| rebound, vertex);
            assert!(before.distance(after) < 1e-5, "{before} became {after}");
        }
    }
}
