//! Building play (D407): acting on a base with the hammer (`act`, `tool`), what building
//! costs and how each kind goes in (`act::BuildRules`), the build menu (`build_menu`) and the
//! dial (`dial`), doors and shutters (`props`), what comes down, its dust and heaps
//! (`loss`), the base's fire (`fire`), a house going up (`going_up`), the kit sheet and
//! villages laid as scenes (`sheet`, `village`), and the remote's building methods
//! (`remote`); `building` wires it over the base.
//!
//! It stands on survival (`terra_survival`: what building costs and draws on, the catalogue,
//! claims and props), the walker (`terra_walker`), the base on screen (`terra_bases`), the
//! world (`terra_world`) and the plain building crates. The game's `GameplayPlugin` adds
//! its plugins. See docs/building.md and docs/building-plan.md.

// Bevy systems take their inputs as arguments, so long argument lists and nested query
// types are normal here.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod act;
pub mod build_menu;
pub mod building;
mod contacts;
pub mod dial;
pub mod fire;
mod going_up;
mod loss;
pub mod props;
mod remote;
mod sheet;
pub mod tool;
mod village;
