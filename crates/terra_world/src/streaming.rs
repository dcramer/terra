//! What is streamed after the loading screen lifts (D603, D608): the ground's far levels, the
//! woods' far cards and what comes after them, worked out on a pool of threads of their own,
//! as open worlds stream their distant cells in round a player already playing (Unreal's
//! World Partition and HLODs, Skyrim's distant LOD). One scheduler starts the work in priority
//! classes, nearest the camera first, a few tasks at a time (`order::next`), and lets the
//! results into the world under a budget a frame of the main thread's time and the bytes to
//! upload (`order::Meter`, `assets/data/perf/budgets.ron`), each result charged what it
//! really costs the main thread once in: its spawn, applied and timed by `admit`, and what
//! other systems then do with it (`Aftermath`), measured (`order::Costs`). The loading screen waits for what
//! is near; a scripted run is still held for the rest (`loading::hold`), so it sees the same
//! world on any machine.

pub(crate) mod order;

use crate::loading::Screen;
use crate::planet::MainCamera;
use bevy::ecs::world::CommandQueue;
use bevy::{
    prelude::*,
    tasks::{Task, TaskPool},
};
pub use order::Class;
pub(crate) use order::Place;
use order::{Budget, Costs, Meter};
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
/// Other layers stream their own under its budget through `may_enter`, `commands` and
/// `entered` (the woods' floor, D622).
#[derive(Resource)]
pub struct Streaming {
    pool: TaskPool,
    waiting: Vec<Waiting>,
    running: Vec<(Class, Task<()>)>,
    budget: Budget,
    meter: Meter,
    /// What each class's results cost the main thread once in, measured.
    costs: Costs,
    /// Each class's results' commands this frame (`commands`), applied together and timed by
    /// `admit`.
    queues: [CommandQueue; Class::COUNT],
    /// How many of each class entered the frame before this one: what the other systems'
    /// work this frame (`Aftermath`) was over.
    before: [usize; Class::COUNT],
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
            costs: Costs::default(),
            queues: Default::default(),
            before: [0; Class::COUNT],
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

    /// This frame's budget: the main thread's ms and the bytes.
    fn frame_budget(&self) -> (f32, usize) {
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
        (ms, (mib * 1024.0 * 1024.0) as usize)
    }

    /// Whether one more result of `class` may enter the world this frame, at what one has
    /// been measured to cost.
    pub fn may_enter(&self, class: Class) -> bool {
        let budget = self.frame_budget();
        let cost = self.costs.of(class, budget.0);
        self.meter.may_enter(class, budget, cost)
    }

    /// Where a result of `class` is put in the world: commands applied by `admit` this frame,
    /// timed, so what spawning it costs is measured and charged to its class. Anything that
    /// enters the world under the budget is spawned through these.
    pub fn commands<'w>(&mut self, class: Class, commands: &Commands<'w, '_>) -> Commands<'w, '_> {
        commands.rebound_to(&mut self.queues[class.index()])
    }

    /// Counts a result of `class` in, put in the world since `began` and bringing `bytes`:
    /// the time since `began` and what one of its class costs once in.
    pub fn entered(&mut self, class: Class, began: Instant, bytes: usize) {
        let budget = self.frame_budget().0;
        let ms = began.elapsed().as_secs_f32() * 1000.0 + self.costs.of(class, budget);
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
        .init_resource::<Aftermath>()
        .add_systems(First, new_frame)
        .add_systems(Update, feed.run_if(lifted))
        .add_systems(
            PostUpdate,
            admit.before(bevy::transform::TransformSystems::Propagate),
        );
}

/// The main thread's time other systems spend on what the streaming let in (D608): a system
/// that does work for what has just entered the world (fitting a streamed tree's trunk to
/// the ground) files how long it took, and the streaming charges it to what entered, so the
/// budget holds what its results cost after they are in too, as well as their spawn.
#[derive(Resource, Default)]
pub struct Aftermath {
    ms: f32,
}

impl Aftermath {
    /// Files the time since `began` as work done for what streamed in.
    pub fn spent(&mut self, began: Instant) {
        self.ms += began.elapsed().as_secs_f32() * 1000.0;
    }
}

/// A new frame: nothing has entered it yet. What other systems did last frame was for what
/// entered the frame before (`admit` puts it in the world at the end of its frame), and is
/// charged to it.
fn new_frame(
    screen: Option<Res<State<Screen>>>,
    mut streaming: ResMut<Streaming>,
    mut aftermath: ResMut<Aftermath>,
) {
    let streaming = &mut *streaming;
    let ms = std::mem::take(&mut aftermath.ms);
    streaming.costs.after(ms, streaming.before);
    streaming.before = std::mem::take(&mut streaming.meter).entered;
    streaming.loading = !lifted(screen);
}

