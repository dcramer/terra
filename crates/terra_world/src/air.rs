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
//! [`AirScale`] times what it is in the world: on the small planet 18, so hills a few
//! hundred metres off are as hazy as mountains tens of kilometres off on Earth; on a world
//! whose views reach kilometres, as the 30 km world's mountains do, near the true air's. Its centre follows the camera, so
//! "up" is always the planet's up where the camera is.
//!
//! The clouds, the stars and the moon are still our own sky (`sky`), drawn over it, and so
//! is the far land a world may ask for below the horizon (D273), coloured by the air as it is
//! seen: a small picture of it all round the camera, which Bevy works out from the air's
//! tables each frame ([`AirPicture`]).
//! Its sky goes dark as soon as the sun is down, where a real one glows on through the
//! dusk, so from sunset that sky draws the dusk's and the night's colours (`daylight`).

use crate::daylight::{Clock, SkyPath};
use crate::planet::{MainCamera, Planet, Tweaks};
use bevy::{
    light::{
        Atmosphere, AtmosphereEnvironmentMapLight, GeneratedEnvironmentMapLight, SunDisk,
        atmosphere::{Falloff, PhaseFunction, ScatteringMedium, ScatteringTerm},
    },
    pbr::AtmosphereSettings,
    prelude::*,
    render::sync_world::SyncToRenderWorld,
};

/// How many times longer every distance through the air is than in the world, in the open, on
/// the small planet, where it was tuned (D212).
pub const HAZE_SCALE: f32 = 18.0;
/// How far, in the air's own metres, the farthest ground seen stands, about: the small
/// planet's horizon from its highest hill, about 780 m, times 18 (D334).
const SEEN_IN_THE_AIR_M: f32 = 14_000.0;

/// How many times longer every distance through the air is than in the world, in the open, for
/// a world (D334): so the farthest ground one sees, the horizon from its highest ground, is as
/// far through the air as the small planet's was, and never less than the true air. On the
/// 30 km world with mountains a kilometre and more high, the horizon from a summit is 12 km,
/// and the air is nearly the true air: peaks 5 km off are as hazy as 5 km of real air, where
/// at 18 times they would have stood 90 km off in it.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct AirScale(pub f32);

impl AirScale {
    pub fn for_world(radius_m: f32, relief_m: f32) -> Self {
        let horizon_m = libm::sqrtf(2.0 * radius_m * relief_m.max(1.0));
        Self((SEEN_IN_THE_AIR_M / horizon_m).clamp(1.0, HAZE_SCALE))
    }
}
/// And under a closed canopy (D299): the air between the trunks is in the canopy's shade, not
/// the sunlit sky's, so a trunk fifty metres off is not as blue as a mountain nine hundred
/// off. At the open air's scale the forest's shade measured blue over green 0.68 where Sons of
/// the Forest's is 0.38; at this, 0.32 to 0.4.
const FOREST_HAZE_SCALE: f32 = 9.0;

/// How long the air's distances are for an eye under so much open sky (`OpenOverhead`): the
/// forest's under the trees, the open air's out of them, as the forest's haze gives way to the
/// open air's (D279), between them evenly by ratio. The forest's is the open air's halved, as
/// 9 is to the small planet's 18 (D299), and never shorter than the true air.
pub fn haze_scale(open: f32, air: AirScale) -> f32 {
    let t = ((open - 0.55) / (0.98 - 0.55)).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    let forest = (air.0 * FOREST_HAZE_SCALE / HAZE_SCALE).max(1.0);
    libm::expf(libm::logf(forest) * (1.0 - t) + libm::logf(air.0) * t)
}
/// How much dust the air holds now, as `medium` takes it: so many times Earth's clear day's.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct AirDust(pub f32);

/// Rayleigh scattering, dust's scattering and absorption, and ozone's absorption, per metre at
/// the ground (ozone's at its layer's peak), as `medium` makes the air.
const RAYLEIGH: Vec3 = Vec3::new(5.802e-6, 13.558e-6, 33.1e-6);
const RAYLEIGH_HEIGHT_M: f32 = 8_000.0;
const DUST_SCATTERS: f32 = 3.996e-6;
const DUST_ABSORBS: f32 = 0.444e-6;
const DUST_HEIGHT_M: f32 = 1_200.0;
const OZONE: Vec3 = Vec3::new(0.65e-6, 1.881e-6, 0.085e-6);
/// Ozone's tent, 0.3 of the air's 100 km wide at its foot: as much as 15 km of its peak.
const OZONE_COLUMN_M: f32 = 15_000.0;

