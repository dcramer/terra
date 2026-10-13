//! Stone walls (docs/building-kits.md, D179): each wall of a masonry kit's cubes laid as stone
//! sections, which the game draws with the kit's models (D177), a metre square where it
//! can, half-metre blocks where it cannot, and a column of cornerstones at every corner
//! and wall end.
//!
//! **The layout** is worked out from the cubes alone (D181), in bands two cubes high. A
//! row of cubes running one way takes a metre-wide section for each two cubes in a band,
//! every other band starting with a column of half blocks so the seams between sections
//! never line up. A cube with stone beside it both ways, or at the end of a row, or alone,
//! is a corner: cornerstones, a metre at a time, and a half block for an odd cube. A cube
//! cut open or glazed is a hole: a door or window is set in it once the building is up,
//! with a timber lintel over it. Each section's seed picks which of the kit's models it
//! is and turns it, so no two neighbours match.

use crate::logs::{LogKit, Piece};
use crate::{Base, CELL_M, Fill, Kind};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// Which of a masonry kit's models a stone piece is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stone {
    /// A section of wall a metre long and a metre high.
    Wall,
    /// A block half a metre each way.
    Half,
    /// A column of cornerstones half a metre square and a metre high.
    Corner,
}

/// How much bigger than its box a piece is drawn, so neighbours close up without a seam.
const OVERLAP: f32 = 1.02;
/// How deep a lintel is over an opening, and how far it runs into the wall either side.
const LINTEL_DEEP_M: f32 = 0.18;
const LINTEL_BEARING_M: f32 = 0.2;

/// Lays every wall of a base in the kit's kind as stone.
pub fn lay(base: &Base, kit: &LogKit) -> Vec<Piece> {
    let kind = kit.kind();
    let ours = |at: IVec3| {
        base.cube(at)
            .filter(|cube| cube.kind == kind && base.in_wall(at))
            .map(|cube| cube.fill)
    };
    let solid = |at: IVec3| ours(at) == Some(Fill::Solid);
    // Stone of ours either side along an axis, counting holes, which the wall runs through.
    let beside =
        |at: IVec3, step: IVec3| ours(at + step).is_some() as u8 + ours(at - step).is_some() as u8;
    let mut pieces = Vec::new();
    let mut corners: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut rows: BTreeMap<(bool, i32, i32), BTreeSet<i32>> = BTreeMap::new();
    for (at, cube) in base.cubes() {
        if cube.kind != kind || !base.in_wall(at) || cube.fill != Fill::Solid {
            continue;
        }
        let (x, z) = (beside(at, IVec3::X), beside(at, IVec3::Z));
        match (x, z) {
            (2, 0) => rows.entry((true, at.z, at.y)).or_default().insert(at.x),
            (0, 2) => rows.entry((false, at.x, at.y)).or_default().insert(at.z),
            _ => corners.insert(at.to_array()),
        };
    }

    // Each corner column, a metre at a time.
    let mut columns: BTreeMap<(i32, i32), Vec<i32>> = BTreeMap::new();
    for &[x, y, z] in &corners {
        columns.entry((x, z)).or_default().push(y);
    }
    for ((x, z), layers) in columns {
        for (low, count) in stacks(&layers) {
            let mut layer = low;
            while layer < low + count {
                let tall = if layer + 1 < low + count { 2 } else { 1 };
                let min = Vec3::new(x as f32, layer as f32, z as f32) * CELL_M;
                let size = Vec3::new(1.0, tall as f32, 1.0) * CELL_M;
                pieces.push(stone(
                    if tall == 2 {
                        Stone::Corner
                    } else {
                        Stone::Half
                    },
                    min,
                    size,
                    true,
                    seed(&[x, layer, z, 1]),
                ));
                layer += tall;
            }
        }
    }

    // Each row, in bands two cubes high: whole sections where both cubes of two columns are
    // there, half blocks elsewhere.
    for (&(along_x, row, layer), alongs) in &rows {
        // The upper cube of a band is laid with the lower.
        if layer.rem_euclid(2) == 1 && rows.contains_key(&(along_x, row, layer - 1)) {
            continue;
        }
        let upper = rows
            .get(&(along_x, row, layer + 1))
            .filter(|_| layer.rem_euclid(2) == 0);
        let both = |a: i32| alongs.contains(&a) && upper.is_some_and(|u| u.contains(&a));
        let cube = |along: i32, layer: i32| {
            if along_x {
                IVec3::new(along, layer, row)
            } else {
                IVec3::new(row, layer, along)
            }
        };
        let mut done: BTreeSet<(i32, i32)> = BTreeSet::new();
        let band = layer.div_euclid(2);
        for &a in alongs {
            if done.contains(&(a, layer)) {
                continue;
            }
            // Every other band starts on a half, staggering the seams.
            let run_start = !alongs.contains(&(a - 1));
            let stagger = run_start && band.rem_euclid(2) == 1;
            if !stagger && both(a) && both(a + 1) && !done.contains(&(a + 1, layer)) {
                let min = cube(a, layer).as_vec3() * CELL_M;
                let size = if along_x {
                    Vec3::new(2.0, 2.0, 1.0)
                } else {
                    Vec3::new(1.0, 2.0, 2.0)
                } * CELL_M;
                pieces.push(stone(
                    Stone::Wall,
                    min,
                    size,
                    along_x,
                    seed(&[a, layer, row, 2]),
                ));
                for along in [a, a + 1] {
                    done.insert((along, layer));
                    done.insert((along, layer + 1));
                }
                continue;
            }
            let layers: &[i32] = if both(a) {
                &[layer, layer + 1]
            } else {
                &[layer]
            };
            for &l in layers {
                let min = cube(a, l).as_vec3() * CELL_M;
                pieces.push(stone(
                    Stone::Half,
                    min,
                    Vec3::splat(CELL_M),
                    along_x,
                    seed(&[a, l, row, 3]),
                ));
                done.insert((a, l));
            }
        }
        // The upper cubes no section took.
        if let Some(upper) = upper {
            for &a in upper.iter().filter(|&&a| !done.contains(&(a, layer + 1))) {
                let min = cube(a, layer + 1).as_vec3() * CELL_M;
                pieces.push(stone(
                    Stone::Half,
                    min,
                    Vec3::splat(CELL_M),
                    along_x,
                    seed(&[a, layer + 1, row, 3]),
                ));
            }
        }
    }

    openings(base, kind, &solid, &mut pieces);
    pieces
}

