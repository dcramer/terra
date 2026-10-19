//! A market town's place, found by what the land's rules make of it (docs/demos.md §3, D321):
//! level ground for a town of about 200 m, warm enough below the tree line for the fields a
//! town lives on, the closed forest a few hundred metres off round much of it, and mountains
//! standing within some kilometres in view, as a valley town at the foot of a range is.
//! A query over the land round the planet's start, as the mountain lake's (`site`) and the
//! tundra's are; nothing is painted, and if the rules put no such place on a world the query
//! says why.
//!
//! Below the tree line the rules stand closed forest on all level ground (`woods`), so a town
//! has cleared its own land: its fields and pasture round it, a few hundred metres out, their
//! edge ragged as fields' are, the forest thinning into it ([`Clearing`]). The trees, the
//! far forest's billboards and the ground's canopy all ask the same clearing.

use crate::Land;
use crate::lakes;
use crate::region::Region;
use crate::woods::{Footing, Seasons, stand};
use glam::{Vec2, Vec3};
use terra_gen::smoothstep;

/// A town's place found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TownSite {
    /// The direction from the centre to the ground its square is wanted on.
    pub toward: Vec3,
    /// Its height over the sea, its growing season's mean and the tree line's height there.
    pub height_m: f32,
    pub growing_mean_c: f32,
    pub tree_line_m: f32,
    /// How much the ground within the town's reach rises and falls, in metres, and how steep
    /// it is at the middle, in degrees.
    pub relief_m: f32,
    pub slope_deg: f32,
    /// The share of the ways out of it on which closed forest stands past its fields.
    pub forest: f32,
    /// The highest ground within the query's reach, its height over the sea and how far off
    /// it is, in metres.
    pub peak: Vec3,
    pub peak_m: f32,
    pub peak_off_m: f32,
    /// How high the skyline stands seen from the square at eye height, in degrees: its
    /// highest, and the share of the ways round on which it stands over two degrees.
    pub skyline_deg: f32,
    pub ringed: f32,
    /// Where the world's start stands, out of the town by its fields, for the player's base
    /// and what is laid round it.
    pub start: Vec3,
    /// Kilometres east and north of the planet's start.
    pub at_km: Vec2,
    /// Where the nearest shore of a lake stands from its middle, metres east and north, as
    /// the world draws its water, if the query asked for one (`MarketTown::water_m`).
    pub water: Option<Vec2>,
}

/// What a market town's place must be (docs/demos.md §3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarketTown {
    /// How much warmer than the tree line's 6.4 °C its growing season is, at least: fields of
    /// barley and oats want a season well inside the trees' (Alpine valleys farm to about
    /// 500 m under the tree line).
    pub warmer_c: f32,
    /// How steep its middle may be, in degrees, and how much the ground within `reach_m` of
    /// it may rise and fall, in metres: a town of 200 m on a gentle slope.
    pub flatter_than_deg: f32,
    pub relief_m: f32,
    pub reach_m: f32,
    /// How far out its fields clear the forest, and its edge's width, in metres.
    pub clearing_m: f32,
    pub clearing_edge_m: f32,
    /// How far past the fields the closed forest must stand on a way out, and on what share of
    /// the ways at least.
    pub forest_within_m: f32,
    pub forest_share: f32,
    /// How much higher the highest ground within `peaks_within_m` stands, at least, and how
    /// far over the tree line, so its tops are bare; and how high the skyline must stand from
    /// the square, in degrees.
    pub peaks_above_m: f32,
    pub peaks_over_trees_m: f32,
    pub peaks_within_m: f32,
    pub skyline_deg: f32,
    /// How far from the town the world's start stands, and how far round it its own clearing
    /// reaches (a base, the village and its herd).
    pub start_m: f32,
    pub start_clearing_m: f32,
    /// How far from the planet's start to look, and the grid looked over, in metres.
    pub search_m: f32,
    pub step_m: f32,
    /// A lake wanted beside it, its nearest shore between so many metres from the middle and
    /// so many, and at least so big, in square metres, as the world draws its water (its
    /// lakes found on its own ten-metre grid, `round::Round`), not the search's coarse one;
    /// none if not asked.
    pub water_m: Option<(f32, f32, f32)>,
    /// Where a demo's place was found and stays, kilometres east and north of the planet's
    /// start (`--site <name>@<east>,<north>`, D393): of the places that fit within
    /// [`PINNED_WITHIN_KM`](crate::site::PINNED_WITHIN_KM) of it, the nearest is taken, so a change to the land that makes
    /// a better place elsewhere does not move the demo, and one that spoils this place says so.
    /// None to search everywhere.
    pub pinned_km: Option<Vec2>,
}

