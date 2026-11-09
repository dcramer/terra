//! A grown tree as it is drawn: its wood and crown at each level of detail, its far crown,
//! and the stand-ins only the sun sees, which cast its shadow.

use super::{DETAIL_FOR_M, Foliage, SHADOW_M, SUN_ONLY, TreeModel, between, detail};
use bevy::camera::visibility::RenderLayers;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

/// The leaves' material, the far crowns' and what only the sun sees.
pub(super) type Looks<'a> = (
    &'a Handle<Foliage>,
    &'a Handle<StandardMaterial>,
    &'a Handle<StandardMaterial>,
);

/// A tree's parts under its entity, `height` metres tall. A leafless one (a cap tree) is
/// its wood alone: its crown's parts would each be an empty mesh, and a forest of them
/// cost the far realm three frames in four (16 a second against 62, 950 trees in view).
pub(super) fn tree(
    plant: &mut ChildSpawnerCommands,
    model: &TreeModel,
    height: f32,
    (leaves, far, solid): Looks,
) {
    let size = (height / DETAIL_FOR_M).clamp(0.45, 1.4);
    for level in 0..3 {
        // The simplest trunk stands on under the far crown too.
        let last = if level == 2 { 3 } else { level };
        let mut wood = plant.spawn((
            Mesh3d(model.wood[level].clone()),
            MeshMaterial3d(model.bark.clone()),
            detail(level, last, size),
        ));
        if level == 2 {
            wood.insert(NotShadowCaster);
        }
        if model.leafless {
            continue;
        }
        plant.spawn((
            Mesh3d(model.crown[level].clone()),
            MeshMaterial3d(leaves.clone()),
            detail(level, level, size),
            NotShadowCaster,
        ));
    }
    if model.leafless {
        return;
    }
    // Far off, the crown as its solid lumps in the leaves' colour, so the forest
    // carries on past where its cards end.
    plant.spawn((
        Mesh3d(model.far.clone()),
        MeshMaterial3d(far.clone()),
        detail(3, 3, size),
        NotShadowCaster,
    ));
    plant.spawn((
        Mesh3d(model.shadow_cards.clone()),
        MeshMaterial3d(leaves.clone()),
        between(0.0, SHADOW_M[0] * size),
        RenderLayers::layer(SUN_ONLY),
    ));
    plant.spawn((
        Mesh3d(model.shadow_lumps.clone()),
        MeshMaterial3d(solid.clone()),
        between(SHADOW_M[0] * size, SHADOW_M[1] * size),
        RenderLayers::layer(SUN_ONLY),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(leafless: bool) -> usize {
        let model = TreeModel {
            wood: default(),
            crown: default(),
            bark: default(),
            far: default(),
            shadow_cards: default(),
            shadow_lumps: default(),
            leafless,
            trunk_m: 0.4,
        };
        let mut world = World::new();
        let plant = world.spawn_empty().id();
        let (leaves, plain) = (Handle::default(), Handle::default());
        world.commands().entity(plant).with_children(|plant| {
            tree(plant, &model, 20.0, (&leaves, &plain, &plain));
        });
        world.flush();
        world.get::<Children>(plant).map_or(0, |parts| parts.len())
    }

    /// A tree is its wood and its crown at three levels of detail, its far crown and two
    /// shadows; one with no leaves is its wood alone.
    #[test]
    fn a_leafless_tree_is_drawn_as_its_wood_alone() {
        assert_eq!(parts(false), 9);
        assert_eq!(parts(true), 3);
    }
}
