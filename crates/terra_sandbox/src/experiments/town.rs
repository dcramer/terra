//! The living town demo (docs/demos.md, a living town): a market town of thirty to forty
//! buildings out in the world, its square on the most level ground a few hundred metres
//! from the start, and its people about their day (`terra_game::play::town`, `terra_life`).
//! With `play`:
//!
//!   cargo run --release -p terra_sandbox -- --experiment play --experiment town --time 10
//!
//! The log gives its vantage points (`Town vantage: …`) as `--vantage` takes them.

use bevy::prelude::*;
use terra_game::play::town::Wanted;

/// The town shown: its seed, chosen for a long square with a side street up the slope.
const SEED: u32 = 2;

fn wanted(lived_in: bool) -> Wanted {
    Wanted {
        seed: SEED,
        near: Vec2::new(300.0, 0.0),
        within: 220.0,
        arrive: true,
        lived_in,
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(wanted(true));
}

/// The same town with nobody in it, to measure what its people and animals cost.
pub fn empty(app: &mut App) {
    app.insert_resource(wanted(false));
}