impl MarketTown {
    /// The demo's: a valley floor at the foot of the range, its fields 300 m out, closed
    /// forest round most of it and peaks a kilometre over it within 7 km.
    pub const DEMO: MarketTown = MarketTown {
        warmer_c: 3.0,
        flatter_than_deg: 5.0,
        relief_m: 12.0,
        reach_m: 120.0,
        clearing_m: 300.0,
        clearing_edge_m: 70.0,
        forest_within_m: 700.0,
        forest_share: 0.55,
        peaks_above_m: 900.0,
        peaks_over_trees_m: 200.0,
        peaks_within_m: 7000.0,
        skyline_deg: 5.0,
        start_m: 330.0,
        start_clearing_m: 170.0,
        search_m: 9000.0,
        step_m: 50.0,
        water_m: None,
        pinned_km: None,
    };

    /// A settlement newly founded (D381): the same valley floor, but only its plots cleared
    /// of the forest, the trees standing a few dozen metres past its last houses in bays and
    /// tongues, as a clearing cut for a village is.
    ///
    /// By a lake if one is to be had among the best places (D381): its shore just past its
    /// last houses, so the settlement looks over water to the forest and the range, as a
    /// village is founded by water.
    pub const SETTLEMENT: MarketTown = MarketTown {
        clearing_m: 210.0,
        clearing_edge_m: 45.0,
        water_m: Some((230.0, 420.0, 40_000.0)),
        ..MarketTown::DEMO
    };

    /// The forest a town found by this query has cleared: its fields round it, and round the
    /// world's start beside it.
    pub fn clearings(&self, land: &Land, site: &TownSite) -> Vec<Clearing> {
        let seed = land.ground.spec().seed.derive("fields");
        let mut clearings = vec![
            Clearing::new(
                land,
                site.toward,
                self.clearing_m,
                self.clearing_edge_m,
                seed,
            ),
            Clearing::new(
                land,
                site.start,
                self.start_clearing_m,
                self.clearing_edge_m,
                seed.derive("start"),
            ),
        ];
        // A settlement by a lake clears down to its shore (D381), so the water is seen from
        // its houses and they from it, not hidden behind a belt of trees.
        if let Some(water) = site.water {
            clearings.push(Clearing::new(
                land,
                land.along(site.toward, water * 0.7),
                water.length() * 0.5,
                self.clearing_edge_m,
                seed.derive("shore"),
            ));
        }
        clearings
    }
}

/// Ground a settlement has cleared of the forest for its fields and pasture: none stand
/// within it, and at its edge, ragged as fields' edges are, the forest thins into it.
#[derive(Clone, Debug)]
pub struct Clearing {
    /// Its middle's direction from the centre, and the world's radius.
    pub middle: Vec3,
    world_m: f32,
    /// How far out it reaches on average, and how wide its edge is, in metres.
    pub radius_m: f32,
    pub edge_m: f32,
    ragged: terra_gen::Noise,
}

/// How far a clearing's edge wanders in and out of its average, as a share of it, and how
/// long its bays and tongues are, in metres.
const RAGGED: f32 = 0.22;
const RAGGED_M: f32 = 160.0;

/// How far past a clearing's edge its forest is young growth, in metres.
const YOUNG_M: f32 = 30.0;

