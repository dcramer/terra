//! The world's own modules leave nothing behind (docs/instances.md, "The test that proves
//! nothing leaks"), without a GPU: the land, sky, air, lakes, scene, day, climate, weather
//! and wind are built on entering a world as in the game, from their own `entering`, and
//! after a round trip to another world and back the census equals the one before.

use super::*;
use crate::planet::{MainCamera, Planet};
use crate::shell::Options;
use bevy::{
    asset::AssetPlugin, light::atmosphere::ScatteringMedium,
    post_process::auto_exposure::AutoExposureCompensationCurve, state::app::StatesPlugin,
};

fn spec(seed: u64) -> WorldSpec {
    WorldSpec {
        key: InstanceKey(seed),
        kind: "world".into(),
        seed,
    }
}

/// The camera, as the planet's start-up makes it, and the first world crossed to.
fn start(mut commands: Commands) {
    commands.spawn((Name::new("Camera"), MainCamera, Transform::default()));
    commands.queue(Cross(spec(1)));
}

/// A sun, as the planet's set-up puts one in each world, for the scene to style.
fn sun(mut commands: Commands) {
    commands.spawn((
        Name::new("Sun"),
        DirectionalLight::default(),
        Transform::default(),
    ));
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
        .add_plugins(plugin)
        .insert_resource(
            Options::from_args(["--shot", "x.png"].map(String::from).into_iter()).unwrap(),
        )
        .insert_resource(crate::daylight::WorldClimate(crate::data::read(
            crate::daylight::WorldClimate::FILE,
        )))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<crate::sky::SkyMaterial>()
        .init_asset::<ScatteringMedium>()
        .init_asset::<AutoExposureCompensationCurve>()
        .init_asset::<crate::lakes::LakeMaterial>()
        .init_asset::<crate::lakes::MistMaterial>()
        .census_assets::<Mesh>()
        .census_assets::<Image>()
        .census_assets::<crate::sky::SkyMaterial>()
        .census_assets::<ScatteringMedium>()
        .census_assets::<AutoExposureCompensationCurve>()
        .census_assets::<crate::lakes::LakeMaterial>()
        .census_assets::<crate::lakes::MistMaterial>()
        .world_resource::<Planet>()
        .world_resource::<crate::start::Start>()
        .world_resource::<crate::start::Sited>()
        .world_resource::<crate::planet::Tweaks>()
        .add_systems(Startup, start)
        .add_systems(
            OnEnter(InWorld),
            (
                crate::planet::entering::land.in_set(Building::Land),
                sun.in_set(Building::Place),
            ),
        );
    crate::sky::entering(&mut app);
    crate::air::entering(&mut app);
    crate::lakes::entering(&mut app);
    crate::scene::entering(&mut app);
    crate::daylight::entering(&mut app);
    crate::climate::entering(&mut app);
    crate::weather::entering(&mut app);
    crate::wind::entering(&mut app);
    settle(&mut app);
    app
}

/// Enough frames for a crossing to leave, enter and play, and for dropped assets to go.
fn settle(app: &mut App) {
    for _ in 0..6 {
        app.update();
    }
}

fn cross(app: &mut App, to: WorldSpec) {
    app.world_mut().commands().queue(Cross(to));
    app.world_mut().flush();
    settle(app);
}

fn names(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query::<&Name>();
    query.iter(world).map(|name| name.to_string()).collect()
}

#[test]
fn the_worlds_modules_leave_nothing_behind() {
    let mut app = app();
    let before = Census::take(app.world());
    for (what, wanted) in [
        ("the sky", "::sky::Sky"),
        ("the clock", "::daylight::Clock"),
        ("the climate", "::climate::ClimateFields"),
        ("the air", "::air::AirScale"),
    ] {
        assert!(
            before.resources.iter().any(|name| name.ends_with(wanted)),
            "{what} is made on entering: {:?}",
            before.resources
        );
    }
    let first = names(&mut app);
    for made in ["Sky", "Air", "Haze", "Sun"] {
        assert!(
            first.iter().any(|name| name == made),
            "{made} is in the world: {first:?}"
        );
    }

    cross(&mut app, spec(2));
    assert_eq!(app.world().resource::<WorldSpec>().seed, 2);
    let there = Census::take(app.world());
    for wanted in [
        "::sky::Sky",
        "::daylight::Clock",
        "::climate::ClimateFields",
    ] {
        assert!(
            there.resources.iter().any(|name| name.ends_with(wanted)),
            "{wanted} is made again in the next world: {:?}",
            there.resources
        );
    }
    let skies = names(&mut app).iter().filter(|name| *name == "Sky").count();
    assert_eq!(skies, 1, "the first world's sky went with it");

    cross(&mut app, spec(1));
    let after = Census::take(app.world());
    assert_eq!(before.diff(&after), Vec::<String>::new());
    assert_eq!(before, after);
    assert!(before.scoped > 0 && before.global == 1, "{before:?}");
}
