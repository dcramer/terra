//! The living town demo (docs/demos.md, a living town): the market town a world's land
//! stands (`terra_world::start::Towns`, D620) and its people about their day
//! (`terra_game::play::town`, `terra_town`), with the player taken to its main street once it
//! is laid. Every world with mountains lays its first town by the start without this; with
//! `--site town` the town stands where the land's rules make room for one, below the range
//! with the forest past its fields (`terra_land::town`). With `play`:
//!
//!   cargo run --release -p terra_sandbox -- --experiment play --experiment town --radius 30000 --site town --time 10
//!
//! The log gives its vantage points (`Town vantage: …`) as `--vantage` takes them.

use bevy::prelude::*;
use terra_game::play::town::Visit;

pub fn plugin(app: &mut App) {
    app.insert_resource(Visit { lived_in: true });
}

/// The same town with nobody in it, to measure what its people and animals cost.
pub fn empty(app: &mut App) {
    app.insert_resource(Visit { lived_in: false });
}