impl Clearing {
    pub fn new(
        land: &Land,
        middle: Vec3,
        radius_m: f32,
        edge_m: f32,
        seed: terra_gen::Seed,
    ) -> Self {
        Self {
            middle: middle.normalize(),
            world_m: land.radius_m(),
            radius_m,
            edge_m,
            ragged: terra_gen::Noise::new(seed),
        }
    }

    /// The share of its stand the clearing leaves a point on the ground: none inside it, all
    /// past its edge.
    pub fn keeps(&self, point: Vec3) -> f32 {
        self.past(point, 0.0)
    }

    /// How grown the forest is at a point, 0 to 1: young growth where it starts at the
    /// clearing's edge, full grown `YOUNG_M` past it (D381), as a forest's edge on fields is a
    /// mantle of young trees before the old stand.
    pub fn grown(&self, point: Vec3) -> f32 {
        self.past(point, YOUNG_M)
    }

    /// How far a point is past the clearing's ragged edge, 0 inside it to 1 past its edge and
    /// `further` metres more.
    fn past(&self, point: Vec3, further: f32) -> f32 {
        let up = point.normalize_or(Vec3::Y);
        let distance = up.distance(self.middle) * self.world_m;
        if distance > self.radius_m * (1.0 + RAGGED * 1.5) + self.edge_m + further {
            return 1.0;
        }
        // The noise's spread is about 0.27 a sample (`woods::Microsites`).
        let wander =
            (self.ragged.fbm(up * self.world_m / RAGGED_M, 3, 2.1, 0.5) / 0.27).clamp(-1.5, 1.5);
        let edge = self.radius_m * (1.0 + RAGGED * wander);
        smoothstep(
            edge - self.edge_m * 0.5,
            edge + self.edge_m * 0.5 + further,
            distance,
        )
    }
}

/// How many cells a town query looked at, and what ruled them out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Searched {
    pub cells: usize,
    pub wrong_season: usize,
    pub too_rough: usize,
    pub no_forest: usize,
    pub no_peaks: usize,
    /// Fit, but too far from the query's pin to be the pinned place.
    pub off_pin: usize,
    pub fit: usize,
    /// Whether the place found is by a lake, as asked.
    pub by_water: bool,
}

/// How many of the best places a search looks at again for a lake beside, and the grid it
/// finds their water on, the world's own (`round::LAKE_CELL_M`).
const WATER_TRIED: usize = 2_000;
const WATER_CELL_M: f32 = 10.0;

enum Verdict {
    Season,
    Rough,
    NoForest,
    NoPeaks,
    Fit(f32, TownSite),
}

/// How many ways round a place the forest and the skyline are looked for along.
const WAYS: usize = 32;
/// The eye's height over the ground, in metres.
const EYE_M: f32 = 1.7;

impl Land {
    /// The best place for a market town near the planet's start, if the land has one.
    pub fn market_town(&self, start: Vec3, want: &MarketTown) -> Option<TownSite> {
        self.market_town_searched(start, want).0
    }

