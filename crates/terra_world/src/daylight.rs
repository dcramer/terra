//! Day and night (game design 001, survival.md): a clock, the sun crossing the sky by it,
//! and the light, sky and shade following how high the sun is.
//!
//! The sun keeps a real path over the start, as seen from a middle latitude on the date:
//! it rises in the east, stands high at noon and sets in the west, and its path swings
//! with the world's year (`terra_climate`, D315). The clock starts in late August, when the
//! day is a little longer than the night. After it sets, the same light becomes the moon's,
//! cool and dim, from the other side of the sky, so there is one light casting shadows.
//! The stars turn with the sky. A new game starts in the morning, with the low warm
//! light the art was made for.
//!
//! How long a day is, and what night does besides being dark, are the developers' to
//! say; the numbers here are a proposal (prototypes.md).

use super::scene::EXPOSURE_EV100;
use super::skylight::SkyLight;
use crate::planet::{MainCamera, Planet, Tweaks};
use crate::shell::Options;
use crate::sky::Sky;
use bevy::{camera::Exposure, pbr::DistanceFog, prelude::*};
use std::f32::consts::TAU;
use terra_gen::smoothstep;

/// How long a whole day and night lasts, in seconds of play. Proposed: half an hour, as
/// Valheim's and Enshrouded's are.
pub const DAY_SECONDS: f32 = 30.0 * 60.0;
/// The sky lights the shade this many times as brightly as the keys below say. Open sky
/// gives level ground something like a sixth of what the sun does; the keys gave it about a
/// sixteenth, so every shadow was near black (D216). Under trees the shade is darker than open
/// ground's: 1.6 left it three stops brighter than Sons of the Forest's (D286).
const SKY_LIGHT_GAIN: f32 = 1.2;
/// By night, how much of the day's saturation is left, as an eye loses colour in the dark
/// (D302); its white balance stays neutral (D370).
const NIGHT_SATURATION: f32 = 0.45;
/// From sunset, how much of its colour what is dimly lit keeps against what is bright, the
/// grade's shadows against its midtones (D302).
const DIM_SATURATION: f32 = 0.2;
/// By night, where the grade's midtones start (Bevy's 0.2 by day), its shadows wholly under
/// it less 0.1 (D302).
const NIGHT_MIDTONES: f32 = 0.12;
/// Through the dusk, how much the grade lifts every colour before the tonemapper (D302).
/// Khronos PBR Neutral's toe takes a dark colour's least channel down to 6.25 times its
/// square (x - 6.25x² under 0.08), so a dim colour leaves it far more saturated than it came:
/// grass the grade had faded to 0.2 came out at 0.62, lawn green, and the shaded hill at luma 3
/// (lake film r8). Lifted, the least channel sits higher up the toe, so the dim land comes out
/// as grey as it went in, and dark shapes keep their form, as a dusk photograph's do.
const DUSK_LIFT: f32 = 0.02;
/// By night, how bright the haze is against the day's, and its colour, the moon's (D310).
const NIGHT_HAZE: f32 = 0.15;
const MOON_HAZE: Vec3 = Vec3::new(0.32, 0.38, 0.52);
/// How high the moon stands by night, in radians (D310).
const MOON_HEIGHT: f32 = 0.7;

/// How far into the night it is: 0 by day, 1 once the sun is twelve degrees under the horizon.
/// The haze, the grade and the eye follow it (D310).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Nightfall(pub f32);

/// How far one sees in the open air, out of the trees, before the forest's haze swallows
/// what is there, and how bright that haze is against the horizon's colour (D273).
const OPEN_VISIBLE_M: f32 = 2_500.0;
const OPEN_AIR: f32 = 0.45;

/// The sun's direction moves on in this many steps an hour: a twentieth of a degree each,
/// a few times a second at the game's pace.
const SUN_STEPS: f32 = 300.0;

/// The hour a new game starts at: morning, the sun low in the east.
pub const MORNING: f32 = terra_land::frame::MORNING;
/// Where the planet's own start is on its world, which with the date shapes the sun's path:
/// how high it climbs and how long it is up. Elsewhere a place's latitude is its own, from
/// the world's frame (`terra_land::frame`): the sun is fixed in the sky, and a place further
/// north sees it lower (docs/demos.md).
pub const LATITUDE: f32 = terra_land::frame::START_LATITUDE;

/// The world's climate (`assets/data/climate.ron`, D314). The sky follows its year; the
/// rest of the climate is read from it as the climate's later steps come (climate.md).
#[derive(Resource, Clone, Debug)]
pub struct WorldClimate(pub terra_climate::Climate);

impl WorldClimate {
    pub const FILE: &str = "climate.ron";

    /// How far north of the equator the sun stands on a day of the clock, in radians.
    pub fn declination(&self, day: i64) -> f32 {
        let year = &self.0.year;
        terra_climate::declination(year.tilt_deg.to_radians(), year.phase(day))
    }
}

