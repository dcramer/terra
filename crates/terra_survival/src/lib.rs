//! What a player carries, makes and needs (D407): the inventory, the bag and storage with
//! their screens, the catalogue of what can be built and the registries of kinds, crafting
//! at stations, props set down, interacting, claims and contracts, harvesting, digging and
//! work, vitals, warmth and death, skills, wear and gains, the tiles and torchlight, piles,
//! what a player is told, hazards, what is held and worn, the noise things make, and the
//! item icons.
//!
//! It stands on the walker (`terra_walker`), the base on screen (`terra_bases`) and the
//! world (`terra_world`); creatures, townsfolk and building play stand on it. The game's
//! `GameplayPlugin` adds each module's plugin. See docs/player.md and docs/building.md.

// Bevy systems take their inputs as arguments, so long argument lists and nested query
// types are normal here.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod bag;
pub mod catalogue;
pub mod claim;
pub mod contract;
pub mod crafting;
pub mod death;
pub mod dig;
pub mod gains;
pub mod harvest;
pub mod hazards;
pub mod held;
pub mod icons;
pub mod interact;
pub mod inventory;
pub mod kinds;
pub mod noise;
pub mod pile;
pub mod props;
pub mod registries;
pub mod skills;
pub mod station_screen;
pub mod storage;
pub mod storage_screen;
pub mod tiles;
pub mod told;
pub mod torchlight;
pub mod vitals;
pub mod warmth;
pub mod wear;
pub mod work;
pub mod worn;

use bevy::prelude::*;
use terra_build::kit::Kits;

/// The kinds a village is built of (`terra_town::village`): each role's material as the
/// settlement's data names it, and the roof each takes (D290). Here, not in the bases, which
/// stand below the town, nor in the town, since it reads the kits' files as the bases find
/// them; building play and the townsfolk ask it, and this is the lowest crate both stand on.
pub fn village_kinds(kits: &Kits) -> Option<terra_town::village::Palette> {
    let assets = terra_bases::kit_assets();
    let read = |file: &str| {
        std::fs::read_to_string(assets.join(file)).map_err(|why| format!("{file}: {why}"))
    };
    let names = terra_bases::kit_names();
    let named = |name: &str| {
        names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(k, _)| *k)
    };
    terra_town::village::Palette::read(&read, &named, kits)
        .map_err(|why| error!("{why}"))
        .ok()
}

/// A world with what the play systems read and write that is not of any one entity: the
/// kinds of item, and the messages about what players carry and are told. For tests, here
/// and in the crates above (the `testing` feature).
#[cfg(any(test, feature = "testing"))]
pub fn test_world() -> World {
    let mut world = World::new();
    world.init_resource::<inventory::Items>();
    world.init_resource::<Messages<inventory::Gained>>();
    world.init_resource::<Messages<inventory::Broke>>();
    world.init_resource::<Messages<inventory::Spilled>>();
    world.init_resource::<Messages<told::Told>>();
    world.init_resource::<Messages<interact::Interacted>>();
    world
}

/// A player standing at a place, with what the world's systems read of one, for tests
/// that put several in one world.
#[cfg(any(test, feature = "testing"))]
pub fn test_player(world: &mut World, at: Vec3) -> Entity {
    let mut items = world.get_resource_or_init::<inventory::Items>();
    let kit = inventory::starting_kit(&mut items);
    world
        .spawn((
            terra_walker::Player,
            terra_walker::Motion::facing(Vec3::X),
            terra_walker::Stance::default(),
            vitals::Vitals::default(),
            skills::Skills::default(),
            kit,
            terra_walker::intent::Intent::default(),
            Transform::from_translation(at),
        ))
        .id()
}
