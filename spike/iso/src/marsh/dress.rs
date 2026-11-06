//! What stands in the marsh: dead trees in the water at the frame's edges, mossy rocks,
//! stumps and logs half sunk along the banks, a camp fire on the knight's bank (its flames
//! the ruin's flipbook, found by its `Halo` card) and a lantern on a post in the mere,
//! both flickering and mirrored in the water (`pool::Glint`).

use super::land::shape;
use crate::motes::{Halo, soft_image};
use crate::pool::Glint;
use crate::ruin::effects::kit::{Clock, glow};
use crate::scene::model;
use bevy::prelude::*;

/// The camp fire's bed, its size (m) and light (lumens, range m).
/// On the near channel's bank, so its mirror image (about 0.4 m toward the camera at this
/// pitch) falls on the water: a fire further up the bank glints on mud.
pub const FIRE: Vec2 = Vec2::new(-1.8, 1.35);
const FIRE_SIZE: f32 = 0.55;
const FIRE_LM: f32 = 90_000.0;
/// The lantern's post, standing in the mere's shallows on its far side (so its mirror image
/// falls 1.3 m toward the camera into open water, and the lamp stands well inside the frame:
/// in the near channel it sat on the frame's bottom edge), and its light.
pub const LANTERN: Vec2 = Vec2::new(6.0, -1.0);
const POST: f32 = 1.8;
const LANTERN_LM: f32 = 20_000.0;

/// Model, place, yaw (degrees), scale, how far sunk (m).
const PROPS: [(&str, Vec2, f32, f32, f32); 14] = [
    ("models/iso/dead-tree-a.glb", Vec2::new(-7.6, 2.9), 20.0, 1.0, 0.3),
    ("models/iso/dead-tree-b.glb", Vec2::new(8.6, -4.8), 200.0, 1.0, 0.3),
    ("models/iso/dead-tree-a.glb", Vec2::new(-5.4, -7.0), 110.0, 0.8, 0.3),
    ("models/iso/dead-tree-b.glb", Vec2::new(9.8, 3.6), 290.0, 0.9, 0.3),
    // The scanned rocks are boulders 2 to 3 m across at scale 1: stones here.
    ("models/nature/forest/mossy_rock_a_2.glb", Vec2::new(-3.6, 0.9), 30.0, 0.3, 0.05),
    ("models/nature/forest/mossy_rock_b_3.glb", Vec2::new(4.4, 0.2), 140.0, 0.35, 0.05),
    ("models/nature/forest/mossy_rock_a_5.glb", Vec2::new(-6.2, -2.4), 250.0, 0.45, 0.1),
    ("models/nature/forest/mossy_rock_b_6.glb", Vec2::new(0.9, 1.9), 75.0, 0.22, 0.03),
    ("models/nature/forest/mossy_rock_a_1.glb", Vec2::new(5.6, -6.6), 10.0, 0.5, 0.1),
    ("models/nature/forest/stump.glb", Vec2::new(-4.8, -0.6), 0.0, 1.0, 0.05),
    ("models/nature/forest/stump.glb", Vec2::new(6.2, -2.8), 130.0, 0.8, 0.05),
    ("models/props/Log.glb", Vec2::new(-2.4, 2.6), 70.0, 1.0, 0.12),
    ("models/props/Log.glb", Vec2::new(3.6, 2.4), 160.0, 1.0, 0.18),
    ("models/props/Log.glb", Vec2::new(-8.4, -4.2), 35.0, 1.0, 0.12),
];

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, flicker);
}

/// A light that flickers about its lumens.
#[derive(Component)]
struct Flicker {
    lm: f32,
    seed: f32,
}

fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    for (path, p, yaw, scale, sunk) in PROPS {
        model(
            &mut commands,
            &asset,
            path,
            p.extend(shape(p) - sunk).xzy(),
            yaw.to_radians(),
            scale,
        );
    }
    // The camp fire: its ring and logs, the halo the flipbook finds, its light.
    let bed = FIRE.extend(shape(FIRE)).xzy();
    model(&mut commands, &asset, "models/props/Campfire.glb", bed, 0.4, 0.7);
    let soft = images.add(soft_image());
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    commands.spawn((
        Mesh3d(card),
        MeshMaterial3d(materials.add(glow(LinearRgba::rgb(0.16, 0.06, 0.012), Some(soft)))),
        Transform::from_translation(bed + Vec3::Y * FIRE_SIZE * 0.35)
            .with_scale(Vec3::splat(FIRE_SIZE * 2.6)),
        bevy::light::NotShadowCaster,
        Halo,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.55, 0.22),
            intensity: FIRE_LM,
            range: 9.0,
            radius: 0.2,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_translation(bed + Vec3::Y * 0.6),
        Flicker {
            lm: FIRE_LM,
            seed: 1.0,
        },
        Glint,
    ));
    // The lantern on its post: a dark stake, the lamp hung at its top, a warm glow card.
    let foot = LANTERN.extend(shape(LANTERN)).xzy();
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.09, POST, 0.09))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.07, 0.05, 0.035),
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::from_translation(foot + Vec3::Y * (POST * 0.5 - 0.2))
            .with_rotation(Quat::from_rotation_z(0.06)),
    ));
    let lamp = foot + Vec3::Y * (POST - 0.2);
    model(&mut commands, &asset, "models/props/Lantern.glb", lamp, 0.3, 1.0);
    // Its flame, a hot core over the bloom (no `Halo`: that would light a flipbook fire).
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.05))),
        MeshMaterial3d(materials.add(glow(LinearRgba::rgb(9.0, 4.5, 1.4), None))),
        Transform::from_translation(lamp + Vec3::Y * 0.22),
        bevy::light::NotShadowCaster,
    ));
    // The glass's glow, a soft card turned to the fixed camera and set a little toward it so
    // the lamp's frame does not hide it (the flame alone reads as a dark box at 960 px).
    let eye = crate::ruin::camera::offset().normalize();
    let at = lamp + Vec3::Y * 0.2 + eye * 0.25;
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(0.7, 0.7))),
        MeshMaterial3d(materials.add(glow(
            LinearRgba::rgb(1.6, 0.75, 0.25),
            Some(images.add(soft_image())),
        ))),
        Transform::from_translation(at).looking_at(at - eye, Vec3::Y),
        bevy::light::NotShadowCaster,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.62, 0.3),
            intensity: LANTERN_LM,
            range: 6.0,
            radius: 0.08,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(lamp + Vec3::Y * 0.25),
        Flicker {
            lm: LANTERN_LM,
            seed: 7.0,
        },
        Glint,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marsh::LEVEL;

    /// The fire burns on dry ground within a metre of the water (so it glints in it), and
    /// the lantern's post stands in the shallows.
    #[test]
    fn the_fire_is_dry_by_the_water_and_the_post_wet() {
        assert!(shape(FIRE) > LEVEL + 0.03, "the fire is wet: {}", shape(FIRE));
        let near = (1..=10).any(|i| shape(FIRE + Vec2::new(0.0, i as f32 * 0.1)) < LEVEL);
        assert!(near, "no water within a metre toward the camera of the fire");
        let d = shape(LANTERN) - LEVEL;
        assert!((-0.5..-0.05).contains(&d), "the post's foot is {d} under the face");
        // Its mirror image, 0.7 of the lamp's height toward the camera, is on open water.
        let eye = crate::ruin::camera::offset();
        let image = LANTERN + Vec2::new(eye.x, eye.z).normalize() * 0.7 * POST;
        assert!(shape(image) < LEVEL - 0.1, "the lantern glints on the bank at {image}");
    }
}

/// The flames' light breathing: two slow waves and a quick one, never under 70%.
fn flicker(clock: Res<Clock>, mut lights: Query<(&Flicker, &mut PointLight)>) {
    for (f, mut l) in &mut lights {
        let t = clock.t + f.seed;
        let k = 0.85
            + 0.07 * (t * 7.3).sin()
            + 0.05 * (t * 13.1 + 1.7).sin()
            + 0.03 * (t * 29.0 + 0.4).sin();
        l.intensity = f.lm * k;
    }
}
