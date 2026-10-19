//! Building drawn (D407): what `terra_build`'s layout lays, made into surfaces by material,
//! ready for the game to upload, with no engine (docs/building-plan.md, drawing).
//!
//! - **A base is drawn a section at a time** ([`sections`], D211), each at the detail its
//!   distance asks: near, its pieces ([`draw`]): long members swept ([`timber`]), modules
//!   laid along lines ([`modules`]), a kit's dress by role ([`dress`]), roofs ([`roofing`]),
//!   block kinds as their cubes' own surface ([`surface`]); far off, its hull ([`hull`]).
//! - **A material's maps are read on the CPU** ([`relief`], [`cells`]): what a surface is
//!   displaced by and the stones and shingles damage breaks it along; what is destroyed
//!   bursts into fragments ([`fragments`]).
//! - **Checks over the drawing**: the kit checker ([`checker`], `cargo xtask kits`), a
//!   material's pack as it is added ([`packs`]), and what a renderer would show as junk,
//!   found from the meshes ([`inspect`]).
//!
//! The rules and the layout stand below this in `terra_build`, which never names it; the
//! fixtures over both are `terra_build_scenes`.

pub mod cells;
pub mod checker;
pub mod draw;
pub mod dress;
pub mod fragments;
pub mod frames;
pub mod hull;
pub mod inspect;
pub mod mesh;
pub mod modules;
pub mod packs;
pub mod relief;
pub mod roofing;
pub mod sections;
pub mod surface;
pub mod timber;

pub use mesh::{Indexed, Material, Surface, Surfaces};

#[cfg(test)]
mod tests;
