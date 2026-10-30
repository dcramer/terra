//! `sandbox.card`: a matte grey card of known reflectance stood in the world, as a
//! photographer holds one up to judge colour and exposure.

use super::{refused, triple};
use bevy::{prelude::*, remote::BrpResult, remote::builtin_methods::parse_some};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Card {
    at: [f32; 3],
    facing: [f32; 3],
    size: Option<f32>,
    albedo: Option<f32>,
}

/// Stands a grey card (`sandbox.card`): a thin slab, so it casts a shadow and is lit only on
/// the face it shows.
pub(super) fn card(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Card {
        at,
        facing,
        size,
        albedo,
    } = parse_some(params)?;
    let start = *world
        .get_resource::<crate::start::Start>()
        .ok_or_else(|| refused("there is no start to stand by"))?;
    let planet = world
        .get_resource::<crate::planet::Planet>()
        .ok_or_else(|| refused("there is no ground to stand on"))?;
    let ground = planet.ground_at(start.transform_point(Vec3::new(at[0], 0.0, at[2])));
    let up = ground.normalize();
    let middle = ground + up * at[1];
    let facing = (start.rotation * Vec3::from_array(facing)).normalize_or_zero();
    if facing == Vec3::ZERO {
        return Err(refused(
            "a card faces some way: `facing` is three numbers, not nothing",
        ));
    }
    // A cuboid's thin side is its z: looking along the reverse of its facing puts +z, its face,
    // toward it. Upright unless it faces up or down.
    let hint = if facing.dot(up).abs() > 0.99 {
        start.rotation * Vec3::Z
    } else {
        up
    };
    let rotation = Transform::IDENTITY.looking_to(-facing, hint).rotation;
    let size = size.unwrap_or(0.5).clamp(0.01, 10.0);
    let albedo = albedo.unwrap_or(0.18).clamp(0.0, 1.0);
    let mesh = world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(size, size, 0.01));
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::linear_rgb(albedo, albedo, albedo),
            perceptual_roughness: 1.0,
            metallic: 0.0,
            ..default()
        });
    let entity = world
        .spawn((
            Name::new(format!("grey card {:.0}%", albedo * 100.0)),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(middle).with_rotation(rotation),
        ))
        .id();
    Ok(json!({ "entity": entity.to_bits(), "at": triple(middle) }))
}
