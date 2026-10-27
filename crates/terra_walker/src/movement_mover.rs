//! The mover (D53, `terra_motion::mover`) walked on the planet's own voxel field, as the walk
//! will drive it: the ground source is [`Planet::distance`] swept by [`Planet::steepest`].
//! The course's fixtures are exact distances; the game's field is a bound (a height field's
//! distance overestimates on a slope, a dug hole's is a CSG of fields), and the mover's crease
//! rule assumes an exact one (round 2's warning), so it is proved here on the real field.

use bevy::math::DVec3;
use bevy::prelude::Vec3;
use terra_core::{Operation, PlanetSpec, Seed};
use terra_motion::mover::{Bodies, Ground, Intent, Kind, Sdf, World};
use terra_world::planet::{HeightMemo, Planet};

const TICK: f32 = 1.0 / 64.0;
const GRAVITY: f32 = 16.0;

/// The game's man: the walk's, his climbing off (`crate::movement::man`).
fn man() -> Kind {
    crate::movement::man()
}

/// A man walked forward over a planet for so long from a place on its ground: after each tick,
/// his feet, ground state, and how deep his capsule is into the ground (negative when clear).
fn walk(planet: &Planet, from: Vec3, seconds: f32) -> Vec<(Vec3, Ground, f32)> {
    let kind = man();
    let ground = Sdf {
        distance: |at: DVec3| planet.distance_in(at, &mut HeightMemo::default()),
        steepest: planet.steepest(),
    };
    let water = |_: Vec3| None;
    let world = World {
        ground: &ground,
        water: &water,
        gravity: GRAVITY,
        centre: Vec3::ZERO,
    };
    let forward = Vec3::X.reject_from(from.normalize()).normalize();
    let mut bodies = Bodies::default();
    bodies.add(0, kind, from, forward, &world);
    let intent = Intent {
        wish: forward,
        ..Intent::default()
    };
    (0..(seconds / TICK) as usize)
        .map(|_| {
            bodies.tick(&world, &[(0, intent)], TICK);
            let body = bodies.get(0).expect("the body");
            let axis = |t: f32| {
                body.feet + body.up * (kind.radius + t * (kind.height - 2.0 * kind.radius))
            };
            let nearest = (0..=8)
                .map(|i| planet.distance(axis(i as f32 / 8.0)))
                .fold(f32::INFINITY, f32::min);
            (body.feet, body.ground, kind.radius - nearest)
        })
        .collect()
}

fn planet() -> Planet {
    Planet::smooth(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    })
}

#[test]
fn the_mover_walks_the_planets_ground_on_it() {
    let planet = planet();
    let start = planet.ground_at(Vec3::Y * 2100.0);
    let trace = walk(&planet, start, 3.0);
    for (tick, (feet, ground, depth)) in trace.iter().enumerate() {
        assert!(*depth < 0.01, "tick {tick}: {depth} m into the ground");
        let over = feet.length() - planet.ground_at(*feet).length();
        assert!(over.abs() < 0.03, "tick {tick}: {over} m over the ground");
        assert_eq!(*ground, Ground::Grounded, "tick {tick}");
    }
    let (end, _, _) = trace[trace.len() - 1];
    assert!(
        end.distance(start) > 10.0,
        "walked {} m",
        end.distance(start)
    );
}

#[test]
fn a_body_walks_into_a_dug_hole_and_its_steep_wall_keeps_it_there() {
    let mut planet = planet();
    let start = planet.ground_at(Vec3::Y * 2100.0);
    let up = start.normalize();
    let forward = Vec3::X.reject_from(up).normalize();
    // A ball 2 m round dug 4 m ahead: its far wall is walkable at its foot and steeper than the
    // slope limit from 0.7 m up, with 1.3 m of it over a step's height above that.
    let centre = start + forward * 4.0;
    planet.apply(&Operation::Dig {
        centre,
        radius: 2.0,
    });
    let trace = walk(&planet, start, 4.0);
    let deepest = trace.iter().map(|t| t.2).fold(f32::MIN, f32::max);
    assert!(deepest < 0.01, "{deepest} m into the ground");
    let (end, _, _) = trace[trace.len() - 1];
    let rim = (start.length() - end.length()).max(0.0);
    assert!(
        end.distance(centre) < 2.0 && rim > 0.5,
        "out of the hole: {} m from its middle, {rim} m under the rim",
        end.distance(centre)
    );
}

