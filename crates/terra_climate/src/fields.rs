//! The climate's quantities as fields: pure functions of a place and a time, from the
//! world's climate and seed (climate.md, D314). Nothing is stored and nothing ticks, so every
//! machine works out the same weather from the clock alone (D52).
//!
//! The year's slow terms (the sun's path, the season's warmth and wetness) are taken at
//! each day's middle, so they move on once a day, at midnight; the day's own swing and the
//! weather change smoothly through it.

use crate::sun::{self, day_length_hours, declination};
use crate::{Climate, WorldTime};
use core::f32::consts::{FRAC_1_SQRT_2, TAU};
use glam::Vec3;
use terra_gen::{Noise, Seed};

/// The spread of one sample of `terra_gen`'s noise over space, measured
/// (`the_noise_spread_is_what_the_weather_assumes`): the weather's noises are divided by it
/// to read in standard deviations.
const NOISE_SD: f32 = 0.27;
/// The fronts are a broad noise and a finer one drifting at another speed, so they change
/// shape as they pass rather than sliding by unchanged; their spread together.
const BROAD: f32 = 0.75;
const FINE: f32 = 0.25;
const FRONT_SD: f32 = 0.79 * NOISE_SD;
/// How far past the threshold, in standard deviations, rain or snow takes to fall at its
/// heaviest, and how far ahead of it the cloud builds: clouds come before the rain.
const RAMP: f32 = 1.0;
const CLOUD_LEAD: f32 = 1.0;
/// The hour of the day's warmest, mid-afternoon.
const WARMEST_HOUR: f32 = 15.0;
/// Water falls as snow below 0 °C and as rain above 2 °C, mixed between, as hydrological
/// models take it (the HBV model's threshold and interval).
const ALL_SNOW_C: f32 = 0.0;
pub(crate) const ALL_RAIN_C: f32 = 2.0;

/// Where a place is, as the climate needs it. The caller knows its world's shape; the
/// climate asks only this.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// Where it is in the world's frame, in metres: where the weather's fronts are.
    pub at: Vec3,
    /// How high it is over the sea, in metres.
    pub height_m: f32,
    /// Its latitude, in radians.
    pub latitude: f32,
}

/// The climate's quantities at a place and a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conditions {
    /// The air's temperature, in degrees Celsius.
    pub temperature_c: f32,
    /// How hard rain or snow falls, in millimetres of water an hour.
    pub precipitation_mm_per_hour: f32,
    /// The share of what falls that is snow.
    pub snow_share: f32,
    /// How cloudy the sky is, 0 clear to 1 overcast.
    pub cloud: f32,
    pub wind: Wind,
    /// The air's relative humidity, 0 to 1.
    pub humidity: f32,
    /// How high the sun stands, in radians; below 0 it is night.
    pub sun_height: f32,
    /// The sun's light on level ground, in watts a square metre.
    pub sunlight_w_m2: f32,
}

/// The wind at a place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wind {
    /// How fast it blows, in metres a second.
    pub speed_m_s: f32,
    /// Where it blows from, in radians clockwise from north.
    pub from: f32,
}

/// The sun's path on a day at a latitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Daylight {
    /// How far north of the equator the sun stands at noon, in radians.
    pub declination: f32,
    /// How long the sun is up, in hours.
    pub day_hours: f32,
}

/// The share of what falls that is snow at a temperature.
pub fn snow_share(temperature_c: f32) -> f32 {
    ((ALL_RAIN_C - temperature_c) / (ALL_RAIN_C - ALL_SNOW_C)).clamp(0.0, 1.0)
}

/// A world's climate fields: its climate and the noises its seed makes for the weather.
#[derive(Clone, Debug)]
pub struct Fields {
    climate: Climate,
    fronts: Noise,
    eddies: Noise,
    air: Noise,
    gusts: Noise,
    /// The ways the fronts, the eddies and the air masses drift, chosen by the seed.
    drift: [Vec3; 3],
}

impl Fields {
    pub fn new(climate: Climate, seed: Seed) -> Self {
        let seed = seed.derive("climate");
        let mut rng = seed.derive("drift").rng();
        let drift = [rng.unit_vector(), rng.unit_vector(), rng.unit_vector()];
        Self {
            climate,
            fronts: Noise::new(seed.derive("fronts")),
            eddies: Noise::new(seed.derive("eddies")),
            air: Noise::new(seed.derive("air")),
            gusts: Noise::new(seed.derive("gusts")),
            drift,
        }
    }

