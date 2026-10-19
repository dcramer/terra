//! Terrain: what the ground is and what it is made of at every point of a round planet,
//! the edits made to it, the chunks it is drawn in at several detail levels, and their
//! meshes (docs/terrain.md, D9).
//!
//! Plain Rust with no Bevy, so every player computes the same ground (D28). Positions
//! are in metres, in planet-local coordinates with the planet's centre at the origin.
//!
//! - [`field`]: the question everything else asks, "how far from the ground, and what is
//!   it made of?", and the planet generated from a seed that answers it.
//! - [`planet`]: a generated planet with the edits made to it, which is what the game
//!   digs, fills and walks on. Edits are kept as the operations themselves (D21).
//! - [`mesh`]: the chunks the ground is drawn in, and meshing one with seams toward finer
//!   neighbours (D32).
//! - [`lod`]: which chunks to draw, and how finely, around the people looking.

pub mod field;
pub mod lod;
pub mod mesh;
pub mod mountains;
pub mod patterned;
pub mod planet;

pub use field::{Field, GeneratedPlanet, Sample, material};
pub use lod::{Detail, chunks_around, may_hold_surface};
pub use mesh::{CELLS, Sides, VOXEL_M, chunk_at, chunk_origin, chunk_size_m, mesh_chunk};
pub use mountains::{Faces, Mountains, Range, Rise};
pub use patterned::Patterned;
pub use planet::VoxelPlanet;
