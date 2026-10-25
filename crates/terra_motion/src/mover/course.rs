//! The mover driven through the controller's plain-Rust course (D457, `crate::course`): the
//! course's fixtures answer as an exact signed distance field, what stands on them as the
//! mover's obstacles, their water as the medium.
//! `cargo xtask controller` runs its test (`tools/controller/criteria.json`'s `course`).

use super::{Bodies, Obstacles, Sdf, World};
use crate::course::{self, Field, Kind, Mover};
use glam::{DVec3, Vec3};

#[derive(Default)]
struct Ours(Bodies);

fn with_world<R>(field: &dyn Field, then: impl FnOnce(&World) -> R) -> R {
    let ground = Sdf {
        distance: |at: DVec3| field.distance_at(at),
        // The course's fixtures are exact distances outside the solid.
        steepest: 1.0,
    };
    let obstacles = Obstacles {
        obstacles: field.obstacles(),
    };
    let water = |at: Vec3| field.water(at);
    let world = World {
        ground: &(&ground, &obstacles),
        water: &water,
        gravity: field.gravity(),
        centre: Vec3::ZERO,
    };
    then(&world)
}

impl Mover for Ours {
    fn add(&mut self, id: u32, kind: &Kind, feet: Vec3, facing: Vec3, field: &dyn Field) {
        with_world(field, |world| self.0.add(id, *kind, feet, facing, world));
    }

    fn tick(&mut self, field: &dyn Field, intents: &[(u32, course::Intent)], dt: f32) {
        with_world(field, |world| self.0.tick(world, intents, dt));
    }

    fn body(&self, id: u32) -> course::Body {
        let body = self.0.get(id).expect("a body the course added");
        course::Body {
            feet: body.feet,
            position: body.position,
            up: body.up,
            velocity: body.velocity,
            grounded: body.ground == super::Ground::Grounded,
        }
    }

    fn facing(&self, id: u32) -> Option<Vec3> {
        self.0.get(id).map(|body| body.facing)
    }

    fn landed(&self, id: u32) -> Option<f32> {
        self.0.get(id).and_then(|body| body.landed)
    }
}

/// The course's numbers for the mover, printed for `cargo xtask controller`. It asserts only
/// that the course measured every criterion: the bars and the ratchet are the scorecard's.
#[test]
fn the_controller_course_measures_the_mover() {
    let measures = course::score(Ours::default);
    course::print(&measures);
    assert!(!measures.is_empty());
    for m in &measures {
        assert!(
            m.value.is_finite() && m.value >= 0.0,
            "{} = {}",
            m.id,
            m.value
        );
    }
}

/// A body of a kind walked forward from a place over a profile for so long: its feet in the
/// course's frame after each tick.
fn walked(kind: &Kind, corners: &[(f32, f32)], seconds: f32) -> Vec<Vec3> {
    use crate::course::field::{Course, Solid, forward_at, local, world};
    let course = Course::of(Solid::Profile(
        corners
            .iter()
            .map(|&(u, h)| glam::Vec2::new(u, h))
            .collect(),
    ));
    let mut ours = Ours::default();
    let feet = world(0.0, 0.0, 0.0);
    let wish = forward_at(feet);
    ours.add(0, kind, feet, wish, &course);
    let intent = course::Intent {
        wish,
        ..Default::default()
    };
    (0..(seconds / course::TICK) as usize)
        .map(|_| {
            ours.tick(&course, &[(0, intent)], course::TICK);
            local(ours.body(0).feet)
        })
        .collect()
}

/// Level ground to 1 m on, then a slope of a grade, deg.
fn slope(deg: f32) -> [(f32, f32); 3] {
    let (s, c) = (libm::sinf(deg.to_radians()), libm::cosf(deg.to_radians()));
    [(-5.0, 0.0), (1.0, 0.0), (1.0 + 40.0 * c, 40.0 * s)]
}

#[test]
fn a_body_keeps_its_speed_along_a_steep_floor() {
    // Carried as its level part, the speed up 45 deg was capped at about 60% of the curve's.
    let man = Kind::man();
    let feet = walked(&man, &slope(45.0), 2.5);
    let second = (1.0 / course::TICK) as usize;
    let speed = (feet[feet.len() - 1] - feet[feet.len() - 1 - second]).length();
    let want = man.jog * man.slope_factor(45.0);
    assert!(
        (speed / want - 1.0).abs() < 0.02,
        "{speed} m/s up 45 deg, not {want}"
    );
}

