//! The fourth scene (`--scene marsh`): the third biome, a marsh at dusk, seen from the
//! ruin's action-RPG camera. Mud banks and mossy hummocks between open channels of still
//! dark water (pool.rs), reeds and grass in the wind, dead trees standing in the water, low
//! fog, midges and fireflies, will-o'-wisps flickering over the channels, a camp fire and a
//! lantern on a post for the warm pools. The knight fights an ogre and a treant on a mud
//! bank while a tree monster wades in; herons fish the shallows and mallards swim.
//!
//! The glade's ground layers draw the land (terrain.rs's `Land` on this scene's shape).

mod dress;
mod fight;
mod land;
mod lawn;
mod life;
mod light;
mod lips;
mod plants;
mod wisps;

use crate::ruin::effects::Stage;
use bevy::prelude::*;

/// The water's face.
pub const LEVEL: f32 = 0.0;
/// The evening's wind (x, z), as the ruin's (its ambient files read their own, the same).
pub const WIND: Vec2 = Vec2::new(0.86, 0.5);
/// Where the knight stands, the foe his blow lands on and the one his bolts fly at.
pub const KNIGHT: Vec2 = Vec2::new(0.0, 0.0);
pub const FOE: Vec2 = Vec2::new(1.9, -1.3);
pub const TARGET: Vec2 = Vec2::new(-3.2, -2.5);

pub fn stage() -> Stage {
    Stage {
        knight: KNIGHT,
        foe: FOE,
        target: TARGET,
        floor: land::shape(KNIGHT),
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(stage())
        .insert_resource(light::fog())
        .insert_resource(crate::terrain::Land::from_shape(land::shape))
        .add_plugins((
            crate::pool::plugin,
            crate::ruin::effects::plugin,
            crate::ruin::fight::plugin,
            land::plugin,
            dress::plugin,
            light::plugin,
            fight::plugin,
            plants::plugin,
            lips::plugin,
            wisps::plugin,
            life::plugin,
            // The ruin's low fog, firelit dust and fireflies, and its wind material.
            crate::ruin::ambient::fog::plugin,
            crate::ruin::ambient::dust::plugin,
            crate::ruin::ambient::grass::plugin,
        ))
        // The ground's clumps, on the forest's foliage material.
        .add_plugins((crate::foliage::plugin, lawn::plugin))
        .add_systems(Startup, camera)
        .add_systems(Update, crate::motes::face_halo);
}

/// The ruin's camera on the middle of the fight, its exposure and the dusk haze over the far
/// marsh the biome's light preset's.
fn camera(
    mut commands: Commands,
    args: Res<crate::Args>,
    light: Res<crate::kit::light::Light>,
    mut images: ResMut<Assets<Image>>,
) {
    let mid = (KNIGHT * 2.0 + FOE + TARGET) / 4.0;
    let focus = mid.extend(land::shape(mid).max(LEVEL) + 1.2).xzy();
    let cam = crate::ruin::camera::arpg(&mut commands, &args, &mut images, focus, light.ev100);
    light.camera(&mut commands, cam, crate::ruin::camera::DISTANCE);
}
