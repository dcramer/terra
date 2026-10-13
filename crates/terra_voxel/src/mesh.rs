//! Meshing a chunk: the surface of the ground within a cube, as triangles, with what it is
//! made of at each vertex. Smooth, by the Transvoxel algorithm (the `transvoxel` crate,
//! D32), so a chunk next to finer ones can be meshed with a transition face toward them
//! and the two meet without cracks.
//!
//! Chunks are cubes on a grid per detail level: level 0 is the finest, with voxels of
//! [`VOXEL_M`], and each level above is twice as coarse. Every chunk is [`CELLS`] voxels
//! on a side, so a coarser chunk covers more ground with the same work.

use crate::field::{Field, material};
use glam::Vec3;
use std::cell::RefCell;
use terra_core::{ChunkId, ChunkMesh, MaterialWeights};
use transvoxel::prelude::*;
use transvoxel::structs::grid_point::GridPoint;
use transvoxel::structs::vertex_index::VertexIndex;
use transvoxel::structs::voxel_blocks::VoxelBlockRelayingToField;
use transvoxel::traits::data_field::DataField;
use transvoxel::traits::mesh_builder::MeshBuilder;

/// Voxels along each side of a chunk.
pub const CELLS: usize = 32;
/// The side of a voxel at the finest level, in metres: half a metre, as Enshrouded's are
/// and the build grid is (D48).
pub const VOXEL_M: f32 = 0.5;

/// The side of a chunk at a level, in metres.
pub fn chunk_size_m(level: u8) -> f32 {
    CELLS as f32 * VOXEL_M * (1u32 << level) as f32
}

/// The corner of a chunk with the lowest coordinates.
pub fn chunk_origin(id: ChunkId) -> Vec3 {
    id.coord.as_vec3() * chunk_size_m(id.level)
}

/// The chunk at a level that holds a point.
pub fn chunk_at(point: Vec3, level: u8) -> ChunkId {
    ChunkId {
        level,
        coord: (point / chunk_size_m(level)).floor().as_ivec3(),
    }
}

/// The faces of a chunk that meet finer chunks, which it is meshed with a transition
/// toward. One bit a face, in the order of [`Sides::ALL`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Sides(pub u8);

impl Sides {
    pub const LOW_X: Sides = Sides(1);
    pub const HIGH_X: Sides = Sides(2);
    pub const LOW_Y: Sides = Sides(4);
    pub const HIGH_Y: Sides = Sides(8);
    pub const LOW_Z: Sides = Sides(16);
    pub const HIGH_Z: Sides = Sides(32);
    pub const ALL: [Sides; 6] = [
        Sides::LOW_X,
        Sides::HIGH_X,
        Sides::LOW_Y,
        Sides::HIGH_Y,
        Sides::LOW_Z,
        Sides::HIGH_Z,
    ];

    pub fn none() -> Sides {
        Sides(0)
    }

    pub fn contains(self, side: Sides) -> bool {
        self.0 & side.0 == side.0
    }

    pub fn with(self, side: Sides) -> Sides {
        Sides(self.0 | side.0)
    }

    /// Which way out of the chunk a single face looks, as a step between chunk coordinates.
    pub fn outward(self) -> glam::IVec3 {
        match self {
            Sides::LOW_X => glam::IVec3::NEG_X,
            Sides::HIGH_X => glam::IVec3::X,
            Sides::LOW_Y => glam::IVec3::NEG_Y,
            Sides::HIGH_Y => glam::IVec3::Y,
            Sides::LOW_Z => glam::IVec3::NEG_Z,
            _ => glam::IVec3::Z,
        }
    }

    fn to_transvoxel(self) -> TransitionSides {
        let mut sides = TransitionSide::none();
        for (ours, theirs) in Sides::ALL.into_iter().zip([
            TransitionSide::LowX,
            TransitionSide::HighX,
            TransitionSide::LowY,
            TransitionSide::HighY,
            TransitionSide::LowZ,
            TransitionSide::HighZ,
        ]) {
            if self.contains(ours) {
                sides |= theirs;
            }
        }
        sides
    }
}

