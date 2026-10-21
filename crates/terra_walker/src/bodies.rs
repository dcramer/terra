//! Bodies: creatures, people and the player take up room, so they cannot stand in each
//! other or in a tree. Each is a circle on the ground; where two overlap, both are pushed
//! apart, the lighter further. Trees and rocks do not move.
//!
//! Players are not pushed here: their own walk stops them at others' bodies (see
//! `push_out`), so the camera never sees one shoved after it has moved. Others are pushed
//! off them instead.
//!
//! A stand-in until creatures walk on voxels with a real character controller (D53).

use crate::Player;
use bevy::prelude::*;
use terra_build::Walker;
use terra_world::flora::Obstacles;
use terra_world::planet::Planet;
use terra_world::spatial::SpatialHash;
use terra_world::tick::Sim;

/// How much room something takes up on the ground, and how hard it is to shove.
#[derive(Component, Clone, Copy)]
pub struct Body {
    pub radius: f32,
    /// Heavier bodies give way less. Zero never gives way.
    pub weight: f32,
}

impl Body {
    pub fn new(radius: f32, weight: f32) -> Self {
        Self { radius, weight }
    }

    /// One that holds its place, such as someone bound.
    pub fn fixed(radius: f32) -> Self {
        Self {
            radius,
            weight: 0.0,
        }
    }
}

/// Something a walker cannot walk into, such as a chest or a bed: the box from its foot,
/// so many metres to each side and twice `half.y` up, turned as its `Transform` is. What
/// is low enough to step over, such as a rug, has none. The layers above add it, as
/// props do to every solid item put down (`props::solid`).
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Solid {
    pub half: Vec3,
}

/// Moves a walker's next position out of any solid it would be standing inside, by the
/// nearest side. A walker is a circle on the ground; a solid is its box, walked through
/// while hidden.
pub fn push_out_of_solids<'a>(
    mut target: Vec3,
    walker_radius: f32,
    solids: impl Iterator<Item = (&'a Solid, &'a Transform, &'a Visibility)>,
) -> Vec3 {
    for (solid, transform, visibility) in solids {
        if *visibility == Visibility::Hidden {
            continue;
        }
        let turned_back = transform.rotation.inverse();
        let mut local = turned_back * (target - transform.translation);
        // Only if the feet are between its foot and its top.
        if local.y < -0.5 || local.y > solid.half.y * 2.0 - 0.05 {
            continue;
        }
        let reach = Vec2::new(solid.half.x, solid.half.z) + walker_radius;
        let inside = reach - Vec2::new(local.x.abs(), local.z.abs());
        if inside.x <= 0.0 || inside.y <= 0.0 {
            continue;
        }
        // Out by the nearest side.
        if inside.x < inside.y {
            local.x = reach.x.copysign(local.x);
        } else {
            local.z = reach.y.copysign(local.z);
        }
        target = transform.translation + transform.rotation * local;
    }
    target
}

pub fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, keep_apart.in_set(Sim::Resolve));
}

/// How far each of two overlapping bodies moves, by weight: the heavier moves less, a fixed
/// one not at all.
fn shares(a: Body, b: Body) -> (f32, f32) {
    match (a.weight == 0.0, b.weight == 0.0) {
        (true, true) => (0.0, 0.0),
        (true, false) => (0.0, 1.0),
        (false, true) => (1.0, 0.0),
        (false, false) => {
            let total = a.weight + b.weight;
            (b.weight / total, a.weight / total)
        }
    }
}

/// Pushes a point at `at` with a body of `radius` out of every body in `others`, along
/// the ground. For a walker that should stop at bodies rather than be shoved by them.
pub fn push_out<'a>(
    mut at: Vec3,
    radius: f32,
    others: impl Iterator<Item = (&'a Body, &'a Transform)>,
) -> Vec3 {
    let up = at.normalize();
    for (body, transform) in others {
        let apart = (at - transform.translation).reject_from(up);
        let reach = radius + body.radius;
        if apart.length_squared() < reach * reach && apart.length_squared() > 1e-8 {
            at += apart.normalize() * (reach - apart.length());
        }
    }
    at
}

