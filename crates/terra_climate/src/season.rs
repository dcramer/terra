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
        let days = self.climate().year.days.round() as i64;
        let (mut growing, mut warmth, mut sum) = (0.0, 0.0, 0.0);
        let (mut coldest, mut warmest) = (f32::MAX, f32::MIN);
        for day in 0..days {
            let mean = self.day(place, day).mean_c();
            sum += mean;
            coldest = coldest.min(mean);
            warmest = warmest.max(mean);
            if mean > GROWING_DAY_C {
                growing += 1.0;
                warmth += mean;
            }
        }
        let year_mean_c = sum / days as f32;
        Season {
            growing_days: growing,
            growing_mean_c: if growing > 0.0 {
                warmth / growing
            } else {
                year_mean_c
            },
            year_mean_c,
            coldest_c: coldest,
            warmest_c: warmest,
        }
    }

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
