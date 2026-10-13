//! Block walls (docs/building-kits.md, D179, D197): a masonry kit's wall cubes, each a
//! block, drawn not as models but as the cubes' own surface, rounded and displaced by the
//! kit's material (`surface.rs`, D197), so any shape is one wall with no seams.
//!
//! **The layout** is worked out from the cubes alone (D181): a block for each solid cube,
//! wall, floor or footing, from the bottom up, which is the order they go in and what a plan's progress
//! counts; and for each hole, a cube cut open or glazed, the door or window set in it once
//! the building is up (D193), with a timber lintel over it where the wall goes on above.

use super::{Piece, Place};
use crate::kit::Kit;
use crate::{Base, CELL_M, Fill, Kind};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;

/// How deep a lintel is over an opening, and how far it runs into the wall either side.
const LINTEL_DEEP_M: f32 = 0.18;
const LINTEL_BEARING_M: f32 = 0.2;

/// Lays every cube of a base in the kit's kind as blocks: its walls, floors and footings,
/// one surface.
pub fn lay(base: &Base, kit: &Kit) -> Vec<Piece> {
    let kind = kit.kind();
    let solid = |at: IVec3| {
        base.cube(at)
            .is_some_and(|cube| cube.kind == kind && super::drawn_fill(&cube) == Fill::Solid)
    };
    let mut pieces: Vec<Piece> = base
        .cubes()
        .map(|(at, _)| at)
        .filter(|&at| solid(at))
        .map(|cube| Piece::Block { cube })
        .collect();
    openings(base, kind, &solid, &mut pieces);
    pieces
}

/// Each hole in the walls, as the door or window set in it and a lintel over it where
/// there is wall above.
fn openings(base: &Base, kind: Kind, solid: &impl Fn(IVec3) -> bool, pieces: &mut Vec<Piece>) {
    let holes: BTreeSet<[i32; 3]> = base
        .cubes()
        .filter(|(at, cube)| {
            cube.kind == kind && super::drawn_fill(cube) != Fill::Solid && base.in_wall(*at)
        })
        .map(|(at, _)| at.to_array())
        .collect();
    let mut seen = BTreeSet::new();
    for &start in &holes {
        let start = IVec3::from_array(start);
        if !seen.insert(start.to_array()) {
            continue;
        }
        // The hole's cubes, joined side by side and up and down.
        let mut region = vec![start];
        let mut next = vec![start];
        while let Some(at) = next.pop() {
            for step in [
                IVec3::X,
                IVec3::NEG_X,
                IVec3::Y,
                IVec3::NEG_Y,
                IVec3::Z,
                IVec3::NEG_Z,
            ] {
                let beside = at + step;
                if holes.contains(&beside.to_array()) && seen.insert(beside.to_array()) {
                    region.push(beside);
                    next.push(beside);
                }
            }
        }
        let low = region.iter().copied().reduce(IVec3::min).unwrap_or(start);
        let high = region.iter().copied().reduce(IVec3::max).unwrap_or(start);
        let (min, max) = (
            low.as_vec3() * CELL_M,
            (high + IVec3::ONE).as_vec3() * CELL_M,
        );
        let along_x = high.x > low.x
            || (high.z == low.z
                && base.along(IVec3::new(low.x, crate::storey_of(low.y), low.z)) == IVec3::X);
        let door = !solid(low - IVec3::Y);
        pieces.push(Piece::Opening {
            min: Place::near(min),
            max: Place::near(max),
            along_x,
            door,
        });
        // A timber lintel, into the wall either side, where the wall goes on over it.
        if solid(IVec3::new(low.x, high.y + 1, low.z)) {
            let reach = if along_x { Vec3::X } else { Vec3::Z } * LINTEL_BEARING_M;
            let across = if along_x { Vec3::Z } else { Vec3::X };
            let inset = across * CELL_M * 0.1;
            pieces.push(Piece::Board {
                min: Place::near(Vec3::new(min.x, max.y - 0.02, min.z) - reach + inset),
                max: Place::near(Vec3::new(max.x, max.y + LINTEL_DEEP_M, max.z) + reach - inset),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::Pattern;
    use crate::{BuildOp, Panel};

    fn kit() -> Kit {
        Kit {
            id: 2,
            pattern: Pattern::Masonry,
            ..Kit::default()
        }
    }

    fn blocks(pieces: &[Piece]) -> BTreeSet<[i32; 3]> {
        pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Block { cube } => Some(cube.to_array()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_room_of_stone_is_a_block_for_every_cube() {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [7, 0, 5],
            kind: kit().kind(),
        });
        let pieces = lay(&base, &kit());
        // Its walls, its floor and its flat roof, one surface.
        let all: BTreeSet<[i32; 3]> = base.cubes().map(|(at, _)| at.to_array()).collect();
        assert_eq!(blocks(&pieces), all);
        assert_eq!(pieces.len(), all.len());
    }

    #[test]
    fn a_door_is_a_hole_with_a_lintel_over_it() {
        let mut base = Base::default();
        for x in 0..6 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 0],
                panel: Some(if x == 2 { Panel::Door } else { Panel::Solid }),
                kind: kit().kind(),
            });
        }
        let pieces = lay(&base, &kit());
        assert_eq!(
            pieces
                .iter()
                .filter(|p| matches!(p, Piece::Opening { door: true, .. }))
                .count(),
            1
        );
        assert_eq!(
            pieces
                .iter()
                .filter(|p| matches!(p, Piece::Board { .. }))
                .count(),
            1
        );
        let solid: BTreeSet<[i32; 3]> = base
            .cubes()
            .filter(|(_, cube)| cube.fill == Fill::Solid)
            .map(|(at, _)| at.to_array())
            .collect();
        assert_eq!(blocks(&pieces), solid);
    }
}
