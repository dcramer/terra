//! Each of the world's sources driving the mover on the planet's own ground, as the walk will
//! chain them: what the course's rows say of a step, a door and a wall, met on real data.

use super::*;
use bevy::math::{DVec3, Mat3, Quat};
use terra_build::{BuildOp, Cube, Facing, Fill, Kind as BuildKind, Panel, Shape};
use terra_core::{PlanetSpec, Seed};
use terra_motion::mover::{Bodies, Capsules, Ground, Intent, Kind, Obstacles, Query, Sdf, World};
use terra_world::planet::HeightMemo;

const TICK: f32 = 1.0 / 64.0;
const GRAVITY: f32 = 16.0;

fn man() -> Kind {
    Kind::parse(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/data/movers.ron"
        )),
        "Man",
    )
    .expect("a man")
}

fn smooth() -> Planet {
    Planet::smooth(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    })
}

/// A place on the smooth planet's ground and the turn of a frame there: x along the way
/// walked, y up.
fn yard(planet: &Planet) -> Transform {
    let start = planet.ground_at(Vec3::Y * 2100.0);
    let up = start.normalize();
    let forward = Vec3::X.reject_from(up).normalize();
    Transform::from_translation(start).with_rotation(Quat::from_mat3(&Mat3::from_cols(
        forward,
        up,
        forward.cross(up),
    )))
}

/// A man walked from a place in a frame along a way in it, over the planet's ground and what
/// else is given: his feet in the frame after each tick, and his ground state.
fn walk(
    planet: &Planet,
    also: &dyn Query,
    water: &dyn Fn(Vec3) -> Option<f32>,
    frame: &Transform,
    (from, way): (Vec3, Vec3),
    seconds: f32,
) -> Vec<(Vec3, Ground)> {
    let ground = Sdf {
        distance: |at: DVec3| planet.distance_in(at, &mut HeightMemo::default()),
        steepest: planet.steepest(),
    };
    let world = World {
        ground: &(&ground, also),
        water,
        gravity: GRAVITY,
        centre: Vec3::ZERO,
    };
    let feet = frame.transform_point(from);
    let way = frame.rotation * way;
    let mut bodies = Bodies::default();
    bodies.add(0, man(), feet, way, &world);
    let intent = Intent {
        wish: way,
        ..Intent::default()
    };
    let back = frame.compute_affine().inverse();
    (0..(seconds / TICK) as usize)
        .map(|_| {
            bodies.tick(&world, &[(0, intent)], TICK);
            let body = bodies.get(0).expect("the body");
            (back.transform_point3(body.feet), body.ground)
        })
        .collect()
}

fn dry(_: Vec3) -> Option<f32> {
    None
}

fn last(trace: &[(Vec3, Ground)]) -> Vec3 {
    trace[trace.len() - 1].0
}

/// A base of cubes, each at a cell (x, layer, z) of a shape.
fn cubes(at: &[([i32; 3], Shape)]) -> Base {
    let mut base = Base::default();
    for &(cell, shape) in at {
        base.apply(BuildOp::Fill {
            min: cell,
            max: cell,
            cube: Some(Cube {
                shape,
                ..Cube::new(BuildKind::PLAIN, Fill::Solid)
            }),
        });
    }
    base
}

/// A base's boxes near a frame's origin as the mover meets them.
fn base_blocks(built: &Base, frame: &Transform) -> Vec<Block> {
    base(built, frame, frame.translation, 12.0, None)
}

