//! `cost.edits`: the mover's cost a tick on a much-dug world. The body of [`super::mover`]
//! walks the 2 km planet's ground with 0, 1,000 and 10,000 edits made to it (digs, filled
//! spheres and dug boxes, a third each): nine in ten spread over the ground within 3 km of
//! the walk, one in ten clustered within 60 m of it (a base's diggings) and some reaching the
//! ground it walks. The ground keeps every edit as made (D21), and the mover asks it a few
//! hundred times a tick, so what an ask costs as edits grow is what this measures.

use super::{Cost, Ground, RUNS, median, mover, on};
use bevy::math::DVec3;
use bevy::prelude::{Quat, Vec3};
use std::time::Instant;
use terra_core::{MaterialId, Operation, PlanetSpec, Seed};
use terra_gen::hash::unit3;
use terra_world::planet::{HeightMemo, Planet};

/// How many edits each scene has made, in turn; `cost.edits` is the last's.
pub const COUNTS: [usize; 3] = [0, 1_000, 10_000];
/// How far over the ground the spread edits reach from the walk's middle, m.
const SPREAD_M: f32 = 3_000.0;
/// How far from the walk's middle the clustered ones are, m.
const CLUSTER_M: f32 = 60.0;
/// The share of edits clustered.
const CLUSTER_SHARE: usize = 10;

/// A number in [0, 1) for edit `i`'s `part`.
fn unit(i: usize, part: u32) -> f32 {
    unit3(0xED17, i as u32, part, 0)
}

/// Edit `i`: where it is (spread or clustered by its number) and what it is.
fn edit(planet: &Planet, middle: Vec3, forward: Vec3, i: usize) -> Operation {
    let up = middle.normalize();
    let right = up.cross(forward).normalize();
    let azimuth = unit(i, 0) * std::f32::consts::TAU;
    let across = forward * libm::cosf(azimuth) + right * libm::sinf(azimuth);
    let reach = if i.is_multiple_of(CLUSTER_SHARE) {
        CLUSTER_M
    } else {
        SPREAD_M
    };
    // Even over the area: the square root of a uniform number as the share of the reach.
    let along = libm::sqrtf(unit(i, 1)) * reach;
    let turn = Quat::from_axis_angle(up.cross(across).normalize(), along / planet.radius_m());
    let ground = planet.surface(turn * up);
    let at_up = ground.normalize();
    let size = 0.4 + unit(i, 2) * 0.8;
    let centre = ground + at_up * (unit(i, 3) - 0.6) * size;
    match i % 3 {
        0 => Operation::Dig {
            centre,
            radius: size,
        },
        1 => Operation::Fill {
            centre,
            radius: size * 0.8,
            material: MaterialId(1),
        },
        _ => Operation::DigBox {
            centre,
            half: Vec3::splat(size * 0.6),
            rotation: Quat::from_rotation_arc(Vec3::Y, at_up),
        },
    }
}

/// The mover's cost a tick with each of [`COUNTS`] edits made, and how long they took to
/// make, s.
pub fn run(kind: &terra_motion::mover::Kind, gravity: f32) -> Vec<(usize, Cost, f32)> {
    let mut planet = Planet::new(PlanetSpec {
        seed: Seed(1),
        radius_m: 2_000.0,
    });
    let (start, forward) = on(&planet);
    let middle = start + forward * 8.0;
    let mut made = 0;
    let mut costs = Vec::new();
    for &count in &COUNTS {
        let began = Instant::now();
        for i in made..count {
            planet.apply(&edit(&planet, middle, forward, i));
        }
        let making = began.elapsed().as_secs_f32();
        made = count;
        let water = |at: Vec3| crate::mover_world::water(&planet, at);
        let precise = |at: DVec3| planet.distance_in(at, &mut HeightMemo::default());
        let ground = Ground {
            precise: &precise,
            planet: Some(&planet),
            steepest: planet.steepest(),
            water: &water,
            gravity,
            start,
            forward,
        };
        let cost = median((0..RUNS).map(|_| mover(&ground, kind)).collect());
        costs.push((count, cost, making));
    }
    costs
}