/// The world's clock: the day since it started, and the hour, 0 to 24, noon at 12. Shared
/// as a [`terra_climate::WorldTime`] (D52).
#[derive(Resource, Reflect, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct Clock {
    pub day: i64,
    pub hour: f32,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            day: 0,
            hour: MORNING,
        }
    }
}

impl Clock {
    /// Moves the clock on by so many seconds of play.
    pub fn advance(&mut self, seconds: f32) {
        let hour = self.hour + seconds / DAY_SECONDS * 24.0;
        self.day += libm::floorf(hour / 24.0) as i64;
        self.hour = hour.rem_euclid(24.0);
    }

    /// The moment, as the climate counts it.
    pub fn time(&self) -> terra_climate::WorldTime {
        terra_climate::WorldTime::at(self.day, self.hour as f64)
    }
}

/// The sky over the start: which ways are east, north and up there, and how far north of
/// the equator the sun stands on the date. The sun is fixed in the world's sky, whatever the
/// place, so a place far from the start sees it at its own height: further north lower, as
/// its latitude says (`terra_land::frame`). How high it stands, and the moon, are taken where
/// the camera is (`seen_from`).
#[derive(Resource, Clone, Copy, Debug)]
pub struct SkyPath {
    east: Vec3,
    north: Vec3,
    up: Vec3,
    declination: f32,
    /// Up where the sky is seen from: the camera's.
    seen_from: Vec3,
}

impl SkyPath {
    /// The sky over a place, turned so the morning sun stands where the art put it on a day
    /// of so much declination: the clock's first day's.
    pub fn over(up: Vec3, declination: f32) -> Self {
        let (_, east, north) = terra_land::frame::Frame::turned_for(up, LATITUDE, declination);
        let up = up.normalize();
        Self {
            east,
            north,
            up,
            declination,
            seen_from: up,
        }
    }

    /// The same sky seen from a place whose up is so.
    pub fn seen_from(self, up: Vec3) -> Self {
        Self {
            seen_from: up.normalize_or(self.up),
            ..self
        }
    }

    /// The sun's path on a day of so much declination, the sky turned as it was.
    pub fn on(self, declination: f32) -> Self {
        Self {
            declination,
            ..self
        }
    }

    /// The direction toward the sun at an hour.
    pub fn sun(&self, hour: f32) -> Vec3 {
        let (x, y, z) = sun_local(LATITUDE, hour, self.declination);
        (self.east * x + self.north * y + self.up * z).normalize()
    }

    /// The direction toward the moon at an hour (D310): opposite the sun round the sky, as a
    /// full moon is, but standing [`MOON_HEIGHT`] up rather than as far below the horizon as the
    /// sun is, so it lights the night from high, not along the ground from the east at dusk.
    pub fn moon(&self, hour: f32) -> Vec3 {
        let up = self.seen_from;
        let across = (-self.sun(hour)).reject_from(up).normalize_or(self.east);
        (across * libm::cosf(MOON_HEIGHT) + up * libm::sinf(MOON_HEIGHT)).normalize()
    }

    /// The axis the sky turns about, which points at the pole star.
    pub fn pole(&self) -> Vec3 {
        (self.north * libm::cosf(LATITUDE) + self.up * libm::sinf(LATITUDE)).normalize()
    }

    /// How high the sun is at an hour, in degrees above the horizon where it is seen from.
    pub fn height(&self, hour: f32) -> f32 {
        libm::asinf(self.sun(hour).dot(self.seen_from).clamp(-1.0, 1.0)).to_degrees()
    }

    /// The hour as the sun keeps it where the sky is seen from: the clock's hour at the start,
    /// later east of it and earlier west, by an hour for every 15 degrees the sky turns between
    /// them. A place a quarter of the way round a small world sees the dawn hours after the
    /// start does, so what goes with the time of day there, the morning's mist and warmth,
    /// goes by its own sun, not the start's clock.
    pub fn local_hour(&self, hour: f32) -> f32 {
        let pole = self.pole();
        let start = self.up.reject_from(pole).normalize_or_zero();
        let here = self.seen_from.reject_from(pole).normalize_or_zero();
        let turned = libm::atan2f(start.cross(here).dot(pole), start.dot(here));
        (hour + turned / TAU * 24.0).rem_euclid(24.0)
    }
}

use terra_land::frame::sun_local;

/// How the world is lit at a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Light {
    /// The sun's or the moon's colour, and how bright.
    light: Vec3,
    illuminance: f32,
    horizon: Vec3,
    zenith: Vec3,
    glow: Vec3,
    /// The shade's colour, and how bright.
    shade: Vec3,
    ambient: f32,
    /// How bright the sky is drawn, and how far into the night.
    sky: f32,
    night: f32,
    /// The camera's exposure (EV at ISO 100): set for sunlit ground by day, opening up a
    /// little through the dusk so that the twilight sky still shows, as an eye adapts.
    exposure: f32,
    /// How much of its colour the land in the grade's shadows and midtones keeps: under a
    /// low sun the meadow greys and the light's warmth carries the colour, as in a dusk
    /// photograph, while the bright sky and the sunlit faces (the highlights) keep theirs.
    /// At full colour the end of the dusk film read as a mid-afternoon's lawn green under a
    /// pink sky (settlement r19).
    colour: f32,
}

