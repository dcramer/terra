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
    mesh::{VertexAttributeValues, skinning::SkinnedMesh},
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

    /// A vertex's share of dual quaternions, from its weights and whether each of its joints
    /// is one that keeps the linear blend.
    pub fn share(&self, weights: [f32; 4], linear: [bool; 4]) -> f32 {
        let total: f32 = weights.iter().sum();
        if total <= 0.0 {
            return 0.0;
        }
        let on_linear: f32 = (0..4)
            .filter(|&i| linear[i])
            .map(|i| weights[i])
            .sum::<f32>()
            / total;
        (1.0 - self.linear_gain * on_linear).clamp(0.0, 1.0)
    }
}

/// Puts `share` into `weights`, normalised, so that they sum to one plus it: the shader reads
/// the share back as the sum less one, and divides it out.
pub fn carry_share(weights: [f32; 4], share: f32) -> [f32; 4] {
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return weights;
    }
    let k = (1.0 + share.clamp(0.0, 1.0)) / total;
    weights.map(|w| w * k)
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
        let indices: Vec<[u16; 4]> = match mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX) {
            Some(VertexAttributeValues::Uint16x4(v)) => v.clone(),
            _ => {
                commands.entity(entity).remove::<AwaitingBlend>();
                continue;
            }
        };
        if let Some(VertexAttributeValues::Float32x4(weights)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_JOINT_WEIGHT)
        {
            let mut dual = 0;
            for (w, joints) in weights.iter_mut().zip(&indices) {
                let on = joints.map(|j| linear.get(j as usize).copied().unwrap_or(false));
                let share = blend.share(*w, on);
                dual += usize::from(share > 0.5);
                *w = carry_share(*w, share);
            }
            info!(
                "skinning: {} of {} vertices of a mesh mostly dual quaternion",
                dual,
                weights.len()
            );
        }
        blended.0.insert(id);
        commands.entity(entity).remove::<AwaitingBlend>();
    }
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

    /// Resolves our shader, and in place of Bevy's own modules it imports, the parts of them
    /// it uses as Bevy declares them (`bevy_pbr/src/render/mesh_types.wesl` and
    /// `mesh_bindings.wesl`).
    struct Modules;

    const MESH_TYPES: &str = "
        struct Mesh {
            world_from_local: mat3x4<f32>,
            current_skin_index: u32,
        };
        @if(SKINNED)
        struct SkinnedMesh {
            data: array<mat4x4<f32>, 256u>,
        };
    ";
    const MESH_BINDINGS: &str = "
        import package::render::mesh_types::Mesh;
        @group(2) @binding(0) var<storage> mesh: array<Mesh>;
    ";
    /// A vertex shader calling it as Bevy's mesh and prepass shaders do.
    const VERTEX: &str = "
        import package::render::skinning;
        @vertex
        fn vertex(
            @builtin(instance_index) instance: u32,
            @location(0) position: vec3<f32>,
            @location(1) normal: vec3<f32>,
            @location(2) joints: vec4<u32>,
            @location(3) weights: vec4<f32>,
        ) -> @builtin(position) vec4<f32> {
            var world_from_local = skinning::skin_model(joints, weights, instance);
            let before = skinning::skin_prev_model(joints, weights, instance);
            let world_normal = skinning::skin_normals(world_from_local, normal);
            return world_from_local * vec4<f32>(position, 1.0) + before * vec4<f32>(world_normal, 0.0);
        }
    ";

    impl wesl::Resolver for Modules {
        fn resolve_source<'a>(
            &'a self,
            path: &wesl::syntax::ModulePath,
        ) -> Result<std::borrow::Cow<'a, str>, wesl::error::ResolveError> {
            match path.components.last().map(String::as_str) {
                Some("skinning") => Ok(OURS.into()),
                Some("mesh_types") => Ok(MESH_TYPES.into()),
                Some("mesh_bindings") => Ok(MESH_BINDINGS.into()),
                Some("vertex") => Ok(VERTEX.into()),
                _ => Err(wesl::error::ResolveError::ModuleNotFound(
                    path.clone(),
                    "not one of the test's modules".into(),
                )),
            }
        }
    }

    #[test]
    fn ours_compiles_and_validates_as_bevy_would_build_it() {
        // Both ways Bevy can hand a skin its joints: uniform buffers where storage buffers
        // are not allowed in a vertex shader, storage buffers elsewhere.
        for uniform in [false, true] {
            // As Bevy's shader cache compiles a module (`bevy_shader`'s `shader_cache.rs`).
            let mut options = wesl::CompileOptions {
                imports: true,
                condcomp: true,
                visibility: false,
                ..Default::default()
            };
            options.features.set("SKINNED", true);
            options.features.set("SKINS_USE_UNIFORM_BUFFERS", uniform);
            let main = wesl::syntax::ModulePath {
                origin: wesl::syntax::PathOrigin::Package("bevy_pbr".into()),
                components: vec!["render".into(), "vertex".into()],
            };
            let wgsl = wesl::compile(&main, &options, &Modules)
                .unwrap_or_else(|e| panic!("uniform {uniform}: {}", e.diagnostic().render_plain()))
                .to_string();
            let module = naga::front::wgsl::parse_str(&wgsl)
                .unwrap_or_else(|e| panic!("uniform {uniform}: {}", e.emit_to_string(&wgsl)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::default(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("uniform {uniform}: {}", e.emit_to_string(&wgsl)));
        }
    }

    #[test]
    fn a_vertex_leaves_dual_quaternions_by_the_gain_times_its_weight_on_linear_joints() {
        let blend = Blend {
            linear_joints: vec!["hips".into()],
            linear_gain: 4.0,
        };
        let hips = [true, false, false, false];
        // On no linear joint: dual quaternions throughout.
        assert_eq!(blend.share([0.6, 0.4, 0.0, 0.0], [false; 4]), 1.0);
        // An eighth on the hips, four times over: half and half.
        assert!((blend.share([0.125, 0.875, 0.0, 0.0], hips) - 0.5).abs() < 1e-6);
        // A quarter or more: the linear blend throughout, never less than none.
        assert_eq!(blend.share([0.25, 0.75, 0.0, 0.0], hips), 0.0);
        assert_eq!(blend.share([0.8, 0.2, 0.0, 0.0], hips), 0.0);
        // Weights are shares of their sum: 0.25 of 2 is an eighth.
        assert!((blend.share([0.25, 1.75, 0.0, 0.0], hips) - 0.5).abs() < 1e-6);
        // Two linear joints count together.
        assert!(
            (blend.share([0.0625, 0.0625, 0.875, 0.0], [true, true, false, false]) - 0.5).abs()
                < 1e-6
        );
        // No weight at all: the linear blend, as Bevy would.
        assert_eq!(blend.share([0.0; 4], hips), 0.0);
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
            let w = carry_share([0.5, 0.3, 0.2, 0.0], share);
            let total: f32 = w.iter().sum();
            // What the shader reads: the share, and the weights once divided by their sum.
            assert!((total - 1.0 - share).abs() < 1e-6);
            assert!((w[0] / total - 0.5).abs() < 1e-6);
            assert!((w[1] / total - 0.3).abs() < 1e-6);
        }
        // Weights that did not sum to one are normalised first.
        let w = carry_share([0.25, 0.25, 0.0, 0.0], 0.0);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }
}
