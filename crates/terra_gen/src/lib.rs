//! Generation toolkit: seeds, repeatable random streams and noise.
//!
//! Everything generated in the game is a function of a seed and a position, so only
//! changes are ever stored or sent (D41). That only works if every machine computes the
//! same thing, so nothing here depends on the platform: the arithmetic is integer mixing
//! and plain floating-point add and multiply, which behave identically everywhere. The
//! tests pin exact outputs, and CI runs them on Linux, macOS and Windows.

mod noise;
mod rng;
mod seed;

pub use noise::Noise;
pub use rng::Rng;
pub use seed::Seed;
