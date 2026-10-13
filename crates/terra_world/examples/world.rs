//! The world on its own, built without any of the game: the planet, its terrain, sky and
//! light, and what grows, seen from the free camera. For trying the world's parts where
//! nothing else compiles, runs or gets in the way.
//!
//!   cargo run -p terra_world --example world
//!   cargo run -p terra_world --example world -- --offscreen --time 13 --shot world.png
//!
//! It takes the shell's options (`--help`). With the mouse captured, W A S D and the mouse fly.

use bevy::prelude::*;
use terra_world::{WorldPlugin, planet, shell, terrain};

fn main() -> AppExit {
    let options = match shell::Options::from_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(problem) => {
            eprintln!("{problem}\n\n{}", shell::USAGE);
            return AppExit::error();
        }
    };
    App::new()
        .add_plugins((
            shell::ShellPlugin(options),
            planet::plugin,
            terrain::plugin,
            WorldPlugin,
        ))
        .run()
}
