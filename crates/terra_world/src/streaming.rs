//! What is streamed after the loading screen lifts (D603, D608): the ground's far levels, the
//! woods' far cards and what comes after them, worked out on a pool of threads of their own,
//! as open worlds stream their distant cells in round a player already playing (Unreal's
//! World Partition and HLODs, Skyrim's distant LOD). One scheduler starts the work in priority
//! classes, nearest the camera first, a few tasks at a time (`order::next`), and lets the
//! results into the world under a budget a frame of the main thread's time and the bytes to
//! upload (`order::Meter`, `assets/data/perf/budgets.ron`). The loading screen waits for what
//! is near; a scripted run is still held for the rest (`loading::hold`), so it sees the same
//! world on any machine.

pub(crate) mod order;

use crate::loading::Screen;
use crate::planet::MainCamera;
use bevy::{
    prelude::*,
    tasks::{Task, TaskPool},
};
use order::{Budget, Meter};
pub(crate) use order::{Class, Place};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Instant;
use terra_gen::par::Work;

/// The scheduler of what streams after the load, and the pool it works on: the streaming's
/// share of the background in the table of the game's threads (D606), each thread working its
/// task alone (`terra_gen::par::keep_to_this_thread`), so the game's own threads (the main,
/// render and Bevy's pools) keep their cores while it works, as Unreal's background workers
/// sit below the game and render threads. With all but two of the cores (14 of the bench's 16)
/// the lake's main thread took 15 to 22 ms a frame for the 30 s the far ground streamed, not
/// 6.5 ms; nice 19 did not help (load-time j5), as the threads shared the game's cores' caches
/// and hyperthreads, not only their time (perf-regress j1, D603).
#[derive(Resource)]
pub(crate) struct Streaming {
    pool: TaskPool,
    waiting: Vec<Waiting>,
    running: Vec<(Class, Task<()>)>,
    budget: Budget,
    meter: Meter,
    /// Whether the loading screen shows this frame, so the larger budget holds.
    loading: bool,
    /// Each class's tasks since it last drained: when the first was asked, how many, and the
    /// pool's nanoseconds they took, for the line said when it drains.
    classes: [(Option<Instant>, usize, Arc<AtomicU64>); Class::COUNT],
}

/// A task asked and not yet started.
struct Waiting {
    class: Class,
    place: Place,
    cancelled: Arc<AtomicBool>,
    /// In a lock only so the scheduler, a resource, is `Sync`; only the pool's thread runs it.
    work: Mutex<Box<dyn FnOnce() + Send>>,
}

/// What a task asked of the scheduler will give, once it has run; dropping it cancels the task
/// if it has not started.
pub(crate) struct Ticket<T> {
    slot: Arc<Mutex<Option<T>>>,
    cancelled: Arc<AtomicBool>,
}

impl<T> Ticket<T> {
    /// What the task gave, if it has run.
    pub(crate) fn take(&self) -> Option<T> {
        self.slot.lock().ok()?.take()
    }
}

impl<T> Drop for Ticket<T> {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

/// The budgets file's part the game reads; the rest is the bench's (D607).
#[derive(serde::Deserialize)]
struct Budgets {
    streaming: Budget,
}

impl Default for Streaming {
    fn default() -> Self {
        let budgets: Budgets = crate::data::read("perf/budgets.ron");
        Self {
            pool: crate::threads::pool(Work::Streaming, "Streaming far"),
            waiting: Vec::new(),
            running: Vec::new(),
            budget: budgets.streaming,
            meter: Meter::default(),
            loading: false,
            classes: Default::default(),
        }
    }
}

impl Streaming {
    /// How many tasks to keep on the pool at once: two a thread, so no thread waits a frame
    /// for its next, and what is asked later (the woods' far cards, a chunk the camera has come
    /// nearer) waits behind a few, not behind every one already asked (1473 far chunks at the
    /// lake held the cards 259 s, perf-regress j2).
    pub(crate) fn in_flight(&self) -> usize {
        self.pool.thread_num() * 2
    }

