//! Footings (docs/building-kits.md, D194): a stretch of stone under every straight run of
//! wall standing on the ground.

use super::{Axis, Layout, Piece, Walls, hash, runs_of};
use crate::CELL_M;
use glam::IVec3;
use std::collections::{BTreeMap, BTreeSet};

/// A footing under every straight run of wall that stands on the ground: the cubes of the
/// first layer, with nothing of the wall under them.
pub(super) fn footings(walls: &Walls, ours: &[IVec3], layout: &mut Layout) {
    for axis in [Axis::X, Axis::Z] {
        let mut rows: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();
        for &at in ours.iter().filter(|at| at.y == 0) {
            let alone = !walls.runs(at, Axis::X) && !walls.runs(at, Axis::Z);
            if walls.runs(at, axis) || (alone && axis == Axis::X) {
                let (row, along) = axis.row_of(at);
                rows.entry(row).or_default().insert(along);
            }
        }
        for (row, cells) in rows {
            let keyed: BTreeSet<(i32, i32)> = cells.iter().map(|&a| (0, a)).collect();
            for (_, first, last) in runs_of(&keyed, |cell| cell) {
                let reach = 0.05;
                layout.pieces.push(Piece::Footing {
                    from: axis.point(row, first as f32 * CELL_M - reach, 0.0),
                    to: axis.point(row, (last + 1) as f32 * CELL_M + reach, 0.0),
                    seed: hash(&[axis as u32, row as u32, first as u32]),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::lay;
    use super::super::tests::{cabin, textured, wall};
    use super::*;
    use crate::Panel;
    use glam::Vec3;

    #[test]
    fn a_wall_on_the_ground_stands_on_a_footing_all_along_and_one_above_does_not() {
        let mut base = cabin();
        // A wall on the cabin's roof, on the next storey.
        wall(&mut base, (0..4).map(|x| [x, 1, 2]), Panel::Solid);
        let layout = lay(&base, &textured());
        let footings: Vec<(Vec3, Vec3)> = layout
            .pieces
            .iter()
            .filter_map(|piece| match *piece {
                Piece::Footing { from, to, .. } => Some((from, to)),
                _ => None,
            })
            .collect();
        assert!(
            footings
                .iter()
                .all(|(from, to)| from.y == 0.0 && to.y == 0.0)
        );
        // Under the middle of every cube of wall on the ground, a footing.
        for (at, _) in base.cubes().filter(|(at, _)| at.y == 0) {
            let middle = (at.as_vec3() + Vec3::splat(0.5)) * CELL_M;
            let on = footings.iter().any(|&(from, to)| {
                let along = (to - from).normalize();
                let t = (middle - from).dot(along);
                let off = middle - from - along * t;
                t >= 0.0 && t <= from.distance(to) && off.x.abs() < 1e-4 && off.z.abs() < 1e-4
            });
            assert!(on, "no footing under {at}");
        }
        // And none but under them, past their ends by a little.
        for &(from, to) in &footings {
            let along = (to - from).normalize();
            let mut at = from + along * 0.1;
            while at.distance(from) < from.distance(to) - 0.1 {
                let column = (at / CELL_M).floor().as_ivec3();
                assert!(
                    base.cube(column.with_y(0)).is_some(),
                    "a footing at {at} under no wall"
                );
                at += along * CELL_M * 0.5;
            }
        }
    }
}
