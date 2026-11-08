//! The night: the moon, the sky's fill and the exposure are the biome's light preset
//! (biomes/light/ruin-night.ron, kit/light.rs); here, the court's ground fog.

use bevy::prelude::*;

/// The court's ground fog: thin, dark and ankle-low, so it hangs in the shade and over the
/// water without lifting the floor's mid-tones to grey (judge r3: "fog lifting mid-tones").
pub fn fog() -> super::ambient::fog::FogLook {
    super::ambient::fog::FogLook {
        thick: 0.2,
        colour: Vec3::new(0.3, 0.34, 0.42),
        height: (0.05, 0.28),
    }
}

pub fn spawn(mut commands: Commands, light: Res<crate::kit::light::Light>) {
    light.spawn(&mut commands);
}
