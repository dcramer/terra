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
//!
//! **Warming** (docs/instances.md, "The crossing and the way back": the world beyond is
//! known before it is reached): [`Warm`] begins a world's land on its thread ahead of the
//! crossing, as a player nears the gate to it, and the crossing takes that land up where it
//! has come to, so what was made while they walked is not waited for under the cover. One
//! world is kept warm, the last asked for.

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
    thread: Option<JoinHandle<(Land, Vec<PutIn>)>>,
    stage: Arc<Stage>,
    /// When it began, by the wall clock, and the frames drawn since, for the log.
    began: Instant,
    frames: u32,
}

/// The world being made ahead of a crossing to it ([`Warm`]), by its key: none, or the last
/// asked for. The app's, not a world's: it is made while another world is played.
#[derive(Resource, Default)]
struct Warming(Option<(crate::instance::InstanceKey, Making)>);

/// Begins making a world's land ahead of a crossing to it, if it is not already being made:
/// asked as often as wanted, begun once.
pub struct Warm(pub WorldSpec);

impl Command for Warm {
    type Out = ();

    fn apply(self, world: &mut World) {
        // Only where a world's land is made on its thread at all (a loading screen).
        if !world.contains_resource::<State<Screen>>() {
            return;
        }
        let warm = world.resource::<Warming>();
        if warm.0.as_ref().is_some_and(|(key, _)| *key == self.0.key) {
            return;
        }
        info!(
            "Land: warming {} {} ahead of a crossing",
            self.0.kind, self.0.key.0
        );
        let making = start(world, self.0.clone());
        // Another kept warm before is let go: its thread ends on its own, its land dropped.
        world.resource_mut::<Warming>().0 = Some((self.0.key, making));
    }
}

/// The land made, for [`super::land`] to put in as the world is entered.
#[derive(Resource)]
pub(in crate::planet) struct Made(pub Land);

/// What a module puts in the world of what it made with the land.
pub type PutIn = Box<dyn FnOnce(&mut World) + Send>;

/// What modules make from the land on its own thread once it is made, before the world is
/// entered (D623), each by name: work of seconds that needs nothing but the land (the woods'
/// trees laid out), which in `OnEnter(InWorld)` held the frame the world is entered in, the
/// loading screen frozen 12 s at the lake (crossing-freeze j1). What each gives back is put
/// in the world as the land comes back; a run with no loading screen makes none of it, and
/// the module makes it as it enters.
#[derive(Resource, Default, Clone)]
pub struct WithTheLand(Vec<(&'static str, fn(&Land) -> PutIn)>);

impl WithTheLand {
    /// `make` is made from the land on its thread, after it, as `what`.
    pub fn add(&mut self, what: &'static str, make: fn(&Land) -> PutIn) {
        self.0.push((what, make));
    }
}

pub(in crate::planet) fn plugin(app: &mut App) {
    app.world_resource::<Making>()
        .world_resource::<Made>()
        .init_resource::<WithTheLand>()
        .init_resource::<Warming>()
        .add_systems(OnEnter(Travel::Generating), begin)
        .add_systems(Crossing, come_back.before(crate::instance::carry_out))
        .add_systems(Update, said.in_set(Track));
}

/// Starts making the world's land on its own thread, when a loading screen is there to be
/// drawn meanwhile; or takes it up where it has come to, if it was begun ahead ([`Warm`]).
fn begin(world: &mut World) {
    if !world.contains_resource::<State<Screen>>() {
        return;
    }
    let spec = world.resource::<WorldSpec>().clone();
    let warmed = world.resource_mut::<Warming>().0.take();
    let making = match warmed {
        Some((key, making)) if key == spec.key => {
            info!(
                "Land: begun {:.1} s ahead of the crossing",
                making.began.elapsed().as_secs_f32()
            );
            making
        }
        _ => start(world, spec),
    };
    world.insert_resource(making);
    world.resource_mut::<Generating>().begin(LAND);
}

/// A world's land begun on a thread of its own.
fn start(world: &World, spec: WorldSpec) -> Making {
    let options = world.resource::<Options>().clone();
    let known = world.get_resource::<KnownStart>().copied();
    let known_towns = world.get_resource::<KnownTowns>().cloned();
    let with = world.resource::<WithTheLand>().clone();
    let stage = Arc::new(Stage::default());
    let saying = Arc::clone(&stage);
    let thread = std::thread::Builder::new()
        .name("terra-land".into())
        .spawn(move || {
            let land = made_land(
                &spec,
                &options,
                known.as_ref(),
                known_towns.as_ref(),
                &saying,
            );
            let put_in = with
                .0
                .iter()
                .map(|(what, make)| {
                    let began = Instant::now();
                    let put_in = make(&land);
                    info!(
                        "Land: {what} made with it in {:.1} s",
                        began.elapsed().as_secs_f32()
                    );
                    put_in
                })
                .collect();
            (land, put_in)
        })
        .unwrap_or_else(|why| {
            panic!("the land's thread could not be started ({why}); the machine is out of threads or memory")
        });
    Making {
        thread: Some(thread),
        stage,
        began: Instant::now(),
        frames: 0,
    }
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
    let (land, put_in) = thread
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    info!(
        "Land: made in {:.1} s on its own thread, {} frames of the loading screen drawn meanwhile",
        making.began.elapsed().as_secs_f32(),
        making.frames
    );
    world.remove_resource::<Making>();
    for put_in in put_in {
        put_in(world);
    }
    world.insert_resource(Made(land));
    world.resource_mut::<Generating>().done(LAND);
}

/// The seconds each part of the land takes, as [`Stage::SAID`] names them, then the rest of
/// the load from the land made to the first playable frame, which the first view's ground
/// is the last of: seed 9697367627075512559's New world, release (job
/// 20261105-145004-9d5a-patch, a busy box): the start searched for in 3.6 s, the first town
/// found in 16.9 s, the land made in 40.1 s, loaded in 91.4 s. Only their shares matter.
const SHARES_S: [f32; 4] = [3.6, 19.1, 0.5, 16.9];
const AFTER_THE_LAND_S: f32 = 51.3;

/// The loading screen says which part of the land is being made, and the bar gives each
/// part, and the ground after it, its measured share of the load (`loading::steps`): those
/// made are done, the one under way is coming, those after are expected.
fn said(making: Option<ResMut<Making>>, mut progress: ResMut<Progress>) {
    let Some(mut making) = making else {
        return;
    };
    making.frames += 1;
    let now = making.stage.part();
    for (part, (what, share_s)) in Stage::SAID.into_iter().zip(SHARES_S).enumerate() {
        progress.expect(what, share_s);
        match part.cmp(&now) {
            std::cmp::Ordering::Less => progress.report(what, 0, 1),
            std::cmp::Ordering::Equal => progress.report(what, 1, 0),
            std::cmp::Ordering::Greater => {}
        }
    }
    progress.expect(crate::terrain::SHAPING, AFTER_THE_LAND_S);
}
