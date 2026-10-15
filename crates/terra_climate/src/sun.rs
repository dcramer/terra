//! The sun's path by the date: its declination, how long the day is, how high it stands.
//!
//! One season covers a whole world, with no opposite hemispheres (D149), so a latitude
//! south of the equator has the same days as the same latitude north of it.

use core::f32::consts::{PI, TAU};

/// The sunlight above the air, in watts a square metre: Earth's solar constant.
const ABOVE_THE_AIR_W_M2: f32 = 1361.0;
/// The share of the sun's light a clear sky lets through when the sun is overhead, about
/// three quarters, the value melt models take (Hock 1999), and its natural logarithm.
#[cfg(test)]
const CLEAR_SKY: f32 = 0.75;
const LN_CLEAR_SKY: f32 = -0.287_682;
/// The share of the sunlight a full overcast still lets through.
const UNDER_CLOUD: f32 = 0.25;

/// How far north of the equator the sun stands at noon, in radians, at a share of the year
/// after midwinter: the axis's tilt south at midwinter, north at midsummer, nothing at the
/// equinoxes (an orbit taken as round).
pub fn declination(tilt: f32, phase: f32) -> f32 {
    -libm::asinf(libm::sinf(tilt) * libm::cosf(TAU * phase))
}

/// How long the sun is up, in hours, at a latitude (radians) on a day of a declination:
/// the sunrise equation, cos h = -tan latitude tan declination, h the hour angle from
/// sunrise to noon. Past the polar circles the sun can stay up, or down, all day.
pub fn day_length_hours(latitude: f32, declination: f32) -> f32 {
    let cosine = -libm::tanf(latitude.abs().min(PI / 2.0 - 1e-4)) * libm::tanf(declination);
    if cosine <= -1.0 {
        24.0
    } else if cosine >= 1.0 {
        0.0
    } else {
        libm::acosf(cosine) / PI * 24.0
    }
}

/// How high the sun stands over the horizon, in radians, at a latitude on a day of a
/// declination, at an hour (noon at 12).
pub fn sun_height(latitude: f32, declination: f32, hour: f32) -> f32 {
    libm::asinf(Path::new(latitude, declination).sine(hour))
}

/// The sun's path on a day at a latitude, worked out once for the day's hours: the sine of
/// its height is `over + swing * cos(hour angle)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Path {
    over: f32,
    swing: f32,
}

impl Path {
    pub fn new(latitude: f32, declination: f32) -> Self {
        let latitude = latitude.abs();
        Self {
            over: libm::sinf(latitude) * libm::sinf(declination),
            swing: libm::cosf(latitude) * libm::cosf(declination),
        }
    }

    /// The sine of the sun's height at an hour.
    pub fn sine(&self, hour: f32) -> f32 {
        let angle = (hour - 12.0) / 24.0 * TAU;
        (self.over + self.swing * libm::cosf(angle)).clamp(-1.0, 1.0)
    }
}

/// The sun's light on level ground, in watts a square metre, with the sun so high and the
/// sky so cloudy (0 clear to 1 overcast): the light through the depth of air the sun's
/// height means, on ground it strikes at a slant.
pub fn sunlight(height: f32, cloud: f32) -> f32 {
    on_level(libm::sinf(height), cloud)
}

/// [`sunlight`] from the sine of the sun's height.
pub(crate) fn on_level(sine: f32, cloud: f32) -> f32 {
    if sine <= 0.0 {
        return 0.0;
    }
    let through = libm::expf(LN_CLEAR_SKY / sine.max(0.02));
    ABOVE_THE_AIR_W_M2 * sine * through * (1.0 - (1.0 - UNDER_CLOUD) * cloud.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILT: f32 = 23.44 * PI / 180.0;

    #[test]
    fn the_sun_swings_by_the_tilt_over_the_year() {
        assert!((declination(TILT, 0.0) + TILT).abs() < 1e-5);
        assert!((declination(TILT, 0.5) - TILT).abs() < 1e-5);
        assert!(declination(TILT, 0.25).abs() < 1e-5);
        assert!(declination(TILT, 0.75).abs() < 1e-5);
        assert!(declination(0.0, 0.4).abs() < 1e-7, "no tilt, no seasons");
    }

    #[test]
    fn midsummer_days_are_long_and_midwinter_days_short_but_not_on_the_equator() {
        let (summer, winter) = (declination(TILT, 0.5), declination(TILT, 0.0));
        let north = 45.0_f32.to_radians();
        assert!(day_length_hours(north, summer) > 15.0);
        assert!(day_length_hours(north, winter) < 9.0);
        // The same in the south: one season over the world (D149).
        assert_eq!(
            day_length_hours(-north, summer),
            day_length_hours(north, summer)
        );
        for phase in [0.0, 0.2, 0.5, 0.9] {
            let day = day_length_hours(0.0, declination(TILT, phase));
            assert!((day - 12.0).abs() < 1e-4, "{phase}: {day}");
        }
        // Past the polar circle, the midsummer sun never sets and the midwinter one never rises.
        let arctic = 80.0_f32.to_radians();
        assert_eq!(day_length_hours(arctic, summer), 24.0);
        assert_eq!(day_length_hours(arctic, winter), 0.0);
    }

    #[test]
    fn the_day_is_as_long_as_the_sun_stands_over_the_horizon() {
        // Counted another way: step through the day and count the hours the sun is up.
        for (latitude, phase) in [(40.0_f32, 0.5), (40.0, 0.0), (55.0, 0.3), (10.0, 0.8)] {
            let latitude = latitude.to_radians();
            let tilt = declination(TILT, phase);
            let steps = 24 * 60;
            let up = (0..steps)
                .filter(|minute| sun_height(latitude, tilt, (*minute as f32 + 0.5) / 60.0) > 0.0)
                .count() as f32
                / 60.0;
            let day = day_length_hours(latitude, tilt);
            assert!((up - day).abs() < 0.05, "{up} hours up, not {day}");
        }
    }

    #[test]
    fn the_noon_sun_stands_as_far_from_overhead_as_the_latitude_less_the_declination() {
        let (latitude, tilt) = (40.0_f32.to_radians(), 10.0_f32.to_radians());
        let noon = sun_height(latitude, tilt, 12.0);
        assert!((noon - (PI / 2.0 - latitude + tilt)).abs() < 1e-4);
    }

    #[test]
    fn sunlight_is_strongest_high_and_clear_and_none_at_night() {
        assert!((LN_CLEAR_SKY - libm::logf(CLEAR_SKY)).abs() < 1e-7);
        assert_eq!(sunlight(-0.1, 0.0), 0.0);
        let overhead = sunlight(PI / 2.0, 0.0);
        assert!((overhead - ABOVE_THE_AIR_W_M2 * CLEAR_SKY).abs() < 1.0);
        assert!(sunlight(0.3, 0.0) < sunlight(0.8, 0.0));
        assert!(sunlight(0.8, 1.0) < sunlight(0.8, 0.0) * 0.3);
    }
}