#[test]
fn a_dug_ledge_a_voxel_high_is_stepped_up_and_one_two_high_stops_the_body_at_its_crease() {
    use bevy::prelude::{Mat3, Quat};
    for (deep, climbs) in [(0.5, true), (1.0, false)] {
        let mut planet = planet();
        let start = planet.ground_at(Vec3::Y * 2100.0);
        let up = start.normalize();
        let forward = Vec3::X.reject_from(up).normalize();
        let side = forward.cross(up);
        // A pit with upright walls 3 to 6 m ahead, 3 m wide: where its floor meets its far wall
        // the field's nearest faces are two, the crease.
        let half = Vec3::new(1.5, (1.0 + deep) * 0.5, 1.5);
        let centre = start + forward * 4.5 + up * ((1.0 - deep) * 0.5);
        planet.apply(&Operation::DigBox {
            centre,
            half,
            rotation: Quat::from_mat3(&Mat3::from_cols(forward, up, side)),
        });
        let trace = walk(&planet, start, 3.0);
        let deepest = trace.iter().map(|t| t.2).fold(f32::MIN, f32::max);
        assert!(deepest < 0.01, "{deep} m pit: {deepest} m into the ground");
        let (end, _, _) = trace[trace.len() - 1];
        let (along, height) = ((end - start).dot(forward), end.length() - start.length());
        if climbs {
            assert!(
                along > 7.0 && height.abs() < 0.05,
                "{deep} m: at {along} m, {height} m"
            );
        } else {
            assert!(
                along < 6.0 && (height + deep).abs() < 0.05,
                "{deep} m: at {along} m, {height} m up"
            );
        }
    }
}

#[test]
fn a_body_walks_over_where_two_dug_treads_meet() {
    use bevy::prelude::{Mat3, Quat};
    // Two treads dug side by side, as a builder lays them: 2-5 m ahead 0.6 m down, 5-8 m
    // ahead 0.9 m down, each 3 m across. Where they meet, the ground over the first's floor
    // is gone (more than a step: a wall there would stop the body). Laid in one frame, or
    // each square to the planet's up at its own spot (a few thousandths of a radian apart).
    for own_frames in [false, true] {
        let mut planet = planet();
        let start = planet.ground_at(Vec3::Y * 2100.0);
        let up = start.normalize();
        let forward = Vec3::X.reject_from(up).normalize();
        let side = forward.cross(up);
        let rotation = Quat::from_mat3(&Mat3::from_cols(forward, up, side));
        for (from, deep) in [(2.0, 0.6), (5.0, 0.9)] {
            let middle = start + forward * (from + 1.5);
            let (here, rotation) = match own_frames {
                false => (up, rotation),
                true => {
                    let here = middle.normalize();
                    (here, Quat::from_rotation_arc(up, here) * rotation)
                }
            };
            planet.apply(&Operation::DigBox {
                centre: middle + here * ((1.0 - deep) * 0.5),
                half: Vec3::new(1.5, (1.0 + deep) * 0.5, 1.5),
                rotation,
            });
        }
        let trace = walk(&planet, start, 4.0);
        let deepest = trace.iter().map(|t| t.2).fold(f32::MIN, f32::max);
        assert!(deepest < 0.01, "{deepest} m into the ground");
        let (end, _, _) = trace[trace.len() - 1];
        let along = (end - start).dot(forward);
        let height = end.length() - start.length();
        assert!(
            along > 7.0 && (height + 0.9).abs() < 0.05,
            "own frames {own_frames}: stopped {along} m ahead, {height} m up"
        );
    }
}
