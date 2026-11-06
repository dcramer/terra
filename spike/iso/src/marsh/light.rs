//! Dusk: the sun just down behind the far marsh to the north-west, a low warm light raking
//! the hummocks with long shadows, and a violet sky's fill dim enough that the shade under
//! the reeds and trees goes near black, so the fire, the lantern and the wisps make the
//! pools (vr-pond-slash: p5 0.0036, p50 0.038, p95 0.184).

use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;

/// The low sun on the ground, in lux, its height (degrees) and the sky's fill, in nits.
pub const SUN_LUX: f32 = 350.0;
const SUN_UP: f32 = 6.0;
pub const SKY_NITS: f32 = 3.0;
/// The haze over the far marsh, and its colour toward the sun.
pub const HAZE: Color = Color::srgb(0.10, 0.08, 0.13);
pub const SUN_HAZE: Color = Color::srgb(0.55, 0.28, 0.14);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn);
}

fn spawn(mut commands: Commands) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.55, 0.45, 1.0),
        brightness: SKY_NITS,
        ..default()
    });
    // From the north-west, behind the fight, so the figures stand against the glow.
    let (az, el) = (140.0f32.to_radians(), SUN_UP.to_radians());
    let from = Vec3::new(-az.sin() * el.cos(), el.sin(), -az.cos().abs() * el.cos());
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.55, 0.3),
            illuminance: SUN_LUX,
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
