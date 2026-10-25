//! The mover's cost (docs/controller.md criterion 20, `cost.body`; D52, D138): one body walked
//! forward at jog on one core, over the course (a flight of steps, then a slope) and over the
//! generated planet's own ground (`Planet::new`) at 2 km and at 30 km radius, timed a tick and
//! its ground's field calls counted, for the mover (D53), as the walk drives it. The median of
//! several runs, so a busy machine's outliers do not move it. `cargo xtask controller --bench`
//! runs it in release and judges `cost.body`, the mover's worst scene, against the bar; the
//! rest are its log's `bench` lines. `cost.forest` is the same body walked through the lush
//! yard's trees and rocks ([`forest`]); `cost.crowd` a thousand men walking through each other
//! on the planet at 30 km ([`crowd`]).

use bevy::math::DVec3;
use bevy::prelude::{Vec2, Vec3};
use std::cell::{Cell, RefCell};
use std::time::Instant;
use terra_core::{PlanetSpec, Seed};
use terra_motion::course::field::{Course, Field, Solid, forward_at, world};
use terra_motion::mover::{Bodies, Intent, Kind, Sdf, World};
use terra_world::planet::{HeightMemo, Planet};

#[path = "movement_bench_crowd.rs"]
mod crowd;
#[path = "movement_bench_forest.rs"]
mod forest;

const TICK: f32 = 1.0 / 64.0;
/// How long each run walks, s, and how many runs a scene's median is of.
const SECONDS: f32 = 4.0;
const RUNS: usize = 9;

/// How many of the land's lattice points a body's memo keeps before it forgets them: a body
/// walking at jog asks about a few new ones a second.
const MEMO_MOST: usize = 1024;

/// The ground a scene walks: its distance, the planet it is when it is one (the mover asks it
/// through a memo kept from tick to tick, as the game's walk does), how steep it may be, its
/// water and gravity, and where the walk starts and goes.
struct Ground<'a> {
    /// The distance at a point in f64, as the mover asks it (D53's f64 frames).
    precise: &'a dyn Fn(DVec3) -> f32,
    planet: Option<&'a Planet>,
    steepest: f32,
    water: &'a dyn Fn(Vec3) -> Option<f32>,
    gravity: f32,
    start: Vec3,
    forward: Vec3,
}

/// A run's time a tick, µs, and the ground's field calls a tick.
struct Cost {
    micros: f32,
    calls: f32,
    /// How far the body got, m: a body stuck costs less and proves nothing.
    walked: f32,
}

/// The median of each over the runs.
fn median(mut runs: Vec<Cost>) -> Cost {
    let mid = runs.len() / 2;
    runs.sort_by(|a, b| a.micros.total_cmp(&b.micros));
    let micros = runs[mid].micros;
    runs.sort_by(|a, b| a.calls.total_cmp(&b.calls));
    Cost {
        micros,
        calls: runs[mid].calls,
        walked: runs[mid].walked,
    }
}

/// One run of the mover over a ground.
fn mover(ground: &Ground, kind: &Kind) -> Cost {
    let calls = Cell::new(0u64);
    let memo = RefCell::new(HeightMemo::default());
    let sdf = Sdf {
        distance: |at: DVec3| {
            calls.set(calls.get() + 1);
            match ground.planet {
                Some(planet) => planet.distance_in(at, &mut memo.borrow_mut()),
                None => (ground.precise)(at),
            }
        },
        steepest: ground.steepest,
    };
    let world = World {
        ground: &sdf,
        water: ground.water,
        gravity: ground.gravity,
        centre: Vec3::ZERO,
    };
    let mut bodies = Bodies::default();
    bodies.add(0, *kind, ground.start, ground.forward, &world);
    let intent = [(
        0,
        Intent {
            wish: ground.forward,
            ..Intent::default()
        },
    )];
    let ticks = (SECONDS / TICK) as usize;
    calls.set(0);
    let began = Instant::now();
    for _ in 0..ticks {
        memo.borrow_mut().keep_within(MEMO_MOST);
        bodies.tick(&world, &intent, TICK);
    }
    Cost {
        micros: began.elapsed().as_secs_f32() * 1.0e6 / ticks as f32,
        calls: calls.get() as f32 / ticks as f32,
        walked: bodies.get(0).map_or(0.0, |b| b.feet.distance(ground.start)),
    }
}

/// A planet's start: dry ground near the top, walked toward +x.
fn on(planet: &Planet) -> (Vec3, Vec3) {
    let r = planet.radius_m();
    let start = (0..64)
        .map(|i| {
            let a = i as f32 * 0.37;
            Vec3::new(libm::sinf(a) * 0.3, 1.0, libm::cosf(a) * 0.3).normalize() * (r + 500.0)
        })
        .map(|above| planet.ground_at(above))
        .find(|at| !planet.under_sea(*at + at.normalize() * 0.5))
        .expect("dry ground near the top");
    (start, Vec3::X.reject_from(start.normalize()).normalize())
}

