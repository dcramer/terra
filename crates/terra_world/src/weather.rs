//! The weather at the eye (docs/weather.md, D418): the climate's cloud, what is falling and the
//! wind where the camera is, eased over a few seconds so a front's edge passes rather than
//! snaps, for everything drawn to read as it reads the one wind (D255). Nothing here decides
//! what the weather is: that is `terra_climate`'s fronts, from the seed and the clock. This
//! only says what the light, the sky, the air and what falls make of it, by measured rules:
//!
//! - how much of the clear sky's light comes through cloud, and how much of that is still the
//!   sun's beam (Kasten and Czeplak 1980, from ten years of hourly readings at Hamburg);
//! - how far one sees through falling rain or snow (the visibility that grades a METAR's light,
//!   moderate and heavy, and Rasmussen et al. 1999 for snow, which hides far more than rain of
//!   the same water).
//!
//! A run can ask for a weather by name (`--weather snowfall`): the clock moves on from its date,
//! a day at a time, to the first day whose hour has it at the start. The named weathers are
//! queries (`assets/data/weather.ron`). The one thing a named weather changes is the grade: a
//! weather may carry its own (D419), as a colourist grades a snowfall cold, eased in while the
//! weather at the eye is it.

use crate::climate::ClimateFields;
use crate::daylight::{Clock, Ticking};
use crate::planet::MainCamera;
use crate::shell::Options;
use crate::start::Start;
use bevy::prelude::*;
use serde::Deserialize;
use terra_climate::Conditions;

/// The weather where the camera is, eased.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Weather {
    /// How much of the sky is cloud, 0 to 1.
    pub cloud: f32,
    /// What is falling, as water, millimetres an hour, and how much of it is snow.
    pub falling_mm_h: f32,
    pub snow_share: f32,
    pub wind_m_s: f32,
    pub temperature_c: f32,
}

impl Weather {
    fn of(now: &Conditions) -> Self {
        Self {
            cloud: now.cloud,
            falling_mm_h: now.precipitation_mm_per_hour,
            snow_share: now.snow_share,
            wind_m_s: now.wind.speed_m_s,
            temperature_c: now.temperature_c,
        }
    }

    /// Snow falling, as water, millimetres an hour.
    pub fn snow_mm_h(&self) -> f32 {
        self.falling_mm_h * self.snow_share
    }

    /// Rain falling, millimetres an hour.
    pub fn rain_mm_h(&self) -> f32 {
        self.falling_mm_h * (1.0 - self.snow_share)
    }

    fn ease(&mut self, to: &Self, t: f32) {
        let mix = |a: &mut f32, b: f32| *a += (b - *a) * t;
        mix(&mut self.cloud, to.cloud);
        mix(&mut self.falling_mm_h, to.falling_mm_h);
        mix(&mut self.snow_share, to.snow_share);
        mix(&mut self.wind_m_s, to.wind_m_s);
        mix(&mut self.temperature_c, to.temperature_c);
    }

    /// The light under this sky, against a clear one's.
    pub fn light(&self) -> Under {
        Under::sky(self.cloud, self.falling_mm_h)
    }

    /// How far one sees through what is falling, metres; none if nothing falls.
    pub fn visibility_m(&self) -> Option<f32> {
        visibility_m(self.rain_mm_h(), self.snow_mm_h())
    }
}

/// The light reaching the ground under cloud, each as a share of a clear sky's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Under {
    /// The sun's own beam, what casts shadows.
    pub direct: f32,
    /// The sky's light, from all over it: under a deck, the deck's.
    pub diffuse: f32,
    /// All of it, the beam and the sky together.
    pub total: f32,
}

/// Of a clear sky's light on level ground, the share that comes from the sky rather than the
/// sun's beam (Kasten and Czeplak 1980: diffuse over global 0.3 under a clear sky).
const CLEAR_DIFFUSE: f32 = 0.3;
/// Rain thickens the deck: nimbostratus lets through about a tenth of the clear day's light
/// where a plain overcast lets through a quarter, so each millimetre an hour takes this much.
const THICKER_PER_MM_H: f32 = 0.6;

