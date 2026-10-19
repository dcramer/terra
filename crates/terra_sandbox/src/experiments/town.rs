//! The living town demo (docs/demos.md, a living town): a market town of thirty to forty
//! buildings out in the world, and its people about their day (`terra_game::play::town`,
//! `terra_town`). With `--site town` on the mountain world, the town stands where the land's
//! rules make room for one, below the range with the forest past its fields
//! (`terra_land::town`); without, its square goes on the most level ground a few hundred
//! metres from the start. With `play`:
//!
//!   cargo run --release -p terra_sandbox -- --experiment play --experiment town --radius 30000 --site town --time 10
//!
//! The log gives its vantage points (`Town vantage: …`) as `--vantage` takes them.

use bevy::prelude::*;
use terra_game::play::town::Wanted;
use terra_world::planet::Planet;
use terra_world::start::{Found, Sited, Start};

/// The town shown: its seed, chosen for a long square with a side street up the slope.
const SEED: u32 = 2;

/// How far from the place the land's rules found its square may move, to the most level
/// ground there on the ground as drawn.
const AT_THE_SITE_M: f32 = 40.0;

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
    app.insert_resource(wanted(true))
        .add_systems(Startup, at_the_site);
}

/// The same town with nobody in it, to measure what its people and animals cost.
pub fn empty(app: &mut App) {
    app.insert_resource(wanted(false))
        .add_systems(Startup, at_the_site);
}

/// The town wanted where the land's rules found a place for it (`--site town`), in metres
/// east and south of the start beside it.
fn at_the_site(
    sited: Option<Res<Sited>>,
    planet: Res<Planet>,
    start: Res<Start>,
    mut wanted: ResMut<Wanted>,
) {
    let Some(Found::Town(site) | Found::Settlement(site)) = sited.as_deref().map(|sited| sited.0)
    else {
        return;
    };
    let at = start.local(planet.surface(site.toward));
    wanted.near = Vec2::new(at.x, at.z);
    wanted.within = AT_THE_SITE_M;
    info!(
        "Town wanted at the site the land's rules found: {:.0} m east and {:.0} m south of the start.",
        at.x, at.z
    );
}