impl Light {
    fn mix(self, other: Light, t: f32) -> Light {
        let f = |a: f32, b: f32| a + (b - a) * t;
        Light {
            light: self.light.lerp(other.light, t),
            illuminance: f(self.illuminance, other.illuminance),
            horizon: self.horizon.lerp(other.horizon, t),
            zenith: self.zenith.lerp(other.zenith, t),
            glow: self.glow.lerp(other.glow, t),
            shade: self.shade.lerp(other.shade, t),
            ambient: f(self.ambient, other.ambient),
            sky: f(self.sky, other.sky),
            night: f(self.night, other.night),
            exposure: f(self.exposure, other.exposure),
            colour: f(self.colour, other.colour),
        }
    }
}

/// The light at a few heights of the sun, in degrees, from deep night to high noon. The
/// light between is blended. At 20 degrees it is the look the art was made in. By night
/// most light is the sky's, which has no direction, so the moon's shadows are faint.
///
/// The sun's light is as it is above the air: the air (`air`) dims and warms it on its
/// way down, the more the lower the sun, and below the horizon it lights only the sky,
/// as twilight. There it hands over to the moon, which fades in as the sky darkens. Deep
/// night's moon and ambient are half what they were before fire round 10 (median sRGB, share
/// under 20, against RDR2's and Sons of the Forest's campfire frames' 2-27 and 59-88%): the
/// yard's 6 m campfire view 34/28% -> 18/54%, the town's street 24/37% -> 11/82%. At a
/// quarter (round 10) the yard was 10/74% but the town's street went black (4/92%) and the
/// lake's shore and hills to silhouettes (fire round 11). At 0.6 of that (fire round 23), with
/// the campfire's light two and a half stops up by night, the yard's field out of its reach is
/// half as bright (sRGB 14 against 26) and the fire sits in a pool of its own light (blind
/// 5.5/10 against 4.5). Its ambient at a third (fire round 25), that field is sRGB 4.4, not 14:
/// the sky lit the grass to the frame's edge (blind 6/10 against 5).
///
/// Under 20 degrees the sun's colour follows a black body's from about 3,800 K at 12 degrees
/// to 3,000 K at 6 and 2,200 K at the horizon, as a low sun's light measures: at the old
/// near-white keys (green 0.94 at 12 degrees) the dusk film's sunlit meadow under a 13-degree
/// sun stayed green over red (sRGB 76, 89, 60) and the end read as afternoon (settlement r21).
fn keys() -> [(f32, Light); 7] {
    let v = Vec3::new;
    let moon = v(0.62, 0.72, 1.0);
    [
        (
            -18.0,
            Light {
                light: moon,
                illuminance: 210.0,
                horizon: v(0.07, 0.09, 0.17),
                zenith: v(0.015, 0.02, 0.06),
                glow: v(0.25, 0.3, 0.45),
                shade: v(0.42, 0.52, 0.9),
                ambient: 30.0,
                sky: 1.6,
                night: 1.0,
                exposure: 9.0,
                colour: 1.0,
            },
        ),
        (
            -6.0,
            Light {
                light: moon,
                illuminance: 125.0,
                horizon: v(0.32, 0.27, 0.42),
                zenith: v(0.07, 0.09, 0.22),
                glow: v(0.85, 0.45, 0.35),
                shade: v(0.48, 0.52, 0.88),
                ambient: 600.0,
                sky: 1.6,
                night: 0.6,
                exposure: 9.0,
                colour: 0.8,
            },
        ),
        (
            0.0,
            Light {
                light: v(1.0, 0.6, 0.34),
                illuminance: 30_000.0,
                horizon: v(0.95, 0.55, 0.38),
                zenith: v(0.16, 0.24, 0.52),
                glow: v(1.0, 0.45, 0.22),
                shade: v(0.8, 0.66, 0.86),
                ambient: 1_150.0,
                sky: 1.7,
                night: 0.0,
                exposure: 10.0,
                colour: 0.74,
            },
        ),
        (
            6.0,
            Light {
                light: v(1.0, 0.69, 0.44),
                illuminance: 30_000.0,
                horizon: v(1.0, 0.64, 0.42),
                zenith: v(0.2, 0.36, 0.72),
                glow: v(1.0, 0.6, 0.32),
                shade: v(0.76, 0.68, 0.92),
                ambient: 1_550.0,
                sky: 1.6,
                night: 0.0,
                exposure: 10.4,
                colour: 0.74,
            },
        ),
        (
            12.0,
            Light {
                light: v(1.0, 0.77, 0.55),
                illuminance: 30_000.0,
                horizon: v(1.0, 0.72, 0.5),
                zenith: v(0.21, 0.4, 0.78),
                glow: v(1.0, 0.6, 0.32),
                shade: v(0.72, 0.72, 0.96),
                ambient: 1_660.0,
                sky: 1.6,
                night: 0.0,
                exposure: 10.4 + (EXPOSURE_EV100 - 10.4) * 0.43,
                colour: 0.84,
            },
        ),
        (
            20.0,
            Light {
                light: v(1.0, 0.96, 0.9),
                illuminance: 30_000.0,
                horizon: v(0.92, 0.86, 0.78),
                zenith: v(0.22, 0.44, 0.84),
                glow: v(1.0, 0.74, 0.46),
                shade: v(0.66, 0.76, 1.0),
                ambient: 1_800.0,
                sky: 1.6,
                night: 0.0,
                exposure: EXPOSURE_EV100,
                colour: 1.0,
            },
        ),
        (
            60.0,
            Light {
                light: v(1.0, 0.98, 0.95),
                illuminance: 32_000.0,
                horizon: v(0.86, 0.9, 0.96),
                zenith: v(0.2, 0.44, 0.86),
                glow: v(1.0, 0.92, 0.8),
                shade: v(0.68, 0.78, 1.0),
                ambient: 2_000.0,
                sky: 1.6,
                night: 0.0,
                exposure: EXPOSURE_EV100 + 1.0,
                colour: 1.0,
            },
        ),
    ]
}

