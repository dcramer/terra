//! Day and night (game design 001, survival.md): a clock, the sun crossing the sky by it,
//! and the light, sky and shade following how high the sun is.
//!
//! The sun keeps a real path over the start, as seen from a middle latitude in early
//! summer: it rises in the east, stands high at noon and sets in the west, and the day is
//! a little longer than the night. After it sets, the same light becomes the moon's,
//! cool and dim, from the other side of the sky, so there is one light casting shadows.
//! The stars turn with the sky. A new game starts in the morning, with the low warm
//! light the art was made for.
//!
//! How long a day is, and what night does besides being dark, are the developers' to
//! say; the numbers here are a proposal (prototypes.md).

use super::scene::{EXPOSURE_EV100, SUN_DIRECTION};
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
/// sixteenth, so every shadow was near black (D216).
const SKY_LIGHT_GAIN: f32 = 1.6;

/// How far one sees in the open air, out of the trees, before the forest's haze swallows
/// what is there, and how bright that haze is against the horizon's colour (D273).
const OPEN_VISIBLE_M: f32 = 2_500.0;
const OPEN_AIR: f32 = 0.45;

/// The sun's direction moves on in this many steps an hour: a twentieth of a degree each,
/// a few times a second at the game's pace.
const SUN_STEPS: f32 = 300.0;

/// The hour a new game starts at: morning, the sun low in the east.
pub const MORNING: f32 = 7.5;
/// Where the start is on its world, and where the sun is in its year, which together
/// shape the sun's path: how high it climbs and how long it is up.
const LATITUDE: f32 = 40.0_f32.to_radians();
const DECLINATION: f32 = 10.0_f32.to_radians();

/// The time of day, as an hour from 0 to 24, noon at 12.
#[derive(Resource, Reflect, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct Clock {
    pub hour: f32,
}

impl Default for Clock {
    fn default() -> Self {
        Self { hour: MORNING }
    }
}

impl Clock {
    /// Moves the clock on by so many seconds of play.
    pub fn advance(&mut self, seconds: f32) {
        self.hour = (self.hour + seconds / DAY_SECONDS * 24.0).rem_euclid(24.0);
    }
}

/// The sky over the start: which ways are east, north and up there.
#[derive(Resource, Clone, Copy, Debug)]
pub struct SkyPath {
    east: Vec3,
    north: Vec3,
    up: Vec3,
}

impl SkyPath {
    /// The sky over a place, turned so the morning sun stands where the art put it.
    pub fn over(up: Vec3) -> Self {
        let up = up.normalize();
        let (x, y, _) = sun_local(MORNING);
        let local = Vec2::new(x, y).normalize();
        let toward = SUN_DIRECTION.reject_from(up).normalize();
        let across = up.cross(toward);
        Self {
            east: toward * local.x - across * local.y,
            north: toward * local.y + across * local.x,
            up,
        }
    }

    /// The direction toward the sun at an hour.
    pub fn sun(&self, hour: f32) -> Vec3 {
        let (x, y, z) = sun_local(hour);
        (self.east * x + self.north * y + self.up * z).normalize()
    }

    /// The axis the sky turns about, which points at the pole star.
    pub fn pole(&self) -> Vec3 {
        (self.north * libm::cosf(LATITUDE) + self.up * libm::sinf(LATITUDE)).normalize()
    }

    /// How high the sun is at an hour, in degrees above the horizon.
    pub fn height(&self, hour: f32) -> f32 {
        libm::asinf(self.sun(hour).dot(self.up).clamp(-1.0, 1.0)).to_degrees()
    }
}

/// The sun at an hour, east, north and up over the start.
fn sun_local(hour: f32) -> (f32, f32, f32) {
    let angle = (hour - 12.0) / 24.0 * TAU;
    let (sd, cd) = (libm::sinf(DECLINATION), libm::cosf(DECLINATION));
    let (sl, cl) = (libm::sinf(LATITUDE), libm::cosf(LATITUDE));
    let (sh, ch) = (libm::sinf(angle), libm::cosf(angle));
    (-cd * sh, sd * cl - cd * ch * sl, sd * sl + cd * ch * cl)
}

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
                illuminance: 370.0,
                horizon: v(0.07, 0.09, 0.17),
                zenith: v(0.015, 0.02, 0.06),
                glow: v(0.25, 0.3, 0.45),
                shade: v(0.42, 0.52, 0.9),
                ambient: 650.0,
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
                ambient: 620.0,
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
fn haze_for(horizon: Vec3, open: f32) -> (Vec3, f32) {
    let open = smoothstep(0.4, 0.95, open);
    let shade = horizon.lerp(Vec3::new(0.42, 0.5, 0.42), 0.35) * 0.62;
    let visible = super::scene::HAZE_VISIBLE_M
        * libm::powf(OPEN_VISIBLE_M / super::scene::HAZE_VISIBLE_M, open);
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
    let t = ((-1.0 - height) / 5.0).clamp(0.0, 1.0);
    light_at(height.min(-6.0)).illuminance * t * t * (3.0 - 2.0 * t)
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Clock>()
        .register_type::<Clock>()
        .add_systems(Startup, set_clock)
        .add_systems(PostStartup, find_sky)
        // On the yard, the flat test level, the sun stands still.
        .add_systems(
            Update,
            (tick.run_if(crate::shell::in_the_world), shine).chain(),
        );
}

