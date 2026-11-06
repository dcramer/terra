//! Dead grass: tufts of dry blades where the earth has taken the court back, thick in drifts
//! beyond the paving and along its broken edge, a few in the holes, none on whole flags. The
//! blades are the glade's (forest.rs: solid, tapered, their normals leaning up so they shade
//! as the ground does), straw and olive-brown, still: the scene's wind is not this module's.

use super::ground::{paved, smooth};
use crate::forest::Rng;
use crate::terrain::fbm;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// The ground the frame sees, and somewhat beyond.
const AREA_X: (f32, f32) = (-18.0, 20.0);
const AREA_Z: (f32, f32) = (-16.0, 11.0);
/// Tufts tried per square metre; the drift field keeps a share of them.
const TRIES: f32 = 2.2;

/// How much grass grows at `p`: drifts of it on the earth, a thin fringe on the broken
/// paving, none where the flags are whole.
pub fn grassy(p: Vec2) -> f32 {
    let earth = 1.0 - smooth(0.35, 0.75, paved(p));
    let drift = smooth(0.3, 0.6, fbm(p * 0.22, 501));
    // None in the pool, a few stalks at its bank (water.rs).
    let dry = 1.0 - smooth(0.05, 0.25, super::water::pool(p));
    earth * (0.25 + 0.75 * drift) * dry
}

pub fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut pos = Vec::new();
    let mut nor = Vec::new();
    let mut col = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let mut rng = Rng::new(0xdead_6a55);
    let tries = ((AREA_X.1 - AREA_X.0) * (AREA_Z.1 - AREA_Z.0) * TRIES) as usize;
    for _ in 0..tries {
        let at = Vec2::new(rng.range(AREA_X.0, AREA_X.1), rng.range(AREA_Z.0, AREA_Z.1));
        let want = grassy(at);
        if rng.f() > want {
            continue;
        }
        let blades = 6 + (rng.f() * 10.0 * want) as usize;
        let tall = rng.range(0.18, 0.45) * (0.6 + 0.5 * want);
        let dry = fbm(at * 0.5, 502);
        for _ in 0..blades {
            let p = at + Vec2::new(rng.range(-0.12, 0.12), rng.range(-0.12, 0.12));
            let h = tall * rng.range(0.6, 1.1);
            let width = rng.range(0.012, 0.022);
            let yaw = rng.f() * std::f32::consts::TAU;
            let side = Vec3::new(yaw.cos(), 0.0, yaw.sin()) * width;
            // Dead blades lean and fold over more than live ones.
            let out = (p - at).normalize_or(Vec2::X);
            let lean = Vec3::new(out.x, 0.0, out.y) * h * rng.range(0.3, 0.9);
            let base = pos.len() as u32;
            let segs = 3;
            for k in 0..=segs {
                let t = k as f32 / segs as f32;
                let centre = Vec3::new(p.x, -0.03, p.y) + Vec3::Y * h * t + lean * t * t;
                let w = 1.0 - t * 0.9;
                pos.push((centre - side * w).to_array());
                pos.push((centre + side * w).to_array());
                let n = (Vec3::Y * 0.8 + side.cross(Vec3::Y).normalize() * 0.2).normalize();
                nor.push(n.to_array());
                nor.push(n.to_array());
                // Straw at the tips, olive-brown and dark at the root.
                let g = 0.3 + 0.7 * t;
                let c = Color::srgb(
                    g * (0.3 + 0.12 * dry),
                    g * (0.25 + 0.06 * dry),
                    g * (0.13 + 0.03 * dry),
                )
                .to_linear()
                .to_f32_array();
                col.push(c);
                col.push(c);
            }
            for k in 0..segs as u32 {
                let a = base + k * 2;
                idx.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
            }
        }
    }
    let n = pos.len();
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n])
    .with_inserted_indices(Indices::U32(idx));
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            perceptual_roughness: 0.9,
            cull_mode: None,
            double_sided: true,
            ..default()
        })),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grass grows on the earth and not on the fight's whole flags.
    #[test]
    fn grass_on_earth_not_flags() {
        assert!(grassy(Vec2::ZERO) < 0.01, "no grass on the fight's flags");
        assert!(
            grassy(Vec2::new(0.0, -13.0)) > 0.2,
            "grass beyond the back wall"
        );
    }
}
