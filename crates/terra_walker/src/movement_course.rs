//! Today's walk driven through the controller's plain-Rust course (D53, docs/controller.md,
//! `terra_motion::course`): the same [`step`] the game's `walk` takes, over the course's
//! fixtures instead of the planet, with other bodies pushed out of as `bodies::push_out`
//! does and no base or obstacles. Its numbers seeded `tools/controller/baseline.tsv`;
//! `cargo xtask controller` runs it. Stage 2's mover replaces this adapter, not the course.

use super::{Around, Ask, Motion, Terrain, step, top_speed};
use crate::{Stance, bodies};
use bevy::prelude::*;
use terra_motion::course::{self, Field, Kind, Mover};

/// A course fixture as the walk asks the ground.
struct Fixture<'a>(&'a dyn Field);

impl Terrain for Fixture<'_> {
    fn ground_at(&self, at: Vec3) -> Vec3 {
        terra_world::planet::surface_through(at, |p| self.0.distance(p))
    }
    fn distance(&self, at: Vec3) -> f32 {
        self.0.distance(at)
    }
    fn normal(&self, at: Vec3) -> Vec3 {
        self.0.normal(at)
    }
    fn water_radius_m(&self, at: Vec3) -> f32 {
        // No water: the sea's level under the ground, as the planet answers.
        self.0.water(at).unwrap_or(0.0)
    }
}

/// Today's walker, a body at a time in the order the course gives them.
#[derive(Default)]
struct Today {
    bodies: Vec<(u32, Kind, Vec3, Motion)>,
}

impl Mover for Today {
    fn add(&mut self, id: u32, kind: &Kind, feet: Vec3, facing: Vec3, field: &dyn Field) {
        let mut motion = Motion::facing(facing);
        motion.on_ground = field.distance(feet) < 0.02;
        self.bodies.push((id, *kind, feet, motion));
    }

    fn tick(&mut self, field: &dyn Field, intents: &[(u32, course::Intent)], dt: f32) {
        let stance = Stance::default();
        for (id, intent) in intents {
            let Some(i) = self.bodies.iter().position(|b| b.0 == *id) else {
                continue;
            };
            let others: Vec<(bodies::Body, Transform)> = self
                .bodies
                .iter()
                .filter(|b| b.0 != *id)
                .map(|(_, kind, feet, _)| {
                    (
                        bodies::Body::new(kind.radius, kind.mass),
                        Transform::from_translation(*feet),
                    )
                })
                .collect();
            let (_, kind, feet, motion) = &mut self.bodies[i];
            let ask = Ask {
                wish: intent.wish,
                top_speed: top_speed(&stance, intent.sprint),
                drive: intent.drive,
                jump: motion.on_ground && intent.jump,
                dt,
            };
            let radius = kind.radius;
            let blocked =
                |target| bodies::push_out(target, radius, others.iter().map(|(b, t)| (b, t)));
            let around = Around {
                blocked,
                held_at: |_, ground| ground,
                ceiling: |_, _| None,
            };
            *feet = step(*feet, motion, &ask, &Fixture(field), around);
        }
    }

    fn body(&self, id: u32) -> course::Body {
        let (_, _, feet, motion) = self
            .bodies
            .iter()
            .find(|b| b.0 == id)
            .expect("a body the course added");
        let up = feet.normalize();
        course::Body {
            feet: *feet,
            up,
            velocity: motion.velocity + up * motion.rising,
            grounded: motion.on_ground,
        }
    }
}

/// The course's numbers for today's walk, printed for `cargo xtask controller`. It asserts
/// only that the course measured every criterion: the bars and the ratchet are the
/// scorecard's (`tools/controller`).
#[test]
fn the_controller_course_measures_todays_walk() {
    let measures = course::score(Today::default);
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
