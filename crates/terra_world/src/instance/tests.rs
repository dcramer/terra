//! A round trip to another world and back leaks nothing (docs/instances.md, "The test that
//! proves nothing leaks"), without a GPU: a stand-in module spawns placed and unplaced
//! entities, one of them disabled, a world resource of assets, one of them also held by a
//! global, and a component on a global; the census after coming back equals the one before.
//! A stand-in that forgets to scope an unplaced entity is caught by the same census.

use super::*;
use bevy::{asset::AssetPlugin, state::app::StatesPlugin};

#[derive(Asset, TypePath)]
struct Stuff;

/// The world's own assets, and the global gear's.
#[derive(Resource)]
struct Grove(Vec<Handle<Stuff>>);

/// What the world puts on the global camera.
#[derive(Component)]
struct Tint;

/// The player's gear: a global's asset, which a world also holds.
#[derive(Resource)]
struct Gear(Handle<Stuff>);

#[derive(Component)]
struct Camera;

/// Whether the stand-in forgets to scope its unplaced entity.
#[derive(Resource)]
struct Leaky(bool);

fn spec(n: u64, kind: &str) -> WorldSpec {
    WorldSpec {
        key: InstanceKey(n),
        kind: kind.into(),
        seed: n,
    }
}

fn start(mut commands: Commands, mut stuff: ResMut<Assets<Stuff>>) {
    let gear = stuff.add(Stuff);
    commands.insert_resource(Gear(gear));
    commands
        .spawn((Name::new("camera"), Camera, Global, Transform::default()))
        .with_child((Name::new("bone"), Transform::default()));
    commands.queue(Cross(spec(1, "forest")));
}

fn build(
    mut commands: Commands,
    spec: Res<WorldSpec>,
    mut stuff: ResMut<Assets<Stuff>>,
    gear: Res<Gear>,
    camera: Single<Entity, With<Camera>>,
    leaky: Res<Leaky>,
) {
    for n in 0..3 {
        commands
            .spawn((
                Name::new(format!("tree of {}", spec.kind)),
                Transform::from_xyz(n as f32, 0.0, 0.0),
            ))
            .with_child((Name::new("branch"), Transform::default()));
    }
    commands.spawn((Name::new("sleeping"), Transform::default(), Disabled));
    commands.spawn((Name::new("sound bed"), WorldScoped));
    if leaky.0 {
        commands.spawn(Name::new("weather emitter"));
    }
    commands.insert_resource(Grove(vec![
        stuff.add(Stuff),
        stuff.add(Stuff),
        gear.0.clone(),
    ]));
    commands.entity(*camera).insert(Tint);
}

fn app(leaky: bool) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
        .add_plugins(plugin)
        .init_asset::<Stuff>()
        .insert_resource(Leaky(leaky))
        .world_resource::<Grove>()
        .world_component::<Tint>()
        .census_assets::<Stuff>()
        .add_systems(Startup, start)
        .add_systems(OnEnter(InWorld), build);
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

fn travel(app: &App) -> Travel {
    *app.world().resource::<State<Travel>>().get()
}

fn names(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query_filtered::<&Name, Allow<Disabled>>();
    query.iter(world).map(|name| name.to_string()).collect()
}

fn grove_ids(app: &App) -> Vec<AssetId<Stuff>> {
    let grove = app.world().resource::<Grove>();
    grove.0.iter().map(Handle::id).collect()
}

#[test]
fn a_round_trip_leaks_nothing() {
    let mut app = app(false);
    assert_eq!(travel(&app), Travel::Playing);
    let before = Census::take(app.world());
    let forest = grove_ids(&app);
    let gear = app.world().resource::<Gear>().0.id();

    cross(&mut app, spec(2, "tundra"));
    assert_eq!(travel(&app), Travel::Playing);
    assert_eq!(app.world().resource::<WorldSpec>().kind, "tundra");
    let there = names(&mut app);
    assert!(
        !there.iter().any(|name| name == "tree of forest"),
        "the forest's trees went with it: {there:?}"
    );
    let stuff = app.world().resource::<Assets<Stuff>>();
    assert!(
        stuff.get(forest[0]).is_none() && stuff.get(forest[1]).is_none(),
        "the forest's own assets are freed in the tundra"
    );
    assert!(
        stuff.get(gear).is_some(),
        "the gear, held by a global, stays"
    );

    cross(&mut app, spec(1, "forest"));
    let after = Census::take(app.world());
    assert_eq!(before.diff(&after), Vec::<String>::new());
    assert_eq!(before, after);
    assert!(before.scoped > 0 && before.global == 1, "{before:?}");
    let back = names(&mut app);
    for kept in ["camera", "bone"] {
        assert!(
            back.iter().any(|name| name == kept),
            "{kept} stays: {back:?}"
        );
    }
}

#[test]
fn the_census_catches_an_unscoped_entity() {
    let mut app = app(true);
    let before = Census::take(app.world());
    cross(&mut app, spec(2, "tundra"));
    cross(&mut app, spec(1, "forest"));
    let after = Census::take(app.world());
    let diff = before.diff(&after);
    assert!(
        diff.iter().any(|line| line.contains("[Name]")),
        "the leaked emitter shows: {diff:?}"
    );
    assert_eq!(
        after.neither,
        before.neither + 2,
        "one leaked on each entry"
    );
}

#[test]
fn leaving_takes_the_worlds_part_off_globals_and_its_resources_away() {
    let mut app = app(false);
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<Camera>>()
        .single(app.world())
        .unwrap();
    assert!(app.world().entity(camera).contains::<Tint>());
    assert!(!app.world().entity(camera).contains::<WorldScoped>());
    // Leave for nowhere: the run is away, nothing of the world left.
    app.world_mut()
        .resource_mut::<NextState<Travel>>()
        .set_if_different(Travel::Leaving);
    settle(&mut app);
    assert_eq!(travel(&app), Travel::Away);
    assert!(!app.world().entity(camera).contains::<Tint>());
    assert!(!app.world().contains_resource::<Grove>());
    assert!(!app.world().contains_resource::<WorldSpec>());
    let left = names(&mut app);
    assert_eq!(left.len(), 2, "only the camera and its bone: {left:?}");
}

#[test]
fn a_global_given_a_transform_later_stays_global() {
    let mut app = app(false);
    let late = app.world_mut().spawn((Name::new("late"), Global)).id();
    app.world_mut()
        .entity_mut(late)
        .insert(Transform::default());
    app.world_mut().flush();
    assert!(!app.world().entity(late).contains::<WorldScoped>());
    let placed = app.world_mut().spawn(Transform::default()).id();
    assert!(app.world().entity(placed).contains::<WorldScoped>());
}