/// A scripted run may set the hour; otherwise it is the morning, or what a save says, or
/// on the yard, noon.
fn set_clock(options: Res<Options>, mut clock: ResMut<Clock>) {
    if options.yard() {
        clock.hour = 12.0;
    }
    if let Some(hour) = options.time {
        clock.hour = hour;
    }
}

fn find_sky(mut commands: Commands, planet: Res<Planet>) {
    commands.insert_resource(SkyPath::over(planet.start()));
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
) {
    let Some(path) = path else { return };
    // The sun's shadows move in small steps rather than every frame: a light turning by a
    // sliver each frame redraws every shadow edge a little differently, and they crawl.
    let sun = path.sun((clock.hour * SUN_STEPS).round() / SUN_STEPS);
    let height = path.height(clock.hour);
    let light = light_at(height);
    let colour = |rgb: Vec3| Color::srgb(rgb.x, rgb.y, rgb.z);
    // Above the horizon the light comes from the sun; below it, from the moon opposite.
    tweaks.sun_direction = if height > -1.0 { sun } else { -sun };
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
    let hour = clock.hour;
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
    for mut grade in &mut grading {
        let warm = 0.035 * low * if hour > 12.0 { 1.0 } else { 0.5 };
        if (grade.global.temperature - warm).abs() > 1e-4 {
            grade.global.temperature = warm;
        }
    }
    // Out from under the trees, where the sky over the eye is open, the forest's haze gives
    // way to the open air's (D273): thinner, so the far forest and the land beyond show, and
    // in the horizon's colour, so they fade toward the sky rather than into a dark band. The
    // air's own haze (`air`) carries the distance from there. A world with no trees to hide
    // the sky keeps the forest's haze.
    let open = open.map_or(0.0, |open| open.0);
    for mut fog in &mut haze {
        let (air, visible) = haze_for(light.horizon, open);
        fog.color = colour(air);
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
    for mut camera in &mut exposure {
        if camera.ev100 != light.exposure {
            camera.ev100 = light.exposure;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> SkyPath {
        SkyPath::over(Vec3::new(0.1, 1.0, 0.2))
    }

    #[test]
    fn out_of_the_trees_the_haze_is_thinner_and_takes_the_horizons_colour() {
        let horizon = Vec3::new(0.9, 0.86, 0.8);
        let (under, near) = haze_for(horizon, 0.1);
        let (out, far) = haze_for(horizon, 1.0);
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
            let (_, now) = haze_for(horizon, k as f32 / 20.0);
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
        let highest = 90.0 - LATITUDE.to_degrees() + DECLINATION.to_degrees();
        assert!((noon - highest).abs() < 0.5, "noon {noon}, not {highest}");
        assert!(path.height(0.0) < -30.0, "midnight is dark");
        // As long up as the sunrise equation says: cos h = -tan latitude tan declination,
        // with h the hour angle from sunrise to noon. In summer, longer than the night.
        let sunrise = libm::acosf(-libm::tanf(LATITUDE) * libm::tanf(DECLINATION));
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
        // Most of the night's light is the sky's, so the moon's shadows are faint.
        assert!(night.illuminance < night.ambient);
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
        assert_eq!(moonlight(-1.0), 0.0);
        assert!(moonlight(-3.0) < moonlight(-6.0));
        assert_eq!(moonlight(-6.0), light_at(-6.0).illuminance);
        assert_eq!(moonlight(-18.0), light_at(-18.0).illuminance);
    }

    #[test]
    fn the_clock_goes_round() {
        let mut clock = Clock { hour: 23.9 };
        clock.advance(DAY_SECONDS / 24.0 * 0.2);
        assert!((clock.hour - 0.1).abs() < 1e-3);
    }
}
