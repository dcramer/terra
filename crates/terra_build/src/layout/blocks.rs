//! Block walls (docs/building-kits.md, D179, D197): a masonry kit's wall cubes, each a
//! block, drawn not as models but as the cubes' own surface, rounded and displaced by the
//! kit's material (`surface.rs`, D197), so any shape is one wall with no seams.
//!
//! **The layout** is worked out from the cubes alone (D181): a block for each solid wall
//! cube, from the bottom up, which is the order they go in and what a plan's progress
//! counts; and for each hole, a cube cut open or glazed, the door or window set in it once
//! the building is up (D193). Round it, where the kit has dressed stone, a lintel over it
//! where the wall goes on above, a sill under a window, and jamb stones up its sides, long
//! and short by turns, as a mason sets them; where it has none, a timber lintel. Where a
//! thinner wall of another kind stands on the blocks, as logs on a stone plinth, a dressed
//! cap covers the blocks' top.

use super::{Piece, Place, hash};
use crate::kit::Kit;
use crate::mesh::Material;
use crate::{Base, CELL_M, Fill, Kind};
use glam::{IVec3, Vec3};
use std::collections::BTreeSet;

/// How deep a lintel is over an opening, and how far it runs into the wall either side.
const LINTEL_DEEP_M: f32 = 0.18;
const LINTEL_BEARING_M: f32 = 0.2;
/// A dressed lintel's height, and how far dressed stone stands out of the rubble.
const STONE_LINTEL_M: f32 = 0.26;
const PROUD_M: f32 = 0.03;
/// How tall a sill is, how far it runs past the jambs and how far it sticks out.
const SILL_M: f32 = 0.11;
const SILL_REACH_M: f32 = 0.12;
const SILL_OUT_M: f32 = 0.06;
/// How far jamb stones reach into the wall, long and short by turns.
const JAMB_LONG_M: f32 = 0.24;
const JAMB_SHORT_M: f32 = 0.14;
/// How thick a cap is over the blocks' top, and how far it overhangs them.
const CAP_M: f32 = 0.07;
const CAP_OUT_M: f32 = 0.025;

/// Lays every cube of a base in the kit's kind as blocks: its walls, floors and footings,
/// one surface; and the dressed stone round its openings and over its tops.
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
    let dressed = kit.textures.contains_key(&Material::Dressed);
    openings(base, kind, dressed, &solid, &mut pieces);
    if dressed {
        caps(base, kind, &solid, &mut pieces);
    }
    pieces
}

/// A dressed stone from one corner to the other, its version from where it is.
fn stone(a: Vec3, b: Vec3) -> Piece {
    let (min, max) = (Place::near(a.min(b)), Place::near(a.max(b)));
    Piece::Dressed {
        min,
        max,
        seed: hash(&[min.0[0] as u32, min.0[1] as u32, min.0[2] as u32, 5]),
    }
}

