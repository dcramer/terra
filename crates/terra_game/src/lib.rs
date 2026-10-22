//! The game in Bevy (D28, D133): the player, what they carry and build, creatures, people,
//! sound and saving, as plugins, standing on the world (`terra_world`: the shell every
//! build runs under, the planet, terrain and the world layer, D198), whose modules are
//! re-exported here.
//!
//! Rules every player must agree on stay in the engine-free crates; this crate holds the
//! game's state as components and runs it as systems (D58). The sandbox (`terra_sandbox`)
//! is what launches it today, with `--experiment play`.

// Bevy systems take their inputs as arguments, so long argument lists and nested query
// types are normal here.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod build;
pub mod play;
#[cfg(test)]
mod seams;

// The walker layer (D198), its own crate since D407, at its old paths.
pub use terra_walker::{appearance, character, lift, pace, skinning, swing};
// What a player carries, makes and needs, its own crate since D407, at its old path.
pub use terra_survival::icons;
pub use terra_world::{
    audio, debug, loading, mipmaps, planet, record, remote, shell, sky, spatial, start, terrain,
};
