//! People's days in a settlement (docs/demos.md, a living town; D324), the same on every
//! machine and on a server with no window (D28, D52).
//!
//! - **What a day is made of** is data, [`plan`]: the things of a town and the places round
//!   each where something is done (smart objects, as The Sims' objects advertise and The
//!   Witcher 3's people pool spots), the activities and their clips, and the roles, each a
//!   day of time windows, as Skyrim's packages are (`assets/data/life.ron`).
//! - **Walking** is a route on the town's own path graph ([`graph`], A*), the layout's
//!   (`terra_build::town`), with walkers kept out of each other's way and off what is built
//!   by ORCA, ported from the RVO2 Library ([`orca`]). No navmesh: nothing supports one on
//!   Bevy 0.20, and a town's layout knows its streets.
//! - **A town's people**, [`Life`]: households in every home, each person a role; each
//!   choosing a free spot for what the hour asks and setting off early enough to be there on
//!   time; ticked by how near a viewer is, and beyond, placed from the clock (D145).
//!
//! Pure and repeatable: no engine, no clock read, no system randomness, `libm` for its
//! roots and exponentials (D30).

pub mod graph;
mod life;
pub mod orca;
pub mod plan;

pub mod check;

pub use life::{
    Arrival, Detail, FULL_M, JOG_PACE, Life, Person, REDUCED_M, Record, Shows, Spot, State,
    WALK_PACE,
};
pub use plan::Plan;

#[cfg(test)]
mod tests;
