//! Drifting snow (D323): how much of what falls the ground at a place keeps against the
//! prevailing wind, from how sheltered it is. The wind scours a rise and drops what it carried
//! in the lee below, so lee slopes hold about twice the snow of windward ones and late snow
//! patches lie in the same hollows every year (research/2026-10-15-biome-demos.md, tundra).
//!
//! Shelter is Winstral's Sx (Winstral, Elder and Davis 2002), the steepest angle up to the
//! ground upwind within 100 m (`Region::shelter_deg`), the index snow models take for it; the
//! catch is a share of open level ground's snow, handed to the climate as `Place::catch`, so
//! the climate crate never reads terrain.

use crate::region::Region;
use glam::{Vec2, Vec3};

/// How far upwind shelter is looked for, as Winstral's Sx takes it for drifting snow.
pub const REACH_M: f32 = 100.0;
/// The catch of the most sheltered and the most exposed ground, and the shelter angle over
/// which it swings from open ground's 1 toward them: Sx of a few degrees either way is most of
/// the difference, as snow surveys on ridges find (lee slopes about twice windward ones).
pub const LEE_CATCH: f32 = 2.2;
pub const SCOURED_CATCH: f32 = 0.25;
const SWING_DEG: f32 = 8.0;

/// The catch of ground so sheltered (Sx, degrees: above 0 in a lee, below 0 exposed).
pub fn catch(shelter_deg: f32) -> f32 {
    let t = libm::tanhf(shelter_deg / SWING_DEG);
    if t >= 0.0 {
        1.0 + (LEE_CATCH - 1.0) * t
    } else {
        1.0 + (1.0 - SCOURED_CATCH) * t
    }
}

/// Which way the prevailing wind comes from, as metres east and north a metre: the climate's
/// bearing, clockwise from north.
pub fn wind_from(prevailing_from_deg: f32) -> Vec2 {
    let from = prevailing_from_deg.to_radians();
    Vec2::new(libm::sinf(from), libm::cosf(from))
}

/// The catch over a region's cells, against a wind from `from`.
#[derive(Clone, Debug)]
pub struct Drift {
    catches: Vec<f32>,
    shelters: Vec<f32>,
    region: Region,
}

impl Drift {
    pub fn over(region: &Region, from: Vec2) -> Self {
        let n = region.cells;
        let cells: Vec<usize> = (0..n * n).collect();
        let shelters =
            crate::maps::par_map(&cells, |&k| region.shelter_deg(k % n, k / n, from, REACH_M));
        let catches = shelters.iter().map(|&s| catch(s)).collect();
        Self {
            catches,
            shelters,
            region: Region {
                heights: Vec::new(),
                ..region.clone()
            },
        }
    }

    /// How sheltered a cell is, Winstral's Sx in degrees.
    pub fn shelter_at_cell(&self, column: usize, row: usize) -> f32 {
        self.shelters[row * self.region.cells + column]
    }

    /// The catch at a cell.
    pub fn at_cell(&self, column: usize, row: usize) -> f32 {
        self.catches[row * self.region.cells + column]
    }

    /// The catch in a direction from the centre, blended between the cells round it; open
    /// ground's outside the region.
    pub fn at(&self, direction: Vec3) -> f32 {
        let region = &self.region;
        let d = direction.normalize();
        let along = d.dot(region.middle.normalize());
        if along <= 0.0 {
            return 1.0;
        }
        let p = d * (region.middle.length() / along) - region.middle;
        let (x, y) = (p.dot(region.east), p.dot(region.north));
        let n = region.cells as f32;
        let fc = x / region.cell_m + n * 0.5 - 0.5;
        let fr = n * 0.5 - 0.5 - y / region.cell_m;
        if fc < 0.0 || fr < 0.0 || fc > n - 1.0 || fr > n - 1.0 {
            return 1.0;
        }
        let (c, r) = (fc as usize, fr as usize);
        let (c2, r2) = ((c + 1).min(region.cells - 1), (r + 1).min(region.cells - 1));
        let (tc, tr) = (fc - c as f32, fr - r as f32);
        let top = self.at_cell(c, r) * (1.0 - tc) + self.at_cell(c2, r) * tc;
        let bottom = self.at_cell(c, r2) * (1.0 - tc) + self.at_cell(c2, r2) * tc;
        top * (1.0 - tr) + bottom * tr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lee_of_a_ridge_catches_more_than_open_ground_and_its_crest_less() {
        // A ridge running north to south, the wind from the west, as the climate's is.
        let mut region = Region {
            middle: Vec3::Y * 30_000.0,
            east: Vec3::X,
            north: Vec3::NEG_Z,
            cells: 61,
            cell_m: 10.0,
            heights: vec![0.0; 61 * 61],
        };
        for row in 0..61 {
            for column in 0..61 {
                let x = column as f32 - 30.0;
                region.heights[row * 61 + column] = (30.0 - 3.0 * x.abs()).max(0.0);
            }
        }
        let west = wind_from(270.0);
        assert!((west - Vec2::new(-1.0, 0.0)).length() < 1e-5);
        let drift = Drift::over(&region, west);
        let crest = drift.at_cell(30, 30);
        let lee = drift.at_cell(34, 30);
        let windward = drift.at_cell(26, 30);
        let open = drift.at_cell(2, 30);
        assert!(crest < 0.8, "crest {crest}");
        assert!(lee > 1.6, "lee {lee}");
        assert!(lee > windward && windward <= 1.0 + 1e-3, "{windward}");
        assert!((open - 1.0).abs() < 0.05, "open {open}");
        // Blended between cells, and open ground's outside.
        let between = drift.at(region.direction(32, 30));
        assert!((between - drift.at_cell(32, 30)).abs() < 1e-3);
        assert_eq!(drift.at(Vec3::NEG_Y), 1.0);
    }

    #[test]
    fn turning_the_wind_moves_the_lee() {
        let mut region = Region {
            middle: Vec3::Y * 30_000.0,
            east: Vec3::X,
            north: Vec3::NEG_Z,
            cells: 61,
            cell_m: 10.0,
            heights: vec![0.0; 61 * 61],
        };
        for row in 0..61 {
            for column in 0..61 {
                let x = column as f32 - 30.0;
                region.heights[row * 61 + column] = (30.0 - 3.0 * x.abs()).max(0.0);
            }
        }
        let from_west = Drift::over(&region, wind_from(270.0));
        let from_east = Drift::over(&region, wind_from(90.0));
        // The east foot is the lee of a west wind and the windward side of an east one.
        assert!(from_west.at_cell(34, 30) > from_east.at_cell(34, 30) + 0.5);
        assert!(from_east.at_cell(26, 30) > from_west.at_cell(26, 30) + 0.5);
    }
}
