//! Weight-blended skinning for every skinned mesh (`assets/shaders/skinning.wgsl`): each vertex
//! skinned partly with Bevy's linear blend and partly with dual quaternions, as Maya's "weight
//! blended" skinning does. Our clips roll a forearm up to half a turn about its length, and
//! the linear blend collapses the skin across such a turn into a thin neck; dual quaternions
//! keep its volume, and a knee's or elbow's through a deep bend, but bulge across the wide
//! blend round the hips, where the linear blend does better (research/skin-deformation.md).
//! Which joints keep the linear blend is data, `models/character/skinning.json`, read by the
//! character tools too, which measure bodies skinned the same way.
//!
//! Bevy has no switch for this, so its own skinning shader, `bevy_pbr::skinning`, is replaced
//! by ours with the same functions and bindings: every mesh shader that imports it, Bevy's and
//! our body's alike, then skins this way. Bevy loads its shader from an embedded file, so ours
//! is put in its place once that has loaded, and again if it ever reloads.
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

/// Where Bevy keeps its skinning shader (`bevy_pbr/src/render/skinning.wgsl`).
const BEVY_SKINNING: &str = "embedded://bevy_pbr/render/skinning.wgsl";
const OURS: &str = include_str!("../../../assets/shaders/skinning.wgsl");
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
        .get_resource::<crate::shell::Options>()
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
    match shaders.insert(id, Shader::from_wgsl(OURS, path)) {
        Ok(()) => info!("skinning: weight-blended, in place of Bevy's linear blend"),
        Err(problem) => error!("could not replace the skinning shader: {problem}"),
    }
}

fn is_ours(shader: &Shader) -> bool {
    matches!(&shader.source, bevy::shader::Source::Wgsl(source) if source.contains(MARK))
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
        let shader = Shader::from_wgsl(OURS, BEVY_SKINNING);
        assert!(is_ours(&shader));
        assert_eq!(
            shader.import_path,
            bevy::shader::ShaderImport::Custom("bevy_pbr::skinning".into())
        );
        // Every function Bevy's shaders call from it.
        for f in [
            "fn skin_model(",
            "fn skin_prev_model(",
            "fn skin_normals(",
            "fn inverse_transpose_3x3m(",
        ] {
            assert!(OURS.contains(f), "{f} missing");
        }
    }

    #[test]
    fn the_rule_reads_and_keeps_the_hips_linear() {
        let blend = Blend::read();
        assert_eq!(blend.linear_joints, vec!["pelvis".to_string()]);
        // A forearm: dual quaternions throughout.
        assert_eq!(blend.share([0.6, 0.4, 0.0, 0.0], [false; 4]), 1.0);
        // Half on the pelvis: the linear blend throughout.
        assert_eq!(
            blend.share([0.5, 0.5, 0.0, 0.0], [true, false, false, false]),
            0.0
        );
        // A quarter on the pelvis: half and half.
        let half = blend.share([0.25, 0.75, 0.0, 0.0], [true, false, false, false]);
        assert!((half - 0.5).abs() < 1e-6);
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
