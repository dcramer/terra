//! What a seed's world holds within a walk of its start (v0 seam 2, D596): whether there is
//! level ground to stand a base on by the start, and which biomes a walker reaches from it on
//! foot. The sweep (`cargo xtask worlds`) runs it over many seeds and finds each world valid
//! or says what it lacks, so the generator is seen to hold up on any seed, not only on the
//! demos' pinned places; the start itself is chosen by the same rules (`spawn`).
//!
//! The biomes are read off the land's own rules on a coarse grid, as the tundra query reads
//! them (`tundra`): the growing season by latitude and height (`Seasons`), the trees it and the
//! slope allow (`woods::stand`), the lakes the ground holds (`lakes::fill`) and the sea.

use crate::region::Region;
use crate::woods::{Footing, Seasons, stand};
use crate::{Land, lakes};
use glam::{Vec2, Vec3};
use terra_core::Plot;

/// How far round the start a walker is followed, in metres east, west, north and south: a
/// day's walk out and back (16 km at 5 km/h is three hours), the reach a survival game puts
/// its first biomes in, and on the 30 km world (D13) a twelfth of the way round it.
pub const REACH_M: f32 = 8_000.0;
/// The grid's cell, in metres.
pub const CELL_M: f32 = 50.0;
/// Steeper than this a walker does not climb, in degrees: the controller's walkable grade
/// (terra_motion's 45 degree limit), less a margin for a cell's mean hiding its steepest part.
pub const WALKABLE_DEG: f32 = 40.0;
/// The level ground a base stands on, as the game's start asks of it (`terra_world::start`'s
/// `choose_site`): `Plot::START` and 3 m round it, its middle on rings 18 to 126 m round the
/// start, rising no more than so many metres under it, its lowest so high over the sea.
pub const PLOT_WITHIN_M: f32 = 126.0;
/// Failing a level plot that near, the nearest out to so far (seed 1's start lies in a
/// valley whose nearest level plot is past 126 m, and the demos stand on that start, D393).
pub const PLOT_FAR_M: f32 = 600.0;
/// The rings, 6 m apart, a plot's middle is tried on: the near ones, then the far.
pub const PLOT_RINGS: (std::ops::Range<usize>, std::ops::Range<usize>) = (3..22, 22..101);
pub const PLOT_RISE_M: f32 = 2.5;
pub const PLOT_OVER_SEA_M: f32 = 1.6;
/// So much of each biome reached makes it there, in square metres: a stand, not a cell; the
/// water a shore of some twenty cells, a lake's or the sea's.
pub const FOREST_M2: f32 = 1_000_000.0;
pub const TUNDRA_M2: f32 = 100_000.0;
pub const BARE_M2: f32 = 100_000.0;
pub const WATER_M2: f32 = 50_000.0;
/// So much dry land reached is a start not walled in by steep ground: a tenth of the walk's
/// square.
pub const OPEN_M2: f32 = 25_000_000.0;

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
    /// The least rise under a base's plot near the start, in metres, if any is dry.
    pub start_rise_m: Option<f32>,
    /// What a walker reaches from the start.
    pub walk: Walk,
}

/// What a walk reaches, in square metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walk {
    /// The dry land reached, and all the dry land of the walk's square.
    pub land_m2: (f32, f32),
    /// Of each biome, in [`Biome::ALL`]'s order, reached on foot (water: its cells beside
    /// ground reached, a shore walked to).
    pub reached_m2: [f32; 5],
    /// Of each biome on the walk's whole square.
    pub present_m2: [f32; 5],
}

impl Walk {
    /// Square metres of a biome reached on foot.
    pub fn reached(&self, biome: Biome) -> f32 {
        self.reached_m2[biome as usize]
    }

    /// The share of the square's dry land a walker reaches.
    pub fn share(&self) -> f32 {
        self.land_m2.0 / self.land_m2.1.max(1.0)
    }

