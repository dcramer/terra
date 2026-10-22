//! What swims tells the water where it goes (`terra_world::wakes`), so a duck paddling across
//! a lake trails a wake: each swimmer afloat, its velocity over the last frames smoothed, as
//! wide as its body, and where it was a wake's length back along its way, so the wake bends
//! with its turns.

use super::{Creature, State};
use crate::species::{Bestiary, Moves};
use bevy::{ecs::entity::EntityHashMap, prelude::*};
use std::collections::VecDeque;
use terra_world::planet::MainCamera;
use terra_world::wakes::{Swimmer, Wakes, reach_m};

pub fn plugin(app: &mut App) {
    app.add_systems(Update, follow);
}

/// How much of a frame's velocity is taken into the smoothed one: the creatures move in fixed
/// steps, which a frame's own difference would show as a jitter.
const SMOOTHING: f32 = 0.2;
/// How often a swimmer's way is marked, seconds, and how many marks are kept: 20 s, longer
/// than a duck takes to swim its wake's length.
const MARK_S: f32 = 0.5;
const MARKS: usize = 40;
/// How far back along the wake its bend is taken from, of the wake's reach.
const BEND_AT: f32 = 0.6;

/// What is kept of a swimmer between frames: where it was, its smoothed velocity, the marks
/// along its way, newest first, and how long since the last.
#[derive(Default)]
struct Track {
    here: Vec3,
    velocity: Vec3,
    marks: VecDeque<Vec3>,
    since_s: f32,
}

/// Where a swimmer was a given distance back, the newest of its marks at least that far off,
/// newest first; none while it has not come so far.
fn was_back(marks: &VecDeque<Vec3>, here: Vec3, back_m: f32) -> Option<Vec3> {
    marks
        .iter()
        .copied()
        .find(|mark| mark.distance_squared(here) >= back_m * back_m)
}

fn follow(
    time: Res<Time>,
    bestiary: Res<Bestiary>,
    creatures: Query<(Entity, &Creature, &Transform)>,
    cameras: Query<&GlobalTransform, With<MainCamera>>,
    mut last: Local<EntityHashMap<Track>>,
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
        let mut track = last.remove(&entity).map_or_else(
            || Track { here, ..default() },
            |mut track| {
                let now = (here - track.here).reject_from(here.normalize_or_zero()) / dt;
                track.velocity = track.velocity.lerp(now, SMOOTHING);
                track.here = here;
                track
            },
        );
        track.since_s += dt;
        if track.marks.is_empty() || track.since_s >= MARK_S {
            track.since_s = 0.0;
            track.marks.push_front(here);
            track.marks.truncate(MARKS);
        }
        let wide_m = about.body.radius * 2.0;
        let back_m = reach_m(wide_m, track.velocity.length()) * BEND_AT;
        swimmers.push(Swimmer {
            at: here,
            velocity: track.velocity,
            wide_m,
            was: was_back(&track.marks, here, back_m),
        });
        seen.insert(entity, track);
    }
    *last = seen;
    wakes.set_if_neq(Wakes::of(swimmers, eye));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wake_bends_back_through_where_the_swimmer_was() {
        // Swam 6 m north, then turned and swam 3 m east; marks newest first.
        let mut marks: VecDeque<Vec3> = (0..=6)
            .map(|north| Vec3::new(0.0, 0.0, north as f32))
            .chain((1..=3).map(|east| Vec3::new(east as f32, 0.0, 6.0)))
            .collect();
        marks.make_contiguous().reverse();
        let here = Vec3::new(3.0, 0.0, 6.0);
        // 4 m back is round the corner, on the way it came north, not straight behind it.
        let was = was_back(&marks, here, 4.0).unwrap();
        assert!(was.x < 0.5 && was.z < 6.0, "{was}");
        // A trail shorter than asked bends nothing.
        assert_eq!(was_back(&marks, here, 20.0), None);
    }
}
