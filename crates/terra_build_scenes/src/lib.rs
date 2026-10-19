//! Scenes: bases built whole from shortcut operations, for the kit sheet to lay and the
//! tests and the drawing checker to hold to their rules. None is a system of its own; each
//! is a fixed set of operations in the kinds it is given, Bevy-free so tests and tools build
//! it headless. Building's fixtures are a crate over the rules, the layout, the drawing and
//! the settlements (D407), so the checks that need all of them live here; the example cabin,
//! the bench village and the junctions, built from the rules alone, stay in `terra_build`,
//! whose own tests lay them, and the castle, the burg and the village are `terra_town`'s.
//!
//! - [`sheet`]: the kit sheet's rows and a material's preview;
//! - [`qa_cabin`]: the QA cabin each material is filmed and checked on;
//! - [`roof_walls`]: walls meeting roofs, the kit sheet's and the checks' rows;
//! - [`fuzz`]: small bases built at random from a seed, for the mesh checker's fuzz pass;
//! - [`registry`]: every scene the game ships or the checks look at, by name, which the
//!   mesh checker, its ratchet, the drawing audit and the voxel contract read.

pub mod fuzz;
pub mod qa_cabin;
pub mod registry;
pub mod roof_walls;
pub mod sheet;

#[cfg(test)]
mod tests;