/// The haze's colour and how far one sees through it, for the horizon's colour and how much
/// of the sky is open over the eye (D224, D273): the forest's own shaded green air under a
/// closed canopy, the open air's in the open.
fn haze_for(horizon: Vec3, open: f32, air: crate::air::AirScale) -> (Vec3, f32) {
    // Only well out from under the trees: in a glade among old oaks, half open, it is still
    // the forest's haze, and it is green, the light come through leaves, not the horizon's
    // blue, which washed the middle distance teal (D279).
    let open = smoothstep(0.55, 0.98, open);
    let shade = horizon.lerp(Vec3::new(0.44, 0.48, 0.3), 0.65) * 0.62;
    // Out in the open as far as the small planet's air let one see, in the air's own metres
    // (D334): on a world whose air is nearer the true air, farther.
    let open_visible = OPEN_VISIBLE_M * crate::air::HAZE_SCALE / air.0;
    let visible = super::scene::HAZE_VISIBLE_M
        * libm::powf(open_visible / super::scene::HAZE_VISIBLE_M, open);
    (shade.lerp(horizon * OPEN_AIR, open), visible)
}

/// The light with the sun so many degrees above the horizon.
fn light_at(height: f32) -> Light {
    let keys = keys();
    let height = height.clamp(keys[0].0, keys[keys.len() - 1].0);
    keys.windows(2)
        .find(|pair| height <= pair[1].0)
        .map(|pair| {
            let t = (height - pair[0].0) / (pair[1].0 - pair[0].0);
            pair[0].1.mix(pair[1].1, t)
        })
        .unwrap_or(keys[keys.len() - 1].1)
}

/// The sun's or the moon's illuminance, lux, with the sun so many degrees up.
#[cfg(test)]
pub(crate) fn illuminance(height: f32) -> f32 {
    light_at(height).illuminance
}

/// The moon's light with the sun so many degrees below the horizon: none as it sets,
/// growing to the night's as the twilight goes.
fn moonlight(height: f32) -> f32 {
    let t = ((-8.0 - height) / 6.0).clamp(0.0, 1.0);
    light_at(height.min(-6.0)).illuminance * t * t * (3.0 - 2.0 * t)
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Clock>()
        .init_resource::<Nightfall>()
        .insert_resource(WorldClimate(crate::data::read(WorldClimate::FILE)))
        .register_type::<Clock>()
        .add_systems(Startup, set_clock)
        .add_systems(PostStartup, find_sky)
        // On the yard, the flat test level, the sun stands still.
        .add_systems(
            Update,
            (
                tick.run_if(crate::shell::in_the_world)
                    .run_if(crate::cine::clock_runs),
                follow_the_date,
                seen_from_the_camera,
                shine,
            )
                .chain()
                .in_set(Ticking),
        );
}

/// The clock moving on and the world lit for it: what reads the clock runs after.
#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Ticking;

/// A scripted run may set the hour and the date; otherwise it is the morning of the clock's
/// first day, or what a save says, or on the yard, noon.
fn set_clock(options: Res<Options>, climate: Res<WorldClimate>, mut clock: ResMut<Clock>) {
    if options.yard() {
        clock.hour = 12.0;
    }
    if let Some(hour) = options.time {
        clock.hour = hour;
    }
    if let Some(date) = options.date {
        clock.day = date.day(&climate.0.year);
    }
}

fn find_sky(
    mut commands: Commands,
    planet: Res<Planet>,
    climate: Res<WorldClimate>,
    clock: Res<Clock>,
) {
    let path = SkyPath::over(planet.start(), climate.declination(0));
    commands.insert_resource(path.on(climate.declination(clock.day)));
}

