//! How masonry breaks into chunks (D305): what was left hanging cut into chunks of uneven
//! sizes, and a chunk that lands hard cut again into smaller ones, down to single stones.
//!
//! Shipped destruction breaks a wall into pieces made beforehand of uneven size (Red
//! Faction: Guerrilla's and Frostbite's Voronoi cells, Teardown's voxel volumes split where
//! they break), big ones that tip and tumble breaking into smaller ones where they land,
//! never a grid of equal blocks. Ours are boxes of cubes: the box round what falls is cut
//! across its longest side at an uneven place, again and again (a k-d split), until every
//! piece is no longer than a limit of its own; each piece then split into the cubes of it
//! that touch, so a chunk is never two walls with air between.

use super::unit;
use glam::IVec3;
use std::collections::BTreeSet;
use terra_gen::hash;

/// Where along its longest side a box is cut, at least and at most, as a share of it.
const CUT_FROM: f32 = 0.3;
const CUT_TO: f32 = 0.7;

/// Cubes at `cubes` cut into chunks, as lists of indices into it: each no longer along any
/// level side than its own limit, from `least` cubes to twice that (and shorter than
/// `below`), half that tall, and all of its cubes touching.
/// The same every time for the same cubes and `seed`.
pub(super) fn chunks(cubes: &[IVec3], least: i32, below: i32, seed: u32) -> Vec<Vec<usize>> {
    let least = least.max(1);
    let mut out = Vec::new();
    let mut todo: Vec<(Vec<usize>, u32)> = vec![((0..cubes.len()).collect(), seed)];
    while let Some((part, seed)) = todo.pop() {
        if part.is_empty() {
            continue;
        }
        let (low, high) = part.iter().fold(
            (IVec3::splat(i32::MAX), IVec3::splat(i32::MIN)),
            |(low, high), &i| (low.min(cubes[i]), high.max(cubes[i])),
        );
        let side = high - low + IVec3::ONE;
        let limit = (least + (unit(&[seed, 1]) * (least as f32 + 0.99)) as i32)
            .min(below - 1)
            .max(1);
        // Masonry parts along its bed joints: a chunk is a run of courses, about half as
        // tall as it is long, not a cube (a wall's face fell as a grid of equal boxes).
        let limits = IVec3::new(limit, (limit + 1) / 2, limit);
        if (0..3).all(|axis| side[axis] <= limits[axis]) {
            out.extend(touching(cubes, &part));
            continue;
        }
        // Across the side furthest over its limit, the level ones first where they tie.
        let axis = [0usize, 2, 1]
            .into_iter()
            .max_by_key(|&axis| {
                (
                    side[axis] * 2 * limit / limits[axis],
                    (hash::of(&[seed, axis as u32]) & 0xFF) as i32,
                )
            })
            .unwrap_or(0);
        let share = CUT_FROM + (CUT_TO - CUT_FROM) * unit(&[seed, 2]);
        let cut = low[axis] + ((side[axis] as f32 * share).round() as i32).clamp(1, side[axis] - 1);
        let (near, far): (Vec<usize>, Vec<usize>) =
            part.into_iter().partition(|&i| cubes[i][axis] < cut);
        todo.push((near, hash::of(&[seed, 3])));
        todo.push((far, hash::of(&[seed, 4])));
    }
    out
}

/// The shortest and longest length, in cubes, a timber course falls as.
const LENGTH_LEAST: i32 = 3;
const LENGTH_MOST: i32 = 7;

