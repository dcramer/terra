//! Which world a run begins in, entered as any other world is, by crossing to it (D469),
//! and the land of each world entered, made from its spec.

use super::{MainCamera, Planet, ROUND_M, Tweaks};
use crate::instance::{Cross, InstanceKey, WorldSpec};
use crate::shell::Options;
use crate::start::{Found, Sited, Start, Towns};
use bevy::{camera::ShadowLodOrigin, camera_controller::free_camera::FreeCamera, prelude::*};
use std::sync::atomic::{AtomicU8, Ordering};
use terra_core::{PlanetSpec, Seed};

pub mod generating;

/// The seed of a run given none, and of every world kept from before worlds had their own.
pub const FIRST_SEED: u64 = 1;

/// The world a run begins in: the home realm of the world's seed (D596), which is the world's
/// own (another realm's derives from it, `terra_gen::hash`), or the yard, the flat test level.
/// The seed is `--seed`, or the kept world's, chosen before this (`choice::Choosing`).
pub fn first_world(options: &Options) -> WorldSpec {
    WorldSpec {
        key: InstanceKey(0),
        kind: match (&options.realm, options.yard()) {
            (Some(realm), _) => realm.clone(),
            (None, true) => "yard".into(),
            (None, false) => "world".into(),
        },
        seed: options.seed.unwrap_or(FIRST_SEED),
    }
}

/// The planet a world stands on: its seed's, as big as the run asks.
pub fn planet_spec(world: &WorldSpec, radius_m: f32) -> PlanetSpec {
    PlanetSpec {
        seed: Seed(world.seed),
        radius_m,
    }
}

/// The start a world kept on disk found when it was first entered, so entering it again does
/// not search for it (seconds on a 30 km world, D596): the way from the centre it is on a
/// planet of that seed and radius, taken only by a planet of both.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct KnownStart {
    pub seed: u64,
    pub radius_m: f32,
    pub direction: Vec3,
}

impl KnownStart {
    /// The start it knows for a planet of that spec, if it is that planet's.
    pub fn of(known: Option<&KnownStart>, spec: PlanetSpec) -> Option<Vec3> {
        known
            .filter(|known| known.seed == spec.seed.0 && known.radius_m == spec.radius_m)
            .map(|known| known.direction)
    }
}

/// The towns a world kept on disk found round its start when it was first entered (D620),
/// none found among them, so entering it again does not search for them (0.3 to 4 s on a
/// 30 km world): taken only by a planet of its seed and radius, as [`KnownStart`].
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct KnownTowns {
    pub seed: u64,
    pub radius_m: f32,
    pub towns: Vec<terra_land::town::TownSite>,
}

impl KnownTowns {
    /// The towns it knows for a planet of that spec, if it is that planet's.
    pub fn of(
        known: Option<&KnownTowns>,
        spec: PlanetSpec,
    ) -> Option<&[terra_land::town::TownSite]> {
        known
            .filter(|known| known.seed == spec.seed.0 && known.radius_m == spec.radius_m)
            .map(|known| known.towns.as_slice())
    }
}

/// A world's land: its planet, the start on it, the site it was asked to stand at and its
/// towns.
pub type Land = (Planet, Start, Option<Sited>, Towns);

/// How far making a world's land has come, for the loading screen to say (D623): set by
/// [`made_land`] as each of its parts begins, read on the main thread.
#[derive(Default, Debug)]
pub struct Stage(AtomicU8);

impl Stage {
    /// What each part is called on the loading screen, in the order they come.
    const SAID: [&'static str; 4] = [
        "Finding where the world starts",
        "Working out the land round the start",
        "Choosing where to stand",
        "Finding the first town",
    ];

    fn set(&self, part: usize) {
        self.0.store(part as u8, Ordering::Relaxed);
    }

    /// What the land is being made as now.
    pub fn said(&self) -> &'static str {
        Self::SAID[self.part()]
    }

    /// Which of the parts is being made now.
    pub fn part(&self) -> usize {
        usize::from(self.0.load(Ordering::Relaxed)).min(Self::SAID.len() - 1)
    }
}