    pub fn climate(&self) -> &Climate {
        &self.climate
    }

    /// The sun's path on a day at a latitude.
    pub fn daylight(&self, latitude: f32, day: i64) -> Daylight {
        let year = &self.climate.year;
        let declination = declination(year.tilt_deg.to_radians(), year.phase(day));
        Daylight {
            declination,
            day_hours: day_length_hours(latitude, declination),
        }
    }

    /// A day at a place: its slow terms worked out once, for asking at its hours.
    pub fn day(&self, place: Place, day: i64) -> Day<'_> {
        let climate = &self.climate;
        let phase = climate.year.phase(day);
        let swing = |share: f32, most: f32| share * libm::cosf(TAU * (phase - most));
        let t = &climate.temperature;
        let sine = libm::sinf(place.latitude);
        let lag = t.annual_lag_days / climate.year.days as f32;
        let annual = t.annual_swing_c * sine.abs() / FRAC_1_SQRT_2;
        let base_c = t.equator_c + (t.pole_c - t.equator_c) * sine * sine
            - t.lapse_c_per_km * place.height_m / 1000.0
            - annual * libm::cosf(TAU * (phase - lag));
        let p = &climate.precipitation;
        let share = p.share * (1.0 + swing(p.annual_swing, p.wettest));
        let h = &climate.humidity;
        let daylight = self.daylight(place.latitude, day);
        Day {
            fields: self,
            place,
            day,
            phase,
            daylight,
            sun: sun::Path::new(place.latitude, daylight.declination),
            base_c,
            threshold: upper_quantile(share),
            humidity: h.mean + swing(h.annual_swing, h.most_humid),
        }
    }

    /// The climate at a place and a moment.
    pub fn at(&self, place: Place, time: WorldTime) -> Conditions {
        self.day(place, time.day()).at(time.hour() as f32)
    }

    /// The air's temperature at a place and a moment, in degrees Celsius.
    pub fn temperature(&self, place: Place, time: WorldTime) -> f32 {
        self.day(place, time.day())
            .weather(time.hour() as f32)
            .temperature_c
    }
}

/// A day at a place, its slow terms worked out.
#[derive(Clone, Copy, Debug)]
pub struct Day<'a> {
    fields: &'a Fields,
    place: Place,
    day: i64,
    /// Where in the year the day is, as a share after midwinter.
    pub phase: f32,
    pub daylight: Daylight,
    sun: sun::Path,
    /// The day's temperature before its own swing and the weather's.
    base_c: f32,
    /// How far, in standard deviations, the fronts must rise for something to fall.
    threshold: f32,
    /// The day's humidity before the weather's.
    humidity: f32,
}

/// What the integrated past needs of a moment: the climate without the wind and humidity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Weather {
    pub temperature_c: f32,
    pub precipitation_mm_per_hour: f32,
    pub snow_share: f32,
    pub cloud: f32,
    pub sunlight_w_m2: f32,
    sun_height: f32,
    /// The fronts here, in standard deviations, and the day's swing, -1 to 1.
    front: f32,
    daily: f32,
    /// The weather's position in the noise.
    point: Vec3,
    tau: f32,
}

