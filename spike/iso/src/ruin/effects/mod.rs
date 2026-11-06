//! The seam for the fight's effects (the next job's): spells, hits, sparks, dust, ground fog,
//! fireflies, wind. Empty but for where the blow lands: a `Strike` between the hero's blade
//! and the ogre, for a hit effect to find.

use bevy::prelude::*;

/// Where the hero's blow meets the ogre, a metre and a half up.
#[derive(Component)]
pub struct Strike;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn((Strike, Transform::from_xyz(1.0, 1.5, -0.7)));
    });
}
