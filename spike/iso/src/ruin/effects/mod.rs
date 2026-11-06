//! The fight's effects, made as the ARPGs make theirs (research/2026-11-05-iso-arpg-art-pipeline.md):
//! emissive cores over bloom, flipbook flames, ribbon trails, spark streaks, and a light in
//! every big effect. kit.rs holds what they share (the clock, bursts of sparks, textures);
//! flame.rs the fires, spell.rs the hero's bolts, hit.rs his blow on the ogre.

mod flame;
mod hit;
pub mod kit;
mod spell;

use bevy::prelude::*;

/// Where the hero's blow meets the ogre, a metre and a half up.
pub const STRIKE: Vec3 = Vec3::new(1.0, 1.5, -0.7);

/// Where the hero's blow meets the ogre.
#[derive(Component)]
pub struct Strike;

pub fn plugin(app: &mut App) {
    app.add_plugins((kit::plugin, flame::plugin, spell::plugin, hit::plugin))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn((Strike, Transform::from_translation(STRIKE)));
        });
}
