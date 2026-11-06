//! The ruin's floor: a court paved in dressed flags (Poly Haven's scanned Large Grey Tiles,
//! the game's crypt floor, at its true 3 m), worn and grimed, broken away to packed earth at a
//! ragged edge and in holes, with loose and displaced slabs where it breaks. The paving is one
//! mesh cut along the edge's contour, so the flags' joints are the scan's own: sunk,
//! chipped and mossed, never a checkerboard of boxes.

use super::blocks::Blocks;
use crate::forest::Rng;
use crate::scene::material;
use crate::terrain::fbm;
use bevy::asset::RenderAssetUsages;
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// The court's paving, west to east and north to south, before its ragged edge.
pub const PAVED_X: (f32, f32) = (-9.0, 13.0);
pub const PAVED_Z: (f32, f32) = (-7.2, 9.0);
/// Where the flags' tops lie, and the earth below them.
pub const TOP: f32 = 0.01;
const EARTH: f32 = -0.03;
/// The paving's mesh spacing: fine enough that its cut edge reads as broken stone.
const CELL: f32 = 0.2;
/// The flag texture's width in metres (assets/materials/README.md).
const FLAGS_M: f32 = 3.0;

/// How much paving is left at `p`, 0 (earth) to 1 (whole): a soft rectangle with a ragged,
/// noisy edge 2.5 m deep, and a few holes inside where slabs are gone.
pub fn paved(p: Vec2) -> f32 {
    let inside = (p.x - PAVED_X.0)
        .min(PAVED_X.1 - p.x)
        .min(p.y - PAVED_Z.0)
        .min(PAVED_Z.1 - p.y);
    let ragged = (fbm(p * 0.45, 301) - 0.5) * 3.2 + (fbm(p * 1.7, 302) - 0.5) * 0.8;
    let edge = smooth(-0.4, 0.4, inside + ragged);
    // Holes: none under the fight, so it stands on whole flags.
    let clear = smooth(3.5, 5.5, (p - Vec2::new(0.5, -1.0)).length());
    let holes = smooth(0.66, 0.72, fbm(p * 0.6 + Vec2::splat(40.0), 303)) * clear;
    // The rain pool by the west brazier, where the flags have sunk and broken (water.rs).
    let pool = smooth(0.2, 0.5, super::water::pool(p));
    edge * (1.0 - holes) * (1.0 - pool)
}

pub fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut weathered: ResMut<Assets<super::flags::FlagsMaterial>>,
) {
    // The earth: a wide plane, its texture repeated every 2 m, dark as night soil.
    let mut earth = material(&asset, "packed_earth", Color::srgb(0.4, 0.36, 0.32));
    earth.uv_transform = Affine2::from_scale(Vec2::splat(40.0));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(80.0, 80.0))),
        MeshMaterial3d(materials.add(earth)),
        Transform::from_xyz(0.0, EARTH, 0.0),
    ));

    let flags = materials.add(material(&asset, "large_grey_tiles", Color::WHITE));
    commands.spawn((
        Mesh3d(meshes.add(paving())),
        MeshMaterial3d(weathered.add(super::flags::flags(&asset))),
    ));

    // Loose slabs where the paving breaks: lifted, tipped and fallen, grey as the flags.
    let mut loose = Blocks::default();
    let mut rng = Rng::new(0x5ab_1e5);
    let mut placed = 0;
    while placed < 70 {
        let p = Vec2::new(
            rng.range(PAVED_X.0 - 2.0, PAVED_X.1 + 2.0),
            rng.range(PAVED_Z.0 - 1.0, PAVED_Z.1 + 2.0),
        );
        let left = paved(p);
        if !(0.05..0.6).contains(&left) {
            continue;
        }
        placed += 1;
        let size = Vec3::new(
            rng.range(0.45, 1.1),
            rng.range(0.08, 0.14),
            rng.range(0.4, 0.8),
        );
        let tip = rng.range(2.0, 9.0f32).to_radians() * (1.0 - left);
        let rot = Quat::from_euler(
            EulerRot::YXZ,
            rng.range(-0.4, 0.4),
            tip * if rng.f() < 0.5 { 1.0 } else { -1.0 },
            rng.range(-tip, tip),
        );
        let v = rng.range(0.35, 0.6);
        let at = Vec3::new(p.x, EARTH + size.y * 0.35, p.y);
        let shift = Vec2::new(rng.f(), rng.f());
        loose.add(
            at,
            size,
            rot,
            FLAGS_M,
            shift,
            Color::srgb(v, v * 0.97, v * 0.93),
        );
    }
    commands.spawn((Mesh3d(meshes.add(loose.mesh())), MeshMaterial3d(flags)));
}