    /// What the walk lacks of a start's, in words, with each least area so many times over:
    /// open ground, a forest, water and the heights (tundra or bare rock) past the trees,
    /// three biomes a player walks to (docs/v0-goals.md, Land).
    pub fn faults(&self, over: f32) -> Vec<String> {
        let mut faults = Vec::new();
        let km2 = |m2: f32| m2 / 1e6;
        if self.land_m2.0 < OPEN_M2 * over {
            faults.push(format!(
                "walled in: {:.1} km² of land reached on foot, under {:.0}",
                km2(self.land_m2.0),
                km2(OPEN_M2 * over)
            ));
        }
        let lacks = |biome: Biome, least: f32| self.reached(biome) < least * over;
        let said = |biome: Biome, least: f32| {
            format!(
                "{:.2} km² of {} reached on foot ({:.2} km² within {} km), under {:.2}",
                km2(self.reached(biome)),
                biome.name(),
                km2(self.present_m2[biome as usize]),
                REACH_M / 1000.0,
                km2(least * over),
            )
        };
        for (biome, least) in [(Biome::Forest, FOREST_M2), (Biome::Water, WATER_M2)] {
            if lacks(biome, least) {
                faults.push(said(biome, least));
            }
        }
        if lacks(Biome::Tundra, TUNDRA_M2) && lacks(Biome::Bare, BARE_M2) {
            faults.push(format!(
                "no heights past the trees: {} and {}",
                said(Biome::Tundra, TUNDRA_M2),
                said(Biome::Bare, BARE_M2)
            ));
        }
        faults
    }
}

impl Survey {
    /// Square metres of a biome reached on foot.
    pub fn reached(&self, biome: Biome) -> f32 {
        self.walk.reached(biome)
    }

    /// What the world lacks to be valid, in words; empty when it is valid.
    pub fn faults(&self) -> Vec<String> {
        let mut faults = Vec::new();
        match self.start_rise_m {
            None => faults.push(format!(
                "no dry plot for the start's base within {PLOT_FAR_M} m of the start"
            )),
            Some(rise) if rise > PLOT_RISE_M => faults.push(format!(
                "the start's most level plot rises {rise:.1} m, over {PLOT_RISE_M} m"
            )),
            Some(_) => {}
        }
        faults.extend(self.walk.faults(1.0));
        faults
    }
}

/// The land on a grid round a point: each cell's biome, and whether a walker crosses it.
pub struct Grid {
    pub region: Region,
    pub biomes: Vec<Biome>,
    walkable: Vec<bool>,
}

impl Grid {
    /// The grid so many metres east, west, north and south of a direction from the centre.
    pub fn round(land: &Land, toward: Vec3, half_m: f32) -> Self {
        let region = Region::sample(land, toward, half_m * 2.0, CELL_M);
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
        let walkable = (0..n * n)
            .map(|k| biomes[k] != Biome::Water && region.slope_deg(k % n, k / n) <= WALKABLE_DEG)
            .collect();
        Self {
            region,
            biomes,
            walkable,
        }
    }

    /// Makes water of a cell: a lake's that this grid's own fill let spill off its edge
    /// (`dry::Dry::drown`).
    pub fn drown(&mut self, k: usize) {
        self.biomes[k] = Biome::Water;
        self.walkable[k] = false;
    }

    /// Whether a walker crosses a cell: dry and no steeper than one climbs.
    pub fn walkable(&self, k: usize) -> bool {
        self.walkable[k]
    }

