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

pub mod appearance;
pub mod build;
pub mod character;
pub mod icons;
pub mod lift;
pub mod pace;
pub mod play;
pub mod skinning;
pub mod swing;

pub use terra_world::{
    audio, debug, loading, mipmaps, planet, record, remote, shell, sky, spatial, start, terrain,
};
