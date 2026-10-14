//! Indoor light (D155; docs/building-plan.md, drawing): a room closed all round, roof and
//! floor too (`terra_build::Base::indoors`, D261), is not lit by the open sky. The sky lights the
//! world through the camera's environment map (`terra_world::skylight`); inside each such
//! room stands a light probe holding the same sky, dimmed, which Bevy uses in place of the
//! camera's for what is inside it, as shipped games place a probe per room. The sun still
//! comes in through doors and windows, by its shadows. A probe per room lit from what is
//! really round it is later work.

use super::store::each;
use super::{Built, Site};
use crate::planet::MainCamera;
use bevy::light::{EnvironmentMapLight, LightProbe};
use bevy::prelude::*;
use std::collections::BTreeMap;

/// How much of the sky's light reaches inside a room.
const INDOORS: f32 = 0.6;
/// How far a room's probe reaches past its tiles, into its walls' inner faces and up
/// under its ceiling, in metres; and how much of it fades out at its edges.
const INTO_WALLS_M: f32 = 0.3;
const FALLOFF: f32 = 0.03;

/// How long a room's light takes to change when it is closed or opened, in seconds: a probe
/// is blended in and out, as engines blend a probe's weight, not switched, which dimmed a
/// room by two fifths in one frame as its last cube went in (`cargo xtask artifacts
/// --frames`, going-up).
const FADE_S: f32 = 0.8;

/// A light probe dimming the sky inside a room of a base: the room's box, how far it is
/// faded in, from 0 (lit as outside) to 1, and whether it is going, its room no longer
/// closed.
#[derive(Component)]
struct Indoors {
    room: [i32; 6],
    share: f32,
    going: bool,
}

/// A room's box as a key, to the centimetre.
fn key(min: Vec3, max: Vec3) -> [i32; 6] {
    let cm = |m: f32| (m * 100.0).round() as i32;
    [
        cm(min.x),
        cm(min.y),
        cm(min.z),
        cm(max.x),
        cm(max.y),
        cm(max.z),
    ]
}

/// The revision of each base its rooms were last looked at for, and the rooms' boxes
/// its probes were placed for.
#[derive(Resource, Default)]
struct Placed(BTreeMap<Entity, (u64, Vec<(IVec3, IVec3)>)>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Placed>()
        .add_systems(Update, (place, follow, inside).chain());
}

/// Places a probe in every room indoors, again whenever a base changes: a room that is new
/// fades its probe in, one no longer closed fades its out; rooms that stay keep theirs.
fn place(
    mut commands: Commands,
    mut placed: ResMut<Placed>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
    mut probes: Query<(Entity, &ChildOf, &mut Indoors)>,
    camera: Query<&EnvironmentMapLight, With<MainCamera>>,
) {
    // Nothing to dim until the sky lights the camera.
    let Some(sky) = camera.iter().next().cloned() else {
        return;
    };
    for build in each(&mut site, &mut builds) {
        if placed.0.get(&build.root).map(|(revision, _)| *revision) == Some(build.revision) {
            continue;
        }
        let rooms = build.base.indoors();
        let unchanged = placed
            .0
            .get(&build.root)
            .is_some_and(|(_, before)| *before == rooms);
        placed.0.insert(build.root, (build.revision, rooms.clone()));
        if unchanged {
            continue;
        }
        let boxes: Vec<(Vec3, Vec3)> = build
            .base
            .indoors_m()
            .into_iter()
            .map(|(low, high)| {
                (
                    low - Vec3::splat(INTO_WALLS_M),
                    high + Vec3::splat(INTO_WALLS_M),
                )
            })
            .collect();
        let wanted: Vec<[i32; 6]> = boxes.iter().map(|&(min, max)| key(min, max)).collect();
        let mut kept = Vec::new();
        for (_, parent, mut probe) in &mut probes {
            if parent.parent() != build.root {
                continue;
            }
            probe.going = !wanted.contains(&probe.room);
            if !probe.going {
                kept.push(probe.room);
            }
        }
        for (&(min, max), room) in boxes.iter().zip(wanted) {
            if kept.contains(&room) {
                continue;
            }
            commands.spawn((
                Name::new("Indoors"),
                Indoors {
                    room,
                    share: 0.0,
                    going: false,
                },
                LightProbe {
                    falloff: Vec3::splat(FALLOFF),
                },
                lit(&sky, 0.0),
                Transform::from_translation((min + max) * 0.5).with_scale(max - min),
                Visibility::default(),
                ChildOf(build.root),
            ));
        }
    }
}

