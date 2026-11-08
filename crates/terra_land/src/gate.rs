//! Where a realm's gate stands (v0's "the gate placed from the seed, reachable on every
//! seed"; D470): a place found from the seed on ground a walker reaches from the start, as
//! the start itself is searched for (`spawn`) and a town's site is (`town`), never painted.
//! Shipped worlds place their way onward the same way: Minecraft's strongholds stand in
//! rings round the origin by the seed, Valheim's boss altars at seeded places of their biome.
//!
//! Of the land round the start, on the survey's grid (`survey::Grid`, 50 m cells), a cell may
//! hold the gate if a walk from the start reaches it, it is dry and well over the sea, level
//! enough to stand a ring on, and neither too near the start nor too far ([`NEAR_M`],
//! [`FAR_M`], then [`FARTHEST_M`]: how far a gate stands from where a player begins is the developers' to tune;
//! these are first values). Of those the seed picks one, so worlds differ in which way their
//! gate lies. Where in its cell the ring stands, clear of trees and rocks, is the game's to
//! refine. [`faults`] holds a place to the same rules read afresh, for the sweep.

use crate::survey::{Biome, CELL_M, Grid, PLOT_OVER_SEA_M};
use crate::{Land, region::Region};
use glam::Vec3;

/// The nearest a gate stands from the start, in metres over the ground; how far it is looked
/// for first; and, failing that, the farthest (seed 104's start stands among slopes, its
/// nearest level ground a walk reaches past 400 m), as the start's own search widens
/// (`spawn::SEARCH_M`).
pub const NEAR_M: f32 = 150.0;
pub const FAR_M: f32 = 400.0;
pub const FARTHEST_M: f32 = 800.0;
/// Steeper than this over its cell, in degrees, ground holds no gate.
pub const LEVEL_DEG: f32 = 8.0;

/// The grid round a start that a gate is looked for on, and the start's cell in it.
fn grid(land: &Land, start: Vec3) -> (Grid, (usize, usize)) {
    let grid = Grid::round(land, start, FARTHEST_M + 2.0 * CELL_M);
    let middle = grid.region.cells / 2;
    let cell = grid.region.cell_of(start).unwrap_or((middle, middle));
    (grid, cell)
}

/// How far a cell is from the grid's middle, the start, in metres.
fn from_start_m(region: &Region, column: usize, row: usize) -> f32 {
    region.offset(column, row).length()
}

/// What is wrong with a gate at a cell, if anything: the first rule it breaks.
fn wrong(
    land: &Land,
    grid: &Grid,
    reached: &[bool],
    (column, row): (usize, usize),
) -> Option<String> {
    let region = &grid.region;
    let k = row * region.cells + column;
    let away = from_start_m(region, column, row);
    let over_sea = region.heights[k] - (land.sea_radius_m() - land.radius_m());
    let slope = region.slope_deg(column, row);
    if !(NEAR_M..=FARTHEST_M).contains(&away) {
        Some(format!(
            "it stands {away:.0} m from the start, not {NEAR_M:.0} to {FARTHEST_M:.0} m"
        ))
    } else if grid.biomes[k] == Biome::Water || over_sea < PLOT_OVER_SEA_M {
        Some(format!(
            "it stands in water or {over_sea:.1} m over the sea, under {PLOT_OVER_SEA_M} m"
        ))
    } else if slope > LEVEL_DEG {
        Some(format!(
            "its ground slopes {slope:.0} degrees, over {LEVEL_DEG:.0}"
        ))
    } else if !reached[k] {
        Some("no walk from the start reaches it".to_string())
    } else {
        None
    }
}