/// µs a tick and field calls a tick for the mover, a scene each, and
/// `cost.body`, the mover's worst scene's.
#[test]
#[ignore = "a release bench: cargo xtask controller --bench"]
fn the_mover_bench_times_a_body_walking() {
    let kind = Kind::man();
    // The course: level ground, six steps of 0.25 m on 0.5 m treads, level, then 20 deg up.
    let mut corners = vec![(-5.0, 0.0), (2.0, 0.0)];
    for i in 0..6 {
        let (u, h) = (2.0 + i as f32 * 0.5, (i + 1) as f32 * 0.25);
        corners.extend([(u, h), (u + 0.5, h)]);
    }
    corners.extend([
        (9.0, 1.5),
        (40.0, 1.5 + 31.0 * libm::tanf(20f32.to_radians())),
    ]);
    let course = Course::of(Solid::Profile(
        corners.into_iter().map(|(u, h)| Vec2::new(u, h)).collect(),
    ));
    let course_precise = |at: DVec3| course.distance_at(at);
    let none = |_: Vec3| None;
    let start = world(0.0, 0.0, 0.0);
    let mut scenes = vec![(
        "course".to_string(),
        Ground {
            precise: &course_precise,
            planet: None,
            steepest: 1.0,
            water: &none,
            gravity: course.gravity(),
            start,
            forward: forward_at(start),
        },
    )];
    let planets: Vec<(f32, Planet)> = [2_000.0, 30_000.0]
        .into_iter()
        .map(|radius_m| {
            let spec = PlanetSpec {
                seed: Seed(1),
                radius_m,
            };
            (radius_m, Planet::new(spec))
        })
        .collect();
    let waters: Vec<_> = planets
        .iter()
        .map(|(_, p)| move |at: Vec3| crate::mover_world::water(p, at))
        .collect();
    let precise: Vec<_> = planets
        .iter()
        .map(|(_, p)| move |at: DVec3| p.distance_in(at, &mut HeightMemo::default()))
        .collect();
    for (i, (radius_m, planet)) in planets.iter().enumerate() {
        let (start, forward) = on(planet);
        scenes.push((
            format!("planet {} km", radius_m / 1000.0),
            Ground {
                precise: &precise[i],
                planet: Some(planet),
                steepest: planet.steepest(),
                water: &waters[i],
                gravity: course.gravity(),
                start,
                forward,
            },
        ));
    }
    let mut worst = 0.0_f32;
    for (name, ground) in &scenes {
        let ours = median((0..RUNS).map(|_| mover(ground, &kind)).collect());
        println!(
            "bench\t{name}\tmover {:.1} µs {:.0} calls a tick, {:.1} m",
            ours.micros, ours.calls, ours.walked
        );
        worst = worst.max(ours.micros);
    }
    println!("scorecard\tcost.body\t{worst}");
    let woods = forest::Forest::load();
    let f = forest::median(
        (0..forest::RUNS)
            .map(|_| forest::run(&woods, &kind))
            .collect(),
    );
    println!(
        "bench\tforest ({} solids, {} pieces)\tmover {:.1} µs a tick (worst {:.0}), calls a tick: planet {:.0}, solids {:.0} ({:.1} gathered), pieces offered {:.0}; {:.0} of {:.0} m",
        woods.solids,
        woods.pieces,
        f.micros,
        f.worst,
        f.planet,
        f.solids,
        f.gathered,
        f.pieces,
        f.walked,
        f.route
    );
    println!("scorecard\tcost.forest\t{}", f.micros);
    let (_, far) = &planets[1];
    let (start, forward) = on(far);
    // The bench's own pools, each of THREADS threads with the calling one: rayon's (the
    // server's) and Bevy's (the game's compute pool).
    let rayon = rayon::ThreadPoolBuilder::new()
        .num_threads(crowd::THREADS - 1)
        .build()
        .expect("a pool");
    let tasks = bevy::tasks::TaskPoolBuilder::new()
        .num_threads(crowd::THREADS - 1)
        .build();
    let on_rayon = crowd::OnRayon(&rayon);
    let on_tasks = crate::mover_world::OnTasks(&tasks);
    let crowd_on = |on: &dyn terra_motion::mover::Executor,
                    name: &str,
                    threads: usize,
                    runs: usize| {
        let load = crowd::load();
        let (c, spread) = crowd::median(
            (0..runs)
                .map(|_| crowd::run(far, start, forward, &kind, on, threads))
                .collect(),
        );
        println!(
            "bench\tcrowd ({} bodies, planet 30 km) {name}\t{:.2} ms a tick on {threads} threads (median of {runs}, spread {:.1}%), lanes busy {:.2} ms (CPU), serial {:.2} ms, lanes start {:.3} ms late ({:.3} the last); {:.0} calls a body a tick; walked {:.1} m on average; load {load} -> {}",
            c.bodies,
            c.millis,
            spread * 100.0,
            c.busy,
            c.serial,
            c.late,
            c.latest,
            c.calls,
            c.walked,
            crowd::load(),
        );
        c
    };
    let c = crowd_on(&on_rayon, "rayon", crowd::THREADS, crowd::RUNS);
    crowd_on(&on_tasks, "Bevy's pool", crowd::THREADS, crowd::RUNS);
    crowd_on(&on_rayon, "one thread", 1, 3);
    println!("scorecard\tcost.crowd\t{}", c.millis);
}