#[test]
fn a_bases_cube_is_stepped_onto_two_stop_a_body_and_a_flight_of_stairs_carries_it_up() {
    let planet = smooth();
    let frame = yard(&planet);
    // A row of cubes 2 m ahead, two cells wide across the way, half a metre each.
    let wide = |layers: &[(i32, Shape)], x: i32| -> Vec<([i32; 3], Shape)> {
        (-2..2)
            .flat_map(|z| layers.iter().map(move |&(y, s)| ([x, y, z], s)))
            .collect()
    };
    let trace = |built: &Base| {
        let blocks = base_blocks(built, &frame);
        let obstacles = Obstacles { obstacles: &blocks };
        walk(
            &planet,
            &obstacles,
            &dry,
            &frame,
            (Vec3::ZERO, Vec3::X),
            3.0,
        )
    };
    let walked = |built: &Base| last(&trace(built));
    // One cube, 0.5 m, under the man's 0.55 m step: on it and over.
    let one = walked(&cubes(&wide(&[(0, Shape::Whole)], 4)));
    assert!(one.x > 6.0, "one cube stopped him at {one}");
    // Two, 1 m: stopped at its face, 2 m on, by his radius.
    let two = walked(&cubes(&wide(&[(0, Shape::Whole), (1, Shape::Whole)], 4)));
    assert!(
        (two.x - (2.0 - 0.3)).abs() < 0.05 && two.y.abs() < 0.05,
        "two cubes: at {two}"
    );
    // A flight: a stair rising east on the ground, then on one cube, then on two, to a landing
    // a cube wide 1.5 m up, and off its end.
    let stair = Shape::Stair(Facing::East);
    let mut flight = wide(&[(0, stair)], 4);
    flight.extend(wide(&[(0, Shape::Whole), (1, stair)], 5));
    flight.extend(wide(&[(0, Shape::Whole), (1, Shape::Whole), (2, stair)], 6));
    flight.extend(wide(
        &[(0, Shape::Whole), (1, Shape::Whole), (2, Shape::Whole)],
        7,
    ));
    let up = trace(&cubes(&flight));
    let landing = up
        .iter()
        .find(|(at, g)| at.x > 3.5 && *g == Ground::Grounded)
        .map(|(at, _)| *at);
    assert!(
        landing.is_some_and(|at| (at.y - 1.5).abs() < 0.02),
        "up the flight to {landing:?}"
    );
}

/// A room 4 m by 3 with a doorway a metre wide in its south wall, a door fitted in it opening
/// into the room, shut or open.
fn room_with_door(open: bool) -> Base {
    let mut base = Base::default();
    base.apply(BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [7, 0, 5],
        kind: BuildKind::PLAIN,
    });
    for x in 3..5 {
        base.apply(BuildOp::SetWall {
            tile: [x, 0, 5],
            panel: Some(Panel::Door),
            kind: BuildKind::PLAIN,
        });
    }
    let rules = terra_build::Rules::default();
    let door = terra_build::Prop {
        min: [3, 0, 5],
        max: [4, 3, 5],
        opens: [0, 0, -1],
        open,
        ..terra_build::Prop::default()
    };
    base.act(
        &terra_build::ops::Op::FitProp(door),
        &terra_build::ops::Context::free(&rules),
    )
    .expect("the door fits");
    base
}

#[test]
fn a_shut_door_stops_a_body_in_its_doorway_and_an_open_one_lets_it_in() {
    let planet = smooth();
    let frame = yard(&planet);
    // From 2 m outside the south wall (its outer face at z = 3 m), north through the doorway.
    let way = (Vec3::new(2.0, 0.0, 5.0), Vec3::NEG_Z);
    let walked = |open: bool| {
        let blocks = base_blocks(&room_with_door(open), &frame);
        let obstacles = Obstacles { obstacles: &blocks };
        walk(&planet, &obstacles, &dry, &frame, way, 2.5)
    };
    let shut = last(&walked(false));
    assert!(
        shut.z > 2.5 + 0.3 && shut.z < 3.4,
        "the shut door let him to {shut}"
    );
    // Open, in and on to the north wall's inner face, half a metre in, by his radius.
    let inside = last(&walked(true));
    assert!(
        (inside.z - 0.8).abs() < 0.02,
        "the open door stopped him at {inside}"
    );
}

#[test]
fn a_props_box_stands_from_its_foot_turned_and_a_hidden_one_is_walked_through() {
    let planet = smooth();
    let frame = yard(&planet);
    let up = frame.rotation * Vec3::Y;
    let turn = Quat::from_axis_angle(up, 0.5) * frame.rotation;
    let foot = frame.transform_point(Vec3::new(2.5, 0.0, 0.0));
    let chest = bodies::Solid {
        half: Vec3::new(0.4, 0.5, 0.3),
    };
    let placed = Transform::from_translation(foot).with_rotation(turn);
    let block = prop(&chest, &placed);
    // Its top a metre over its foot, its turned side 0.4 m from its middle.
    assert!(block.distance(foot + up * 1.25) - 0.25 < 1e-3);
    let side = turn * Vec3::X;
    assert!((block.distance(foot + up * 0.5 + side * 0.6) - 0.2).abs() < 1e-3);
    // Shown, a man walked at it is kept out of it (sliding round its turned face); hidden, he
    // walks through it.
    for (shown, stops) in [(Visibility::Inherited, true), (Visibility::Hidden, false)] {
        let blocks = props([(&chest, &placed, &shown)], foot, 4.0);
        assert_eq!(blocks.len(), usize::from(stops));
        let obstacles = Obstacles { obstacles: &blocks };
        let trace = walk(
            &planet,
            &obstacles,
            &dry,
            &frame,
            (Vec3::ZERO, Vec3::X),
            2.0,
        );
        let nearest = trace
            .iter()
            .map(|(at, _)| block.distance(frame.transform_point(*at + Vec3::Y * 0.5)))
            .fold(f32::INFINITY, f32::min);
        if stops {
            assert!(nearest > 0.3 - 0.01, "{nearest} m from the box");
        } else {
            assert!(nearest < 0.0, "kept {nearest} m from a hidden box");
        }
    }
}

