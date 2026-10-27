//! `cost.climb`: the mover's cost a tick of a body holding a face and climbing it (D477,
//! docs/controller.md "Climbing"). A man walks into each of the course's climbing faces (the
//! sheer wall, the dug cliff and a base's wall of cubes, `course::climbing_faces`), takes hold
//! and climbs straight up; only the ticks he starts holding the face are timed, each alone,
//! with the ground's field calls they make. A climbing tick probes the face three times, moves
//! and snaps back to it, so it costs more than a walking one; the bar is a walking body's.

use super::{Cost, RUNS, median};
use bevy::math::DVec3;
use bevy::prelude::Vec3;
use std::cell::Cell;
use std::time::Instant;
use terra_motion::course::field::{Course, Field, forward_at};
use terra_motion::mover::{Bodies, Ground, Intent, Kind, Obstacles, Sdf, World};

const TICK: f32 = 1.0 / 64.0;
/// The longest a man is given to reach a face and take hold, s, and how long he climbs then.
const REACH_S: f32 = 3.0;
const CLIMB_S: f32 = 1.5;

/// One run up a face: the µs and field calls of a climbing tick, and how far the feet rose.
fn climb(course: &Course, start: Vec3, kind: &Kind) -> Cost {
    let calls = Cell::new(0u64);
    let sdf = Sdf {
        distance: |at: DVec3| {
            calls.set(calls.get() + 1);
            course.distance_at(at)
        },
        // The course's fixtures are exact distances outside the solid.
        steepest: 1.0,
    };
    let obstacles = Obstacles {
        obstacles: course.obstacles(),
    };
    let water = |_: Vec3| None;
    let world = World {
        ground: &(&sdf, &obstacles),
        water: &water,
        gravity: course.gravity(),
        centre: Vec3::ZERO,
    };
    let forward = forward_at(start);
    let mut bodies = Bodies::default();
    bodies.add(0, *kind, start, forward, &world);
    let intent = [(
        0,
        Intent {
            wish: forward,
            ..Intent::default()
        },
    )];
    let holding = |bodies: &Bodies| bodies.get(0).is_some_and(|b| b.ground == Ground::Climbing);
    for _ in 0..(REACH_S / TICK) as usize {
        if holding(&bodies) {
            break;
        }
        bodies.tick(&world, &intent, TICK);
    }
    let from = bodies.get(0).map_or(start, |b| b.feet);
    let (mut seconds, mut ticks) = (0.0_f64, 0u32);
    calls.set(0);
    for _ in 0..(CLIMB_S / TICK) as usize {
        if !holding(&bodies) {
            break;
        }
        let began = Instant::now();
        bodies.tick(&world, &intent, TICK);
        seconds += began.elapsed().as_secs_f64();
        ticks += 1;
    }
    let up = start.normalize();
    let ticks_f = ticks.max(1) as f32;
    Cost {
        micros: seconds as f32 * 1.0e6 / ticks_f,
        calls: calls.get() as f32 / ticks_f,
        walked: bodies.get(0).map_or(0.0, |b| (b.feet - from).dot(up)),
    }
}

/// Each face's median cost a climbing tick, a bench line each, and `cost.climb`, the worst.
pub fn print(kind: &Kind) {
    let mut worst = 0.0_f32;
    for (name, course, start) in terra_motion::course::climbing_faces(kind) {
        let cost = median((0..RUNS).map(|_| climb(&course, start, kind)).collect());
        println!(
            "bench\tclimbing the {name}\tmover {:.1} µs {:.0} calls a climbing tick, {:.2} m up",
            cost.micros, cost.calls, cost.walked
        );
        worst = worst.max(cost.micros);
    }
    println!("scorecard\tcost.climb\t{worst}");
}
