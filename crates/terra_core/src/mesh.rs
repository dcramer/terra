use crate::MaterialId;
use glam::{IVec3, Vec3};

/// Which piece of terrain a mesh is: its detail level and its place in that level's grid.
/// Level 0 is the finest; each level above is twice as coarse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkId {
    pub level: u8,
    pub coord: IVec3,
}

/// Up to four materials meeting at one vertex, and how much of each. Weights sum to 255.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialWeights {
    pub ids: [MaterialId; 4],
    pub weights: [u8; 4],
}

impl MaterialWeights {
    /// A vertex made entirely of one material.
    pub fn single(material: MaterialId) -> Self {
        MaterialWeights {
            ids: [material; 4],
            weights: [255, 0, 0, 0],
        }
    }
}

/// What the terrain hands the renderer for one chunk. One entry per vertex in
/// `positions`, `normals` and `materials`; `indices` lists triangles, three at a time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChunkMesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub materials: Vec<MaterialWeights>,
    pub indices: Vec<u32>,
}

impl ChunkMesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Whether the arrays agree with each other. A renderer can rely on this.
    pub fn is_consistent(&self) -> bool {
        let vertices = self.positions.len();
        self.normals.len() == vertices
            && self.materials.len() == vertices
            && self.indices.len().is_multiple_of(3)
            && self.indices.iter().all(|&i| (i as usize) < vertices)
    }
}
