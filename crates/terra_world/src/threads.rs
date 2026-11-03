//! Bevy's pools and the game's own, each sized from the one table of the game's threads
//! (`terra_gen::par::Threads`, D606): Bevy's compute, async compute and IO pools through
//! `TaskPoolOptions`, and every pool of the game's own ([`pool`]). A pool made anywhere else
//! sizes itself as if it were alone, as the far view's streaming did when it took 14 of the
//! bench's 16 cores (perf-regress j1); the one-definition check fails one.

use bevy::{
    app::{TaskPoolOptions, TaskPoolPlugin, TaskPoolThreadAssignmentPolicy},
    tasks::{TaskPool, TaskPoolBuilder},
};
use terra_gen::par::{Phase, Threads, Work};

/// Bevy's task pools as the table has them on this machine.
pub fn plugin() -> TaskPoolPlugin {
    TaskPoolPlugin {
        task_pool_options: options(terra_gen::par::threads()),
    }
}

/// Bevy's three pools from `threads`: each pool's count fixed, and their total all of them,
/// so Bevy's own share-out (IO, then async compute, then compute the rest) gives each its own.
fn options(threads: Threads) -> TaskPoolOptions {
    let fixed = |count: usize| TaskPoolThreadAssignmentPolicy {
        min_threads: count,
        max_threads: count,
        percent: 1.0,
        on_thread_spawn: None,
        on_thread_destroy: None,
    };
    let total = threads.io + threads.async_compute + threads.frame;
    TaskPoolOptions {
        min_total_threads: total,
        max_total_threads: total,
        io: fixed(threads.io),
        async_compute: fixed(threads.async_compute),
        compute: fixed(threads.frame),
    }
}

/// A pool of the game's own for `work`, of the most threads the table gives it in either
/// phase, named `name` (as a profiler shows its threads): its sender keeps no more tasks
/// running than the phase's share (`terra_gen::par::share`), so building's pool has the
/// loading screen's threads and in play works on its quarter's. A pool's threads are fixed
/// when it is made, and an idle one costs nothing. A background pool's threads each work
/// their task alone (`terra_gen::par::keep_to_this_thread`), so a map asked on one never fans
/// out over the
/// cores the frame's threads hold, as Unreal's background workers sit below the game and
/// render threads and Godot's low-priority tasks keep to their share.
pub fn pool(work: Work, name: &str) -> TaskPool {
    let builder = TaskPoolBuilder::new()
        .num_threads(terra_gen::par::threads().most(work))
        .thread_name(name.to_string());
    if matches!(work, Work::Building | Work::Streaming) {
        builder
            .on_thread_spawn(terra_gen::par::keep_to_this_thread)
            .build()
    } else {
        builder.build()
    }
}

/// The loading screen goes up: building's work takes the load's share of the cores (D606).
pub(crate) fn loading() {
    terra_gen::par::set_phase(Phase::Load);
}

/// The loading screen lifts: building's work keeps to its quarter of the cores (D606).
pub(crate) fn playing() {
    terra_gen::par::set_phase(Phase::Play);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bevy_gives_each_of_its_pools_the_tables_count() {
        // Bevy's own share-out, read from its source (bevy_app 0.20, `task_pool_plugin.rs`):
        // IO first, then async compute from what is left, then compute the rest.
        for cores in [4, 8, 16, 32] {
            let threads = Threads::on(cores);
            let options = options(threads);
            let total = options.min_total_threads;
            assert_eq!(options.max_total_threads, total);
            let mut left = total;
            for (policy, wanted) in [
                (&options.io, threads.io),
                (&options.async_compute, threads.async_compute),
                (&options.compute, threads.frame),
            ] {
                let share = ((total as f32 * policy.percent).round() as usize)
                    .min(left)
                    .clamp(policy.min_threads, policy.max_threads);
                assert_eq!(share, wanted, "{cores} cores");
                left = left.saturating_sub(share);
            }
        }
    }

    #[test]
    fn a_background_pool_has_its_share_and_never_fans_out() {
        let pool = pool(Work::Building, "test");
        assert_eq!(pool.thread_num(), terra_gen::par::threads().load_building);
        let items: Vec<u32> = (0..64).collect();
        let (here, workers) = bevy::tasks::block_on(pool.spawn(async move {
            let workers = terra_gen::par::map(&items, |_| {
                std::thread::sleep(std::time::Duration::from_millis(1));
                std::thread::current().id()
            });
            (std::thread::current().id(), workers)
        }));
        assert!(workers.iter().all(|id| *id == here), "the map fanned out");
    }
}
