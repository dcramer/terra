//! A crowd's cost (docs/controller.md criterion 20, `cost.crowd`; D52, D138): a thousand men
//! on the generated planet's ground at 30 km, on a grid 1.5 m apart, each walking or sprinting
//! across to its place mirrored through the crowd's middle and back, through the others, as
//! the course's crowd does; timed a tick on [`THREADS`] threads, the median of several runs.
//! The bodies' sweeps, contacts and pushes find each other through the mover's broadphase
//! (`Grid`); their steps are shared over the threads (`Bodies::tick_on`), each thread its own
//! memo of the planet's heights, on a pool of the bench's own: rayon's (the server's) and
//! Bevy's (the game's), each of [`THREADS`] threads with the caller's, timed apart: the tick's
//! wall time, its lanes' time busy (its CPU time), what runs on one thread (serial) and how late
//! the lanes start after they are handed out (the pool's threads woken).

use bevy::math::DVec3;
use bevy::prelude::{Vec2, Vec3};
use std::cell::RefCell;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use terra_motion::mover::{Bodies, Executor, InTurn, Intent, Kind, Sdf, World};
use terra_world::planet::{HeightMemo, Planet};

const TICK: f32 = 1.0 / 64.0;
/// The crowd: so many columns and rows, so far apart, m.
const COLUMNS: u32 = 40;
const ROWS: u32 = 25;
const APART_M: f32 = 1.5;
/// How long each run walks, s, and how many runs the median is of.
const SECONDS: f32 = 2.0;
pub const RUNS: usize = 7;
/// The planet's lattice points kept a body (as the bench's single body, `MEMO_MOST`).
const MEMO_MOST: usize = 1024;
/// The cores the server gives the bodies' tick (docs/controller.md criterion 20: 4 ms of the
/// 15.6 ms tick "on the server's cores"): an assumption until the server's machine is chosen,
/// half a 16-thread machine's eight cores, the other half the tick's other systems'.
pub const THREADS: usize = 8;

/// A thread's own: its memo of the planet's heights, and how many times it asked the ground;
/// a cache line pair of its own, so lanes counting their calls never share one (false sharing).
#[derive(Default)]
#[repr(align(128))]
struct Lane {
    memo: HeightMemo,
    calls: u64,
}

pub struct Cost {
    /// A tick of the whole crowd, ms.
    pub millis: f32,
    /// A tick's lanes' time busy (its CPU time), what of it ran on one thread outside them, and
    /// how late after they were handed out its lanes started, on average and the last, ms.
    pub busy: f32,
    pub serial: f32,
    pub late: f32,
    pub latest: f32,
    /// The ground's field calls a body a tick.
    pub calls: f32,
    /// The crowd's mean distance from where it started, m: a crowd stuck proves nothing.
    pub walked: f32,
    pub bodies: usize,
}

/// What a tick's lanes spent, summed over the ticks, s.
#[derive(Default)]
struct Spent {
    /// The executor's wall time, its lanes' time busy, how late they started (all but the
    /// first to start, on average) and the last to start.
    wall: f64,
    busy: f64,
    late: f64,
    latest: f64,
}

/// An executor with its lanes timed.
struct Timed<'a> {
    on: &'a dyn Executor,
    spent: Mutex<Spent>,
}

impl Executor for Timed<'_> {
    fn each(&self, lanes: usize, lane: &(dyn Fn(usize) + Sync)) {
        let began = Instant::now();
        let ran = Mutex::new(Vec::with_capacity(lanes));
        self.on.each(lanes, &|k| {
            let start = began.elapsed();
            lane(k);
            let end = began.elapsed();
            ran.lock().expect("a lane's time").push((start, end));
        });
        let wall = began.elapsed();
        let mut ran = ran.into_inner().expect("the lanes' times");
        ran.sort();
        let mut spent = self.spent.lock().expect("the tick's time");
        spent.wall += wall.as_secs_f64();
        spent.busy += ran
            .iter()
            .map(|(s, e)| (*e - *s).as_secs_f64())
            .sum::<f64>();
        let woken = &ran[1.min(ran.len() - 1)..];
        spent.late +=
            woken.iter().map(|(s, _)| s.as_secs_f64()).sum::<f64>() / woken.len().max(1) as f64;
        spent.latest += ran.last().map_or(0.0, |(s, _)| s.as_secs_f64());
    }
}

/// rayon's pool, the first lane on the calling thread.
pub struct OnRayon<'a>(pub &'a rayon::ThreadPool);

