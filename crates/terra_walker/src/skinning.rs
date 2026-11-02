//! Weight-blended skinning for every skinned mesh (`assets/shaders/skinning.wesl`): each vertex
//! skinned partly with Bevy's linear blend and partly with dual quaternions, as Maya's "weight
//! blended" skinning does. Our clips roll a forearm up to half a turn about its length, and
//! the linear blend collapses the skin across such a turn into a thin neck; dual quaternions
//! keep its volume, and a knee's or elbow's through a deep bend, but bulge across the wide
//! blend round the hips, where the linear blend does better (research/skin-deformation.md).
//! Which joints keep the linear blend is data, `models/character/skinning.json`, read by the
//! character tools too, which measure bodies skinned the same way.
//!
//! Bevy has no switch for this, so its own skinning shader, the WESL module
//! `bevy_pbr::render::skinning`, is replaced by ours with the same functions and bindings:
//! every mesh shader that imports it, Bevy's and our body's alike, then skins this way. Bevy
//! loads its shader from an embedded file, which names the module, so ours is put in its place
//! under the same path once that has loaded, and again if it ever reloads.
//!
//! A vertex's share of dual quaternions travels in its joint weights, which then sum to one
//! plus the share (`carry_share`): no new vertex attribute, so every material's pipeline is
//! left as Bevy makes it. Each skinned mesh is given its shares once, when it first appears,
//! from the names of its joints; a mesh never given them has weights summing to one, and is
//! skinned with the linear blend alone, as Bevy would.

use bevy::{
    mesh::{MeshVertexAttribute, VertexAttributeValues, skinning::SkinnedMesh},
    prelude::*,
    shader::Shader,
};
use serde::Deserialize;
use std::collections::HashSet;

/// Where Bevy keeps its skinning shader (`bevy_pbr/src/render/skinning.wesl`); a shader at
/// this path is the module `bevy_pbr::render::skinning`.
const BEVY_SKINNING: &str = "embedded://bevy_pbr/render/skinning.wesl";
const OURS: &str = include_str!("../../../assets/shaders/skinning.wesl");
/// A line only our shader has.
const MARK: &str = "TERRA_DUAL_QUATERNION_SKINNING";

/// Bevy's skinning shader, held so it stays loaded under our source.
#[derive(Resource)]
struct Skinning(Handle<Shader>);

/// Which joints keep the linear blend, and how strongly (`models/character/skinning.json`).
#[derive(Resource, Deserialize, Clone, Debug, PartialEq)]
pub struct Blend {
    /// A vertex these joints move is skinned with the linear blend by `linear_gain` times
    /// their weight on it, and with dual quaternions for the rest.
    pub linear_joints: Vec<String>,
    pub linear_gain: f32,
}

impl Blend {
    fn read() -> Blend {
        serde_json::from_str(include_str!(
            "../../../assets/models/character/skinning.json"
        ))
        .unwrap_or_else(|error| {
            error!("models/character/skinning.json: {error}");
            Blend {
                linear_joints: Vec::new(),
                linear_gain: 0.0,
            }
        })
    }

    /// A vertex's share of dual quaternions, from its weights (four, or eight, D601) and
    /// whether each of its joints is one that keeps the linear blend.
    pub fn share(&self, weights: &[f32], linear: &[bool]) -> f32 {
        let total: f32 = weights.iter().sum();
        if total <= 0.0 {
            return 0.0;
        }
        let on_linear: f32 = weights
            .iter()
            .zip(linear)
            .filter(|(_, linear)| **linear)
            .map(|(w, _)| w)
            .sum::<f32>()
            / total;
        (1.0 - self.linear_gain * on_linear).clamp(0.0, 1.0)
    }
}

/// Puts `share` into a vertex's `weights` (all of them, four or eight), normalised, so that
/// they sum to one plus it: the shader reads the share back as the sum less one, and divides
/// it out.
pub fn carry_share(weights: &mut [f32], share: f32) {
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return;
    }
    let k = (1.0 + share.clamp(0.0, 1.0)) / total;
    for w in weights {
        *w *= k;
    }
}

/// The meshes already given their shares: a mesh shared by many bodies, once.
#[derive(Resource, Default)]
struct Blended(HashSet<AssetId<Mesh>>);

/// On a skinned mesh whose shares are still to be given (its mesh may not have loaded yet).
#[derive(Component)]
struct AwaitingBlend;

