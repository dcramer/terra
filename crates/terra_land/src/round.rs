//! The land round a place, its rules worked out ahead for asking about it often: its lakes
//! (`water`), its growing seasons (`woods::Seasons`) and its snow on a day (`snow`), over the
//! span of a region. What the world on screen asks of every tree and every vertex of ground.

use crate::Land;
use crate::drift::Drift;
use crate::heath::{self, HeathMap};
use crate::snow::Snow;
use crate::water::Waters;
use crate::woods::{self, Footing, Seasons, Stand};
use glam::Vec3;

/// The land round a place.
pub struct Round {
    pub land: Land,
    pub waters: Waters,
    /// Where its ground is patterned, the pools in its polygons, on a finer grid nearer the
    /// middle.
    pub pools: Option<Waters>,
    pub seasons: Seasons,
    pub snow: Snow,
    /// How much of the falling snow its ground keeps against the prevailing wind (D323), over
    /// the lakes' grid.
    pub drift: Drift,
    /// What covers its open ground past the trees, community by community, over the lakes' grid.
    pub heath: HeathMap,
    /// How much its places' ground favours a tree beyond their season (`woods::Microsites`).
    pub microsites: woods::Microsites,
    /// Its middle, and how far across it reaches.
    pub middle: Vec3,
    pub across_m: f32,
}

/// Lakes are found on a grid this fine, round the middle, this far across.
const LAKE_CELL_M: f32 = 10.0;
const LAKES_ACROSS_M: f32 = 10_000.0;
/// Where the ground is patterned, the pools of its polygons are found on a grid this fine, this
/// far across: a polygon's sunken middle is a few metres wide.
const POOL_CELL_M: f32 = 2.0;
const POOLS_ACROSS_M: f32 = 1_600.0;
/// A pool stands as water once it is this deep and this big; shallower hollows are wet ground.
const POOL_DEEPEST_M: f32 = 0.35;
const POOL_AREA_M2: f32 = 8.0;

impl Round {
    /// The land round a direction, `across_m` wide, its snow as it lies on `day`.
    pub fn new(land: Land, toward: Vec3, across_m: f32, day: i64) -> Self {
        let middle = toward.normalize();
        let waters = Waters::round(&land, middle, LAKES_ACROSS_M.min(across_m), LAKE_CELL_M);
        let drift = Drift::over(
            waters.region(),
            crate::drift::wind_from(land.fields.climate().wind.prevailing_from_deg),
        );
        let latitude = land.frame.latitude(middle);
        let reach = across_m * 0.5 / land.radius_m();
        let latitudes = (latitude - reach, latitude + reach);
        let heights = (-50.0, land.ground.reach_m() + 50.0);
        let seasons = Seasons::over(&land.fields, latitudes, heights);
        let snow = Snow::on(
            &land.fields,
            day,
            middle * land.radius_m(),
            latitudes,
            heights,
        );
        let pools = land
            .ground
            .patterned()
            .is_some_and(|p| {
                let over_sea = land.surface(middle).length() - land.sea_radius_m();
                p.strength(middle, over_sea) > 0.2
            })
            .then(|| {
                // Only the thawed pools: the polygons' low middles a hand deep are wet hollows,
                // their water in the moss, not standing.
                Waters::round(&land, middle, POOLS_ACROSS_M.min(across_m), POOL_CELL_M).keeping(
                    |pool| pool.deepest_m >= POOL_DEEPEST_M && pool.area_m2 >= POOL_AREA_M2,
                )
            });
        let microsites = woods::Microsites::new(land.ground.spec().seed);
        let heath = heath_over(
            &land,
            &waters,
            pools.as_ref(),
            &seasons,
            &drift,
            &microsites,
        );
        Self {
            land,
            waters,
            pools,
            seasons,
            snow,
            drift,
            heath,
            microsites,
            middle,
            across_m,
        }
    }

    /// The surface of the water over a direction, a pool's or a lake's, else the sea's.
    pub fn water_level(&self, up: Vec3) -> Option<f32> {
        use terra_core::Water;
        if let Some(level) = self.pools.as_ref().and_then(|pools| pools.lake_level(up)) {
            return Some(level);
        }
        self.waters.level(up)
    }

    /// The highest water's surface at or beside a direction, a pool's within a cell of its
    /// grid, else a lake's within `cells` of the lakes'.
    pub fn water_near(&self, up: Vec3, cells: usize) -> Option<f32> {
        self.pools
            .as_ref()
            .and_then(|pools| pools.lake_near(up, 1))
            .or_else(|| self.waters.lake_near(up, cells))
    }