#[test]
fn a_wide_body_walks_up_a_shallow_slope_from_its_foot() {
    // At the crease the field's one contact blends the floor and the slope, and the sweep along
    // it stops on the slope's face at once: the body stood at the foot until the next
    // collision iteration solved against that face too.
    let wide = Kind {
        radius: 0.5,
        height: 2.4,
        jog: 3.5,
        accel: 12.0,
        ..Kind::man()
    };
    let feet = walked(&wide, &slope(15.0), 2.0);
    let end = feet[feet.len() - 1];
    assert!(end.x > 3.0, "stopped at {end:?}, the slope's foot at 1 m");
}

#[test]
fn a_landing_is_reported_at_the_speed_of_the_fall() {
    use crate::course::field::{Course, forward_at, world};
    let man = Kind::man();
    let flat = Course::flat();
    for height in [0.5, 1.0, 2.0, 4.0] {
        let mut ours = Ours::default();
        let at = world(0.0, height, 0.0);
        ours.add(0, &man, at, forward_at(at), &flat);
        let landed = (0..128).find_map(|_| {
            ours.tick(&flat, &[], course::TICK);
            ours.landed(0)
        });
        let want = libm::sqrtf(2.0 * flat.gravity() * height);
        let landed = landed.expect("it lands");
        assert!(
            (landed - want).abs() < 0.01,
            "{landed} m/s from {height} m, not {want}"
        );
    }
}

/// A man on the flat course's ground at its origin, facing along it.
fn on_flat() -> (Ours, crate::course::Course, Vec3) {
    use crate::course::field::{Course, forward_at, world};
    let flat = Course::flat();
    let mut ours = Ours::default();
    let at = world(0.0, 0.0, 0.0);
    ours.add(0, &Kind::man(), at, forward_at(at), &flat);
    (ours, flat, forward_at(at))
}

#[test]
fn a_body_taken_out_is_gone_and_the_rest_still_move() {
    use crate::course::field::world;
    let (mut ours, flat, forward) = on_flat();
    let beside = world(0.0, 0.0, 3.0);
    ours.add(1, &Kind::man(), beside, forward, &flat);
    assert!(ours.0.remove(0).is_some_and(|body| body.id == 0));
    assert!(ours.0.remove(0).is_none() && ours.0.get(0).is_none());
    let walk = course::Intent {
        wish: forward,
        ..Default::default()
    };
    for _ in 0..64 {
        ours.tick(&flat, &[(0, walk), (1, walk)], course::TICK);
    }
    assert_eq!(ours.0.iter().count(), 1);
    assert!(
        ours.body(1).feet.distance(beside) > 3.0,
        "the other walked on"
    );
}

#[test]
fn a_body_placed_stands_or_falls_where_it_is_put() {
    use crate::course::field::world;
    let (mut ours, flat, _) = on_flat();
    let man = Kind::man();
    // Teleported 2 m up and still: airborne, it falls and lands at sqrt(2gh).
    let high = world(5.0, 2.0, 0.0);
    with_world(&flat, |w| assert!(ours.0.place(0, high, Vec3::ZERO, w)));
    assert!(!ours.body(0).grounded && ours.body(0).feet == high);
    let landed = (0..128).find_map(|_| {
        ours.tick(&flat, &[], course::TICK);
        ours.landed(0)
    });
    let want = libm::sqrtf(2.0 * flat.gravity() * 2.0);
    assert!(
        landed.is_some_and(|v| (v - want).abs() < 0.01),
        "{landed:?}"
    );
    // Put on the ground at a run and then held still (downed): grounded at once, unmoving.
    let ground = world(9.0, 0.0, 0.0);
    with_world(&flat, |w| ours.0.place(0, ground, Vec3::ZERO, w));
    let body = ours.body(0);
    assert!(body.grounded && body.velocity == Vec3::ZERO);
    ours.tick(&flat, &[], course::TICK);
    let moved = ours.body(0).feet - ground;
    // Within a float's step at 2 km from the centre.
    assert!(moved.length() < 1e-3, "{moved:?}");
    assert!(with_world(&flat, |w| !ours.0.place(
        7,
        ground,
        Vec3::ZERO,
        w
    )));
    assert_eq!(ours.0.get(0).map(|b| b.kind.jog), Some(man.jog));
}

#[test]
fn a_share_of_its_speed_is_its_top_speed() {
    let man = Kind::man();
    for (scale, sprint) in [(0.5, false), (0.35, true), (1.0, false)] {
        let (mut ours, flat, forward) = on_flat();
        let intent = course::Intent {
            wish: forward,
            sprint,
            scale,
            ..Default::default()
        };
        for _ in 0..128 {
            ours.tick(&flat, &[(0, intent)], course::TICK);
        }
        let speed = ours.body(0).velocity.length();
        let want = if sprint { man.sprint } else { man.jog } * scale;
        assert!((speed / want - 1.0).abs() < 0.005, "{speed}, not {want}");
    }
}

