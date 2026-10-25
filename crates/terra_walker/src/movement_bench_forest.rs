//! The mover's cost in the forest (`cost.forest`, docs/controller.md criterion 20): one body
//! walked at jog along the GPU `walk` scene's route (xtask/src/scenes.rs) over the lush yard's
//! own ground (`Planet::rolling`, seed 1, 2 km, as `--experiment lush --level yard` makes it)
//! among the trees, rocks, stumps and logs flora fitted there, gathered near it each tick as
//! the game's walk gathers them (`mover_world::flora` out of a `flora::Obstacles`). The
//! solids are a fixture (`movement_bench_forest.json`), dumped from the running game by the
//! `world.solids` remote method, since fitting them needs their models' meshes. Timed a tick,
//! with the calls a tick by source: the planet's field, the solids asked, the pieces offered.

use bevy::math::DVec3;
use bevy::prelude::{Vec2, Vec3};
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::time::Instant;
use terra_core::{Piece, PlanetSpec, Seed, Solid};
use terra_motion::mover::{Bodies, Intent, Kind, Obstacles, Part, Sdf, Shaped, World};
use terra_world::flora;
use terra_world::planet::{HeightMemo, Planet};
use terra_world::start::Start;

const TICK: f32 = 1.0 / 64.0;
/// The most a run walks, s: the route is about 290 m.
const SECONDS: f32 = 90.0;
/// How many runs the median is of.
pub const RUNS: usize = 3;
/// How far round a body the walk gathers what may stop it, m, before its speed's tick
/// (`movement::REACH_M`).
const REACH_M: f32 = 3.0;
/// A body's memo of the land's lattice (`movement::MEMO_MOST`).
const MEMO_MOST: usize = 1024;
/// A spot of the route is reached this near it, m.
const REACHED_M: f32 = 2.0;

#[derive(Deserialize)]
struct Fixture {
    start: [f32; 3],
    route: Vec<[f32; 2]>,
    solids: Vec<Fitted>,
}

#[derive(Deserialize)]
struct Fitted {
    at: [f32; 3],
    frame: [[f32; 3]; 3],
    pieces: Vec<Written>,
}

#[derive(Deserialize)]
struct Written {
    hull: Vec<[f32; 2]>,
    radius: f32,
    top: f32,
    crest: f32,
}

/// The forest: its planet, start, route (east and south of the start) and flora's index of it.
pub struct Forest {
    planet: Planet,
    start: Start,
    route: Vec<Vec2>,
    flora: flora::Obstacles,
    pub solids: usize,
    pub pieces: usize,
}

impl Forest {
    pub fn load() -> Self {
        let text = include_str!("movement_bench_forest.json");
        let fixture: Fixture = serde_json::from_str(text).expect("the forest fixture");
        let planet = Planet::rolling(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        let start = Start::choose(&planet, true);
        let written = Vec3::from_array(fixture.start);
        assert!(
            start.translation.distance(written) < 0.01,
            "the yard's start moved ({} against the fixture's {written}): dump the fixture again",
            start.translation
        );
        let v = Vec3::from_array;
        let mut flora = flora::Obstacles::default();
        let (mut solids, mut pieces) = (0, 0);
        for fitted in fixture.solids {
            let parts: Vec<Piece> = fitted
                .pieces
                .into_iter()
                .map(|part| Piece {
                    hull: part.hull.into_iter().map(Vec2::from_array).collect(),
                    radius: part.radius,
                    top: part.top,
                    crest: part.crest,
                })
                .collect();
            solids += 1;
            pieces += parts.len();
            let frame = fitted.frame.map(v);
            flora.add_solid(v(fitted.at), Solid::framed(v(fitted.at), frame, parts));
        }
        Forest {
            planet,
            start,
            route: fixture.route.into_iter().map(Vec2::from_array).collect(),
            flora,
            solids,
            pieces,
        }
    }
}

/// What a run asked a tick, on the mean.
#[derive(Default)]
pub struct Cost {
    pub micros: f32,
    /// The slowest tick's, µs.
    pub worst: f32,
    pub planet: f32,
    pub solids: f32,
    pub pieces: f32,
    /// How many solids were gathered near the body a tick.
    pub gathered: f32,
    /// How far along the route the body got, m, and of how far.
    pub walked: f32,
    pub route: f32,
}

/// The calls a run made, by source.
#[derive(Default)]
struct Tally {
    planet: Cell<u64>,
    solids: Cell<u64>,
    pieces: Cell<u64>,
}

/// A solid as the mover asks it, its calls tallied.
struct Tallied<'a> {
    solid: &'a Solid,
    tally: &'a Tally,
}

