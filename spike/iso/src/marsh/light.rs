//! Moonrise over the marsh: a cold moon up behind the fight to the north-west keys the
//! hummocks and rims every body against the warm fire and lantern in front of them, the
//! water mirrors it as a sheen over the frame's top, and a cold blue sky's fill keeps the
//! shade under the reeds and trees near black, so the fire, the lantern and the wisps make
//! the warm and cold pools (vr-pond-slash: p5 0.0036, p50 0.038, p95 0.184).

use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;

/// The moon on the ground, in lux, and the sky's fill, in nits.
pub const MOON_LUX: f32 = 700.0;
pub const SKY_NITS: f32 = 22.0;
/// Toward the moon: 40 degrees up, 20 degrees left of the camera's line, so its mirror in
/// the water falls over the frame's upper left and fades down it (`light::tests`).
pub const MOON: Vec3 = Vec3::new(-0.493, 0.643, -0.587);
/// The haze over the far marsh, and its colour toward the moon.
pub const HAZE: Color = Color::srgb(0.05, 0.07, 0.12);
pub const SUN_HAZE: Color = Color::srgb(0.2, 0.26, 0.38);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn);
}

fn spawn(mut commands: Commands) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.35, 0.5, 1.0),
        brightness: SKY_NITS,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.6, 0.72, 1.0),
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
        Transform::from_translation(MOON).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruin::camera::{FOV, PITCH, YAW};

    /// The moon's mirror is brightest on water at the frame's top and dims toward its foot,
    /// left of the middle more than right: where vr-pond-slash's sheen lies.
    #[test]
    fn the_moon_shines_on_the_top_of_the_water() {
        let (p, y) = (PITCH.to_radians(), YAW.to_radians());
        // From the eye toward the focus, and the camera's right and up.
        let f = -Vec3::new(p.cos() * y.sin(), p.sin(), p.cos() * y.cos());
        let right = f.cross(Vec3::Y).normalize();
        let up = right.cross(f);
        let t = (FOV.to_radians() / 2.0).tan();
        // A flat face's mirror of the view ray through the frame at (sx, sy), -1..1.
        let sheen = |sx: f32, sy: f32| {
            let d = (f + right * sx * t * 16.0 / 9.0 + up * sy * t).normalize();
            let r = d - 2.0 * d.dot(Vec3::Y) * Vec3::Y;
            r.dot(MOON.normalize()).max(0.0).powf(8.0)
        };
        assert!((MOON.length() - 1.0).abs() < 0.01, "MOON is not a unit vector");
        let (top, foot) = (sheen(0.0, 0.9), sheen(0.0, -0.9));
        assert!(top > 0.6 && foot < top * 0.5, "top {top}, foot {foot}");
        assert!(sheen(-0.8, 0.5) > sheen(0.8, 0.5), "the sheen leans right");
    }
}
