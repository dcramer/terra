//! A crowd's cost (docs/controller.md criterion 20, `cost.crowd`; D52, D138): a thousand men
//! on the generated planet's ground at 30 km, on a grid 1.5 m apart, each walking or sprinting
//! across to its place mirrored through the crowd's middle and back, through the others, as
//! the course's crowd does; timed a tick on one core, the median of a few runs. The bodies'
//! sweeps, contacts and pushes find each other through the mover's broadphase (`Grid`).

use bevy::math::DVec3;
use bevy::prelude::{Vec2, Vec3};
use std::cell::{Cell, RefCell};
use std::time::Instant;
use terra_motion::mover::{Bodies, Intent, Kind, Sdf, World};
use terra_world::planet::{HeightMemo, Planet};

const TICK: f32 = 1.0 / 64.0;
/// The crowd: so many columns and rows, so far apart, m.
const COLUMNS: u32 = 40;
const ROWS: u32 = 25;
const APART_M: f32 = 1.5;
/// How long each run walks, s, and how many runs the median is of.
const SECONDS: f32 = 2.0;
pub const RUNS: usize = 3;
/// The planet's lattice points kept a body (as the bench's single body, `MEMO_MOST`).
const MEMO_MOST: usize = 1024;

pub struct Cost {
    /// A tick of the whole crowd, ms.
    pub millis: f32,
    /// The ground's field calls a body a tick.
    pub calls: f32,
    /// The crowd's mean distance from where it started, m: a crowd stuck proves nothing.
    pub walked: f32,
    pub bodies: usize,
}

/// One run of the crowd, from `start` with `forward` along the ground.
pub fn run(planet: &Planet, start: Vec3, forward: Vec3, kind: &Kind) -> Cost {
    let up = start.normalize();
    let side = up.cross(forward).normalize();
    let memo = RefCell::new(HeightMemo::default());
    let calls = Cell::new(0u64);
    let sdf = Sdf {
        distance: |at: DVec3| {
            calls.set(calls.get() + 1);
            planet.distance_in(at, &mut memo.borrow_mut())
        },
        steepest: planet.steepest(),
    };
    let water = |at: Vec3| crate::mover_world::water(planet, at);
    let world = World {
        ground: &sdf,
        water: &water,
        gravity: 9.81,
        centre: Vec3::ZERO,
    };
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
        bodies.add(id, *kind, planet.ground_at(above), forward, &world);
    }
    let began: Vec<Vec3> = bodies.iter().map(|b| b.feet).collect();
    let mut back = vec![false; count as usize];
    let mut intents = Vec::with_capacity(count as usize);
    let ticks = (SECONDS / TICK) as usize;
    calls.set(0);
    let clock = Instant::now();
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
        memo.borrow_mut().keep_within(MEMO_MOST * count as usize);
        bodies.tick(&world, &intents, TICK);
    }
    let millis = clock.elapsed().as_secs_f32() * 1.0e3 / ticks as f32;
    let walked = bodies
        .iter()
        .zip(&began)
        .map(|(b, from)| b.feet.distance(*from))
        .sum::<f32>()
        / count as f32;
    Cost {
        millis,
        calls: calls.get() as f32 / (ticks as f32 * count as f32),
        walked,
        bodies: count as usize,
    }
}

pub fn median(mut runs: Vec<Cost>) -> Cost {
    runs.sort_by(|a, b| a.millis.total_cmp(&b.millis));
    runs.swap_remove(runs.len() / 2)
}
