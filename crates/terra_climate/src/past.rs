//! What the weather of the days before leaves at a place: snow on the ground, water in it,
//! ice on water, and the warmth and chill plants count (climate.md, D314). Each is folded
//! over the days before the asked one, per coarse cell, worked out when asked and kept by
//! the day in a small cache, never saved.
//!
//! **How far back.** A fold starts at the first midsummer at least half a year before the
//! asked day, so it covers 0.5 to 1.5 years (183 to 548 days of a 365-day year):
//! - warmth is counted from midwinter and chill from midsummer, and the fold holds both
//!   resets, so they are exact;
//! - snow and ice are taken as none at midsummer, true wherever summer clears them; snow
//!   that outlasts summers is a glacier, for the world's generation to place;
//! - the ground's water forgets where it started within weeks, once rain fills it or
//!   drought empties it, and the fold runs at least half a year
//!   (`the_grounds_start_is_forgotten`).
//!
//! Stepping a kept day on to the next gives exactly the fold's answer, except across the
//! midwinter where the fold's start moves on a year; the cache folds afresh there.
//!
//! **What it costs.** A day is eight samples of the weather, three hours apart, of three
//! noise samples and a few sines each: about 1.8 µs. A cell folded afresh, 362 days on
//! average, takes about 0.65 ms; a kept cell's next day, 1.8 µs. Measured on 2026-10-14,
//! release build, one core of a Ryzen 9 5900X.

use crate::fields::{Fields, Place};
use glam::Vec3;
use std::collections::HashMap;

/// How many times a day the weather is sampled, evenly: often enough to follow the day's
/// warmth and a passing front.
const SAMPLES: usize = 8;
/// Warmth is counted above this daily mean, in degree-days: the base of the thermal
/// growing season. Chill is counted below the other. A species' year is said in these.
pub const WARMTH_BASE_C: f32 = 5.0;
pub const CHILL_BASE_C: f32 = 10.0;
/// Melt by the enhanced temperature index (Hock 1999): so many millimetres of water an hour
/// for each degree above freezing, and more for the sun's light on it.
const MELT_MM_PER_HOUR_C: f32 = 0.075;
const MELT_PER_W_M2: f32 = 0.0005;
/// Lying snow's bulk density, settled, in kilograms a cubic metre.
const SNOW_DENSITY: f32 = 250.0;
/// How much water the topsoil holds, in millimetres, and how full it is where a fold starts.
pub const GROUND_HOLDS_MM: f32 = 60.0;
const GROUND_START: f32 = 0.5;
/// Ice on still water grows by Stefan's law, as the square root of the frost degree-days,
/// so many metres for each root degree-day: 2.7 cm, a lake's usual. It thins so many metres
/// a degree-day of thaw.
const STEFAN_M: f32 = 0.027;
const THAW_M: f32 = 0.008;
/// The side of a cell of the past, in metres: tens of metres, as climate.md asks. Snow,
/// wetness and warmth change over hundreds of metres with height and the fronts.
pub const CELL_M: f32 = 32.0;

/// What the past has left at a place, at the start of a day.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Past {
    /// The water in the snow lying on the ground, in millimetres.
    pub snow_mm: f32,
    /// The water in the topsoil, in millimetres, up to [`GROUND_HOLDS_MM`].
    pub ground_mm: f32,
    /// How thick ice on still water would be, in metres.
    pub ice_m: f32,
    /// Degree-days of daily mean above [`WARMTH_BASE_C`] since midwinter.
    pub warmth: f32,
    /// Degree-days of daily mean below [`CHILL_BASE_C`] since midsummer.
    pub chill: f32,
    /// How many days each count has run since it started again: which began more lately
    /// tells a plant whether the leaves it dropped to the chill are still to come back to
    /// the warmth (`plants`).
    pub warmth_days: f32,
    pub chill_days: f32,
}

impl Past {
    /// How deep the snow lies, in metres.
    pub fn snow_depth_m(&self) -> f32 {
        self.snow_mm / SNOW_DENSITY
    }