/// How much of the sun's light comes through the air to the ground with the sun so many
/// radians up, the air holding so much dust, per colour: as Bevy's atmosphere dims and warms
/// the sun's light on everything it lights (its transmittance table), worked out on the CPU
/// for what is lit by bounce rather than by the sun directly. The air's path through each of
/// its layers, its height times Kasten and Young's air mass (1989), which holds to the horizon.
pub fn sun_through(height: f32, dust: f32) -> Vec3 {
    if height <= -0.01 {
        return Vec3::ZERO;
    }
    let degrees = height.max(0.0).to_degrees();
    let air_mass =
        1.0 / (libm::sinf(height.max(0.0)) + 0.50572 * libm::powf(degrees + 6.07995, -1.6364));
    let depth = RAYLEIGH * RAYLEIGH_HEIGHT_M
        + Vec3::splat((DUST_SCATTERS + DUST_ABSORBS) * dust * DUST_HEIGHT_M)
        + OZONE * OZONE_COLUMN_M;
    let through = depth * air_mass;
    Vec3::new(
        libm::expf(-through.x),
        libm::expf(-through.y),
        libm::expf(-through.z),
    )
}

/// The air's planet, as Earth's, in metres of the air's own space.
const INNER_RADIUS_M: f32 = 6_360_000.0;
const OUTER_RADIUS_M: f32 = 6_460_000.0;
/// How much more dust hangs in the air than Earth's clear-day average, which is what
/// whitens the distance (Mie scattering), at the small planet's scale. A world whose air is
/// nearer the true air has more (`dust_for`).
const DUST: f32 = 1.5;

/// The most dust a world's air holds, as Earth's clean continental air on a clear day: an
/// aerosol optical depth of 0.064 at 550 nm (OPAC's "clean continental", Hess, Koepke and
/// Schult 1998, BAMS 79), where a clear day's here is (3.996 + 0.444) x 1e-6 per metre over
/// 1.2 km, 0.0053: twelve times. Alpine air is cleaner still (Jungfraujoch's about 0.03).
const CLEAR_DAY_DUST: f32 = 12.0;

/// The dust in a world's air: as much in each of the world's metres as the small planet's
/// air at 18 times holds (D334), so far ridges whiten over kilometres as the hills do over
/// hundreds of metres, and the sky over them is as pale, while the gases' blue stays the true
/// air's; but never more than a clean clear day's (`CLEAR_DAY_DUST`). On the 30 km world with
/// mountains, whose air is nearly the true air, that is the cap: some 70 km of visibility on
/// the valley floor, the range 5 to 10 km off standing dark under a paler sky, and 20 km of
/// air still leaving only a third of the land's own light. Held to the small planet's
/// per metre it was 25 times, a hazy day of 40 km, and the far range came out only 0.05
/// darker than the sky where the references' is 0.2 (valley range, D426).
fn dust_for(scale: AirScale) -> f32 {
    (DUST * HAZE_SCALE / scale.0).min(CLEAR_DAY_DUST)
}

/// How much of its dust's haze a day's air holds for how warm it is: the haze that whitens
/// the distance is mostly water on fine particles, and cold air holds little (a saturated
/// vapour pressure a third of a mild day's at freezing), so polar and winter air is clear and
/// its far land stands dark under a pale sky (the tundra's photographs: far land 0.45 of the
/// sky's brightness against the mountain lake's 0.34). As a mild day's air at `MILD_C` and
/// warmer, the haze as tuned; never under a third of it.
fn haze_for(day_mean_c: f32) -> f32 {
    // Magnus's formula for water's saturated vapour pressure.
    let vapour = |t: f32| 6.112 * libm::expf(17.62 * t / (243.12 + t));
    (vapour(day_mean_c) / vapour(MILD_C)).clamp(0.35, 1.0)
}
const MILD_C: f32 = 10.0;

