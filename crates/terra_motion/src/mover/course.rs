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