    /// Asks for `work` of `class` at `place`, started when its turn comes.
    pub(crate) fn ask<T: Send + 'static>(
        &mut self,
        class: Class,
        place: Place,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Ticket<T> {
        let slot = Arc::new(Mutex::new(None));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (state, nanos) = {
            let (asked, count, nanos) = &mut self.classes[class.index()];
            asked.get_or_insert_with(Instant::now);
            *count += 1;
            (slot.clone(), nanos.clone())
        };
        self.waiting.push(Waiting {
            class,
            place,
            cancelled: cancelled.clone(),
            work: Mutex::new(Box::new(move || {
                let began = Instant::now();
                let made = work();
                nanos.fetch_add(began.elapsed().as_nanos() as u64, Ordering::Relaxed);
                if let Ok(mut slot) = state.lock() {
                    *slot = Some(made);
                }
            })),
        });
        Ticket { slot, cancelled }
    }

    /// Whether one more result of `class` may enter the world this frame.
    pub(crate) fn may_enter(&self, class: Class) -> bool {
        let Budget {
            frame_ms,
            upload_mib,
            loading_frame_ms,
            loading_upload_mib,
        } = self.budget;
        let (ms, mib) = if self.loading {
            (loading_frame_ms, loading_upload_mib)
        } else {
            (frame_ms, upload_mib)
        };
        self.meter
            .may_enter(class, (ms, (mib * 1024.0 * 1024.0) as usize))
    }

    /// Counts a result of `class` in, put in the world since `began` and bringing `bytes`.
    pub(crate) fn entered(&mut self, class: Class, began: Instant, bytes: usize) {
        let ms = began.elapsed().as_secs_f32() * 1000.0;
        self.meter.entered(class, ms, bytes);
    }

    /// Starts what may run now, the most wanted first.
    fn feed(&mut self, viewer: Vec3) {
        self.running.retain(|(_, task)| !task.is_finished());
        self.waiting
            .retain(|w| !w.cancelled.load(Ordering::Relaxed));
        let room = self.in_flight().saturating_sub(self.running.len());
        if room == 0 || self.waiting.is_empty() {
            return;
        }
        let mut underway = [0; Class::COUNT];
        for (class, _) in &self.running {
            underway[class.index()] += 1;
        }
        let asked: Vec<(Class, Place)> = self.waiting.iter().map(|w| (w.class, w.place)).collect();
        let mut chosen = vec![false; asked.len()];
        let order = order::next(&asked, underway, viewer, room);
        for &i in &order {
            chosen[i] = true;
        }
        let mut starting: Vec<Option<Waiting>> = Vec::with_capacity(order.len());
        starting.resize_with(asked.len(), || None);
        let mut kept = Vec::with_capacity(asked.len() - order.len());
        for (i, waiting) in std::mem::take(&mut self.waiting).into_iter().enumerate() {
            if chosen[i] {
                starting[i] = Some(waiting);
            } else {
                kept.push(waiting);
            }
        }
        self.waiting = kept;
        for i in order {
            let Some(Waiting { class, work, .. }) = starting[i].take() else {
                continue;
            };
            let Ok(work) = work.into_inner() else {
                continue;
            };
            self.running
                .push((class, self.pool.spawn(async move { work() })));
        }
    }

