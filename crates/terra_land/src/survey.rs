//! What a seed's world holds within a walk of its start (v0 seam 2, D596): whether there is
//! level ground to stand a base on by the start, and which biomes a walker reaches from it on
//! foot. The sweep (`cargo xtask worlds`) runs it over many seeds and finds each world valid
//! or says what it lacks, so the generator is seen to hold up on any seed, not only on the
//! demos' pinned places.
//!
//! The biomes are read off the land's own rules on a coarse grid, as the tundra query reads
//! them (`tundra`): the growing season by latitude and height (`Seasons`), the trees it and the
//! slope allow (`woods::stand`), the lakes the ground holds (`lakes::fill`) and the sea.

use crate::region::Region;
use crate::woods::{Footing, Seasons, stand};
use crate::{Land, PLANET_START, lakes};
use glam::Vec2;

/// How far round the start a walker is followed, in metres: a long walk, an hour or two.
pub const REACH_M: f32 = 8_000.0;
/// The grid's cell, in metres.
pub const CELL_M: f32 = 50.0;
/// Steeper than this a walker does not climb, in degrees: the controller's walkable grade
/// (terra_motion's 45 degree limit), less a margin for a cell's mean hiding its steepest part.
pub const WALKABLE_DEG: f32 = 40.0;
/// The level ground a base stands on, as the game's start asks of it (`terra_world::start`):
/// a plot so many metres long and wide (`Plot::START` and 3 m round it) within so far of the
/// start, rising no more than so many metres under it, its lowest so high over the sea.
pub const PLOT_M: (f32, f32) = (20.0, 10.0);
pub const PLOT_WITHIN_M: f32 = 130.0;
pub const PLOT_RISE_M: f32 = 2.5;
pub const PLOT_OVER_SEA_M: f32 = 1.6;
/// So much of each biome reached makes it there, in square metres: a stand, not a cell.
pub const FOREST_M2: f32 = 1_000_000.0;
pub const TUNDRA_M2: f32 = 100_000.0;

/// What a place of the grid is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Biome {
    /// A closed forest: trees standing more than six tenths of a closed forest's density.
    Forest,
    /// Open woods: trees, but thinning, by the tree line or on a steep slope.
    Woods,
    /// Open ground past the trees' season, at height or by latitude: tundra or alpine.
    Tundra,
    /// Bare, steep or loose ground with no trees, in a season that would grow them.
    Bare,
    /// A lake or the sea.
    Water,
}

impl Biome {
    pub const ALL: [Biome; 5] = [
        Biome::Forest,
        Biome::Woods,
        Biome::Tundra,
        Biome::Bare,
        Biome::Water,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Biome::Forest => "forest",
            Biome::Woods => "woods",
            Biome::Tundra => "tundra",
            Biome::Bare => "bare",
            Biome::Water => "water",
        }
    }
}

/// What a walk from a world's start finds.
#[derive(Clone, Debug, PartialEq)]
pub struct Survey {
    /// The least rise under a base's square near the start, in metres, if any is dry.
    pub start_rise_m: Option<f32>,
    /// The share of the grid's dry land a walker reaches from the start.
    pub reached_share: f32,
    /// Square metres of each biome, in [`Biome::ALL`]'s order, reached on foot (water: its
    /// cells beside ground reached, a shore walked to).
    pub reached_m2: [f32; 5],
    /// Square metres of each biome on the whole grid.
    pub present_m2: [f32; 5],
}

impl Survey {
    /// Square metres of a biome reached on foot.
    pub fn reached(&self, biome: Biome) -> f32 {
        self.reached_m2[biome as usize]
    }