/// Timber at `cubes` cut into lengths, as lists of indices into it: each a run of cubes along
/// one axis, its grain, the way the most of them run through its first cube (a course of logs
/// along its wall, a post up), from three to seven cubes long, a short end left on the length
/// before it, so a log wall falls as its logs snapped into a few lengths, never a grid of
/// sawn blocks (castle film round 7: a tower's timber storey fell as one-cube blocks, each
/// showing its sawn end at every joint). The same every time for the same cubes and `seed`.
pub(super) fn lengths(cubes: &[IVec3], seed: u32) -> Vec<Vec<usize>> {
    let index: std::collections::BTreeMap<[i32; 3], usize> = cubes
        .iter()
        .enumerate()
        .map(|(i, at)| (at.to_array(), i))
        .collect();
    let mut taken = vec![false; cubes.len()];
    let mut order: Vec<usize> = (0..cubes.len()).collect();
    order.sort_by_key(|&i| (cubes[i].y, cubes[i].x, cubes[i].z));
    let run = |from: IVec3, step: IVec3, taken: &[bool]| -> Vec<usize> {
        let mut out = Vec::new();
        let mut at = from;
        while let Some(&i) = index.get(&at.to_array()) {
            if taken[i] {
                break;
            }
            out.push(i);
            at += step;
        }
        out
    };
    let mut out = Vec::new();
    for first in order {
        if taken[first] {
            continue;
        }
        let from = cubes[first];
        // Level first: a course of logs runs along its wall; a post stands up.
        let grain = [IVec3::X, IVec3::Z, IVec3::Y]
            .into_iter()
            .map(|step| run(from, step, &taken))
            .enumerate()
            .max_by_key(|(axis, cubes)| (cubes.len(), usize::MAX - axis))
            .map(|(_, cubes)| cubes)
            .unwrap_or_default();
        let mut left = grain.as_slice();
        let mut salt = 0u32;
        while !left.is_empty() {
            salt += 1;
            let span = (LENGTH_MOST - LENGTH_LEAST + 1) as f32;
            let length = (LENGTH_LEAST
                + (unit(&[seed, from.x as u32, from.y as u32, from.z as u32, salt]) * span) as i32)
                .min(LENGTH_MOST) as usize;
            // A stub shorter than the least goes with this one, if that stays drawable.
            let length = if left.len() < length + LENGTH_LEAST as usize
                && left.len() <= super::MOST_DRAWN as usize
            {
                left.len()
            } else {
                length.min(left.len())
            };
            let (piece, rest) = left.split_at(length);
            for &i in piece {
                taken[i] = true;
            }
            out.push(piece.to_vec());
            left = rest;
        }
    }
    out
}

/// The cubes of a part, split into those that touch each other face to face.
fn touching(cubes: &[IVec3], part: &[usize]) -> Vec<Vec<usize>> {
    let mut left: BTreeSet<[i32; 3]> = part.iter().map(|&i| cubes[i].to_array()).collect();
    let index: std::collections::BTreeMap<[i32; 3], usize> =
        part.iter().map(|&i| (cubes[i].to_array(), i)).collect();
    let mut out = Vec::new();
    while let Some(&first) = left.iter().next() {
        left.remove(&first);
        let mut group = vec![index[&first]];
        let mut todo = vec![IVec3::from_array(first)];
        while let Some(at) = todo.pop() {
            for step in [
                IVec3::X,
                IVec3::NEG_X,
                IVec3::Y,
                IVec3::NEG_Y,
                IVec3::Z,
                IVec3::NEG_Z,
            ] {
                let next = (at + step).to_array();
                if left.remove(&next) {
                    group.push(index[&next]);
                    todo.push(at + step);
                }
            }
        }
        out.push(group);
    }
    out
}

