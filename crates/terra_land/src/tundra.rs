//! Open tundra found by what the land's rules make of it (docs/demos.md §2, D321): past the
//! trees' niches, by latitude or by height, where the growing season is too cold for a tree,
//! over ground frozen at depth, the forest's last islands in sight. A query over the land round
//! the planet's start, as the mountain lake's is (`site`); nothing is painted, and if the rules
//! put no tundra on a world the query says why.

use crate::Land;
use crate::lakes;
use crate::region::Region;
use crate::woods::{Footing, Seasons, stand};
use glam::{Vec2, Vec3};

/// Open tundra found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TundraSite {
    /// The direction from the centre to it.
    pub toward: Vec3,
    /// Its height over the sea, in metres, and its growing season's mean and its year's
    /// mean, in degrees Celsius.
    pub height_m: f32,
    pub growing_mean_c: f32,
    pub year_mean_c: f32,
    /// How far off the nearest closed forest stands, in metres, and the direction to it.
    pub forest_m: f32,
    pub forest: Vec3,
    /// The share of the ground within 1.5 km of it under small lakes and pools.
    pub pools: f32,
    /// Kilometres east and north of the planet's start.
    pub at_km: Vec2,
}

/// What open tundra by the forest's edge must be: past the trees' niches, its growing season
/// colder than the tree line's, but not so cold that only lichen holds; level, low country
/// under a big sky; over permafrost, for the polygons and their pools; and the forest-tundra's
/// last spruce islands in sight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpenTundra {
    /// How much colder than the tree line's 6.4 °C its growing season is, at least and at
    /// most: past the last krummholz, short of the polar desert.
    pub colder_c: (f32, f32),
    /// How steep it may be, in degrees, and how much the ground within `relief_reach_m` of it
    /// may rise and fall, in metres.
    pub flatter_than_deg: f32,
    pub relief_m: f32,
    pub relief_reach_m: f32,
    /// How far off the nearest closed forest stands, at least and at most, in metres.
    pub forest_within_m: (f32, f32),
    /// The year's mean under which the ground stays frozen at depth.
    pub permafrost_below_c: f32,
    /// How far from the planet's start to look, and the grid looked over, in metres.
    pub search_m: f32,
    pub step_m: f32,
}

impl OpenTundra {
    /// The demo's: shrub tundra a degree to three past the tree line, its rises a few tens of
    /// metres, the forest's edge one or two kilometres off.
    pub const DEMO: OpenTundra = OpenTundra {
        colder_c: (0.8, 3.0),
        flatter_than_deg: 7.0,
        relief_m: 40.0,
        relief_reach_m: 300.0,
        forest_within_m: (500.0, 3000.0),
        permafrost_below_c: -2.0,
        search_m: 30_000.0,
        step_m: 100.0,
    };
}

/// How many cells a tundra query looked at, and what ruled them out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Searched {
    pub cells: usize,
    pub wrong_season: usize,
    pub thawed: usize,
    pub too_rough: usize,
    pub no_forest: usize,
    pub fit: usize,
}

enum Verdict {
    Season,
    Thawed,
    Rough,
    NoForest,
    Fit(f32, TundraSite),
}

impl Land {
    /// The best open tundra near the planet's start, if the land has any.
    pub fn tundra(&self, start: Vec3, want: &OpenTundra) -> Option<TundraSite> {
        self.tundra_searched(start, want).0
    }

