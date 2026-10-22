//! The base on screen (`terra_bases`, D407) at its old path, and building play on it, whose
//! modules move to their own crate later (research/2026-10-18-crate-layout.md, step 8).

pub use terra_bases::*;

pub(crate) mod act;
pub mod building;
pub(crate) mod fire;
mod going_up;
mod loss;
pub(crate) mod props;
mod remote;
mod sheet;
mod village;

/// The kinds a village is built of, which building play and the townsfolk ask, from the lowest
/// crate both stand on.
pub(crate) use terra_survival::village_kinds;
