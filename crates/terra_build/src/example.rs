//! The example base, laid by the build experiment and the yard, and the footprint the start
//! is chosen for, so the place every test position is measured from stays where it is.

use crate::{BuildOp, CELL_M, Kind, Panel, Side};
use glam::IVec3;

/// The example base: two rooms joined by a corridor, a small upper storey, windows on
/// every side and a door. Written in metres, so it is the same base whatever the grid.
pub fn example(kind: Kind) -> Vec<BuildOp> {
    // A block of tiles from one outside corner in metres to the other, on a storey. Rooms
    // that meet share the row of tiles between them, so they open into one another.
    let room = |west: f32, east: f32, north: f32, south: f32, storey: i32| BuildOp::AddRoom {
        min: [cells_from(west), storey, cells_from(north)],
        max: [cells_from(east) - 1, storey, cells_from(south) - 1],
        kind,
    };
    let mut ops = vec![
        room(-6.0, 9.0, -6.0, 3.0, 0),
        room(8.5, 15.5, -3.0, 0.0, 0),
        room(15.0, 21.0, -6.0, 3.0, 0),
        room(-3.0, 3.0, -6.0, 0.0, 1),
    ];
    // An opening in a wall: which way the wall faces, where the wall is, and the stretch
    // of it to open, all in metres.
    let mut cut = |side: Side, wall_at: f32, from: f32, to: f32, storey: i32, panel: Panel| {
        // The tile of the wall, just inside its outer face.
        let inside = match side {
            Side::East | Side::South => cells_from(wall_at) - 1,
            _ => cells_from(wall_at),
        };
        for along in cells_from(from)..cells_from(to) {
            let cell = match side {
                Side::East | Side::West => IVec3::new(inside, storey, along),
                _ => IVec3::new(along, storey, inside),
            };
            ops.push(BuildOp::SetWall {
                tile: cell.to_array(),
                panel: Some(panel),
                kind,
            });
        }
    };
    cut(Side::South, 3.0, -3.0, 6.0, 0, Panel::WINDOW);
    cut(Side::South, 3.0, 7.0, 8.5, 0, Panel::Door);
    cut(Side::West, -6.0, -2.5, -0.5, 0, Panel::WINDOW);
    cut(Side::South, 0.0, -2.5, 2.5, 1, Panel::TALL_WINDOW);
    cut(Side::South, 0.0, 12.5, 14.5, 0, Panel::WINDOW);
    cut(Side::South, 3.0, 18.5, 20.5, 0, Panel::WINDOW);
    cut(Side::East, 21.0, -2.5, -0.5, 0, Panel::TALL_WINDOW);
    cut(Side::North, -6.0, -5.5, -3.5, 0, Panel::WINDOW);
    cut(Side::North, -6.0, 6.5, 8.5, 0, Panel::WINDOW);
    cut(Side::North, -6.0, 18.5, 20.5, 0, Panel::WINDOW);
    cut(Side::North, -6.0, -2.5, -0.5, 1, Panel::WINDOW);
    ops
}

/// The grid line a distance in metres falls on. Unlike [`crate::cells`], which counts, this
/// places, so it can be negative or zero.
fn cells_from(metres: f32) -> i32 {
    (metres / CELL_M).round() as i32
}
