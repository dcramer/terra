//! What grows in the world the run is in is the world's (D469): what stands in a walker's
//! way, the trunks, the meadow and the pieces' meshes, planted on entering it. Their
//! instance buffers go with them.

use super::{FloraSystems, Meadow, Obstacles, Trunks, piece, plant};
use crate::instance::{Building, InWorld, InstanceApp};
use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.init_world_resource::<Obstacles>()
        .init_world_resource::<Trunks>()
        .init_world_resource::<piece::Built>()
        .world_resource::<Meadow>()
        // The yard, the flat test level, has nothing growing on it.
        .add_systems(
            OnEnter(InWorld),
            plant
                .run_if(crate::shell::in_the_world)
                .in_set(FloraSystems::Plant)
                .in_set(Building::View),
        );
}