/// Each hole in the walls, as the door or window set in it and a lintel over it where
/// there is wall above.
fn openings(base: &Base, kind: Kind, solid: &impl Fn(IVec3) -> bool, pieces: &mut Vec<Piece>) {
    let holes: BTreeSet<[i32; 3]> = base
        .cubes()
        .filter(|(at, cube)| cube.kind == kind && cube.fill != Fill::Solid && base.in_wall(*at))
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
            min,
            max,
            along_x,
            door,
        });
        // A timber lintel, into the wall either side, where the wall goes on over it.
        if solid(IVec3::new(low.x, high.y + 1, low.z)) {
            let reach = if along_x { Vec3::X } else { Vec3::Z } * LINTEL_BEARING_M;
            let across = if along_x { Vec3::Z } else { Vec3::X };
            let inset = across * CELL_M * 0.1;
            pieces.push(Piece::Board {
                min: Vec3::new(min.x, max.y - 0.02, min.z) - reach + inset,
                max: Vec3::new(max.x, max.y + LINTEL_DEEP_M, max.z) + reach - inset,
            });
        }
    }
}

/// Runs of consecutive layers in a sorted list: where each starts and how many.
fn stacks(layers: &[i32]) -> Vec<(i32, i32)> {
    let mut sorted = layers.to_vec();
    sorted.sort();
    let mut out: Vec<(i32, i32)> = Vec::new();
    for layer in sorted {
        match out.last_mut() {
            Some((low, count)) if *low + *count == layer => *count += 1,
            _ => out.push((layer, 1)),
        }
    }
    out
}