    /// What the world lacks to be valid, in words; empty when it is valid.
    pub fn faults(&self) -> Vec<String> {
        let mut faults = Vec::new();
        match self.start_rise_m {
            None => faults.push(format!(
                "no dry plot {} by {} m within {PLOT_WITHIN_M} m of the start",
                PLOT_M.0, PLOT_M.1
            )),
            Some(rise) if rise > PLOT_RISE_M => faults.push(format!(
                "the start's most level plot rises {rise:.1} m, over {PLOT_RISE_M} m"
            )),
            Some(_) => {}
        }
        for (biome, least) in [(Biome::Forest, FOREST_M2), (Biome::Tundra, TUNDRA_M2)] {
            let reached = self.reached(biome);
            if reached < least {
                faults.push(format!(
                    "{:.2} km² of {} reached on foot ({:.2} km² within {} km), under {:.2}",
                    reached / 1e6,
                    biome.name(),
                    self.present_m2[biome as usize] / 1e6,
                    REACH_M / 1000.0,
                    least / 1e6,
                ));
            }
        }
        faults
    }
}

/// The survey of a land round its start.
pub fn of(land: &Land) -> Survey {
    let start = PLANET_START;
    let region = Region::sample(land, start, REACH_M * 2.0, CELL_M);
    let n = region.cells;
    let sea_m = land.sea_radius_m() - land.radius_m();
    let found = lakes::fill(&region, land.ground.sea_radius_m().map(|_| sea_m));
    let cells: Vec<usize> = (0..n * n).collect();
    let latitudes = crate::maps::par_map(&cells, |&k| {
        land.frame.latitude(region.direction(k % n, k / n))
    });
    let (low, high) = latitudes
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), &x| (l.min(x), h.max(x)));
    let seasons = Seasons::over_by(
        &land.fields,
        (low, high),
        (-50.0, land.ground.reach_m() + 50.0),
        0.25,
        50.0,
        2,
    );
    let biomes: Vec<Biome> = crate::maps::par_map(&cells, |&k| {
        let over_sea = region.heights[k] - sea_m;
        if over_sea < 0.0 || found.lake[k].is_some() {
            return Biome::Water;
        }
        let season = seasons.at(latitudes[k], over_sea, 0.0);
        let slope = region.slope_deg(k % n, k / n);
        let trees = stand(&season, slope, Footing::Soil).density;
        if trees > 0.6 {
            Biome::Forest
        } else if trees > 0.05 {
            Biome::Woods
        } else if season.growing_mean_c < terra_climate::TREE_LINE_SEASON_C {
            Biome::Tundra
        } else {
            Biome::Bare
        }
    });
    // A walk from the cells round the start, where a base may stand (the start's own may be
    // a lake's), over dry cells no steeper than a walker climbs.
    let walkable =
        |k: usize| biomes[k] != Biome::Water && region.slope_deg(k % n, k / n) <= WALKABLE_DEG;
    let mut reached = vec![false; n * n];
    let near = (PLOT_WITHIN_M / CELL_M).ceil() as i64;
    let middle = (n / 2) as i64;
    let mut queue = Vec::new();
    for r in middle - near..=middle + near {
        for c in middle - near..=middle + near {
            let k = r as usize * n + c as usize;
            if walkable(k) {
                reached[k] = true;
                queue.push(k);
            }
        }
    }
    while let Some(k) = queue.pop() {
        let (c, r) = (k % n, k / n);
        let near = [
            (c > 0).then(|| k - 1),
            (c + 1 < n).then_some(k + 1),
            (r > 0).then(|| k - n),
            (r + 1 < n).then_some(k + n),
        ];
        for next in near.into_iter().flatten() {
            if !reached[next] && walkable(next) {
                reached[next] = true;
                queue.push(next);
            }
        }
    }
    let cell_m2 = CELL_M * CELL_M;
    let (mut reached_m2, mut present_m2) = ([0.0; 5], [0.0; 5]);
    let (mut land_cells, mut land_reached) = (0usize, 0usize);
    for k in 0..n * n {
        let biome = biomes[k] as usize;
        present_m2[biome] += cell_m2;
        if biomes[k] == Biome::Water {
            let (c, r) = (k % n, k / n);
            let shore = [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)]
                .iter()
                .any(|(dc, dr)| {
                    let (cc, rr) = (c as i64 + dc, r as i64 + dr);
                    (0..n as i64).contains(&cc)
                        && (0..n as i64).contains(&rr)
                        && reached[rr as usize * n + cc as usize]
                });
            if shore {
                reached_m2[biome] += cell_m2;
            }
            continue;
        }
        land_cells += 1;
        if reached[k] {
            land_reached += 1;
            reached_m2[biome] += cell_m2;
        }
    }
    Survey {
        start_rise_m: start_rise_m(land),
        reached_share: land_reached as f32 / land_cells.max(1) as f32,
        reached_m2,
        present_m2,
    }
}