#[test]
fn a_drive_moves_a_body_at_its_own_speed_from_the_first_tick() {
    // A roll: 8.5 m/s along the way, past the kind's sprint, with no acceleration.
    let (mut ours, flat, forward) = on_flat();
    let intent = course::Intent {
        drive: Some(forward * 8.5),
        ..Default::default()
    };
    let start = ours.body(0).feet;
    for _ in 0..32 {
        ours.tick(&flat, &[(0, intent)], course::TICK);
        assert!((ours.body(0).velocity.length() - 8.5).abs() < 0.01);
        assert!(ours.body(0).grounded);
    }
    let gone = ours.body(0).feet.distance(start);
    assert!((gone - 8.5 * 0.5).abs() < 0.02, "{gone} m in half a second");
}

#[test]
fn a_jump_is_reported_on_the_tick_it_leaves_not_the_press() {
    use crate::course::field::world;
    let (mut ours, flat, _) = on_flat();
    let jump = course::Intent {
        jump: true,
        ..Default::default()
    };
    // Pressed standing: it leaves that tick.
    ours.tick(&flat, &[(0, jump)], course::TICK);
    assert!(ours.0.get(0).is_some_and(|b| b.jumped));
    ours.tick(&flat, &[], course::TICK);
    assert!(ours.0.get(0).is_some_and(|b| !b.jumped));
    // Pressed in the air a few ticks before it lands: not then, but on landing (the buffer),
    // and once.
    let man = Kind::man();
    let drop = 0.3;
    with_world(&flat, |w| {
        ours.0.place(0, world(0.0, drop, 0.0), Vec3::ZERO, w)
    });
    let fall = (libm::sqrtf(2.0 * drop / flat.gravity()) / course::TICK) as usize;
    let press = fall - (man.buffer_s / course::TICK) as usize / 2;
    let jumped: Vec<usize> = (0..fall + 16)
        .filter(|&tick| {
            let intents = if tick == press {
                vec![(0, jump)]
            } else {
                vec![]
            };
            ours.tick(&flat, &intents, course::TICK);
            ours.0.get(0).is_some_and(|b| b.jumped)
        })
        .collect();
    assert_eq!(jumped.len(), 1, "{jumped:?}");
    assert!(
        jumped[0] > press,
        "jumped at {jumped:?}, pressed at {press}"
    );
}

#[test]
fn a_body_walking_up_a_slope_asks_the_ground_as_often_as_on_the_level() {
    // Gravity's press into a slope has a level part: taken for a wish to move, it made the
    // walk look blocked every tick, and a step up, forward and down was tried for nothing.
    use crate::course::field::{Course, Solid, forward_at, local, world};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Counted(Course, AtomicUsize);
    impl Field for Counted {
        fn distance(&self, at: Vec3) -> f32 {
            self.1.fetch_add(1, Ordering::Relaxed);
            self.0.distance(at)
        }
        fn distance_at(&self, at: DVec3) -> f32 {
            self.1.fetch_add(1, Ordering::Relaxed);
            self.0.distance_at(at)
        }
    }
    let man = Kind::man();
    let calls_a_tick = |corners: [(f32, f32); 3]| {
        let profile = corners.iter().map(|&(u, h)| glam::Vec2::new(u, h));
        let field = Counted(Course::of(Solid::Profile(profile.collect())), 0.into());
        let mut ours = Ours::default();
        let feet = world(0.0, 0.0, 0.0);
        ours.add(0, &man, feet, forward_at(feet), &field);
        let (mut ticks, mut calls) = (0, 0);
        for _ in 0..(2.5 / course::TICK) as usize {
            let intent = course::Intent {
                wish: forward_at(ours.body(0).feet),
                ..Default::default()
            };
            let before = field.1.load(Ordering::Relaxed);
            ours.tick(&field, &[(0, intent)], course::TICK);
            // On the slope's face, past its foot's crease.
            if local(ours.body(0).feet).x > 2.0 {
                ticks += 1;
                calls += field.1.load(Ordering::Relaxed) - before;
            }
        }
        calls as f32 / ticks as f32
    };
    let level = calls_a_tick([(-5.0, 0.0), (1.0, 0.0), (40.0, 0.0)]);
    let up = calls_a_tick(slope(20.0));
    assert!(
        up < level * 1.5,
        "{up} calls a tick up 20 deg, {level} on the level"
    );
}