    /// The trees a point on the ground holds, facing `normal`, standing on `footing`.
    pub fn stand(&self, point: Vec3, normal: Vec3, footing: Footing) -> Stand {
        let place = self.land.place(point, normal);
        let season = self
            .seasons
            .at(place.latitude, place.height_m, place.sunward);
        let up = point.normalize_or(Vec3::Y);
        let slope = libm::acosf(normal.dot(up).clamp(-1.0, 1.0)).to_degrees();
        let region = self.waters.region();
        let shelter = region
            .cell_of(up)
            .map_or(0.0, |(c, r)| self.drift.shelter_at_cell(c, r));
        woods::stand_in(&season, slope, footing, self.microsites.at(point, shelter))
    }

    /// How deep the snow lies at a point on the ground, facing `normal`, in metres, as its
    /// shelter from the wind catches it (D323).
    pub fn snow_m(&self, point: Vec3, normal: Vec3) -> f32 {
        // The ground's lean over its lakes' 10 m grid, not a vertex's own: the metre-scale
        // relief of polygons and detail would turn the sun's melt from vertex to vertex.
        let up = point.normalize_or(Vec3::Y);
        let region = self.waters.region();
        let normal = region
            .cell_of(up)
            .map_or(normal, |(c, r)| region.normal(c, r));
        let place = self.place(point, normal);
        self.snow
            .depth_caught_m(place.latitude, place.height_m, place.sunward, place.catch)
    }

    /// A point on the ground facing `normal` as the climate takes it, with how much drifting
    /// snow it catches.
    pub fn place(&self, point: Vec3, normal: Vec3) -> terra_climate::Place {
        terra_climate::Place {
            catch: self.drift.at(point),
            ..self.land.place(point, normal)
        }
    }
}

/// The heath over the lakes' grid round a place: each cell's ground as the land makes it, its
/// growing season against the tree line's, the trees on it, its shelter from the wind and how
/// wet it is.
fn heath_over(
    land: &Land,
    waters: &Waters,
    pools: Option<&Waters>,
    seasons: &Seasons,
    drift: &Drift,
    microsites: &woods::Microsites,
) -> HeathMap {
    let region = waters.region();
    let radius = land.radius_m();
    let drainage = heath::Drainage::new(land.ground.spec().seed);
    HeathMap::over(region, |c, r| {
        let up = region.direction(c, r);
        let point = up * (radius + region.height(c, r));
        let normal = region.normal(c, r);
        let place = land.place(point, normal);
        let season = seasons.at(place.latitude, place.height_m, place.sunward);
        // A pool's water within a few metres soaks the ground as a lake's does.
        let above_water = pools
            .and_then(|pools| pools.lake_near(up, 3))
            .or_else(|| waters.lake_near(up, 1))
            .map_or(50.0, |level| point.length() - level);
        if above_water < 0.0 {
            return heath::Heath {
                wet: 1.0,
                ..heath::Heath::default()
            };
        }
        let footing = land.footing(point, None);
        if footing == Footing::Rock {
            return heath::Heath::default();
        }
        let slope_deg = region.slope_deg(c, r);
        let microsite = microsites.at(point, drift.shelter_at_cell(c, r));
        let trees = woods::stand_in(&season, slope_deg, footing, microsite).density;
        let frozen = smoothstep(-1.0, -4.0, season.year_mean_c);
        heath::heath(heath::Ground {
            colder_c: terra_climate::TREE_LINE_SEASON_C - season.growing_mean_c,
            trees,
            shelter_deg: drift.shelter_at_cell(c, r),
            wet: (heath::wetness(region, c, r, above_water, frozen) + drainage.at(point))
                .clamp(0.0, 1.0),
            slope_deg,
        })
    })
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What a still lake mirrors, all round it (D322): in each of so many ways out, how high the
/// ground stands against the sky seen from the water, as an angle; the highest ground's height
/// over the water; and the tree line's and the snow line's over it. A lake's surface reflects
/// the slopes round it more than the sky, and this is how they stand, for its shader to draw
/// them in its reflection without drawing the world twice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Skyline {
    pub elevation: f32,
    pub top_m: f32,
    pub tree_line_m: f32,
    pub snow_line_m: f32,
}

/// How far out the skyline round a lake is looked for, and in steps of how far.
const SKYLINE_M: f32 = 9000.0;
const SKYLINE_STEP_M: f32 = 40.0;