/// How far what falls has thickened cloud, 0 to 1: the share of the light under it that rain
/// or snow at so many millimetres an hour takes away.
pub fn thickening(falling_mm_h: f32) -> f32 {
    1.0 - 1.0 / (1.0 + THICKER_PER_MM_H * falling_mm_h.max(0.0))
}

impl Under {
    /// Kasten and Czeplak's fit for cloud covering so much of the sky, 0 to 1: the global light
    /// is 1 - 0.75 N^3.4 of a clear sky's, and diffuse over global is 0.3 + 0.7 N², so under a
    /// full deck all of it is the sky's.
    pub fn sky(cloud: f32, falling_mm_h: f32) -> Self {
        let n = cloud.clamp(0.0, 1.0);
        let thick = 1.0 - thickening(falling_mm_h);
        let total = (1.0 - 0.75 * libm::powf(n, 3.4)) * thick;
        let diffuse_share = CLEAR_DIFFUSE + (1.0 - CLEAR_DIFFUSE) * n * n;
        Self {
            direct: total * (1.0 - diffuse_share) / (1.0 - CLEAR_DIFFUSE),
            diffuse: total * diffuse_share / CLEAR_DIFFUSE,
            total,
        }
    }

    /// How many stops the eye opens for this light: most of those lost, as a camera's meter
    /// takes them back, so a dull day reads a little duller than a clear one, not dark.
    pub fn opens_ev(&self) -> f32 {
        EYE_OPENS * libm::log2f(self.total.max(1e-3))
    }
}

/// The share of the stops lost to cloud that the eye takes back. At a half, the snowfall's
/// forest under a full deck had a mean of 0.25 against Sons of the Forest's winter's 0.37,
/// its snow grey and its near trunks black (weather round 2).
const EYE_OPENS: f32 = 0.8;

/// How far one sees in falling rain and snow, metres, for so many millimetres an hour of each
/// (snow as its water): the visibilities a METAR grades intensity by, light snow over a
/// kilometre, heavy under 400 m at about 2.5 mm/h of water (Rasmussen et al. 1999); heavy rain
/// of 8 mm/h leaves some 3 km and a downpour of 50 a kilometre. Each hides as its own extinction,
/// so both falling add up.
pub fn visibility_m(rain_mm_h: f32, snow_mm_h: f32) -> Option<f32> {
    // Koschmieder: visibility is 3.9 over the extinction, so extinctions add as 1/V.
    let snow = (snow_mm_h > 0.01).then(|| 900.0 / libm::powf(snow_mm_h, 0.7));
    let rain = (rain_mm_h > 0.01).then(|| 9000.0 / libm::powf(rain_mm_h, 0.55));
    match (snow, rain) {
        (None, None) => None,
        (Some(v), None) | (None, Some(v)) => Some(v),
        (Some(a), Some(b)) => Some(1.0 / (1.0 / a + 1.0 / b)),
    }
}

/// The deck's light: overcast daylight is near neutral, a little blue of white (about 6,500 K,
/// against the sun's 5,500).
pub const DECK_GREY: Vec3 = Vec3::new(0.92, 0.95, 1.0);
/// The air under the deck, seen to the horizon through falling snow or rain, against the deck
/// over it: a little darker, lit from above only.
const VEIL: f32 = 0.85;

