//! The ground streamed round the camera: the first view is the one seen, and no frame waits
//! for the ground a move brings, a scripted run's included (it is held instead,
//! `loading::hold`). Waiting, the play world's first frame took 277 s.

use super::*;
use crate::shell::Options;
use std::time::{Duration, Instant};
use terra_core::{PlanetSpec, Seed};

/// A small world drawn by the terrain plugin alone, as a scripted run, the camera standing
/// above the planet's start.
fn app() -> App {
    let planet = Planet::new(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    let start = planet.start();
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        bevy::state::app::StatesPlugin,
        crate::instance::plugin,
    ))
    .insert_resource(planet)
    .insert_resource(GroundLook(Handle::default()))
    .insert_resource(Assets::<Mesh>::default())
    .insert_resource(crate::quality::Quality::High)
    .insert_resource(Progress::default())
    .insert_resource(Options::from_args(["--shot", "x.png"].map(String::from).into_iter()).unwrap())
    .add_plugins(plugin);
    stand(&mut app, start + start.normalize() * 2.0);
    // The first view is meshed on entering the world.
    let world = crate::instance::WorldSpec {
        key: crate::instance::InstanceKey(0),
        kind: "world".into(),
        seed: 1,
    };
    app.world_mut()
        .commands()
        .queue(crate::instance::Cross(world));
    app.world_mut().flush();
    app
}

/// Puts the camera at a place, spawning it the first time.
fn stand(app: &mut App, at: Vec3) {
    let place = (
        Transform::from_translation(at),
        GlobalTransform::from_translation(at),
    );
    let world = app.world_mut();
    match world
        .query_filtered::<Entity, With<MainCamera>>()
        .iter(world)
        .next()
    {
        Some(camera) => {
            world.entity_mut(camera).insert(place);
        }
        None => {
            world.spawn((MainCamera, place.0, place.1));
        }
    }
}

fn chunks(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    let mut chunks: Vec<Entity> = world
        .query_filtered::<Entity, With<Chunk>>()
        .iter(world)
        .collect();
    chunks.sort();
    chunks
}

fn coming(app: &App) -> usize {
    app.world().resource::<Terrain>().coming()
}

#[test]
fn the_first_view_is_the_one_seen() {
    let mut app = app();
    app.update();
    let first = chunks(&mut app);
    assert!(!first.is_empty(), "the first view has ground");
    // The preset says it changed on its first frame; nothing is chosen or meshed again.
    for _ in 0..5 {
        app.update();
        assert_eq!(
            coming(&app),
            0,
            "nothing more is coming for a camera that stood still"
        );
    }
    assert_eq!(
        chunks(&mut app),
        first,
        "the first view's chunks stay, none meshed again"
    );
}

#[test]
fn a_frame_that_moves_the_camera_on_does_not_wait_for_the_new_ground() {
    let mut app = app();
    app.update();
    let first = chunks(&mut app);
    // 300 m along the surface: most of the near chunks are new.
    let start = app.world().resource::<Planet>().start();
    let along = start.normalize().cross(Vec3::X).normalize();
    let there = app
        .world()
        .resource::<Planet>()
        .ground_at(start + along * 300.0);
    stand(&mut app, there + there.normalize() * 2.0);
    app.update();
    assert!(
        coming(&app) > 0,
        "the frame that moved went on before the new ground was chosen"
    );
    assert_eq!(chunks(&mut app), first, "and showed the ground as it was");
    // It comes over the frames after, as each chunk is ready.
    let began = Instant::now();
    while coming(&app) > 0 {
        assert!(
            began.elapsed() < Duration::from_secs(60),
            "the new ground came"
        );
        std::thread::sleep(Duration::from_millis(2));
        app.update();
    }
    let terrain = app.world().resource::<Terrain>();
    assert!(
        terrain
            .wanted
            .keys()
            .all(|id| terrain.shown.contains_key(id)),
        "every wanted chunk is shown"
    );
    assert!(
        terrain
            .shown
            .keys()
            .all(|id| terrain.wanted.contains_key(id)),
        "and only those"
    );
}
