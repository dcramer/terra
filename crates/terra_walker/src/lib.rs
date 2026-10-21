//! The walker (D198): a body this machine steers over the planet, the camera behind it,
//! the controls and options menu that steer it, and the character's look, from the
//! appearance chosen to the skinning, the lift, the swing and the pace of its animations.
//!
//! It stands on the world (`terra_world`) and the base on screen (`terra_bases`), so a run
//! with no gameplay on, such as `--experiment walk`, goes for a walk with nothing else of
//! the game in it. What lies above it talks to it through the small types re-exported
//! here; the player's own components (`Player`, `Motion`, `Stance`, `View`, `Downed`) are
//! the walker's, for every layer to read. See docs/player.md.

// Bevy systems take their inputs as arguments, so long argument lists and nested query
// types are normal here.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod appearance;
pub mod bodies;
pub mod button;
pub mod camera;
pub mod character;
pub mod controls;
pub mod figure;
pub mod intent;
pub mod lift;
/// What the model files carry, for this crate's tests and those of the crates above
/// (the `testing` feature).
#[cfg(any(test, feature = "testing"))]
pub mod model_files;
pub mod movement;
pub mod options;
pub mod pace;
pub mod shake;
pub mod skinning;
pub mod swing;
pub mod walker;

pub use walker::{
    ArriveAt, Downed, Hands, IsLocalPlayer, LocalPlayer, Motion, Player, PlayerInput, Stamina,
    Stance, View, WalkerPlugin, WarpTo,
};
