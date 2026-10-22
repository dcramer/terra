//! Creatures and fighting (D407): the kinds of creature (`species`) and the creatures
//! themselves, wildlife put out round the players, fliers, coats that turn with the seasons,
//! staged figures, what a film sees of the animals, and both sides of a fight: where a blow
//! lands, the sword and shield, the bow, combat and how a blow is shown.
//!
//! It stands on what a player carries, makes and needs (`terra_survival`), the walker
//! (`terra_walker`), the base on screen (`terra_bases`) and the world (`terra_world`); the
//! townsfolk stand on it. The game's `GameplayPlugin` adds each module's plugin. See
//! docs/creatures.md and docs/combat.md.

// Bevy systems take their inputs as arguments, so long argument lists and nested query
// types are normal here.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod archery;
pub mod coats;
pub mod combat;
pub mod creatures;
pub mod fliers;
pub mod hits;
pub mod melee;
pub mod sighted;
pub mod species;
pub mod staged;
pub mod wildlife;
pub mod zones;
