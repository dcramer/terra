//! Bodies: creatures, people and the player take up room, so they cannot stand in each
//! other or in a tree. Each is a circle on the ground; where two overlap, both are pushed
//! apart, the lighter further. Trees and rocks do not move.
//!
//! Players are not pushed here: the mover stops them at others' bodies (D53,
//! `mover_world::others`), so the camera never sees one shoved after it has moved. Others
//! are pushed off them instead. People push softly (D479): a person is met by the mover only by
//! a man's core, so a player walks past one, and two people, or a person and a player, ease
//! apart by a share of their overlap a tick, as the mover's soft pairs do (a man's `soft_s`).
//!
//! A stand-in until creatures walk the mover too (D53).

use crate::Player;
use crate::movement::Movers;
use bevy::prelude::*;
use terra_build::Walker;
use terra_motion::mover::Push;
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
    /// How it pushes (D479): a person softly, anything else hard.
    pub push: Push,
}

impl Body {
    /// A creature's, which pushes hard.
    pub fn new(radius: f32, weight: f32) -> Self {
        Self {
            radius,
            weight,
            push: Push::Hard,
        }
    }

    /// A person's, who pushes softly (D479): gives way and never blocks.
    pub fn person(radius: f32, weight: f32) -> Self {
        Self {
            push: Push::Soft,
            ..Self::new(radius, weight)
        }
    }

    /// One that holds its place, such as someone bound.
    pub fn fixed(radius: f32) -> Self {
        Self::new(radius, 0.0)
    }

    /// Whether it gives way softly: a person who is not held in place.
    pub fn soft(&self) -> bool {
        self.push == Push::Soft && self.weight > 0.0
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

/// Within this of a player a body is kept out of what stands every tick; further off, one
/// tick in so many.
const NEAR_M: f32 = 30.0;
const FAR_EVERY: u32 = 4;

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

/// Moves overlapping bodies apart, and out of trees, rocks and players, then puts them
/// back on the ground. Each body is compared only with those filed near it, so the cost
/// grows with how crowded a place is, not with how many bodies there are (D127).
#[allow(clippy::too_many_arguments)]
pub(crate) fn keep_apart(
    time: Res<Time>,
    planet: Res<Planet>,
    obstacles: Res<Obstacles>,
    movers: Option<Res<Movers>>,
    players: Query<&Transform, With<Player>>,
    mut bodies: Query<(Entity, &Body, &mut Transform), Without<Player>>,
    mut filed: Local<SpatialHash<usize>>,
    mut standing: Local<SpatialHash<()>>,
    mut settled: Local<Settled>,
    mut tick: Local<u32>,
) {
    *tick = tick.wrapping_add(1);
    let mut all: Vec<_> = bodies.iter_mut().collect();
    let widest = (all.iter().map(|(_, body, _)| body.radius)).fold(0.0, f32::max);
    filed.clear();
    filed.extend(
        all.iter()
            .enumerate()
            .map(|(index, (_, _, transform))| (transform.translation, index)),
    );
    // A soft pair parts by this share of its overlap a tick (a man's kind's time, as the
    // mover's soft pairs); with no walk, at once.
    let soft_s = movers.map_or(0.0, |movers| movers.kind().soft_s);
    let eased = if soft_s > 0.0 {
        1.0 - libm::expf(-time.delta_secs() / soft_s)
    } else {
        1.0
    };
    let mut moved = vec![Vec3::ZERO; all.len()];
    for i in 0..all.len() {
        let (a, here) = (*all[i].1, all[i].2.translation);
        let up = here.normalize();
        for &(there, j) in filed.near(here, a.radius + widest) {
            if j <= i {
                continue;
            }
            let b = *all[j].1;
            let apart = (here - there).reject_from(up);
            let reach = a.radius + b.radius;
            if apart.length_squared() >= reach * reach {
                continue;
            }
            // Two in the same spot part along any line.
            let away = apart.normalize_or(up.any_orthonormal_vector());
            let mut overlap = reach - apart.length();
            if a.soft() && b.soft() {
                overlap *= eased;
            }
            let (share_a, share_b) = shares(a, b);
            moved[i] += away * overlap * share_a;
            moved[j] -= away * overlap * share_b;
        }
    }
    let walker = Walker::default().radius;
    standing.clear();
    standing.extend(players.iter().map(|player| (player.translation, ())));
    settled.forget_the_gone(all.len());
    for (index, (entity, body, transform)) in all.iter_mut().enumerate() {
        if body.weight == 0.0 {
            continue;
        }
        // One at rest where it was last settled, with nobody pushing it and no player in
        // reach, is where it belongs still: most of a land's animals at any moment, and
        // asking the trees and rocks about each of them every tick was most of this step
        // (1.9 ms a frame with 170 about).
        let pushed = moved[index] != Vec3::ZERO;
        let reach = body.radius + walker;
        let by_player = standing.near(transform.translation, reach).next().is_some();
        if !pushed && !by_player && settled.at(*entity) == Some(transform.translation) {
            continue;
        }
        // Far from every player, what stands is asked of one tick in four, each body on its
        // own: an animal steers round trunks itself (its mind's), this is what catches it
        // when it does not, and a sixteenth of a second inside a trunk a field away is
        // not seen.
        let far = !players.iter().any(|player| {
            player.translation.distance_squared(transform.translation) < NEAR_M * NEAR_M
        });
        if far && !pushed && (*tick).wrapping_add(entity.index_u32()) % FAR_EVERY != 0 {
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
                let ease = if body.soft() { eased } else { 1.0 };
                let away = apart.normalize_or(up.any_orthonormal_vector());
                at += away * (reach - apart.length()) * ease;
            }
        }
        // What is lower than a step they walk over, as the player does (D267).
        at = obstacles.push_out(at, body.radius, Walker::default().step_up);
        if at != transform.translation {
            transform.translation = planet.ground_at(at);
        }
        settled.keep(*entity, transform.translation);
    }
}

/// Where each body was when it was last looked at against what stands, so one that has not
/// moved since is not looked at again.
#[derive(Default)]
pub(crate) struct Settled(bevy::platform::collections::HashMap<Entity, Vec3>);

impl Settled {
    fn at(&self, body: Entity) -> Option<Vec3> {
        self.0.get(&body).copied()
    }

