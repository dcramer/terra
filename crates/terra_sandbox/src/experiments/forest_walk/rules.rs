//! What the forest walk's keys do, as plain functions of the clock, the year and the climate,
//! so each is tested without a window. Nothing here makes weather or seasons: each key moves
//! the world's own clock to a moment that has what was asked, as `--date` and `--weather` do.

use terra_climate::{Date, Year};

/// The dates a season key flips between: high summer and a January snowy spell in the lush
/// forest (research/2026-10-15-seasons.md: half a metre of snow on 15 January).
pub const SUMMER: &str = "07-20";
pub const WINTER: &str = "01-15";

/// The season a day falls in, by the months as forecasts count them (December to February
/// winter, and so on), from the year's share after midwinter: a label to show, never read by a
/// system (docs/climate.md).
pub fn season(year: &Year, day: i64) -> &'static str {
    match year.phase(day) {
        p if !(0.1918..0.9452).contains(&p) => "winter",
        p if p < 0.4438 => "spring",
        p if p < 0.6959 => "summer",
        _ => "autumn",
    }
}

/// The day the season key goes to from a day: winter from summer or spring, summer otherwise.
pub fn flipped(year: &Year, day: i64) -> i64 {
    let to = match season(year, day) {
        "summer" | "spring" => WINTER,
        _ => SUMMER,
    };
    Date::parse(to).map_or(day, |date| date.day(year))
}

/// The times of day the time key steps through, with the hour each is on a day whose sun's
/// declination is so (radians): dawn with the sun 4° up in the morning, noon, dusk with it
/// 2° up in the evening, night at 23:00.
pub fn times_of_day(latitude: f32, declination: f32) -> [(&'static str, f32); 4] {
    let (dawn, dusk) = sun_at(latitude, declination, 4f32.to_radians())
        .map_or((6.0, 18.0), |h| (12.0 - h, 12.0 + h));
    let dusk = sun_at(latitude, declination, 2f32.to_radians()).map_or(dusk, |h| 12.0 + h);
    [
        ("dawn", dawn),
        ("noon", 12.0),
        ("dusk", dusk),
        ("night", 23.0),
    ]
}

/// Hours from noon to when the sun stands so high (radians), on a day of so much declination,
/// by the sun's height through the day (cos H from sin h = sin φ sin δ + cos φ cos δ cos H); none
/// when it never comes that low or that high.
fn sun_at(latitude: f32, declination: f32, height: f32) -> Option<f32> {
    let across = libm::cosf(latitude) * libm::cosf(declination);
    let cos = (libm::sinf(height) - libm::sinf(latitude) * libm::sinf(declination)) / across;
    (-1.0..=1.0)
        .contains(&cos)
        .then(|| libm::acosf(cos).to_degrees() / 15.0)
}

/// The next time of day after an hour, on the same day: after night, dawn again.
pub fn next_time_of_day(times: &[(&'static str, f32); 4], hour: f32) -> (&'static str, f32) {
    times
        .iter()
        .copied()
        .find(|(_, at)| *at > hour + 0.05)
        .unwrap_or(times[0])
}

/// What a weather is called: the last of the named weathers it is, in their file's order, as
/// its grade is taken (`weather::grade_of`); with none, clear under a mostly open sky with
/// nothing falling, otherwise cloudy.
pub fn called(named: &[String], is: impl Fn(usize) -> bool, cloud: f32, falling: f32) -> String {
    match (0..named.len()).rev().find(|&i| is(i)) {
        Some(i) => named[i].clone(),
        None if cloud < 0.5 && falling < 0.05 => CLEAR.into(),
        None => "cloudy".into(),
    }
}

pub const CLEAR: &str = "clear";

/// How many days either way the weather key looks: about a month, so it stays in the season.
pub const LOOKS_DAYS: i64 = 30;

/// The days the weather key looks at, nearest first: today, a day on, a day back, two on...
pub fn nearest_days() -> impl Iterator<Item = i64> {
    (0..=LOOKS_DAYS).flat_map(|n| if n == 0 { vec![0] } else { vec![n, -n] })
}

/// The weather key's next stop from what the weather is now: the first weather after it in the
/// cycle (clear, then the named ones in their file's order) that a day in reach has at this
/// hour, on the nearest such day, given what each day is called, nearest first. Returns its
/// name, how many days on (back, below 0) it is, and the weathers passed over.
pub fn next_weather(
    cycle: &[String],
    now: &str,
    days: &[(i64, String)],
) -> Option<(String, i64, Vec<String>)> {
    let at = cycle.iter().position(|w| w == now).unwrap_or(0);
    let mut passed = Vec::new();
    for step in 1..cycle.len() {
        let wanted = &cycle[(at + step) % cycle.len()];
        match days.iter().find(|(_, d)| d == wanted) {
            Some((on, _)) => return Some((wanted.clone(), *on, passed)),
            None => passed.push(wanted.clone()),
        }
    }
    None
}

/// The number the next feedback takes in a folder of them: one past the highest there
/// (`<n>-<time>.png`), from 1.
pub fn next_number<'a>(names: impl Iterator<Item = &'a str>) -> u32 {
    names
        .filter_map(|name| name.split('-').next()?.parse::<u32>().ok())
        .max()
        .map_or(1, |n| n + 1)
}

/// An hour as a clock shows it, `07:05`.
pub fn clock_text(hour: f32) -> String {
    let minutes = libm::roundf(hour.rem_euclid(24.0) * 60.0) as u32 % 1440;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}
