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

/// A light probe dimming the sky inside a room of a base.
#[derive(Component)]
struct Indoors;

/// The revision of each base its rooms were last looked at for, and the rooms' boxes
/// its probes were placed for.
#[derive(Resource, Default)]
struct Placed(BTreeMap<Entity, (u64, Vec<(IVec3, IVec3)>)>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Placed>()
        .add_systems(Update, (place, follow, inside).chain());
}

/// Places a probe in every room indoors, again whenever a base changes.
fn place(
    mut commands: Commands,
    mut placed: ResMut<Placed>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
    probes: Query<(Entity, &ChildOf), With<Indoors>>,
    camera: Query<&EnvironmentMapLight, With<MainCamera>>,
) {
    // Nothing to dim until the sky lights the camera.
    let Some(sky) = camera.iter().next().map(dimmed) else {
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
        for (entity, parent) in &probes {
            if parent.parent() == build.root {
                commands.entity(entity).try_despawn();
            }
        }
        for (low, high) in build.base.indoors_m() {
            let min = low - Vec3::splat(INTO_WALLS_M);
            let max = high + Vec3::splat(INTO_WALLS_M);
            commands.spawn((
                Name::new("Indoors"),
                Indoors,
                LightProbe {
                    falloff: Vec3::splat(FALLOFF),
                },
                sky.clone(),
                Transform::from_translation((min + max) * 0.5).with_scale(max - min),
                Visibility::default(),
                ChildOf(build.root),
            ));
        }
    }
}

/// The sky's light as it reaches inside: the camera's, at the brightness the sky gives it
/// now, dimmed.
fn dimmed(light: &EnvironmentMapLight) -> EnvironmentMapLight {
    EnvironmentMapLight {
        intensity: light.intensity * INDOORS,
        ..light.clone()
    }
}

/// Keeps every room's light the sky's, dimmed, as the day turns.
fn follow(
    camera: Query<&EnvironmentMapLight, With<MainCamera>>,
    mut probes: Query<&mut EnvironmentMapLight, (With<Indoors>, Without<MainCamera>)>,
) {
    let Some(sky) = camera.iter().next().map(dimmed) else {
        return;
    };
    for mut probe in &mut probes {
        if probe.intensity != sky.intensity
            || probe.diffuse_map != sky.diffuse_map
            || probe.specular_map != sky.specular_map
        {
            *probe = sky.clone();
        }
    }
}

/// Says whether the camera is in a room closed and covered, so the eye can open further
/// there (`terra_world::scene::Indoors`).
fn inside(
    camera: Query<&GlobalTransform, With<MainCamera>>,
    rooms: Query<&GlobalTransform, (With<Indoors>, Without<MainCamera>)>,
    mut indoors: ResMut<terra_world::scene::Indoors>,
) {
    let Some(eye) = camera.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    // Each room's probe spans it, a unit box stretched over it.
    let within = rooms.iter().any(|room| {
        let local = room.affine().inverse().transform_point3(eye);
        local.abs().cmple(Vec3::splat(0.5)).all()
    });
    if indoors.0 != within {
        indoors.0 = within;
    }
}
