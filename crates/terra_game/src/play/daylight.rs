//! Day and night (game design 001, survival.md): a clock, the sun crossing the sky by it,
//! and the light, sky, shade and haze following how high the sun is.
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

use super::scene::SUN_DIRECTION;
use crate::planet::{MainCamera, Planet, Tweaks};
use crate::shell::Options;
use crate::sky::Sky;
use bevy::prelude::*;
use std::f32::consts::TAU;

/// How long a whole day and night lasts, in seconds of play. Proposed: half an hour, as
/// Valheim's and Enshrouded's are.
pub const DAY_SECONDS: f32 = 30.0 * 60.0;
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
        }
    }
}

/// The light at a few heights of the sun, in degrees, from deep night to high noon. The
/// light between is blended. At 20 degrees it is the look the art was made in. By night
/// most light is the sky's, which has no direction, so the moon's shadows are faint.
fn keys() -> [(f32, Light); 6] {
    let v = Vec3::new;
    let moon = v(0.62, 0.72, 1.0);
    [
        (
            -18.0,
            Light {
                light: moon,
                illuminance: 600.0,
                horizon: v(0.07, 0.09, 0.17),
                zenith: v(0.015, 0.02, 0.06),
                glow: v(0.25, 0.3, 0.45),
                shade: v(0.42, 0.52, 0.9),
                ambient: 1_050.0,
                sky: 1.6,
                night: 1.0,
            },
        ),
        (
            -6.0,
            Light {
                light: moon,
                illuminance: 200.0,
                horizon: v(0.32, 0.27, 0.42),
                zenith: v(0.07, 0.09, 0.22),
                glow: v(0.85, 0.45, 0.35),
                shade: v(0.48, 0.52, 0.88),
                ambient: 1_000.0,
                sky: 2.0,
                night: 0.6,
            },
        ),
        (
            0.0,
            Light {
                light: v(1.0, 0.55, 0.3),
                illuminance: 0.0,
                horizon: v(0.95, 0.55, 0.38),
                zenith: v(0.16, 0.24, 0.52),
                glow: v(1.0, 0.45, 0.22),
                shade: v(0.68, 0.66, 0.9),
                ambient: 1_150.0,
                sky: 2.3,
                night: 0.0,
            },
        ),
        (
            6.0,
            Light {
                light: v(1.0, 0.68, 0.45),
                illuminance: 11_000.0,
                horizon: v(1.0, 0.72, 0.52),
                zenith: v(0.2, 0.36, 0.72),
                glow: v(1.0, 0.6, 0.32),
                shade: v(0.66, 0.72, 0.98),
                ambient: 1_550.0,
                sky: 2.4,
                night: 0.0,
            },
        ),
        (
            20.0,
            Light {
                light: v(1.0, 0.87, 0.7),
                illuminance: 21_000.0,
                horizon: v(0.98, 0.85, 0.7),
                zenith: v(0.24, 0.46, 0.84),
                glow: v(1.0, 0.74, 0.46),
                shade: v(0.66, 0.76, 1.0),
                ambient: 1_800.0,
                sky: 2.4,
                night: 0.0,
            },
        ),
        (
            60.0,
            Light {
                light: v(1.0, 0.96, 0.9),
                illuminance: 26_000.0,
                horizon: v(0.86, 0.9, 0.96),
                zenith: v(0.2, 0.44, 0.86),
                glow: v(1.0, 0.92, 0.8),
                shade: v(0.68, 0.78, 1.0),
                ambient: 2_000.0,
                sky: 2.4,
                night: 0.0,
            },
        ),
    ]
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

pub fn plugin(app: &mut App) {
    app.init_resource::<Clock>()
        .register_type::<Clock>()
        .add_systems(Startup, set_clock)
        .add_systems(PostStartup, find_sky)
        .add_systems(Update, (tick, shine).chain());
}

/// A scripted run may set the hour; otherwise it is the morning, or what a save says.
fn set_clock(options: Res<Options>, mut clock: ResMut<Clock>) {
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

/// Lights the world for the hour: the sun or the moon, the sky, the shade and the haze.
fn shine(
    clock: Res<Clock>,
    path: Option<Res<SkyPath>>,
    mut tweaks: ResMut<Tweaks>,
    mut sky: ResMut<Sky>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut fog: Query<&mut DistanceFog, With<MainCamera>>,
) {
    let Some(path) = path else { return };
    let sun = path.sun(clock.hour);
    let height = path.height(clock.hour);
    let light = light_at(height);
    let colour = |rgb: Vec3| Color::srgb(rgb.x, rgb.y, rgb.z);
    // Above the horizon the light comes from the sun; below it, from the moon opposite.
    tweaks.sun_direction = if height > -1.0 { sun } else { -sun };
    tweaks.sun_color = colour(light.light);
    tweaks.sun_illuminance = light.illuminance;
    sky.horizon = colour(light.horizon);
    sky.zenith = colour(light.zenith);
    sky.glow = colour(light.glow);
    sky.brightness = light.sky;
    sky.night = light.night;
    sky.spin_axis = path.pole();
    sky.spin = (clock.hour - MORNING) / 24.0 * TAU;
    ambient.color = colour(light.shade);
    ambient.brightness = light.ambient;
    for mut fog in &mut fog {
        fog.color = colour(light.horizon);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> SkyPath {
        SkyPath::over(Vec3::new(0.1, 1.0, 0.2))
    }

    #[test]
    fn the_sun_rises_stands_high_at_noon_and_sets() {
        let path = path();
        assert!(path.height(MORNING) > 10.0 && path.height(MORNING) < 30.0);
        assert!(
            path.height(9.0) > path.height(MORNING),
            "the morning sun climbs"
        );
        let noon = path.height(12.0);
        assert!((noon - (90.0 - 40.0 + 10.0)).abs() < 0.5, "noon {noon}");
        assert!(path.height(0.0) < -30.0, "midnight is dark");
        // Up a little longer than it is down.
        let up = (0..240)
            .filter(|i| path.height(*i as f32 / 10.0) > 0.0)
            .count();
        assert!((125..145).contains(&up), "{up} tenths of an hour of day");
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
        let art = light_at(20.0);
        assert_eq!(art.illuminance, 21_000.0);
        let night = light_at(-40.0);
        assert_eq!(night.night, 1.0);
        assert!(night.illuminance < art.illuminance / 20.0);
        assert!(night.ambient < art.ambient);
        // Most of the night's light is the sky's, so the moon's shadows are faint.
        assert!(night.illuminance < night.ambient);
        // Blended smoothly between.
        let dusk = light_at(-3.0);
        assert!(dusk.illuminance > 0.0 && dusk.illuminance < 300.0);
    }

    #[test]
    fn the_clock_goes_round() {
        let mut clock = Clock { hour: 23.9 };
        clock.advance(DAY_SECONDS / 24.0 * 0.2);
        assert!((clock.hour - 0.1).abs() < 1e-3);
    }
}
