//! The game's threads: how many each kind of work has ([`Threads`], D606), the one table
//! every pool and parallel loop sizes itself from, and work shared out over the cores a thread
//! may use ([`map`]): a map's cells, or a test's sweep over every case of something (D406).
//! A map's result is in the items' order whatever the threads did, so what is worked out stays
//! repeatable.

use std::cell::Cell;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A kind of work the game's threads do, by how soon the game needs it, each with its share
/// of the cores in [`Threads`] (D606). The shape is shipped practice: Unreal's named threads
/// (the game and render threads keep a core each) and its task priorities (`ETaskPriority`:
/// foreground work before background), Godot's `WorkerThreadPool`, whose low-priority tasks
/// hold at most `threading/worker_pool/low_priority_thread_ratio` (0.3) of its threads,
/// Naughty Dog's fiber job system (a worker a core, never more threads than cores; Gyrling,
/// GDC 2015), and bevy_tasks' three pools (compute, async compute and IO, sized by
/// `TaskPoolOptions`, which Bevy alone fills to 8 + 4 + 4 threads on 16 cores).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    /// The frame's own: Bevy's compute pool, which runs the systems and their `par_iter`, with
    /// the main thread (Unreal's foreground workers).
    Frame,
    /// Wanted soon, never waited on by the frame: Bevy's async compute pool (the ground, grass
    /// and the sky's light round the player, the horizon, climate's painting), its threads
    /// each working their task alone ([`keep_to_this_thread`]) so the pool keeps its share.
    Async,
    /// Background: a base laid and drawn again after an edit or on the load, on building's
    /// own pool (`terra_bases::layout::builders`), apart so it never queues behind streaming.
    Building,
    /// Background: the threads a building's job may draw a section's parts on beside its own,
    /// all of them together (`terra_build_mesh::sections::Spare`, D308).
    Parts,
    /// Background: the far view streamed in while the game plays (D603), Unreal's
    /// `BackgroundLow`.
    Streaming,
    /// Files: Bevy's IO pool. Its threads wait on the disk most of the time, so, as Bevy's own
    /// defaults have it, they are not counted against the cores.
    Io,
    /// Behind the loading screen, or a tool's or test's sweep (D406): nothing else wants the
    /// cores then, so every one.
    Load,
}

/// How many threads each kind of [`Work`] has on a machine of `cores` (logical) cores (D606):
/// the main and render threads keep a core each; background work (streaming, building and its
/// parts) holds a quarter of the cores at most, as Godot caps its low-priority tasks; Bevy's
/// async compute pool an eighth, up to four; the frame's compute pool the rest; so the threads
/// that can be busy at once never outnumber the cores (from eight cores up; below that each
/// kind still keeps a thread). The load and a test's sweep take every core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Threads {
    pub cores: usize,
    /// The main and render threads (Unreal's game and render threads), never shared.
    pub named: usize,
    pub frame: usize,
    pub async_compute: usize,
    pub building: usize,
    pub parts: usize,
    pub streaming: usize,
    pub io: usize,
    pub load: usize,
    /// Building's jobs behind the loading screen ([`Phase::Load`]): with its parts, all the
    /// cores but the main and render threads'.
    pub load_building: usize,
    pub load_parts: usize,
}

/// Which part of the game is running, for the shares of the cores that differ by it (D606).
/// Shipped engines size background work by it: behind a loading screen the load takes every
/// core it can, in play it is capped, as Unity's `Application.backgroundLoadingPriority`
/// (`High` behind a loading screen, `BelowNormal` or `Low` in play) and Unreal's
/// `FlushAsyncLoading` on a map's load against `s.AsyncLoadingTimeLimit` in play have it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The loading screen is up: building's work, the scene's buildings laid and drawn before
    /// the first thing is seen (D135), takes all but the main and render threads' cores.
    Load,
    /// The game plays: the background holds its quarter.
    Play,
}

/// The main and render threads.
const NAMED: usize = 2;