    /// How wet the ground is, 0 dry to 1 as wet as it holds.
    pub fn ground_wetness(&self) -> f32 {
        self.ground_mm / GROUND_HOLDS_MM
    }

    /// Whether warmth has been counted since chill was: the part of the year after
    /// midwinter, when what the chill took is still to come back.
    pub fn warming(&self) -> bool {
        self.warmth_days < self.chill_days
    }

    /// The past part of the way from one day's start to the next's: what lies on the ground
    /// and is in it changes through the day rather than at midnight. The counts are the
    /// earlier day's, as plants keep them by the day.
    pub fn between(&self, next: &Past, t: f32) -> Past {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        Past {
            snow_mm: lerp(self.snow_mm, next.snow_mm),
            ground_mm: lerp(self.ground_mm, next.ground_mm),
            ice_m: lerp(self.ice_m, next.ice_m),
            ..*self
        }
    }

    fn fresh() -> Self {
        Self {
            ground_mm: GROUND_HOLDS_MM * GROUND_START,
            ..Self::default()
        }
    }
}

/// The first day a fold for the start of `day` begins on: the first midsummer at least
/// half a year before.
pub fn fold_start(fields: &Fields, day: i64) -> i64 {
    let year = &fields.climate().year;
    let last = day - 1;
    let summers = year.summers(last);
    // Past midwinter since that midsummer, the fold can start there; before it, a year on.
    let since = if year.winters(last) > summers {
        summers
    } else {
        summers - 1
    };
    year.first_day_of_summer(since)
}

/// What the past has left at a place at the start of a day, folded over the days before.
pub fn past(fields: &Fields, place: Place, day: i64) -> Past {
    let start = fold_start(fields, day);
    fold_from(fields, place, Past::fresh(), start, day)
}

/// Folds the days from `from` up to `to` (not including it) into a past.
fn fold_from(fields: &Fields, place: Place, mut past: Past, from: i64, to: i64) -> Past {
    for day in from..to {
        past = step(fields, place, past, day);
    }
    past
}

/// The past at the start of the day after `day`, from the past at its start.
pub fn step(fields: &Fields, place: Place, mut past: Past, day: i64) -> Past {
    let year = &fields.climate().year;
    if year.winters(day) != year.winters(day - 1) {
        past.warmth = 0.0;
        past.warmth_days = 0.0;
    }
    if year.summers(day) != year.summers(day - 1) {
        past.chill = 0.0;
        past.chill_days = 0.0;
    }
    let today = fields.day(place, day);
    let hours = 24.0 / SAMPLES as f32;
    let mut sum = 0.0;
    for sample in 0..SAMPLES {
        let weather = today.weather((sample as f32 + 0.5) * hours);
        let t = weather.temperature_c;
        let falls = weather.precipitation_mm_per_hour * hours;
        let snow = falls * weather.snow_share;
        past.snow_mm += snow;
        let melt = if t > 0.0 {
            ((MELT_MM_PER_HOUR_C + MELT_PER_W_M2 * weather.ground_sunlight_w_m2) * t * hours)
                .min(past.snow_mm)
        } else {
            0.0
        };
        past.snow_mm -= melt;
        past.ground_mm = (past.ground_mm + falls - snow + melt).min(GROUND_HOLDS_MM);
        sum += t;
    }
    let mean = sum / SAMPLES as f32;
    // Bare ground above freezing dries; under snow, or frozen, it keeps its water.
    if past.snow_mm <= 0.0 && mean > 0.0 {
        past.ground_mm = (past.ground_mm - drying_mm(mean, today.daylight.day_hours)).max(0.0);
    }
    past.ice_m = if mean < 0.0 {
        (past.ice_m * past.ice_m + STEFAN_M * STEFAN_M * -mean).sqrt()
    } else {
        (past.ice_m - THAW_M * mean).max(0.0)
    };
    past.warmth += (mean - WARMTH_BASE_C).max(0.0);
    past.chill += (CHILL_BASE_C - mean).max(0.0);
    past.warmth_days += 1.0;
    past.chill_days += 1.0;
    past
}

