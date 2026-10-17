//! Generation toolkit: seeds, repeatable random streams, noise and the hash mixers
//! (`hash`) every crate uses.
//!
//! Everything generated in the game is a function of a seed and a position, so only
//! changes are ever stored or sent (D41). That only works if every machine computes the
//! same thing, so nothing here depends on the platform: the arithmetic is integer mixing
//! and plain floating-point add and multiply, which behave identically everywhere. The
//! tests pin exact outputs, so any machine that runs `cargo xtask check` checks them; CI,
//! which is off and runs only when started by hand (D37), runs them on Linux, macOS and
//! Windows.

pub mod hash;
mod noise;
mod rng;
mod seed;

pub use noise::{Noise, layer_shift};
pub use rng::Rng;
pub use seed::Seed;