    /// The cells a walk reaches from the cells round one, where a base may stand (that cell
    /// may be a lake's), over dry cells no steeper than a walker climbs, kept within `half`
    /// cells of it.
    pub fn reached(&self, (column, row): (usize, usize), half: usize) -> Vec<bool> {
        let n = self.region.cells;
        let (low_c, high_c) = (column.saturating_sub(half), (column + half).min(n - 1));
        let (low_r, high_r) = (row.saturating_sub(half), (row + half).min(n - 1));
        let mut reached = vec![false; n * n];
        let near = (PLOT_WITHIN_M / CELL_M).ceil() as usize;
        let mut queue = Vec::new();
        for r in row.saturating_sub(near)..=(row + near).min(n - 1) {
            for c in column.saturating_sub(near)..=(column + near).min(n - 1) {
                let k = r * n + c;
                if self.walkable[k] {
                    reached[k] = true;
                    queue.push(k);
                }
            }
        }
        while let Some(k) = queue.pop() {
            let (c, r) = (k % n, k / n);
            let next = [
                (c > low_c).then(|| k - 1),
                (c < high_c).then_some(k + 1),
                (r > low_r).then(|| k - n),
                (r < high_r).then_some(k + n),
            ];
            for next in next.into_iter().flatten() {
                if !reached[next] && self.walkable[next] {
                    reached[next] = true;
                    queue.push(next);
                }
            }
        }
        reached
    }

    /// What that walk reaches ([`Grid::reached`]), in square metres by biome.
    pub fn walk(&self, (column, row): (usize, usize), half: usize) -> Walk {
        let n = self.region.cells;
        let (low_c, high_c) = (column.saturating_sub(half), (column + half).min(n - 1));
        let (low_r, high_r) = (row.saturating_sub(half), (row + half).min(n - 1));
        let reached = self.reached((column, row), half);
        let cell_m2 = CELL_M * CELL_M;
        let mut walk = Walk {
            land_m2: (0.0, 0.0),
            reached_m2: [0.0; 5],
            present_m2: [0.0; 5],
        };
        for r in low_r..=high_r {
            for c in low_c..=high_c {
                let k = r * n + c;
                let biome = self.biomes[k] as usize;
                walk.present_m2[biome] += cell_m2;
                if self.biomes[k] == Biome::Water {
                    let shore = [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)]
                        .iter()
                        .any(|(dc, dr)| {
                            let (cc, rr) = (c as i64 + dc, r as i64 + dr);
                            (0..n as i64).contains(&cc)
                                && (0..n as i64).contains(&rr)
                                && reached[rr as usize * n + cc as usize]
                        });
                    if shore {
                        walk.reached_m2[biome] += cell_m2;
                    }
                    continue;
                }
                walk.land_m2.1 += cell_m2;
                if reached[k] {
                    walk.land_m2.0 += cell_m2;
                    walk.reached_m2[biome] += cell_m2;
                }
            }
        }
        walk
    }
}

/// The survey of a land round a start, a direction from the centre.
pub fn of(land: &Land, start: Vec3) -> Survey {
    let grid = Grid::round(land, start, REACH_M);
    let middle = grid.region.cells / 2;
    Survey {
        start_rise_m: plot_rise_m(land, start),
        walk: grid.walk((middle, middle), middle),
    }
}

/// The least rise under the plot a start's base stands on, as `terra_world::start` chooses
/// it: on the near rings (`least_rise_m`), or failing a level one there, on the first of the
/// far rings out to `PLOT_FAR_M` that has one (`PLOT_RINGS`).
pub fn plot_rise_m(land: &Land, start: Vec3) -> Option<f32> {
    let near = least_rise_m(land, start, 0.0);
    if near.is_some_and(|rise| rise <= PLOT_RISE_M) {
        return near;
    }
    let far = rise_on_rings(land, start, PLOT_RISE_M, PLOT_RINGS.1);
    match (near, far) {
        (Some(near), Some(far)) => Some(near.min(far)),
        (near, far) => near.or(far),
    }
}

/// The least rise under a dry base's plot near a start, as `terra_world::start` looks for one:
/// `Plot::START` and 3 m round it, its heights every 3 m, its middle on rings 18 to 126 m round
/// the start, 24 spokes a ring, the plot turned eight ways. It stops at the first that rises
/// no more than `enough`.
pub fn least_rise_m(land: &Land, start: Vec3, enough: f32) -> Option<f32> {
    rise_on_rings(land, start, enough, PLOT_RINGS.0)
}

