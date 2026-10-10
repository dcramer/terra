//! Experiments: small, self-contained plugins switched on from the command line.
//!
//! To add one, write a module with a `pub fn plugin(app: &mut App)` and list it in `ALL`.
//! It can use everything the shell and the planet provide, and nothing depends on it.

use bevy::prelude::*;

mod build;
mod forest;
mod look;
mod scatter;

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
        add: |app| {
            app.add_plugins(build::plugin);
        },
    },
    Experiment {
        name: "forest",
        about: "About a million generated pines over the whole planet. F or click chops one; K chops all in view.",
        add: |app| {
            app.add_plugins(forest::plugin);
        },
    },
    Experiment {
        name: "look",
        about: "An art-direction test: faceted shapes, warm light, cool shadows, haze and sky.",
        add: |app| {
            app.add_plugins(look::plugin);
        },
    },
];