/// How much of the sky is one deck rather than clouds with sky between: none until most of it
/// is cloud, all of it at overcast.
pub fn deck_cover(cloud: f32) -> f32 {
    let t = ((cloud - 0.7) / 0.3).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What the sky shader is told of the deck: how much of the sky it covers, how dark rain has
/// made it, its brightness seen from below (the sky's light under it, as the shade has it, so
/// the deck and the ground it lights agree) and the wind, which drives its scud.
pub fn deck(weather: &Weather, sky_light: f32) -> Vec4 {
    Vec4::new(
        deck_cover(weather.cloud),
        thickening(weather.falling_mm_h),
        sky_light,
        weather.wind_m_s,
    )
}

/// The haze's colour under a deck whose light is so bright, for an eye exposed at so many EV:
/// the deck's brightness on screen, as Bevy mixes the haze in after the exposure.
pub fn veil(sky_light: f32, ev100: f32) -> Vec3 {
    let exposure = 1.0 / (1.2 * libm::exp2f(ev100));
    DECK_GREY * sky_light * exposure * VEIL
}

/// The shade's colour from the sky, toward the deck's grey at its own brightness as the deck
/// closes.
pub fn under_deck(shade: Vec3, deck: f32) -> Vec3 {
    let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    let grey = DECK_GREY * (luma(shade) / luma(DECK_GREY)).max(0.0);
    shade.lerp(grey, deck)
}

/// A named weather: what conditions it means, for finding a moment and for people to say, and
/// the grade it may carry.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Named {
    pub name: String,
    /// Cloud at least, 0 to 1.
    #[serde(default)]
    pub cloud: f32,
    /// Rain at least, and snow at least, as water, millimetres an hour.
    #[serde(default)]
    pub rain_mm_h: f32,
    #[serde(default)]
    pub snow_mm_h: f32,
    /// Wind at least, metres a second.
    #[serde(default)]
    pub wind_m_s: f32,
    /// The grade while this weather holds (D419); none leaves the grade neutral (D370).
    #[serde(default)]
    pub grade: Option<Grade>,
}

/// A weather's grade over the camera's neutral one (D419): white balance as Bevy's
/// `ColorGradingGlobal` takes it (temperature toward blue below 0, tint toward magenta above 0),
/// and the saturation as a share of the neutral grade's.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Grade {
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub tint: f32,
    #[serde(default = "Grade::full")]
    pub saturation: f32,
}

impl Grade {
    pub const NEUTRAL: Self = Self {
        temperature: 0.0,
        tint: 0.0,
        saturation: 1.0,
    };

    fn full() -> f32 {
        1.0
    }

    fn eased(self, to: Self, t: f32) -> Self {
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self {
            temperature: mix(self.temperature, to.temperature),
            tint: mix(self.tint, to.tint),
            saturation: mix(self.saturation, to.saturation),
        }
    }

    fn near(self, other: Self) -> bool {
        (self.temperature - other.temperature).abs() < 1e-4
            && (self.tint - other.tint).abs() < 1e-4
            && (self.saturation - other.saturation).abs() < 1e-4
    }
}

impl Default for Grade {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

/// The grade drawn now: the weather at the eye's own, eased as the weather is.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct WeatherGrade(pub Grade);

/// The named weathers, read once.
#[derive(Resource)]
struct Names(Vec<Named>);

/// `weather.ron`: the named weathers, and how far snow lying round the eye opens the camera.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct WeatherFile {
    pub named: Vec<Named>,
    /// Stops the exposure opens with the ground round the eye under snow, scaled by how much
    /// of it is (D419): a camera's meter reads snow as grey, so an eye on a snowfield opens
    /// up to keep it white. Ground with no snow on it is not moved.
    #[serde(default)]
    pub snow_lying_opens_ev: f32,
}

impl WeatherFile {
    pub fn read() -> Self {
        crate::data::read(Named::FILE)
    }

    /// Stops the exposure opens for so much of the ground round the eye under snow, 0 to 1.
    pub fn snow_opens_ev(&self, cover: f32) -> f32 {
        self.snow_lying_opens_ev * cover.clamp(0.0, 1.0)
    }
}

/// Stops the exposure opens now for the snow lying round the eye (D419), eased as the grade is.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct SnowOpens(pub f32);

#[derive(Resource)]
struct SnowLyingOpens(f32);

