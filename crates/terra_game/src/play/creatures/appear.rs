//! A creature comes into sight and goes out of it dithered over `FADE_S`, never whole in a
//! frame (docs/scene-rules.md, Wildlife and Appearance; the lake film's "animals pop into
//! existence"). Wildlife places animals out of view where it can; this is the rule for every
//! creature however it is put out or let go, as Godot's `visibility_range_fade_mode` (MIT)
//! and Unreal's LOD dither fade a thing in and out.
//!
//! It is drawn with Bevy's own visibility-range crossfade (D240), already on each creature's
//! meshes for the distance it is seen to: while it fades, the range's near margin is set
//! round where the camera stands from it, so the dither Bevy's shader reads from the
//! distance (`get_visibility_range_dither_level`, bevy_pbr's mesh_functions.wesl) is the
//! fade's. Bevy keeps every distinct range it is given for the app's life, so the distance
//! and the fade are rounded: a creature's fades use at most 64 ranges a metre of the
//! distance it is seen to.

use crate::play::interact::CanInteract;
use bevy::camera::visibility::VisibilityRange;
use bevy::prelude::*;
use terra_world::planet::MainCamera;

/// How long a creature takes to come into sight or go out of it, seconds.
pub const FADE_S: f32 = 0.6;
/// The widest the margin a fade is drawn over, metres: the narrower, the more the dither
/// differs from a body's near side to its far one.
const MARGIN_M: f32 = 8.0;
/// The distance a fade is drawn at is rounded to this, metres: a sixteenth of a dither step.
const QUANTUM_M: f32 = 0.25;
/// Bevy's dither has this many steps (`visibility_range_dither`).
const STEPS: f32 = 16.0;

/// How far into sight a creature is, 0 unseen to 1 whole, and whether it is going.
#[derive(Component, Clone, Copy, Debug)]
pub struct Shown {
    /// How far off it is drawn (`SpeciesDef::seen_m`).
    seen_m: f32,
    shown: f32,
    leaving: bool,
    /// Whether its meshes are in, so a fade is not over before there is anything to see.
    ready: bool,
    /// The range last set on its meshes.
    set: Option<Ranged>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ranged([f32; 4]);

impl Shown {
    pub fn new(seen_m: f32) -> Self {
        Self {
            seen_m,
            shown: 0.0,
            leaving: false,
            ready: false,
            set: None,
        }
    }

    /// The range its meshes are drawn with now: unseen until the fade begins.
    pub fn range_now(&self) -> VisibilityRange {
        match self.set {
            Some(Ranged([a, b, c, d])) => VisibilityRange {
                start_margin: a..b,
                end_margin: c..d,
                use_aabb: false,
            },
            None => unseen(),
        }
    }
}

/// Drawn whole, out to the distance it is seen to and crossfaded over 4 m past it.
pub fn whole(seen_m: f32) -> VisibilityRange {
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: seen_m..seen_m + 4.0,
        use_aabb: false,
    }
}

fn unseen() -> VisibilityRange {
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: 0.0..0.0,
        use_aabb: false,
    }
}

/// The range that draws a creature so far off, so far into sight: Bevy's near crossfade
/// moved round it, so its shader's dither level is 16 times how far it is from whole (the
/// far one where the near would reach past the distance it is seen to).
pub fn range_at(distance_m: f32, shown: f32, seen_m: f32) -> VisibilityRange {
    let steps = (shown.clamp(0.0, 1.0) * STEPS).round() / STEPS;
    if steps >= 1.0 || distance_m >= seen_m {
        return whole(seen_m);
    }
    if steps <= 0.0 {
        return unseen();
    }
    let near = (distance_m / QUANTUM_M).round() * QUANTUM_M;
    let rises_to = near + (1.0 - steps) * MARGIN_M;
    if rises_to <= seen_m {
        return VisibilityRange {
            start_margin: near - steps * MARGIN_M..rises_to,
            end_margin: seen_m..seen_m + 4.0,
            use_aabb: false,
        };
    }
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: near - (1.0 - steps) * MARGIN_M..near + steps * MARGIN_M,
        use_aabb: false,
    }
}