/// The direction from the planet's centre of the place a world's gate stands, for a start;
/// none if no ground round the start holds one, which the sweep counts a fault of the world.
pub fn find(land: &Land, start: Vec3) -> Option<Vec3> {
    let (grid, cell) = grid(land, start);
    let n = grid.region.cells;
    let reached = grid.reached(cell, n);
    let seed = land.ground.spec().seed.derive("gate").0 as u32;
    let may: Vec<(usize, usize)> = (0..n * n)
        .map(|k| (k % n, k / n))
        .filter(|&at| wrong(land, &grid, &reached, at).is_none())
        .collect();
    [FAR_M, FARTHEST_M].into_iter().find_map(|far| {
        may.iter()
            .filter(|&&(column, row)| from_start_m(&grid.region, column, row) <= far)
            // The seed's pick among the cells that may hold it: the least of their hashes.
            .min_by_key(|&&(column, row)| terra_gen::hash::mix3(seed, column as u32, row as u32, 0))
            .map(|&(column, row)| grid.region.direction(column, row))
    })
}

/// What is wrong with a gate standing in a direction from the centre, for a start: nothing
/// if a walker reaches it and it stands as a gate may. Read afresh from the land, not from
/// what [`find`] kept.
pub fn faults(land: &Land, start: Vec3, gate: Vec3) -> Vec<String> {
    let (grid, cell) = grid(land, start);
    let Some(at) = grid.region.cell_of(gate) else {
        return vec![format!(
            "the gate is over {FARTHEST_M:.0} m from the start, off the land searched"
        )];
    };
    let reached = grid.reached(cell, grid.region.cells);
    wrong(land, &grid, &reached, at)
        .map(|why| format!("the gate: {why}"))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ground, start_climate};
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
    fn a_gate_is_found_from_the_seed_where_a_walk_reaches_and_differs_by_world() {
        let seeds: Vec<u64> = (2..10).collect();
        let found = terra_gen::par::map(&seeds, |&seed| {
            let land = land(seed);
            // The planet's own start, not the seed's searched one (seconds a seed): the sweep
            // holds every seed's gate from its own start (`cargo xtask worlds`).
            let start = crate::PLANET_START;
            let gate = find(&land, start).unwrap_or_else(|| panic!("seed {seed} has no gate"));
            assert_eq!(
                faults(&land, start, gate),
                Vec::<String>::new(),
                "seed {seed}"
            );
            // The same again: a world's gate is its seed's.
            assert_eq!(find(&land, start), Some(gate), "seed {seed}");
            // Where it lies from the start, metres east and north.
            let region = Region::sample(&land, start, 2.5 * FARTHEST_M, CELL_M);
            let (column, row) = region.cell_of(gate).expect("in reach");
            let off = region.offset(column, row);
            (off.x as i32, off.y as i32)
        });
        // Worlds differ in where their gate stands: eight worlds, at least six places. (Not
        // by compass quarter: from the planet's own start these worlds' level, dry ground all
        // lies to the east, the range and its lake to the west.)
        let places: std::collections::HashSet<(i32, i32)> = found.iter().copied().collect();
        assert!(
            places.len() >= 6,
            "eight worlds' gates stand in {} places",
            places.len()
        );
    }

    #[test]
    fn a_gate_in_the_water_too_near_or_out_of_reach_is_a_fault() {
        let land = land(2);
        let start = crate::PLANET_START;
        // At the start itself: too near.
        let near = faults(&land, start, start);
        assert!(near[0].contains("from the start"), "{near:?}");
        // Out past the land searched.
        let region = Region::sample(&land, start, 4_000.0, CELL_M);
        let far = faults(&land, start, region.direction(0, 0));
        assert!(far[0].contains("off the land searched"), "{far:?}");
        // The nearest water or steep ground in the ring, if this world has any there, is
        // refused too; every cell the search refuses is one `faults` names.
        let (grid, cell) = grid(&land, start);
        let n = grid.region.cells;
        let reached = grid.reached(cell, n);
        let refused = (0..n * n)
            .map(|k| (k % n, k / n))
            .filter(|&at| wrong(&land, &grid, &reached, at).is_some())
            .count();
        assert!(
            refused > 0 && refused < n * n,
            "{refused} of {} cells refused",
            n * n
        );
    }
}
