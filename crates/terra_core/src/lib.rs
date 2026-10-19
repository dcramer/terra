//! The contracts the engine's parts share (docs/engine.md).
//!
//! These are what let the parts be built separately: the player, the sky, scattering and
//! networking are written against these types, on a stand-in planet, before the voxel
//! terrain exists. When it does, it implements the same contracts and replaces the
//! stand-in.
//!
//! Positions are in metres, in planet-local coordinates with the planet's centre at the
//! origin.

mod avoid;
mod ground;
mod mesh;
mod operation;
mod plot;
mod solid;
mod stand_in;
mod walker;

pub use avoid::{Avoidance, Round};
pub use glam::{IVec3, Vec3};
pub use ground::{Ground, RayHit, Water};
pub use mesh::{ChunkId, ChunkMesh, MaterialWeights};
pub use operation::{Harvested, KindId, ObjectId, Operation, Yield};
pub use plot::Plot;
pub use solid::{Piece, Placement, Relief, Solid, convex_hull};
pub use stand_in::StandInPlanet;
pub use terra_gen::Seed;
pub use walker::Walker;

/// What a planet is, before anything is generated from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanetSpec {
    pub seed: Seed,
    /// Distance from the centre to the nominal surface. Hills and seas are measured
    /// relative to this.
    pub radius_m: f32,
}

/// What a piece of ground is made of. Which materials exist is design (D43); the engine
/// only needs to tell them apart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialId(pub u16);