/// Lets a creature go: it fades out and is then despawned; one never shown goes at once.
pub fn leave(commands: &mut Commands, creature: Entity) {
    commands.queue(move |world: &mut World| {
        let Ok(mut entity) = world.get_entity_mut(creature) else {
            return;
        };
        match entity.get_mut::<Shown>() {
            Some(mut shown) if shown.ready && shown.shown > 0.0 => {
                shown.leaving = true;
                // Going, it is not there to act on: a carcass is not butchered twice.
                entity.remove::<CanInteract>();
            }
            _ => entity.despawn(),
        }
    });
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        fade.after(TransformSystems::Propagate)
            .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
    );
}

/// Its meshes are in: drawn as it stands in its fade.
pub(super) fn meshes_in(shown: &mut Shown) -> VisibilityRange {
    shown.ready = true;
    shown.range_now()
}

/// Moves each creature's fade on and draws its meshes by it; one gone out of sight is
/// despawned.
pub(super) fn fade(
    mut commands: Commands,
    time: Res<Time>,
    cameras: Query<&GlobalTransform, With<MainCamera>>,
    mut creatures: Query<(Entity, &mut Shown, &GlobalTransform)>,
    children: Query<&Children>,
    mut ranges: Query<&mut VisibilityRange>,
) {
    let step = time.delta_secs() / FADE_S;
    for (entity, mut shown, at) in &mut creatures {
        if !shown.ready {
            continue;
        }
        let settled = !shown.leaving && shown.shown >= 1.0;
        if settled && shown.set == Some(ranged(&whole(shown.seen_m))) {
            continue;
        }
        shown.shown = if shown.leaving {
            (shown.shown - step).max(0.0)
        } else {
            (shown.shown + step).min(1.0)
        };
        if shown.leaving && shown.shown <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let distance = cameras
            .iter()
            .map(|eye| eye.translation().distance(at.translation()))
            .fold(f32::MAX, f32::min);
        let range = range_at(distance, shown.shown, shown.seen_m);
        let now = ranged(&range);
        if shown.set == Some(now) {
            continue;
        }
        shown.set = Some(now);
        for child in children.iter_descendants(entity) {
            if let Ok(mut mesh) = ranges.get_mut(child) {
                *mesh = range.clone();
            }
        }
    }
}

