//! The sandbox: a test environment for working on graphics and utilities.
//!
//! It opens a window on a stand-in planet with a free camera and live tweaks, and can
//! switch on experiments, each a small self-contained plugin. See the README beside this
//! crate for the controls and for how to add an experiment.
//!
//!   cargo run -p terra_sandbox -- --experiment scatter
//!   cargo run -p terra_sandbox -- --list

// Bevy systems take their inputs as arguments, so long argument lists and nested query
// types are normal here.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod experiments;

use bevy::prelude::*;
use terra_game::{appearance, character};
use terra_world::{planet, shell, terrain};

fn main() -> AppExit {
    let options = match shell::Options::from_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(problem) => {
            eprintln!("{problem}\n\n{}", shell::USAGE);
            return AppExit::error();
        }
    };
    if options.list {
        for experiment in experiments::ALL {
            println!("{:10} {}", experiment.name, experiment.about);
        }
        return AppExit::Success;
    }

    let mut app = App::new();
    app.add_plugins((
        shell::ShellPlugin(options.clone()),
        planet::plugin,
        terrain::plugin,
        character::plugin,
        appearance::plugin,
    ));
    for name in &options.experiments {
        let Some(experiment) = experiments::ALL.iter().find(|e| e.name == name) else {
            eprintln!("No experiment called `{name}`. Run with --list to see them.");
            return AppExit::error();
        };
        (experiment.add)(&mut app);
    }
    app.run()
}