    /// [`Land::tundra`], and what each test ruled out, so a query that finds nothing says why.
    pub fn tundra_searched(
        &self,
        start: Vec3,
        want: &OpenTundra,
    ) -> (Option<TundraSite>, Searched) {
        let mut searched = Searched::default();
        let start = start.normalize();
        let region = Region::sample(self, start, want.search_m * 2.0, want.step_m);
        let n = region.cells;
        let sea_m = self.sea_radius_m() - self.radius_m();
        let found = lakes::fill(&region, self.ground.sea_radius_m().map(|_| sea_m));
        let cells: Vec<usize> = (0..n * n).collect();
        let latitudes = crate::maps::par_map(&cells, |&k| {
            self.frame.latitude(region.direction(k % n, k / n))
        });
        let (low, high) = latitudes
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), &x| (l.min(x), h.max(x)));
        let seasons = Seasons::over_by(
            &self.fields,
            (low, high),
            (-50.0, self.ground.reach_m() + 50.0),
            0.25,
            50.0,
            2,
        );
        let season = |k: usize| {
            let over_sea = region.heights[k] - sea_m;
            (seasons.at(latitudes[k], over_sea, 0.0), over_sea)
        };
        // Where a closed forest stands, on the search's own grid.
        let forest: Vec<bool> = crate::maps::par_map(&cells, |&k| {
            let (s, over_sea) = season(k);
            over_sea > 0.0
                && found.lake[k].is_none()
                && stand(&s, region.slope_deg(k % n, k / n), Footing::Soil).density > 0.6
        });
        let reach = (want.relief_reach_m / want.step_m).ceil() as i64;
        let forest_reach = (want.forest_within_m.1 / want.step_m).ceil() as i64;
        let pool_reach = (1500.0 / want.step_m) as i64;
        let inside = |c: i64, r: i64| c >= 0 && r >= 0 && c < n as i64 && r < n as i64;
        let verdicts = crate::maps::par_map(&cells, |&k| {
            let (c, r) = ((k % n) as i64, (k / n) as i64);
            let (s, over_sea) = season(k);
            let colder = terra_climate::TREE_LINE_SEASON_C - s.growing_mean_c;
            if over_sea < 5.0
                || found.lake[k].is_some()
                || colder < want.colder_c.0
                || colder > want.colder_c.1
            {
                return Verdict::Season;
            }
            if s.year_mean_c > want.permafrost_below_c {
                return Verdict::Thawed;
            }
            if region.slope_deg(c as usize, r as usize) > want.flatter_than_deg {
                return Verdict::Rough;
            }
            let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
            for dr in -reach..=reach {
                for dc in -reach..=reach {
                    if !inside(c + dc, r + dr) {
                        return Verdict::Rough;
                    }
                    let h = region.height((c + dc) as usize, (r + dr) as usize);
                    lowest = lowest.min(h);
                    highest = highest.max(h);
                }
            }
            if highest - lowest > want.relief_m {
                return Verdict::Rough;
            }
            let mut nearest: Option<(i64, (usize, usize))> = None;
            for dr in -forest_reach..=forest_reach {
                for dc in -forest_reach..=forest_reach {
                    let d = dc * dc + dr * dr;
                    if d > forest_reach * forest_reach || nearest.is_some_and(|(b, _)| d >= b) {
                        continue;
                    }
                    let (cc, rr) = (c + dc, r + dr);
                    if inside(cc, rr) && forest[rr as usize * n + cc as usize] {
                        nearest = Some((d, (cc as usize, rr as usize)));
                    }
                }
            }
            let Some((d, (fc, fr))) = nearest else {
                return Verdict::NoForest;
            };
            let forest_m = (d as f32).sqrt() * want.step_m;
            if forest_m < want.forest_within_m.0 {
                return Verdict::NoForest;
            }
            let (mut wet, mut all) = (0usize, 0usize);
            for dr in -pool_reach..=pool_reach {
                for dc in -pool_reach..=pool_reach {
                    let (cc, rr) = (c + dc, r + dr);
                    if !inside(cc, rr) {
                        continue;
                    }
                    all += 1;
                    let small = found.lake[rr as usize * n + cc as usize]
                        .is_some_and(|l| found.lakes[l as usize].area_m2 < 2.0e6);
                    wet += usize::from(small);
                }
            }
            let pools = wet as f32 / all.max(1) as f32;
            let offset = region.offset(c as usize, r as usize);
            let toward = region.direction(c as usize, r as usize);
            // Best: the forest's edge a kilometre or two off, level ground, pools round it,
            // near the start.
            let score = (forest_m - 1500.0).abs() / 1000.0
                + (highest - lowest) / want.relief_m
                + offset.length() / want.search_m
                - (pools * 8.0).min(1.0);
            Verdict::Fit(
                score,
                TundraSite {
                    toward,
                    height_m: over_sea,
                    growing_mean_c: s.growing_mean_c,
                    year_mean_c: s.year_mean_c,
                    forest_m,
                    forest: region.direction(fc, fr),
                    pools,
                    at_km: self.east_north_km(start, toward),
                },
            )
        });
        let mut best: Option<(f32, TundraSite)> = None;
        for verdict in verdicts {
            searched.cells += 1;
            match verdict {
                Verdict::Season => searched.wrong_season += 1,
                Verdict::Thawed => searched.thawed += 1,
                Verdict::Rough => searched.too_rough += 1,
                Verdict::NoForest => searched.no_forest += 1,
                Verdict::Fit(score, site) => {
                    searched.fit += 1;
                    if best.is_none_or(|(b, _)| score < b) {
                        best = Some((score, site));
                    }
                }
            }
        }
        (best.map(|(_, site)| site), searched)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PLANET_START, ground, start_climate};
    use terra_core::{PlanetSpec, Seed};

    #[test]
    fn the_tundra_found_is_past_the_trees_over_permafrost_with_the_forest_in_sight() {
        let land = Land::new(
            ground::world(PlanetSpec {
                seed: Seed(1),
                radius_m: 30_000.0,
            }),
            start_climate().unwrap(),
        );
        let want = OpenTundra::DEMO;
        let (site, searched) = land.tundra_searched(PLANET_START, &want);
        let site = site.unwrap_or_else(|| panic!("no tundra: {searched:?}"));
        // Counted again from the climate at its own place, not from the query's table.
        let ground = land.surface(site.toward);
        let season = land.fields.season(land.place(ground, ground.normalize()));
        assert!(!season.holds_trees(), "{season:?}");
        let colder = terra_climate::TREE_LINE_SEASON_C - season.growing_mean_c;
        assert!(
            colder > want.colder_c.0 - 0.3 && colder < want.colder_c.1 + 0.3,
            "{colder}"
        );
        assert!(
            season.year_mean_c < want.permafrost_below_c + 0.3,
            "{season:?}"
        );
        // North of the start, where the climate puts it, and the forest's edge in sight of it.
        assert!(site.at_km.y > 5.0, "{:?}", site.at_km);
        let forest_ground = land.surface(site.forest);
        let forest = land
            .fields
            .season(land.place(forest_ground, forest_ground.normalize()));
        assert!(forest.holds_trees(), "{forest:?}");
        let apart = (forest_ground - ground).length();
        assert!(
            (want.forest_within_m.0 - 150.0..want.forest_within_m.1 + 150.0).contains(&apart),
            "{apart}"
        );
    }
}
