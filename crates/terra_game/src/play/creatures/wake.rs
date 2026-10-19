//! What swims tells the water where it goes (`terra_world::wakes`), so a duck paddling across
//! a lake trails a wake: each swimmer afloat, its velocity over the last frames smoothed, as
//! wide as its body.

use super::{Creature, State};
use crate::play::species::{Bestiary, Moves};
use bevy::{ecs::entity::EntityHashMap, prelude::*};
use terra_world::planet::MainCamera;
use terra_world::wakes::{Swimmer, Wakes};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, follow);
}

/// How much of a frame's velocity is taken into the smoothed one: the creatures move in fixed
/// steps, which a frame's own difference would show as a jitter.
const SMOOTHING: f32 = 0.2;

fn follow(
    time: Res<Time>,
    bestiary: Res<Bestiary>,
    creatures: Query<(Entity, &Creature, &Transform)>,
    cameras: Query<&GlobalTransform, With<MainCamera>>,
    mut last: Local<EntityHashMap<(Vec3, Vec3)>>,
    mut wakes: ResMut<Wakes>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Some(eye) = cameras.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let mut seen = EntityHashMap::default();
    let mut swimmers = Vec::new();
    for (entity, creature, at) in &creatures {
        let about = bestiary.get(creature.species);
        if !matches!(about.moves, Moves::Swims(_)) || matches!(creature.state, State::Fly { .. }) {
            continue;
        }
        let here = at.translation;
        let velocity = last.get(&entity).map_or(Vec3::ZERO, |&(before, velocity)| {
            let now = (here - before).reject_from(here.normalize_or_zero()) / dt;
            velocity.lerp(now, SMOOTHING)
        });
        seen.insert(entity, (here, velocity));
        swimmers.push(Swimmer {
            at: here,
            velocity,
            wide_m: about.body.radius * 2.0,
        });
    }
    *last = seen;
    wakes.set_if_neq(Wakes::of(swimmers, eye));
}
