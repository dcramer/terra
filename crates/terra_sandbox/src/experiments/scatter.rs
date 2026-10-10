//! Things on the ground: rocks placed from a seed, which can be harvested.
//!
//! Each rock is identified by its index in the seed's sequence, so "which rock" is just a
//! number, and removing one is an operation that could be sent to another player or
//! saved. Nothing about a rock is stored until it is removed.

use crate::planet::Planet;
use bevy::prelude::*;
use terra_core::{Ground, Harvested, KindId, ObjectId, Operation, Yield};
use terra_gen::Seed;

const ROCKS: u64 = 2000;
/// Rocks are spread within this distance of the start, in metres.
const SPREAD_M: f32 = 150.0;
/// How far away the ground can be and still be pointed at.
const REACH_M: f32 = 400.0;
/// How close a rock must be to the point looked at to count as the one meant.
const GRAB_M: f32 = 10.0;
const ROCK: KindId = KindId(0);
/// How far things sink into the ground, as a share of their height.
const BEDDING: f32 = 0.35;

#[derive(Component)]
struct Rock(ObjectId);

#[derive(Component)]
struct Readout;

/// What has been taken, and the operations that took it.
#[derive(Resource, Default)]
struct HarvestLog {
    operations: Vec<Operation>,
    total: Yield,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<HarvestLog>()
        .add_systems(Startup, spawn)
        .add_systems(Update, (harvest, update_readout).chain());
}

pub(super) struct Placement {
    pub id: ObjectId,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

/// Where `count` things go, on the ground around the start. The same seed always gives
/// the same places.
pub(super) fn placements(planet: &Planet, seed: Seed, count: u64) -> Vec<Placement> {
    let start_up = planet.start().normalize();
    let east = start_up.cross(Vec3::X).normalize();
    let north = east.cross(start_up);
    let radius_m = planet.ground.spec().radius_m;
    (0..count)
        .map(|index| {
            let mut rng = seed.index(index).rng();
            // A point in a disc, by trying points in a square until one lands inside.
            let offset = loop {
                let (x, y) = (rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
                if x * x + y * y <= 1.0 {
                    break (east * x + north * y) * SPREAD_M;
                }
            };
            let on_ground = planet.surface((start_up * radius_m + offset).normalize());
            let up = planet.ground.normal(on_ground);
            let turn = Quat::from_axis_angle(Vec3::Y, rng.range(0.0, std::f32::consts::TAU));
            let scale = Vec3::new(
                rng.range(0.4, 1.6),
                rng.range(0.3, 0.9),
                rng.range(0.4, 1.6),
            );
            Placement {
                id: ObjectId(index),
                // Bedded a little into the ground rather than balanced on top of it.
                position: on_ground - up * scale.y * BEDDING,
                rotation: Quat::from_rotation_arc(Vec3::Y, up) * turn,
                scale,
            }
        })
        .collect()
}

fn spawn(
    mut commands: Commands,
    planet: Res<Planet>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // One mesh and one material shared by every rock, so the renderer can batch them.
    let mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.36, 0.34, 0.33),
        perceptual_roughness: 0.9,
        ..default()
    });
    let seed = planet.ground.spec().seed.derive("scatter rocks");
    for rock in placements(&planet, seed, ROCKS) {
        commands.spawn((
            Rock(rock.id),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform {
                translation: rock.position,
                rotation: rock.rotation,
                scale: rock.scale,
            },
        ));
    }
    commands.spawn((
        Readout,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(12),
            ..default()
        },
    ));
}

fn harvest(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    planet: Res<Planet>,
    rocks: Query<(Entity, &Rock, &Transform)>,
    mut log: ResMut<HarvestLog>,
) {
    let (camera, camera_transform) = *camera;
    // F aims with the middle of the view; a click aims with the cursor.
    let ray = if keys.just_pressed(KeyCode::KeyF) {
        Some(Ray3d::new(
            camera_transform.translation(),
            camera_transform.forward(),
        ))
    } else if mouse.just_pressed(MouseButton::Left) {
        window
            .cursor_position()
            .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor).ok())
    } else {
        None
    };
    let Some(ray) = ray else { return };
    let Some(hit) = planet.ground.raycast(ray.origin, *ray.direction, REACH_M) else {
        info!("Nothing in reach.");
        return;
    };
    let candidates = rocks
        .iter()
        .map(|(entity, rock, transform)| ((entity, rock.0), transform.translation));
    let Some((entity, id)) = nearest_within(hit.point, candidates, GRAB_M) else {
        info!("No rock near where you are pointing.");
        return;
    };
    commands.entity(entity).despawn();
    log.operations.push(Operation::RemoveObject { id });
    log.total.add(Harvested::Object(ROCK), 1.0);
    info!(
        "Harvested rock {}. Total: {}.",
        id.0,
        log.total.amount(Harvested::Object(ROCK))
    );
}

fn update_readout(
    log: Res<HarvestLog>,
    rocks: Query<(), With<Rock>>,
    mut readout: Single<&mut Text, With<Readout>>,
) {
    readout.0 = format!(
        "Rocks: {} left, {} harvested ({} operations)\nLeft click or F to harvest",
        rocks.iter().count(),
        log.total.amount(Harvested::Object(ROCK)),
        log.operations.len()
    );
}

/// The item closest to a point, if any is within `radius`.
fn nearest_within<T>(
    point: Vec3,
    items: impl Iterator<Item = (T, Vec3)>,
    radius: f32,
) -> Option<T> {
    items
        .map(|(item, position)| (item, position.distance_squared(point)))
        .filter(|(_, distance_squared)| *distance_squared <= radius * radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(item, _)| item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terra_core::{PlanetSpec, StandInPlanet};

    fn planet() -> Planet {
        let spec = PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        };
        Planet {
            ground: Arc::new(StandInPlanet::new(spec, 150.0, None)),
        }
    }

    #[test]
    fn rocks_sit_on_the_ground_near_the_start_and_repeat() {
        let planet = planet();
        let seed = Seed(1).derive("scatter rocks");
        let rocks = placements(&planet, seed, ROCKS);
        assert_eq!(rocks.len(), ROCKS as usize);
        for rock in &rocks {
            // On the ground, bedded in by no more than their own height.
            let depth = -planet.ground.distance(rock.position);
            assert!((0.0..=rock.scale.y).contains(&depth), "depth {depth}");
            assert!(rock.position.distance(planet.start()) < SPREAD_M * 1.2);
            assert!((rock.rotation.length() - 1.0).abs() < 1e-4);
        }
        let again = placements(&planet, seed, ROCKS);
        assert!(
            rocks
                .iter()
                .zip(&again)
                .all(|(a, b)| a.position == b.position && a.id == b.id)
        );
        let other = placements(&planet, Seed(2).derive("scatter rocks"), ROCKS);
        assert!(
            rocks
                .iter()
                .zip(&other)
                .any(|(a, b)| a.position != b.position)
        );
    }

    #[test]
    fn picks_the_nearest_item_in_range() {
        let items = || {
            [
                ("far", Vec3::X * 9.0),
                ("near", Vec3::X * 2.0),
                ("out", Vec3::X * 30.0),
            ]
            .into_iter()
        };
        assert_eq!(nearest_within(Vec3::ZERO, items(), 10.0), Some("near"));
        assert_eq!(nearest_within(Vec3::ZERO, items(), 1.0), None);
        assert_eq!(nearest_within(Vec3::X * 29.0, items(), 10.0), Some("out"));
    }
}
