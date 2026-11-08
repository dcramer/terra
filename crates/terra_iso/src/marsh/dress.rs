//! What stands in the marsh: dead trees in the water at the frame's edges, mossy rocks,
//! stumps and logs half sunk along the banks, a camp fire on the knight's bank (its flames
//! the ruin's flipbook, found by its `Halo` card) and a lantern on a post in the mere,
//! both flickering and mirrored in the water (`kit::water::Glint`); the lantern hangs from a
//! crossarm on a round pole, a crisp flame in its lit glass.

use super::land::shape;
use crate::kit::water::Glint;
use crate::motes::{Halo, soft_image};
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
/// The pole's length (sunk `SUNK` into the mud), its crossarm's way (toward the screen's
/// left) and reach, and how far under the arm the lamp's foot hangs (Lantern.glb is 0.53 m
/// tall, its origin at its foot, its glass from 0.06 to 0.30 m up).
const POST: f32 = 2.2;
const SUNK: f32 = 0.2;
const ARM: Vec3 = Vec3::new(-0.94, 0.0, 0.342);
const ARM_LONG: f32 = 0.6;
const HANG: f32 = 0.6;
const LANTERN_LM: f32 = 20_000.0;

/// Where the lamp's foot hangs: under the crossarm's end.
fn lamp() -> Vec3 {
    let foot = LANTERN.extend(shape(LANTERN) - SUNK).xzy();
    foot + Vec3::Y * (POST - 0.1 - HANG) + ARM * ARM_LONG
}

/// Model, place, yaw (degrees), scale, how far sunk (m).
pub const PROPS: [(&str, Vec2, f32, f32, f32); 20] = [
    // Stones at the path's edges and standing in the shallows, breaking the water's line.
    (
        "models/nature/forest/mossy_rock_b_2.glb",
        Vec2::new(3.3, -1.7),
        60.0,
        0.2,
        0.04,
    ),
    (
        "models/nature/forest/mossy_rock_a_3.glb",
        Vec2::new(-1.8, -1.7),
        200.0,
        0.16,
        0.03,
    ),
    (
        "models/nature/forest/mossy_rock_b_4.glb",
        Vec2::new(-7.0, -2.8),
        120.0,
        0.25,
        0.06,
    ),
    (
        "models/nature/forest/mossy_rock_a_4.glb",
        Vec2::new(-0.6, 2.3),
        15.0,
        0.22,
        0.08,
    ),
    (
        "models/nature/forest/mossy_rock_b_5.glb",
        Vec2::new(2.6, 2.9),
        300.0,
        0.28,
        0.1,
    ),
    (
        "models/nature/forest/mossy_rock_b_1.glb",
        Vec2::new(5.0, 1.8),
        80.0,
        0.24,
        0.12,
    ),
    (
        "models/iso/dead-tree-a.glb",
        Vec2::new(-7.6, 2.9),
        20.0,
        1.0,
        0.3,
    ),
    (
        "models/iso/dead-tree-b.glb",
        Vec2::new(8.6, -4.8),
        200.0,
        1.0,
        0.3,
    ),
    (
        "models/iso/dead-tree-a.glb",
        Vec2::new(-5.4, -7.0),
        110.0,
        0.8,
        0.3,
    ),
    (
        "models/iso/dead-tree-b.glb",
        Vec2::new(9.8, 3.6),
        290.0,
        0.9,
        0.3,
    ),
    // The scanned rocks are boulders 2 to 3 m across at scale 1: stones here.
    (
        "models/nature/forest/mossy_rock_a_2.glb",
        Vec2::new(-3.6, 0.9),
        30.0,
        0.3,
        0.05,
    ),
    (
        "models/nature/forest/mossy_rock_b_3.glb",
        Vec2::new(4.4, 0.2),
        140.0,
        0.35,
        0.05,
    ),
    (
        "models/nature/forest/mossy_rock_a_5.glb",
        Vec2::new(-6.2, -2.4),
        250.0,
        0.45,
        0.1,
    ),
    (
        "models/nature/forest/mossy_rock_b_6.glb",
        Vec2::new(0.9, 1.9),
        75.0,
        0.22,
        0.03,
    ),
    (
        "models/nature/forest/mossy_rock_a_1.glb",
        Vec2::new(5.6, -6.6),
        10.0,
        0.5,
        0.1,
    ),
    (
        "models/nature/forest/stump.glb",
        Vec2::new(-4.8, -0.6),
        0.0,
        1.0,
        0.05,
    ),
    (
        "models/nature/forest/stump.glb",
        Vec2::new(6.2, -2.8),
        130.0,
        0.8,
        0.05,
    ),
    (
        "models/props/Log.glb",
        Vec2::new(-2.4, 2.6),
        70.0,
        1.0,
        0.12,
    ),
    (
        "models/props/Log.glb",
        Vec2::new(3.6, 2.4),
        160.0,
        1.0,
        0.18,
    ),
    (
        "models/props/Log.glb",
        Vec2::new(-8.4, -4.2),
        35.0,
        1.0,
        0.12,
    ),
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
    model(
        &mut commands,
        &asset,
        "models/props/Campfire.glb",
        bed,
        0.4,
        0.7,
    );
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
    // The lantern on its pole: a round post of rough wood with a crossarm toward the
    // screen's left, the lamp hung from the arm's end, a crisp flame in its lit glass (a
    // 0.7 m soft glow card here read as "a bloom ball with no readable post", "a sprite").
    let foot = LANTERN.extend(shape(LANTERN) - SUNK).xzy();
    let wood = materials.add(crate::scene::material(
        &asset,
        "rough_wood",
        Color::srgb(0.55, 0.45, 0.36),
    ));
    let mut rod = |r: f32, long: f32| {
        meshes.add(
            Mesh::from(Cylinder::new(r, long))
                .with_generated_tangents()
                .expect("a cylinder has normals and UVs"),
        )
    };
    let (pole, arm) = (rod(0.06, POST), rod(0.035, ARM_LONG + 0.1));
    commands.spawn((
        Mesh3d(pole),
        MeshMaterial3d(wood.clone()),
        Transform::from_translation(foot + Vec3::Y * POST * 0.5),
    ));
    let top = foot + Vec3::Y * (POST - 0.1);
    commands.spawn((
        Mesh3d(arm),
        MeshMaterial3d(wood),
        Transform::from_translation(top + ARM * ARM_LONG * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, ARM)),
    ));
    let lamp = lamp();
    model(
        &mut commands,
        &asset,
        "models/props/Lantern.glb",
        lamp,
        0.3,
        1.0,
    );
    // The lit housing inside the glass, and the flame just before it toward the eye (no
    // `Halo`: that would light a flipbook fire).
    let eye = crate::ruin::camera::offset().normalize();
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.15, 0.2, 0.15))),
        MeshMaterial3d(materials.add(glow(LinearRgba::rgb(2.2, 1.0, 0.3), None))),
        Transform::from_translation(lamp + Vec3::Y * 0.18),
        bevy::light::NotShadowCaster,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(materials.add(glow(LinearRgba::rgb(12.0, 6.0, 1.8), None))),
        Transform::from_translation(lamp + Vec3::Y * 0.17 + eye * 0.08)
            .with_scale(Vec3::new(0.025, 0.06, 0.025)),
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
        Transform::from_translation(lamp + Vec3::Y * 0.17 + eye * 0.1),
        Flicker {
            lm: LANTERN_LM,
            seed: 7.0,
        },
        Glint,
    ));
}

