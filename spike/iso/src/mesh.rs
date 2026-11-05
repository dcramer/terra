//! Meshes the scene is built from: boxes whose textures keep their size in metres on every
//! face, a gable's triangle, and a ground patch whose edge fades out by its vertices' alpha.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

#[derive(Default)]
struct Builder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Builder {
    /// A quad `c ± u ± v` facing `n`, its texture `tile` metres across, measured from the
    /// mesh's origin so neighbouring faces line up.
    fn quad(&mut self, c: Vec3, u: Vec3, v: Vec3, n: Vec3, tile: f32) {
        let base = self.positions.len() as u32;
        let (un, vn) = (u.normalize(), v.normalize());
        for p in [c - u - v, c + u - v, c + u + v, c - u + v] {
            self.positions.push(p.to_array());
            self.normals.push(n.to_array());
            self.uvs.push([p.dot(un) / tile, p.dot(vn) / tile]);
            self.colors.push([1.0; 4]);
        }
        if u.cross(v).dot(n) > 0.0 {
            self.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            self.indices
                .extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }

    fn build(self, colors: bool) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_indices(Indices::U32(self.indices));
        if colors {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        }
        mesh.with_generated_tangents().expect("tangents")
    }
}

/// A box of `size` centred on its origin, its texture `tile` metres across on every face;
/// on the sides the texture's top is up.
pub fn block(size: Vec3, tile: f32) -> Mesh {
    let h = size / 2.0;
    let mut b = Builder::default();
    let (x, y, z) = (Vec3::X, Vec3::Y, Vec3::Z);
    b.quad(z * h.z, x * h.x, -y * h.y, z, tile);
    b.quad(-z * h.z, -x * h.x, -y * h.y, -z, tile);
    b.quad(x * h.x, -z * h.z, -y * h.y, x, tile);
    b.quad(-x * h.x, z * h.z, -y * h.y, -x, tile);
    b.quad(y * h.y, x * h.x, z * h.z, y, tile);
    b.quad(-y * h.y, x * h.x, -z * h.z, -y, tile);
    b.build(false)
}

/// A gable end: a triangle `width` across its foot and `rise` high, in the XY plane, seen
/// from both sides.
pub fn gable(width: f32, rise: f32, tile: f32) -> Mesh {
    let mut b = Builder::default();
    let corners = [
        Vec3::new(-width / 2.0, 0.0, 0.0),
        Vec3::new(width / 2.0, 0.0, 0.0),
        Vec3::new(0.0, rise, 0.0),
    ];
    for (side, n) in [(0u32, Vec3::Z), (3, -Vec3::Z)] {
        for p in corners {
            b.positions.push(p.to_array());
            b.normals.push(n.to_array());
            b.uvs.push([p.x / tile, -p.y / tile]);
            b.colors.push([1.0; 4]);
        }
        if side == 0 {
            b.indices.extend([0, 1, 2]);
        } else {
            b.indices.extend([3, 5, 4]);
        }
    }
    b.build(false)
}

/// A flat patch of ground, `half` metres from its centre to each side in cells of `cell`,
/// keeping the cells `inside` says more than half in.
pub fn patch(half: f32, cell: f32, tile: f32, inside: impl Fn(Vec2) -> f32) -> Mesh {
    let n = (half * 2.0 / cell).ceil() as u32;
    let mut b = Builder::default();
    for j in 0..=n {
        for i in 0..=n {
            let p = Vec2::new(-half + i as f32 * cell, -half + j as f32 * cell);
            b.positions.push([p.x, 0.0, p.y]);
            b.normals.push([0.0, 1.0, 0.0]);
            b.uvs.push([p.x / tile, p.y / tile]);
            b.colors.push([1.0, 1.0, 1.0, inside(p).clamp(0.0, 1.0)]);
        }
    }
    let row = n + 1;
    for j in 0..n {
        for i in 0..n {
            let a = j * row + i;
            let corners = [a, a + 1, a + row, a + row + 1];
            if corners
                .iter()
                .map(|&k| b.colors[k as usize][3])
                .sum::<f32>()
                < 2.0
            {
                continue;
            }
            b.indices
                .extend([a, a + row, a + 1, a + 1, a + row, a + row + 1]);
        }
    }
    b.build(false)
}
