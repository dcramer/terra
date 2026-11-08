//! The forest demo (iso-forest-demo): what play needs to make a run and its fights read, kept
//! apart from the hero's own body and moves (play.rs, play/) so the two lanes do not collide
//! (D635). Each part its own module: `occlude` keeps the hero in view, `steer` walks the
//! roster's foes to him so none is stuck.

pub mod occlude;
// Its plain part only so far, played by its test; the bodies take it next (mobs.rs).
#[allow(dead_code)]
pub mod steer;
pub mod tally;

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((occlude::plugin, tally::plugin));
}