/// The grade of the last named weather, in the file's order, that this weather is and that
/// carries one; neutral when none does.
pub fn grade_of(names: &[Named], weather: &Weather) -> Grade {
    names
        .iter()
        .rev()
        .filter(|n| n.holds(weather))
        .find_map(|n| n.grade)
        .unwrap_or_default()
}

impl Named {
    pub const FILE: &str = "weather.ron";

    /// Whether these conditions are this weather.
    pub fn is(&self, now: &Conditions) -> bool {
        self.holds(&Weather::of(now))
    }

    /// Whether the weather at the eye is this weather.
    pub fn holds(&self, w: &Weather) -> bool {
        w.cloud >= self.cloud
            && w.rain_mm_h() >= self.rain_mm_h
            && w.snow_mm_h() >= self.snow_mm_h
            && w.wind_m_s >= self.wind_m_s
    }
}

/// How often the weather at the eye is taken again, in days of the clock (a few seconds of play
/// at half an hour a day), and how fast what is drawn follows it, a share a second.
const EVERY_DAYS: f64 = 0.002;
const FOLLOW: f32 = 0.25;
/// How many days on from its date a run looks for the weather it asks for.
const LOOKS_DAYS: i64 = 730;

pub fn plugin(app: &mut App) {
    let file = WeatherFile::read();
    app.init_resource::<Weather>()
        .init_resource::<WeatherGrade>()
        .init_resource::<SnowOpens>()
        .insert_resource(SnowLyingOpens(file.snow_lying_opens_ev))
        .insert_resource(Names(file.named))
        .add_systems(PostStartup, pick_day)
        .add_systems(Update, (follow, grade).chain().before(Ticking));
}

/// A run asking for a weather moves its clock on to the first day whose hour has it at the start.
fn pick_day(
    options: Res<Options>,
    fields: Option<Res<ClimateFields>>,
    start: Option<Res<Start>>,
    mut clock: ResMut<Clock>,
) {
    let Some(name) = options.weather.as_deref() else {
        return;
    };
    let (Some(fields), Some(start)) = (fields, start) else {
        warn!("--weather {name}: no climate here to find it in");
        return;
    };
    let named = WeatherFile::read().named;
    let Some(wanted) = named.iter().find(|w| w.name == name) else {
        let known: Vec<&str> = named.iter().map(|w| w.name.as_str()).collect();
        panic!(
            "--weather {name}: not in {}; there are {known:?}",
            Named::FILE
        );
    };
    let place = fields.place(start.0.translation);
    let from = clock.day;
    let found = (from..from + LOOKS_DAYS).find(|&day| {
        let mut at = *clock;
        at.day = day;
        wanted.is(&fields.fields.at(place, at.time()))
    });
    match found {
        Some(day) => {
            clock.day = day;
            let now = fields.fields.at(place, clock.time());
            info!(
                "--weather {name}: day {day} ({} on), {:.1} °C, cloud {:.2}, {:.2} mm/h ({:.0}% snow), wind {:.1} m/s",
                day - from,
                now.temperature_c,
                now.cloud,
                now.precipitation_mm_per_hour,
                now.snow_share * 100.0,
                now.wind.speed_m_s
            );
        }
        None => panic!(
            "--weather {name}: none at {:.1} h in {LOOKS_DAYS} days",
            clock.hour
        ),
    }
}