/// A world's land, made from its spec ([`made_land`]), saying to no one how far it has come.
pub fn land_of(
    world: &WorldSpec,
    options: &Options,
    known: Option<&KnownStart>,
    known_towns: Option<&KnownTowns>,
) -> Land {
    made_land(world, options, known, known_towns, &Stage::default())
}

/// A world's land: its planet, the start on it and the site it was asked to stand at
/// (`--site`). On a world with mountains the land round the start is worked out before
/// anything stands on it (D322): its lakes, seasons and snow on the clock's date, and the
/// start's plot chosen on dry ground once its lakes are known. A site the
/// land's rules make is found first, and the start stood on its shore once its lakes are
/// known. A start found before (`known`) is taken for a planet of its seed and radius, and so
/// are the towns found before (`known_towns`). Seconds on a 30 km world: the game makes it on
/// a thread of its own behind the loading screen (`generating`, D623), saying each part in
/// `stage` as it begins.
pub fn made_land(
    world: &WorldSpec,
    options: &Options,
    known: Option<&KnownStart>,
    known_towns: Option<&KnownTowns>,
    stage: &Stage,
) -> Land {
    let yard = options.yard();
    let spec = planet_spec(world, options.radius_m);
    crate::crash::note("seed", spec.seed.0);
    stage.set(0);
    let mut planet = if yard && options.rolling {
        Planet::rolling(spec)
    } else if yard {
        Planet::smooth(spec)
    } else {
        Planet::starting(spec, KnownStart::of(known, spec))
    };
    // Its realm's share of plants (D642), before anything is laid on it.
    if let Some(realm) = crate::realm::Realms::shared().of(&world.kind)
        && (realm.plants.is_some() || realm.trees.is_some())
    {
        planet.set_plants(realm.plants.unwrap_or(1.0), realm.trees);
    }
    let climate: terra_climate::Climate = crate::data::read(crate::daylight::WorldClimate::FILE);
    let day = options.date.map_or(0, |date| date.day(&climate.year));
    match options.site.clone() {
        Some(name) => {
            let found = crate::start::find_site(&planet, &name);
            stage.set(1);
            planet.look_round(found.middle(), ROUND_M, day);
            planet.clear_woods(|land| found.clearings(land));
            stage.set(2);
            let start = found.start(&planet);
            let towns = Towns(match found {
                Found::Town(site) | Found::Settlement(site) => vec![site],
                Found::Lake(_) | Found::Tundra(_) => Vec::new(),
            });
            (planet, start, Some(Sited(found)), towns)
        }
        None => {
            // The lakes are known before the start's plot is chosen, so it is never chosen
            // under one's water (seed 9697367627075512559 stood knee-deep): the plot is
            // within 600 m of the planet's start, well inside the land looked round.
            let looked = planet.has_mountains() && !yard;
            if looked {
                stage.set(1);
                planet.look_round(planet.start(), ROUND_M, day);
            }
            stage.set(2);
            let start = Start::choose(&planet, yard);
            let mut towns = Towns::default();
            if looked {
                stage.set(3);
                towns.0 = match KnownTowns::of(known_towns, spec) {
                    Some(kept) => kept.to_vec(),
                    None => first_town(&planet, &start).into_iter().collect(),
                };
                let want = terra_land::town::MarketTown::FIRST;
                planet.clear_woods(|land| {
                    towns
                        .0
                        .iter()
                        .flat_map(|site| want.clearings(land, site))
                        .collect()
                });
            }
            (planet, start, None, towns)
        }
    }
}

/// A world's first market town (D620), found round the player's start by the land's rules
/// (`terra_land::Land::first_town`), on the land worked out round it; none if its land makes
/// no such place near, which the log says, and why.
fn first_town(planet: &Planet, start: &Start) -> Option<terra_land::town::TownSite> {
    let round = planet.round()?;
    let began = std::time::Instant::now();
    let (site, searched) = round
        .land
        .first_town(start.translation.normalize(), Some(round.as_ref()));
    let took = began.elapsed().as_secs_f32();
    let Some(site) = site else {
        info!("First town: none near the start ({searched:?}), {took:.1} s");
        return None;
    };
    let at = start.local(planet.surface(site.toward));
    info!(
        "First town: {:.0} m east and {:.0} m south of the start, {:.0} m over the sea, the range to {:.0} m {:.1} km off; found in {took:.1} s",
        at.x,
        at.z,
        site.height_m,
        site.peak_m,
        site.peak_off_m / 1000.0
    );
    Some(site)
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
        foliage_hue: 0.0,
    }
}

