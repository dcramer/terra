//! The forest demo (iso-forest-demo): what play needs to make a run and its fights read, kept
//! apart from the hero's own body and moves (play.rs, play/) so the two lanes do not collide
//! (D635). Each part its own module: `occlude` keeps the hero in view, `steer` walks the
//! roster's foes to him so none is stuck, `blows` lands their swings on him.

pub mod blows;
pub mod mobs;
pub mod occlude;
pub mod steer;
pub mod tally;
pub mod tell;

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((occlude::plugin, tally::plugin, mobs::plugin, blows::plugin));
}