/// The sun's path follows the date, moving on once a day, at midnight.
fn follow_the_date(clock: Res<Clock>, climate: Res<WorldClimate>, path: Option<ResMut<SkyPath>>) {
    let Some(mut path) = path else { return };
    let declination = climate.declination(clock.day);
    if path.declination != declination {
        *path = path.on(declination);
    }
}

/// The sky is seen from where the camera is: how high the sun stands there, and so the light,
/// is its latitude's. Taken again once the camera has gone a twentieth of a degree round the
/// world, so a walk does not mark the sky changed every frame.
fn seen_from_the_camera(
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    path: Option<ResMut<SkyPath>>,
    planet: Res<Planet>,
    options: Res<Options>,
) {
    // On a world too small to be to scale the sky is the start's everywhere, as it always was:
    // a few hundred metres there would be degrees of latitude. The yard's sun stands still.
    if !terra_land::frame::has_latitudes(planet.radius_m()) || options.yard() {
        return;
    }
    let (Some(camera), Some(mut path)) = (camera, path) else {
        return;
    };
    let up = camera.translation().normalize_or(path.up);
    if up.dot(path.seen_from) < libm::cosf(0.05_f32.to_radians()) {
        *path = path.seen_from(up);
    }
}

fn tick(time: Res<Time>, mut clock: ResMut<Clock>) {
    clock.advance(time.delta_secs());
}

