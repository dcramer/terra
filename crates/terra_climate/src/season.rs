//! A place's growing season, as its climate's averages have it: what decides where trees can
//! stand (D313, D321). The tree line the world over sits where the growing season's mean
//! temperature is about 6.4 °C and it lasts at least 94 days, a growing day being one whose
//! mean is above 0.9 °C: Paulsen and Körner's global tree line model (2014, *Alpine Botany*
//! 124), from 46 tree lines from 68° N to 42° S (Körner and Paulsen 2004). It is the warmth
//! a tree's new tissue needs to form, not the cold of winter, that stops trees climbing.

use crate::{Fields, Place};

/// A day is part of the growing season when its mean is above this, in degrees Celsius.
pub const GROWING_DAY_C: f32 = 0.9;
/// The tree line's growing season, as Paulsen and Körner find it the world over: its mean
/// temperature, in degrees Celsius, and how many days it must last at least.
pub const TREE_LINE_SEASON_C: f32 = 6.4;
pub const TREE_LINE_DAYS: f32 = 94.0;

/// A place's year, by its climate's averages: no weather, the same every year.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Season {
    /// How many days of the year are growing days, and their mean temperature, in degrees
    /// Celsius (the year's mean where there are none).
    pub growing_days: f32,
    pub growing_mean_c: f32,
    /// The year's mean, and its coldest and warmest day's.
    pub year_mean_c: f32,
    pub coldest_c: f32,
    pub warmest_c: f32,
}

impl Season {
    /// Whether trees can grow here: a growing season warm and long enough.
    pub fn holds_trees(&self) -> bool {
        self.growing_mean_c >= TREE_LINE_SEASON_C && self.growing_days >= TREE_LINE_DAYS
    }
}

impl Fields {
    /// A place's growing season over a year of its days, from the averages alone, so it is
    /// the same whichever year is asked.
    pub fn season(&self, place: Place) -> Season {
        self.seasons_up(place, &[place.height_m])[0]
    }

    /// The growing seasons at a place's latitude and leaning, at each of so many heights over
    /// the sea, as [`Fields::season`] has each: a day's sun on the ground is worked out once
    /// for all of them, since only the lapse changes with height.
    pub fn seasons_up(&self, place: Place, heights_m: &[f32]) -> Vec<Season> {
        let days = self.climate().year.days.round() as i64;
        let mut years = vec![Tally::new(); heights_m.len()];
        for day in 0..days {
            let today = self.day(place, day);
            let slope_c = today.slope_c();
            let (warmth_c, annual_c) = self.level_warmth(place.latitude, today.phase);
            for (tally, &height_m) in years.iter_mut().zip(heights_m) {
                tally.add(self.base_c(warmth_c, height_m, annual_c) + slope_c);
            }
        }
        years.iter().map(|tally| tally.season(days)).collect()
    }
}

/// A year's days' means, counted as they come.
#[derive(Clone, Copy)]
struct Tally {
    growing: f32,
    warmth: f32,
    sum: f32,
    coldest: f32,
    warmest: f32,
}

impl Tally {
    fn new() -> Self {
        Self {
            growing: 0.0,
            warmth: 0.0,
            sum: 0.0,
            coldest: f32::MAX,
            warmest: f32::MIN,
        }
    }

    fn add(&mut self, mean: f32) {
        self.sum += mean;
        self.coldest = self.coldest.min(mean);
        self.warmest = self.warmest.max(mean);
        if mean > GROWING_DAY_C {
            self.growing += 1.0;
            self.warmth += mean;
        }
    }

    fn season(&self, days: i64) -> Season {
        let year_mean_c = self.sum / days as f32;
        Season {
            growing_days: self.growing,
            growing_mean_c: if self.growing > 0.0 {
                self.warmth / self.growing
            } else {
                year_mean_c
            },
            year_mean_c,
            coldest_c: self.coldest,
            warmest_c: self.warmest,
        }
    }
}

