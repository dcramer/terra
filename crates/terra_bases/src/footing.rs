//! A base set on the ground: its frame raised until its floor clears the ground under it.

use bevy::prelude::*;
use terra_build::{Base, CELL_M};
use terra_world::planet::Planet;
use terra_world::start::floor_points;

/// Raises a build's frame until its floor clears the highest ground under it
/// (`start::lifted`): under its rooms, or with none, as a floor laid alone, under its floor.
pub fn lifted(frame: Transform, base: &Base, planet: &Planet) -> Transform {
    let mut columns: Vec<[i32; 2]> = base
        .room_cells()
        .filter(|(at, _)| at.y == 0)
        .map(|(at, _)| [at.x, at.z])
        .collect();
    if columns.is_empty() {
        let floor = terra_build::floor_layer(0);
        columns = base
            .cubes()
            .filter(|(at, _)| at.y == floor)
            .map(|(at, _)| [at.x, at.z])
            .collect();
    }
    terra_world::start::lifted(frame, floor_points(columns, CELL_M), planet)
}