/// Lights the world for the hour: the sun or the moon, the sky and the shade.
fn shine(
    clock: Res<Clock>,
    path: Option<Res<SkyPath>>,
    mut tweaks: ResMut<Tweaks>,
    mut sky: ResMut<Sky>,
    mut skylight: ResMut<SkyLight>,
    mut exposure: Query<&mut Exposure, With<MainCamera>>,
    mut haze: Query<&mut DistanceFog, With<MainCamera>>,
    open: Option<Res<crate::sky_occlusion::OpenOverhead>>,
    mut mist: Query<&mut bevy::light::FogVolume>,
    mut grading: Query<&mut bevy::render::view::ColorGrading, With<MainCamera>>,
    mut volumetric: Query<&mut bevy::light::VolumetricFog, With<MainCamera>>,
    mut nightfall: ResMut<Nightfall>,
    air_scale: Option<Res<crate::air::AirScale>>,
    (eye, planet, weather, cast, snow_opens): (
        Query<&GlobalTransform, With<MainCamera>>,
        Option<Res<Planet>>,
        Option<Res<crate::weather::Weather>>,
        Option<Res<crate::weather::WeatherGrade>>,
        Option<Res<crate::weather::SnowOpens>>,
    ),
    mut clear_sky: Local<Option<f32>>,
) {
    let Some(path) = path else { return };
    // The sun's shadows move in small steps rather than every frame: a light turning by a
    // sliver each frame redraws every shadow edge a little differently, and they crawl.
    let sun = path.sun((clock.hour * SUN_STEPS).round() / SUN_STEPS);
    let height = path.height(clock.hour);
    let light = light_at(height);
    let colour = |rgb: Vec3| Color::srgb(rgb.x, rgb.y, rgb.z);
    // Under cloud (D418): the sun's beam thins to nothing under a deck, the sky's light stays
    // near a clear sky's, and the shade takes the deck's grey.
    let weather = weather.as_deref().copied().unwrap_or_default();
    let under = weather.light();
    let deck = crate::weather::deck_cover(weather.cloud);
    let up = eye
        .iter()
        .next()
        .map_or(Vec3::Y, |eye| eye.translation().normalize_or(Vec3::Y));
    let at = (clock.hour * SUN_STEPS).round() / SUN_STEPS;
    light_the_world(
        &mut tweaks,
        &path,
        (sun, at, height, up),
        &light,
        under,
        deck,
    );
    sky.horizon = colour(light.horizon);
    // The haze between the trees (D224): the horizon's colour, greyed toward the green the
    // canopy lets through and dimmed by the shade it hangs in, so trees fade into it with
    // distance as they do in a real forest, before any of their levels of detail change.
    // The hour in the air (D244): the mist thickest at dawn, burning off by midday and
    // gathering a little toward evening, so a morning and an evening do not look alike; the
    // light, not a grade, warms as the sun goes low (D370).
    let hour = path.local_hour(clock.hour);
    let morning = 1.0 - smoothstep(7.0, 11.5, hour);
    let evening = smoothstep(16.0, 19.5, hour);
    let thick = 1.0 + 1.2 * morning * smoothstep(4.5, 6.5, hour) + 0.35 * evening;
    // An eye risen over the mist sees it from above (D273).
    let above = eye
        .iter()
        .next()
        .zip(planet.as_deref())
        .map_or(1.0, |(eye, planet)| {
            let at = eye.translation();
            above_mist(at.length() - planet.ground_at(at).length())
        });
    for mut fog in &mut mist {
        let wanted = super::scene::HAZE_DENSITY * thick * above;
        if (fog.density_factor - wanted).abs() > 1e-5 {
            fog.density_factor = wanted;
        }
    }
    let dark = 1.0 - smoothstep(-12.0, 0.0, height);
    nightfall.set_if_neq(Nightfall(dark));
    let cast = cast.map(|c| c.0).unwrap_or_default();
    for mut grade in &mut grading {
        fade_colour(&mut grade, height, light.colour, cast);
    }
    // Out from under the trees, where the sky over the eye is open, the forest's haze gives
    // way to the open air's (D273): thinner, so the far forest and the land beyond show, and
    // in the horizon's colour, so they fade toward the sky rather than into a dark band. The
    // air's own haze (`air`) carries the distance from there. A world with no trees to hide
    // the sky keeps the forest's haze on the small planet, as it was tuned; on a world whose
    // views reach kilometres (D334) its far ground stands in the open air.
    let far_seeing = air_scale
        .as_deref()
        .is_some_and(|scale| scale.0 < crate::air::HAZE_SCALE);
    let open = open.map_or(if far_seeing { 1.0 } else { 0.0 }, |open| open.0);
    let night_air = 1.0 - smoothstep(-12.0, 0.0, path.height(clock.hour));
    mist_light(&mut volumetric, light.shade, open, night_air);
    // Bevy mixes the haze in after the exposure, at the brightness it is given: left at the
    // day's, by night it lit every shadow from within, the night only two stops under the day
    // (D310). So it dims as night falls, toward the moon's cold light.
    let level = NIGHT_HAZE + (1.0 - NIGHT_HAZE) * (1.0 - dark);
    // Snow lying round the eye opens the camera about half a stop, so it reads white (D419).
    let snow_opens = snow_opens.map_or(0.0, |s| s.0);
    let set = open_exposure(light.exposure, open, far_seeing, dark) + under.opens_ev() - snow_opens;
    for mut fog in &mut haze {
        let air_scale = air_scale
            .as_deref()
            .copied()
            .unwrap_or(crate::air::AirScale(crate::air::HAZE_SCALE));
        let (air, visible) = haze_for(light.horizon, open, air_scale);
        // Under a deck the haze is the deck's light on the air, as bright on screen as the
        // deck at the horizon, so the far land fades into the sky rather than a band; what
        // falls shortens how far one sees (D418). Among the trees it is the air under the
        // crowns, lit by the sky they let through, as the forest's own haze is: at the deck's
        // full light it stood at luma 140 over snow at 87, and the snow read grey.
        let wooded = 1.0 - smoothstep(0.55, 0.98, open);
        let veil = crate::weather::veil(light.ambient * SKY_LIGHT_GAIN * under.diffuse, set)
            * (1.0 - (1.0 - WOODED_VEIL) * wooded);
        let air = air.lerp(MOON_HAZE, dark) * level;
        fog.color = colour(air.lerp(veil, deck));
        fog.directional_light_color = colour(light.light * 0.55 * under.direct).with_alpha(0.5);
        let visible = weather.visibility_m().map_or(visible, |v| v.min(visible));
        fog.falloff = bevy::pbr::FogFalloff::from_visibility_squared(visible);
    }
    let base = *clear_sky.get_or_insert(sky.cloudiness);
    let cloudiness = base.max(weather.cloud);
    if sky.cloudiness != cloudiness {
        sky.cloudiness = cloudiness;
    }
    sky.deck = crate::weather::deck(&weather, light.ambient * SKY_LIGHT_GAIN * under.diffuse);
    sky.zenith = colour(light.zenith);
    sky.glow = colour(light.glow);
    sky.brightness = light.sky;
    sky.night = light.night;
    sky.twilight = 1.0 - smoothstep(-1.0, 6.0, height);
    sky.sun = sun;
    sky.spin_axis = path.pole();
    sky.spin = (clock.hour - MORNING) / 24.0 * TAU;
    let lit = SkyLight {
        shade: colour(crate::weather::under_deck(light.shade, deck)),
        brightness: light.ambient * SKY_LIGHT_GAIN * under.diffuse,
    };
    // Set only when it changes, so the sky is painted again only then.
    skylight.set_if_neq(lit);
    for mut camera in &mut exposure {
        let ev = set;
        if camera.ev100 != ev {
            camera.ev100 = ev;
        }
    }
}

/// The volumetric haze's own sky light (D310): under the trees the little that
/// comes through the leaves, green; out in the open the whole sky's, its shade's colour,
/// so mist over a lake glows pale in the morning seen with the sun behind, not hanging
/// as a dark brown smear where no sunbeam crosses it.
fn mist_light(
    volumetric: &mut Query<&mut bevy::light::VolumetricFog, With<MainCamera>>,
    shade: Vec3,
    open: f32,
    night_air: f32,
) {
    for mut air in volumetric.iter_mut() {
        let wanted = super::scene::HAZE_AMBIENT
            * (1.0 + (OPEN_HAZE_GLOW - 1.0) * open)
            * (NIGHT_HAZE + (1.0 - NIGHT_HAZE) * (1.0 - night_air));
        if (air.ambient_intensity - wanted).abs() > 1e-6 {
            air.ambient_intensity = wanted;
        }
        // The open sky's light on the mist is skylight, on the daylight locus by day, as the
        // painted sky's is (`skylight`): the low sun's lilac shade key tinted the mist lilac.
        let day = 1.0 - night_air;
        let sky_tint = shade.lerp(crate::skylight::srgb_on_daylight_locus(shade), day);
        let tint = super::scene::HAZE_TINT.lerp(sky_tint, open);
        let tint = Color::srgb(tint.x, tint.y, tint.z);
        if air.ambient_color != tint {
            air.ambient_color = tint;
        }
    }
}