/// The paving's mesh: a grid over the court and its edge, cut where the paving gives out
/// (`paved` below a half), its colour the grime (value only, darker in drifts and toward the broken edge, where dirt
/// has washed over the flags), its height a little uneven where the ground has settled.
fn paving() -> Mesh {
    let (x0, x1) = (PAVED_X.0 - 3.0, PAVED_X.1 + 3.0);
    let (z0, z1) = (PAVED_Z.0 - 3.0, PAVED_Z.1 + 3.0);
    let nx = ((x1 - x0) / CELL) as u32 + 1;
    let nz = ((z1 - z0) / CELL) as u32 + 1;
    let mut pos = Vec::new();
    let mut uv = Vec::new();
    // The weather flags.wesl lays over the flags: how mossy, how wet.
    let mut weather = Vec::new();
    let mut col = Vec::new();
    let mut idx = Vec::new();
    let corner = |i: u32, j: u32| Vec2::new(x0 + i as f32 * CELL, z0 + j as f32 * CELL);
    for j in 0..nz - 1 {
        for i in 0..nx - 1 {
            let (a, b, c, d) = (
                corner(i, j),
                corner(i + 1, j),
                corner(i, j + 1),
                corner(i + 1, j + 1),
            );
            // Wound so the face looks up: (+z) then (+x) seen from above.
            for tri in [[a, c, b], [b, c, d]] {
                let kept = clip(tri, |p| paved(p) - 0.5);
                if kept.len() < 3 {
                    continue;
                }
                let base = pos.len() as u32;
                for p in &kept {
                    let left = paved(*p);
                    let settle = (fbm(*p * 0.3, 311) - 0.5) * 0.03;
                    pos.push([p.x, TOP + settle, p.y]);
                    uv.push((*p / FLAGS_M).to_array());
                    weather.push([super::litter::mossy(*p), super::water::puddled(*p)]);
                    // Grime: drifts of dirt washed over the flags, thickest at the broken edge.
                    let drift = smooth(0.35, 0.75, fbm(*p * 0.25, 312));
                    let speck = fbm(*p * 2.5, 313);
                    let edge = 1.0 - smooth(0.5, 0.95, left);
                    let v = 0.78 * (1.0 - 0.45 * drift - 0.55 * edge) * (0.85 + 0.3 * speck);
                    col.push(
                        Color::srgb(v, v * 0.97, v * 0.93)
                            .to_linear()
                            .to_f32_array(),
                    );
                }
                for k in 1..kept.len() as u32 - 1 {
                    idx.extend([base, base + k, base + k + 1]);
                }
            }
        }
    }
    let n = pos.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, weather)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
    .with_inserted_indices(Indices::U32(idx))
    .with_generated_tangents()
    .expect("the paving has normals and UVs")
}

/// The part of a triangle where `f` is at least 0, a polygon (Sutherland-Hodgman against a
/// field, each cut where `f` crosses 0 along an edge, as marching triangles finds a contour).
fn clip(tri: [Vec2; 3], f: impl Fn(Vec2) -> f32) -> Vec<Vec2> {
    let v = tri.map(&f);
    let mut out = Vec::with_capacity(4);
    for k in 0..3 {
        let (p, q) = (tri[k], tri[(k + 1) % 3]);
        let (fp, fq) = (v[k], v[(k + 1) % 3]);
        if fp >= 0.0 {
            out.push(p);
        }
        if (fp >= 0.0) != (fq >= 0.0) {
            out.push(p + (q - p) * (fp / (fp - fq)));
        }
    }
    out
}

pub fn smooth(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fight's ground is paved and the far earth is not: the court is where the refs'
    /// fights stand, the edge gives way to earth.
    #[test]
    fn court_paved_edges_earth() {
        assert!(paved(Vec2::ZERO) > 0.9, "the fight's floor is paved");
        assert!(
            paved(Vec2::new(0.0, -12.0)) < 0.01,
            "beyond the back wall is earth"
        );
        assert!(
            paved(Vec2::new(-14.0, 0.0)) < 0.01,
            "beyond the west wall is earth"
        );
    }

    /// A triangle cut by a straight contour keeps exactly the half on the kept side: the
    /// right triangle (0,0),(2,0),(0,2) cut at x = 1 keeps a trapezoid of area 1.5.
    #[test]
    fn clip_keeps_the_kept_side() {
        let tri = [Vec2::ZERO, Vec2::new(2.0, 0.0), Vec2::new(0.0, 2.0)];
        let kept = clip(tri, |p| 1.0 - p.x);
        let area: f32 = (1..kept.len() - 1)
            .map(|k| (kept[k] - kept[0]).perp_dot(kept[k + 1] - kept[0]) * 0.5)
            .sum();
        assert!(
            (area.abs() - 1.5).abs() < 1e-5,
            "kept {kept:?}, area {area}"
        );
        assert!(kept.iter().all(|p| p.x <= 1.0 + 1e-5));
    }

    /// The paving's triangles face up, so the flags are lit from above and not culled.
    #[test]
    fn paving_faces_up() {
        let mesh = paving();
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(pos)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions are Float32x3");
        };
        let Some(Indices::U32(idx)) = mesh.indices() else {
            panic!("indices are u32");
        };
        let p = |i: u32| Vec3::from_array(pos[i as usize]);
        let t = &idx[0..3];
        let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
        assert!(n.y > 0.0, "the first triangle faces {n}");
    }
}
