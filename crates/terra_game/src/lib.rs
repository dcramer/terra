//! The game in Bevy (D28, D133): the world on screen, the player, what they carry and
//! build, creatures, people, sound and saving, as plugins, and the shell every build runs
//! under: the window and camera, the launch options, scripted runs, the frame-time readout
//! and the remote protocol (D69, D77).
//!
//! Rules every player must agree on stay in the engine-free crates; this crate holds the
//! game's state as components and runs it as systems (D58). The sandbox (`terra_sandbox`)
//! is what launches it today, with `--experiment play`.

// Bevy systems take their inputs as arguments, so long argument lists and nested query
// types are normal here.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod appearance;
pub mod audio;
pub mod build;
pub mod character;
pub mod debug;
pub mod icons;
pub mod loading;
pub mod planet;
pub mod play;
pub mod record;
pub mod remote;
pub mod shell;
pub mod sky;
pub mod spatial;
pub mod terrain;
