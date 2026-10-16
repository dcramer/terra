//! The world on screen in Bevy (D28, D198): the shell every run starts in (the window and
//! camera, the launch options, scripted runs, the frame-time readout and the remote
//! protocol, D69, D77), the planet, its terrain and sky, the start, and the world layer of
//! the game, [`WorldPlugin`]: the light and air, the day, what grows, the tick and levels
//! of detail.
//!
//! It knows nothing of what a player does. The game (`terra_game`) stands on it, and talks
//! to it through the small types here, such as a [`footprint::Footprint`] that keeps what
//! grows out of what is built, or a [`lod::Viewpoint`] that detail is graded from.

// Bevy systems take their inputs as arguments, so long argument lists and nested query
// types are normal here.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod air;
pub mod audio;
pub mod cache;
pub mod cine;
pub mod climate;
pub mod cover;
pub mod data;
pub mod daylight;
pub mod debug;
pub mod detail;
pub mod dither;
pub mod drift;
pub mod fire;
pub mod flora;
pub mod flora_kinds;
pub mod footprint;
pub mod gltf_gate;
pub mod gpu_turn;
pub mod grown;
pub mod lakes;
pub mod loading;
pub mod lod;
pub mod mipmaps;
pub mod models;
pub mod pacing;
pub mod parting;
pub mod planet;
pub mod quality;
pub mod record;
pub mod remote;
pub mod rises;
pub mod scene;
pub mod shell;
pub mod sky;
pub mod sky_occlusion;
pub mod skylight;
pub mod smoke;
pub mod spatial;
pub mod start;
pub mod terrain;
pub mod tick;
pub mod upscale;
pub mod waves;
pub mod wind;
pub mod woods;
pub mod world;

pub use world::WorldPlugin;
