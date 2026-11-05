//! A mesh's bounds worked out once, not once for each thing that draws it. Bevy's
//! `calculate_bounds` asks each new entity's mesh for its box (`MeshAabb::get_aabb`,
//! bevy_camera 0.20), and a mesh still holding its vertices walks all of them each time, so
//! the world's entering, putting in thousands of plants of a few hundred meshes, spent
//! 1.2 s of its frame there (crossing-freeze j2). This fills each such mesh's `final_aabb`
//! first, which `get_aabb` returns at once, as Bevy does itself when a mesh's data goes to
//! the GPU (`Mesh::take_gpu_data`). Only for a mesh drawn and not kept on the CPU: its data
//! is gone once drawn, so nothing changes its vertices after, and a box filled in would
//! never go stale.

use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::{Aabb, MeshAabb};
use bevy::camera::visibility::{NoAutoAabb, NoFrustumCulling, VisibilitySystems};
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::shape::Aabb3d;

pub fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        bound_once.before(VisibilitySystems::CalculateBounds),
    );
}

/// Fills the box of each mesh a new entity draws, once a mesh.
fn bound_once(
    new: Query<
        &Mesh3d,
        (
            Without<Aabb>,
            Without<NoFrustumCulling>,
            Without<NoAutoAabb>,
        ),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mut seen: HashSet<AssetId<Mesh>> = HashSet::default();
    for mesh in &new {
        let id = mesh.id();
        if !seen.insert(id) {
            continue;
        }
        let unbounded = meshes.get(id).is_some_and(|mesh| {
            mesh.final_aabb.is_none() && mesh.asset_usage == RenderAssetUsages::RENDER_WORLD
        });
        if !unbounded {
            continue;
        }
        // Unannounced: an announced change would send the mesh to the GPU again.
        if let Some(mesh) = meshes.get_mut_untracked(id) {
            mesh.final_aabb = mesh.get_aabb().map(Aabb3d::from);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::mesh::PrimitiveTopology;

    fn mesh(usage: RenderAssetUsages) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, usage).with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[-1.0, 0.0, 2.0], [3.0, 5.0, 2.0], [0.0, -2.0, -4.0]],
        )
    }

    #[test]
    fn a_mesh_drawn_many_times_is_bounded_once_and_one_kept_on_the_cpu_is_left() {
        let mut world = World::new();
        let mut meshes = Assets::<Mesh>::default();
        let drawn = meshes.add(mesh(RenderAssetUsages::RENDER_WORLD));
        let kept = meshes.add(mesh(RenderAssetUsages::all()));
        world.insert_resource(meshes);
        for _ in 0..3 {
            world.spawn(Mesh3d(drawn.clone()));
        }
        world.spawn(Mesh3d(kept.clone()));
        world
            .run_system_once(bound_once)
            .expect("the system runs on a world with meshes");
        let meshes = world.resource::<Assets<Mesh>>();
        let bounded = meshes.get(&drawn).and_then(|mesh| mesh.final_aabb);
        // The box round the three corners, by hand.
        let expected = Aabb3d {
            min: Vec3A::new(-1.0, -2.0, -4.0),
            max: Vec3A::new(3.0, 5.0, 2.0),
        };
        assert_eq!(bounded, Some(expected));
        // And what Bevy's bounds read of it is the same box.
        let read = meshes.get(&drawn).and_then(MeshAabb::get_aabb);
        assert_eq!(read.map(Aabb3d::from), Some(expected));
        assert_eq!(meshes.get(&kept).and_then(|mesh| mesh.final_aabb), None);
    }
}