    /// [`Land::market_town`], and what each test ruled out, so a query that finds nothing
    /// says why.
    pub fn market_town_searched(
        &self,
        start: Vec3,
        want: &MarketTown,
    ) -> (Option<TownSite>, Searched) {
        let mut searched = Searched::default();
        if self.ground.mountains().is_none() {
            return (None, searched);
        }
        let start = start.normalize();
        let size_m = (want.search_m + want.peaks_within_m) * 2.0;
        let region = Region::sample(self, start, size_m, want.step_m);
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
            5,
        );
        let sunward = crate::maps::par_map(&cells, |&k| {
            let (c, r) = (k % n, k / n);
            self.frame
                .sunward(region.direction(c, r), region.normal(c, r))
        });
        let season = |k: usize| {
            let over_sea = region.heights[k] - sea_m;
            (seasons.at(latitudes[k], over_sea, sunward[k]), over_sea)
        };
        // Where closed forest stands, on the search's own grid.
        let forest: Vec<bool> = crate::maps::par_map(&cells, |&k| {
            let (s, over_sea) = season(k);
            over_sea > 0.0
                && found.lake[k].is_none()
                && stand(&s, region.slope_deg(k % n, k / n), Footing::Soil).density > 0.6
        });
        let search = (want.search_m / want.step_m) as i64;
        let middle = (n / 2) as i64;
        let reach = (want.reach_m / want.step_m).ceil() as i64;
        let peaks = (want.peaks_within_m / want.step_m).ceil() as i64;
        let inside = |c: i64, r: i64| c >= 0 && r >= 0 && c < n as i64 && r < n as i64;
        let ways: Vec<Vec2> = (0..WAYS)
            .map(|w| {
                let angle = w as f32 / WAYS as f32 * core::f32::consts::TAU;
                Vec2::new(libm::cosf(angle), libm::sinf(angle))
            })
            .collect();
        let verdicts = crate::maps::par_map(&cells, |&k| {
            let (c, r) = ((k % n) as i64, (k / n) as i64);
            if (c - middle).abs() > search || (r - middle).abs() > search {
                return None;
            }
            let (s, over_sea) = season(k);
            let warmer = s.growing_mean_c - terra_climate::TREE_LINE_SEASON_C;
            if over_sea < 10.0 || found.lake[k].is_some() || warmer < want.warmer_c {
                return Some(Verdict::Season);
            }
            let slope_deg = region.slope_deg(c as usize, r as usize);
            if slope_deg > want.flatter_than_deg {
                return Some(Verdict::Rough);
            }
            let here = region.height(c as usize, r as usize);
            let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
            for dr in -reach..=reach {
                for dc in -reach..=reach {
                    if dc * dc + dr * dr > reach * reach {
                        continue;
                    }
                    let (cc, rr) = (c + dc, r + dr);
                    if !inside(cc, rr) || found.lake[rr as usize * n + cc as usize].is_some() {
                        return Some(Verdict::Rough);
                    }
                    let h = region.height(cc as usize, rr as usize);
                    lowest = lowest.min(h);
                    highest = highest.max(h);
                }
            }
            let relief_m = highest - lowest;
            if relief_m > want.relief_m {
                return Some(Verdict::Rough);
            }
            let point = region.point(self, c as usize, r as usize);
            if self.footing(point, None) != Footing::Soil {
                return Some(Verdict::Rough);
            }
            let at = region.offset(c as usize, r as usize);
            // The closed forest past its fields, way by way.
            let wooded = ways
                .iter()
                .filter(|&&way| {
                    let mut d = want.clearing_m;
                    while d <= want.clearing_m + want.forest_within_m {
                        let p = at + way * d;
                        let (cc, rr) = (
                            libm::floorf(p.x / want.step_m + n as f32 * 0.5) as i64,
                            libm::floorf(n as f32 * 0.5 - p.y / want.step_m) as i64,
                        );
                        if inside(cc, rr) && forest[rr as usize * n + cc as usize] {
                            return true;
                        }
                        d += want.step_m;
                    }
                    false
                })
                .count();
            let forest_share = wooded as f32 / WAYS as f32;
            if forest_share < want.forest_share {
                return Some(Verdict::NoForest);
            }
            // The highest ground within reach, and the skyline seen from the square.
            let mut peak = (f32::MIN, (c, r));
            for dr in -peaks..=peaks {
                for dc in -peaks..=peaks {
                    if dc * dc + dr * dr > peaks * peaks || !inside(c + dc, r + dr) {
                        continue;
                    }
                    let h = region.height((c + dc) as usize, (r + dr) as usize);
                    if h > peak.0 {
                        peak = (h, (c + dc, r + dr));
                    }
                }
            }
            let peak_m = peak.0 - sea_m;
            let tree_line_m = self
                .fields
                .tree_line_m(latitudes[k], 0.0, 6000.0)
                .unwrap_or(0.0);
            if peak.0 - here < want.peaks_above_m || peak_m < tree_line_m + want.peaks_over_trees_m
            {
                return Some(Verdict::NoPeaks);
            }
            let eye = here + EYE_M;
            let world_m = self.radius_m();
            let skyline: Vec<f32> = ways
                .iter()
                .map(|&way| {
                    let mut steepest = f32::MIN;
                    let mut d = want.clearing_m;
                    while d <= want.peaks_within_m {
                        let rise = region.height_at(at + way * d) - eye - d * d / (2.0 * world_m);
                        steepest = steepest.max(libm::atanf(rise / d));
                        d += want.step_m;
                    }
                    steepest.to_degrees()
                })
                .collect();
            let skyline_deg = skyline.iter().copied().fold(f32::MIN, f32::max);
            if skyline_deg < want.skyline_deg {
                return Some(Verdict::NoPeaks);
            }
            let ringed = skyline.iter().filter(|&&d| d > 2.0).count() as f32 / WAYS as f32;
            let toward = region.direction(c as usize, r as usize);
            let peak_at = region.offset(peak.1.0 as usize, peak.1.1 as usize);
            // Best: level, the forest round it, the range standing high over it and round
            // much of it, but open on some side, and near the start.
            let score = relief_m / want.relief_m + slope_deg / want.flatter_than_deg
                - forest_share
                - (skyline_deg / 15.0).min(1.0)
                - (ringed * 1.4).min(1.0)
                + at.length() / want.search_m;
            Some(Verdict::Fit(
                score,
                TownSite {
                    toward,
                    height_m: over_sea,
                    growing_mean_c: s.growing_mean_c,
                    tree_line_m,
                    relief_m,
                    slope_deg,
                    forest: forest_share,
                    peak: region.direction(peak.1.0 as usize, peak.1.1 as usize),
                    peak_m,
                    peak_off_m: peak_at.distance(at),
                    skyline_deg,
                    ringed,
                    start: toward,
                    at_km: self.east_north_km(start, toward),
                    water: None,
                },
            ))
        });
        let mut fits: Vec<(f32, TownSite)> = Vec::new();
        for verdict in verdicts.into_iter().flatten() {
            searched.cells += 1;
            match verdict {
                Verdict::Season => searched.wrong_season += 1,
                Verdict::Rough => searched.too_rough += 1,
                Verdict::NoForest => searched.no_forest += 1,
                Verdict::NoPeaks => searched.no_peaks += 1,
                Verdict::Fit(_, site) if crate::site::off_pin(want.pinned_km, site.at_km) => {
                    searched.off_pin += 1;
                }
                Verdict::Fit(score, site) => {
                    searched.fit += 1;
                    let score = crate::site::pinned_score(want.pinned_km, site.at_km, score);
                    fits.push((score, site));
                }
            }
        }
        fits.sort_by(|a, b| a.0.total_cmp(&b.0));
        // By a lake if asked: the best few places looked at again as the world draws its
        // water, the first with a lake's shore where it is wanted and none in its plots.
        let watered = want.water_m.and_then(|wanted| {
            let tried: Vec<&(f32, TownSite)> = fits.iter().take(WATER_TRIED).collect();
            let found =
                crate::maps::par_map(&tried, |(_, site)| self.lake_beside(site, want, wanted));
            tried.iter().zip(found).find_map(|((_, site), water)| {
                water.map(|water| TownSite {
                    water: Some(water),
                    ..*site
                })
            })
        });
        searched.by_water = watered.is_some();
        let best = watered.or_else(|| fits.first().map(|(_, site)| *site));
        let best = best.map(|site| TownSite {
            start: self.start_beside(&site, want),
            ..site
        });
        (best, searched)
    }

    /// Where a lake's nearest shore stands from a town's middle, metres east and north, if
    /// one at least `wanted.2` square metres has it between `wanted.0` and `wanted.1` metres
    /// off and no water lies within its reach: its lakes found as the world draws them, on its
    /// own ten-metre grid (`round::Round`).
    fn lake_beside(
        &self,
        site: &TownSite,
        want: &MarketTown,
        wanted: (f32, f32, f32),
    ) -> Option<Vec2> {
        let (near_m, far_m, least_m2) = wanted;
        let waters =
            crate::water::Waters::round(self, site.toward, far_m * 2.0 + 100.0, WATER_CELL_M);
        let region = waters.region();
        let n = region.cells;
        // The sea's shore counts as a lake's, as the world draws its water.
        let sea_m = self.ground.sea_radius_m().map(|r| r - self.radius_m());
        let mut nearest: Option<Vec2> = None;
        for row in 0..n {
            for column in 0..n {
                let lake = waters.lake_at(column, row);
                let sea = sea_m.is_some_and(|sea| region.height(column, row) < sea);
                if lake.is_none() && !sea {
                    continue;
                }
                let at = region.offset(column, row);
                let off = at.length();
                if off < want.reach_m.min(near_m) {
                    return None;
                }
                let big = sea || lake.is_some_and(|l| waters.lakes()[l].area_m2 >= least_m2);
                if !big || off > far_m {
                    continue;
                }
                if nearest.is_none_or(|n| off < n.length()) {
                    nearest = Some(at);
                }
            }
        }
        nearest.filter(|at| at.length() >= near_m)
    }

    /// Where the world's start stands beside a town: of the ways out of it at `start_m`, the
    /// most level dry ground, away from the range, so the base and the village beside it do
    /// not stand between the town and its mountains.
    fn start_beside(&self, site: &TownSite, want: &MarketTown) -> Vec3 {
        let sea = self.sea_radius_m();
        let (east, north) = (self.frame.east(site.toward), self.frame.north(site.toward));
        let to_peak = {
            let p = site.peak - site.toward * site.peak.dot(site.toward);
            Vec2::new(p.dot(east), p.dot(north)).normalize_or_zero()
        };
        let mut best: Option<(f32, Vec3)> = None;
        for w in 0..WAYS {
            let angle = w as f32 / WAYS as f32 * core::f32::consts::TAU;
            let way = Vec2::new(libm::cosf(angle), libm::sinf(angle));
            // Away from its lake, so the start stands dry and the town is seen with its water.
            if site
                .water
                .is_some_and(|water| way.dot(water.normalize_or_zero()) > 0.0)
            {
                continue;
            }
            let at = self.along(site.toward, way * want.start_m);
            let middle = self.surface(at).length();
            if middle < sea + 3.0 {
                continue;
            }
            let (mut low, mut high) = (f32::MAX, f32::MIN);
            for ring in [10.0, 25.0, 40.0] {
                for k in 0..12 {
                    let a = k as f32 / 12.0 * core::f32::consts::TAU;
                    let p = self.along(at, Vec2::new(libm::cosf(a), libm::sinf(a)) * ring);
                    let h = self.surface(p).length();
                    low = low.min(h);
                    high = high.max(h);
                }
            }
            if low < sea + 2.0 {
                continue;
            }
            let score = (high - low) + 4.0 * (way.dot(to_peak) + 1.0);
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, at));
            }
        }
        best.map_or(site.toward, |(_, at)| at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PLANET_START, ground, start_climate};
    use terra_core::{PlanetSpec, Seed};

    fn land(radius_m: f32) -> Land {
        Land::new(
            ground::world(PlanetSpec {
                seed: Seed(1),
                radius_m,
            }),
            start_climate().unwrap(),
        )
    }

    #[test] // Slow: minutes.
    fn the_town_found_is_level_warm_wooded_round_and_under_peaks() {
        let land = land(30_000.0);
        let want = MarketTown::DEMO;
        let (site, searched) = land.market_town_searched(PLANET_START, &want);
        let site = site.unwrap_or_else(|| panic!("no town site: {searched:?}"));
        // Counted again from the ground and the climate at its own place, not from the
        // query's grid and tables.
        let ground = land.surface(site.toward);
        let season = land.fields.season(land.place(ground, ground.normalize()));
        assert!(
            season.growing_mean_c > terra_climate::TREE_LINE_SEASON_C + want.warmer_c - 0.3,
            "{season:?}"
        );
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for ring in [30.0, 60.0, 90.0, 115.0] {
            for k in 0..16 {
                let a = k as f32 / 16.0 * core::f32::consts::TAU;
                let p = land.along(site.toward, Vec2::new(libm::cosf(a), libm::sinf(a)) * ring);
                let h = land.surface(p).length();
                low = low.min(h);
                high = high.max(h);
            }
        }
        assert!(high - low < want.relief_m + 4.0, "{}", high - low);
        let peak = land.surface(site.peak).length() - land.sea_radius_m();
        assert!(peak > site.height_m + want.peaks_above_m, "{peak}");
        assert!(peak > site.tree_line_m, "{peak}");
        // No tree stands in the town or the start's ground, near or far, and the forest stands
        // past its fields: the woods as the world grows them, round the town.
        let mut round = crate::round::Round::new(land.clone(), site.toward, 2400.0, 0);
        round.clearings = want.clearings(&land, &site);
        let trees = crate::woods::Woods::of(&round).trees(
            ground,
            0.0,
            1000.0,
            6.5,
            Seed(5).derive("woods"),
        );
        let off = |tree: &crate::woods::Tree, at: Vec3| {
            (tree.at.normalize() - at.normalize()).length() * land.radius_m()
        };
        let inner = want.clearing_m * (1.0 - RAGGED * 1.5) - want.clearing_edge_m * 0.5;
        assert!(
            trees.iter().all(|t| off(t, site.toward) > inner),
            "a tree in the town"
        );
        let start_inner = want.start_clearing_m * (1.0 - RAGGED * 1.5) - want.clearing_edge_m * 0.5;
        assert!(
            trees.iter().all(|t| off(t, site.start) > start_inner),
            "a tree at the start"
        );
        let past = trees.iter().filter(|t| off(t, site.toward) > 600.0).count();
        assert!(past > 1000, "{past} trees past the fields");
        // Its edge grows back young: the trees at the fields' edge stand shorter than the
        // forest's past them.
        let mean = |within: std::ops::Range<f32>| {
            let statures: Vec<f32> = trees
                .iter()
                .filter(|t| within.contains(&off(t, site.toward)))
                .map(|t| t.stature)
                .collect();
            assert!(
                statures.len() > 20,
                "{} trees in {within:?}",
                statures.len()
            );
            statures.iter().sum::<f32>() / statures.len() as f32
        };
        let edge = mean(inner..want.clearing_m + want.clearing_edge_m * 0.3);
        let forest = mean(600.0..900.0);
        assert!(
            edge < forest * 0.85,
            "the edge's {edge}, the forest's {forest}"
        );
        // The start stands out of the town, by its fields.
        let apart = (land.surface(site.start) - ground).length();
        assert!(
            (want.start_m - 20.0..want.start_m + 20.0).contains(&apart),
            "{apart}"
        );
    }

    #[test]
    fn a_clearing_keeps_no_trees_within_and_all_beyond() {
        let land = land(30_000.0);
        let clearing = Clearing::new(&land, PLANET_START, 300.0, 60.0, Seed(3));
        assert_eq!(clearing.keeps(PLANET_START * 30_000.0), 0.0);
        let near = land.along(PLANET_START, Vec2::new(150.0, 0.0));
        assert_eq!(clearing.keeps(near), 0.0);
        let far = land.along(PLANET_START, Vec2::new(0.0, 600.0));
        assert_eq!(clearing.keeps(far), 1.0);
        // Its edge wanders: not every way out reaches it at the same distance.
        let reached: Vec<f32> = (0..16)
            .map(|k| {
                let a = k as f32 / 16.0 * core::f32::consts::TAU;
                let way = Vec2::new(libm::cosf(a), libm::sinf(a));
                (200..500)
                    .step_by(5)
                    .map(|d| d as f32)
                    .find(|&d| clearing.keeps(land.along(PLANET_START, way * d)) > 0.5)
                    .unwrap_or(500.0)
            })
            .collect();
        let (low, high) = reached
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), &x| (l.min(x), h.max(x)));
        assert!(high - low > 30.0, "{reached:?}");
    }

    #[test]
    fn a_world_without_mountains_has_no_town_site() {
        let land = land(2000.0);
        assert_eq!(land.market_town(PLANET_START, &MarketTown::DEMO), None);
    }
}