fn stone(shape: Stone, min: Vec3, size: Vec3, along_x: bool, seed: u32) -> Piece {
    Piece::Stone {
        shape,
        centre: min + size * 0.5,
        size: size * OVERLAP,
        along_x,
        seed,
    }
}

fn seed(parts: &[i32]) -> u32 {
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    for &p in parts {
        x ^= p as u32 as u64;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 31;
    }
    (x >> 16) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logs::Pattern;
    use crate::{BuildOp, Panel};

    fn kit() -> LogKit {
        LogKit {
            id: 2,
            pattern: Pattern::Masonry,
            ..LogKit::default()
        }
    }

    fn stones(pieces: &[Piece]) -> Vec<(Stone, Vec3)> {
        pieces
            .iter()
            .filter_map(|p| match *p {
                Piece::Stone { shape, size, .. } => Some((shape, size / OVERLAP)),
                _ => None,
            })
            .collect()
    }

    /// Every cube of a set is covered by exactly one piece's box.
    fn covers_each_once(pieces: &[Piece], cubes: &BTreeSet<[i32; 3]>) {
        let mut count: BTreeMap<[i32; 3], u32> = BTreeMap::new();
        for piece in pieces {
            let Piece::Stone { centre, size, .. } = *piece else {
                continue;
            };
            let size = size / OVERLAP;
            let low = ((centre - size * 0.5) / CELL_M).round().as_ivec3();
            let high = ((centre + size * 0.5) / CELL_M).round().as_ivec3();
            for x in low.x..high.x {
                for y in low.y..high.y {
                    for z in low.z..high.z {
                        *count.entry([x, y, z]).or_default() += 1;
                    }
                }
            }
        }
        for at in cubes {
            assert_eq!(count.get(at), Some(&1), "{at:?}");
        }
        assert_eq!(count.len(), cubes.len());
    }

    #[test]
    fn a_room_of_stone_is_sections_with_cornerstones_and_every_cube_is_laid_once() {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [7, 0, 5],
            kind: kit().kind(),
        });
        let pieces = lay(&base, &kit());
        let cubes: BTreeSet<[i32; 3]> = base
            .cubes()
            .filter(|(at, _)| base.in_wall(*at))
            .map(|(at, _)| at.to_array())
            .collect();
        covers_each_once(&pieces, &cubes);
        let stones = stones(&pieces);
        // Four corners, three metres of cornerstones each.
        assert_eq!(
            stones.iter().filter(|(s, _)| *s == Stone::Corner).count(),
            4 * 3
        );
        assert!(stones.iter().filter(|(s, _)| *s == Stone::Wall).count() > 10);
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
        let doors: Vec<_> = pieces
            .iter()
            .filter(|p| matches!(p, Piece::Opening { door: true, .. }))
            .collect();
        assert_eq!(doors.len(), 1);
        assert_eq!(
            pieces
                .iter()
                .filter(|p| matches!(p, Piece::Board { .. }))
                .count(),
            1
        );
        let cubes: BTreeSet<[i32; 3]> = base
            .cubes()
            .filter(|(at, cube)| cube.fill == Fill::Solid && base.in_wall(*at))
            .map(|(at, _)| at.to_array())
            .collect();
        covers_each_once(&pieces, &cubes);
    }

    #[test]
    fn the_seams_of_one_band_do_not_line_up_with_the_next() {
        let mut base = Base::default();
        for x in 0..12 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 0],
                panel: Some(Panel::Solid),
                kind: kit().kind(),
            });
        }
        let pieces = lay(&base, &kit());
        let seams = |band: i32| -> BTreeSet<i32> {
            pieces
                .iter()
                .filter_map(|p| match *p {
                    Piece::Stone {
                        shape: Stone::Wall,
                        centre,
                        ..
                    } if ((centre.y - 0.5) / 1.0).round() as i32 == band => {
                        Some(((centre.x - 0.5) / CELL_M).round() as i32)
                    }
                    _ => None,
                })
                .collect()
        };
        let (first, second) = (seams(0), seams(1));
        assert!(!first.is_empty() && !second.is_empty());
        assert!(first.is_disjoint(&second), "{first:?} {second:?}");
    }
}