pub fn plugin(app: &mut App) {
    let linear = app
        .world()
        .get_resource::<terra_world::shell::Options>()
        .is_some_and(|o| o.linear_skinning);
    if linear {
        info!("skinning: Bevy's linear blend (--linear-skinning)");
        return;
    }
    let handle = app.world().resource::<AssetServer>().load(BEVY_SKINNING);
    app.insert_resource(Skinning(handle))
        .insert_resource(Blend::read())
        .init_resource::<Blended>()
        .add_systems(PostUpdate, replace_skinning)
        .add_systems(Update, (wait_for_blend, blend_meshes).chain());
}

/// Puts our source in place of Bevy's whenever Bevy's (re)appears.
fn replace_skinning(
    skinning: Res<Skinning>,
    mut events: MessageReader<AssetEvent<Shader>>,
    mut shaders: ResMut<Assets<Shader>>,
) {
    let id = skinning.0.id();
    let touched = events.read().any(|e| match e {
        AssetEvent::Added { id: i }
        | AssetEvent::Modified { id: i }
        | AssetEvent::LoadedWithDependencies { id: i } => *i == id,
        _ => false,
    });
    if !touched {
        return;
    }
    let Some(shader) = shaders.get(id) else {
        return;
    };
    if is_ours(shader) {
        return;
    }
    let path = shader.path.clone();
    match shaders.insert(id, Shader::from_wesl(OURS, path)) {
        Ok(()) => info!("skinning: weight-blended, in place of Bevy's linear blend"),
        Err(problem) => error!("could not replace the skinning shader: {problem}"),
    }
}

fn is_ours(shader: &Shader) -> bool {
    matches!(&shader.source, bevy::shader::Source::Wesl(source) if source.contains(MARK))
}

/// Every skinned mesh as it appears is marked to be given its shares.
fn wait_for_blend(new: Query<Entity, Added<SkinnedMesh>>, mut commands: Commands) {
    for entity in &new {
        commands.entity(entity).insert(AwaitingBlend);
    }
}

/// Gives each waiting skinned mesh's mesh its shares, once per mesh, from its joints' names.
fn blend_meshes(
    waiting: Query<(Entity, &Mesh3d, &SkinnedMesh), With<AwaitingBlend>>,
    names: Query<&Name>,
    blend: Res<Blend>,
    mut blended: ResMut<Blended>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
) {
    for (entity, mesh3d, skinned) in &waiting {
        let id = mesh3d.0.id();
        if blended.0.contains(&id) {
            commands.entity(entity).remove::<AwaitingBlend>();
            continue;
        }
        let linear: Vec<bool> = skinned
            .joints
            .iter()
            .map(|joint| {
                names
                    .get(*joint)
                    .is_ok_and(|n| blend.linear_joints.iter().any(|l| l == n.as_str()))
            })
            .collect();
        let Some(mut mesh) = meshes.get_mut(id) else {
            continue; // not loaded yet: next frame
        };
        if let Some((dual, count)) = give_shares(&mut mesh, &blend, &linear) {
            info!("skinning: {dual} of {count} vertices of a mesh mostly dual quaternion");
        }
        blended.0.insert(id);
        commands.entity(entity).remove::<AwaitingBlend>();
    }
}

/// A set of four joints and their weights from `mesh`, by its attributes.
fn four(mesh: &Mesh, index: MeshVertexAttribute, weight: MeshVertexAttribute) -> Option<Four> {
    match (mesh.attribute(index)?, mesh.attribute(weight)?) {
        (VertexAttributeValues::Uint16x4(i), VertexAttributeValues::Float32x4(w)) => {
            Some((i.clone(), w.clone()))
        }
        _ => None,
    }
}

type Four = (Vec<[u16; 4]>, Vec<[f32; 4]>);