/// The flames' light breathing: two slow waves and a quick one, never under 70%.
fn flicker(clock: Res<Clock>, mut lights: Query<(&Flicker, &mut PointLight)>) {
    for (f, mut l) in &mut lights {
        let t = clock.t + f.seed;
        let k = 0.85
            + 0.07 * libm::sinf(t * 7.3)
            + 0.05 * libm::sinf(t * 13.1 + 1.7)
            + 0.03 * libm::sinf(t * 29.0 + 0.4);
        l.intensity = f.lm * k;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marsh::LEVEL;

    /// The fire burns on dry ground within a metre of the water (so it glints in it), and
    /// the lantern's post stands in the shallows.
    #[test]
    fn the_fire_is_dry_by_the_water_and_the_post_wet() {
        assert!(
            shape(FIRE) > LEVEL + 0.03,
            "the fire is wet: {}",
            shape(FIRE)
        );
        let near = (1..=10).any(|i| shape(FIRE + Vec2::new(0.0, i as f32 * 0.1)) < LEVEL);
        assert!(
            near,
            "no water within a metre toward the camera of the fire"
        );
        let d = shape(LANTERN) - LEVEL;
        assert!(
            (-0.5..-0.05).contains(&d),
            "the post's foot is {d} under the face"
        );
        // Its mirror image, 0.7 of the flame's height toward the camera from under the lamp
        // (hung from the crossarm), is on open water.
        let eye = crate::ruin::camera::offset();
        let flame = lamp() + Vec3::Y * 0.17;
        let under = Vec2::new(flame.x, flame.z);
        let image = under + Vec2::new(eye.x, eye.z).normalize() * 0.7 * (flame.y - LEVEL);
        assert!(
            shape(image) < LEVEL - 0.1,
            "the lantern glints on the bank at {image}"
        );
    }

    /// The lamp hangs clear of the pole and the water: under the crossarm's end, a reach
    /// from the pole, its foot well over the face.
    #[test]
    fn the_lamp_hangs_from_the_crossarm_over_the_water() {
        let l = lamp();
        let reach = Vec2::new(l.x, l.z).distance(LANTERN);
        assert!(
            reach > 0.4,
            "the lamp hangs {reach} m from the pole, in its post"
        );
        assert!(
            l.y - LEVEL > 0.8,
            "the lamp's foot is {} m over the water",
            l.y - LEVEL
        );
        let top = shape(LANTERN) - SUNK + POST;
        assert!(l.y + 0.53 < top, "the lamp stands over the pole's top");
    }
}
