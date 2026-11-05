//! A tree's mesh being built: its vertices with their sway, colour and, for wind in levels,
//! their limbs' and clumps' pivots (D597).

use crate::grown::pivots;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

/// A mesh being built.
#[derive(Default)]
pub struct Build {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub sway: Vec<[f32; 2]>,
    pub colours: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    /// For a crown whose cards drop out with distance: each vertex's offset from its
    /// card's middle, and in w the card's rank in its clump plus a hundredth of how many
    /// cards the clump has (D223). Carried as the tangent, which leaves have no other use for.
    pub dropping: Vec<[f32; 4]>,
    /// For a shadow stand-in, which only the sun sees: each vertex's offset from its card's
    /// or lump's middle in its colour, since Bevy's shadow passes are given no tangents
    /// (D234).
    pub offset_in_colour: bool,
    /// For a grown tree's wood and crowns, wind in levels (D597, `pivots`): each vertex's
    /// great limb's pivot, the one the next vertices take, and whether the mesh carries them.
    pub pivots: Vec<[f32; 4]>,
    pub pivot: Option<[f32; 4]>,
    /// Each vertex's clump's own pivot, where it sits on its limb (`pivots::ATTRIBUTE_TWIG`).
    pub twigs: Vec<[f32; 4]>,
    pub twig: Option<[f32; 4]>,
    pub levels: bool,
}

impl Build {
    pub fn vertex(&mut self, at: Vec3, normal: Vec3, uv: Vec2, sway: Vec2, colour: Vec3) -> u32 {
        self.pivots.push(self.pivot.unwrap_or(pivots::NO_PIVOT));
        self.twigs.push(self.twig.unwrap_or(pivots::NO_PIVOT));
        self.positions.push(at.into());
        self.normals.push(normal.into());
        self.uvs.push(uv.into());
        self.sway.push(sway.into());
        self.colours.push([colour.x, colour.y, colour.z, 1.0]);
        self.positions.len() as u32 - 1
    }

    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn mesh(self) -> Mesh {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.sway)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colours)
        .with_inserted_indices(Indices::U32(self.indices));
        let mesh = if self.levels {
            mesh.with_inserted_attribute(pivots::ATTRIBUTE_PIVOT, self.pivots)
                .with_inserted_attribute(pivots::ATTRIBUTE_TWIG, self.twigs)
        } else {
            mesh
        };
        if self.dropping.is_empty() {
            mesh
        } else {
            mesh.with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, self.dropping)
        }
    }
}
