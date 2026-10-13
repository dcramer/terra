//! Floors (docs/building-kits.md): the floors of rooms in a kit's kind, in panels.

use super::{Layout, Piece, Walls};
use crate::{CELL_M, STOREY_M};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;

/// The floors of rooms in the kit's kind, in panels: two tiles square where four make one,
pub(super) fn floors(walls: &Walls, layout: &mut Layout) {
    let ours = |tile: IVec3| {
        walls.base.contains(tile)
            && walls
                .base
                .cube(IVec3::new(tile.x, crate::floor_layer(tile.y), tile.z))
                .is_some_and(|cube| cube.kind == walls.kind)
    };
    let mut done: BTreeSet<[i32; 3]> = BTreeSet::new();
    for (tile, _) in walls.base.tiles() {
        if done.contains(&tile.to_array()) || !ours(tile) {
            continue;
        }
        let block = [IVec3::ZERO, IVec3::X, IVec3::Z, IVec3::new(1, 0, 1)];
        let size = if block
            .iter()
            .all(|step| ours(tile + *step) && !done.contains(&(tile + *step).to_array()))
        {
            2
        } else {
            1
        };
        for x in 0..size {
            for z in 0..size {
                done.insert((tile + IVec3::new(x, 0, z)).to_array());
            }
        }
        let y = tile.y as f32 * STOREY_M;
        let low = Vec3::new(tile.x as f32 * CELL_M, y, tile.z as f32 * CELL_M);
        layout.pieces.push(Piece::Floor {
            min: low,
            max: low + Vec3::new(size as f32 * CELL_M, 0.0, size as f32 * CELL_M),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::lay;
    use super::super::tests::{cabin, kit, textured};
    use super::*;
    use crate::BuildOp;
    use std::collections::BTreeMap;

    #[test]
    fn a_kit_with_floors_lays_one_under_every_tile_of_its_rooms_once() {
        let mut base = cabin();
        // A room on top of it, and one opening off its north side, its edge a tile out of
        // step with the panels laid inside the cabin.
        base.apply(BuildOp::AddRoom {
            min: [0, 1, 0],
            max: [7, 1, 5],
            kind: kit().kind(),
        });
        base.apply(BuildOp::AddRoom {
            min: [3, 0, -3],
            max: [7, 0, 0],
            kind: kit().kind(),
        });
        let layout = lay(&base, &textured());
        let mut under: BTreeMap<[i32; 3], usize> = BTreeMap::new();
        let mut panels = 0;
        let mut largest = 0;
        for piece in &layout.pieces {
            let Piece::Floor { min, max } = *piece else {
                continue;
            };
            panels += 1;
            assert_eq!(min.y, max.y);
            let storey = (min.y / STOREY_M).round() as i32;
            assert!((min.y - storey as f32 * STOREY_M).abs() < 1e-5, "{min}");
            let (low, high) = ((min / CELL_M).round(), (max / CELL_M).round());
            largest = largest.max((high.x - low.x) as i32);
            for x in low.x as i32..high.x as i32 {
                for z in low.z as i32..high.z as i32 {
                    *under.entry([x, storey, z]).or_default() += 1;
                }
            }
        }
        // Inside the cabin's walls, six tiles by four, on both storeys; and the other
        // room's three by two, and the three tiles of wall between taken away.
        let inside: BTreeMap<[i32; 3], usize> = (0..2)
            .flat_map(|y| (1..7).flat_map(move |x| (1..5).map(move |z| ([x, y, z], 1))))
            .chain((4..7).flat_map(|x| (-2..1).map(move |z| ([x, 0, z], 1))))
            .collect();
        assert_eq!(under, inside);
        // In panels two tiles square where they fit.
        assert_eq!(largest, 2);
        assert!(panels < inside.len());
    }
}