/// Gives each vertex of `mesh` its share of dual quaternions, carried in its weights: all
/// eight of a vertex that has a second set (glTF's JOINTS_1 and WEIGHTS_1, D601), so the
/// shader reads one sum across both. `linear` says, by joint index, which keep the linear
/// blend. How many vertices went mostly dual quaternion, of how many; `None` if unskinned.
pub fn give_shares(mesh: &mut Mesh, blend: &Blend, linear: &[bool]) -> Option<(usize, usize)> {
    let (indices, mut weights) = four(
        mesh,
        Mesh::ATTRIBUTE_JOINT_INDEX,
        Mesh::ATTRIBUTE_JOINT_WEIGHT,
    )?;
    let mut second = four(
        mesh,
        Mesh::ATTRIBUTE_JOINT_INDEX_1,
        Mesh::ATTRIBUTE_JOINT_WEIGHT_1,
    )
    .filter(|(i, w)| i.len() == indices.len() && w.len() == weights.len());
    let is_linear = |j: u16| linear.get(j as usize).copied().unwrap_or(false);
    let mut dual = 0;
    for v in 0..weights.len() {
        let mut w = [0.0f32; 8];
        let mut on = [false; 8];
        w[..4].copy_from_slice(&weights[v]);
        on[..4].copy_from_slice(&indices[v].map(is_linear));
        let n = if let Some((i1, w1)) = &second {
            w[4..].copy_from_slice(&w1[v]);
            on[4..].copy_from_slice(&i1[v].map(is_linear));
            8
        } else {
            4
        };
        let share = blend.share(&w[..n], &on[..n]);
        dual += usize::from(share > 0.5);
        carry_share(&mut w[..n], share);
        weights[v].copy_from_slice(&w[..4]);
        if let Some((_, w1)) = &mut second {
            w1[v].copy_from_slice(&w[4..]);
        }
    }
    let count = weights.len();
    mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
    if let Some((_, w1)) = second {
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT_1, w1);
    }
    Some((dual, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ours_stands_in_for_bevys_skinning() {
        let shader = Shader::from_wesl(OURS, BEVY_SKINNING);
        assert!(is_ours(&shader));
        assert_eq!(
            shader.import_path,
            bevy::shader::ShaderImport::Custom("bevy_pbr::render::skinning".into())
        );
        // What it imports, as Bevy's own does, so the shader cache can resolve it.
        for import in [
            "bevy_pbr::render::mesh_types::SkinnedMesh",
            "bevy_pbr::render::mesh_bindings::mesh",
        ] {
            assert!(
                shader
                    .imports
                    .contains(&bevy::shader::ShaderImport::Custom(import.into())),
                "{import} missing from {:?}",
                shader.imports
            );
        }
    }

    // That it compiles and validates through Bevy's mesh and prepass shaders, in Bevy's place,
    // is terra_world's `shaders_tests` (D549).

    #[test]
    fn a_vertex_leaves_dual_quaternions_by_the_gain_times_its_weight_on_linear_joints() {
        let blend = Blend {
            linear_joints: vec!["hips".into()],
            linear_gain: 4.0,
        };
        let hips = &[true, false, false, false];
        // On no linear joint: dual quaternions throughout.
        assert_eq!(blend.share(&[0.6, 0.4, 0.0, 0.0], &[false; 4]), 1.0);
        // An eighth on the hips, four times over: half and half.
        assert!((blend.share(&[0.125, 0.875, 0.0, 0.0], hips) - 0.5).abs() < 1e-6);
        // A quarter or more: the linear blend throughout, never less than none.
        assert_eq!(blend.share(&[0.25, 0.75, 0.0, 0.0], hips), 0.0);
        assert_eq!(blend.share(&[0.8, 0.2, 0.0, 0.0], hips), 0.0);
        // Weights are shares of their sum: 0.25 of 2 is an eighth.
        assert!((blend.share(&[0.25, 1.75, 0.0, 0.0], hips) - 0.5).abs() < 1e-6);
        // Two linear joints count together.
        assert!(
            (blend.share(&[0.0625, 0.0625, 0.875, 0.0], &[true, true, false, false]) - 0.5).abs()
                < 1e-6
        );
        // No weight at all: the linear blend, as Bevy would.
        assert_eq!(blend.share(&[0.0; 4], hips), 0.0);
        // Of eight (D601), a linear joint among the last four counts as one among the first.
        let mut eighth = [false; 8];
        eighth[6] = true;
        assert!(
            (blend.share(&[0.5, 0.2, 0.1, 0.05, 0.025, 0.0, 0.125, 0.0], &eighth) - 0.5).abs()
                < 1e-6
        );
    }

    #[test]
    fn the_shipped_rule_names_joints_the_skeleton_has() {
        // Read as the game reads it, without the fallback that would hide a broken file.
        let blend: Blend = serde_json::from_str(include_str!(
            "../../../assets/models/character/skinning.json"
        ))
        .unwrap();
        assert!(blend.linear_gain > 0.0);
        assert!(!blend.linear_joints.is_empty());
        let joints = crate::model_files::joints(&crate::model_files::json(crate::appearance::BASE));
        for joint in &blend.linear_joints {
            assert!(
                joints.contains(joint),
                "{joint} is not a joint of the base body"
            );
        }
    }

    #[test]
    fn a_share_rides_in_the_weights_and_comes_back_out() {
        for share in [0.0, 0.3, 1.0] {
            let mut w = [0.5, 0.3, 0.2, 0.0];
            carry_share(&mut w, share);
            let total: f32 = w.iter().sum();
            // What the shader reads: the share, and the weights once divided by their sum.
            assert!((total - 1.0 - share).abs() < 1e-6);
            assert!((w[0] / total - 0.5).abs() < 1e-6);
            assert!((w[1] / total - 0.3).abs() < 1e-6);
        }
        // Weights that did not sum to one are normalised first.
        let mut w = [0.25, 0.25, 0.0, 0.0];
        carry_share(&mut w, 0.0);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }

    /// A mesh of two vertices: the first with eight influences, the second with one.
    fn eight_influence_mesh() -> Mesh {
        Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; 2])
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4(vec![[0, 1, 2, 3], [1, 0, 0, 0]]),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_JOINT_WEIGHT,
            vec![[0.4f32, 0.2, 0.1, 0.1], [1.0, 0.0, 0.0, 0.0]],
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX_1,
            VertexAttributeValues::Uint16x4(vec![[4, 5, 6, 7], [0, 0, 0, 0]]),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_JOINT_WEIGHT_1,
            vec![[0.08f32, 0.06, 0.04, 0.02], [0.0; 4]],
        )
    }

    fn fours(mesh: &Mesh) -> Vec<[f32; 8]> {
        let (Some(VertexAttributeValues::Float32x4(a)), Some(VertexAttributeValues::Float32x4(b))) = (
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT),
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT_1),
        ) else {
            panic!("no eight weights");
        };
        a.iter()
            .zip(b)
            .map(|(a, b)| [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]])
            .collect()
    }

    #[test]
    fn a_vertex_of_eight_carries_its_share_across_all_eight_weights() {
        let blend = Blend {
            linear_joints: vec!["hips".into()],
            linear_gain: 4.0,
        };
        // Joint 6 keeps the linear blend: 0.04 of the first vertex's weight is on it.
        let mut linear = vec![false; 8];
        linear[6] = true;
        let mut mesh = eight_influence_mesh();
        let before = fours(&mesh);
        assert_eq!(give_shares(&mut mesh, &blend, &linear), Some((2, 2)));
        let after = fours(&mesh);
        let share = 1.0 - 4.0 * 0.04;
        let total: f32 = after[0].iter().sum();
        assert!((total - 1.0 - share).abs() < 1e-5, "{total}");
        // Every weight, the last four too, keeps its part of the whole.
        for (b, a) in before[0].iter().zip(&after[0]) {
            assert!((a / total - b).abs() < 1e-6, "{a} of {total} for {b}");
        }
        // The vertex of one influence: all of it dual quaternion, its second set still empty.
        assert!((after[1][0] - 2.0).abs() < 1e-6);
        assert_eq!(after[1][4..], [0.0; 4]);
    }

    /// One triangle skinned to eight joints a vertex, as glTF writes it: JOINTS_0 and
    /// JOINTS_1 (unsigned bytes) and WEIGHTS_0 and WEIGHTS_1 (floats), each vertex's eight
    /// weights `WEIGHTS` and joints its own turn of 0..8, in one buffer, `eight.bin`.
    const WEIGHTS: [f32; 8] = [0.3, 0.2, 0.15, 0.1, 0.09, 0.07, 0.05, 0.04];

    fn joints_of(vertex: usize) -> [u8; 8] {
        std::array::from_fn(|i| ((i + vertex) % 8) as u8)
    }

    fn eight_influence_gltf() -> (String, Vec<u8>) {
        let mut bin: Vec<u8> = Vec::new();
        for p in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            bin.extend(p.iter().flat_map(|f| f.to_le_bytes()));
        }
        let joints = bin.len(); // 36
        for set in 0..2 {
            for v in 0..3 {
                bin.extend_from_slice(&joints_of(v)[set * 4..set * 4 + 4]);
            }
        }
        let weights = bin.len(); // 60
        for set in 0..2 {
            for _ in 0..3 {
                bin.extend(
                    WEIGHTS[set * 4..set * 4 + 4]
                        .iter()
                        .flat_map(|f| f.to_le_bytes()),
                );
            }
        }
        let poses = bin.len(); // 108
        for _ in 0..8 {
            bin.extend(
                Mat4::IDENTITY
                    .to_cols_array()
                    .iter()
                    .flat_map(|f| f.to_le_bytes()),
            );
        }
        let nodes: Vec<String> = (0..8)
            .map(|i| format!(r#"{{"name":"joint{i}"}}"#))
            .chain([r#"{"name":"body","mesh":0,"skin":0}"#.into()])
            .collect();
        let gltf = format!(
            r#"{{
"asset":{{"version":"2.0"}},
"scene":0,"scenes":[{{"nodes":[0,1,2,3,4,5,6,7,8]}}],
"nodes":[{nodes}],
"skins":[{{"joints":[0,1,2,3,4,5,6,7],"inverseBindMatrices":5}}],
"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"JOINTS_0":1,"JOINTS_1":2,"WEIGHTS_0":3,"WEIGHTS_1":4}}}}]}}],
"buffers":[{{"byteLength":{len},"uri":"eight.bin"}}],
"bufferViews":[
 {{"buffer":0,"byteOffset":0,"byteLength":36}},
 {{"buffer":0,"byteOffset":{joints},"byteLength":24}},
 {{"buffer":0,"byteOffset":{weights},"byteLength":96}},
 {{"buffer":0,"byteOffset":{poses},"byteLength":512}}],
"accessors":[
 {{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}},
 {{"bufferView":1,"byteOffset":0,"componentType":5121,"count":3,"type":"VEC4"}},
 {{"bufferView":1,"byteOffset":12,"componentType":5121,"count":3,"type":"VEC4"}},
 {{"bufferView":2,"byteOffset":0,"componentType":5126,"count":3,"type":"VEC4"}},
 {{"bufferView":2,"byteOffset":48,"componentType":5126,"count":3,"type":"VEC4"}},
 {{"bufferView":3,"componentType":5126,"count":8,"type":"MAT4"}}]
}}"#,
            nodes = nodes.join(","),
            len = bin.len(),
        );
        (gltf, bin)
    }

    /// The meshes the game's glTF loader makes of `gltf` and its buffer, loaded as the game
    /// loads a model.
    fn load_gltf((gltf, bin): (String, Vec<u8>)) -> Vec<Mesh> {
        use bevy::asset::io::{
            AssetSourceBuilder, AssetSourceId,
            memory::{Dir, MemoryAssetReader},
        };
        let dir = Dir::default();
        dir.insert_asset_text(std::path::Path::new("eight.gltf"), &gltf);
        dir.insert_asset(std::path::Path::new("eight.bin"), bin);
        let mut app = App::new();
        let reader = MemoryAssetReader { root: dir };
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || Box::new(reader.clone())),
        )
        .add_plugins((
            bevy::app::TaskPoolPlugin::default(),
            AssetPlugin::default(),
            bevy::world_serialization::WorldSerializationPlugin,
            bevy::mesh::MeshPlugin,
            // The game's glTF plugin: Bevy's loader behind a gate (D361).
            terra_world::gltf_gate::GatedGltfPlugin::default(),
        ));
        app.finish();
        app.cleanup();
        let handle: Handle<bevy::gltf::Gltf> =
            app.world().resource::<AssetServer>().load("eight.gltf");
        for _ in 0..10_000 {
            app.update();
            match app
                .world()
                .resource::<AssetServer>()
                .get_load_state(&handle)
            {
                Some(bevy::asset::LoadState::Loaded) => break,
                Some(bevy::asset::LoadState::Failed(e)) => panic!("{e}"),
                _ => {}
            }
        }
        app.world()
            .resource::<Assets<Mesh>>()
            .iter()
            .map(|(_, m)| m.clone())
            .collect()
    }

    #[test]
    fn a_gltf_of_eight_influences_brings_all_eight_to_the_mesh() {
        let meshes = load_gltf(eight_influence_gltf());
        assert_eq!(meshes.len(), 1);
        let mesh = &meshes[0];
        let Some(VertexAttributeValues::Uint16x4(first)) =
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX)
        else {
            panic!("no JOINTS_0");
        };
        let Some(VertexAttributeValues::Uint16x4(second)) =
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX_1)
        else {
            panic!("JOINTS_1 did not reach the mesh: the loader keeps four influences a vertex");
        };
        let weights = fours(mesh);
        for v in 0..3 {
            let joints: Vec<u16> = first[v].iter().chain(&second[v]).copied().collect();
            let want: Vec<u16> = joints_of(v).map(u16::from).to_vec();
            assert_eq!(joints, want, "vertex {v}'s joints");
            assert_eq!(weights[v], WEIGHTS, "vertex {v}'s weights");
        }
        // All of a vertex's weight, not the first four's 0.75 of it.
        assert!((weights[0].iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }
}