/// How much water the ground loses to the air in a day, in millimetres, at a daily mean
/// temperature with the sun up so many hours: Hamon's potential evapotranspiration, as the
/// US Geological Survey's water balance models take it (Lu et al. 2005).
fn drying_mm(mean_c: f32, day_hours: f32) -> f32 {
    let saturated_hpa = 6.108 * libm::expf(17.27 * mean_c / (mean_c + 237.3));
    let vapour_g_m3 = 216.7 * saturated_hpa / (mean_c + 273.3);
    0.1651 * (day_hours / 12.0) * vapour_g_m3 * 1.2
}

/// A cell of the past: a cube [`CELL_M`] on a side in the world's frame, which the
/// ground's surface passes through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Cell {
    /// The cell a point in the world's frame is in.
    pub fn containing(at: Vec3) -> Self {
        let cell = (at / CELL_M).floor();
        Self {
            x: cell.x as i32,
            y: cell.y as i32,
            z: cell.z as i32,
        }
    }

    /// Its middle, in the world's frame.
    pub fn middle(self) -> Vec3 {
        (Vec3::new(self.x as f32, self.y as f32, self.z as f32) + 0.5) * CELL_M
    }
}

/// The past of the cells asked about lately, each kept as of the last day asked, so the next
/// day costs a day's fold rather than a year's. Never saved: it is worked out again.
#[derive(Clone, Debug)]
pub struct PastCache {
    kept: HashMap<Cell, Kept>,
    holds: usize,
    used: u64,
}

#[derive(Clone, Copy, Debug)]
struct Kept {
    place: Place,
    start: i64,
    day: i64,
    past: Past,
    used: u64,
}

impl PastCache {
    /// A cache keeping up to so many cells; past that, it forgets the half least lately asked.
    pub fn new(holds: usize) -> Self {
        Self {
            kept: HashMap::new(),
            holds: holds.max(2),
            used: 0,
        }
    }

    /// How many cells it keeps now.
    pub fn len(&self) -> usize {
        self.kept.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kept.is_empty()
    }

    /// The past of a cell at the start of a day. `place` is the cell's own, its middle on
    /// the ground, the same each time it is asked.
    pub fn past(&mut self, fields: &Fields, cell: Cell, place: Place, day: i64) -> Past {
        self.used += 1;
        let start = fold_start(fields, day);
        let past = match self.kept.get(&cell) {
            Some(kept) if kept.place == place && kept.start == start && kept.day <= day => {
                fold_from(fields, place, kept.past, kept.day, day)
            }
            _ => fold_from(fields, place, Past::fresh(), start, day),
        };
        if !self.kept.contains_key(&cell) && self.kept.len() >= self.holds {
            self.forget();
        }
        self.kept.insert(
            cell,
            Kept {
                place,
                start,
                day,
                past,
                used: self.used,
            },
        );
        past
    }