/// [`least_rise_m`] on these rings.
fn rise_on_rings(
    land: &Land,
    start: Vec3,
    enough: f32,
    rings: std::ops::Range<usize>,
) -> Option<f32> {
    let plot = Plot::START;
    let squares = || plot.squares().map(|[e, s]| Vec2::new(e as f32, s as f32));
    let low = squares().fold(Vec2::MAX, Vec2::min) * plot.square_m - Vec2::splat(3.0);
    let high =
        (squares().fold(Vec2::MIN, Vec2::max) + Vec2::ONE) * plot.square_m + Vec2::splat(3.0);
    let middle = (low + high) * 0.5;
    let mut points = Vec::new();
    let mut x = low.x;
    while x <= high.x {
        let mut z = low.y;
        while z <= high.y {
            // East and south of the plot's middle, as north is minus south.
            points.push(Vec2::new(x - middle.x, middle.y - z));
            z += 3.0;
        }
        x += 3.0;
    }
    let sea_m = land.sea_radius_m() - land.radius_m();
    let height = |at: Vec2| land.ground.height_m(land.along(start, at)) - sea_m;
    // A ring's placements on every core, each given up once it rises past the best of the
    // rings before, so what each finds does not hang on the order they ran in.
    let placements: Vec<(usize, usize)> = (0..24)
        .flat_map(|spoke| (0..8).map(move |turn| (spoke, turn)))
        .collect();
    let mut best: Option<f32> = None;
    for ring in rings {
        let worst = best.unwrap_or(f32::MAX);
        let rises = crate::maps::par_map(&placements, |&(spoke, turn)| {
            let angle = spoke as f32 / 24.0 * std::f32::consts::TAU;
            let centre = Vec2::new(libm::cosf(angle), libm::sinf(angle)) * (ring as f32 * 6.0);
            let turn = turn as f32 * std::f32::consts::TAU / 8.0;
            let (cos, sin) = (libm::cosf(turn), libm::sinf(turn));
            let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
            for p in &points {
                let h = height(centre + Vec2::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos));
                lowest = lowest.min(h);
                highest = highest.max(h);
                if highest - lowest >= worst || lowest < PLOT_OVER_SEA_M {
                    return None;
                }
            }
            Some(highest - lowest)
        });
        if let Some(least) = rises.into_iter().flatten().reduce(f32::min) {
            best = Some(least);
            if least <= enough {
                return best;
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
    fn the_first_world_walks_from_its_start_into_its_forest_and_up_to_tundra() {
        // The start of seed 1's 30 km world, found as any seed's is (`spawn`), is in wooded
        // foothills under mountains with water in reach: the walk reaches all three.
        let land = Land::new(
            ground::world(PlanetSpec {
                seed: Seed(1),
                radius_m: 30_000.0,
            }),
            start_climate().unwrap(),
        );
        let survey = of(&land, crate::spawn::find(&land));
        assert!(survey.reached(Biome::Forest) >= FOREST_M2, "{survey:?}");
        assert!(survey.reached(Biome::Tundra) >= TUNDRA_M2, "{survey:?}");
        assert!(survey.reached(Biome::Water) > 0.0, "{survey:?}");
        // Reached is part of what is there, and not all of it: some is too steep to climb.
        let walk = survey.walk;
        for biome in Biome::ALL {
            assert!(walk.reached(biome) <= walk.present_m2[biome as usize]);
        }
        assert!(walk.share() > 0.0 && walk.share() < 1.0, "{survey:?}");
        assert!(survey.faults().is_empty(), "{:?}", survey.faults());
    }

    #[test]
    fn a_world_of_sea_alone_says_so() {
        let survey = Survey {
            start_rise_m: None,
            walk: Walk {
                land_m2: (0.0, 0.0),
                reached_m2: [0.0; 5],
                present_m2: [0.0, 0.0, 0.0, 0.0, 1e8],
            },
        };
        let faults = survey.faults();
        assert_eq!(faults.len(), 5, "{faults:?}");
        assert!(faults[0].contains("no dry"), "{faults:?}");
        assert!(faults[1].contains("walled in"), "{faults:?}");
    }
}
