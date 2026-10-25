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

mod bed;
mod experiments;

/// mimalloc, a thread-caching allocator, as engines replace the system one (Unreal's
/// mimalloc and TBB options, Bevy's examples): the game's workers make and free hundreds of
/// megabytes of section meshes and base copies while the main and render threads run, and
/// on Windows the system heap's lock made both wait tens of milliseconds behind them
/// (D340).
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

use bevy::prelude::*;
use terra_game::{appearance, character};
use terra_world::{planet, shell, terrain};

fn main() -> AppExit {
    // A panic leaves a short summary for whoever reads it (terra_world::crash).
    terra_world::crash::install();
    // A film's sound bed, written and done with, no window (`bed`).
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(at) = args.iter().position(|a| a == "--sound-bed") {
        return match bed::run(&args[at + 1..]) {
            Ok(done) => {
                println!("{done}");
                AppExit::Success
            }
            Err(problem) => {
                eprintln!("{problem}");
                AppExit::error()
            }
        };
    }
    let mut options = match shell::Options::from_args(experiments::expanded(args).into_iter()) {
        Ok(options) => options,
        Err(problem) => {
            eprintln!("{problem}\n\n{}", shell::USAGE);
            return AppExit::error();
        }
    };
    // The lush forest stands on gently rolling ground rather than the yard's flat (D242).
    options.rolling = options.experiments.iter().any(|name| name == "lush");
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
    // After the experiments, so it knows whether the lush forest brought its materials.
    app.add_plugins(experiments::woods_conifers);
    app.run()
}