/// Puts a world's land in, the first's too: the land made behind the loading screen
/// (`generating`, D623), or else, in a run with no loading screen, made from its spec now.
/// What sets the land's colours goes after it, in `Building::Land`.
pub(crate) fn land(world: &mut World) {
    let yard = world.resource::<Options>().yard();
    let (planet, start, sited, towns) = match world.remove_resource::<generating::Made>() {
        Some(made) => made.0,
        None => land_of(
            world.resource::<WorldSpec>(),
            world.resource::<Options>(),
            world.get_resource::<KnownStart>(),
            world.get_resource::<KnownTowns>(),
        ),
    };
    world.insert_resource(planet);
    world.insert_resource(start);
    world.insert_resource(towns);
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

/// The camera every world is seen through (D469), made before any world is: each world
/// stands it and sets how far it sees (`stand_the_camera`). It moves at speeds suited to a
/// planet rather than a room (the wheel changes them). A player, once there is one, steers
/// it instead. Shadows' levels of detail are measured from it (`ShadowLodOrigin`, D612):
/// Bevy otherwise takes the camera of the lowest entity, which may be the interface's or an
/// icon's, far from the ground, and the shadow stand-ins shrink with their distance from it.
pub(super) fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("Camera"),
        MainCamera,
        ShadowLodOrigin,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            near: 0.2,
            ..default()
        }),
        Transform::default(),
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

/// How far the camera sees on a planet: past its far side, so the whole of a small world
/// shows from space and nothing of a big one is cut off.
fn far_m(planet: &Planet) -> f32 {
    planet.radius_m() * 40.0
}

