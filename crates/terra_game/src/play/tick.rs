//! The game's tick (D138): gameplay runs at a fixed rate, in `FixedUpdate`, apart from
//! what is shown, which runs every frame. A server ticks the same way, and a client
//! predicts its own player with the same ticks (D51).
//!
//! Each tick runs in stages, in order (`Sim`): players act on their intent, then the world
//! answers and goes on, then what happened is settled. What players mean to do is read
//! before the ticks of a frame (`intent::read`), and a press lasts until a tick has seen
//! it.
//!
//! Bodies move tick by tick, which at the screen's rate would stutter, so a body that moves
//! (`Smoothed`) is shown between where it stood after the last tick and where it stands
//! after this one, by how far the frame is into the next tick. Gameplay always sees where
//! it truly stands. This is what `bevy_transform_interpolation` does.

use bevy::prelude::*;

/// The stages of a tick, in order.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sim {
    /// Players act on their intent: moving, using tools, building, interacting.
    Act,
    /// The world answers what players did, and goes on: creatures, people, stations, falls.
    World,
    /// What happened is settled: blows taken, bodies kept apart.
    Resolve,
    /// The tick is over: presses it has seen are spent.
    End,
}

/// Something that moves tick by tick and is shown smoothly between ticks: where it stood
/// before the tick and where it stands after it.
#[derive(Component, Default, Clone, Copy)]
pub struct Smoothed {
    before: Option<Transform>,
    after: Option<Transform>,
    /// What was last shown, so a move made outside the ticks (a teleport) is noticed and
    /// not undone.
    shown: Option<Transform>,
}

pub fn plugin(app: &mut App) {
    app.configure_sets(
        FixedUpdate,
        (Sim::Act, Sim::World, Sim::Resolve, Sim::End).chain(),
    )
    .add_systems(FixedFirst, take_up)
    .add_systems(FixedLast, put_down)
    .add_systems(
        RunFixedMainLoop,
        show.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
    );
}

/// Before a tick: each body stands where it truly is, which is where it starts the tick.
/// One moved outside the ticks since it was shown stays where it was moved to.
fn take_up(mut bodies: Query<(&mut Transform, &mut Smoothed)>) {
    for (mut transform, mut smoothed) in &mut bodies {
        let moved = smoothed.shown.is_some_and(|shown| shown != *transform);
        if !moved && let Some(after) = smoothed.after {
            *transform = after;
        }
        smoothed.before = Some(*transform);
        smoothed.after = None;
    }
}

/// After a tick: where each body stands now.
fn put_down(mut bodies: Query<(&Transform, &mut Smoothed)>) {
    for (transform, mut smoothed) in &mut bodies {
        smoothed.after = Some(*transform);
        smoothed.shown = None;
    }
}

/// Each frame, after the ticks: each body is shown as far between its last two places as
/// the frame is into the next tick.
fn show(time: Res<Time<Fixed>>, mut bodies: Query<(&mut Transform, &mut Smoothed)>) {
    let along = time.overstep_fraction();
    for (mut transform, mut smoothed) in &mut bodies {
        let (Some(before), Some(after)) = (smoothed.before, smoothed.after) else {
            continue;
        };
        // Moved outside the ticks since: shown where it was moved to.
        if smoothed.shown.is_some_and(|shown| shown != *transform) {
            smoothed.before = Some(*transform);
            smoothed.after = Some(*transform);
            smoothed.shown = Some(*transform);
            continue;
        }
        let between = between(&before, &after, along);
        if *transform != between {
            *transform = between;
        }
        smoothed.shown = Some(between);
    }
}

/// So far from one place and turn to another.
fn between(from: &Transform, to: &Transform, along: f32) -> Transform {
    Transform {
        translation: from.translation.lerp(to.translation, along),
        rotation: from.rotation.slerp(to.rotation, along),
        scale: from.scale.lerp(to.scale, along),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn between_two_ticks_is_between_their_places() {
        let from = Transform::from_xyz(0.0, 0.0, 0.0);
        let to = Transform::from_xyz(2.0, 0.0, 0.0);
        assert_eq!(between(&from, &to, 0.0), from);
        assert_eq!(between(&from, &to, 1.0), to);
        assert!((between(&from, &to, 0.25).translation.x - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_tick_starts_where_the_body_truly_is_and_a_teleport_stays() {
        let mut world = World::new();
        let body = world
            .spawn((Transform::from_xyz(1.0, 0.0, 0.0), Smoothed::default()))
            .id();
        // A tick moves it from 1 to 3; it is shown halfway between at 2.
        world.run_system_cached(take_up).unwrap();
        world.get_mut::<Transform>(body).unwrap().translation.x = 3.0;
        world.run_system_cached(put_down).unwrap();
        let smoothed = *world.get::<Smoothed>(body).unwrap();
        let shown = between(&smoothed.before.unwrap(), &smoothed.after.unwrap(), 0.5);
        *world.get_mut::<Transform>(body).unwrap() = shown;
        world.get_mut::<Smoothed>(body).unwrap().shown = Some(shown);
        // The next tick starts where it truly is, not where it was shown.
        world.run_system_cached(take_up).unwrap();
        assert_eq!(world.get::<Transform>(body).unwrap().translation.x, 3.0);
        // Moved outside the ticks, as by a warp: it stays where it was moved to.
        world.run_system_cached(put_down).unwrap();
        world.get_mut::<Smoothed>(body).unwrap().shown = Some(Transform::from_xyz(3.0, 0.0, 0.0));
        world.get_mut::<Transform>(body).unwrap().translation.x = 10.0;
        world.run_system_cached(take_up).unwrap();
        assert_eq!(world.get::<Transform>(body).unwrap().translation.x, 10.0);
    }
}
