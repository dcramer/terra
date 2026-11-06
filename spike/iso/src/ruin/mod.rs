//! The second scene (`--scene ruin`): an action RPG's night fight in a ruined court, seen as
//! Diablo IV and Path of Exile 2 see theirs. A knight against an ogre and a tree monster on a
//! broken flagstone floor between fallen walls, lit by the moon and by braziers and torches.
//! The glade (the default scene) shares only its helpers.

pub mod ambient;
mod beyond;
mod blocks;
pub mod camera;
mod dress;
pub mod effects;
pub mod fight;
mod fire;
mod flags;
mod grass;
mod ground;
mod held;
mod life;
mod light;
mod litter;
mod scatter;
mod walls;
mod water;

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.insert_resource(light::fog())
        .add_systems(
            Startup,
            (
                camera::spawn,
                ground::spawn,
                walls::spawn,
                dress::spawn,
                beyond::spawn,
                grass::spawn,
                light::spawn,
                fire::spawn,
                fight::spawn,
                water::spawn,
                litter::spawn,
                scatter::spawn,
            ),
        )
        .add_plugins((
            crate::pool::plugin,
            flags::plugin,
            fight::plugin,
            fire::plugin,
            effects::plugin,
            ambient::plugin,
            life::plugin,
        ));
}
