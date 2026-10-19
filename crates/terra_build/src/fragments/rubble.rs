//! The heap a fall leaves (D305): what lands and breaks raises the ground under it by its
//! volume, loosened as broken masonry is, and the heap rolls down to its angle of repose, as
//! a sandpile automaton's grains do, so what comes down after lands on what came down before and a
//! tower sinks into its own rubble rather than every block reaching the ground. The heap goes
//! as its pieces fade.

use super::Fragment;
use crate::CELL_M;
use glam::Vec3;
use std::collections::BTreeMap;

/// How much more room broken masonry takes than it did standing (rubble's bulking, 1.3 to
/// 1.6), less the share of it that lands again whole and is laid again then.
const BULKS: f32 = 1.0;
/// The steepest a heap stands, as a rise a metre: rubble's angle of repose, about 37°.
pub const REPOSE: f32 = 0.75;
/// How long a heap lies after it was last laid on, as its big pieces do, and how long it
/// takes to go, in seconds.
const LIES_S: f32 = 9.0;
const GOES_S: f32 = 2.0;
/// How high a grain of rubble is, in metres of a cell, and the most cells it rolls over.
const GRAIN_M: f32 = 0.1;
const MOST_ROLLS: usize = 48;

/// A heap's cell: the floor it lies on, how high it stands over it, in metres, and when it
/// was last laid on, in seconds on the heap's clock.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cell {
    floor: f32,
    high: f32,
    laid_s: f32,
}

/// The rubble of a base's falls, by column of cubes, in the base's own space.
#[derive(Clone, Debug, Default)]
pub struct Rubble {
    cells: BTreeMap<[i32; 2], Cell>,
}

fn column(at: Vec3) -> [i32; 2] {
    [
        (at.x / CELL_M).floor() as i32,
        (at.z / CELL_M).floor() as i32,
    ]
}

impl Rubble {
    /// How high the heap stands over a point at `now`, if there is one there: the top of it.
    pub fn top(&self, at: Vec3, now: f32) -> Option<f32> {
        let cell = self.cells.get(&column(at))?;
        let gone = ((now - cell.laid_s - LIES_S) / GOES_S).clamp(0.0, 1.0);
        let high = cell.high * (1.0 - gone);
        (high > 0.01).then_some(cell.floor + high)
    }

    /// A whole fragment broken where it landed, on `floor` (the ground or what is built,
    /// under the heap), at `now`: its volume laid on the cells under it as grains, each
    /// rolling down the heap while it stands steeper than its repose, as a sandpile's do.
    pub fn lay(&mut self, fragment: &Fragment, floor: f32, now: f32) {
        self.cells
            .retain(|_, cell| now - cell.laid_s < LIES_S + GOES_S);
        let side = ((fragment.size.x * fragment.size.z).sqrt() / CELL_M)
            .round()
            .max(1.0) as i32;
        let area = (side as f32 * CELL_M).powi(2);
        let rise = fragment.size.x * fragment.size.y * fragment.size.z * BULKS / area;
        let grains = (rise / GRAIN_M).ceil().max(1.0);
        let wide = side as f32 * CELL_M;
        let [x, z] = column(fragment.at - Vec3::new(wide, 0.0, wide) * 0.5 + CELL_M * 0.5);
        for dx in 0..side {
            for dz in 0..side {
                for _ in 0..grains as u32 {
                    self.roll([x + dx, z + dz], rise / grains, floor, now);
                }
            }
        }
    }

    /// A grain `high` metres of a cell laid at `at`, rolled down to where it rests.
    fn roll(&mut self, mut at: [i32; 2], high: f32, floor: f32, now: f32) {
        let steepest = REPOSE * CELL_M;
        let top = |cells: &BTreeMap<[i32; 2], Cell>, at: [i32; 2], or: f32| {
            cells.get(&at).map_or(or, |cell| cell.floor + cell.high)
        };
        for _ in 0..MOST_ROLLS {
            let here = top(&self.cells, at, floor);
            let lowest = [[1, 0], [-1, 0], [0, 1], [0, -1]]
                .map(|[dx, dz]| [at[0] + dx, at[1] + dz])
                .into_iter()
                .map(|next| (next, top(&self.cells, next, floor)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .expect("four neighbours");
            if here + high - lowest.1 <= steepest {
                break;
            }
            at = lowest.0;
        }
        let cell = self.cells.entry(at).or_insert(Cell {
            floor,
            high: 0.0,
            laid_s: now,
        });
        cell.high += high;
        cell.laid_s = now;
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{chunks, cube};
    use super::super::{Shape, burst};
    use super::*;
    use crate::Shape as CubeShape;
    use glam::IVec3;

    #[test]
    fn blocks_landing_in_one_place_heap_up_no_steeper_than_rubble_stands_and_the_heap_goes_later() {
        let column: Vec<(IVec3, crate::Cube)> = (0..40)
            .flat_map(|y| (0..4).flat_map(move |x| (0..4).map(move |z| IVec3::new(x, y, z))))
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect();
        let blocks = burst(&column, &|_| false, &chunks, 1.0, 5000);
        let mut rubble = Rubble::default();
        for block in blocks.iter().filter(|b| b.shape == Shape::Whole) {
            rubble.lay(block, 0.0, 0.0);
        }
        let middle = Vec3::new(1.0, 0.0, 1.0);
        let high = rubble.top(middle, 0.0).expect("a heap where they landed");
        // Higher than one block, lower than what stood.
        assert!(high > 1.5 && high < 20.0 * CELL_M, "{high} m");
        // Spread wider than the column it came from, at no more than its repose.
        let mut widest = 0.0f32;
        for step in 1..40 {
            let out = middle + Vec3::X * step as f32 * CELL_M;
            let (Some(here), Some(next)) = (
                rubble.top(out - Vec3::X * CELL_M, 0.0),
                rubble.top(out, 0.0),
            ) else {
                break;
            };
            widest = step as f32 * CELL_M;
            assert!(
                here - next <= REPOSE * CELL_M + GRAIN_M + 0.01,
                "{} over {CELL_M}",
                here - next
            );
        }
        assert!(widest > 3.0, "the heap is {widest} m out from its middle");
        assert!(
            rubble.top(middle, LIES_S + GOES_S + 1.0).is_none(),
            "the heap never goes"
        );
    }
}