/// Takes the weather where the camera is every few seconds of the clock and eases toward it;
/// the first taking is not eased, so a run starts in its weather.
fn follow(
    time: Res<Time>,
    fields: Option<Res<ClimateFields>>,
    clock: Res<Clock>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut weather: ResMut<Weather>,
    mut taken: Local<Option<(f64, Weather)>>,
) {
    let (Some(fields), Some(camera)) = (fields, camera) else {
        return;
    };
    let now = clock.time().0;
    let first = taken.is_none();
    if taken.is_none_or(|(when, _)| (now - when).abs() >= EVERY_DAYS) {
        let at = camera.translation();
        *taken = Some((
            now,
            Weather::of(&fields.fields.at(fields.place(at), clock.time())),
        ));
    }
    let Some((_, wanted)) = *taken else { return };
    if first {
        info!(
            "weather: cloud {:.2}, {:.2} mm/h falling, wind {:.1} m/s",
            wanted.cloud, wanted.falling_mm_h, wanted.wind_m_s
        );
        *weather = wanted;
        return;
    }
    let t = 1.0 - libm::expf(-time.delta_secs() * FOLLOW * 4.0);
    let mut eased = *weather;
    eased.ease(&wanted, t);
    // Only when it has moved enough to matter, so what reads it changes only then.
    let moved = (eased.cloud - weather.cloud).abs() > 1e-3
        || (eased.falling_mm_h - weather.falling_mm_h).abs() > 1e-3
        || (eased.wind_m_s - weather.wind_m_s).abs() > 1e-2;
    if moved {
        *weather = eased;
    }
}

