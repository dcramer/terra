//! The ruin's air and ground between the blows, as the ARPGs fill theirs (Diablo IV's
//! haze, V Rising's fog, embers and dust in the light pools, wind in the grass): fog.rs low
//! fog in drifts, dust.rs motes in the firelight and fireflies, grass.rs the wind in the
//! dead grass (and its material for cloth), banner.rs a torn banner on the wind. Each moves
//! on the effects' clock, so a still is one moment and a film plays.

mod banner;
pub mod dust;
pub mod fog;
pub mod grass;

use bevy::prelude::*;

/// The night's wind across the court (x, z), as the smoke and embers lean.
pub const WIND: Vec2 = Vec2::new(0.86, 0.5);

pub fn plugin(app: &mut App) {
    app.add_plugins((
        fog::plugin,
        dust::plugin,
        grass::plugin,
        banner::plugin,
    ));
}