#[test]
fn a_trunk_borrowed_from_flora_and_a_body_standing_there_stop_a_man_at_their_width() {
    let planet = smooth();
    let frame = yard(&planet);
    let ahead = frame.transform_point(Vec3::new(3.0, 0.0, 0.0));
    let mut trees = flora::Obstacles::default();
    trees.add(ahead, 0.4);
    let near = flora(&trees, frame.translation, 6.0);
    assert_eq!(near.len(), 1);
    let trunk = Obstacles { obstacles: &near };
    let end = last(&walk(
        &planet,
        &trunk,
        &dry,
        &frame,
        (Vec3::ZERO, Vec3::X),
        2.0,
    ));
    assert!((end.x - (3.0 - 0.4 - 0.3)).abs() < 0.05, "at {end}");
    // A person there instead, 0.35 m round.
    let person = bodies::Body::new(0.35, 70.0);
    let standing = Transform::from_translation(ahead);
    let capsules = others([(7, &person, &standing)], 1.8);
    let them = Capsules {
        bodies: &capsules,
        skip: 0,
    };
    let end = last(&walk(
        &planet,
        &them,
        &dry,
        &frame,
        (Vec3::ZERO, Vec3::X),
        2.0,
    ));
    assert!((end.x - (3.0 - 0.35 - 0.3)).abs() < 0.05, "at {end}");
}

#[test]
fn water_is_where_the_ground_is_under_the_sea_and_a_body_put_in_it_swims() {
    let planet = Planet::new(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    let sea = planet.sea_radius_m();
    let ground = |up: Vec3| planet.surface(up).length();
    let directions = (0..4000).map(|i| {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / 4000.0;
        let around = i as f32 * 2.399_963;
        let flat = libm::sqrtf(1.0 - y * y);
        Vec3::new(flat * libm::cosf(around), y, flat * libm::sinf(around))
    });
    let (mut wet, mut dry_land) = (None, None);
    for up in directions {
        if wet.is_none() && ground(up) < sea - 4.0 && planet.water_radius_m(up * sea) == sea {
            wet = Some(up);
        }
        if dry_land.is_none() && ground(up) > sea + 10.0 {
            dry_land = Some(up);
        }
    }
    let (wet, dry_land) = (wet.expect("deep sea"), dry_land.expect("a hill"));
    // Over the sea floor, the sea's surface; on the hill, no water, even under the sea's level.
    assert_eq!(water(&planet, wet * (ground(wet) + 1.0)), Some(sea));
    assert_eq!(water(&planet, dry_land * (ground(dry_land) + 0.1)), None);
    assert_eq!(water(&planet, dry_land * (sea - 2.0)), None);
    // A man put a metre under the surface swims.
    let water_at = |at: Vec3| water(&planet, at);
    let field = Sdf {
        distance: |at: DVec3| planet.distance_in(at, &mut HeightMemo::default()),
        steepest: planet.steepest(),
    };
    let world = World {
        ground: &field,
        water: &water_at,
        gravity: GRAVITY,
        centre: Vec3::ZERO,
    };
    let mut bodies = Bodies::default();
    bodies.add(
        0,
        man(),
        wet * (sea - 1.0),
        wet.any_orthonormal_vector(),
        &world,
    );
    for _ in 0..32 {
        bodies.tick(&world, &[], TICK);
    }
    assert_eq!(bodies.get(0).map(|b| b.ground), Some(Ground::Swimming));
}