impl Executor for OnRayon<'_> {
    fn each(&self, lanes: usize, lane: &(dyn Fn(usize) + Sync)) {
        self.0.in_place_scope(|scope| {
            for k in 1..lanes {
                scope.spawn(move |_| lane(k));
            }
            lane(0);
        });
    }
}

/// The machine's load over the last minute, if it says.
pub fn load() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|l| l.split_whitespace().next().map(str::to_owned))
        .unwrap_or_else(|| "?".to_owned())
}

/// One run of the crowd on so many threads of an executor (one thread: in turn), from `start`
/// with `forward` along the ground.
pub fn run(
    planet: &Planet,
    start: Vec3,
    forward: Vec3,
    kind: &Kind,
    on: &dyn Executor,
    threads: usize,
) -> Cost {
    let up = start.normalize();
    let side = up.cross(forward).normalize();
    let steepest = planet.steepest();
    let water = |at: Vec3| crate::mover_world::water(planet, at);
    let world = |lane: &mut Lane, then: &mut dyn FnMut(&World)| {
        let lane = RefCell::new(lane);
        let sdf = Sdf {
            distance: |at: DVec3| {
                let mut lane = lane.borrow_mut();
                lane.calls += 1;
                planet.distance_in(at, &mut lane.memo)
            },
            steepest,
        };
        then(&World {
            ground: &sdf,
            water: &water,
            gravity: 9.81,
            centre: Vec3::ZERO,
        });
    };
    let mut lanes: Vec<Lane> = (0..threads).map(|_| Lane::default()).collect();
    let place = |id: u32| {
        let (col, row) = ((id % COLUMNS) as f32, (id / COLUMNS) as f32);
        Vec2::new(
            (col - (COLUMNS - 1) as f32 * 0.5) * APART_M,
            (row - (ROWS - 1) as f32 * 0.5) * APART_M,
        )
    };
    let at = |p: Vec2| start + forward * p.x + side * p.y;
    let mut bodies = Bodies::default();
    let count = COLUMNS * ROWS;
    for id in 0..count {
        let above = at(place(id)).normalize() * (start.length() + 50.0);
        let feet = planet.ground_at(above);
        world(&mut lanes[0], &mut |w| {
            bodies.add(id, *kind, feet, forward, w)
        });
    }
    let began: Vec<Vec3> = bodies.iter().map(|b| b.feet).collect();
    let mut back = vec![false; count as usize];
    let mut intents = Vec::with_capacity(count as usize);
    let ticks = (SECONDS / TICK) as usize;
    lanes.iter_mut().for_each(|lane| lane.calls = 0);
    let timed = Timed {
        on: if threads > 1 { on } else { &InTurn },
        spent: Mutex::new(Spent::default()),
    };
    let mut ticking = Duration::ZERO;
    for _ in 0..ticks {
        intents.clear();
        for body in bodies.iter() {
            let id = body.id;
            let home = place(id);
            let goal = at(if back[id as usize] { home } else { -home });
            let to = (goal - body.feet).reject_from(body.up);
            if to.length() < 0.5 {
                back[id as usize] = !back[id as usize];
            }
            intents.push((
                id,
                Intent {
                    wish: to.normalize_or_zero(),
                    sprint: id % 3 == 0,
                    ..Intent::default()
                },
            ));
        }
        for lane in &mut lanes {
            lane.memo
                .keep_within(MEMO_MOST * (count as usize).div_ceil(threads));
        }
        let clock = Instant::now();
        bodies.tick_on(&timed, &mut lanes, &world, &intents, TICK);
        ticking += clock.elapsed();
    }
    let a_tick = |s: f64| (s * 1.0e3 / ticks as f64) as f32;
    let millis = a_tick(ticking.as_secs_f64());
    let spent = timed.spent.into_inner().expect("the ticks' time");
    let walked = bodies
        .iter()
        .zip(&began)
        .map(|(b, from)| b.feet.distance(*from))
        .sum::<f32>()
        / count as f32;
    Cost {
        millis,
        busy: a_tick(spent.busy),
        serial: millis - a_tick(spent.wall),
        late: a_tick(spent.late),
        latest: a_tick(spent.latest),
        calls: lanes.iter().map(|lane| lane.calls).sum::<u64>() as f32
            / (ticks as f32 * count as f32),
        walked,
        bodies: count as usize,
    }
}

/// The median run, and how far apart the runs' times lie, a share of the median's.
pub fn median(mut runs: Vec<Cost>) -> (Cost, f32) {
    runs.sort_by(|a, b| a.millis.total_cmp(&b.millis));
    let spread = (runs[runs.len() - 1].millis - runs[0].millis) / runs[runs.len() / 2].millis;
    (runs.swap_remove(runs.len() / 2), spread)
}