/// The world's light: above the horizon the sun's, below it the moon's, its beam as thin as
/// the cloud leaves it, and under a deck the key from high overhead (`keyed`), for the sun
/// so many degrees up at `height`, seen by an eye whose up is `up`.
fn light_the_world(
    tweaks: &mut Tweaks,
    path: &SkyPath,
    (sun, at, height, up): (Vec3, f32, f32, Vec3),
    light: &Light,
    under: crate::weather::Under,
    deck: f32,
) {
    tweaks.sun_direction = if height > -1.0 { sun } else { path.moon(at) };
    tweaks.sun_color = Color::srgb(light.light.x, light.light.y, light.light.z);
    let (direct, toward) = keyed(under, deck, tweaks.sun_direction, up);
    let beam = if height > -1.0 {
        light.illuminance
    } else {
        moonlight(height)
    };
    tweaks.sun_illuminance = direct * beam;
    tweaks.light_direction = toward;
    tweaks.light_size = light_size(deck);
}

/// How wide the world's light is as a source under a deck so much of the sky: the sun's own
/// disc under a clear sky, crisp; under a deck the key is the sky's light, so what the crowns
/// cast is a soft shade, not the sun's dapples (as Unreal's directional light's Source Angle
/// widens under overcast).
fn light_size(deck: f32) -> f32 {
    DECK_LIGHT_SIZE * deck
}

/// Under a deck the sky is brightest overhead (CIE overcast, `skylight`), and what faces up
/// takes that light where nothing over it hides it: a crown's top, not its underside. The
/// irradiance volume's cells are too coarse to see that, so a share of the deck's light comes
/// as a key from high over the sun's side, and the crowns shade what lies under them.
/// The world's light as a share of the sun's beam under a clear sky, and the direction it
/// comes from: the sun's beam, and under a deck a key from high overhead toward the sun's side.
fn keyed(under: crate::weather::Under, deck: f32, sun: Vec3, up: Vec3) -> (f32, Vec3) {
    let key = OVERHEAD_KEY * deck * under.total;
    let direct = under.direct + key;
    let turned = key / direct.max(1e-6);
    (direct, sun.lerp(up, turned * KEY_UP).normalize_or(up))
}

/// Colour fading as the light goes (D302), for the sun so many degrees up.
/// A weather may carry its own grade over it (`cast`, D419).
fn fade_colour(
    grade: &mut bevy::render::view::ColorGrading,
    height: f32,
    kept: f32,
    cast: crate::weather::Grade,
) {
    // The grade's white balance is left neutral (`scene`) at every hour and everywhere but
    // in a weather with its own (falling snow grades cool, D419), as
    // a camera balanced for daylight (D65) sees (D370): a grey card in the noon sun stays
    // grey, the sky's shade is blue, the low sun warm and the light through leaves green,
    // all by the light itself. By night colour fades, as an eye's does in the dark (D302),
    // and the dim first: from sunset the land in the shade loses its colour, as an eye's
    // rods take over in dim light (mesopic vision), while the bright sky and a fire keep
    // theirs until the twilight goes. Faded all at once, from 4 degrees under, a dusk
    // under a pink sky was the noon's green land dimmed or a grey sky (lake film r7).
    // By night only the dim loses its colour, the whole picture's fade on its shadows: a
    // fire keeps its colour on what it lights, as an eye's cones see by a fire's light while
    // its rods see the moonlit land round it. Faded over the whole picture, the ground a
    // campfire lit at night kept 7% of its colour and stood grey (fire sheet, round 6).
    let day = smoothstep(-12.0, -4.0, height);
    let night = NIGHT_SATURATION + (1.0 - NIGHT_SATURATION) * day;
    let saturation = super::scene::SATURATION * cast.saturation;
    if (grade.global.post_saturation - saturation).abs() > 1e-4 {
        grade.global.post_saturation = saturation;
    }
    if (grade.global.temperature - cast.temperature).abs() > 1e-5 {
        grade.global.temperature = cast.temperature;
    }
    if (grade.global.tint - cast.tint).abs() > 1e-5 {
        grade.global.tint = cast.tint;
    }
    let lit = smoothstep(-3.0, 3.0, height);
    let dim = (DIM_SATURATION + (1.0 - DIM_SATURATION) * lit) * night * kept;
    if (grade.midtones.saturation - kept).abs() > 1e-4 {
        grade.midtones.saturation = kept;
    }
    if (grade.shadows.saturation - dim).abs() > 1e-4 {
        grade.shadows.saturation = dim;
    }
    // By night the dim is the moonlit, not what a fire lights: the midtones start lower, so
    // ground a campfire lights (level 0.08 to 0.2) keeps its colour while the moonlit land
    // (0.02) stays faded. From Bevy's 0.2 the firelit yard kept 9% of its colour and stood
    // grey (R/B 1.1 at 1.5 m; 0.12: 3.6; 0.05 coloured the moonlit grass green, fire round 7).
    let start = 0.2 + (NIGHT_MIDTONES - 0.2) * (1.0 - lit);
    if (grade.global.midtones_range.start - start).abs() > 1e-4 {
        grade.global.midtones_range.start = start;
    }
    // The same lift for every section: Bevy blends its sections at their edges, so one lifted
    // alone would step where the shadows end.
    let lift = DUSK_LIFT * (1.0 - lit) * day;
    for section in [
        &mut grade.shadows,
        &mut grade.midtones,
        &mut grade.highlights,
    ] {
        if (section.lift - lift).abs() > 1e-5 {
            section.lift = lift;
        }
    }
}

