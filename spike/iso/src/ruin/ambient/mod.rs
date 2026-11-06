//! The ruin's air and ground between the blows, as the ARPGs fill theirs (Diablo IV's
//! haze, V Rising's fog, embers and dust in the light pools, wind in the grass, water with
//! the fire in it): fog.rs low fog in drifts. Each moves on the effects' clock, so a still
//! is one moment and a film plays.

mod fog;

use bevy::prelude::*;

/// The night's wind across the court (x, z), as the smoke and embers lean.
pub const WIND: Vec2 = Vec2::new(0.86, 0.5);

pub fn plugin(app: &mut App) {
    app.add_plugins(fog::plugin);
}
