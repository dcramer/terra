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

/// How long a whole day and night lasts, in seconds of play. Proposed: half an hour, as
/// Valheim's and Enshrouded's are.
pub const DAY_SECONDS: f32 = 30.0 * 60.0;
/// The sky lights the shade this many times as brightly as the keys below say. Open sky
/// gives level ground something like a sixth of what the sun does; the keys gave it about a
/// sixteenth, so every shadow was near black (D216). Under trees the shade is darker than open
/// ground's: 1.6 left it three stops brighter than Sons of the Forest's (D286).
const SKY_LIGHT_GAIN: f32 = 1.2;
/// By night, how much of the day's saturation is left, how much cooler the grade is, and how
/// much toward magenta, which the leaves' green bounce otherwise tints (D302, D310).
const NIGHT_SATURATION: f32 = 0.45;
const NIGHT_COOL: f32 = 0.12;
const NIGHT_TINT: f32 = 0.04;
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
        }
    }
}

/// The light at a few heights of the sun, in degrees, from deep night to high noon. The
/// light between is blended. At 20 degrees it is the look the art was made in. By night
/// most light is the sky's, which has no direction, so the moon's shadows are faint.
///
/// The sun's light is as it is above the air: the air (`air`) dims and warms it on its
/// way down, the more the lower the sun, and below the horizon it lights only the sky,
/// as twilight. There it hands over to the moon, which fades in as the sky darkens.
fn keys() -> [(f32, Light); 6] {
    let v = Vec3::new;
    let moon = v(0.62, 0.72, 1.0);
    [
        (
            -18.0,
            Light {
                light: moon,
                illuminance: 700.0,
                horizon: v(0.07, 0.09, 0.17),
                zenith: v(0.015, 0.02, 0.06),
                glow: v(0.25, 0.3, 0.45),
                shade: v(0.42, 0.52, 0.9),
                ambient: 300.0,
                sky: 1.6,
                night: 1.0,
                exposure: 9.0,
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
            },
        ),
        (
            0.0,
            Light {
                light: v(1.0, 0.85, 0.7),
                illuminance: 30_000.0,
                horizon: v(0.95, 0.55, 0.38),
                zenith: v(0.16, 0.24, 0.52),
                glow: v(1.0, 0.45, 0.22),
                shade: v(0.68, 0.66, 0.9),
                ambient: 1_150.0,
                sky: 1.7,
                night: 0.0,
                exposure: 10.0,
            },
        ),
        (
            6.0,
            Light {
                light: v(1.0, 0.92, 0.84),
                illuminance: 30_000.0,
                horizon: v(1.0, 0.72, 0.52),
                zenith: v(0.2, 0.36, 0.72),
                glow: v(1.0, 0.6, 0.32),
                shade: v(0.66, 0.72, 0.98),
                ambient: 1_550.0,
                sky: 1.6,
                night: 0.0,
                exposure: 10.4,
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

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
) {
    let Some(path) = path else { return };
    // The sun's shadows move in small steps rather than every frame: a light turning by a
    // sliver each frame redraws every shadow edge a little differently, and they crawl.
    let sun = path.sun((clock.hour * SUN_STEPS).round() / SUN_STEPS);
    let height = path.height(clock.hour);
    let light = light_at(height);
    let colour = |rgb: Vec3| Color::srgb(rgb.x, rgb.y, rgb.z);
    // Above the horizon the light comes from the sun; below it, from the moon.
    tweaks.sun_direction = if height > -1.0 {
        sun
    } else {
        path.moon((clock.hour * SUN_STEPS).round() / SUN_STEPS)
    };
    tweaks.sun_color = colour(light.light);
    tweaks.sun_illuminance = if height > -1.0 {
        light.illuminance
    } else {
        moonlight(height)
    };
    sky.horizon = colour(light.horizon);
    // The haze between the trees (D224): the horizon's colour, greyed toward the green the
    // canopy lets through and dimmed by the shade it hangs in, so trees fade into it with
    // distance as they do in a real forest, before any of their levels of detail change.
    // The hour in the air (D244): the mist thickest at dawn, burning off by midday and
    // gathering a little toward evening; and the grade warm while the sun is low, cooler at
    // noon, so a morning and an evening do not look alike.
    let hour = path.local_hour(clock.hour);
    let morning = 1.0 - smoothstep(7.0, 11.5, hour);
    let evening = smoothstep(16.0, 19.5, hour);
    let thick = 1.0 + 1.2 * morning * smoothstep(4.5, 6.5, hour) + 0.35 * evening;
    for mut fog in &mut mist {
        let wanted = super::scene::HAZE_DENSITY * thick;
        if (fog.density_factor - wanted).abs() > 1e-5 {
            fog.density_factor = wanted;
        }
    }
    let low = 1.0 - smoothstep(8.0, 30.0, height.max(0.0));
    let dark = 1.0 - smoothstep(-12.0, 0.0, height);
    nightfall.set_if_neq(Nightfall(dark));
    // How closed a canopy is measured over the eye: none where nothing measures one.
    let canopy = open
        .as_deref()
        .map_or(0.0, |open| 1.0 - smoothstep(0.55, 0.98, open.0));
    for mut grade in &mut grading {
        // The day's warmth goes with the sun (D302): left on, it turned the moon's blue light
        // to brown, and the night read as a dim dusk.
        let day = smoothstep(-4.0, 2.0, height);
        // By night colour fades and cools, as an eye sees by moonlight and as games draw it: the
        // light under the moon is the leaves' green bounce, and it read green and murky.
        // In the open a grey card stays grey at noon (D369).
        let warmth = day_warmth(canopy, low, hour > 12.0);
        let warm = warmth.temperature * day - NIGHT_COOL * (1.0 - day);
        if (grade.global.temperature - warm).abs() > 1e-4 {
            grade.global.temperature = warm;
        }
        let tint = warmth.tint * day + NIGHT_TINT * (1.0 - day);
        if (grade.global.tint - tint).abs() > 1e-4 {
            grade.global.tint = tint;
        }
        let saturation =
            super::scene::SATURATION * (NIGHT_SATURATION + (1.0 - NIGHT_SATURATION) * day);
        if (grade.global.post_saturation - saturation).abs() > 1e-4 {
            grade.global.post_saturation = saturation;
        }
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
    // The volumetric haze's own sky light, likewise (D310): under the trees the little that
    // comes through the leaves, green; out in the open the whole sky's, its shade's colour,
    // so mist over a lake glows pale in the morning seen with the sun behind, not hanging
    // as a dark brown smear where no sunbeam crosses it.
    let night_air = 1.0 - smoothstep(-12.0, 0.0, path.height(clock.hour));
    for mut air in &mut volumetric {
        let wanted = super::scene::HAZE_AMBIENT
            * (1.0 + (OPEN_HAZE_GLOW - 1.0) * open)
            * (NIGHT_HAZE + (1.0 - NIGHT_HAZE) * (1.0 - night_air));
        if (air.ambient_intensity - wanted).abs() > 1e-6 {
            air.ambient_intensity = wanted;
        }
        let tint = super::scene::HAZE_TINT.lerp(light.shade, open);
        let tint = colour(tint);
        if air.ambient_color != tint {
            air.ambient_color = tint;
        }
    }
    // Bevy mixes the haze in after the exposure, at the brightness it is given: left at the
    // day's, by night it lit every shadow from within, the night only two stops under the day
    // (D310). So it dims as night falls, toward the moon's cold light.
    let level = NIGHT_HAZE + (1.0 - NIGHT_HAZE) * (1.0 - dark);
    for mut fog in &mut haze {
        let air_scale = air_scale
            .as_deref()
            .copied()
            .unwrap_or(crate::air::AirScale(crate::air::HAZE_SCALE));
        let (air, visible) = haze_for(light.horizon, open, air_scale);
        fog.color = colour(air.lerp(MOON_HAZE, dark) * level);
        fog.directional_light_color = colour(light.light * 0.55).with_alpha(0.5);
        fog.falloff = bevy::pbr::FogFalloff::from_visibility_squared(visible);
    }
    sky.zenith = colour(light.zenith);
    sky.glow = colour(light.glow);
    sky.brightness = light.sky;
    sky.night = light.night;
    sky.twilight = 1.0 - smoothstep(-1.0, 6.0, height);
    sky.spin_axis = path.pole();
    sky.spin = (clock.hour - MORNING) / 24.0 * TAU;
    let lit = SkyLight {
        shade: colour(light.shade),
        brightness: light.ambient * SKY_LIGHT_GAIN,
    };
    // Set only when it changes, so the sky is painted again only then.
    skylight.set_if_neq(lit);
    let set = open_exposure(light.exposure, open, far_seeing, dark);
    for mut camera in &mut exposure {
        let ev = set;
        if camera.ev100 != ev {
            camera.ev100 = ev;
        }
    }
}

/// The day's grade under a closed canopy: the picture a touch warmer, as tuned against Sons of
/// the Forest's frames (D286), as a camera balancing the leaves' green light would. In the open
/// a grey card at noon stays grey, as a camera balanced for daylight sees it (D369).
const CANOPY_WARM: f32 = 0.02;
/// And while the sun is low, warmer, in the afternoon twice the morning's (D244), along the
/// daylight locus so it reads amber rather than pink (D369).
const LOW_SUN_WARM: f32 = 0.012;
/// How far the daylight locus falls in CIE y as it falls in x, toward bluer white points:
/// from 6,500 K to 7,500 K, x 0.3127 to 0.2990 and y 0.3290 to 0.3150.
const LOCUS_SLOPE: f32 = 1.02;

/// The grade's white balance by day, as Bevy's `temperature` and `tint`: how far the white
/// point is taken from D65 in CIE x (warmer for more) and y (more magenta for more).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Warmth {
    temperature: f32,
    tint: f32,
}

/// The day's white balance with so closed a canopy over the eye (0 to 1), the sun so `low`
/// (0 high, 1 at the horizon), in the afternoon or not. The canopy's warmth moves the white
/// point in x alone, as tuned, leaning magenta against the leaves' green; the low sun's moves
/// it along the daylight locus. Moved in x alone in the open it turned a grey card pink at
/// noon and the sky-lit shade lilac (D369).
fn day_warmth(canopy: f32, low: f32, afternoon: bool) -> Warmth {
    let sun = LOW_SUN_WARM * low * if afternoon { 1.0 } else { 0.5 };
    Warmth {
        temperature: CANOPY_WARM * canopy + sun,
        tint: -LOCUS_SLOPE * sun,
    }
}

/// How many times brighter the volumetric haze's sky light is out in the open than under
/// the trees: the open sky lights mist from every side.
const OPEN_HAZE_GLOW: f32 = 4.0;

/// How many stops brighter the eye is set by day out in the open on a world whose views reach
/// kilometres (D338). The day's exposures were tuned under the lush forest's canopy against
/// Sons of the Forest; out over the mountain lake at noon they left the sky at 0.17 where the
/// references' is 0.73, and the whole frame at 0.24 against 0.39. A stop and a half brighter
/// measured 0.55 and 0.45, as a photographer opens up for a bright open view a forest's
/// meter would stop down for.
const OPEN_EYE_EV: f32 = 1.5;

/// The camera's exposure for the hour's `key`, with the sky so `open` over the eye, on a world
/// that sees far or not, so far into the night: the key in a forest, under trees and by night;
/// opened up by day in the open air of a far-seeing world.
fn open_exposure(key: f32, open: f32, far_seeing: bool, night: f32) -> f32 {
    if !far_seeing {
        return key;
    }
    key - OPEN_EYE_EV * smoothstep(0.55, 0.98, open) * (1.0 - night)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::SUN_DIRECTION;

    #[test]
    fn the_open_air_at_noon_is_graded_neutral_and_the_low_sun_amber_not_pink() {
        // A grey card in the open with the sun high: the white point left at D65.
        assert_eq!(day_warmth(0.0, 0.0, true), Warmth::default());
        // Under a closed canopy the picture is warmer.
        assert!(day_warmth(1.0, 0.0, true).temperature > 0.0);
        // A low sun warms it by moving the white point toward a bluer daylight, as from D65
        // (CIE x 0.3127, y 0.3290) toward D75 (0.2990, 0.3149): within a few degrees of that
        // line, not along x alone, which tints pink.
        let low = day_warmth(0.0, 1.0, true);
        let moved = Vec2::new(-low.temperature, low.tint);
        let locus = Vec2::new(0.2990 - 0.3127, 0.3149 - 0.3290);
        assert!(moved.length() > 0.0);
        assert!(
            moved.angle_to(locus).abs() < 5_f32.to_radians(),
            "{moved} against {locus}"
        );
        // And more in the evening than the morning.
        assert!(low.temperature > day_warmth(0.0, 1.0, false).temperature);
    }

    fn path() -> SkyPath {
        SkyPath::over(Vec3::new(0.1, 1.0, 0.2), climate().declination(0))
    }

    #[test]
    fn the_eye_opens_up_by_day_in_the_open_of_a_far_seeing_world_and_nowhere_else() {
        let key = 11.8;
        // The lush forest's world and any world's forest keep the key; so does the night.
        assert_eq!(open_exposure(key, 1.0, false, 0.0), key);
        assert_eq!(open_exposure(key, 0.2, true, 0.0), key);
        assert_eq!(open_exposure(key, 1.0, true, 1.0), key);
        // Out on a mountain lake's shore by day, brighter: a lower EV.
        let open = open_exposure(key, 1.0, true, 0.0);
        assert!(open < key - 1.0, "{open}");
        // And smoothly between, as one walks out of the trees.
        let mut last = key;
        for k in 0..=20 {
            let now = open_exposure(key, k as f32 / 20.0, true, 0.0);
            assert!(now <= last && last - now < 0.3, "{last} then {now}");
            last = now;
        }
    }

    #[test]
    fn out_of_the_trees_the_haze_is_thinner_and_takes_the_horizons_colour() {
        let horizon = Vec3::new(0.9, 0.86, 0.8);
        let small = crate::air::AirScale(crate::air::HAZE_SCALE);
        let (under, near) = haze_for(horizon, 0.1, small);
        let (out, far) = haze_for(horizon, 1.0, small);
        assert_eq!(near, super::super::scene::HAZE_VISIBLE_M);
        assert!(far > near * 8.0, "{far} against {near}");
        // Under the canopy green and dim; out of it the horizon's own hue.
        assert!(under.y > under.x && under.y > under.z, "{under}");
        let hue = |c: Vec3| c / c.max_element();
        assert!(
            (hue(out) - hue(horizon)).abs().max_element() < 1e-4,
            "{out}"
        );
        // Between, it changes smoothly as the eye walks out.
        let mut last = near;
        for k in 1..=20 {
            let (_, now) = haze_for(horizon, k as f32 / 20.0, small);
            assert!(now >= last && now < last * 1.6, "{last} then {now}");
            last = now;
        }
    }

    #[test]
    fn the_sun_rises_stands_high_at_noon_and_sets() {
        let path = path();
        assert!(path.height(MORNING) > 10.0 && path.height(MORNING) < 30.0);
        assert!(
            path.height(9.0) > path.height(MORNING),
            "the morning sun climbs"
        );
        // At noon the sun is as far from overhead as the latitude, less its declination.
        let noon = path.height(12.0);
        let highest = 90.0 - LATITUDE.to_degrees() + path.declination.to_degrees();
        assert!((noon - highest).abs() < 0.5, "noon {noon}, not {highest}");
        assert!(path.height(0.0) < -30.0, "midnight is dark");
        // As long up as the sunrise equation says: cos h = -tan latitude tan declination,
        // with h the hour angle from sunrise to noon. In summer, longer than the night.
        let sunrise = libm::acosf(-libm::tanf(LATITUDE) * libm::tanf(path.declination));
        let day_tenths = sunrise / TAU * 2.0 * 240.0;
        let up = (0..240)
            .filter(|i| path.height(*i as f32 / 10.0) > 0.0)
            .count();
        assert!(
            (up as f32 - day_tenths).abs() <= 2.0,
            "{up} tenths of an hour of day, not {day_tenths}"
        );
        assert!(up > 120);
        // East in the morning, west in the evening.
        assert!(path.sun(7.0).dot(path.east) > 0.5);
        assert!(path.sun(17.0).dot(path.east) < -0.5);
    }

    #[test]
    fn the_morning_sun_stands_where_the_art_put_it() {
        let path = path();
        let toward = SUN_DIRECTION.reject_from(path.up).normalize();
        let sun = path.sun(MORNING).reject_from(path.up).normalize();
        assert!(sun.dot(toward) > 0.999);
    }

    #[test]
    fn the_light_is_the_art_at_twenty_degrees_and_dark_at_night() {
        let keys = keys();
        let (_, art) = keys.iter().find(|(height, _)| *height == 20.0).unwrap();
        let (_, deepest) = keys[0];
        let close = |a: f32, b: f32| (a - b).abs() <= 1e-3 * b.abs().max(1.0);
        // At each key the light is the key's, and past the last ones it stays.
        for (height, light) in keys {
            let at = light_at(height);
            assert!(close(at.illuminance, light.illuminance), "{height}");
            assert!(close(at.ambient, light.ambient), "{height}");
            assert!(at.zenith.distance(light.zenith) < 1e-4, "{height}");
        }
        let night = light_at(-90.0);
        assert_eq!(night, deepest);
        assert_eq!(night.night, 1.0);
        assert!(night.illuminance < art.illuminance && night.ambient < art.ambient);
        // Most of the night's light is the moon's, so it lights the night from one side and
        // casts shadows; the sky's light under it is a glow (D310).
        assert!(night.illuminance > night.ambient);
        // Halfway between two keys, the light is halfway between theirs.
        for pair in keys.windows(2) {
            let ((low, a), (high, b)) = (pair[0], pair[1]);
            let between = light_at((low + high) / 2.0);
            let halfway = (a.illuminance + b.illuminance) / 2.0;
            assert!(close(between.illuminance, halfway), "{low} to {high}");
            assert!(close(between.sky, (a.sky + b.sky) / 2.0), "{low} to {high}");
        }
    }

    #[test]
    fn the_moon_rises_from_nothing_as_the_sun_sets() {
        // Not till the dusk has gone, so the afterglow is not lit from the far side (D310).
        assert_eq!(moonlight(-1.0), 0.0);
        assert_eq!(moonlight(-8.0), 0.0);
        assert!(moonlight(-10.0) < moonlight(-13.0));
        assert_eq!(moonlight(-14.0), light_at(-14.0).illuminance);
        assert_eq!(moonlight(-18.0), light_at(-18.0).illuminance);
    }

    #[test]
    fn the_moon_stands_high_on_the_far_side_of_the_sky_from_the_sun() {
        let path = SkyPath::over(Vec3::Y, climate().declination(0));
        for hour in [21.0, 23.5, 1.0, 3.0] {
            let moon = path.moon(hour);
            let up = libm::asinf(moon.dot(Vec3::Y));
            assert!((up - MOON_HEIGHT).abs() < 1e-3, "{hour}: {up}");
            let sun = path.sun(hour).reject_from(Vec3::Y);
            assert!(
                moon.reject_from(Vec3::Y).dot(sun) < 0.0,
                "{hour}: on the sun's side"
            );
        }
    }

    #[test]
    fn a_place_east_of_the_start_keeps_a_later_hour_by_its_own_sun() {
        let path = SkyPath::over(Vec3::Y, climate().declination(160));
        // A place 15 degrees round the pole to the east: its sky an hour on from the start's.
        let east = Quat::from_axis_angle(path.pole(), 15f32.to_radians()) * Vec3::Y;
        assert!(east.dot(path.east) > 0.0, "the turn goes east");
        let there = path.seen_from(east);
        for hour in [6.0, 9.0, 13.0, 17.5] {
            assert!(
                (path.local_hour(hour) - hour).abs() < 1e-4,
                "the start keeps the clock"
            );
            let local = there.local_hour(hour);
            assert!((local - (hour + 1.0)).abs() < 1e-3, "{hour}: {local}");
            // And its sun stands as the start's does at that hour.
            assert!(
                (there.height(hour) - path.height(hour + 1.0)).abs() < 0.05,
                "{hour}"
            );
        }
    }

    #[test]
    fn the_clock_goes_round() {
        let mut clock = Clock { day: 3, hour: 23.9 };
        clock.advance(DAY_SECONDS / 24.0 * 0.2);
        assert!((clock.hour - 0.1).abs() < 1e-3);
        assert_eq!(clock.day, 4);
        assert_eq!(clock.time().day(), 4);
        assert!((clock.time().hour() - 0.1).abs() < 1e-3);
    }

    fn climate() -> WorldClimate {
        WorldClimate(crate::data::read(WorldClimate::FILE))
    }

    #[test]
    fn the_clock_starts_on_the_sun_the_art_was_made_for_and_then_follows_the_year() {
        // The art was made under a sun 10° north of the equator (D315).
        let climate = climate();
        let first = climate.declination(0);
        assert!(
            (first.to_degrees() - 10.0).abs() < 0.1,
            "{}",
            first.to_degrees()
        );
        // Through the year the noon sun climbs higher than that, at midsummer, and lower,
        // at midwinter, and the morning sun stays where the art put it on the first day.
        let year = climate.0.year;
        let after = |phase: f64| {
            let days = ((phase - year.start).rem_euclid(1.0) * year.days) as i64;
            path().on(climate.declination(days)).height(12.0)
        };
        let first_noon = path().height(12.0);
        assert!(after(0.5) > first_noon + 10.0, "{}", after(0.5));
        assert!(after(0.0) < first_noon - 25.0, "{}", after(0.0));
    }
}
