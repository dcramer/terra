//! A square of a world's ground sampled on a grid, level at its middle: its heights, and from
//! them how steep and which way each cell faces, and how sheltered from the prevailing wind.
//! What the field maps draw and site queries search (D326). Rows run from north to south and
//! columns from west to east, as a map is read.

use crate::Land;
use glam::{Vec2, Vec3};

/// A grid over a square of ground.
#[derive(Clone, Debug)]
pub struct Region {
    /// The middle, on the ground's sphere at the nominal radius, and east and north there.
    pub middle: Vec3,
    pub east: Vec3,
    pub north: Vec3,
    /// How many cells along a side, and how wide each is, in metres.
    pub cells: usize,
    pub cell_m: f32,
    /// The ground's height over the nominal radius at each cell's middle, row by row from
    /// the north.
    pub heights: Vec<f32>,
}

impl Region {
    /// The ground round a direction from the centre, `size_m` across in cells of `cell_m`.
    pub fn sample(land: &Land, toward: Vec3, size_m: f32, cell_m: f32) -> Self {
        let toward = toward.normalize();
        let middle = toward * land.radius_m();
        let (east, north) = (land.frame.east(middle), land.frame.north(middle));
        let cells = (size_m / cell_m).round().max(2.0) as usize;
        let mut region = Self {
            middle,
            east,
            north,
            cells,
            cell_m,
            heights: Vec::new(),
        };
        let directions: Vec<Vec3> = (0..cells * cells)
            .map(|k| region.direction(k % cells, k / cells))
            .collect();
        region.heights = crate::maps::par_map(&directions, |&d| land.ground.height_m(d));
        region
    }

    /// Metres east and north of the middle of a cell.
    pub fn offset(&self, column: usize, row: usize) -> Vec2 {
        let half = self.cells as f32 * 0.5;
        Vec2::new(
            (column as f32 + 0.5 - half) * self.cell_m,
            (half - row as f32 - 0.5) * self.cell_m,
        )
    }

    /// The direction from the centre to a cell's middle.
    pub fn direction(&self, column: usize, row: usize) -> Vec3 {
        let at = self.offset(column, row);
        (self.middle + self.east * at.x + self.north * at.y).normalize()
    }

    /// The point on the ground at a cell's middle.
    pub fn point(&self, land: &Land, column: usize, row: usize) -> Vec3 {
        self.direction(column, row) * (land.radius_m() + self.height(column, row))
    }

    pub fn height(&self, column: usize, row: usize) -> f32 {
        self.heights[row * self.cells + column]
    }

    /// The height at a place given in metres east and north of the middle, between the cells
    /// round it; clamped at the edge.
    pub fn height_at(&self, at: Vec2) -> f32 {
        let half = self.cells as f32 * 0.5;
        let fx = (at.x / self.cell_m + half - 0.5).clamp(0.0, (self.cells - 1) as f32);
        let fy = (half - 0.5 - at.y / self.cell_m).clamp(0.0, (self.cells - 1) as f32);
        let (x0, y0) = (fx as usize, fy as usize);
        let (x1, y1) = ((x0 + 1).min(self.cells - 1), (y0 + 1).min(self.cells - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let top = self.height(x0, y0) * (1.0 - tx) + self.height(x1, y0) * tx;
        let bottom = self.height(x0, y1) * (1.0 - tx) + self.height(x1, y1) * tx;
        top * (1.0 - ty) + bottom * ty
    }

    /// How the ground rises at a cell, metres a metre east and north, from its neighbours.
    pub fn gradient(&self, column: usize, row: usize) -> Vec2 {
        let last = self.cells - 1;
        let (w, e) = (column.saturating_sub(1), (column + 1).min(last));
        let (n, s) = (row.saturating_sub(1), (row + 1).min(last));
        Vec2::new(
            (self.height(e, row) - self.height(w, row)) / ((e - w) as f32 * self.cell_m),
            (self.height(column, n) - self.height(column, s)) / ((s - n) as f32 * self.cell_m),
        )
    }

    /// Which way the ground faces at a cell.
    pub fn normal(&self, column: usize, row: usize) -> Vec3 {
        let up = self.direction(column, row);
        let rise = self.gradient(column, row);
        (up - self.east * rise.x - self.north * rise.y).normalize()
    }

    /// How steep the ground is at a cell, in degrees.
    pub fn slope_deg(&self, column: usize, row: usize) -> f32 {
        libm::atanf(self.gradient(column, row).length()).to_degrees()
    }

    /// Winstral's Sx (Winstral, Elder and Davis 2002), the standard index of how sheltered
    /// ground is from a wind: the steepest angle up to the ground upwind within `reach_m`, in
    /// degrees. Above 0 sheltered, in the lee of what is upwind; below 0 exposed, as a crest
    /// is. `from` is the way the wind comes from, east and north.
    pub fn shelter_deg(&self, column: usize, row: usize, from: Vec2, reach_m: f32) -> f32 {
        let at = self.offset(column, row);
        let here = self.height(column, row);
        let step = (self.cell_m * 0.5).min(10.0);
        let mut steepest = f32::MIN;
        let mut d = step;
        while d <= reach_m {
            let rise = self.height_at(at + from * d) - here;
            steepest = steepest.max(libm::atanf(rise / d));
            d += step;
        }
        steepest.to_degrees()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ground, start_climate};
    use terra_core::{PlanetSpec, Seed};

    fn land() -> Land {
        Land::new(
            ground::world(PlanetSpec {
                seed: Seed(1),
                radius_m: 30_000.0,
            }),
            start_climate().unwrap(),
        )
    }

    #[test]
    fn a_regions_heights_are_the_grounds_and_north_is_up_the_map() {
        let land = land();
        let region = Region::sample(&land, Vec3::Y, 2000.0, 100.0);
        assert_eq!(region.cells, 20);
        for (c, r) in [(0, 0), (7, 13), (19, 19)] {
            let direction = region.direction(c, r);
            assert!((region.height(c, r) - land.ground.height_m(direction)).abs() < 1e-4);
        }
        // The top row is the north edge: further north than the bottom one.
        let north = land.frame.latitude(region.direction(10, 0));
        let south = land.frame.latitude(region.direction(10, 19));
        assert!(north > south);
        // Between the cells, the height is between theirs.
        let a = region.offset(3, 3);
        let b = region.offset(4, 3);
        let between = region.height_at((a + b) * 0.5);
        let (ha, hb) = (region.height(3, 3), region.height(4, 3));
        assert!(between >= ha.min(hb) - 1e-3 && between <= ha.max(hb) + 1e-3);
    }

    #[test]
    fn a_crest_is_exposed_and_its_lee_sheltered() {
        // A ridge running north to south, the wind from the west: the crest sees nothing
        // higher upwind, the foot of its east side sees the crest.
        let mut region = Region {
            middle: Vec3::Y * 30_000.0,
            east: Vec3::X,
            north: Vec3::Z,
            cells: 41,
            cell_m: 10.0,
            heights: vec![0.0; 41 * 41],
        };
        for row in 0..41 {
            for column in 0..41 {
                let x = column as f32 - 20.0;
                region.heights[row * 41 + column] = (40.0 - 4.0 * x.abs()).max(0.0);
            }
        }
        let west = Vec2::new(-1.0, 0.0);
        let crest = region.shelter_deg(20, 20, west, 100.0);
        let lee = region.shelter_deg(24, 20, west, 100.0);
        assert!(crest < 0.0, "{crest}");
        assert!(lee > 10.0, "{lee}");
        assert!((region.slope_deg(18, 20) - libm::atanf(0.4).to_degrees()).abs() < 0.5);
    }
}
