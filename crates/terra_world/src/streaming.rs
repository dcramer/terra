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

/// The pool the far things are worked out on: a quarter of the cores, each thread working its
/// task alone (`terra_gen::par::keep_to_this_thread`), so the game's own threads (the main,
/// render and Bevy's pools) keep their cores while it works, as Unreal's background workers
/// sit below the game and render threads. With all but two of the cores (14 of the bench's
/// 16) the lake's main thread took 15 to 22 ms a frame for the 30 s the far ground streamed,
/// not 6.5 ms; nice 19 did not help (load-time j5), as the threads shared the game's cores'
/// caches and hyperthreads, not only their time (perf-regress j1, D603).
#[derive(Resource)]
pub(crate) struct Streaming(pub(crate) TaskPool);

impl Default for Streaming {
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        Self(
            TaskPoolBuilder::new()
                .num_threads(threads(cores))
                .thread_name("Streaming far".into())
                .on_thread_spawn(terra_gen::par::keep_to_this_thread)
                .build(),
        )
    }
}

/// The streaming pool's threads on a machine of `cores`: a quarter, at least one.
fn threads(cores: usize) -> usize {
    (cores / 4).max(1)
}

/// Whether the loading screen has lifted, so the far things may start: any screen but the
/// loading one, or none (a test's app).
pub(crate) fn lifted(screen: Option<Res<State<Screen>>>) -> bool {
    screen.is_none_or(|screen| *screen.get() != Screen::Loading)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_streaming_pool_leaves_the_game_three_quarters_of_the_cores_and_never_fans_out() {
        // A quarter of any machine's cores, at least one: the bench's 16 give 4.
        for (cores, most) in [(1, 1), (4, 1), (16, 4), (32, 8)] {
            assert_eq!(threads(cores), most, "{cores} cores");
        }
        // A map asked on one of its threads stays on that thread (the woods' far cards ask
        // one), however many cores the machine has.
        let pool = Streaming::default();
        let items: Vec<u32> = (0..64).collect();
        let (here, workers) = bevy::tasks::block_on(pool.0.spawn(async move {
            let workers = terra_gen::par::map(&items, |_| {
                std::thread::sleep(std::time::Duration::from_millis(1));
                std::thread::current().id()
            });
            (std::thread::current().id(), workers)
        }));
        assert!(workers.iter().all(|id| *id == here), "the map fanned out");
    }
}
