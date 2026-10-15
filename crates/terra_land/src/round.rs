//! The land round a place, its rules worked out ahead for asking about it often: its lakes
//! (`water`), its growing seasons (`woods::Seasons`) and its snow on a day (`snow`), over the
//! span of a region. What the world on screen asks of every tree and every vertex of ground.

use crate::Land;
use crate::snow::Snow;
use crate::water::Waters;
use crate::woods::{self, Footing, Seasons, Stand};
use glam::Vec3;

/// The land round a place.
pub struct Round {
    pub land: Land,
    pub waters: Waters,
    pub seasons: Seasons,
    pub snow: Snow,
    /// Its middle, and how far across it reaches.
    pub middle: Vec3,
    pub across_m: f32,
}

/// Lakes are found on a grid this fine, round the middle, this far across.
const LAKE_CELL_M: f32 = 10.0;
const LAKES_ACROSS_M: f32 = 10_000.0;

impl Round {
    /// The land round a direction, `across_m` wide, its snow as it lies on `day`.
    pub fn new(land: Land, toward: Vec3, across_m: f32, day: i64) -> Self {
        let middle = toward.normalize();
        let waters = Waters::round(&land, middle, LAKES_ACROSS_M.min(across_m), LAKE_CELL_M);
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
        Self {
            land,
            waters,
            seasons,
            snow,
            middle,
            across_m,
        }
    }

    /// The trees a point on the ground holds, facing `normal`, standing on `footing`.
    pub fn stand(&self, point: Vec3, normal: Vec3, footing: Footing) -> Stand {
        let place = self.land.place(point, normal);
        let season = self
            .seasons
            .at(place.latitude, place.height_m, place.sunward);
        let up = point.normalize_or(Vec3::Y);
        let slope = libm::acosf(normal.dot(up).clamp(-1.0, 1.0)).to_degrees();
        woods::stand(&season, slope, footing)
    }

    /// How deep the snow lies at a point on the ground, facing `normal`, in metres.
    pub fn snow_m(&self, point: Vec3, normal: Vec3) -> f32 {
        let place = self.land.place(point, normal);
        self.snow
            .depth_m(place.latitude, place.height_m, place.sunward)
    }
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
    }
}