impl Threads {
    /// The table on a machine of `cores` cores.
    pub const fn on(cores: usize) -> Self {
        let cores = if cores == 0 { 1 } else { cores };
        // A quarter for the background, as perf-regress j1 found the far view's streaming
        // could take and leave lake-play its 6.5 ms main thread; half of it the streaming's,
        // the rest building's pool and the parts its jobs draw beside it.
        let background = max(1, cores / 4);
        let streaming = max(1, background / 2);
        let building = max(1, (background - streaming) / 2);
        let parts = background.saturating_sub(streaming + building);
        let async_compute = clamp(cores / 8, 1, 4);
        let io = clamp(cores / 4, 1, 4);
        let others = NAMED + async_compute + streaming + building + parts;
        // Behind the loading screen building and its parts share every core but the named
        // threads', half each, never fewer than in play.
        let loading = max(1, cores.saturating_sub(NAMED));
        let load_building = max(building, loading / 2);
        let load_parts = max(parts, loading.saturating_sub(load_building));
        Self {
            cores,
            named: NAMED,
            frame: max(1, cores.saturating_sub(others)),
            async_compute,
            building,
            parts,
            streaming,
            io,
            load: cores,
            load_building,
            load_parts,
        }
    }

    /// The threads `work` has in `phase`: building and its parts more behind the loading
    /// screen, every other kind the same in both.
    pub const fn during(&self, phase: Phase, work: Work) -> usize {
        match (phase, work) {
            (Phase::Load, Work::Building) => self.load_building,
            (Phase::Load, Work::Parts) => self.load_parts,
            _ => self.of(work),
        }
    }

    /// The most threads `work` has in either phase: a pool's size, its work rationed to the
    /// phase's share ([`share`]) by whoever sends it.
    pub const fn most(&self, work: Work) -> usize {
        max(
            self.during(Phase::Load, work),
            self.during(Phase::Play, work),
        )
    }

    /// The threads `work` has while the game plays.
    pub const fn of(&self, work: Work) -> usize {
        match work {
            Work::Frame => self.frame,
            Work::Async => self.async_compute,
            Work::Building => self.building,
            Work::Parts => self.parts,
            Work::Streaming => self.streaming,
            Work::Io => self.io,
            Work::Load => self.load,
        }
    }

    /// The background's threads: streaming, building and its parts.
    pub const fn background(&self) -> usize {
        self.streaming + self.building + self.parts
    }

    /// The threads that can be working at once while the game plays: every kind but IO's,
    /// which wait on the disk, and the load's, which has the machine to itself.
    pub const fn busy(&self) -> usize {
        self.named + self.frame + self.async_compute + self.background()
    }
}