impl Round {
    /// The skyline round a point on a lake whose water stands `level_m` from the centre, in
    /// `ways` ways out, the first due east and turning toward the north.
    pub fn skyline(&self, at: Vec3, level_m: f32, ways: usize) -> Vec<Skyline> {
        let land = &self.land;
        let up = at.normalize();
        let latitude = land.frame.latitude(up);
        let water_over_sea = level_m - land.sea_radius_m();
        let tree_line = land
            .fields
            .tree_line_m(latitude, 0.0, 6000.0)
            .unwrap_or(0.0)
            - water_over_sea;
        let snow_line = (0..240)
            .map(|k| k as f32 * 25.0)
            .find(|&h| self.snow.depth_m(latitude, h, 0.0) > 0.1)
            .map_or(1.0e4, |h| h - water_over_sea);
        let ways: Vec<usize> = (0..ways).collect();
        let count = ways.len();
        crate::maps::par_map(&ways, |&k| {
            let angle = k as f32 / count as f32 * core::f32::consts::TAU;
            let way = glam::Vec2::new(libm::cosf(angle), libm::sinf(angle));
            let (mut elevation, mut top) = (0.0f32, 0.0f32);
            let mut d = SKYLINE_STEP_M;
            while d <= SKYLINE_M {
                let ground = land.surface(land.along(up, way * d)).length() - level_m;
                // The planet's curve drops the far ground away from a level line of sight.
                let drop = d * d / (2.0 * level_m);
                let seen = libm::atan2f(ground - drop, d);
                if seen > elevation {
                    elevation = seen;
                    top = ground;
                }
                d += SKYLINE_STEP_M;
            }
            Skyline {
                elevation,
                top_m: top,
                tree_line_m: tree_line,
                snow_line_m: snow_line,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::MountainLake;
    use crate::{PLANET_START, ground, start_climate};
    use terra_core::{PlanetSpec, Seed};

    #[test]
    fn round_a_mountain_lake_in_early_summer_its_shore_is_bare_and_wooded_and_its_peaks_white() {
        let climate = start_climate().unwrap();
        let day = terra_climate::Date::parse("06-10")
            .unwrap()
            .day(&climate.year);
        let land = Land::new(
            ground::world(PlanetSpec {
                seed: Seed(1),
                radius_m: 30_000.0,
            }),
            climate,
        );
        let site = land
            .mountain_lake(PLANET_START, &MountainLake::DEMO)
            .unwrap();
        let round = Round::new(land, site.toward, 8000.0, day);
        let shore = round.land.surface(site.shore);
        let up = shore.normalize();
        assert_eq!(round.snow_m(shore, up), 0.0, "snow on the shore");
        assert!(round.stand(shore, up, Footing::Soil).density > 0.5);
        // Counted the long way: the fold at the shore's own place.
        let place = round.land.place(shore, up);
        assert_eq!(
            terra_climate::past(&round.land.fields, place, day).snow_depth_m(),
            0.0
        );
        // And ground round it, up toward its peaks, white.
        let white = (0..64).any(|k| {
            let a = k as f32 / 64.0 * core::f32::consts::TAU;
            let at = round.land.surface(round.land.along(
                site.toward,
                glam::Vec2::new(libm::cosf(a), libm::sinf(a)) * 2500.0,
            ));
            round.snow_m(at, at.normalize()) > 0.2
        });
        assert!(white, "no snow on the ground round it");
        // Its skyline: mountains round it, the highest as high over it as the query found.
        let level = round.land.sea_radius_m() + site.level_m;
        let skyline = round.skyline(site.toward * level, level, 32);
        let highest = skyline.iter().map(|s| s.top_m).fold(0.0, f32::max);
        assert!(
            highest > MountainLake::DEMO.peaks_above_m * 0.9,
            "{highest}"
        );
        assert!(
            skyline
                .iter()
                .all(|s| s.elevation >= 0.0 && s.elevation < 0.8)
        );
        assert!(skyline[0].tree_line_m > 0.0 && skyline[0].snow_line_m > skyline[0].tree_line_m);
        // No tree round its shore stands in its water: every foot is above the surface of any
        // lake within two cells of it.
        let trees = crate::woods::Woods::of(&round).trees(site.toward, 0.0, 700.0, 4.0, Seed(5));
        assert!(trees.len() > 1000, "{}", trees.len());
        for tree in &trees {
            if let Some(level) = round.waters.lake_near(tree.at.normalize(), 2) {
                assert!(
                    tree.at.length() > level,
                    "a tree {:.2} m under the lake",
                    level - tree.at.length()
                );
            }
        }
    }
}
