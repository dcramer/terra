//! Moonrise over the marsh: the moon, the sky's fill, the exposure and the far haze are the
//! biome's light preset (biomes/light/marsh-dusk.ron, kit/light.rs, with why each number);
//! here, the marsh's ground fog, and the test that keeps the moon's lane on the water.

use bevy::prelude::*;

/// The marsh's ground fog: thin, cold and low, so it does not grey the deep blue water and
/// the fight (the default's grey cards cut the frame's saturation, iso-art j24).
pub fn fog() -> crate::ruin::ambient::fog::FogLook {
    crate::ruin::ambient::fog::FogLook {
        thick: 0.22,
        colour: Vec3::new(0.2, 0.3, 0.5),
        height: (0.05, 0.35),
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn);
}

fn spawn(mut commands: Commands, light: Res<crate::kit::light::Light>) {
    light.spawn(&mut commands);
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
        let f = -Vec3::new(
            libm::cosf(p) * libm::sinf(y),
            libm::sinf(p),
            libm::cosf(p) * libm::cosf(y),
        );
        let right = f.cross(Vec3::Y).normalize();
        let up = right.cross(f);
        let t = libm::tanf(FOV.to_radians() / 2.0);
        // A flat face's mirror of the view ray through the frame at (sx, sy), -1..1.
        let ray = |sx: f32, sy: f32| (f + right * sx * t * 16.0 / 9.0 + up * sy * t).normalize();
        // Toward the moon, as biomes/marsh.ron lights it.
        let biome = crate::kit::biome::Biome::load("marsh").unwrap();
        let moon = biome.light().unwrap().key.toward.vector();
        let mirror = |d: Vec3| (d - 2.0 * d.dot(Vec3::Y) * Vec3::Y).dot(moon);
        assert!(
            (moon.length() - 1.0).abs() < 0.01,
            "the marsh's key light's toward is not a unit vector"
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
            at.0.abs() < 0.8 && at.1 > 0.2 && at.1 < 0.9,
            "the lane's head is at {at:?}"
        );
        let mid = (crate::marsh::KNIGHT * 2.0 + crate::marsh::FOE + crate::marsh::TARGET) / 4.0;
        let focus = mid
            .extend(super::super::land::shape(mid).max(0.0) + 1.2)
            .xzy();
        let eye = focus + crate::ruin::camera::offset();
        // Clear of the fight: a fifth of the frame or more from where the caster, the
        // knight and the foe stand on screen (behind the caster its glare ate the bolts).
        for c in [
            crate::marsh::TARGET,
            crate::marsh::KNIGHT,
            crate::marsh::FOE,
        ] {
            let d = c.extend(1.0).xzy() - eye;
            let z = d.dot(f);
            let (fx, fy) = (d.dot(right) / z / t / (16.0 / 9.0), d.dot(up) / z / t);
            let gap = Vec2::new((at.0 - fx) * 16.0 / 9.0, at.1 - fy).length();
            assert!(gap > 0.4, "the lane's head {at:?} is {gap} from {c}");
        }
        // Where that ray meets the water's face, the water is open.
        let d = ray(at.0, at.1);
        let hit = eye + d * (-eye.y / d.y);
        let ground = super::super::land::shape(hit.xz());
        assert!(ground < -0.2, "the lane's head at {hit} is {ground} deep");
    }
}
