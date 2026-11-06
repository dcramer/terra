//! The ruin's floor: flagstones laid in running-bond rows on packed earth, worn and shifted,
//! a few split, more of them gone toward the edges where the earth has taken the floor back.

use super::blocks::Blocks;
use crate::forest::Rng;
use crate::scene::material;
use bevy::math::Affine2;
use bevy::prelude::*;

/// The paved floor's extent, west to east and north to south.
pub const PAVED_X: (f32, f32) = (-9.0, 9.0);
pub const PAVED_Z: (f32, f32) = (-8.0, 6.0);
/// Where the slabs' tops lie, and the earth between them.
pub const TOP: f32 = 0.01;
const EARTH: f32 = -0.03;

pub fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // The earth: a wide plane, its texture repeated every 2 m.
    let mut earth = material(&asset, "packed_earth", Color::srgb(0.62, 0.56, 0.5));
    earth.uv_transform = Affine2::from_scale(Vec2::splat(30.0));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(60.0, 60.0))),
        MeshMaterial3d(materials.add(earth)),
        Transform::from_xyz(0.0, EARTH, 0.0),
    ));

    // Two stones, so the floor is not one quarry's.
    let mut worn = Blocks::default();
    let mut dry = Blocks::default();
    let mut rng = Rng::new(0x5ab_1e5);
    let mut z = PAVED_Z.0;
    let mut row = 0;
    while z < PAVED_Z.1 {
        let depth = rng.range(0.45, 0.75);
        // Running bond: each row starts half a slab along from the last.
        let mut x = PAVED_X.0 - if row % 2 == 0 { 0.0 } else { 0.35 };
        while x < PAVED_X.1 {
            let len = rng.range(0.5, 0.9);
            let mid = Vec2::new(x + len * 0.5, z + depth * 0.5);
            x += len;
            // Missing: a few anywhere, most toward the edges, the edge itself ragged.
            let edge = ((mid.x / PAVED_X.1).abs())
                .max(((mid.y + 1.0) / ((PAVED_Z.1 - PAVED_Z.0) * 0.5)).abs())
                + rng.range(-0.12, 0.12);
            let gone = 0.06 + 0.9 * smooth(0.72, 1.0, edge);
            if rng.f() < gone {
                continue;
            }
            let gap = rng.range(0.02, 0.04);
            let size = Vec3::new(len - gap, 0.12, depth - gap);
            let tilt = Quat::from_euler(
                EulerRot::YXZ,
                rng.range(-0.03, 0.03),
                rng.range(-2.0, 2.0f32).to_radians(),
                rng.range(-2.0, 2.0f32).to_radians(),
            );
            let y = TOP - 0.06 + rng.range(-0.02, 0.02);
            let v = rng.range(0.62, 1.0);
            let tint = if rng.f() < 0.07 {
                // Moss in the shade of a joint.
                Color::srgb(v * 0.78, v * 0.86, v * 0.62)
            } else {
                Color::srgb(v, v * 0.97, v * 0.92)
            };
            let (into, tile) = if rng.f() < 0.7 {
                (&mut worn, 2.0)
            } else {
                (&mut dry, 1.8)
            };
            let shift = Vec2::new(rng.f(), rng.f());
            let at = Vec3::new(mid.x, y, mid.y);
            if rng.f() < 0.1 {
                // Split across: two halves a crack apart, each settled its own way.
                let half = Vec3::new(size.x * 0.5 - 0.01, size.y, size.z);
                for s in [-1.0, 1.0] {
                    let sink = Vec3::Y * rng.range(-0.025, 0.0);
                    let lean = Quat::from_rotation_z(s * rng.range(0.0, 2.5f32).to_radians());
                    let at = at + tilt * Vec3::X * s * (size.x * 0.25 + 0.005) + sink;
                    into.add(at, half, tilt * lean, tile, shift, tint);
                }
            } else {
                into.add(at, size, tilt, tile, shift, tint);
            }
        }
        z += depth;
        row += 1;
    }
    for (blocks, name) in [(worn, "worn_rock_natural_01"), (dry, "rock_boulder_dry")] {
        commands.spawn((
            Mesh3d(meshes.add(blocks.mesh())),
            MeshMaterial3d(materials.add(material(&asset, name, Color::WHITE))),
        ));
    }
}

pub fn smooth(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
