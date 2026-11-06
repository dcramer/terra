//! Stone blocks, each a box turned and placed, merged into one mesh a material: the
//! flagstones, the walls' courses, the pillars' drums and the fallen stones. Each block's
//! texture is box-mapped in the world (the face's dominant axis, as a triplanar shader picks)
//! at the texture's true size, shifted by a block's own offset so no two stones repeat, and
//! tinted a little by a vertex colour, so the joints between blocks read as stones laid.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

#[derive(Default)]
pub struct Blocks {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl Blocks {
    /// A box of `size`, its middle at `at`, turned by `rot`; `tile` the texture's metres
    /// across, `shift` the block's offset into it, `tint` its colour.
    pub fn add(&mut self, at: Vec3, size: Vec3, rot: Quat, tile: f32, shift: Vec2, tint: Color) {
        let h = size * 0.5;
        let c = tint.to_linear().to_f32_array();
        // Each face: its normal and two edges spanning it.
        let faces = [
            (Vec3::X, Vec3::Y, Vec3::Z),
            (-Vec3::X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::Z, Vec3::X),
            (-Vec3::Y, Vec3::X, Vec3::Z),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (-Vec3::Z, Vec3::Y, Vec3::X),
        ];
        for (n, a, b) in faces {
            let base = self.pos.len() as u32;
            let wn = rot * n;
            for (sa, sb) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let local = n * h + a * h * sa + b * h * sb;
                let p = at + rot * local;
                self.pos.push(p.to_array());
                self.nor.push(wn.to_array());
                self.uv.push((box_map(p, wn) / tile + shift).to_array());
                self.col.push(c);
            }
            // Wound counter-clockwise seen from outside: a × b = n for each face above.
            self.idx
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    pub fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
        .with_inserted_indices(Indices::U32(self.idx))
        .with_generated_tangents()
        .expect("a block mesh has normals and UVs")
    }
}

/// The face's two world axes across its dominant one, in metres; v down a wall.
fn box_map(p: Vec3, n: Vec3) -> Vec2 {
    let a = n.abs();
    if a.y >= a.x && a.y >= a.z {
        Vec2::new(p.x, p.z)
    } else if a.x >= a.z {
        Vec2::new(p.z, -p.y)
    } else {
        Vec2::new(p.x, -p.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every face's triangles wind outward: the cross product of its edges is its normal.
    #[test]
    fn faces_wind_outward() {
        let mut b = Blocks::default();
        b.add(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.8, 0.3, 0.5),
            Quat::from_rotation_y(0.7),
            2.0,
            Vec2::ZERO,
            Color::WHITE,
        );
        for t in b.idx.chunks(3) {
            let p = |i: u32| Vec3::from_array(b.pos[i as usize]);
            let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0])).normalize();
            let want = Vec3::from_array(b.nor[t[0] as usize]);
            assert!(
                n.dot(want) > 0.99,
                "face winds {n} against its normal {want}"
            );
        }
    }

    /// A box's top is flat in UV at the texture's size: 2 m of stone is one tile of a 2 m map.
    #[test]
    fn top_maps_at_true_size() {
        let mut b = Blocks::default();
        b.add(
            Vec3::ZERO,
            Vec3::new(2.0, 0.2, 2.0),
            Quat::IDENTITY,
            2.0,
            Vec2::ZERO,
            Color::WHITE,
        );
        let top: Vec<_> = (0..b.pos.len()).filter(|i| b.nor[*i][1] > 0.9).collect();
        let us: Vec<f32> = top.iter().map(|i| b.uv[*i][0]).collect();
        let span = us.iter().cloned().fold(f32::MIN, f32::max)
            - us.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            (span - 1.0).abs() < 1e-5,
            "2 m across a 2 m tile spans {span}"
        );
    }
}
