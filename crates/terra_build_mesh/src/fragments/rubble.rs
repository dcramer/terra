//! The heap a fall leaves (D305): what lands and breaks raises the ground under it by its
//! volume, loosened as broken masonry is, and the heap rolls down to its angle of repose, as
//! a sandpile automaton's grains do, so what comes down after lands on what came down before and a
//! tower sinks into its own rubble rather than every block reaching the ground. The heap goes
//! as its pieces fade.

use super::Fragment;
use glam::{IVec3, Vec3};
use std::collections::BTreeMap;
use terra_build::{CELL_M, Cube};

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

/// What came down, by column of cubes: how low, in metres, the lowest of it in each was.
/// What lay on it, a heap or a fragment at rest, comes down with it ([`Rubble::fell`],
/// [`Fragment::wake`]).
pub type Gone = BTreeMap<[i32; 2], f32>;

/// The columns `fell` came down from, each with the bottom of its lowest cube.
pub fn gone(fell: &[(IVec3, Cube)]) -> Gone {
    let mut gone = Gone::new();
    for (at, _) in fell {
        let low = at.y as f32 * CELL_M;
        gone.entry([at.x, at.z])
            .and_modify(|was| *was = was.min(low))
            .or_insert(low);
    }
    gone
}

impl Fragment {
    /// Sets it falling again if it lay on what has now come down: a piece at rest on a
    /// keep's roof falls with the keep rather than hanging where the roof was. Whether it
    /// did.
    pub fn wake(&mut self, gone: &Gone) -> bool {
        if !self.still {
            return false;
        }
        let low = gone.get(&column(self.at));
        if low.is_none_or(|low| self.at.y < *low) {
            return false;
        }
        self.still = false;
        true
    }
}

impl Rubble {
    /// What came down takes the heap lying on it, or over it, with it: a heap laid on a
    /// keep's roof by what fell off it as it burned, left standing when the keep fell, held
    /// the keep's own blocks up at its roof line for seconds (castle film round 2).
    pub fn fell(&mut self, gone: &Gone) {
        self.cells
            .retain(|at, cell| gone.get(at).is_none_or(|low| cell.floor < *low));
    }

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
        // What it landed on may be this heap: the floor under the heap is its cell's. Taken
        // as the floor, the heap's top became the floor of every cell new beside it, so the
        // heap stood as a column on itself and the castle's keep rose into a dome of blocks
        // resting on nothing seen, over its own roof (2026-10-21).
        let floor = self
            .cells
            .get(&column(fragment.at))
            .map_or(floor, |cell| cell.floor.min(floor));
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
    use super::super::{GRAVITY, Shape, burst};
    use super::*;
    use terra_build::Shape as CubeShape;

    #[test]
    fn blocks_landing_in_one_place_heap_up_no_steeper_than_rubble_stands_and_the_heap_goes_later() {
        let column: Vec<(IVec3, terra_build::Cube)> = (0..40)
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

    #[test]
    fn blocks_landing_on_their_own_heap_raise_it_no_higher_than_on_the_ground() {
        // As the game lays them: each on what it landed on, the heap laid before included.
        let column: Vec<(IVec3, terra_build::Cube)> = (0..40)
            .flat_map(|y| (0..4).flat_map(move |x| (0..4).map(move |z| IVec3::new(x, y, z))))
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect();
        let blocks = burst(&column, &|_| false, &chunks, 1.0, 5000);
        let (mut on_ground, mut on_heap) = (Rubble::default(), Rubble::default());
        for block in blocks.iter().filter(|b| b.shape == Shape::Whole) {
            on_ground.lay(block, 0.0, 0.0);
            let under = on_heap.top(block.at, 0.0).unwrap_or(0.0);
            on_heap.lay(block, under, 0.0);
        }
        let middle = Vec3::new(1.0, 0.0, 1.0);
        let reference = on_ground.top(middle, 0.0).expect("a heap");
        let high = on_heap.top(middle, 0.0).expect("a heap");
        assert!(
            (high - reference).abs() < 0.1 * reference,
            "{high} m on its own heap, {reference} m on the ground"
        );
    }

    #[test]
    fn what_lay_on_a_tower_comes_down_when_the_tower_does() {
        // A tower 4 cubes a side, 30 high: something broke on its roof and lies there, a
        // heap under it, and beside the tower another heap on the ground.
        let tower: Vec<(IVec3, terra_build::Cube)> = (0..30)
            .flat_map(|y| (0..4).flat_map(move |x| (0..4).map(move |z| IVec3::new(x, y, z))))
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect();
        let roof = 30.0 * CELL_M;
        let mut piece = burst(&tower[..1], &|_| true, &chunks, 1.0, 1)[0];
        piece.at = Vec3::new(1.0, roof + 0.1, 1.0);
        piece.still = true;
        let mut beside = piece;
        beside.at = Vec3::new(4.0, 0.1, 1.0);
        let mut rubble = Rubble::default();
        rubble.lay(&piece, roof, 0.0);
        rubble.lay(&beside, 0.0, 0.0);
        assert!(rubble.top(piece.at, 0.0).is_some_and(|top| top > roof));
        // The tower comes down.
        let gone = gone(&tower);
        rubble.fell(&gone);
        assert!(
            piece.wake(&gone),
            "the piece on the roof hangs where the roof was"
        );
        assert!(!beside.wake(&gone), "the piece beside it stirred");
        let heap = rubble.top(piece.at, 0.0);
        assert!(
            heap.is_none_or(|top| top < 1.0),
            "a heap stands at {heap:?} m over nothing"
        );
        assert!(
            rubble.top(beside.at, 0.0).is_some(),
            "the heap beside it went too"
        );
        // Woken, it falls to the ground in the time a drop from the roof takes, not later.
        let mut fell_s = None;
        for step in 0..240 {
            let floor = |at: Vec3| rubble.top(at, 0.0).unwrap_or(0.0);
            piece.step(1.0 / 60.0, &floor, &|_| false);
            if piece.at.y < 1.0 {
                fell_s = Some(step as f32 / 60.0);
                break;
            }
        }
        let drop_s = libm::sqrtf(2.0 * roof / GRAVITY);
        assert!(
            fell_s.is_some_and(|s| s < drop_s * 1.3),
            "it came down in {fell_s:?} s; a drop from the roof takes {drop_s:.1} s"
        );
    }
}