/// In every world the camera sees as far as its planet asks, and a camera nobody steers
/// stands a little above its start, looking out along the ground.
pub(super) fn stand_the_camera(
    planet: Res<Planet>,
    mut cameras: Query<(&mut Transform, &mut Projection, Has<FreeCamera>), With<MainCamera>>,
) {
    for (mut transform, mut projection, free) in &mut cameras {
        if let Projection::Perspective(perspective) = &mut *projection {
            perspective.far = far_m(&planet);
        }
        if free {
            *transform = looking_out(&planet);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{camera::RenderTarget, ecs::system::RunSystemOnce};

    fn planet(radius_m: f32) -> Planet {
        Planet::smooth(PlanetSpec {
            seed: Seed(1),
            radius_m,
        })
    }

    /// The camera is made before any world, and each world entered, the first included,
    /// stands it above its start and sets its far plane: a bigger planet than the first is
    /// not cut off at the first's.
    #[test]
    fn the_camera_sees_as_far_as_each_world_asks() {
        let mut world = World::new();
        world.run_system_once(spawn_camera).unwrap();
        let seen = |world: &mut World| {
            let mut cameras = world.query_filtered::<(&Transform, &Projection), With<MainCamera>>();
            let (at, projection) = cameras.single(world).unwrap();
            let Projection::Perspective(seen) = projection else {
                panic!("the camera is a perspective one");
            };
            (at.translation, seen.far)
        };
        let mut far = Vec::new();
        for radius_m in [2000.0, 8000.0] {
            let planet = planet(radius_m);
            let start = planet.start();
            world.insert_resource(planet);
            world.run_system_once(stand_the_camera).unwrap();
            let (at, seen_m) = seen(&mut world);
            // Past the world's far side, and a little above its start.
            assert!(
                seen_m > 2.0 * radius_m,
                "{seen_m} m on a {radius_m} m planet"
            );
            assert!(at.distance(start) < 50.0, "{at} against the start {start}");
            far.push(seen_m);
        }
        assert!(far[1] > far[0], "{far:?}");
    }

    /// A new world's start is never under a lake's water: neither its base's plot nor where
    /// the player arrives by the door, about 7 m east and 12 m south of its corner
    /// (`terra_bases::Site::approach`). Seed 9697367627075512559 stood knee-deep in a lake, its
    /// start chosen before its lakes were known; the rest are random seeds of the sweep's. So is
    /// a start a site's rules found (`--site`): seed 7's tundra start stood knee-deep in a pool,
    /// the site found before its water was known. Slow, a 30 km world a case, on the CPU:
    /// `cargo xtask remote-render --cpu -- cargo test --release -p terra_world --lib
    /// a_new_worlds_start -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_new_worlds_start_is_never_under_a_lakes_water() {
        let seeds: [u64; 8] = [
            9697367627075512559,
            11178272265655892607,
            17407411704616813729,
            17492275374842887031,
            14695981039346656037,
            14695981039346656045,
            14695981039346656053,
            14695981039346656061,
        ];
        let sited = [("tundra", 7), ("settlement", 7), ("town", 2)];
        let cases = seeds
            .iter()
            .map(|&seed| (None, seed))
            .chain(sited.iter().map(|&(site, seed)| (Some(site), seed)));
        let mut wet = Vec::new();
        for (site, seed) in cases {
            let args = ["--experiment", "play", "--radius", "30000", "--seed"];
            let mut args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            args.push(seed.to_string());
            if let Some(site) = site {
                args.extend(["--site".to_string(), site.to_string()]);
            }
            let options = Options::from_args(args.into_iter()).unwrap();
            let seed = format!("{} {seed}", site.unwrap_or("seed"));
            let at = std::time::Instant::now();
            let (planet, start, _, _) = land_of(&first_world(&options), &options, None, None);
            let lakes = planet.waters().expect("a 30 km world's lakes are found");
            // The plot and 3 m round it, every metre, and the way out of the door to where the
            // player arrives.
            let mut points: Vec<(f32, f32)> = Vec::new();
            for east in -9..=24 {
                for south in -9..=6 {
                    points.push((east as f32, south as f32));
                }
            }
            points.extend((6..=14).map(|south| (7.25, south as f32)));
            let under: Vec<(f32, f32)> = points
                .into_iter()
                .filter(|&(east, south)| {
                    let ground = planet.ground_at(start.at(east, south));
                    lakes
                        .lake_level(ground.normalize())
                        .is_some_and(|level| level > ground.length())
                })
                .collect();
            eprintln!(
                "{seed}: worked out in {:.1} s, {} lakes, {} points under water",
                at.elapsed().as_secs_f32(),
                lakes.lakes().iter().filter(|lake| lake.cells > 0).count(),
                under.len()
            );
            if let Some((east, south)) = under.first() {
                wet.push(format!(
                    "{seed}: {} points under a lake, the first {east} m east and {south} m south",
                    under.len()
                ));
            }
        }
        assert!(wet.is_empty(), "{}", wet.join("; "));
    }

    /// Shadows' levels of detail are measured from the main camera, not from whichever
    /// camera has the lowest entity, as the interface's made first has (D612).
    #[test]
    fn shadows_are_measured_from_the_main_camera() {
        let mut world = World::new();
        // Both draw into the window, as the interface's camera and the main one do in play.
        world.spawn(Camera::default());
        world.run_system_once(spawn_camera).unwrap();
        world.spawn(Camera::default());
        let origin = world
            .run_system_once(
                |mut cameras: Query<(Entity, &RenderTarget), With<Camera>>,
                 mut origins: Query<Entity, With<ShadowLodOrigin>>,
                 mut lights: Query<Entity, Or<(With<PointLight>, With<SpotLight>)>>| {
                    bevy::light::get_shadow_lod_origin(
                        cameras.transmute_lens_filtered(),
                        origins.transmute_lens_filtered(),
                        lights.transmute_lens_filtered(),
                    )
                },
            )
            .unwrap();
        let main = world
            .query_filtered::<Entity, With<MainCamera>>()
            .single(&world)
            .unwrap();
        assert_eq!(origin, Some(main));
    }
}
