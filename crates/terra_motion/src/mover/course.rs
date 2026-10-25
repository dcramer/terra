//! The mover driven through the controller's plain-Rust course (D457, `crate::course`): the
//! course's fixtures answer as an exact signed distance field, their water as the medium.
//! `cargo xtask controller` runs its test (`tools/controller/criteria.json`'s `course`).

use super::{Bodies, Sdf, World};
use crate::course::{self, Field, Kind, Mover};
use glam::Vec3;

#[derive(Default)]
struct Ours(Bodies);

fn with_world<R>(field: &dyn Field, then: impl FnOnce(&World) -> R) -> R {
    let ground = Sdf {
        distance: |at: Vec3| field.distance(at),
        // The course's fixtures are exact distances outside the solid.
        steepest: 1.0,
    };
    let water = |at: Vec3| field.water(at);
    let world = World {
        ground: &ground,
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