/// A voxel as the mesher sees it: a density, positive inside the ground.
#[derive(Clone, Copy, Debug, Default)]
struct Voxel {
    density: f32,
}

impl transvoxel::traits::voxel_data::VoxelData for Voxel {
    type Density = f32;

    fn density(&self) -> f32 {
        self.density
    }
}

/// Collects what the mesher makes into the shared chunk mesh.
#[derive(Default)]
struct Builder {
    mesh: ChunkMesh,
}

impl MeshBuilder<Voxel, f32> for Builder {
    fn add_vertex_between(
        &mut self,
        a: GridPoint<Voxel, f32>,
        b: GridPoint<Voxel, f32>,
        toward_b: f32,
    ) -> VertexIndex {
        let at = |point: &GridPoint<Voxel, f32>| {
            Vec3::new(point.position.x, point.position.y, point.position.z)
        };
        let gradient = |point: &GridPoint<Voxel, f32>| {
            Vec3::new(point.gradient.0, point.gradient.1, point.gradient.2)
        };
        let position = at(&a).lerp(at(&b), toward_b);
        // Density rises into the ground, so the surface faces down its gradient.
        let normal = -gradient(&a).lerp(gradient(&b), toward_b);
        self.mesh.positions.push(position);
        self.mesh.normals.push(normal.normalize_or(Vec3::Y));
        VertexIndex(self.mesh.positions.len() - 1)
    }

    fn add_triangle(&mut self, a: VertexIndex, b: VertexIndex, c: VertexIndex) {
        self.mesh
            .indices
            .extend([a.0 as u32, b.0 as u32, c.0 as u32]);
    }
}

/// Meshes the ground within a chunk, with transition faces toward any sides that meet
/// finer chunks. Empty if the surface does not pass through it.
pub fn mesh_chunk<F: Field>(field: &F, id: ChunkId, finer: Sides) -> ChunkMesh {
    let origin = chunk_origin(id);
    let block = Block::new(
        [origin.x, origin.y, origin.z],
        chunk_size_m(id.level),
        CELLS,
    );
    // Nearby voxels share work, such as the land's height, through one memo. What the
    // ground is made of is only asked near the surface: nothing deeper or higher reaches a
    // vertex.
    //
    // A coarse chunk sees the ground at its own coarseness; where it meets a finer chunk it
    // reads that seam at the finer one's, so the two agree there and meet without cracks.
    let spacing = VOXEL_M * (1u32 << id.level) as f32;
    let reader = |spacing: f32| {
        let memo = RefCell::new(field.memo_for(spacing));
        move |x: f32, y: f32, z: f32| Voxel {
            density: -field.distance_in(Vec3::new(x, y, z), &mut memo.borrow_mut()),
        }
    };
    let sample = reader(spacing);
    let seam = reader(spacing * 0.5);
    let relay: &dyn DataField<Voxel, f32> = &seam;
    // The chunk's own voxels are read once each. A finer neighbour is only read where it
    // meets this chunk, so it asks the field directly. (The crate's own cached path reads
    // neighbours from the wrong block, which cracks seams, so this builds the view itself.)
    let mut blocks = BlockStarView::new_simple(VoxelVecBlock::cache(&sample, block));
    for side in finer.to_transvoxel() {
        blocks = blocks.with_neighbour(
            VoxelBlockRelayingToField {
                field: relay,
                block: block.high_res_neighbour_to(side),
            },
            side,
        );
    }
    let mut mesh = extract(&blocks, 0.0, Builder::default()).mesh;
    // What each vertex is made of is asked where it lies, just under the surface, so a
    // coarse chunk shows the same topsoil as a fine one rather than whatever its widely
    // spaced voxels happen to sit in.
    let mut memo = field.memo_for(VOXEL_M);
    mesh.materials = mesh
        .positions
        .iter()
        .zip(&mesh.normals)
        .map(|(&point, &normal)| {
            let under = field.sample_in(point - normal * UNDER_SURFACE_M, &mut memo);
            MaterialWeights::single(match under.material {
                material::AIR => material::STONE,
                other => other,
            })
        })
        .collect();
    mesh
}

