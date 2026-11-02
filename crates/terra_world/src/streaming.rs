//! What the start's view has far off, worked out after the loading screen lifts (load-time
//! j3): the ground's coarsest levels and the woods' coarse cards, on a pool of threads of
//! their own, as open worlds stream their distant cells in round a player already playing
//! (Unreal's World Partition and HLODs, Skyrim's distant LOD). The loading screen waits for
//! what is near; a scripted run is still held for the rest (`loading::hold`), so it sees the
//! same world on any machine.

use crate::loading::Screen;
use bevy::{
    prelude::*,
    tasks::{TaskPool, TaskPoolBuilder},
};

/// The pool the far things are worked out on: all but two cores, so the game's own threads
/// (the main, render and Bevy's pools) keep cores to draw on while it works.
#[derive(Resource)]
pub(crate) struct Streaming(pub(crate) TaskPool);

impl Default for Streaming {
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        Self(
            TaskPoolBuilder::new()
                .num_threads(cores.saturating_sub(2).max(1))
                .thread_name("Streaming far".into())
                .build(),
        )
    }
}

/// Whether the loading screen has lifted, so the far things may start: any screen but the
/// loading one, or none (a test's app).
pub(crate) fn lifted(screen: Option<Res<State<Screen>>>) -> bool {
    screen.is_none_or(|screen| *screen.get() != Screen::Loading)
}
