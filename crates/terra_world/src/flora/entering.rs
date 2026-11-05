//! What grows in the world the run is in is the world's (D469): what stands in a walker's
//! way, the trunks, what the plants were cleared for and the pieces' meshes, planted on
//! entering it. Their instance buffers go with them.

use super::{Cleared, FloraSystems, Obstacles, Trunks, Undergrown, piece, plant};
use crate::instance::{Building, InWorld, InstanceApp};
use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.init_world_resource::<Obstacles>()
        .init_world_resource::<Trunks>()
        .init_world_resource::<piece::Built>()
        .world_resource::<Cleared>()
        .world_resource::<Undergrown>()
        // The yard, the flat test level, has nothing growing on it. Planted once the woods'
        // floor is laid, which says where it lays its own undergrowth (`Undergrown`).
        .add_systems(
            OnEnter(InWorld),
            plant
                .run_if(crate::shell::in_the_world)
                .in_set(FloraSystems::Plant)
                .in_set(Building::View)
                .after(crate::cover::forest::LayingFloor),
        );
}