impl Shaped for Tallied<'_> {
    fn distance(&self, at: Vec3) -> f32 {
        let tally = self.tally;
        tally.solids.set(tally.solids.get() + 1);
        tally
            .pieces
            .set(tally.pieces.get() + self.solid.pieces.len() as u64);
        Shaped::distance(self.solid, at)
    }

    fn distance_at(&self, at: DVec3) -> f32 {
        self.distance(at.as_vec3())
    }

    fn bounds(&self) -> (Vec3, f32) {
        self.solid.bounds()
    }

    fn parts_near(&self, at: Vec3, reach: f32, out: &mut Vec<Part>) {
        self.solid.parts_near(at, reach, out);
    }

    /// The query's union asks a solid here: tallied as one solid asked and the pieces it was
    /// offered, the most it may answer exactly (those its bounds do not skip).
    fn nearest_of(&self, parts: &[Part], at: DVec3, best: f32) -> f32 {
        let tally = self.tally;
        tally.solids.set(tally.solids.get() + 1);
        tally.pieces.set(tally.pieces.get() + parts.len() as u64);
        self.solid.nearest_of(parts, at, best)
    }
}

/// One walk along the route among the forest's solids.
pub fn run(forest: &Forest, kind: &Kind) -> Cost {
    let planet = &forest.planet;
    let tally = Tally::default();
    let memo = RefCell::new(HeightMemo::default());
    let field = Sdf {
        distance: |at: DVec3| {
            tally.planet.set(tally.planet.get() + 1);
            planet.distance_in(at, &mut memo.borrow_mut())
        },
        steepest: planet.steepest(),
    };
    let water = |at: Vec3| crate::mover_world::water(planet, at);
    let spot = |i: usize| {
        let p = forest.route[i];
        planet.ground_at(forest.start.at(p.x, p.y))
    };
    let feet = spot(0);
    let up = feet.normalize();
    let forward = (spot(1) - feet).reject_from(up).normalize();
    let mut bodies = Bodies::default();
    let quiet = World {
        ground: &field,
        water: &water,
        gravity: crate::movement::GRAVITY,
        centre: Vec3::ZERO,
    };
    bodies.add(0, *kind, feet, forward, &quiet);
    let length: f32 = forest
        .route
        .windows(2)
        .map(|leg| leg[0].distance(leg[1]))
        .sum();
    let (mut next, mut done) = (1, 0.0_f32);
    let (mut ticks, mut micros, mut worst, mut gathered) = (0u64, 0.0_f64, 0.0_f32, 0u64);
    for _ in 0..(SECONDS / TICK) as usize {
        let body = bodies.get(0).expect("the body");
        let (feet, speed) = (body.feet, body.velocity.length());
        while next < forest.route.len() && feet.distance(spot(next)) < REACHED_M {
            done += forest.route[next - 1].distance(forest.route[next]);
            next += 1;
        }
        if next >= forest.route.len() {
            break;
        }
        let up = feet.normalize();
        let wish = (spot(next) - feet).reject_from(up).normalize_or_zero();
        let intent = [(
            0,
            Intent {
                wish,
                ..Intent::default()
            },
        )];
        let began = Instant::now();
        // As the walk does a tick: gather, then move.
        let near = crate::mover_world::flora(&forest.flora, feet, REACH_M + speed * TICK);
        let tallied: Vec<Tallied> = near
            .iter()
            .map(|solid| Tallied {
                solid,
                tally: &tally,
            })
            .collect();
        let trees = Obstacles {
            obstacles: &tallied,
        };
        let ground = (&field, &trees);
        let world = World {
            ground: &ground,
            water: &water,
            gravity: crate::movement::GRAVITY,
            centre: Vec3::ZERO,
        };
        memo.borrow_mut().keep_within(MEMO_MOST);
        bodies.tick(&world, &intent, TICK);
        let took = began.elapsed().as_secs_f64() * 1.0e6;
        micros += took;
        worst = worst.max(took as f32);
        gathered += near.len() as u64;
        ticks += 1;
    }
    let along = bodies.get(0).map_or(0.0, |body| {
        let (last, here) = (spot(next - 1), body.feet);
        done + last.distance(here).min(length - done)
    });
    let per = |n: u64| n as f32 / ticks.max(1) as f32;
    Cost {
        micros: (micros / ticks.max(1) as f64) as f32,
        worst,
        planet: per(tally.planet.get()),
        solids: per(tally.solids.get()),
        pieces: per(tally.pieces.get()),
        gathered: per(gathered),
        walked: along,
        route: length,
    }
}

/// The median run by time, the others' numbers beside it.
pub fn median(mut runs: Vec<Cost>) -> Cost {
    runs.sort_by(|a, b| a.micros.total_cmp(&b.micros));
    runs.swap_remove(runs.len() / 2)
}
