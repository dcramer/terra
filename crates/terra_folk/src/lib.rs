//! Townsfolk (D407): people (`people`), the settlement of the first quest (`settlement`)
//! and the place where magic pools (`pooling`), a market town laid out and living its days
//! (`town`), its people's bodies (`townsfolk`), its animals, fences, stalls and gardens, its
//! murmur and what its people do that is heard (`town_sound`), and a town rising as a
//! time-lapse with its scaffolds (`rising`, `scaffold`).
//!
//! It stands on creatures (`terra_creatures`: a town's animals are tame creatures, its lairs
//! and perches wildlife's), what a player carries, makes and needs (`terra_survival`), the
//! walker (`terra_walker`), the base on screen (`terra_bases`), the world (`terra_world`)
//! and the town's layout and life (`terra_town`). The game's `GameplayPlugin` adds each
//! module's plugin. See docs/people.md and docs/demos.md (a living town).

// Bevy systems take their inputs as arguments, so long argument lists and nested query
// types are normal here.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod people;
pub mod pooling;
pub mod rising;
pub mod scaffold;
pub mod settlement;
pub mod stall_goods;
pub mod town;
pub mod town_animals;
pub mod town_fences;
pub mod town_gardens;
pub mod town_sound;
pub mod townsfolk;