/// A cap over every block with a wall of another, thinner kind standing on it.
fn caps(base: &Base, kind: Kind, solid: &impl Fn(IVec3) -> bool, pieces: &mut Vec<Piece>) {
    for (at, _) in base.cubes() {
        let above = at + IVec3::Y;
        let thinner = base.cube(above).is_some_and(|cube| {
            cube.kind != kind && cube.kind != Kind::PLAIN && base.in_wall(above)
        });
        if !solid(at) || !thinner {
            continue;
        }
        let top = (at.y + 1) as f32 * CELL_M;
        let low = at.as_vec3() * CELL_M;
        // Over the sides open to the air, past the blocks by its overhang.
        let reach = |step: IVec3| if solid(at + step) { 0.0 } else { CAP_OUT_M };
        let a = Vec3::new(
            low.x - reach(IVec3::NEG_X),
            top - 0.02,
            low.z - reach(IVec3::NEG_Z),
        );
        let b = Vec3::new(
            low.x + CELL_M + reach(IVec3::X),
            top + CAP_M,
            low.z + CELL_M + reach(IVec3::Z),
        );
        pieces.push(stone(a, b));
    }
}
/// Each hole in the walls, as the door or window set in it and a lintel over it where
/// there is wall above.
fn openings(
    base: &Base,
    kind: Kind,
    dressed: bool,
    solid: &impl Fn(IVec3) -> bool,
    pieces: &mut Vec<Piece>,
) {
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
        let over = solid(IVec3::new(low.x, high.y + 1, low.z));
        if dressed {
            surround((min, max), (low, high), along_x, door, over, solid, pieces);
            continue;
        }
        // A timber lintel, into the wall either side, where the wall goes on over it.
        if over {
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

/// Dressed stone round an opening from `min` to `max` (the cubes from `low` to `high`): a
/// lintel over it where the wall goes on above, a sill under a window, and jamb stones up
/// each side where there is wall, long and short by turns, each standing a little out of
/// the rubble.
fn surround(
    (min, max): (Vec3, Vec3),
    (low, high): (IVec3, IVec3),
    along_x: bool,
    door: bool,
    over: bool,
    solid: &impl Fn(IVec3) -> bool,
    pieces: &mut Vec<Piece>,
) {
    let (along, across) = if along_x {
        (Vec3::X, Vec3::Z)
    } else {
        (Vec3::Z, Vec3::X)
    };
    let step = if along_x { IVec3::X } else { IVec3::Z };
    // A box from so far along to so far, so high, and so far out either face.
    let (first, last) = (min.dot(along), max.dot(along));
    let (inside, outside) = (min.dot(across), max.dot(across));
    let boxed = |a: f32, b: f32, bottom: f32, top: f32, out: f32| {
        stone(
            along * a + Vec3::Y * bottom + across * (inside - out),
            along * b + Vec3::Y * top + across * (outside + out),
        )
    };
    if over {
        pieces.push(boxed(
            first - LINTEL_BEARING_M,
            last + LINTEL_BEARING_M,
            max.y,
            max.y + STONE_LINTEL_M,
            PROUD_M,
        ));
    }
    if !door {
        pieces.push(boxed(
            first - SILL_REACH_M,
            last + SILL_REACH_M,
            min.y - SILL_M,
            min.y,
            SILL_OUT_M,
        ));
    }
    for layer in low.y..=high.y {
        for (side, edge, outward) in [(low - step, first, -1.0), (high + step, last, 1.0)] {
            if !solid(side.with_y(layer)) {
                continue;
            }
            let reach = if (layer + outward as i32).rem_euclid(2) == 0 {
                JAMB_LONG_M
            } else {
                JAMB_SHORT_M
            };
            let (bottom, top) = (layer as f32 * CELL_M, (layer + 1) as f32 * CELL_M);
            let (a, b) = (edge, edge + outward * reach);
            pieces.push(boxed(a.min(b), a.max(b), bottom, top, PROUD_M));
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

    /// The stone kit with dressed stone, so it lays stone round its openings.
    fn dressed() -> Kit {
        let texture = crate::kit::Texture {
            path: "dressed".into(),
            metres: [1.0, 1.0],
            depth: 0.0,
            reflectance: crate::kit::matte(),
            strips: None,
        };
        let mut kit = kit();
        kit.textures.insert(Material::Dressed, texture);
        kit
    }

    fn stones(pieces: &[Piece]) -> Vec<(Vec3, Vec3)> {
        pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Dressed { min, max, .. } => Some((min.metres(), max.metres())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_window_in_stone_has_a_lintel_a_sill_and_jamb_stones_long_and_short() {
        let mut base = Base::default();
        for x in 0..6 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 0],
                panel: Some(if x == 2 { Panel::WINDOW } else { Panel::Solid }),
                kind: kit().kind(),
            });
        }
        let stones = stones(&lay(&base, &dressed()));
        // The window's cubes are 1 to 2 m up, a metre to a metre and a half along.
        let lintel = stones
            .iter()
            .filter(|(a, _)| (a.y - 2.0).abs() < 0.01)
            .count();
        let sill = stones
            .iter()
            .filter(|(_, b)| (b.y - 1.0).abs() < 0.01 && b.x - 1.5 > 0.1)
            .count();
        assert_eq!((lintel, sill), (1, 1), "{stones:?}");
        // A jamb stone each side for each of its two cubes' layers, long and short.
        let jambs: Vec<_> = stones
            .iter()
            .filter(|(a, b)| a.y >= 1.0 - 1e-3 && b.y <= 2.0 + 1e-3 && b.y - a.y > 0.4)
            .collect();
        assert_eq!(jambs.len(), 4, "{jambs:?}");
        let widths: BTreeSet<i32> = jambs
            .iter()
            .map(|(a, b)| ((b.x - a.x) * 100.0).round() as i32)
            .collect();
        assert_eq!(widths.len(), 2, "{widths:?}");
        // Each stands no more than a few centimetres out of the wall's faces.
        for (a, b) in &stones {
            assert!(a.z > -0.07 && b.z < 0.57, "{a} {b}");
        }
    }

    #[test]
    fn a_thinner_wall_of_another_kind_on_stone_has_a_cap_under_it() {
        let mut base = Base::default();
        let logs = Kind(1);
        for x in 0..4 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 0],
                panel: Some(Panel::Solid),
                kind: logs,
            });
        }
        // The bottom metre of it in stone.
        base.apply(BuildOp::Fill {
            min: [0, 0, 0],
            max: [3, 1, 0],
            cube: Some(crate::Cube::new(kit().kind(), Fill::Solid)),
        });
        let stones = stones(&lay(&base, &dressed()));
        assert_eq!(stones.len(), 4, "{stones:?}");
        for (a, b) in stones {
            assert!(a.y < 1.0 && b.y > 1.0 && b.y < 1.1, "{a} {b}");
        }
    }
}