/// How far under the surface a vertex's material is read.
const UNDER_SURFACE_M: f32 = 0.2;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GeneratedPlanet, VoxelPlanet};
    use glam::IVec3;
    use std::collections::HashMap;
    use terra_core::{Ground, Operation, PlanetSpec};
    use terra_gen::Seed;

    fn planet() -> VoxelPlanet<GeneratedPlanet> {
        VoxelPlanet::new(GeneratedPlanet::new(
            PlanetSpec {
                seed: Seed(7),
                radius_m: 2000.0,
            },
            150.0,
            Some(-20.0),
        ))
    }

    /// The point on the ground straight up from the centre.
    fn top(planet: &VoxelPlanet<GeneratedPlanet>) -> Vec3 {
        Vec3::Y * (2000.0 + planet.field().height_m(Vec3::Y))
    }

    #[test]
    fn a_chunk_on_the_surface_has_a_sound_mesh_of_its_materials() {
        let planet = planet();
        let mesh = mesh_chunk(&planet, chunk_at(top(&planet), 0), Sides::none());
        assert!(mesh.is_consistent());
        assert!(
            mesh.triangle_count() > 100,
            "{} triangles",
            mesh.triangle_count()
        );
        // The surface faces the way the ground does there.
        let up = planet.normal(top(&planet));
        let facing: Vec3 = mesh.normals.iter().copied().sum::<Vec3>().normalize();
        assert!(facing.dot(up) > 0.95);
        // And its triangles are wound to be seen from above, counter-clockwise from outside
        // as the renderer expects, or the ground would be culled and seen from below.
        let wound: Vec3 = mesh
            .indices
            .chunks(3)
            .map(|t| {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize]);
                (b - a).cross(c - a)
            })
            .sum();
        assert!(wound.normalize().dot(up) > 0.95);
        let soil = MaterialWeights::single(material::SOIL);
        assert!(mesh.materials.contains(&soil));
        // Every vertex is on the ground.
        assert!(
            mesh.positions
                .iter()
                .all(|&point| planet.sample(point).distance.abs() < VOXEL_M)
        );
        // Wholly in the air, or wholly in the ground, there is no surface to draw.
        for clear in [Vec3::Y * 200.0, Vec3::Y * -60.0] {
            let id = chunk_at(top(&planet) + clear, 0);
            assert_eq!(mesh_chunk(&planet, id, Sides::none()).triangle_count(), 0);
        }
    }

    /// Edges used by one triangle only, after joining vertices at the same place.
    fn open_edges(meshes: &[ChunkMesh]) -> Vec<(Vec3, Vec3)> {
        let key = |p: Vec3| (p * 1000.0).round().as_ivec3();
        let mut count: HashMap<(IVec3, IVec3), (usize, Vec3, Vec3)> = HashMap::new();
        for mesh in meshes {
            for triangle in mesh.indices.chunks(3) {
                for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                    let (a, b) = (
                        mesh.positions[triangle[i] as usize],
                        mesh.positions[triangle[j] as usize],
                    );
                    let (ka, kb) = (key(a), key(b));
                    if ka == kb {
                        continue;
                    }
                    let edge = if (ka.x, ka.y, ka.z) < (kb.x, kb.y, kb.z) {
                        (ka, kb)
                    } else {
                        (kb, ka)
                    };
                    count.entry(edge).or_insert((0, a, b)).0 += 1;
                }
            }
        }
        count
            .into_values()
            .filter(|(n, ..)| *n == 1)
            .map(|(_, a, b)| (a, b))
            .collect()
    }

    /// Meshes a coarse chunk whose high-x face is the plane x = 0, holding the ground
    /// there, and the four chunks one level finer on the other side; returns the open
    /// edges on that face, away from the region's own outer rim, which are cracks.
    fn cracks_at(level: u8, transition: bool) -> usize {
        let planet = planet();
        let ground = top(&planet);
        let size = chunk_size_m(level);
        let coarse = ChunkId {
            level,
            coord: IVec3::new(-1, (ground.y / size).floor() as i32, -1),
        };
        let height = ground.y - coarse.coord.y as f32 * size;
        assert!(
            (size * 0.2..size * 0.8).contains(&height),
            "the ground is well inside the coarse chunk: {height} m up it"
        );
        let sides = if transition {
            Sides::HIGH_X
        } else {
            Sides::none()
        };
        let mut meshes = vec![mesh_chunk(&planet, coarse, sides)];
        for (y, z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let fine = ChunkId {
                level: level - 1,
                coord: IVec3::new(0, coarse.coord.y * 2 + y, coarse.coord.z * 2 + z),
            };
            meshes.push(mesh_chunk(&planet, fine, Sides::none()));
        }
        let low = chunk_origin(coarse);
        let inside_face = |p: Vec3| {
            p.x.abs() < 1e-3
                && p.y > low.y + 1e-3
                && p.y < low.y + size - 1e-3
                && p.z > low.z + 1e-3
                && p.z < low.z + size - 1e-3
        };
        open_edges(&meshes)
            .into_iter()
            .filter(|&(a, b)| inside_face(a) && inside_face(b))
            .count()
    }

    #[test]
    fn a_coarse_chunk_meets_finer_ones_without_cracks() {
        // The finest seam, and a coarse one, where both sides blend fewer lattice points.
        for level in [1, 3] {
            assert_eq!(cracks_at(level, true), 0, "cracks at level {level}");
        }
        // Without the transition face there would be cracks, so the test can see them.
        assert!(
            cracks_at(1, false) > 0,
            "the test finds cracks when there are some"
        );
    }

    #[test]
    fn a_dug_hole_has_no_gaps() {
        let mut planet = VoxelPlanet::new(GeneratedPlanet::new(
            PlanetSpec {
                seed: Seed(1),
                radius_m: 2000.0,
            },
            150.0,
            Some(-20.0),
        ));
        let up = Vec3::Y;
        let ground = up * (2000.0 + planet.field().height_m(up));
        for depth in [0.2, 0.8, 1.4] {
            planet.apply(&Operation::Dig {
                centre: ground - up * depth,
                radius: 0.7,
            });
        }
        let middle = chunk_at(ground, 0);
        let mut meshes = Vec::new();
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let id = ChunkId {
                        level: 0,
                        coord: middle.coord + IVec3::new(x, y, z),
                    };
                    meshes.push(mesh_chunk(&planet, id, Sides::none()));
                }
            }
        }
        let near: Vec<Vec3> = open_edges(&meshes)
            .into_iter()
            .map(|(at, _)| at)
            .filter(|at| at.distance(ground) < 4.0)
            .collect();
        assert!(
            near.is_empty(),
            "{} open edges near the hole, such as {:?}",
            near.len(),
            near.first()
        );
        // The floor of the hole faces up, out of it.
        let floor: Vec<(Vec3, Vec3)> = meshes
            .iter()
            .flat_map(|mesh| {
                mesh.positions
                    .iter()
                    .copied()
                    .zip(mesh.normals.iter().copied())
            })
            .filter(|(at, _)| at.reject_from(up).distance(ground.reject_from(up)) < 0.3)
            .filter(|(at, _)| at.y < ground.y - 1.0)
            .collect();
        assert!(!floor.is_empty());
        for (at, normal) in &floor {
            assert!(normal.y > 0.5, "the floor at {at:?} faces {normal:?}");
        }
    }
}
