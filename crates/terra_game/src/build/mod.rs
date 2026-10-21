//! The base on screen (`terra_bases`, D407) at its old path, and building play on it, whose
//! modules move to their own crate later (research/2026-10-18-crate-layout.md, step 8).

pub use terra_bases::*;
use terra_build::kit::Kits;

pub(crate) mod act;
pub mod building;
pub(crate) mod fire;
mod going_up;
mod loss;
pub(crate) mod props;
mod remote;
mod sheet;
mod village;

/// The kinds a village is built of (`terra_town::village`): each role's material as the
/// settlement's data names it, and the roof each takes (D290). Here, not in the bases, which
/// stand below the town; building play and the townsfolk ask it.
pub(crate) fn village_kinds(kits: &Kits) -> Option<terra_town::village::Palette> {
    let assets = kit_assets();
    let read = |file: &str| {
        std::fs::read_to_string(assets.join(file)).map_err(|why| format!("{file}: {why}"))
    };
    let names = kit_names();
    let named = |name: &str| {
        names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(k, _)| *k)
    };
    terra_town::village::Palette::read(&read, &named, kits)
        .map_err(|why| bevy::log::error!("{why}"))
        .ok()
}