/// Moves overlapping bodies apart, and out of trees, rocks and players, then puts them
/// back on the ground. Each body is compared only with those filed near it, so the cost
/// grows with how crowded a place is, not with how many bodies there are (D127).
fn keep_apart(
    planet: Res<Planet>,
    obstacles: Res<Obstacles>,
    players: Query<&Transform, With<Player>>,
    mut bodies: Query<(&Body, &mut Transform), Without<Player>>,
    mut filed: Local<SpatialHash<usize>>,
    mut standing: Local<SpatialHash<()>>,
) {
    let mut all: Vec<_> = bodies.iter_mut().collect();
    let widest = all.iter().map(|(body, _)| body.radius).fold(0.0, f32::max);
    filed.clear();
    filed.extend(
        all.iter()
            .enumerate()
            .map(|(index, (_, transform))| (transform.translation, index)),
    );
    let mut moved = vec![Vec3::ZERO; all.len()];
    for i in 0..all.len() {
        let (a, here) = (*all[i].0, all[i].1.translation);
        let up = here.normalize();
        for &(there, j) in filed.near(here, a.radius + widest) {
            if j <= i {
                continue;
            }
            let b = *all[j].0;
            let apart = (here - there).reject_from(up);
            let reach = a.radius + b.radius;
            if apart.length_squared() >= reach * reach {
                continue;
            }
            // Two in the same spot part along any line.
            let away = apart.normalize_or(up.any_orthonormal_vector());
            let overlap = reach - apart.length();
            let (share_a, share_b) = shares(a, b);
            moved[i] += away * overlap * share_a;
            moved[j] -= away * overlap * share_b;
        }
    }
    let walker = Walker::default().radius;
    standing.clear();
    standing.extend(players.iter().map(|player| (player.translation, ())));
    for (index, (body, transform)) in all.iter_mut().enumerate() {
        if body.weight == 0.0 {
            continue;
        }
        let start = transform.translation + moved[index];
        let mut at = start;
        let up = start.normalize();
        // Off players, who do not give way here, and out of trees and rocks.
        let reach = body.radius + walker;
        for &(place, ()) in standing.near(start, reach) {
            let apart = (at - place).reject_from(up);
            let reach = walker + body.radius;
            if apart.length_squared() < reach * reach {
                at += apart.normalize_or(up.any_orthonormal_vector()) * (reach - apart.length());
            }
        }
        // What is lower than a step they walk over, as the player does (D267).
        at = obstacles.push_out(at, body.radius, Walker::default().step_up);
        if at != transform.translation {
            transform.translation = planet.ground_at(at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use terra_core::{PlanetSpec, Seed};

    #[test]
    fn a_walker_is_pushed_out_of_a_solid_by_its_nearest_side() {
        let solid = Solid {
            half: Vec3::splat(0.4),
        };
        let push = |target: Vec3, visibility: Visibility| {
            push_out_of_solids(
                target,
                0.3,
                [(&solid, &Transform::IDENTITY, &visibility)].into_iter(),
            )
        };
        // Just inside the east side: out to the east, a radius clear of the box.
        let out = push(Vec3::new(0.5, 0.0, 0.1), Visibility::Inherited);
        assert!((out.x - 0.7).abs() < 1e-5 && (out.z - 0.1).abs() < 1e-5);
        // Clear of it already, or standing on top of it: left alone.
        for free in [Vec3::new(0.8, 0.0, 0.0), Vec3::new(0.0, 0.8, 0.0)] {
            assert_eq!(push(free, Visibility::Inherited), free);
        }
        // Hidden, it is walked through.
        let inside = Vec3::new(0.1, 0.0, 0.1);
        assert_eq!(push(inside, Visibility::Hidden), inside);
    }

    /// A world on a smooth planet, and a place on its ground.
    fn world() -> (World, Vec3) {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        let ground = planet.ground_at(Vec3::Y * 2100.0);
        let mut world = World::new();
        world.insert_resource(planet);
        world.init_resource::<Obstacles>();
        (world, ground)
    }

    fn apart(world: &World, a: Entity, b: Entity) -> f32 {
        let at = |entity| world.get::<Transform>(entity).unwrap().translation;
        (at(a) - at(b)).reject_from(at(a).normalize()).length()
    }

    #[test]
    fn overlapping_bodies_are_pushed_apart() {
        let (mut world, ground) = world();
        let deer = world
            .spawn((Body::new(0.5, 3.0), Transform::from_translation(ground)))
            .id();
        let spider = world
            .spawn((
                Body::new(0.4, 1.0),
                Transform::from_translation(ground + Vec3::X * 0.3),
            ))
            .id();
        world.run_system_once(keep_apart).unwrap();
        assert!(apart(&world, deer, spider) > 0.9 - 1e-3);
    }

    #[test]
    fn a_fixed_body_stays_where_it_is() {
        let (mut world, ground) = world();
        let bound = world
            .spawn((Body::fixed(0.4), Transform::from_translation(ground)))
            .id();
        let spider = world
            .spawn((
                Body::new(0.5, 1.0),
                Transform::from_translation(ground + Vec3::X * 0.3),
            ))
            .id();
        world.run_system_once(keep_apart).unwrap();
        assert_eq!(world.get::<Transform>(bound).unwrap().translation, ground);
        assert!(apart(&world, bound, spider) > 0.9 - 1e-3);
    }

    #[test]
    fn the_lighter_body_gives_way_more() {
        let (deer, spider) = (Body::new(0.5, 3.0), Body::new(0.5, 1.0));
        let (deer_moves, spider_moves) = shares(deer, spider);
        assert!(spider_moves > deer_moves);
        assert!((deer_moves + spider_moves - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_fixed_body_never_moves() {
        let (bound, spider) = (Body::fixed(0.4), Body::new(0.5, 1.0));
        assert_eq!(shares(bound, spider), (0.0, 1.0));
        assert_eq!(shares(bound, bound), (0.0, 0.0));
    }

    #[test]
    fn a_walker_stops_at_a_body() {
        let up = Vec3::Y * 100.0;
        let spider = (Body::new(0.6, 1.0), Transform::from_translation(up));
        let walked = push_out(
            up + Vec3::X * 0.3,
            0.4,
            std::iter::once((&spider.0, &spider.1)),
        );
        assert!(((walked - up).reject_from(Vec3::Y).length() - 1.0).abs() < 1e-4);
    }
}