impl Day<'_> {
    /// The climate at an hour of the day, 0 to 24.
    pub fn at(&self, hour: f32) -> Conditions {
        let weather = self.weather(hour);
        let fields = self.fields;
        let w = &fields.climate.wind;
        let gust = |shift: f32| {
            let point = weather.point * 1.5 + fields.drift[1] * (1.3 * weather.tau);
            fields.gusts.sample(point + Vec3::splat(shift)) / NOISE_SD
        };
        let storm = 0.8 * gust(0.0) + 0.6 * weather.front.clamp(-2.0, 2.0);
        let speed_m_s = (w.mean_m_s * (1.0 + w.gustiness * storm)).max(0.0);
        let turn = (gust(57.3) / 2.0).clamp(-1.0, 1.0);
        let from = (w.prevailing_from_deg + w.turning_deg * turn)
            .to_radians()
            .rem_euclid(TAU);
        let mut humidity = self.humidity + 0.08 * weather.front - 0.1 * weather.daily;
        if weather.precipitation_mm_per_hour > 0.0 {
            humidity = humidity.max(0.95);
        }
        Conditions {
            temperature_c: weather.temperature_c,
            precipitation_mm_per_hour: weather.precipitation_mm_per_hour,
            snow_share: weather.snow_share,
            cloud: weather.cloud,
            wind: Wind { speed_m_s, from },
            humidity: humidity.clamp(0.05, 1.0),
            sun_height: weather.sun_height,
            sunlight_w_m2: weather.sunlight_w_m2,
        }
    }

    pub(crate) fn weather(&self, hour: f32) -> Weather {
        let fields = self.fields;
        let climate = &fields.climate;
        let days = self.day as f64 + hour as f64 / crate::time::HOURS;
        let tau = (days / climate.fronts.spell_days as f64) as f32;
        let point = self.place.at / climate.fronts.size_m;
        let broad = fields.fronts.sample(point + fields.drift[0] * tau);
        let fine = fields
            .eddies
            .sample(point * 2.3 + fields.drift[1] * (1.6 * tau));
        let front = (BROAD * broad + FINE * fine) / FRONT_SD;
        let mass = fields
            .air
            .sample(point * 0.6 + fields.drift[2] * (0.7 * tau))
            / NOISE_SD;
        let above = front - self.threshold;
        let precipitation =
            climate.precipitation.heaviest_mm_per_hour * (above / RAMP).clamp(0.0, 1.0);
        // Said from how far past the threshold the fronts are, which is infinite where
        // nothing ever falls, or always does.
        let cloud = smoothstep(-CLOUD_LEAD, 0.0, above);
        let daily = libm::cosf((hour - WARMEST_HOUR) / 24.0 * TAU);
        let t = &climate.temperature;
        let temperature_c =
            self.base_c + t.daily_swing_c * (1.0 - 0.5 * cloud) * daily + t.weather_swing_c * mass;
        let sine = self.sun.sine(hour);
        Weather {
            temperature_c,
            precipitation_mm_per_hour: precipitation,
            snow_share: snow_share(temperature_c),
            cloud,
            sunlight_w_m2: sun::on_level(sine, cloud),
            sun_height: libm::asinf(sine),
            front,
            daily,
            point,
            tau,
        }
    }
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How many standard deviations a normal variable must pass to do so only a share of the
/// time (Abramowitz and Stegun 26.2.23, within 0.00045). No share is never; all is always.
fn upper_quantile(share: f32) -> f32 {
    if share <= 0.0 {
        return f32::INFINITY;
    }
    if share >= 1.0 {
        return f32::NEG_INFINITY;
    }
    let tail = |p: f32| {
        let t = (-2.0 * libm::logf(p)).sqrt();
        t - (2.515_517 + 0.802_853 * t + 0.010_328 * t * t)
            / (1.0 + 1.432_788 * t + 0.189_269 * t * t + 0.001_308 * t * t * t)
    };
    if share <= 0.5 {
        tail(share)
    } else {
        -tail(1.0 - share)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{Fronts, Humidity, Precipitation, Temperatures, Winds, Year};
    use terra_gen::Rng;

    /// A temperate world to test with. The tests check behaviour, not these numbers.
    pub fn temperate() -> Climate {
        Climate {
            year: Year {
                days: 365.0,
                tilt_deg: 23.44,
                start: 0.68,
            },
            temperature: Temperatures {
                equator_c: 27.0,
                pole_c: -18.0,
                lapse_c_per_km: 6.5,
                annual_swing_c: 13.0,
                annual_lag_days: 30.0,
                daily_swing_c: 5.0,
                weather_swing_c: 4.0,
            },
            precipitation: Precipitation {
                share: 0.12,
                annual_swing: 0.15,
                wettest: 0.55,
                heaviest_mm_per_hour: 2.5,
            },
            wind: Winds {
                mean_m_s: 4.0,
                gustiness: 0.5,
                prevailing_from_deg: 270.0,
                turning_deg: 90.0,
            },
            humidity: Humidity {
                mean: 0.7,
                annual_swing: 0.1,
                most_humid: 0.0,
            },
            fronts: Fronts {
                size_m: 3000.0,
                spell_days: 3.0,
            },
        }
    }

    pub fn place(rng: &mut Rng) -> Place {
        Place {
            at: rng.unit_vector() * rng.range(0.0, 50_000.0),
            height_m: rng.range(0.0, 300.0),
            latitude: rng.range(-1.2, 1.2),
        }
    }

    fn here() -> Place {
        Place {
            at: Vec3::new(120.0, 2000.0, -40.0),
            height_m: 50.0,
            latitude: 40.0_f32.to_radians(),
        }
    }

    #[test]
    fn the_noise_spread_is_what_the_weather_assumes() {
        // Measured over many places, the noise's standard deviation and the fronts'.
        let noise = Noise::new(Seed(9));
        let other = Noise::new(Seed(10));
        let mut rng = Seed(4).rng();
        let (mut one, mut both, count) = (0.0, 0.0, 200_000);
        for _ in 0..count {
            let p = rng.unit_vector() * rng.range(0.0, 1000.0);
            let n = noise.sample(p);
            let m = BROAD * n + FINE * other.sample(p * 2.3);
            one += n * n;
            both += m * m;
        }
        let (one, both) = ((one / count as f32).sqrt(), (both / count as f32).sqrt());
        assert!((one / NOISE_SD - 1.0).abs() < 0.05, "noise spread {one}");
        assert!(
            (both / FRONT_SD - 1.0).abs() < 0.05,
            "fronts' spread {both}"
        );
    }

    #[test]
    fn it_falls_about_the_share_of_the_time_the_climate_asks() {
        for share in [0.05, 0.12, 0.3, 0.6] {
            let mut climate = temperate();
            climate.precipitation.share = share;
            climate.precipitation.annual_swing = 0.0;
            let fields = Fields::new(climate, Seed(3));
            let mut rng = Seed(8).rng();
            let count = 20_000;
            let wet = (0..count)
                .filter(|_| {
                    let time = WorldTime(rng.range(-2000.0, 2000.0) as f64);
                    fields.at(place(&mut rng), time).precipitation_mm_per_hour > 0.0
                })
                .count() as f32
                / count as f32;
            assert!(
                (wet / share - 1.0).abs() < 0.25,
                "{wet} of the time, not {share}"
            );
        }
    }

    #[test]
    fn with_no_annual_swing_every_date_is_as_warm() {
        let spread = |swing: f32| {
            let mut climate = temperate();
            climate.temperature.annual_swing_c = swing;
            climate.temperature.weather_swing_c = 0.0;
            let fields = Fields::new(climate, Seed(1));
            let temperatures: Vec<f32> = (0..365)
                .map(|day| fields.temperature(here(), WorldTime::at(day, 9.0)))
                .collect();
            let low = temperatures.iter().cloned().fold(f32::MAX, f32::min);
            let high = temperatures.iter().cloned().fold(f32::MIN, f32::max);
            high - low
        };
        assert!(spread(0.0) < 1e-3, "{}", spread(0.0));
        // And the swing, where there is one, is what the climate says at that latitude.
        let swung = 2.0 * 10.0 * libm::sinf(40.0_f32.to_radians()) / FRAC_1_SQRT_2;
        assert!((spread(10.0) - swung).abs() < 0.05, "{}", spread(10.0));
    }

    #[test]
    fn higher_is_colder_by_the_lapse_rate() {
        let fields = Fields::new(temperate(), Seed(2));
        let mut rng = Seed(1).rng();
        for _ in 0..200 {
            let low = place(&mut rng);
            let high = Place {
                height_m: low.height_m + 1000.0,
                ..low
            };
            let time = WorldTime(rng.range(-500.0, 500.0) as f64);
            let drop = fields.temperature(low, time) - fields.temperature(high, time);
            assert!((drop - 6.5).abs() < 1e-3, "{drop}");
        }
    }

    #[test]
    fn the_coldest_and_warmest_times_lag_midwinter_and_midsummer() {
        let mut climate = temperate();
        climate.temperature.weather_swing_c = 0.0;
        climate.year.start = 0.0;
        let fields = Fields::new(climate, Seed(1));
        let warmth: Vec<f32> = (0..365)
            .map(|day| fields.temperature(here(), WorldTime::at(day, 9.0)))
            .collect();
        let coldest = (0..365)
            .min_by(|a, b| warmth[*a].total_cmp(&warmth[*b]))
            .unwrap();
        let warmest = (0..365)
            .max_by(|a, b| warmth[*a].total_cmp(&warmth[*b]))
            .unwrap();
        assert!((coldest as i32 - 30).abs() <= 2, "coldest on {coldest}");
        assert!((warmest as i32 - 212).abs() <= 2, "warmest on {warmest}");
    }

    #[test]
    fn afternoons_are_warmer_than_dawns() {
        let mut climate = temperate();
        climate.temperature.weather_swing_c = 0.0;
        let fields = Fields::new(climate, Seed(1));
        for day in [0, 90, 200] {
            let at = |hour| fields.temperature(here(), WorldTime::at(day, hour));
            assert!(at(15.0) > at(10.0) && at(10.0) > at(4.0), "{day}");
        }
    }

    #[test]
    fn the_same_seed_and_time_give_the_same_weather_and_another_seed_other_weather() {
        let (one, again, other) = (
            Fields::new(temperate(), Seed(5)),
            Fields::new(temperate(), Seed(5)),
            Fields::new(temperate(), Seed(6)),
        );
        let mut rng = Seed(2).rng();
        let mut differ = 0;
        for _ in 0..500 {
            let (place, time) = (place(&mut rng), WorldTime(rng.range(-900.0, 900.0) as f64));
            let a = one.at(place, time);
            assert_eq!(a, again.at(place, time));
            differ += usize::from(a.temperature_c != other.at(place, time).temperature_c);
        }
        assert!(differ > 490, "only {differ} of 500 differ");
    }

    #[test]
    fn the_weather_moves_on_with_the_clock_smoothly() {
        let fields = Fields::new(temperate(), Seed(5));
        let mut rng = Seed(3).rng();
        let (mut changed, mut jumps) = (0.0, 0.0_f32);
        for _ in 0..300 {
            let place = place(&mut rng);
            let day = rng.range(-500.0, 500.0) as i64;
            let at = |hour: f64| fields.day(place, day).weather(hour as f32).front;
            // Over a spell the fronts change by about their spread; over a few minutes, little.
            changed += (at(0.0) - fields.day(place, day + 3).weather(0.0).front).abs();
            jumps = jumps.max((at(10.0) - at(10.1)).abs());
        }
        assert!(changed / 300.0 > 0.5, "{}", changed / 300.0);
        assert!(jumps < 0.05, "{jumps}");
    }

    #[test]
    fn snow_falls_below_freezing_and_rain_above() {
        assert_eq!(snow_share(-5.0), 1.0);
        assert_eq!(snow_share(0.0), 1.0);
        assert_eq!(snow_share(1.0), 0.5);
        assert_eq!(snow_share(3.0), 0.0);
    }

    #[test]
    fn clouds_come_before_rain_and_dim_the_sun() {
        let fields = Fields::new(temperate(), Seed(7));
        let mut rng = Seed(6).rng();
        for _ in 0..3000 {
            let c = fields.at(place(&mut rng), WorldTime(rng.range(-900.0, 900.0) as f64));
            if c.precipitation_mm_per_hour > 0.0 {
                assert!(c.cloud > 0.99 && c.humidity >= 0.95, "{c:?}");
            }
            assert!((0.0..=1.0).contains(&c.cloud) && (0.05..=1.0).contains(&c.humidity));
            assert!(c.wind.speed_m_s >= 0.0 && (0.0..TAU).contains(&c.wind.from));
        }
    }

    #[test]
    fn a_climate_is_data_and_the_clock_is_sent_as_a_number() {
        let climate = temperate();
        let text = ron::to_string(&climate).unwrap();
        assert_eq!(ron::from_str::<Climate>(&text).unwrap(), climate);
        let time = WorldTime::at(12, 7.5);
        assert_eq!(ron::to_string(&time).unwrap(), "(12.3125)");
    }

    #[test]
    fn the_quantile_is_the_normal_tail() {
        assert!(upper_quantile(0.5).abs() < 1e-3);
        assert!((upper_quantile(0.1587) - 1.0).abs() < 2e-3);
        assert!((upper_quantile(0.8413) + 1.0).abs() < 2e-3);
        assert!((upper_quantile(0.0228) - 2.0).abs() < 2e-3);
    }
}