/// The least rise under a dry base's plot near the start, as `terra_world::start` looks for
/// one: on rings 18 to 126 m round the start, 24 spokes a ring, the plot turned four ways,
/// its heights every 2.5 m.
fn start_rise_m(land: &Land) -> Option<f32> {
    let sea_m = land.sea_radius_m() - land.radius_m();
    let height = |at: Vec2| land.ground.height_m(land.along(PLANET_START, at)) - sea_m;
    let mut best: Option<f32> = None;
    for ring in 3..=(PLOT_WITHIN_M / 6.0) as usize {
        for spoke in 0..24 {
            let angle = spoke as f32 / 24.0 * std::f32::consts::TAU;
            let middle = Vec2::new(libm::cosf(angle), libm::sinf(angle)) * (ring as f32 * 6.0);
            for turn in 0..4 {
                let turn = turn as f32 * std::f32::consts::FRAC_PI_4;
                let (along, across) = (
                    Vec2::new(libm::cosf(turn), libm::sinf(turn)),
                    Vec2::new(-libm::sinf(turn), libm::cosf(turn)),
                );
                let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
                for i in 0..=8 {
                    for j in 0..=4 {
                        let off = along * (i as f32 / 8.0 - 0.5) * PLOT_M.0
                            + across * (j as f32 / 4.0 - 0.5) * PLOT_M.1;
                        let h = height(middle + off);
                        lowest = lowest.min(h);
                        highest = highest.max(h);
                    }
                }
                if lowest < PLOT_OVER_SEA_M {
                    continue;
                }
                let rise = highest - lowest;
                if best.is_none_or(|b| rise < b) {
                    best = Some(rise);
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ground, start_climate};
    use terra_core::{PlanetSpec, Seed};

    #[test]
    fn the_first_world_walks_from_its_lake_into_its_forest_and_up_to_tundra() {
        // The start of seed 1's 30 km world is by a lake in a wooded valley under mountains
        // (the demos', D393): the walk starts round the lake, not in it, and reaches both.
        let land = Land::new(
            ground::world(PlanetSpec {
                seed: Seed(1),
                radius_m: 30_000.0,
            }),
            start_climate().unwrap(),
        );
        let survey = of(&land);
        assert!(survey.reached(Biome::Forest) >= FOREST_M2, "{survey:?}");
        assert!(survey.reached(Biome::Tundra) >= TUNDRA_M2, "{survey:?}");
        assert!(survey.reached(Biome::Water) > 0.0, "{survey:?}");
        // Reached is part of what is there, and not all of it: some is too steep to climb.
        for biome in Biome::ALL {
            assert!(survey.reached(biome) <= survey.present_m2[biome as usize]);
        }
        assert!(
            survey.reached_share > 0.0 && survey.reached_share < 1.0,
            "{survey:?}"
        );
    }

    #[test]
    fn a_world_of_sea_alone_says_so() {
        let survey = Survey {
            start_rise_m: None,
            reached_share: 0.0,
            reached_m2: [0.0; 5],
            present_m2: [0.0, 0.0, 0.0, 0.0, 1e8],
        };
        let faults = survey.faults();
        assert_eq!(faults.len(), 3, "{faults:?}");
        assert!(faults[0].contains("no dry"), "{faults:?}");
    }
}
