//! Which world a run begins in, entered as any other world is, by crossing to it (D469),
//! and the land of each world entered, made from its spec.

use super::{MainCamera, Planet, ROUND_M, Tweaks};
use crate::instance::{Cross, Entered, InstanceKey, WorldSpec};
use crate::shell::Options;
use crate::start::{Sited, Start};
use bevy::{camera_controller::free_camera::FreeCamera, prelude::*};
use terra_core::{PlanetSpec, Seed};

/// The world a run begins in: the planet of seed 1, or the yard, the flat test level.
pub fn first_world(options: &Options) -> WorldSpec {
    WorldSpec {
        key: InstanceKey(0),
        kind: if options.yard() { "yard" } else { "world" }.into(),
        seed: 1,
    }
}

/// The planet a world stands on: its seed's, as big as the run asks.
pub fn planet_spec(world: &WorldSpec, radius_m: f32) -> PlanetSpec {
    PlanetSpec {
        seed: Seed(world.seed),
        radius_m,
    }
}

/// A world's land: its planet, the start on it and the site it was asked to stand at
/// (`--site`). On a world with mountains the land round the start is worked out before
/// anything stands on it (D322): its lakes, seasons and snow on the clock's date. A site the
/// land's rules make is found first, and the start stood on its shore once its lakes are
/// known.
pub fn land_of(world: &WorldSpec, options: &Options) -> (Planet, Start, Option<Sited>) {
    let yard = options.yard();
    let spec = planet_spec(world, options.radius_m);
    crate::crash::note("seed", spec.seed.0);
    let mut planet = if yard && options.rolling {
        Planet::rolling(spec)
    } else if yard {
        Planet::smooth(spec)
    } else {
        Planet::new(spec)
    };
    let climate: terra_climate::Climate = crate::data::read(crate::daylight::WorldClimate::FILE);
    let day = options.date.map_or(0, |date| date.day(&climate.year));
    match options.site.clone() {
        Some(name) => {
            let found = crate::start::find_site(&planet, &name);
            planet.look_round(found.middle(), ROUND_M, day);
            planet.clear_woods(|land| found.clearings(land));
            let start = found.start(&planet);
            (planet, start, Some(Sited(found)))
        }
        None => {
            let start = Start::choose(&planet, yard);
            if planet.has_mountains() {
                planet.look_round(start.translation, ROUND_M, day);
            }
            (planet, start, None)
        }
    }
}

/// What the land is drawn and lit with before anything tunes it: the sun high, the sea at
/// its level but on the yard, the ground's and grass's colours.
pub fn tweaks(yard: bool) -> Tweaks {
    Tweaks {
        sun_direction: Vec3::new(0.5, 0.75, 0.45),
        sun_color: Color::WHITE,
        sun_illuminance: 12_000.0,
        light_direction: Vec3::ZERO,
        light_size: 0.0,
        sea: !yard,
        sea_level_m: super::SEA_LEVEL_M,
        sea_shallow: Color::srgb(0.1, 0.38, 0.5),
        sea_deep: Color::srgb(0.02, 0.1, 0.24),
        ground_low: Color::srgb(0.42, 0.33, 0.26),
        ground_high: Color::srgb(0.72, 0.66, 0.58),
        ground_steep: Color::srgb(0.3, 0.27, 0.25),
        ground_faceted: false,
        ground_cover: 0.0,
        grass_dry: Color::srgb(0.6, 0.58, 0.26),
        grass_lush: Color::srgb(0.3, 0.5, 0.2),
        sand: Color::srgb(0.84, 0.74, 0.55),
    }
}

/// Puts a world's land in: the first world's was made as the app was built, for what
/// start-up reads of it; every later one is made here from its spec. What sets the land's
/// colours goes after it, in `Building::Land`.
pub(crate) fn land(world: &mut World) {
    if world.contains_resource::<Planet>() {
        return;
    }
    let spec = world.resource::<WorldSpec>().clone();
    let options = world.resource::<Options>();
    let yard = options.yard();
    let (planet, start, sited) = land_of(&spec, options);
    world.insert_resource(planet);
    world.insert_resource(start);
    if let Some(sited) = sited {
        world.insert_resource(sited);
    }
    world.insert_resource(tweaks(yard));
}

/// A run starts away, in no world, and crosses to its first once the camera and settings
/// are made (D469).
pub(super) fn cross_to_the_first(mut commands: Commands, options: Res<Options>) {
    commands.queue(Cross(first_world(&options)));
}

/// The camera every world is seen through (D469), a little above the start, looking out
/// along the ground, at speeds suited to a planet rather than a room (the wheel changes
/// them). A player, once there is one, steers it instead.
pub(super) fn spawn_camera(mut commands: Commands, planet: Res<Planet>) {
    commands.spawn((
        Name::new("Camera"),
        MainCamera,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            near: 0.2,
            far: planet.radius_m() * 40.0,
            ..default()
        }),
        looking_out(&planet),
        FreeCamera {
            walk_speed: 30.0,
            run_speed: 300.0,
            ..default()
        },
    ));
}

fn looking_out(planet: &Planet) -> Transform {
    let start = planet.start();
    let up = start.normalize();
    let along = up.cross(Vec3::X).normalize();
    Transform::from_translation(start + up * 25.0).looking_to(along - up * 0.25, up)
}

/// In every world after the first, a camera nobody steers stands above its start again.
pub(super) fn stand_the_camera(
    planet: Res<Planet>,
    entered: Res<Entered>,
    mut camera: Single<&mut Transform, (With<MainCamera>, With<FreeCamera>)>,
) {
    if entered.0 > 1 {
        **camera = looking_out(&planet);
    }
}
