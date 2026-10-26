//! How bodies move and fight, by rules every player and the server agree on (D447, D28):
//! no engine in it, so it tests in seconds and a server runs it without a window.
//!
//! - `hold`: what each hand holds, the stance the pair gives, and the move set it plays,
//!   looked up by class and stance with the fallback (docs/holding.md, D459).
//! - `moves`: move sets as data and the move runner: phases, cancels, the buffer, i-frames,
//!   and how a blow that meets a guard is judged.
//! - `course` (feature `course`): the controller's plain-Rust course, analytic fixtures and the
//!   numbers its criteria are judged by (D53, `cargo xtask controller`).
//! - `mover`: the character controller every body is moved by (D53, from Jolt's
//!   `CharacterVirtual`): collide and slide, steps, the floor, ground states, pushing by mass.
//! - `blade`: a blade swept from tick to tick against a body's hurt capsule.
//! - `contact`: what a blow that reached a body does to both fighters, and the fighter's own
//!   state (health, guard, hit-stop, reeling, shoves) it changes.
//! - `feet`: feet locked where they land while a move drives or turns the body over them,
//!   stepping when it has gone too far, and the two-bone solve that bends a leg to them.
//! - `placement`: feet raised onto the ground under them (foot IK), probed once a lock.
//! - `stamina`: what a move costs and what a body too tired for it may still do.
//! - `struck`: what a blow meets (steel, wood, flesh) and how heavy it is, as it is heard.
//! - `script`: a fight script, the intents a scripted fighter gives on times or on what the
//!   fight is doing (`--duel`).
//!
//! What shows (spawning, intent, clips, flashes, sounds) is `terra_creatures::melee`'s. The
//! crate grows into docs/movement.md's intent, mover and actions layers as they land.

pub mod blade;
pub mod contact;
#[cfg(any(test, feature = "course"))]
pub mod course;
pub mod distance;
pub mod feet;
pub mod hold;
pub mod mover;
pub mod moves;
pub mod placement;
pub mod script;
pub mod stamina;
pub mod struck;

#[cfg(test)]
mod duel;