    /// Says each class that has drained since it was last said: its tasks, the seconds since
    /// the first was asked and the pool's thread-seconds they took, so a far chunk's cost
    /// against a near one's shows (D608).
    fn say_drained(&mut self) {
        for class in Class::ALL {
            let busy = self.waiting.iter().any(|w| w.class == class)
                || self.running.iter().any(|(c, _)| *c == class);
            let (asked, count, nanos) = &mut self.classes[class.index()];
            let Some(since) = (*asked).filter(|_| !busy) else {
                continue;
            };
            let work = nanos.swap(0, Ordering::Relaxed) as f64 * 1e-9;
            info!(
                "Streamed {class:?}: {count} tasks in {:.1} s, {work:.1} s of the pool's threads",
                since.elapsed().as_secs_f32()
            );
            (*asked, *count) = (None, 0);
        }
    }
}

/// The streaming's systems, once whichever plugin needs it first adds them.
pub(crate) fn plugin(app: &mut App) {
    if app.world().contains_resource::<Streaming>() {
        return;
    }
    app.init_resource::<Streaming>()
        .add_systems(First, new_frame)
        .add_systems(Update, feed.run_if(lifted));
}

/// A new frame: nothing has entered it yet.
fn new_frame(screen: Option<Res<State<Screen>>>, mut streaming: ResMut<Streaming>) {
    streaming.meter = Meter::default();
    streaming.loading = !lifted(screen);
}

/// Starts what may run, nearest the camera first, and says each class drained.
fn feed(
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut streaming: ResMut<Streaming>,
) {
    let viewer = camera.map_or(Vec3::ZERO, |camera| camera.translation());
    streaming.feed(viewer);
    streaming.say_drained();
}

/// Whether the loading screen has lifted, so the far things may start: any screen but the
/// loading one, or none (a test's app).
pub(crate) fn lifted(screen: Option<Res<State<Screen>>>) -> bool {
    screen.is_none_or(|screen| *screen.get() != Screen::Loading)
}

/// What a mesh brings for the GPU: its vertices and indices.
pub(crate) fn mesh_bytes(mesh: &Mesh) -> usize {
    mesh.get_vertex_buffer_size() + mesh.get_index_buffer_bytes().map_or(0, <[u8]>::len)
}

/// Says how long the far view took to come in after the loading screen lifted: the ground's
/// far chunks and the woods' far cards, all of what `Terrain::coming` and `FarCards::coming`
/// count, drawn. The number a change to the streaming's order, pool or budget is judged by
/// (D608), which the benchmark says when it comes within the run (`Far view in`).
pub(crate) fn time_far_view(
    screen: Option<Res<State<Screen>>>,
    terrain: Option<Res<crate::terrain::Terrain>>,
    cards: Option<Res<crate::woods::FarCards>>,
    mut since: Local<Option<(Instant, bool)>>,
) {
    if !lifted(screen) {
        *since = None;
        return;
    }
    let (ground, woods) = (
        terrain.map_or(0, |terrain| terrain.coming()),
        cards.map_or(0, |cards| cards.coming()),
    );
    let (began, said) = since.get_or_insert_with(|| (Instant::now(), false));
    if *said || ground + woods > 0 {
        return;
    }
    *said = true;
    info!(
        "Far view in {:.1} s after the loading screen lifted: the ground's far chunks and the woods' far cards all drawn",
        began.elapsed().as_secs_f32()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_streaming_pool_has_its_share_of_the_background_and_never_fans_out() {
        let pool = Streaming::default();
        assert_eq!(pool.pool.thread_num(), terra_gen::par::threads().streaming);
        // A map asked on one of its threads stays on that thread (the woods' far cards ask
        // one), however many cores the machine has.
        let items: Vec<u32> = (0..64).collect();
        let (here, workers) = bevy::tasks::block_on(pool.pool.spawn(async move {
            let workers = terra_gen::par::map(&items, |_| {
                std::thread::sleep(std::time::Duration::from_millis(1));
                std::thread::current().id()
            });
            (std::thread::current().id(), workers)
        }));
        assert!(workers.iter().all(|id| *id == here), "the map fanned out");
    }

    #[test]
    fn the_scheduler_keeps_a_few_on_the_pool_and_every_ask_comes_back() {
        let mut streaming = Streaming::default();
        let place = |x: f32| Place::Bounds(Vec3::X * x, Vec3::X * x + Vec3::ONE);
        let tickets: Vec<Ticket<usize>> = (0..40)
            .map(|i| streaming.ask(Class::FarGround, place(i as f32), move || i))
            .collect();
        let dropped = streaming.ask(Class::FarGround, place(0.0), || 99);
        drop(dropped);
        let began = Instant::now();
        let mut back = vec![None; tickets.len()];
        while back.iter().any(Option::is_none) {
            streaming.feed(Vec3::ZERO);
            assert!(streaming.running.len() <= streaming.in_flight());
            for (i, ticket) in tickets.iter().enumerate() {
                if let Some(value) = ticket.take() {
                    back[i] = Some(value);
                }
            }
            assert!(began.elapsed().as_secs() < 30, "every task ran");
        }
        assert_eq!(back, (0..40).map(Some).collect::<Vec<_>>());
        // The cancelled ask never ran: nothing waits or runs once the rest are back.
        streaming.feed(Vec3::ZERO);
        streaming.running.retain(|(_, task)| !task.is_finished());
        assert!(streaming.waiting.is_empty());
    }
}