/// A box so many cubes a side cut into smaller boxes where it lands hard, as a chunk of
/// masonry cracks into a few: each its low corner and its size, in cubes. A box of one cube
/// is left whole.
pub(super) fn crack(side: IVec3, seed: u32) -> Vec<(IVec3, IVec3)> {
    let cubes: Vec<IVec3> = (0..side.x)
        .flat_map(|x| (0..side.y).flat_map(move |y| (0..side.z).map(move |z| IVec3::new(x, y, z))))
        .collect();
    // Into pieces about a third of it across, and some halves.
    let least = ((side.max_element() + 1) / 3).max(1);
    chunks(&cubes, least, side.max_element(), seed)
        .into_iter()
        .map(|part| {
            let (low, high) = part.iter().fold(
                (IVec3::splat(i32::MAX), IVec3::splat(i32::MIN)),
                |(low, high), &i| (low.min(cubes[i]), high.max(cubes[i])),
            );
            (low, high - low + IVec3::ONE)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wall_is_cut_into_uneven_chunks_that_between_them_hold_every_cube_once() {
        // A wall 40 cubes long, 2 thick and 20 high, and a ring of one round a hollow.
        let wall: Vec<IVec3> = (0..40)
            .flat_map(|x| (0..20).flat_map(move |y| (0..2).map(move |z| IVec3::new(x, y, z))))
            .collect();
        let parts = chunks(&wall, 3, i32::MAX, 9);
        let mut seen: Vec<usize> = parts.iter().flatten().copied().collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..wall.len()).collect::<Vec<_>>());
        let sides: BTreeSet<[i32; 3]> = parts
            .iter()
            .map(|part| {
                let (low, high) = part.iter().fold(
                    (IVec3::splat(i32::MAX), IVec3::splat(i32::MIN)),
                    |(low, high), &i| (low.min(wall[i]), high.max(wall[i])),
                );
                (high - low + IVec3::ONE).to_array()
            })
            .collect();
        // No side longer than twice the least, and not one size over and over: a grid.
        assert!(
            sides.iter().all(|side| side.iter().all(|&s| s <= 6)),
            "{sides:?}"
        );
        assert!(sides.len() >= 5, "only {} sizes: {sides:?}", sides.len());
        // Far fewer chunks than cubes, and more than a handful.
        assert!(
            parts.len() > 20 && parts.len() < wall.len() / 6,
            "{}",
            parts.len()
        );
    }

    #[test]
    fn a_wall_parts_along_its_courses_into_chunks_longer_than_they_are_tall() {
        let wall: Vec<IVec3> = (0..40)
            .flat_map(|x| (0..20).flat_map(move |y| (0..2).map(move |z| IVec3::new(x, y, z))))
            .collect();
        let parts = chunks(&wall, 2, i32::MAX, 5);
        let (mut long, mut tall) = (0.0, 0.0);
        for part in &parts {
            let (low, high) = part.iter().fold(
                (IVec3::splat(i32::MAX), IVec3::splat(i32::MIN)),
                |(low, high), &i| (low.min(wall[i]), high.max(wall[i])),
            );
            let side = high - low + IVec3::ONE;
            long += side.x as f32;
            tall += side.y as f32;
        }
        assert!(long > tall * 1.5, "{long} cubes long to {tall} tall in all");
    }

    #[test]
    fn a_chunk_is_never_two_walls_with_air_between() {
        // Two walls three cubes apart: whatever the cuts, no chunk holds cubes of both.
        let walls: Vec<IVec3> = (0..12)
            .flat_map(|x| (0..6).flat_map(move |y| [IVec3::new(x, y, 0), IVec3::new(x, y, 4)]))
            .collect();
        for seed in 0..20 {
            for part in chunks(&walls, 6, i32::MAX, seed) {
                let z: BTreeSet<i32> = part.iter().map(|&i| walls[i].z).collect();
                assert_eq!(z.len(), 1, "a chunk spans the gap: {z:?}");
            }
        }
    }

    #[test]
    fn a_landing_chunk_cracks_into_a_few_smaller_boxes_filling_it() {
        let side = IVec3::new(6, 4, 2);
        let pieces = crack(side, 3);
        let volume: i32 = pieces.iter().map(|(_, s)| s.x * s.y * s.z).sum();
        assert_eq!(volume, 48);
        assert!(pieces.len() >= 3 && pieces.len() <= 24, "{}", pieces.len());
        assert!(pieces.iter().all(|(_, s)| s.max_element() < 6));
        let sizes: BTreeSet<[i32; 3]> = pieces.iter().map(|(_, s)| s.to_array()).collect();
        assert!(sizes.len() >= 2, "{sizes:?}");
        assert_eq!(crack(IVec3::ONE, 3), vec![(IVec3::ZERO, IVec3::ONE)]);
    }
}
