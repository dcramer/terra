//! Moonrise over the marsh: a cold moon up behind the fight to the north-west keys the
//! hummocks and rims every body against the warm fire and lantern in front of them, the
//! water mirrors it as a sheen over the frame's top, and a cold blue sky's fill keeps the
//! shade under the reeds and trees near black, so the fire, the lantern and the wisps make
//! the warm and cold pools (vr-pond-slash: p5 0.0036, p50 0.038, p95 0.184).

use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;

/// The moon on the ground, in lux, and the sky's fill, in nits.
pub const MOON_LUX: f32 = 1100.0;
pub const SKY_NITS: f32 = 22.0;
/// Toward the moon: 46 degrees up behind the fight, just left of the camera's line, so its
/// mirror falls on the far channel a quarter down the frame and its lane runs down the water
/// toward the eye (`light::tests`; at 34 degrees it was over the frame's top and the water
/// read flat, blind-marsh6).
pub const MOON: Vec3 = Vec3::new(-0.322, 0.724, -0.611);
/// Its light's colour (linear rgb, the key's and the lane's: sRGB 0.6, 0.72, 1).
pub const MOON_COLOUR: Vec3 = Vec3::new(0.319, 0.477, 1.0);

/// The marsh's ground fog: thin, cold and low, so it does not grey the deep blue water and
/// the fight (the default's grey cards cut the frame's saturation, iso-art j24).
pub fn fog() -> crate::ruin::ambient::fog::FogLook {
    crate::ruin::ambient::fog::FogLook {
        thick: 0.22,
        colour: Vec3::new(0.2, 0.3, 0.5),
        height: (0.05, 0.35),
    }
}
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
            color: Color::linear_rgb(MOON_COLOUR.x, MOON_COLOUR.y, MOON_COLOUR.z),
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

    /// The moon's mirror on a flat face (the middle of its lane) falls in the frame's upper
    /// half on the far channel's open water, so the lane runs down water in frame.
    #[test]
    fn the_moons_lane_lies_on_the_far_channel_in_frame() {
        let (p, y) = (PITCH.to_radians(), YAW.to_radians());
        // From the eye toward the focus, and the camera's right and up.
        let f = -Vec3::new(p.cos() * y.sin(), p.sin(), p.cos() * y.cos());
        let right = f.cross(Vec3::Y).normalize();
        let up = right.cross(f);
        let t = (FOV.to_radians() / 2.0).tan();
        // A flat face's mirror of the view ray through the frame at (sx, sy), -1..1.
        let ray = |sx: f32, sy: f32| (f + right * sx * t * 16.0 / 9.0 + up * sy * t).normalize();
        let mirror = |d: Vec3| (d - 2.0 * d.dot(Vec3::Y) * Vec3::Y).dot(MOON);
        assert!(
            (MOON.length() - 1.0).abs() < 0.01,
            "MOON is not a unit vector"
        );
        // The brightest point of the frame's mirror, on a grid.
        let (mut best, mut at) = (-1.0, (0.0, 0.0));
        for i in -20..=20 {
            for j in -20..=20 {
                let (sx, sy) = (i as f32 / 20.0, j as f32 / 20.0);
                let m = mirror(ray(sx, sy));
                if m > best {
                    (best, at) = (m, (sx, sy));
                }
            }
        }
        assert!(
            best > 0.999,
            "the moon's mirror is out of frame (best {best} at {at:?})"
        );
        assert!(
            at.0.abs() < 0.6 && at.1 > 0.2,
            "the lane's head is at {at:?}"
        );
        // Where that ray meets the water's face, the water is open.
        let mid = (crate::marsh::KNIGHT * 2.0 + crate::marsh::FOE + crate::marsh::TARGET) / 4.0;
        let focus = mid
            .extend(super::super::land::shape(mid).max(0.0) + 1.2)
            .xzy();
        let eye = focus + crate::ruin::camera::offset();
        let d = ray(at.0, at.1);
        let hit = eye + d * (-eye.y / d.y);
        let ground = super::super::land::shape(hit.xz());
        assert!(ground < -0.2, "the lane's head at {hit} is {ground} deep");
    }
}