/// How thick the volumetric haze is drawn for an eye so many metres over the ground, as a
/// share of how thick it is round a walker (D273). The haze box stands for the air between
/// the trees and the mist lying in their hollows, seen from among them; an eye risen over
/// the mist, as a bird's-eye time-lapse's or a hover's camera 15 to 30 m up, looks down
/// through clear air onto it, as a drone over a morning meadow sees the ground through its
/// mist. Drawn as thick as round a walker, every slant ray from up there crossed some 20 m
/// of mist glowing with the open sky's light and the ground below was a flat yellow veil.
fn above_mist(height_m: f32) -> f32 {
    1.0 - ABOVE_MIST_THINS * smoothstep(MIST_TOP_M, OVER_MIST_M, height_m)
}

/// The mist's top over the ground (scene.rs `haze_density`'s band), and the height by which an
/// eye looks down on it from clear air, the haze then this much thinner.
const MIST_TOP_M: f32 = 6.0;
const OVER_MIST_M: f32 = 28.0;
const ABOVE_MIST_THINS: f32 = 0.6;

/// How many times brighter the volumetric haze's sky light is out in the open than under
/// the trees: the open sky lights mist from every side.
const OPEN_HAZE_GLOW: f32 = 4.0;
/// Under a deck, how bright the haze among the trees is beside the open air's: the forest's
/// own haze is dimmed so by the shade it hangs in (`haze_for`).
const WOODED_VEIL: f32 = 0.62;
/// Of the light under a full deck, the share that comes as a key from high overhead. At 0.3
/// the snowfall's crowns took luma 197 at their 95th percentile (172 without), and the snow
/// read white over the trunks' darks (weather round 20).
const OVERHEAD_KEY: f32 = 0.3;
/// How far toward the zenith that key turns from the sun's direction.
const KEY_UP: f32 = 0.8;
/// How wide the key is as a source under a full deck, as Bevy's PCSS light size. From 10 up
/// the crowns' dapples on the snow went to a soft shade, keeping a trunk's contact darkening;
/// 40 won blind over the crisp key (weather round 21).
const DECK_LIGHT_SIZE: f32 = 40.0;

/// How many stops brighter the eye is set by day out in the open on a world whose views reach
/// kilometres (D338). The day's exposures were tuned under the lush forest's canopy against
/// Sons of the Forest; out over the mountain lake at noon they left the sky at 0.17 where the
/// references' is 0.73, and the whole frame at 0.24 against 0.39. A stop and a half brighter
/// measured 0.55 and 0.45, as a photographer opens up for a bright open view a forest's
/// meter would stop down for.
const OPEN_EYE_EV: f32 = 1.5;
/// And by night, on the open moonlit land of a far-seeing world (fire round 24): with the deep
/// night's moon and ambient at 0.6 (`keys`), tuned for the yard's campfire to sit in a pool of
/// its own light, the town's street and the mountain lake's shore went to silhouettes (the lake
/// at mean sRGB 2-5, every pixel under 8%). The small planet's yard, where the campfire's pool
/// is judged, keeps the key. With the ambient at a third (round 25), 1.45, under the day's: at
/// 1.6 the town's street was mean sRGB 14, 84% under 8%, against 18/69%, the lake 5.8 v 4.4.
const OPEN_NIGHT_EV: f32 = 1.45;

/// The camera's exposure for the hour's `key`, with the sky so `open` over the eye, on a world
/// that sees far or not, so far into the night: the key in a forest, under trees and by night;
/// opened up in the open air of a far-seeing world, by day and less by night.
fn open_exposure(key: f32, open: f32, far_seeing: bool, night: f32) -> f32 {
    if !far_seeing {
        return key;
    }
    let night = night.clamp(0.0, 1.0);
    key - smoothstep(0.55, 0.98, open) * (OPEN_EYE_EV * (1.0 - night) + OPEN_NIGHT_EV * night)
}

#[cfg(test)]
mod tests;
