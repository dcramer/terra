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

pub mod field;
pub mod planet;

pub use field::{Field, GeneratedPlanet, Sample, material};
pub use planet::VoxelPlanet;
