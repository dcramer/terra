//! Experiments: small, self-contained plugins switched on from the command line.
//!
//! To add one, write a module with a `pub fn plugin(app: &mut App)` and list it in `ALL`.
//! It can use everything the shell and the planet provide, and nothing depends on it.

use bevy::prelude::*;

mod forest;
mod icons;
mod look;
mod scatter;
mod workbench;

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
        about: "Construction: a base you shape. G and H push and pull a wall, V cycles windows and doors, B adds a room.",
        add: terra_game::build::plugin,
    },
    Experiment {
        name: "forest",
        about: "About a million generated pines over the whole planet. F or click chops one; K chops all in view.",
        add: |app| {
            app.add_plugins(forest::plugin);
        },
    },
    Experiment {
        name: "play",
        about: "A player on the ground with a builder tool: walk, jump, go indoors, and reshape the base.",
        add: terra_game::play::plugin,
    },
    Experiment {
        name: "workbench",
        about: "Models on a plain floor in the game's light: --model <path> to load, --anim <clip> to play, --hold and --attach for what they carry, --sheet <prefix> to capture views and exit; --remote to drive it live.",
        add: workbench::plugin,
    },
    Experiment {
        name: "icons",
        about: "The icon baker (D129): renders every item from its look into assets/icons, then exits; --sheet <prefix> also lays them out; --model <path> previews a look. Run as cargo xtask icons.",
        add: icons::plugin,
    },
    Experiment {
        name: "look",
        about: "An art-direction test: faceted shapes, warm light, cool shadows, haze and sky.",
        add: |app| {
            app.add_plugins(look::plugin);
        },
    },
];
