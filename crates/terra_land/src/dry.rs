//! Dry ground for what the seed stands on the land (the gate, the den's mouth): whether water
//! stands at a place as the world draws it (`round::Round`: its lakes filled on a ten-metre
//! grid ten kilometres across, its patterned ground's pools on a two-metre one), asked
//! without working that land out, which takes ten seconds and more.
//!
//! The survey's grid alone (`survey::Grid`, 50 m cells over the land searched) misses water
//! two ways, and a gate stood on a lake's bed on four of seeds 1 to 24, a den's mouth on one:
//! a lake that reaches past the grid's edge spills off it and reads as dry ground
//! (`lakes::fill`: water leaves by the edge), and a pond of a few cells falls between its
//! samples (as seed 7's first town found, `town::MarketTown::dry_m`). So the lakes are
//! filled twice here, round the start as the world's are: on the survey's cells over the
//! world's own ten kilometres, where no lake near the start is cut off, and on the world's
//! ten-metre cells over three kilometres, which holds whole every pond near the land
//! searched. Measured against the world's own fill on the survey's cells round five starts
//! (seeds 2, 4, 5, 8, 9): the two together miss none of its water, and call wet at most 14
//! cells of 1,296 that it draws dry; either alone misses up to 767.

use crate::Land;
use crate::round::{self, LAKES_ACROSS_M};
use crate::survey::{CELL_M, Grid};
use crate::water::Waters;
use glam::Vec3;

/// Ponds are found on the world's ten-metre cells over this much land round the start: the
/// land a gate or a den is searched for on and 500 m more each way.
const NEAR_ACROSS_M: f32 = 3_000.0;
/// A patterned ground's pools are looked for over a window this far across round a place.
const POOL_ACROSS_M: f32 = 120.0;

/// The water round a start, for asking whether places near it are dry.
pub struct Dry<'a> {
    land: &'a Land,
    /// The lakes on the survey's cells, over as much land as the world fills its lakes on.
    wide: Waters,
    /// The lakes on the world's own cells, over the land near the start.
    near: Waters,
    /// Whether the world finds the patterned ground's pools here (`round::Round::new`).
    pooled: bool,
}

impl<'a> Dry<'a> {
    /// The water round a start.
    pub fn round(land: &'a Land, start: Vec3) -> Self {
        let wide = Waters::round(land, start, LAKES_ACROSS_M, CELL_M);
        // As the world keeps them: on patterned ground a lake is deeper or wider than a
        // polygon's pool, and the pools are a finer grid's.
        let over_sea = land.surface(start).length() - land.sea_radius_m();
        let patterned = (land.ground.patterned()).map_or(0.0, |p| p.strength(start, over_sea));
        let near = Waters::round(land, start, NEAR_ACROSS_M, round::LAKE_CELL_M)
            .keeping(|lake| patterned <= 0.0 || round::is_lake(lake));
        Self {
            land,
            wide,
            near,
            pooled: patterned > round::POOLED,
        }
    }

    /// Makes water of the survey's cells a lake covers that its own fill missed, so no walk
    /// crosses them.
    pub fn drown(&self, grid: &mut Grid) {
        let n = grid.region.cells;
        for k in 0..n * n {
            if self.lake(grid.region.direction(k % n, k / n), 0.0) {
                grid.drown(k);
            }
        }
    }

    /// Whether a lake, a pond or the sea lies within so many metres of a direction. Ground
    /// past what is known counts as wet.
    fn lake(&self, toward: Vec3, within_m: f32) -> bool {
        self.wide.wet_within(toward, within_m) != Some(false)
            || self.near.wet_within(toward, within_m) != Some(false)
    }

    /// Whether water stands within so many metres of a direction from the centre: a lake, the
    /// sea, a pond or a pool. Ground past what is known counts as wet.
    pub fn wet(&self, toward: Vec3, within_m: f32) -> bool {
        self.lake(toward, within_m)
            || self.pooled
                && Waters::round(self.land, toward, POOL_ACROSS_M, round::POOL_CELL_M)
                    .keeping(round::is_pool)
                    .wet_within(toward, within_m)
                    != Some(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ground, start_climate};
    use terra_core::{PlanetSpec, Seed};

    /// Seed 4's world round the planet's own start, on a lake's shore: the survey's grid
    /// alone draws 33 of the lake's 139 cells as dry ground.
    #[test]
    fn a_lake_the_surveys_grid_let_spill_is_wet_and_no_walk_crosses_it() {
        let land = Land::new(
            ground::world(PlanetSpec {
                seed: Seed(4),
                radius_m: 30_000.0,
            }),
            start_climate().unwrap(),
        );
        let start = crate::PLANET_START;
        let dry = Dry::round(&land, start);
        // Known another way: the lakes on the world's ten-metre cells, filled over another
        // width of land than either fill here, wide enough to hold this lake whole.
        let world = Waters::round(&land, start, 4_000.0, round::LAKE_CELL_M);
        let mut grid = Grid::round(&land, start, 900.0);
        let n = grid.region.cells;
        let lake = |k: usize| {
            let toward = grid.region.direction(k % n, k / n);
            (world.lake_level(toward).is_some()).then_some(toward)
        };
        // Cells the survey calls dry ground a walker crosses that the world draws a lake on.
        let missed: Vec<(usize, Vec3)> = (0..n * n)
            .filter(|&k| grid.walkable(k))
            .filter_map(|k| lake(k).map(|toward| (k, toward)))
            .collect();
        assert!(
            missed.len() >= 4,
            "the survey misses {} cells of lake",
            missed.len()
        );
        dry.drown(&mut grid);
        for &(k, toward) in &missed {
            assert!(dry.wet(toward, 0.0), "cell {k} is under a lake");
            assert!(!grid.walkable(k), "cell {k} is walked across");
        }
        // And ground the world draws no water within 60 m of stays dry, most of the land.
        let far: Vec<Vec3> = (0..n * n)
            .map(|k| grid.region.direction(k % n, k / n))
            .filter(|&toward| world.wet_within(toward, 60.0) == Some(false))
            .collect();
        assert!(far.len() > n * n / 2, "{} cells far from water", far.len());
        for toward in far {
            assert!(!dry.wet(toward, 0.0), "dry ground reads as wet");
        }
    }
}