/// Puts this frame's results in the world, a class at a time, timing each: the spawn and
/// insertion of everything they brought, with the hooks and observers it sets off, which
/// Bevy defers past the systems that asked for it, so no system's own clock sees it.
fn admit(world: &mut World) {
    let mut queues: [CommandQueue; Class::COUNT] =
        std::mem::take(&mut world.resource_mut::<Streaming>().queues);
    for class in Class::ALL {
        let queue = &mut queues[class.index()];
        if queue.is_empty() {
            continue;
        }
        let began = Instant::now();
        queue.apply(world);
        let ms = began.elapsed().as_secs_f32() * 1000.0;
        let mut streaming = world.resource_mut::<Streaming>();
        let count = streaming.meter.entered[class.index()];
        streaming.costs.applied(class, ms, count);
    }
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
    use bevy::ecs::lifecycle::HookContext;
    use bevy::ecs::world::DeferredWorld;
    use std::time::Duration;

    /// Something whose spawn takes 2 ms of the main thread, in its hook.
    #[derive(Component)]
    #[component(on_add = slow_to_add)]
    struct SlowToAdd;

    fn slow_to_add(_: DeferredWorld, _: HookContext) {
        std::thread::sleep(Duration::from_millis(2));
    }

    /// Something another system takes 1 ms over once it is in.
    #[derive(Component)]
    struct SlowAfter;

    /// An app streaming in up to 20 of `class` a frame, each spawned as `bundle` gives.
    fn streaming_app<B: Bundle>(class: Class, bundle: fn() -> B) -> App {
        let mut app = App::new();
        plugin(&mut app);
        app.add_systems(
            Update,
            move |commands: Commands, mut streaming: ResMut<Streaming>| {
                for _ in 0..20 {
                    if !streaming.may_enter(class) {
                        break;
                    }
                    let began = Instant::now();
                    streaming.commands(class, &commands).spawn(bundle());
                    streaming.entered(class, began, 0);
                }
            },
        );
        app
    }

    /// The entered of `class` the frame before last, as the meter counted them.
    fn entered_before(app: &App, class: Class) -> usize {
        app.world().resource::<Streaming>().before[class.index()]
    }

    /// A result's spawn, which Bevy defers past the system that asked for it, is what it costs
    /// the frame: something whose spawn takes 2 ms comes in one a frame under the 2 ms budget,
    /// however little its queueing takes, and is in the world by the frame's end.
    #[test]
    fn a_results_spawn_is_charged_to_its_class() {
        let mut app = streaming_app(Class::Other, || SlowToAdd);
        for _ in 0..4 {
            app.update();
        }
        let streaming = app.world().resource::<Streaming>();
        assert!(streaming.costs.of(Class::Other, 0.0) >= 2.0);
        assert_eq!(entered_before(&app, Class::Other), 1);
        let world = app.world_mut();
        assert_eq!(world.query::<&SlowToAdd>().iter(world).count(), 4);
        // Something cheap to spawn comes in many a frame.
        let mut app = streaming_app(Class::Other, || Name::new("cheap"));
        for _ in 0..12 {
            app.update();
        }
        assert!(entered_before(&app, Class::Other) > 5);
    }

    /// What another system does with a result once it is in, filed as the streaming's
    /// aftermath, is charged to its class too: 1 ms a result after holds them to about two a
    /// frame under the 2 ms budget, where their spawn alone would let twenty in.
    #[test]
    fn what_comes_after_a_result_is_charged_to_its_class() {
        let mut app = streaming_app(Class::Flora, || SlowAfter);
        app.add_systems(
            Update,
            |added: Query<(), Added<SlowAfter>>, mut aftermath: ResMut<Aftermath>| {
                let began = Instant::now();
                for () in &added {
                    std::thread::sleep(Duration::from_millis(1));
                }
                if !added.is_empty() {
                    aftermath.spent(began);
                }
            },
        );
        for _ in 0..30 {
            app.update();
        }
        let cost = app
            .world()
            .resource::<Streaming>()
            .costs
            .of(Class::Flora, 0.0);
        assert!(cost >= 0.9, "{cost} ms");
        assert!(entered_before(&app, Class::Flora) <= 3);
    }

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
