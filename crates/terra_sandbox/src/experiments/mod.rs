//! Experiments: small, self-contained plugins switched on from the command line.
//!
//! To add one, write a module with a `pub fn plugin(app: &mut App)` and list it in `ALL`.
//! It can use everything the shell and the planet provide, and nothing depends on it.

use bevy::prelude::*;

mod crowd;
mod forest;
mod icons;
mod look;
mod lush;
mod scatter;
mod town;
mod workbench;

/// Not an experiment: always on, it gives the world's woods the lush forest's Scots pines
/// and Norway spruces to draw (D366).
pub use lush::conifers::plugin as woods_conifers;

pub struct Experiment {
    pub name: &'static str,
    pub about: &'static str,
    pub add: fn(&mut App),
}

pub const ALL: &[Experiment] = &[
    Experiment {
        name: "scatter",
        about: "Rocks placed from a seed around the start. Left click, or F, harvests one.",
        add: |app| {
            app.add_plugins(scatter::plugin);
        },
    },
    Experiment {
        name: "build",
        about: "Construction: a base of walls, floors and roofs. V cycles a wall cell through windows and doors, B adds a room.",
        add: terra_game::build::building::plugin,
    },
    Experiment {
        name: "forest",
        about: "About a million generated pines over the whole planet. F or click chops one; K chops all in view.",
        add: |app| {
            app.add_plugins(forest::plugin);
        },
    },
    Experiment {
        name: "walk",
        about: "The world and a body to walk through it, and nothing else of the game: no building, items, creatures or sound. With lush, a walk through the forest.",
        add: |app| {
            use terra_game::play::{GamePlugins, GameplayPlugin};
            app.add_plugins(GamePlugins.build().disable::<GameplayPlugin>());
        },
    },
    Experiment {
        name: "play",
        about: "A player on the ground with a builder tool: walk, jump, go indoors, and build.",
        add: terra_game::play::plugin,
    },
    Experiment {
        name: "workbench",
        about: "Models on a plain floor in the game's light: --model <path> to load, --anim <clip> to play, --hold and --attach for what they carry, --sheet <prefix> to capture views and exit; --remote to drive it live.",
        add: workbench::plugin,
    },
    Experiment {
        name: "crowd",
        about: "Many dressed characters, every template in turn, walking, jogging and idle: --crowd <n> (100 by default), --wear to change the outfit; prints what they add up to. With --bench, times the frames.",
        add: crowd::plugin,
    },
    Experiment {
        name: "icons",
        about: "The icon baker (D129): renders every item from its look into assets/icons, then exits; --sheet <prefix> also lays them out, --renders <folder> keeps the renders. Run as cargo xtask icons.",
        add: icons::plugin,
    },
    Experiment {
        name: "lush",
        about: "Spike: a forest as rich as big games make them, with trees grown in code and leaves as cards. Use with walk, or play, on the yard.",
        add: lush::plugin,
    },
    Experiment {
        name: "town",
        about: "A market town and its people about their day: use with play and --time; with --radius 30000 --site town it stands where the land's rules make room for one, below the range with the forest past its fields; the log gives its vantage points.",
        add: town::plugin,
    },
    Experiment {
        name: "town-empty",
        about: "The same market town with nobody in it, to measure what its people and animals cost.",
        add: town::empty,
    },
    Experiment {
        name: "look",
        about: "An art-direction test: faceted shapes, warm light, cool shadows, haze and sky.",
        add: |app| {
            app.add_plugins(look::plugin);
        },
    },
];