/// The air's dust as the day's warmth where the start is leaves it (`haze_for`), once the
/// climate and the clock are known.
fn clear_cold_air(
    fields: Option<Res<crate::climate::ClimateFields>>,
    clock: Res<crate::daylight::Clock>,
    start: Res<crate::start::Start>,
    scale: Option<Res<AirScale>>,
    air: Query<&Atmosphere, With<Air>>,
    mut media: ResMut<Assets<ScatteringMedium>>,
    mut done: Local<bool>,
    mut commands: Commands,
) {
    let (Some(fields), Some(scale)) = (fields, scale) else {
        return;
    };
    if *done {
        return;
    }
    let Ok(atmosphere) = air.single() else {
        return;
    };
    *done = true;
    let place = fields.place(start.translation);
    let mean = fields.fields.day(place, clock.time().day()).mean_c();
    let haze = haze_for(mean);
    if let Some(mut medium) = media.get_mut(&atmosphere.medium) {
        *medium = self::medium(dust_for(*scale) * haze);
        commands.insert_resource(AirDust(dust_for(*scale) * haze));
    }
    info!(
        "Air: the day's mean {mean:.1} °C at the start leaves {:.0}% of its haze, its dust {:.1} times a clear day's",
        haze * 100.0,
        dust_for(*scale) * haze
    );
}

/// The atmosphere entity.
#[derive(Component)]
struct Air;

/// The air as seen from the camera, all round it: a cube of its light (Bevy's
/// `AtmosphereEnvironmentMapLight`, unexposed), worked out each frame from the same tables
/// the sky is drawn from, so what is drawn with it matches the sky. For the sky's own
/// drawing, not the world's light, which is the painted sky's (`skylight`).
#[derive(Resource, Clone, Debug)]
pub struct AirPicture(pub Handle<Image>);

/// The entity Bevy keeps the picture on.
#[derive(Component)]
struct Pictured;

/// How many pixels along each face of the picture: fine enough to read the air just over
/// the horizon, which is where it changes most.
const PICTURE: u32 = 128;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn)
        .add_systems(Update, (settings, picture, clear_cold_air))
        .add_systems(Update, discs)
        .add_systems(PostUpdate, follow.after(TransformSystems::Propagate));
}

