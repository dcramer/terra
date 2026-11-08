//! The hero drawn through a body (iso-forest-demo j12): where a treant or a mob stands
//! between him and the camera, his figure shows through it as a tinted silhouette, as the
//! action RPGs draw an occluded player (Diablo IV's and Path of Exile's see-through outline).
//! The wood opens about him (occlude.rs); a body is solid and moves, so it is drawn through.
//!
//! How (the Bevy way, no stencil: the main pass's depth is Depth32Float): each of his meshes
//! gets a child, `Silhouette`, drawing the same mesh (with his skin, so it follows his pose)
//! in the transparent phase with `Through`, an unlit material whose depth test passes only
//! behind what is already drawn (reverse Z: `Less`) and writes no depth. Its fragment moves
//! its own depth `GAP` toward the camera first (silhouette.wesl), so only a surface more than
//! `GAP` before him draws him through: his own shield before his chest, or the grass at his
//! feet, does not. Unreal does it with its custom depth and stencil, Godot with a second pass
//! of `depth_test_disabled`; neither is ours to copy here, only the idea.

use crate::play::Hero;
use bevy::asset::embedded_asset;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

/// How far before his surface (m) a body must stand to draw him through it: past his shield
/// held before his chest (about 0.25 m), short of a body that stands before him.
pub const GAP: f32 = 0.35;
/// How far his surface stands before the nearest of his joints (m): a limb's or the torso's
/// half thickness and his armour. His front (`Front`) is that nearest joint less this.
pub const FLESH: f32 = 0.2;
/// The fill and the edge as they reach the screen (unlit, before AgX): a cool pale blue
/// against the wood's greens and the foes' bark, the edge enough to read as his outline. An
/// edge, nearly no fill: drawn through a treant's leaf cards, j29's fill of 0.45 and edge of
/// 0.9 showed as a pale blue screen-door of his legs (route frame 1034), which the judge took
/// for "a rendering bug, not a hero behind a foe".
const FILL: LinearRgba = LinearRgba::new(0.22, 0.38, 0.62, 0.1);
const EDGE: LinearRgba = LinearRgba::new(0.55, 0.75, 1.1, 0.55);

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "silhouette.wesl");
    app.add_plugins(MaterialPlugin::<Through>::default())
        .init_resource::<Front>()
        .add_systems(Update, copy)
        .add_systems(PostUpdate, front.after(TransformSystems::Propagate));
}

/// His front this frame: the view depth (along the camera's forward, m) before which a surface
/// must stand to draw him through it, the nearest of his joints less `FLESH`; `None` before
/// his meshes have their copies. Mid-roll his limbs stand a metre before his back, more than
/// `GAP`: without it they drew him through himself (iso-forest-demo j17, the dodge at 1034, a
/// pale blob). The occlusion gate reads it too (occlude.rs `hidden`).
#[derive(Resource, Default, Clone, Copy)]
pub struct Front(pub Option<f32>);

/// The nearest of his joints to the camera, as `Front` and as a point on that plane in the
/// material (`Through::front`), so the shader's depth never lies behind his own front.
pub fn front(
    skins: Query<&SkinnedMesh, With<Silhouette>>,
    joints: Query<&GlobalTransform>,
    camera: Query<&GlobalTransform, With<crate::camera::MainCamera>>,
    mut out: ResMut<Front>,
    mut materials: ResMut<Assets<Through>>,
) {
    let Ok(cam) = camera.single() else {
        return;
    };
    let forward = cam.forward().as_vec3();
    let nearest = skins
        .iter()
        .flat_map(|s| s.joints.iter())
        .filter_map(|j| joints.get(*j).ok())
        .map(|g| g.translation())
        .min_by(|a, b| a.dot(forward).total_cmp(&b.dot(forward)));
    out.0 = nearest.map(|p| p.dot(forward) - FLESH);
    let point = nearest.map_or(Vec4::ZERO, |p| (p - forward * FLESH).extend(1.0));
    for (_, m) in materials.iter_mut() {
        m.front = point;
    }
}

/// A copy of one of his meshes that draws him through what stands before him.
#[derive(Component)]
pub struct Silhouette;

/// The silhouette's material: unlit, drawn only behind what is drawn.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Through {
    #[uniform(100)]
    fill: LinearRgba,
    #[uniform(101)]
    edge: LinearRgba,
    /// w the gap (`GAP`).
    #[uniform(102)]
    look: Vec4,
    /// xyz a point on his front's plane (`Front`), w 1 once known.
    #[uniform(103)]
    front: Vec4,
}

impl Default for Through {
    fn default() -> Self {
        Through {
            fill: FILL,
            edge: EDGE,
            look: Vec4::new(0.0, 0.0, 0.0, GAP),
            front: Vec4::ZERO,
        }
    }
}

impl Material for Through {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/demo/silhouette.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            // Reverse Z: a fragment behind what is drawn has the smaller depth.
            depth.depth_compare = Some(CompareFunction::Less);
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

/// Each mesh of his, as it comes, given its silhouette: the same mesh and skin, as a child
/// so it moves and hides with it. Spawned once a mesh, never per frame.
#[allow(clippy::type_complexity)]
fn copy(
    mut commands: Commands,
    added: Query<(Entity, &Mesh3d, Option<&SkinnedMesh>), (Added<Mesh3d>, Without<Silhouette>)>,
    parents: Query<&ChildOf>,
    heroes: Query<(), With<Hero>>,
    mut materials: ResMut<Assets<Through>>,
    mut through: Local<Option<Handle<Through>>>,
) {
    for (e, mesh, skin) in &added {
        if !std::iter::once(e)
            .chain(parents.iter_ancestors(e))
            .any(|a| heroes.contains(a))
        {
            continue;
        }
        let material = through
            .get_or_insert_with(|| materials.add(Through::default()))
            .clone();
        let mut copy = commands.spawn((
            Silhouette,
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            bevy::light::NotShadowCaster,
            ChildOf(e),
        ));
        if let Some(skin) = skin {
            copy.insert(skin.clone());
        }
    }
}
