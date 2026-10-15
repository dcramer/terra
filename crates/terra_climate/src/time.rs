//! The world's clock and where it stands in the year.

use serde::{Deserialize, Serialize};

/// Hours in a day of the world's clock, whatever a day lasts in seconds of play (D134).
pub const HOURS: f64 = 24.0;

/// A moment on a world's clock: days since the clock started, the time of day the fraction.
///
/// This is all the server sends of the climate (D52): every machine works out the same
/// weather, snow and warmth from it and the world's seed. Days before the clock started
/// are negative; the world has a past from before anyone played.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct WorldTime(pub f64);

impl WorldTime {
    /// An hour, 0 to 24, of a day.
    pub fn at(day: i64, hour: f64) -> Self {
        Self(day as f64 + hour / HOURS)
    }

    /// Which day it is.
    pub fn day(self) -> i64 {
        self.0.floor() as i64
    }

    /// The hour of the day, 0 to 24.
    pub fn hour(self) -> f64 {
        (self.0 - self.0.floor()) * HOURS
    }
}

/// A world's year (D149): how long it is, how far its sun's path swings, and where in it
/// the clock started. A share of the year is counted from midwinter, the shortest day:
/// 0 is midwinter, 0.25 the spring equinox, 0.5 midsummer, 0.75 the autumn equinox.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Year {
    /// How many days a year lasts.
    pub days: f64,
    /// How far the world's axis leans, in degrees, which is how far north and south of
    /// the equator the sun's path swings over the year: Earth's 23.44.
    pub tilt_deg: f32,
    /// Where in the year the clock's day 0 began, as a share of the year after midwinter.
    pub start: f64,
}

impl Year {
    /// How many years have passed, at so many days on the clock, since the midwinter
    /// before the clock started. Its whole part counts midwinters, its fraction is where
    /// in the year it is.
    pub fn cycle(&self, days: f64) -> f64 {
        self.start + days / self.days
    }

    /// Where in the year a day is, as a share after midwinter, taken at its middle: the
    /// year's slow terms (the sun's path, the season's warmth) move on once a day.
    pub fn phase(&self, day: i64) -> f32 {
        self.cycle(day as f64 + 0.5).rem_euclid(1.0) as f32
    }

    /// The count of midwinters a day's middle is past: warmth is counted from each.
    pub fn winters(&self, day: i64) -> i64 {
        self.cycle(day as f64 + 0.5).floor() as i64
    }

    /// The count of midsummers a day's middle is past: chill is counted from each.
    pub fn summers(&self, day: i64) -> i64 {
        (self.cycle(day as f64 + 0.5) - 0.5).floor() as i64
    }

    /// The first day whose middle is past the given midsummer.
    pub fn first_day_of_summer(&self, summers: i64) -> i64 {
        let guess = ((summers as f64 + 0.5 - self.start) * self.days - 0.5).ceil() as i64;
        let mut day = guess;
        while self.summers(day - 1) >= summers {
            day -= 1;
        }
        while self.summers(day) < summers {
            day += 1;
        }
        day
    }
}

/// A date on a world's clock, as a person gives one (the sandbox's and the field maps' `--date`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Date {
    /// A day of the clock, counted from when it started.
    Day(i64),
    /// A share of the year after midwinter, from a day of an Earth calendar.
    OfYear(f64),
}

/// Days of an Earth calendar's months, and how many days from the 1st of January midwinter
/// (the 21st of December) comes.
const MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const MIDWINTER_DAY: u32 = 354;

impl Date {
    pub fn parse(text: &str) -> Result<Self, String> {
        let wrong = || {
            format!(
                "--date wants a month and day such as 10-15, or a day of the clock such as 40, not `{text}`"
            )
        };
        if let Ok(day) = text.parse::<i64>() {
            return Ok(Date::Day(day));
        }
        let (month, day) = text.split_once('-').ok_or_else(wrong)?;
        let (month, day) = (
            month.parse::<usize>().map_err(|_| wrong())?,
            day.parse::<u32>().map_err(|_| wrong())?,
        );
        if !(1..=12).contains(&month) || day == 0 || day > MONTH_DAYS[month - 1] {
            return Err(wrong());
        }
        let of_year = MONTH_DAYS[..month - 1].iter().sum::<u32>() + day - 1;
        let since = (of_year + 365 - MIDWINTER_DAY) % 365;
        // The calendar day's middle, as the clock takes a day's.
        Ok(Date::OfYear((since as f64 + 0.5) / 365.0))
    }

    /// The day of the clock it is, in a world of this year: the first from the clock's start
    /// whose middle is that far through the year.
    pub fn day(self, year: &Year) -> i64 {
        match self {
            Date::Day(day) => day,
            Date::OfYear(share) => {
                let ahead = (share - year.start).rem_euclid(1.0) * year.days - 0.5;
                (ahead.round() as i64).rem_euclid(year.days.round() as i64)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_reads_back_its_day_and_hour() {
        for (day, hour) in [(0, 7.5), (12, 0.0), (-3, 23.0), (400, 12.25)] {
            let time = WorldTime::at(day, hour);
            assert_eq!(time.day(), day);
            assert!((time.hour() - hour).abs() < 1e-9, "{day} {hour}");
        }
    }

    #[test]
    fn the_year_turns_once_in_its_days_from_where_it_started() {
        let year = Year {
            days: 100.0,
            tilt_deg: 20.0,
            start: 0.3,
        };
        // Day 0's middle is half a day on from the start.
        assert!((year.phase(0) - 0.305).abs() < 1e-6);
        assert!((year.phase(100) - year.phase(0)).abs() < 1e-6);
        assert!((year.phase(50) - 0.805).abs() < 1e-6);
        // Midwinter comes 70 days in, midsummer 20 days in.
        assert_eq!(year.winters(69), 0);
        assert_eq!(year.winters(70), 1);
        assert_eq!(year.summers(19), -1);
        assert_eq!(year.summers(20), 0);
        assert_eq!(year.first_day_of_summer(0), 20);
        assert_eq!(year.first_day_of_summer(1), 120);
        assert_eq!(year.first_day_of_summer(-1), -80);
    }
}
