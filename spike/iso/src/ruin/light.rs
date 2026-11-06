//! The night: a cold moon low from the north-west, casting long soft shadows off the walls,
//! and a faint blue sky's fill, low enough that the shade goes truly black (the ARPGs' night
//! frames: the darkest twentieth of the frame under 0.001 of white, refs/stats.json).

use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;

/// The moon's light on the ground, in lux, and the sky's fill, in nits.
pub const MOON_LUX: f32 = 190.0;
pub const SKY_NITS: f32 = 1.5;

/// The court's ground fog: thin, dark and ankle-low, so it hangs in the shade and over the
/// water without lifting the floor's mid-tones to grey (judge r3: "fog lifting mid-tones").
pub fn fog() -> super::ambient::fog::FogLook {
    super::ambient::fog::FogLook {
        thick: 0.2,
        colour: Vec3::new(0.3, 0.34, 0.42),
        height: (0.05, 0.28),
    }
}

pub fn spawn(mut commands: Commands) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.45, 0.55, 1.0),
        brightness: SKY_NITS,
        ..default()
    });
    // From the north-west (behind the back wall, to the left), 35 degrees up.
    let (az, el) = (135.0f32.to_radians(), 35.0f32.to_radians());
    let from = Vec3::new(-az.sin() * el.cos(), el.sin(), -az.cos().abs() * el.cos());
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.45, 0.6, 1.0),
            illuminance: MOON_LUX,
            shadow_maps_enabled: true,
            soft_shadow_size: Some(1.0),
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 2,
            minimum_distance: 1.0,
            first_cascade_far_bound: 30.0,
            maximum_distance: 60.0,
            overlap_proportion: 0.2,
        }
        .build(),
        Transform::from_translation(from).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
