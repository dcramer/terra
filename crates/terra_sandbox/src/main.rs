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
/// (D340). The heap profile's build (`dhat-heap`, `heap`) has its own.
#[cfg(not(feature = "dhat-heap"))]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;
#[cfg(feature = "dhat-heap")]
mod heap;

use bevy::prelude::*;
use terra_game::{appearance, character};
use terra_world::{planet, shell, terrain};

fn main() -> AppExit {
    #[cfg(feature = "dhat-heap")]
    heap::start();
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
    // A painting kept with nothing to render, no window (`icons::keep_alone`).
    if let Some(done) = experiments::icons::keep_alone(&options) {
        return done;
    }

    let mut app = App::new();
    app.add_plugins((
        shell::ShellPlugin(options.clone()),
        planet::plugin,
        terrain::plugin,
        character::plugin,
        appearance::plugin,
    ));
    if let Err(problem) = add_experiments(&mut app, &options) {
        eprintln!("{problem}");
        return AppExit::error();
    }
    #[cfg(feature = "dhat-heap")]
    app.add_plugins(heap::plugin);
    let exit = app.run();
    #[cfg(feature = "dhat-heap")]
    heap::write();
    exit
}

/// Each experiment the options name, once, then the woods' drawing, which needs to know
/// whether an experiment brought the world layer and its woods.
fn add_experiments(app: &mut App, options: &shell::Options) -> Result<(), String> {
    let mut added: Vec<&str> = Vec::new();
    for name in &options.experiments {
        // Named twice, as play is by the launcher and by an experiment that stands for it
        // (`cargo xtask play new -- --experiment portal`): added once.
        if added.contains(&name.as_str()) {
            continue;
        }
        added.push(name);
        let Some(experiment) = experiments::ALL.iter().find(|e| e.name == name) else {
            return Err(format!(
                "No experiment called `{name}`. Run with --list to see them."
            ));
        };
        (experiment.add)(app);
    }
    app.add_plugins(experiments::woods_conifers);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, state::app::StatesPlugin};
    use terra_world::instance::{Cross, InWorld, InstanceKey, WorldSpec};

    /// With no experiment on, the sandbox has no world layer, so no woods and no grown
    /// plants' material: what it adds round the experiments enters a world and runs with
    /// nothing missing (the woods' drawing, always added, once asked for both and panicked).
    #[test]
    fn the_sandbox_enters_a_world_with_no_experiment() {
        let options = shell::Options::from_args(experiments::expanded(Vec::new()).into_iter())
            .expect("no arguments are a run's options");
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
            .add_plugins(terra_world::instance::plugin)
            .add_systems(Startup, |mut commands: Commands| {
                commands.queue(Cross(WorldSpec {
                    key: InstanceKey(1),
                    kind: "forest".into(),
                    seed: 7,
                }));
            });
        add_experiments(&mut app, &options).unwrap();
        // Enough frames to enter the first world and run in it.
        for _ in 0..6 {
            app.update();
        }
        assert!(
            app.world().contains_resource::<State<InWorld>>(),
            "the first world was entered"
        );
    }
}
