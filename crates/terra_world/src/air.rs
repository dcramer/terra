//! The air (D14, D212): the sky's colour, the sun's colour through it, and the haze that
//! turns distance into layers, from Bevy's physical atmosphere. That is Sébastien
//! Hillaire's model ("A Scalable and Production Ready Sky and Atmosphere Rendering
//! Technique", 2020), the one Unreal's sky atmosphere uses: light scattered by the air and
//! by dust, worked out into small lookup tables each frame, so the sky, the sunlight and
//! the haze agree at every hour.
//!
//! Our planet is a stand-in two kilometres round (D110 holds for what stands on it, not
//! for the planet), far smaller than an atmosphere's: one at true size would give no haze
//! at all over the few hundred metres a player sees. So the air is drawn as an Earth-sized
//! one seen through a lens, centred below the camera, with every distance through it
//! [`HAZE_SCALE`] times what it is in the world: hills a few hundred metres off are as
//! hazy as mountains tens of kilometres off on Earth. Its centre follows the camera, so
//! "up" is always the planet's up where the camera is.
//!
//! The clouds, the stars and the moon are still our own sky (`sky`), drawn over it.
//! Its sky goes dark as soon as the sun is down, where a real one glows on through the
//! dusk, so from sunset that sky draws the dusk's and the night's colours (`daylight`).

use crate::daylight::{Clock, SkyPath};
use crate::planet::{MainCamera, Planet, Tweaks};
use bevy::{
    light::{
        Atmosphere, SunDisk,
        atmosphere::{Falloff, PhaseFunction, ScatteringMedium, ScatteringTerm},
    },
    pbr::AtmosphereSettings,
    prelude::*,
};

/// How many times longer every distance through the air is than in the world.
pub const HAZE_SCALE: f32 = 18.0;
/// The air's planet, as Earth's, in metres of the air's own space.
const INNER_RADIUS_M: f32 = 6_360_000.0;
const OUTER_RADIUS_M: f32 = 6_460_000.0;
/// How much more dust hangs in the air than Earth's clear-day average, which is what
/// whitens the distance (Mie scattering).
const DUST: f32 = 1.5;

/// The atmosphere entity.
#[derive(Component)]
struct Air;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn)
        .add_systems(Update, settings)
        .add_systems(Update, discs)
        .add_systems(PostUpdate, follow.after(TransformSystems::Propagate));
}

/// The air's make-up, as Hillaire's paper gives Earth's: its gases' Rayleigh scattering
/// thinning over 8 km, dust (Mie) over 1.2 km, scattering more than it absorbs, and a
/// layer of ozone. Bevy's own Earth differs in two ways that redden and darken the sky:
/// its heights are fractions of 60 km on an atmosphere 100 km deep, and its dust absorbs
/// more than it scatters. Ours has a little more dust than a clear day, so the distance
/// whitens. A realm's air will be data; this is the starting realm's.
fn medium() -> ScatteringMedium {
    ScatteringMedium::new(
        256,
        256,
        [
            ScatteringTerm {
                absorption: Vec3::ZERO,
                scattering: Vec3::new(5.802e-6, 13.558e-6, 33.1e-6),
                falloff: Falloff::Exponential { scale: 8.0 / 100.0 },
                phase: PhaseFunction::Rayleigh,
            },
            ScatteringTerm {
                absorption: Vec3::splat(0.444e-6 * DUST),
                scattering: Vec3::splat(3.996e-6 * DUST),
                falloff: Falloff::Exponential { scale: 1.2 / 100.0 },
                phase: PhaseFunction::Mie { asymmetry: 0.8 },
            },
            ScatteringTerm {
                absorption: Vec3::new(0.65e-6, 1.881e-6, 0.085e-6),
                scattering: Vec3::ZERO,
                falloff: Falloff::Tent {
                    center: 0.75,
                    width: 0.3,
                },
                phase: PhaseFunction::Isotropic,
            },
        ],
    )
    .with_label("terra_air")
}