    fn forget(&mut self) {
        let mut used: Vec<u64> = self.kept.values().map(|kept| kept.used).collect();
        used.sort_unstable();
        let keep_from = used[used.len() / 2];
        self.kept.retain(|_, kept| kept.used >= keep_from);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::tests::{place, temperate};
    use crate::{Climate, Fields};
    use terra_gen::Seed;

    /// A world with no weather and no day, so the temperature is the year's alone, swinging
    /// so far either side of its mean, and rain or snow falling so hard all the time.
    fn plain(mean_c: f32, swing_c: f32, falls_mm_per_hour: f32) -> Fields {
        let mut climate: Climate = temperate();
        let t = &mut climate.temperature;
        (t.equator_c, t.pole_c) = (mean_c, mean_c);
        (t.annual_swing_c, t.annual_lag_days) = (swing_c, 0.0);
        (t.daily_swing_c, t.weather_swing_c) = (0.0, 0.0);
        climate.precipitation.share = if falls_mm_per_hour > 0.0 { 1.0 } else { 0.0 };
        climate.precipitation.heaviest_mm_per_hour = falls_mm_per_hour;
        climate.precipitation.annual_swing = 0.0;
        climate.year.start = 0.0;
        Fields::new(climate, Seed(1))
    }

    /// At 45°, where the annual swing is the climate's own.
    fn at() -> Place {
        Place {
            at: Vec3::new(10.0, 2000.0, 30.0),
            height_m: 0.0,
            latitude: 45.0_f32.to_radians(),
            sunward: 0.0,
        }
    }

    #[test]
    fn snow_builds_through_a_cold_spell_and_melts_after() {
        // From -15 °C at midwinter to +15 °C at midsummer, something falling all the time.
        let fields = plain(0.0, 15.0, 0.1);
        let year: Vec<Past> = (0..=365).map(|day| past(&fields, at(), day)).collect();
        let cold = |day: usize| fields.temperature(at(), crate::WorldTime::at(day as i64, 12.0));
        let mut deepest = 0.0_f32;
        for day in 1..=365 {
            let (before, now) = (year[day - 1].snow_mm, year[day].snow_mm);
            if cold(day - 1) < 0.0 {
                assert!(
                    now > before,
                    "day {day}: snow {before} to {now} in the cold"
                );
            } else if cold(day - 1) > crate::fields::ALL_RAIN_C && before > 0.0 {
                assert!(
                    now < before,
                    "day {day}: snow {before} to {now} in the warmth"
                );
            }
            deepest = deepest.max(now);
        }
        assert!(deepest > 100.0, "{deepest}");
        assert_eq!(year[182].snow_mm, 0.0, "gone by midsummer");
        // A world that is never cold never has snow.
        let warm = plain(15.0, 0.0, 2.0);
        assert_eq!(past(&warm, at(), 30).snow_mm, 0.0);
    }

    #[test]
    fn warmth_counts_only_above_its_base_and_starts_again_at_midwinter() {
        let warm = plain(WARMTH_BASE_C + 3.0, 0.0, 0.0);
        for day in [1, 40, 200, 364] {
            let warmth = past(&warm, at(), day).warmth;
            assert!((warmth - 3.0 * day as f32).abs() < 1e-2, "{day}: {warmth}");
        }
        // Midwinter is between day 364 and day 365, whose middle is past it.
        assert!((past(&warm, at(), 366).warmth - 3.0).abs() < 1e-3);
        let cold = plain(WARMTH_BASE_C - 3.0, 0.0, 0.0);
        assert_eq!(past(&cold, at(), 200).warmth, 0.0);
    }

    #[test]
    fn chill_counts_only_below_its_base_and_starts_again_at_midsummer() {
        let cool = plain(CHILL_BASE_C - 4.0, 0.0, 0.0);
        // Midsummer's day is day 182, whose middle is past it.
        assert!((past(&cool, at(), 183).chill - 4.0).abs() < 1e-3);
        assert!((past(&cool, at(), 182 + 100).chill - 400.0).abs() < 1e-2);
        let warm = plain(CHILL_BASE_C + 4.0, 0.0, 0.0);
        assert_eq!(past(&warm, at(), 300).chill, 0.0);
    }

    #[test]
    fn ice_grows_by_stefans_law_and_thaws() {
        // Known in closed form: after n days at -10 °C, 2.7 cm for each root degree-day.
        let frozen = plain(-10.0, 0.0, 0.0);
        let at_day = |day| past(&frozen, at(), day);
        let start = fold_start(&frozen, 300);
        let days = (300 - start) as f32;
        let stefan = STEFAN_M * (10.0 * days).sqrt();
        assert!(
            (at_day(300).ice_m - stefan).abs() < 1e-3,
            "{}",
            at_day(300).ice_m
        );
        let mild = plain(4.0, 0.0, 0.0);
        assert_eq!(past(&mild, at(), 300).ice_m, 0.0);
    }

    #[test]
    fn rain_wets_the_ground_and_dry_warmth_dries_it() {
        let wet = plain(15.0, 0.0, 2.0);
        assert!(past(&wet, at(), 200).ground_wetness() > 0.9);
        let dry = plain(20.0, 0.0, 0.0);
        assert_eq!(past(&dry, at(), 200).ground_wetness(), 0.0);
        // Frozen, under no rain, it keeps what it had.
        let frozen = plain(-5.0, 0.0, 0.0);
        assert_eq!(past(&frozen, at(), 200).ground_mm, Past::fresh().ground_mm);
    }

    #[test]
    fn the_grounds_start_is_forgotten() {
        // Over the shortest fold, from a midsummer to the midwinter after, a ground that
        // started empty or full ends the same.
        let fields = Fields::new(temperate(), Seed(4));
        let year = fields.climate().year;
        let mut rng = Seed(2).rng();
        for _ in 0..40 {
            let place = place(&mut rng);
            let from = year.first_day_of_summer(rng.below(6) as i64 - 3);
            let day = (from..)
                .find(|day| fold_start(&fields, *day) == from)
                .unwrap();
            assert!(day - from < 185, "{}", day - from);
            let ends = |ground_mm| {
                let start = Past {
                    ground_mm,
                    ..Past::default()
                };
                fold_from(&fields, place, start, from, day).ground_mm
            };
            assert!(
                (ends(0.0) - ends(GROUND_HOLDS_MM)).abs() < 1.0,
                "{place:?} {day}"
            );
        }
    }

    #[test]
    fn stepping_day_by_day_is_the_fold() {
        let fields = Fields::new(temperate(), Seed(4));
        let mut rng = Seed(9).rng();
        let place = place(&mut rng);
        let mut cache = PastCache::new(8);
        let cell = Cell::containing(place.at);
        let mut stepped = 0;
        for day in -400..200 {
            let fresh = past(&fields, place, day);
            assert_eq!(cache.past(&fields, cell, place, day), fresh, "day {day}");
            if fold_start(&fields, day) == fold_start(&fields, day - 1) {
                let before = past(&fields, place, day - 1);
                assert_eq!(step(&fields, place, before, day - 1), fresh, "day {day}");
                stepped += 1;
            }
        }
        assert!(stepped > 590, "{stepped}");
    }

    #[test]
    fn the_cache_forgets_the_least_lately_asked() {
        let fields = Fields::new(temperate(), Seed(4));
        let mut cache = PastCache::new(4);
        let places: Vec<(Cell, Place)> = (0..6)
            .map(|k| {
                let p = Place {
                    at: Vec3::new(k as f32 * 100.0, 2000.0, 0.0),
                    height_m: 0.0,
                    latitude: 0.7,
                    sunward: 0.0,
                };
                (Cell::containing(p.at), p)
            })
            .collect();
        for (cell, place) in &places {
            let got = cache.past(&fields, *cell, *place, 10);
            assert_eq!(got, past(&fields, *place, 10));
            assert!(cache.len() <= 4);
        }
        assert!(cache.kept.contains_key(&places[5].0));
    }

    #[test]
    fn a_cell_holds_its_points() {
        let cell = Cell::containing(Vec3::new(-1.0, 33.0, 64.0));
        assert_eq!((cell.x, cell.y, cell.z), (-1, 1, 2));
        assert_eq!(Cell::containing(cell.middle()), cell);
    }

    #[test]
    fn warming_is_the_part_of_the_year_after_midwinter() {
        // The plain world's year starts at midwinter: day 0's middle is just past it.
        let fields = plain(5.0, 10.0, 0.0);
        for (day, after_midwinter) in [(10, true), (150, true), (190, false), (360, false)] {
            assert_eq!(
                past(&fields, at(), day).warming(),
                after_midwinter,
                "day {day}"
            );
        }
    }

    #[test]
    fn a_day_goes_from_its_start_to_the_next_days() {
        let (a, b) = (
            Past {
                snow_mm: 10.0,
                ground_mm: 20.0,
                warmth: 5.0,
                ..Past::default()
            },
            Past {
                snow_mm: 30.0,
                ground_mm: 0.0,
                warmth: 9.0,
                ..Past::default()
            },
        );
        assert_eq!(a.between(&b, 0.0), a);
        let half = a.between(&b, 0.5);
        assert_eq!((half.snow_mm, half.ground_mm), (20.0, 10.0));
        assert_eq!(half.warmth, 5.0, "plants count by the day");
        assert_eq!(a.between(&b, 1.0).snow_mm, 30.0);
    }
}