/// Eases the grade toward the weather's own at the rate the weather follows; a run starts in
/// its weather's grade. Written only when it has moved, so the camera's grade changes only then.
fn grade(
    time: Res<Time>,
    names: Res<Names>,
    weather: Res<Weather>,
    mut graded: ResMut<WeatherGrade>,
    (lying, cover, mut opens): (
        Res<SnowLyingOpens>,
        Option<Res<crate::climate::SnowCover>>,
        ResMut<SnowOpens>,
    ),
    mut started: Local<bool>,
) {
    let wanted = grade_of(&names.0, &weather);
    let t = if *started {
        1.0 - libm::expf(-time.delta_secs() * FOLLOW * 4.0)
    } else {
        1.0
    };
    *started = true;
    let eased = graded.0.eased(wanted, t);
    if !eased.near(graded.0) {
        graded.0 = eased;
    }
    let cover = cover.map_or(0.0, |c| c.0.clamp(0.0, 1.0));
    let wanted = lying.0 * cover;
    let eased = opens.0 + (wanted - opens.0) * t;
    let eased = if (eased - wanted).abs() < 1e-4 {
        wanted
    } else {
        eased
    };
    opens.set_if_neq(SnowOpens(eased));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clear_sky_is_the_clear_light_and_a_deck_leaves_only_the_skys() {
        let clear = Under::sky(0.0, 0.0);
        assert!((clear.direct - 1.0).abs() < 1e-5 && (clear.diffuse - 1.0).abs() < 1e-5);
        let overcast = Under::sky(1.0, 0.0);
        // A quarter of the light, none of it the beam: no shadows.
        assert!((overcast.total - 0.25).abs() < 1e-3);
        assert!(overcast.direct.abs() < 1e-5);
        // The sky's own light under a deck is near what a clear sky gives (both some 15,000
        // lux at midday in summer), so the shade dims a little while the sunlit ground dims
        // to a quarter.
        assert!(overcast.diffuse > 0.7 && overcast.diffuse < 0.95);
    }

    #[test]
    fn rain_darkens_the_deck_and_more_cloud_never_brightens_the_ground() {
        let plain = Under::sky(1.0, 0.0);
        let raining = Under::sky(1.0, 2.5);
        assert!(raining.total < plain.total * 0.5 && raining.total > plain.total * 0.25);
        let mut last = f32::MAX;
        for k in 0..=20 {
            let total = Under::sky(k as f32 / 20.0, 0.0).total;
            assert!(total <= last + 1e-6);
            last = total;
        }
    }

    #[test]
    fn the_eye_opens_for_most_of_the_stops_lost() {
        // A full deck lets through a quarter of the light, two stops under a clear day's.
        let overcast = Under::sky(1.0, 0.0);
        assert!(overcast.opens_ev() < -1.0 && overcast.opens_ev() > -2.0);
        assert!(Under::sky(0.0, 0.0).opens_ev().abs() < 1e-5);
    }

    #[test]
    fn snow_hides_far_more_than_rain_of_the_same_water() {
        assert_eq!(visibility_m(0.0, 0.0), None);
        let heavy_snow = visibility_m(0.0, 2.5).unwrap();
        assert!(heavy_snow < 500.0 && heavy_snow > 300.0, "{heavy_snow}");
        let light_snow = visibility_m(0.0, 0.3).unwrap();
        assert!(light_snow > 1000.0, "{light_snow}");
        let rain = visibility_m(2.5, 0.0).unwrap();
        assert!(rain > 4.0 * heavy_snow);
        let downpour = visibility_m(50.0, 0.0).unwrap();
        assert!(downpour > 700.0 && downpour < 1500.0, "{downpour}");
        // Both falling see less than either alone.
        let both = visibility_m(2.0, 0.5).unwrap();
        assert!(both < visibility_m(2.0, 0.0).unwrap() && both < visibility_m(0.0, 0.5).unwrap());
    }

    #[test]
    fn the_weather_file_reads_and_snowfall_is_snow() {
        let named = WeatherFile::read().named;
        let snowfall = named.iter().find(|w| w.name == "snowfall").unwrap();
        let mut now = Conditions {
            temperature_c: -4.0,
            precipitation_mm_per_hour: 1.5,
            snow_share: 1.0,
            cloud: 1.0,
            wind: terra_climate::Wind {
                speed_m_s: 4.0,
                from: 0.0,
            },
            humidity: 0.95,
            sun_height: 0.2,
            sunlight_w_m2: 100.0,
            ground_sunlight_w_m2: 100.0,
        };
        assert!(snowfall.is(&now));
        now.snow_share = 0.0;
        assert!(!snowfall.is(&now), "rain is not snowfall");
    }

    #[test]
    fn only_falling_snow_grades_cold() {
        // The developers chose a cool grade for snow (D419), and no other weather's look moves.
        let named = WeatherFile::read().named;
        let of = |cloud: f32, falling_mm_h: f32, snow_share: f32, wind_m_s: f32| {
            grade_of(
                &named,
                &Weather {
                    cloud,
                    falling_mm_h,
                    snow_share,
                    wind_m_s,
                    temperature_c: 0.0,
                },
            )
        };
        for snow in [
            of(0.95, 1.0, 1.0, 2.0),
            of(1.0, 2.5, 1.0, 3.0),
            of(1.0, 1.5, 1.0, 10.0),
        ] {
            assert!(snow.temperature < 0.0, "{snow:?}");
            assert!(snow.saturation < 1.0, "{snow:?}");
        }
        for other in [
            of(0.0, 0.0, 0.0, 0.0),
            of(0.95, 0.0, 0.0, 2.0),
            of(1.0, 1.0, 0.0, 2.0),
            of(1.0, 5.0, 0.0, 12.0),
            // Snow lying under a clear or grey sky, nothing falling.
            of(0.2, 0.0, 1.0, 1.0),
            of(1.0, 0.0, 1.0, 1.0),
        ] {
            assert_eq!(other, Grade::NEUTRAL);
        }
    }

    #[test]
    fn snow_lying_opens_the_exposure_and_bare_ground_does_not() {
        // The developers, on opening the exposure when snow lies: "Yes, about +0.5 EV" (D419).
        // Summer and every weather over bare ground keep the exposure they had.
        let file = WeatherFile::read();
        assert_eq!(file.snow_opens_ev(0.0), 0.0, "bare ground is not moved");
        let full = file.snow_opens_ev(1.0);
        assert!((0.25..=0.75).contains(&full), "about half a stop: {full}");
        assert!(file.snow_opens_ev(0.5) < full && file.snow_opens_ev(0.5) > 0.0);
        assert_eq!(file.snow_opens_ev(3.0), full, "no more than all the ground");
    }
}