/// The air's make-up, as Hillaire's paper gives Earth's: its gases' Rayleigh scattering
/// thinning over 8 km, dust (Mie) over 1.2 km, scattering more than it absorbs, and a
/// layer of ozone. Bevy's own Earth differs in two ways that redden and darken the sky:
/// its heights are fractions of 60 km on an atmosphere 100 km deep, and its dust absorbs
/// more than it scatters. Ours has a little more dust than a clear day, so the distance
/// whitens, `dust` times Earth's. A realm's air will be data; this is the starting realm's.
fn medium(dust: f32) -> ScatteringMedium {
    ScatteringMedium::new(
        256,
        256,
        [
            ScatteringTerm {
                absorption: Vec3::ZERO,
                scattering: RAYLEIGH,
                falloff: Falloff::Exponential { scale: 8.0 / 100.0 },
                phase: PhaseFunction::Rayleigh,
            },
            ScatteringTerm {
                absorption: Vec3::splat(DUST_ABSORBS * dust),
                scattering: Vec3::splat(DUST_SCATTERS * dust),
                falloff: Falloff::Exponential { scale: 1.2 / 100.0 },
                phase: PhaseFunction::Mie { asymmetry: 0.8 },
            },
            ScatteringTerm {
                absorption: OZONE,
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

fn spawn(mut commands: Commands, mut media: ResMut<Assets<ScatteringMedium>>, planet: Res<Planet>) {
    let scale = AirScale::for_world(planet.radius_m(), planet.relief_m());
    commands.insert_resource(scale);
    commands.insert_resource(AirDust(dust_for(scale)));
    info!(
        "Air: its distances {:.2} times the world's, its dust {:.1} times a clear day's",
        scale.0,
        dust_for(scale)
    );
    commands.spawn((
        Name::new("Air"),
        Air,
        Atmosphere {
            inner_radius: INNER_RADIUS_M,
            outer_radius: OUTER_RADIUS_M,
            // Grass and earth, seen from above.
            ground_albedo: Vec3::new(0.12, 0.14, 0.08),
            medium: media.add(medium(dust_for(scale))),
        },
        Transform::from_scale(Vec3::splat(1.0 / scale.0)),
    ));
    commands.spawn((
        Name::new("Air seen"),
        Pictured,
        AtmosphereEnvironmentMapLight {
            intensity: 0.0,
            affects_lightmapped_mesh_diffuse: false,
            size: UVec2::splat(PICTURE),
        },
        SyncToRenderWorld::default(),
    ));
}

/// Keeps the air's picture once Bevy has made it, and stops Bevy filtering it into light
/// every frame, which only a light probe needs.
fn picture(
    mut commands: Commands,
    made: Query<(Entity, &GeneratedEnvironmentMapLight), With<Pictured>>,
) {
    for (entity, generated) in &made {
        commands.insert_resource(AirPicture(generated.environment_map.clone()));
        commands
            .entity(entity)
            .remove::<GeneratedEnvironmentMapLight>();
    }
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
/// each is half as fine and marched in half the steps, as the sky is smooth; at ultra the
/// sky's and the haze's twice as fine (D428).
fn settings(
    quality: Res<crate::quality::Quality>,
    scale: Option<Res<AirScale>>,
    mut applied: Local<Option<crate::quality::Quality>>,
    mut commands: Commands,
    camera: Query<Entity, With<MainCamera>>,
    planet: Res<Planet>,
) {
    let (Some(camera), Some(scale)) = (camera.iter().next(), scale) else {
        return;
    };
    if *applied == Some(*quality) {
        return;
    }
    *applied = Some(*quality);
    let full = AtmosphereSettings::default();
    let settings = match *quality {
        crate::quality::Quality::Low => AtmosphereSettings {
            multiscattering_lut_dirs: full.multiscattering_lut_dirs / 2,
            multiscattering_lut_samples: full.multiscattering_lut_samples / 2,
            sky_view_lut_size: full.sky_view_lut_size / 2,
            sky_view_lut_samples: full.sky_view_lut_samples / 2,
            aerial_view_lut_samples: full.aerial_view_lut_samples / 2,
            ..full
        },
        // Films' (D428): the haze over distance in twice the slices and steps, so a town a
        // few hundred metres off and the range behind it fall in slices of their own, and
        // the sky round a low sun twice as fine.
        crate::quality::Quality::Ultra => AtmosphereSettings {
            sky_view_lut_size: full.sky_view_lut_size * 2,
            sky_view_lut_samples: full.sky_view_lut_samples * 2,
            aerial_view_lut_size: full.aerial_view_lut_size * 2,
            aerial_view_lut_samples: full.aerial_view_lut_samples * 2,
            ..full
        },
        _ => full,
    };
    commands.entity(camera).insert(AtmosphereSettings {
        // The haze reaches across the whole planet, in the air's longer distances, or as far
        // as one sees on a world whose views reach farther.
        aerial_view_lut_max_distance: (2_500.0 * HAZE_SCALE)
            .max(2.0 * libm::sqrtf(2.0 * planet.radius_m() * planet.relief_m().max(1.0)) * scale.0),
        ..settings
    });
}

/// Where the air's centre is for a camera, its distances `scale` times the world's: straight
/// below it, so far that the camera is at the air's ground when it is at the sea, and higher
/// as it climbs.
fn centre(camera: Vec3, sea_radius_m: f32, scale: f32) -> Vec3 {
    let up = camera.normalize_or(Vec3::Y);
    up * (sea_radius_m - INNER_RADIUS_M / scale)
}

fn follow(
    planet: Res<Planet>,
    air_scale: Option<Res<AirScale>>,
    open: Option<Res<crate::sky_occlusion::OpenOverhead>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut air: Query<(&mut Transform, &mut GlobalTransform), (With<Air>, Without<MainCamera>)>,
) {
    let (Ok(camera), Some(air_scale)) = (camera.single(), air_scale) else {
        return;
    };
    // A world with no canopy to stand under keeps the open air's.
    let scale = open.map_or(air_scale.0, |open| haze_scale(open.0, *air_scale));
    let at = centre(camera.translation(), planet.sea_radius_m(), scale);
    for (mut transform, mut global) in &mut air {
        transform.translation = at;
        transform.scale = Vec3::splat(1.0 / scale);
        *global = GlobalTransform::from(*transform);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_air_holds_less_haze_than_mild_and_never_none() {
        assert_eq!(haze_for(MILD_C), 1.0);
        assert_eq!(haze_for(25.0), 1.0);
        let freezing = haze_for(0.0);
        // Water's saturated vapour pressure at 0 and 10 °C, 6.1 and 12.3 hPa: half.
        assert!((freezing - 6.11 / 12.27).abs() < 0.02, "{freezing}");
        assert!(haze_for(3.0) > freezing && haze_for(-30.0) == 0.35);
    }

    #[test]
    fn the_air_is_the_small_planets_on_it_and_near_the_true_air_among_mountains() {
        // The small planet, its hills and the lush forest's rolling ground, and the flat yard
        // of the 30 km world, keep the air they were tuned in.
        assert_eq!(AirScale::for_world(2000.0, 150.0).0, HAZE_SCALE);
        assert_eq!(AirScale::for_world(2000.0, 6.0).0, HAZE_SCALE);
        assert_eq!(AirScale::for_world(30_000.0, 0.0).0, HAZE_SCALE);
        // Mountains 2.5 km high on the 30 km world see 12 km: nearly the true air.
        let mountains = AirScale::for_world(30_000.0, 2600.0).0;
        assert!((1.0..1.5).contains(&mountains), "{mountains}");
        // The small planet's dust as tuned; among the mountains more of it in the air's
        // metres, but no more than a clean clear day's: an optical depth of 0.06 to 0.07.
        assert_eq!(dust_for(AirScale(HAZE_SCALE)), DUST);
        let among = dust_for(AirScale(mountains));
        assert!(among > 4.0 * DUST, "{among}");
        let depth = (DUST_SCATTERS + DUST_ABSORBS) * among * DUST_HEIGHT_M;
        assert!((0.055..0.075).contains(&depth), "{depth}");
        // Under trees never shorter than the true air.
        assert!(haze_scale(0.0, AirScale(1.0)) >= 1.0);
    }

    #[test]
    fn the_camera_stands_at_the_airs_ground_at_sea_level_and_rises_with_it() {
        let sea = 2000.0;
        for up in [Vec3::Y, Vec3::new(0.3, 0.9, -0.2).normalize()] {
            for above in [0.0, 40.0] {
                let camera = up * (sea + above);
                let inside = (camera - centre(camera, sea, HAZE_SCALE)) * HAZE_SCALE;
                let height = inside.length() - INNER_RADIUS_M;
                assert!((height - above * HAZE_SCALE).abs() < 2.0, "{height}");
                assert!(inside.normalize().dot(up) > 0.9999);
            }
        }
    }

    #[test]
    fn the_sun_comes_through_the_air_redder_the_lower_it_is_and_not_at_all_below_it() {
        let overhead = sun_through(std::f32::consts::FRAC_PI_2, 1.0);
        let horizon = sun_through(0.0, 1.0);
        // Straight up most of it comes through, the blue least.
        assert!(
            overhead.min_element() > 0.6 && overhead.z < overhead.x,
            "{overhead}"
        );
        // At the horizon the air is Kasten and Young's 38.09 airs thick: the depth each colour
        // goes through is that many times its depth overhead.
        for k in 0..3 {
            let mass = libm::logf(horizon[k]) / libm::logf(overhead[k]);
            assert!((mass - 38.09).abs() < 0.2, "{mass}");
        }
        let redness = |t: Vec3| t.x / t.z;
        let mut last = redness(overhead);
        for degrees in [60.0_f32, 30.0, 15.0, 6.0, 2.0, 0.0] {
            let now = redness(sun_through(degrees.to_radians(), 1.0));
            assert!(now > last, "{degrees}: {now} after {last}");
            last = now;
        }
        // Dust dims every colour alike.
        let dusty = sun_through(0.3, 16.0);
        let clear = sun_through(0.3, 1.0);
        let dimmed = dusty / clear;
        assert!(dimmed.max_element() < 0.9, "{dimmed}");
        assert!(
            dimmed.max_element() - dimmed.min_element() < 1e-4,
            "{dimmed}"
        );
        assert_eq!(sun_through(-0.05, 1.0), Vec3::ZERO);
    }

    #[test]
    fn under_trees_the_airs_distances_are_shorter_and_lengthen_evenly_into_the_open() {
        let small = AirScale(HAZE_SCALE);
        assert!((haze_scale(0.1, small) - FOREST_HAZE_SCALE).abs() < 1e-4);
        assert!((haze_scale(1.0, small) - HAZE_SCALE).abs() < 1e-3);
        let mut last = haze_scale(0.0, small);
        for k in 1..=40 {
            let now = haze_scale(k as f32 / 40.0, small);
            assert!(now >= last && now < last * 1.15, "{last} then {now}");
            last = now;
        }
    }
}
