//! Where the water is (D45, D322): the sea, and every lake of a region filled to its spill
//! point (`lakes`), answered by position as `terra_core::Water` asks it. The region is the
//! land round a place, sampled finely enough for a lake's shore: lakes beyond it are not yet
//! asked about (docs/demos.md, what remains).

use crate::Land;
use crate::lakes::{self, Lake, Lakes};
use crate::region::Region;
use glam::Vec3;

/// The water of a region and the world's sea.
#[derive(Clone, Debug)]
pub struct Waters {
    region: Region,
    lakes: Lakes,
    radius_m: f32,
    sea_radius_m: Option<f32>,
}

impl Waters {
    /// The lakes of the land round a direction, `size_m` across in cells of `cell_m`, and the
    /// sea.
    pub fn round(land: &Land, toward: Vec3, size_m: f32, cell_m: f32) -> Self {
        let region = Region::sample(land, toward, size_m, cell_m);
        let sea = land.ground.sea_radius_m().map(|r| r - land.radius_m());
        let lakes = lakes::fill(&region, sea);
        Self {
            region,
            lakes,
            radius_m: land.radius_m(),
            sea_radius_m: land.ground.sea_radius_m(),
        }
    }

    /// The region the lakes were found in.
    pub fn region(&self) -> &Region {
        &self.region
    }

    /// Its lakes.
    pub fn lakes(&self) -> &[Lake] {
        &self.lakes.lakes
    }

    /// The cells of a lake, column and row.
    pub fn cells_of(&self, lake: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        let n = self.region.cells;
        self.lakes
            .lake
            .iter()
            .enumerate()
            .filter(move |(_, l)| **l == Some(lake as u32))
            .map(move |(k, _)| (k % n, k / n))
    }

    /// Which lake a cell is in, if any.
    pub fn lake_at(&self, column: usize, row: usize) -> Option<usize> {
        self.lakes.lake[row * self.region.cells + column].map(|l| l as usize)
    }

    /// The surface of the lake at or nearest a direction, within so many cells of it: the
    /// highest such, where two meet. What a shore's wet band and a bed's darkening are measured
    /// from.
    pub fn lake_near(&self, direction: Vec3, cells: usize) -> Option<f32> {
        let (c, r) = self.region.cell_of(direction)?;
        let n = self.region.cells as i64;
        let reach = cells as i64;
        let mut level: Option<f32> = None;
        for dr in -reach..=reach {
            for dc in -reach..=reach {
                let (cc, rr) = (c as i64 + dc, r as i64 + dr);
                if cc < 0 || rr < 0 || cc >= n || rr >= n {
                    continue;
                }
                if let Some(lake) = self.lake_at(cc as usize, rr as usize) {
                    let here = self.lake_radius_m(lake);
                    level = Some(level.map_or(here, |l: f32| l.max(here)));
                }
            }
        }
        level
    }

    /// The direction to the dry ground nearest a direction over a lake: its shore, there. None
    /// if the direction is over no lake.
    pub fn shore_of(&self, direction: Vec3) -> Option<Vec3> {
        let (c, r) = self.region.cell_of(direction)?;
        self.lake_at(c, r)?;
        let n = self.region.cells as i64;
        let mut best: Option<(i64, (usize, usize))> = None;
        for ring in 1..n {
            if best.is_some_and(|(d, _)| d <= ring * ring) {
                break;
            }
            for dr in -ring..=ring {
                for dc in -ring..=ring {
                    if dr.abs() != ring && dc.abs() != ring {
                        continue;
                    }
                    let (cc, rr) = (c as i64 + dc, r as i64 + dr);
                    if cc < 0 || rr < 0 || cc >= n || rr >= n {
                        continue;
                    }
                    if self.lake_at(cc as usize, rr as usize).is_some() {
                        continue;
                    }
                    let d = dc * dc + dr * dr;
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, (cc as usize, rr as usize)));
                    }
                }
            }
        }
        best.map(|(_, (c, r))| self.region.direction(c, r))
    }

    /// The surface of the lake over a direction, if one is.
    pub fn lake_level(&self, direction: Vec3) -> Option<f32> {
        let (c, r) = self.region.cell_of(direction)?;
        self.lake_at(c, r).map(|lake| self.lake_radius_m(lake))
    }

    /// The distance from the centre to a lake's surface.
    pub fn lake_radius_m(&self, lake: usize) -> f32 {
        self.radius_m + self.lakes.lakes[lake].level_m
    }
}

impl terra_core::Water for Waters {
    /// A lake's surface where a lake is; the sea's elsewhere, where there is a sea.
    fn level(&self, direction: Vec3) -> Option<f32> {
        if let Some((c, r)) = self.region.cell_of(direction)
            && let Some(lake) = self.lake_at(c, r)
        {
            return Some(self.lake_radius_m(lake));
        }
        self.sea_radius_m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::MountainLake;
    use crate::{PLANET_START, ground, start_climate};
    use terra_core::{PlanetSpec, Seed, Water};

    #[test]
    fn a_lake_stands_at_its_level_over_its_own_ground_and_the_sea_elsewhere() {
        let land = Land::new(
            ground::world(PlanetSpec {
                seed: Seed(1),
                radius_m: 30_000.0,
            }),
            start_climate().unwrap(),
        );
        let site = land
            .mountain_lake(PLANET_START, &MountainLake::DEMO)
            .expect("a lake");
        let waters = Waters::round(&land, site.toward, 3000.0, 16.0);
        let level = waters.level(site.toward).expect("water over the lake");
        // Over the sea: the lake's water stands well above it, and above its own ground.
        assert!(level > land.sea_radius_m() + 100.0);
        assert!(level > land.surface(site.toward).length());
        // Its level is the site's, found on a coarser grid, within that grid's error.
        assert!(
            (level - land.sea_radius_m() - site.level_m).abs() < 12.0,
            "{level}"
        );
        // On its shore, dry ground: only the sea's level, which is far below.
        assert_eq!(waters.level(site.shore), land.ground.sea_radius_m());
        // The region's cells come back from their own directions.
        let region = waters.region();
        for (c, r) in [(0, 0), (40, 77), (region.cells - 1, region.cells - 1)] {
            assert_eq!(region.cell_of(region.direction(c, r)), Some((c, r)));
        }
    }
}
