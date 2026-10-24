//! How bodies move and fight, by rules every player and the server agree on (D447, D28):
//! no engine in it, so it tests in seconds and a server runs it without a window.
//!
//! - `moves`: move sets as data and the move runner: phases, cancels, the buffer, i-frames,
//!   and how a blow that meets a guard is judged.
//! - `blade`: a blade swept from tick to tick against a body's hurt capsule.
//! - `contact`: what a blow that reached a body does to both fighters, and the fighter's own
//!   state (health, guard, hit-stop, reeling, shoves) it changes.
//! - `stamina`: what a move costs and what a body too tired for it may still do.
//!
//! What shows (spawning, intent, clips, flashes, sounds) is `terra_creatures::melee`'s. The
//! crate grows into docs/movement.md's intent, mover and actions layers as they land.

pub mod blade;
pub mod contact;
pub mod moves;
pub mod stamina;

#[cfg(test)]
mod duel;