impl Fields {
    /// How high over the sea the tree line stands at a latitude, on ground leaning so far
    /// toward the equator: the lowest height whose season no longer holds trees, found by
    /// halving, within a metre. `None` where trees grow at any height up to `highest_m`, or
    /// at none from the sea up.
    pub fn tree_line_m(&self, latitude: f32, sunward: f32, highest_m: f32) -> Option<f32> {
        let holds = |height_m: f32| {
            self.season(Place {
                at: glam::Vec3::ZERO,
                height_m,
                latitude,
                sunward,
                catch: 1.0,
            })
            .holds_trees()
        };
        if !holds(0.0) || holds(highest_m) {
            return None;
        }
        let (mut low, mut high) = (0.0, highest_m);
        while high - low > 1.0 {
            let middle = (low + high) * 0.5;
            if holds(middle) {
                low = middle;
            } else {
                high = middle;
            }
        }
        Some((low + high) * 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::tests::temperate;
    use glam::Vec3;
    use terra_gen::Seed;

    fn at(height_m: f32, latitude_deg: f32, sunward: f32) -> Place {
        Place {
            at: Vec3::ZERO,
            height_m,
            latitude: latitude_deg.to_radians(),
            sunward,
            catch: 1.0,
        }
    }

    #[test]
    fn the_season_is_shorter_and_colder_higher_and_further_from_the_equator() {
        let fields = Fields::new(temperate(), Seed(1));
        let low = fields.season(at(0.0, 40.0, 0.0));
        let high = fields.season(at(1500.0, 40.0, 0.0));
        let north = fields.season(at(0.0, 60.0, 0.0));
        for colder in [high, north] {
            assert!(colder.growing_days <= low.growing_days);
            assert!(colder.year_mean_c < low.year_mean_c);
        }
        // The year's mean falls by the lapse rate, whatever the season does.
        assert!((low.year_mean_c - high.year_mean_c - 1.5 * 6.5).abs() < 0.05);
        assert!(low.coldest_c < low.year_mean_c && low.year_mean_c < low.warmest_c);
    }

    /// The season as it was counted before `seasons_up`: each day's mean at the place itself.
    fn counted_day_by_day(fields: &Fields, place: Place) -> Season {
        let days = fields.climate().year.days.round() as i64;
        let mut tally = Tally::new();
        for day in 0..days {
            tally.add(fields.day(place, day).mean_c());
        }
        tally.season(days)
    }

    #[test]
    fn the_seasons_up_a_place_are_each_heights_own_to_the_bit() {
        let fields = Fields::new(temperate(), Seed(1));
        let heights: Vec<f32> = (0..40).map(|j| -50.0 + 97.5 * j as f32).collect();
        for latitude_deg in [-71.3, -15.34, 0.0, 23.9, 48.2, 66.6] {
            for sunward in [-0.9, -0.3, 0.0, 0.45, 0.9] {
                let place = at(0.0, latitude_deg, sunward);
                let up = fields.seasons_up(place, &heights);
                for (season, &height_m) in up.iter().zip(&heights) {
                    let own = counted_day_by_day(&fields, Place { height_m, ..place });
                    let bits = |s: &Season| {
                        [
                            s.growing_days,
                            s.growing_mean_c,
                            s.year_mean_c,
                            s.coldest_c,
                            s.warmest_c,
                        ]
                        .map(f32::to_bits)
                    };
                    assert_eq!(
                        bits(season),
                        bits(&own),
                        "{latitude_deg} {sunward} {height_m}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_slope_toward_the_sun_is_warmer_and_one_away_colder() {
        let fields = Fields::new(temperate(), Seed(1));
        let level = fields.season(at(800.0, 45.0, 0.0)).year_mean_c;
        let sunny = fields.season(at(800.0, 45.0, 0.5)).year_mean_c;
        let shaded = fields.season(at(800.0, 45.0, -0.5)).year_mean_c;
        assert!(
            sunny > level + 0.2 && shaded < level - 0.2,
            "{shaded} {level} {sunny}"
        );
        // A few degrees at most: the ground's own warmth, not another climate.
        assert!(sunny - shaded < 4.0);
    }

    #[test]
    fn the_tree_line_is_where_the_season_stops_holding_trees() {
        let fields = Fields::new(temperate(), Seed(1));
        let line = fields
            .tree_line_m(40.0_f32.to_radians(), 0.0, 6000.0)
            .unwrap();
        // Counted another way: step up a metre at a time to the first height without trees.
        let stepped = (0..6000)
            .map(|m| m as f32)
            .find(|&m| !fields.season(at(m, 40.0, 0.0)).holds_trees())
            .unwrap();
        assert!((line - stepped).abs() < 2.0, "{line} against {stepped}");
        // Lower on a slope facing away from the sun, and lower toward the pole. A slope facing
        // the sun at the middle latitudes gains little in the summer, when the sun is high
        // anyway, and more in spring and autumn, which lengthens its season with cool days; so
        // its line is nearly level ground's, and well above the shaded slope's.
        let sunny = fields
            .tree_line_m(40.0_f32.to_radians(), 0.5, 6000.0)
            .unwrap();
        let shaded = fields
            .tree_line_m(40.0_f32.to_radians(), -0.5, 6000.0)
            .unwrap();
        let north = fields
            .tree_line_m(55.0_f32.to_radians(), 0.0, 6000.0)
            .unwrap();
        assert!(
            shaded < line - 50.0 && sunny > shaded + 50.0,
            "{shaded} {line} {sunny}"
        );
        assert!(north < line, "{north} {line}");
    }
}
