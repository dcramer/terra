//! The second scene (`--scene ruin`): an action RPG's night fight in a ruined court, seen as
//! Diablo IV and Path of Exile 2 see theirs. A knight against an ogre and a tree monster on a
//! broken flagstone floor between fallen walls, lit by the moon and by braziers and torches.
//! The glade (the default scene) shares only its helpers.

mod blocks;
mod camera;
mod dress;
pub mod effects;
mod fight;
mod fire;
mod ground;
mod light;
mod walls;

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Startup,
        (
            camera::spawn,
            ground::spawn,
            walls::spawn,
            dress::spawn,
            light::spawn,
            fire::spawn,
            fight::spawn,
        ),
    )
    .add_plugins((fight::plugin, fire::plugin, effects::plugin));
}
