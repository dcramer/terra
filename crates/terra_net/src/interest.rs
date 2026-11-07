//! Where things are, by cell (D652): a hashed grid of cubes over a room, so what is near a
//! place is found without asking everything. Cubes, not squares of ground: a realm is round
//! and has caves under it, and a cube serves both.

use terra_gen::hash::QuickMap;

/// A place in a room, m.
pub type Place = [f64; 3];
/// A cube of a grid.
pub type Cell = [i32; 3];

/// The cube of a size a place is in.
pub fn cell_of(place: Place, size: f64) -> Cell {
    place.map(|along| (along / size).floor() as i32)
}

/// The distance between two places, squared.
pub fn distance_squared(a: Place, b: Place) -> f64 {
    (0..3).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum()
}

/// Things by the cube they are in.
pub struct Grid {
    size: f64,
    cells: QuickMap<Cell, Vec<u32>>,
}

impl Grid {
    pub fn new(size: f64) -> Grid {
        Grid {
            size,
            cells: QuickMap::default(),
        }
    }

    pub fn insert(&mut self, id: u32, place: Place) {
        self.cells
            .entry(cell_of(place, self.size))
            .or_default()
            .push(id);
    }

    pub fn remove(&mut self, id: u32, place: Place) {
        let cell = cell_of(place, self.size);
        if let Some(ids) = self.cells.get_mut(&cell) {
            if let Some(index) = ids.iter().position(|&other| other == id) {
                ids.swap_remove(index);
            }
            if ids.is_empty() {
                self.cells.remove(&cell);
            }
        }
    }

    /// A thing moved: nothing is touched while it stays in its cube.
    pub fn shift(&mut self, id: u32, from: Place, to: Place) {
        if cell_of(from, self.size) != cell_of(to, self.size) {
            self.remove(id, from);
            self.insert(id, to);
        }
    }

    /// Every thing in a cube any of which is within a reach of a place: all within the reach,
    /// and some up to a cube's diagonal beyond it, for the caller to measure.
    pub fn near(&self, place: Place, reach: f64, out: &mut Vec<u32>) {
        let low = cell_of(place.map(|along| along - reach), self.size);
        let high = cell_of(place.map(|along| along + reach), self.size);
        for x in low[0]..=high[0] {
            for y in low[1]..=high[1] {
                for z in low[2]..=high[2] {
                    if let Some(ids) = self.cells.get(&[x, y, z]) {
                        out.extend_from_slice(ids);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_finds_all_within_reach_and_nothing_far() {
        let mut grid = Grid::new(32.0);
        // Either side of the origin, where rounding toward zero would merge two cubes.
        grid.insert(1, [-1.0, 0.0, 0.0]);
        grid.insert(2, [1.0, 0.0, 0.0]);
        grid.insert(3, [500.0, 0.0, 0.0]);
        grid.insert(4, [0.0, -100.0, 0.0]);
        assert_ne!(
            cell_of([-1.0, 0.0, 0.0], 32.0),
            cell_of([1.0, 0.0, 0.0], 32.0)
        );
        let mut found = Vec::new();
        grid.near([0.0, 0.0, 0.0], 100.0, &mut found);
        found.sort_unstable();
        assert_eq!(found, [1, 2, 4]);
        // Moved far, then removed.
        grid.shift(2, [1.0, 0.0, 0.0], [600.0, 0.0, 0.0]);
        grid.remove(1, [-1.0, 0.0, 0.0]);
        found.clear();
        grid.near([0.0, 0.0, 0.0], 100.0, &mut found);
        assert_eq!(found, [4]);
        found.clear();
        grid.near([550.0, 0.0, 0.0], 60.0, &mut found);
        found.sort_unstable();
        assert_eq!(found, [2, 3]);
    }
}
