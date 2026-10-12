//! Simulation level of detail (D145): what is near a player is simulated in full, what is
//! farther off thinks less often, and what is beyond every player waits. A world with
//! thousands of creatures (D127) and a thousand players spends its ticks where someone
//! can see the result. It is what shipped games call simulation or AI LOD, and what
//! Unreal's significance manager does.
//!
//! Each thing simulated this way has a `Detail`, worked out every tick from how far it
//! is from the nearest player. Systems that simulate it ask its detail whether it acts on
//! this tick, and over how long. Things at reduced detail take turns, so their thinking is
//! spread over the ticks rather than falling on one.

use super::Player;
use super::tick::Sim;
use crate::spatial::SpatialHash;
use bevy::prelude::*;

/// Within this of a player, simulated in full.
const FULL_WITHIN_M: f32 = 64.0;
/// Within this, simulated at reduced detail; beyond it, waiting.
const REDUCED_WITHIN_M: f32 = 256.0;
/// At reduced detail, it thinks once in so many ticks.
const REDUCED_EVERY: u64 = 8;

/// How much of something is simulated.
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Detail {
    /// Near a player: every tick.
    #[default]
    Full,
    /// Farther off: it still moves every tick, so it is seen to move smoothly, but thinks
    /// only now and then.
    Reduced,
    /// Beyond every player: it waits where it is.
    Dormant,
}

impl Detail {
    /// Whether something at this detail thinks on a tick, and if so over how many
    /// seconds: a tick's, or as many ticks' as it waited.
    pub fn thinks(self, tick: u64, entity: Entity, dt: f32) -> Option<f32> {
        match self {
            Detail::Full => Some(dt),
            Detail::Reduced => (tick + u64::from(entity.index_u32()))
                .is_multiple_of(REDUCED_EVERY)
                .then_some(dt * REDUCED_EVERY as f32),
            Detail::Dormant => None,
        }
    }

    /// Whether it moves at all.
    pub fn moves(self) -> bool {
        self != Detail::Dormant
    }

    /// The detail for something so far across from the nearest player.
    fn at(apart: f32) -> Detail {
        if apart <= FULL_WITHIN_M {
            Detail::Full
        } else if apart <= REDUCED_WITHIN_M {
            Detail::Reduced
        } else {
            Detail::Dormant
        }
    }
}

/// Runs before anything that reads the detail.
#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Grading;

pub fn plugin(app: &mut App) {
    app.configure_sets(FixedUpdate, Grading.before(Sim::World))
        .add_systems(FixedUpdate, grade.in_set(Grading));
}

/// Works out each thing's detail from the nearest player.
fn grade(
    players: Query<&Transform, With<Player>>,
    mut things: Query<(&Transform, &mut Detail), Without<Player>>,
    mut near: Local<Option<SpatialHash<()>>>,
) {
    let near = near.get_or_insert_with(|| SpatialHash::new(REDUCED_WITHIN_M));
    near.clear();
    near.extend(players.iter().map(|at| (at.translation, ())));
    for (at, mut detail) in &mut things {
        let here = at.translation;
        let apart = near
            .near(here, REDUCED_WITHIN_M)
            .map(|(player, _)| player.distance(here))
            .fold(f32::INFINITY, f32::min);
        detail.set_if_neq(Detail::at(apart));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_falls_with_distance_from_the_nearest_player() {
        let mut world = World::new();
        let ground = Vec3::Y * 2000.0;
        world.spawn((Player, Transform::from_translation(ground)));
        world.spawn((
            Player,
            Transform::from_translation(ground + Vec3::X * 1000.0),
        ));
        let at = |world: &mut World, east: f32| {
            world
                .spawn((
                    Transform::from_translation(ground + Vec3::X * east),
                    Detail::Full,
                ))
                .id()
        };
        let near = at(&mut world, 10.0);
        let far = at(&mut world, 150.0);
        let beyond = at(&mut world, 500.0);
        let by_the_other = at(&mut world, 990.0);
        world.run_system_cached(grade).unwrap();
        let detail = |entity| *world.get::<Detail>(entity).unwrap();
        assert_eq!(detail(near), Detail::Full);
        assert_eq!(detail(far), Detail::Reduced);
        assert_eq!(detail(beyond), Detail::Dormant);
        assert_eq!(detail(by_the_other), Detail::Full);
    }

    #[test]
    fn reduced_detail_thinks_once_in_so_many_ticks_over_all_of_them() {
        let entity = Entity::PLACEHOLDER;
        let dt = 1.0 / 64.0;
        let thought: Vec<f32> = (0..REDUCED_EVERY * 4)
            .filter_map(|tick| Detail::Reduced.thinks(tick, entity, dt))
            .collect();
        assert_eq!(thought.len(), 4);
        assert!(
            thought
                .iter()
                .all(|&over| over == dt * REDUCED_EVERY as f32)
        );
        assert_eq!(Detail::Full.thinks(3, entity, dt), Some(dt));
        assert_eq!(Detail::Dormant.thinks(0, entity, dt), None);
    }
}
