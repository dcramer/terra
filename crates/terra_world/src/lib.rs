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

pub mod adaptation;
pub mod air;
pub mod air_start;
pub mod audio;
pub mod cache;
pub mod cine;
pub mod climate;
pub mod cooked;
pub mod cover;
pub mod crash;
pub mod data;
pub mod daylight;
pub mod debug;
pub mod debug_menu;
pub mod detail;
pub mod dither;
pub mod drift;
pub mod falloff;
#[cfg(test)]
mod far_air;
#[cfg(test)]
mod far_strata;
pub mod fire;
pub mod flora;
pub mod flora_kinds;
pub mod footprint;
pub mod frame_setup;
pub mod gltf_gate;
pub mod gpu_memory;
pub mod gpu_turn;
pub mod grading;
pub mod grown;
pub mod hable;
pub mod haze_start;
pub mod headless;
pub mod horizon;
pub mod indoor;
#[cfg(test)]
mod joint_cracks;
pub mod lakes;
pub mod loading;
pub mod lod;
pub mod log_file;
#[cfg(test)]
mod mesh_upload_tests;
pub mod mipmaps;
pub mod models;
pub mod overlay;
pub mod pacing;
pub mod parting;
pub mod planet;
pub mod quality;
pub mod record;
pub mod recorder;
pub mod remote;
pub mod render_errors;
pub mod rises;
#[cfg(test)]
mod samplers_tests;
pub mod scatter;
pub mod scene;
pub mod shader_patch;
#[cfg(test)]
mod shaders_tests;
pub mod shell;
pub mod shore;
pub mod sky;
pub mod sky_occlusion;
pub mod skylight;
pub mod smoke;
pub mod spatial;
pub mod start;
pub mod terrain;
pub mod tick;
pub mod tune;
#[cfg(test)]
mod uniform_tests;
pub mod uploads;
pub mod upscale;
pub mod wakes;
pub mod waves;
pub mod weather;
pub mod wind;
pub mod woods;
pub mod world;

pub use world::WorldPlugin;
