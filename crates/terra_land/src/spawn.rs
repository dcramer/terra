//! Where a world's player starts (v0 seam 2, D596): searched for from the seed, as shipped
//! survival games pick a spawn rather than reshape their land for it. Minecraft walks out from
//! the world's origin until it finds ground of a biome it allows a spawn in; Valheim stands
//! its start in the Meadows near the middle. Ours walks out from the planet's own start
//! (`PLANET_START`), nearest first, to the first place where a base's plot lies level, the
//! ground round it is not walled in by steep slopes, and a walk reaches a forest, water and the
//! heights past the trees (`survey`'s rules, which the sweep then checks the start by).
//!
//! Every seed's start is searched for, the first world's too (the developers: "the seed
//! shoudl dictate how worlds look"); it kept the planet's own start while its lake there
//! stood, a cirque the foothills no longer hold (`terra_voxel` `HORNS_STAND_M`). A world too
//! small for latitudes (`frame::has_latitudes`) has no biomes to walk to and keeps the
//! planet's own.

use crate::survey::{self, CELL_M, Grid, PLOT_OVER_SEA_M, PLOT_RISE_M, REACH_M};
use crate::{Land, PLANET_START, frame};
use glam::Vec3;

/// How far from the planet's own start the search goes, in metres: within the first, and
/// failing that within the second, on a grid of its own (seed 102's nearest start that walks
/// to all three biomes is past 6 km).
pub const SEARCH_M: [f32; 2] = [6_000.0, 12_000.0];
/// How far apart the places tried are, in metres.
pub const STEP_M: f32 = 250.0;
/// Steeper than this, in degrees over a cell, a place is not tried as a start.
pub const LEVEL_DEG: f32 = 6.0;
/// The walk from a place tried, read off one grid round the planet's start, must reach so
/// many times each least area the survey asks for before it is walked again on the place's
/// own grid, as the survey reads it: the one grid's cells, flattened onto a plane up to
/// 20 km each way, are a fifth to a third smaller on the ground at its edges, its slopes
/// gentler.
pub const MARGIN: f32 = 1.5;
/// So many places are tried in full (a plot, the walk on their own grid), nearest first,
/// before the search gives up.
pub const PLOTS_TRIED: usize = 48;

/// The start of a world: a direction from the planet's centre.
pub fn find(land: &Land) -> Vec3 {
    find_tried(land).0
}

/// [`find`], and how far its search got, if it searched.
pub fn find_tried(land: &Land) -> (Vec3, Option<Tried>) {
    if !frame::has_latitudes(land.radius_m()) {
        return (PLANET_START, None);
    }
    let mut tried = Tried::default();
    for search_m in SEARCH_M {
        let (found, so_far) = searched(land, search_m);
        tried = so_far;
        if let Some(found) = found {
            return (found, Some(tried));
        }
    }
    (PLANET_START, Some(tried))
}

/// How far the places tried got, so a search that finds nothing says why.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tried {
    /// Places dry, gentle and walkable within the search's reach.
    pub places: usize,
    /// Of them, tried in turn, those whose walk on the one grid reached enough.
    pub walked: usize,
    /// Of those, the ones with a level plot.
    pub level: usize,
}

/// The nearest place to the planet's start that makes a start, if any within so many metres,
/// and how far the places tried got.
pub fn searched(land: &Land, search_m: f32) -> (Option<Vec3>, Tried) {
    let grid = Grid::round(land, PLANET_START, REACH_M + search_m);
    let region = &grid.region;
    let n = region.cells;
    let sea_m = land.sea_radius_m() - land.radius_m();
    let step = (STEP_M / CELL_M).round() as usize;
    let middle = n / 2;
    let within = (search_m / CELL_M) as i64;
    let mut places: Vec<(i64, usize, usize)> = Vec::new();
    for r in (middle % step..n).step_by(step) {
        for c in (middle % step..n).step_by(step) {
            let (dc, dr) = (c as i64 - middle as i64, r as i64 - middle as i64);
            let far = dc * dc + dr * dr;
            let k = r * n + c;
            if far <= within * within
                && grid.walkable(k)
                && region.heights[k] - sea_m >= PLOT_OVER_SEA_M
                && region.slope_deg(c, r) <= LEVEL_DEG
            {
                places.push((far, r, c));
            }
        }
    }
    places.sort_unstable();
    let half = (REACH_M / CELL_M) as usize;
    let mut tried = Tried {
        places: places.len(),
        ..Tried::default()
    };
    // The walks a batch at a time on every core, then their places in order, nearest first.
    let walked = places.chunks(64).flat_map(|batch| {
        let far_enough = crate::maps::par_map(batch, |&(_, r, c)| {
            grid.walk((c, r), half).faults(MARGIN).is_empty()
        });
        batch
            .iter()
            .zip(far_enough)
            .filter(|(_, ok)| *ok)
            .map(|(&(_, r, c), _)| (r, c))
            .collect::<Vec<_>>()
    });
    for (r, c) in walked {
        tried.walked += 1;
        let toward = region.direction(c, r);
        // A level plot, then the walk again on the start's own grid, as the survey takes it.
        let rise = survey::least_rise_m(land, toward, PLOT_RISE_M);
        if rise.is_some_and(|rise| rise <= PLOT_RISE_M) {
            tried.level += 1;
            let own = Grid::round(land, toward, REACH_M);
            let middle = own.region.cells / 2;
            if own.walk((middle, middle), middle).faults(1.0).is_empty() {
                return (Some(toward), tried);
            }
        }
        if tried.walked >= PLOTS_TRIED {
            break;
        }
    }
    (None, tried)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ground, site, start_climate};
    use terra_core::{PlanetSpec, Seed};

    fn land(seed: u64) -> Land {
        Land::new(
            ground::world(PlanetSpec {
                seed: Seed(seed),
                radius_m: 30_000.0,
            }),
            start_climate().unwrap(),
        )
    }

    #[test]
    fn every_seed_finds_a_start_its_survey_passes_the_first_world_too() {
        // The first world's start is searched for as any other's is: the seed says where.
        let first = land(site::PINNED_SEED);
        let start = find(&first);
        let survey = survey::of(&first, start);
        assert!(survey.faults().is_empty(), "{survey:?}");
        let off = (start - PLANET_START).length() * first.radius_m();
        assert!(
            off < SEARCH_M[0],
            "seed 1 starts {off:.0} m from the planet's start"
        );
        eprintln!("seed 1 starts {off:.0} m from the planet's start");
        // Seed 2's own start has no heights past the trees in reach (the sweep of j2): the
        // search moves it to where the survey finds everything a start asks for.
        let land = land(2);
        assert!(!survey::of(&land, PLANET_START).faults().is_empty());
        let start = find(&land);
        assert_ne!(start, PLANET_START);
        let survey = survey::of(&land, start);
        assert!(survey.faults().is_empty(), "{survey:?}");
    }
}