    fn keep(&mut self, body: Entity, at: Vec3) {
        self.0.insert(body, at);
    }

    /// Bodies gone leave their places behind: dropped once they outnumber those here.
    fn forget_the_gone(&mut self, here: usize) {
        if self.0.len() > here * 2 + 64 {
            self.0.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use terra_core::{PlanetSpec, Seed};

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
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(1.0 / 64.0));
        world.insert_resource(time);
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

    /// People push softly (D479): two people in each other ease apart by a man's soft time,
    /// a share a tick, as the mover's soft pairs do; a person and a creature part at once.
    #[test]
    fn two_people_ease_apart_and_a_person_and_a_creature_part_at_once() {
        let parted = |other: Body| {
            let (mut world, ground) = world();
            world.init_resource::<Movers>();
            let a = world
                .spawn((Body::person(0.35, 2.0), Transform::from_translation(ground)))
                .id();
            let b = world
                .spawn((other, Transform::from_translation(ground + Vec3::X * 0.5)))
                .id();
            world.run_system_once(keep_apart).unwrap();
            apart(&world, a, b)
        };
        let soft_s = Movers::default().kind().soft_s;
        let eased = 1.0 - libm::expf(-1.0 / 64.0 / soft_s);
        let want = 0.5 + 0.2 * eased;
        let soft = parted(Body::person(0.35, 2.0));
        assert!((soft - want).abs() < 1e-3, "{soft} apart, want {want}");
        assert!(parted(Body::new(0.35, 2.0)) > 0.7 - 1e-3);
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

    /// A body at rest is left alone tick after tick, and is still moved the tick something
    /// comes on it: a player stepping into it, or another body put on top of it. Skipped
    /// while it rests, it must not stay skipped then.
    #[test]
    fn a_settled_body_is_left_until_something_comes_on_it() {
        let (mut world, ground) = world();
        // One system kept from tick to tick, so what it remembers of each body is kept.
        let mut tick = Schedule::default();
        tick.add_systems(keep_apart);
        let hare = world
            .spawn((Body::new(0.3, 1.0), Transform::from_translation(ground)))
            .id();
        let at = |world: &World| world.get::<Transform>(hare).unwrap().translation;
        for _ in 0..3 {
            tick.run(&mut world);
        }
        let rested = at(&world);
        // A player steps onto it: it gives way.
        let player = world
            .spawn((Player, Transform::from_translation(rested + Vec3::X * 0.1)))
            .id();
        tick.run(&mut world);
        let gave = at(&world);
        assert!(
            gave.distance(rested) > 0.05,
            "it did not give way to the player"
        );
        world.despawn(player);
        // Settled again where it now is, then another body is put on it: both part.
        for _ in 0..2 {
            tick.run(&mut world);
        }
        let boar = world
            .spawn((
                Body::new(0.4, 1.0),
                Transform::from_translation(at(&world) + Vec3::Z * 0.1),
            ))
            .id();
        let before = apart(&world, hare, boar);
        tick.run(&mut world);
        assert!(apart(&world, hare, boar) > before + 0.05);
    }

    /// Far from every player a body is asked about what stands one tick in four, so one that
    /// walks into a trunk is out of it within four ticks, not left there.
    #[test]
    fn a_far_body_in_a_trunk_is_out_of_it_within_four_ticks() {
        let (mut world, ground) = world();
        world.resource_mut::<Obstacles>().add(ground, 0.5);
        let boar = world
            .spawn((
                Body::new(0.4, 1.0),
                Transform::from_translation(ground + Vec3::X * 0.2),
            ))
            .id();
        let mut tick = Schedule::default();
        tick.add_systems(keep_apart);
        for _ in 0..FAR_EVERY {
            tick.run(&mut world);
        }
        let at = world.get::<Transform>(boar).unwrap().translation;
        let off = (at - ground).reject_from(ground.normalize()).length();
        assert!(
            off >= 0.85,
            "it stands {off} m from the trunk's middle, inside it"
        );
    }
}