const fn max(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

const fn clamp(x: usize, low: usize, high: usize) -> usize {
    max(low, if x < high { x } else { high })
}

/// The machine's logical cores, asked once: the only place the game asks (D606; the
/// one-definition check fails `available_parallelism` anywhere else).
pub fn cores() -> usize {
    static CORES: OnceLock<usize> = OnceLock::new();
    *CORES.get_or_init(|| std::thread::available_parallelism().map_or(4, |n| n.get()))
}

/// This machine's table.
pub fn threads() -> Threads {
    Threads::on(cores())
}

/// Whether the game is behind its loading screen ([`Phase::Load`]): it starts there, as every
/// build starts on the loading screen; `terra_world`'s loading screen sets it as it goes up
/// and lifts.
static LOADING: AtomicBool = AtomicBool::new(true);

/// The game is now in `phase`: work rationed by [`share`] takes the phase's share from its
/// next job on.
pub fn set_phase(phase: Phase) {
    LOADING.store(phase == Phase::Load, Ordering::Relaxed);
}

/// Which phase the game is in.
pub fn phase() -> Phase {
    if LOADING.load(Ordering::Relaxed) {
        Phase::Load
    } else {
        Phase::Play
    }
}

/// The threads `work` may keep busy now, in this machine's table for the [`phase`].
pub fn share(work: Work) -> usize {
    threads().during(phase(), work)
}

/// How many threads work shared out on this thread may use: one on a background pool's
/// thread ([`keep_to_this_thread`]), every core elsewhere (the load, a tool, a test's sweep).
pub fn width() -> usize {
    if KEPT_HERE.with(Cell::get) {
        1
    } else {
        threads().load
    }
}

thread_local! {
    /// Whether [`map`] on this thread works its items here alone ([`keep_to_this_thread`]).
    static KEPT_HERE: Cell<bool> = const { Cell::new(false) };
}

/// From now on, [`map`] on this thread works every item on this thread alone, never fanning
/// out over every core: for a background pool's threads (the far view streamed in while the
/// game plays, D603), whose work must leave the game's main and render threads their cores.
/// A pool of a few threads stays a few, as Unreal's background task threads do; on the lake
/// the woods' far cards, made on the streaming pool, spread over all 16 cores and held 286 of
/// lake-play's frames over twice the budget (perf-regress j1).
pub fn keep_to_this_thread() {
    KEPT_HERE.with(|kept| kept.set(true));
}

/// `f` over every item, on as many threads as this one may use ([`width`]), in order. The items are taken in runs, a thread taking
/// the next run not yet taken when it is done with its own, so a few slow items (a castle among
/// cabins) hold up only the thread working them; a run is a single item when there are few. A
/// panic in `f` stops the others taking more and is the caller's, with its own message.
pub fn map<T: Sync, U: Send>(items: &[T], f: impl Fn(&T) -> U + Sync) -> Vec<U> {
    let threads = width().min(items.len()).max(1);
    // Four runs a thread, so the last runs left over are short.
    let run = items.len().div_ceil(threads * 4).max(1);
    let next = AtomicUsize::new(0);
    let work = || {
        let mut done: Vec<(usize, Vec<U>)> = Vec::new();
        loop {
            let start = next.fetch_add(run, Ordering::Relaxed);
            let Some(taken) = items.get(start..items.len().min(start + run)) else {
                return Ok(done);
            };
            let worked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                taken.iter().map(&f).collect::<Vec<U>>()
            }));
            match worked {
                Ok(out) => done.push((start, out)),
                Err(panic) => {
                    next.store(items.len(), Ordering::Relaxed);
                    return Err(panic);
                }
            }
        }
    };
    let mut done: Vec<(usize, Vec<U>)> = Vec::new();
    let mut failed = None;
    std::thread::scope(|scope| {
        let workers: Vec<_> = (1..threads).map(|_| scope.spawn(work)).collect();
        let mut results = vec![work()];
        for worker in workers {
            // `work` catches its own panics, so a thread ends with what it did.
            results.push(worker.join().unwrap_or_else(Err));
        }
        for result in results {
            match result {
                Ok(runs) => done.extend(runs),
                Err(panic) => {
                    failed.get_or_insert(panic);
                }
            }
        }
    });
    if let Some(panic) = failed {
        std::panic::resume_unwind(panic);
    }
    done.sort_unstable_by_key(|(start, _)| *start);
    done.into_iter().flat_map(|(_, out)| out).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_is_worked_once_in_order() {
        for len in [0, 1, 3, 100, 10_007] {
            let items: Vec<usize> = (0..len).collect();
            assert_eq!(
                map(&items, |&i| i * 2),
                (0..len).map(|i| i * 2).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn a_thread_kept_to_itself_works_every_item_alone_and_others_still_fan_out() {
        let items: Vec<usize> = (0..64).collect();
        // How many threads worked the items, each long enough that idle threads take some.
        let threads = |items: &[usize]| {
            let ids = map(items, |_| {
                std::thread::sleep(std::time::Duration::from_millis(1));
                std::thread::current().id()
            });
            let mut ids: Vec<_> = ids.into_iter().map(|id| format!("{id:?}")).collect();
            ids.sort();
            ids.dedup();
            ids.len()
        };
        let kept = std::thread::spawn({
            let items = items.clone();
            move || {
                keep_to_this_thread();
                let here = format!("{:?}", std::thread::current().id());
                let ids = map(&items, |_| format!("{:?}", std::thread::current().id()));
                (
                    ids.iter().all(|id| *id == here),
                    threads(&items),
                    map(&items, |&i| i * 2),
                )
            }
        })
        .join()
        .expect("the kept thread ends");
        assert!(kept.0, "an item went to another thread");
        assert_eq!(kept.1, 1);
        assert_eq!(kept.2, items.iter().map(|i| i * 2).collect::<Vec<_>>());
        // Another thread is not kept: on a machine of several cores its map fans out.
        assert_eq!(threads(&items) > 1, cores() > 1);
    }

    #[test]
    fn the_threads_never_outnumber_the_cores_and_the_background_holds_a_quarter() {
        for cores in 8..=256 {
            let t = Threads::on(cores);
            assert!(
                t.busy() <= cores,
                "{cores} cores: {} threads can be busy at once ({t:?})",
                t.busy()
            );
            assert!(t.background() <= cores / 4, "{cores} cores: {t:?}");
            // The frame's pool has the most of any kind but the load.
            assert!(t.frame >= t.async_compute.max(t.background()), "{t:?}");
            assert_eq!(t.load, cores);
        }
        // Below eight cores each kind still has a thread of its own.
        for cores in 0..8 {
            let t = Threads::on(cores);
            for work in [
                Work::Frame,
                Work::Async,
                Work::Building,
                Work::Streaming,
                Work::Io,
            ] {
                assert!(t.of(work) >= 1, "{cores} cores: {work:?} has none");
            }
        }
        // The bench's sixteen: the frame's pool no smaller than Bevy's own default gives it
        // (all but its IO and async compute pools' quarter each).
        let t = Threads::on(16);
        assert!(t.frame >= 16 - 4 - 4, "{t:?}");
    }

    #[test]
    fn behind_the_loading_screen_building_takes_all_but_the_named_threads() {
        for cores in 0..=256 {
            let t = Threads::on(cores);
            let load = t.during(Phase::Load, Work::Building) + t.during(Phase::Load, Work::Parts);
            if cores >= 4 {
                assert_eq!(load, cores - NAMED, "{cores} cores: {t:?}");
            }
            for work in [Work::Building, Work::Parts] {
                // Never fewer than in play, and the pool sized for the more of the two.
                assert!(t.during(Phase::Load, work) >= t.of(work), "{t:?}");
                assert_eq!(t.most(work), t.during(Phase::Load, work), "{t:?}");
                assert_eq!(t.during(Phase::Play, work), t.of(work), "{t:?}");
            }
            // Every other kind the same in both.
            for work in [
                Work::Frame,
                Work::Async,
                Work::Streaming,
                Work::Io,
                Work::Load,
            ] {
                assert_eq!(t.during(Phase::Load, work), t.of(work), "{t:?}");
            }
        }
        // The game starts behind the loading screen; lifting it caps building to its quarter.
        assert_eq!(phase(), Phase::Load);
        assert_eq!(share(Work::Building), threads().load_building);
        set_phase(Phase::Play);
        assert_eq!(share(Work::Building), threads().building);
        assert_eq!(share(Work::Parts), threads().parts);
        set_phase(Phase::Load);
        assert_eq!(share(Work::Parts), threads().load_parts);
    }

    #[test]
    fn a_kept_thread_is_one_wide_and_another_has_every_core() {
        assert_eq!(width(), cores());
        let kept = std::thread::spawn(|| {
            keep_to_this_thread();
            width()
        });
        assert_eq!(kept.join().expect("the kept thread ends"), 1);
    }

    #[test]
    fn a_panic_in_one_item_is_the_callers_with_its_message() {
        let items: Vec<usize> = (0..64).collect();
        let caught = std::panic::catch_unwind(|| {
            map(&items, |&i| {
                assert!(i != 40, "item {i} is wrong");
                i
            })
        });
        let panic = caught.expect_err("the panic reaches the caller");
        let said = panic.downcast_ref::<String>().cloned().unwrap_or_default();
        assert_eq!(said, "item 40 is wrong");
    }
}