fn spawn(mut commands: Commands, mut media: ResMut<Assets<ScatteringMedium>>) {
    commands.spawn((
        Name::new("Air"),
        Air,
        Atmosphere {
            inner_radius: INNER_RADIUS_M,
            outer_radius: OUTER_RADIUS_M,
            // Grass and earth, seen from above.
            ground_albedo: Vec3::new(0.12, 0.14, 0.08),
            medium: media.add(medium()),
        },
        Transform::from_scale(Vec3::splat(1.0 / HAZE_SCALE)),
    ));
}

/// Shows the sun's disc while the world's light is the sun, and hides it while it is the
/// moon, which `sky` draws as a moon.
fn discs(
    mut commands: Commands,
    clock: Res<Clock>,
    path: Option<Res<SkyPath>>,
    tweaks: Res<Tweaks>,
    added: Query<Entity, (With<DirectionalLight>, Without<SunDisk>)>,
    mut discs: Query<&mut SunDisk>,
) {
    let Some(path) = path else { return };
    let moonlit = tweaks.sun_direction.dot(path.sun(clock.hour)) < 0.0;
    let disc = if moonlit {
        SunDisk::OFF
    } else {
        SunDisk::EARTH
    };
    for light in &added {
        commands.entity(light).insert(disc.clone());
    }
    for mut shown in &mut discs {
        if shown.intensity != disc.intensity {
            *shown = disc.clone();
        }
    }
}

/// The air's tables, worked out every frame, at the quality preset (D239): at the low one
/// each is half as fine and marched in half the steps, as the sky is smooth.
fn settings(
    quality: Res<crate::quality::Quality>,
    mut applied: Local<Option<crate::quality::Quality>>,
    mut commands: Commands,
    camera: Query<Entity, With<MainCamera>>,
) {
    let Some(camera) = camera.iter().next() else {
        return;
    };
    if *applied == Some(*quality) {
        return;
    }
    *applied = Some(*quality);
    let full = AtmosphereSettings::default();
    let settings = if *quality == crate::quality::Quality::Low {
        AtmosphereSettings {
            multiscattering_lut_dirs: full.multiscattering_lut_dirs / 2,
            multiscattering_lut_samples: full.multiscattering_lut_samples / 2,
            sky_view_lut_size: full.sky_view_lut_size / 2,
            sky_view_lut_samples: full.sky_view_lut_samples / 2,
            aerial_view_lut_samples: full.aerial_view_lut_samples / 2,
            ..full
        }
    } else {
        full
    };
    commands.entity(camera).insert(AtmosphereSettings {
        // The haze reaches across the whole planet, in the air's longer distances.
        aerial_view_lut_max_distance: 2_500.0 * HAZE_SCALE,
        ..settings
    });
}

/// Where the air's centre is for a camera: straight below it, so far that the camera is
/// at the air's ground when it is at the sea, and higher as it climbs.
fn centre(camera: Vec3, sea_radius_m: f32) -> Vec3 {
    let up = camera.normalize_or(Vec3::Y);
    up * (sea_radius_m - INNER_RADIUS_M / HAZE_SCALE)
}

fn follow(
    planet: Res<Planet>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut air: Query<(&mut Transform, &mut GlobalTransform), (With<Air>, Without<MainCamera>)>,
) {
    let Ok(camera) = camera.single() else { return };
    let at = centre(camera.translation(), planet.sea_radius_m());
    for (mut transform, mut global) in &mut air {
        transform.translation = at;
        *global = GlobalTransform::from(*transform);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_stands_at_the_airs_ground_at_sea_level_and_rises_with_it() {
        let sea = 2000.0;
        for up in [Vec3::Y, Vec3::new(0.3, 0.9, -0.2).normalize()] {
            for above in [0.0, 40.0] {
                let camera = up * (sea + above);
                let inside = (camera - centre(camera, sea)) * HAZE_SCALE;
                let height = inside.length() - INNER_RADIUS_M;
                assert!((height - above * HAZE_SCALE).abs() < 2.0, "{height}");
                assert!(inside.normalize().dot(up) > 0.9999);
            }
        }
    }
}