/// How far a probe has faded after a frame so long, toward in or out.
fn faded(share: f32, going: bool, seconds: f32) -> f32 {
    let step = seconds / FADE_S;
    if going {
        (share - step).max(0.0)
    } else {
        (share + step).min(1.0)
    }
}

/// The sky's light inside a room faded in so far, eased in and out: as outside at 0, dimmed
/// at 1.
fn dimming(share: f32) -> f32 {
    let eased = share * share * (3.0 - 2.0 * share);
    1.0 + (INDOORS - 1.0) * eased
}

/// The sky's light as it reaches inside a room faded in so far: the camera's, at the
/// brightness the sky gives it now, dimmed.
fn lit(light: &EnvironmentMapLight, share: f32) -> EnvironmentMapLight {
    EnvironmentMapLight {
        intensity: light.intensity * dimming(share),
        ..light.clone()
    }
}

/// Keeps every room's light the sky's, dimmed as far as it has faded in, as the day turns;
/// a probe whose room has gone is taken away once faded out.
fn follow(
    mut commands: Commands,
    time: Res<Time>,
    camera: Query<&EnvironmentMapLight, With<MainCamera>>,
    mut probes: Query<(Entity, &mut Indoors, &mut EnvironmentMapLight), Without<MainCamera>>,
) {
    let Some(sky) = camera.iter().next() else {
        return;
    };
    for (entity, mut probe, mut light) in &mut probes {
        let share = faded(probe.share, probe.going, time.delta_secs());
        if share != probe.share {
            probe.share = share;
        }
        if probe.going && share == 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let now = lit(sky, share);
        if light.intensity != now.intensity
            || light.diffuse_map != now.diffuse_map
            || light.specular_map != now.specular_map
        {
            *light = now;
        }
    }
}

/// Says whether the camera is in a room closed and covered, so the eye can open further
/// there (`terra_world::scene::Indoors`).
fn inside(
    camera: Query<&GlobalTransform, With<MainCamera>>,
    rooms: Query<(&GlobalTransform, &Indoors), Without<MainCamera>>,
    mut indoors: ResMut<terra_world::scene::Indoors>,
) {
    let Some(eye) = camera.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    // Each room's probe spans it, a unit box stretched over it.
    let within = rooms.iter().any(|(room, probe)| {
        if probe.going {
            return false;
        }
        let local = room.affine().inverse().transform_point3(eye);
        local.abs().cmple(Vec3::splat(0.5)).all()
    });
    if indoors.0 != within {
        indoors.0 = within;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_closing_dims_its_light_a_little_each_frame_not_at_once() {
        // At sixty frames a second, from outside's light to the room's and back.
        let frame = 1.0 / 60.0;
        let mut share = 0.0;
        let mut biggest: f32 = 0.0;
        let mut frames = 0;
        while share < 1.0 {
            let next = faded(share, false, frame);
            biggest = biggest.max((dimming(next) - dimming(share)).abs());
            share = next;
            frames += 1;
        }
        assert!((dimming(1.0) - INDOORS).abs() < 1e-6 && dimming(0.0) == 1.0);
        // Two fifths of the light over most of a second, never more than a fiftieth of
        // it in a frame.
        assert!(frames > 30, "{frames} frames");
        assert!(biggest < 0.02, "{biggest} in a frame");
        while share > 0.0 {
            share = faded(share, true, frame);
        }
        assert_eq!(dimming(share), 1.0);
    }
}
