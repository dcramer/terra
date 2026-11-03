//! What the start's view has far off, worked out after the loading screen lifts (load-time
//! j3): the ground's coarsest levels and the woods' coarse cards, on a pool of threads of
//! their own, as open worlds stream their distant cells in round a player already playing
//! (Unreal's World Partition and HLODs, Skyrim's distant LOD). The loading screen waits for
//! what is near; a scripted run is still held for the rest (`loading::hold`), so it sees the
//! same world on any machine.

use crate::loading::Screen;
use bevy::{prelude::*, tasks::TaskPool};
use terra_gen::par::Work;

/// The pool the far things are worked out on: the streaming's share of the background in the
/// table of the game's threads (D606), each thread working its task alone
/// (`terra_gen::par::keep_to_this_thread`), so the game's own threads (the main, render and
/// Bevy's pools) keep their cores while it works, as Unreal's background workers sit below
/// the game and render threads. With all but two of the cores (14 of the bench's 16) the
/// lake's main thread took 15 to 22 ms a frame for the 30 s the far ground streamed, not 6.5
/// ms; nice 19 did not help (load-time j5), as the threads shared the game's cores' caches and
/// hyperthreads, not only their time (perf-regress j1, D603).
#[derive(Resource)]
pub(crate) struct Streaming(pub(crate) TaskPool);

impl Default for Streaming {
    fn default() -> Self {
        Self(crate::threads::pool(Work::Streaming, "Streaming far"))
    }
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
    fn the_streaming_pool_has_its_share_of_the_background_and_never_fans_out() {
        let pool = Streaming::default();
        assert_eq!(pool.0.thread_num(), terra_gen::par::threads().streaming);
        // A map asked on one of its threads stays on that thread (the woods' far cards ask
        // one), however many cores the machine has.
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