fn ranged(range: &VisibilityRange) -> Ranged {
    Ranged([
        range.start_margin.start,
        range.start_margin.end,
        range.end_margin.start,
        range.end_margin.end,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    /// How far from whole Bevy's shader draws a vertex so far from the camera, 0 whole to 16
    /// unseen: the size of the level `get_visibility_range_dither_level` gives it, below 0
    /// fading in by the near margin, above by the far.
    fn dither_level(range: &VisibilityRange, distance: f32) -> i32 {
        let (x, y, z, w) = (
            range.start_margin.start,
            range.start_margin.end,
            range.end_margin.start,
            range.end_margin.end,
        );
        let (offset, (a, b)) = if distance >= z {
            (0, (z, w))
        } else {
            (-16, (x, y))
        };
        let level = ((distance - a) / (b - a) * 16.0).round() as i32;
        (offset + level.clamp(0, 16)).abs()
    }

    #[test]
    fn a_fade_draws_a_creature_as_far_into_sight_as_it_is_and_whole_once_in() {
        let seen = 60.0;
        for distance in [0.7, 3.0, 12.3, 40.0, 59.0] {
            for i in 0..=16 {
                let shown = i as f32 / 16.0;
                let range = range_at(distance, shown, seen);
                assert_eq!(
                    range.is_visible_at_all(distance),
                    shown > 0.0,
                    "{distance} m, {shown}"
                );
                if shown > 0.0 {
                    let level = dither_level(&range, distance);
                    let wanted = (16.0 * (1.0 - shown)).round() as i32;
                    assert!(
                        (level - wanted).abs() <= 1,
                        "{distance} m at {shown}: {level}, not {wanted}"
                    );
                    // A body a metre deep is drawn much the same front and back.
                    if distance > 4.0 {
                        let back = dither_level(&range, distance + 0.5);
                        assert!((back - level).abs() <= 2, "{distance} m: {level} to {back}");
                    }
                }
            }
            assert!(range_at(distance, 1.0, seen) == whole(seen));
        }
        // Beyond the distance it is seen to, its own crossfade holds.
        assert!(range_at(70.0, 0.3, seen) == whole(seen));
    }

    #[test]
    fn the_ranges_a_fade_makes_are_few_however_the_camera_moves() {
        let mut distinct = std::collections::HashSet::new();
        for d in 0..6000 {
            for i in 0..60 {
                let r = range_at(d as f32 * 0.01, i as f32 / 59.0, 60.0);
                distinct.insert(ranged(&r).0.map(f32::to_bits));
            }
        }
        assert!(distinct.len() <= 60 * 64 + 2, "{} ranges", distinct.len());
    }

    fn world_with_creature(at: Vec3) -> (World, Entity, Entity) {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.spawn((MainCamera, GlobalTransform::default()));
        let mut shown = Shown::new(60.0);
        let first = meshes_in(&mut shown);
        let creature = world
            .spawn((shown, GlobalTransform::from_translation(at)))
            .id();
        let mesh = world.spawn((first, ChildOf(creature))).id();
        (world, creature, mesh)
    }

    fn after(world: &mut World, seconds: f32) {
        let frames = (seconds * 60.0).round() as usize;
        for _ in 0..frames {
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(1.0 / 60.0));
            world.run_system_once(fade).unwrap();
        }
    }

    #[test]
    fn a_creature_put_out_in_view_comes_in_over_the_fade_and_goes_out_over_it() {
        let at = Vec3::new(0.0, 0.0, -20.0);
        let (mut world, creature, mesh) = world_with_creature(at);
        let level = |world: &World| {
            let range = world.get::<VisibilityRange>(mesh).unwrap();
            range
                .is_visible_at_all(20.0)
                .then(|| dither_level(range, 20.0))
        };
        // Unseen as it is put out, half in halfway, whole once the fade is over.
        assert_eq!(level(&world), None);
        after(&mut world, FADE_S * 0.5);
        let half = level(&world).expect("seen halfway in");
        assert!((6..=10).contains(&half), "half in, level {half}");
        after(&mut world, FADE_S * 0.5 + 0.05);
        assert!(world.get::<VisibilityRange>(mesh).unwrap() == &whole(60.0));
        // Let go: still there, fading, then gone.
        leave(&mut world.commands(), creature);
        world.flush();
        after(&mut world, FADE_S * 0.5);
        assert!(world.get_entity(creature).is_ok());
        assert!(level(&world).is_some_and(|l| l > 4), "{:?}", level(&world));
        after(&mut world, FADE_S * 0.5 + 0.05);
        assert!(world.get_entity(creature).is_err(), "never went");
    }

    #[test]
    fn a_fade_waits_for_the_creatures_meshes() {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.spawn((MainCamera, GlobalTransform::default()));
        let creature = world
            .spawn((
                Shown::new(60.0),
                GlobalTransform::from_translation(Vec3::Z * -10.0),
            ))
            .id();
        after(&mut world, FADE_S * 2.0);
        let mut shown = *world.get::<Shown>(creature).unwrap();
        let range = meshes_in(&mut shown);
        assert!(
            !range.is_visible_at_all(10.0),
            "whole the moment its meshes came in"
        );
    }
}
