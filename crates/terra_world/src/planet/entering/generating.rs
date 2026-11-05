//! A world's land made off the main thread, behind the loading screen (D623), as shipped
//! games generate a world: the start searched for, the land round it worked out and its
//! first town found take 8 to 23 s on a 30 km world, and were made before the first frame,
//! the window blank. Now the loading screen is drawn meanwhile, saying which part is being
//! made, and the shaders compile beside it.
//!
//! The land is made on a thread of its own, not the game's background pools: they keep work
//! shared out on them to one thread (`crate::threads`), and the land's own work spreads over
//! every core (`terra_gen::par::map`). The world is entered once it comes back, in the same frame
//! (`crate::instance::Generating`). A run with no loading screen, a layer's test, makes it
//! as the world is entered, as before (`super::land`).

use super::{KnownStart, KnownTowns, Land, Stage, made_land};
use crate::instance::{Crossing, Generating, InstanceApp, Travel, WorldSpec};
use crate::loading::{Progress, Screen, Track};
use crate::shell::Options;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use std::sync::Arc;
use std::thread::JoinHandle;

/// What the world waits on while its land is made, by name.
const LAND: &str = "land";

/// The land being made on its thread, and how far it has come.
#[derive(Resource)]
struct Making {
    thread: Option<JoinHandle<Land>>,
    stage: Arc<Stage>,
    /// When it began, by the wall clock, and the frames drawn since, for the log.
    began: Instant,
    frames: u32,
}

/// The land made, for [`super::land`] to put in as the world is entered.
#[derive(Resource)]
pub(in crate::planet) struct Made(pub Land);

pub(in crate::planet) fn plugin(app: &mut App) {
    app.world_resource::<Making>()
        .world_resource::<Made>()
        .add_systems(OnEnter(Travel::Generating), begin)
        .add_systems(Crossing, come_back.before(crate::instance::carry_out))
        .add_systems(Update, said.in_set(Track));
}

/// Starts making the world's land on its own thread, when a loading screen is there to be
/// drawn meanwhile.
fn begin(world: &mut World) {
    if !world.contains_resource::<State<Screen>>() {
        return;
    }
    let spec = world.resource::<WorldSpec>().clone();
    let options = world.resource::<Options>().clone();
    let known = world.get_resource::<KnownStart>().copied();
    let known_towns = world.get_resource::<KnownTowns>().cloned();
    let stage = Arc::new(Stage::default());
    let saying = Arc::clone(&stage);
    let thread = std::thread::Builder::new()
        .name("terra-land".into())
        .spawn(move || {
            made_land(
                &spec,
                &options,
                known.as_ref(),
                known_towns.as_ref(),
                &saying,
            )
        })
        .unwrap_or_else(|why| {
            panic!("the land's thread could not be started ({why}); the machine is out of threads or memory")
        });
    world.insert_resource(Making {
        thread: Some(thread),
        stage,
        began: Instant::now(),
        frames: 0,
    });
    world.resource_mut::<Generating>().begin(LAND);
}

/// The land, once made, kept for entering; a panic making it (a `--site` the land has no
/// place for) goes on on the main thread, as it did there.
fn come_back(world: &mut World) {
    let Some(mut making) = world.get_resource_mut::<Making>() else {
        return;
    };
    if !making.thread.as_ref().is_some_and(JoinHandle::is_finished) {
        return;
    }
    let Some(thread) = making.thread.take() else {
        return;
    };
    let land = thread
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    info!(
        "Land: made in {:.1} s on its own thread, {} frames of the loading screen drawn meanwhile",
        making.began.elapsed().as_secs_f32(),
        making.frames
    );
    world.remove_resource::<Making>();
    world.insert_resource(Made(land));
    world.resource_mut::<Generating>().done(LAND);
}

/// The loading screen says which part of the land is being made.
fn said(making: Option<ResMut<Making>>, mut progress: ResMut<Progress>) {
    if let Some(mut making) = making {
        making.frames += 1;
        progress.report(making.stage.said(), 1, 0);
    }
}
