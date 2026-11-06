//! The fight's effects, made as the ARPGs make theirs (research/2026-11-05-iso-arpg-art-pipeline.md):
//! emissive cores over bloom, flipbook flames, ribbon trails, spark streaks, and a light in
//! every big effect. kit.rs holds what they share (the clock, bursts of sparks, textures);
//! flame.rs the fires, spell.rs the hero's bolts, hit.rs his blow on the ogre, slash.rs his
//! sword's trail through the chop, mark.rs the marks both leave on the ground.

mod flame;
mod hit;
pub mod kit;
mod mark;
mod slash;
mod spell;

use bevy::prelude::*;

/// Where the fight stands, for the effects to aim at and find the bodies by: the hero, the
/// foe his blow lands on, the foe his bolts fly at (each a place on the floor), and the
/// floor's height. The ruin's is the default (fight.rs's CAST); another scene inserts its
/// own before Startup.
#[derive(Resource, Clone, Copy)]
pub struct Stage {
    pub knight: Vec2,
    pub foe: Vec2,
    pub target: Vec2,
    pub floor: f32,
}

impl Default for Stage {
    fn default() -> Self {
        Stage {
            knight: Vec2::ZERO,
            foe: Vec2::new(2.0, -1.4),
            target: Vec2::new(-2.8, -3.4),
            floor: 0.0,
        }
    }
}

impl Stage {
    /// Where the hero's blow meets the foe: halfway to it, a metre and a half up.
    pub fn strike(&self) -> Vec3 {
        let m = (self.knight + self.foe) / 2.0;
        Vec3::new(m.x, self.floor + 1.5, m.y)
    }
    /// The hero's off hand, where his bolts leave.
    pub fn hero(&self) -> Vec3 {
        Vec3::new(self.knight.x, self.floor + 1.35, self.knight.y)
    }
    /// The bark the bolts strike.
    pub fn bark(&self) -> Vec3 {
        Vec3::new(self.target.x, self.floor + 1.5, self.target.y)
    }
    /// The ground under `p`: the scene's land where it has one (the forest, the marsh), else
    /// the stage's floor (the ruin's paving).
    pub fn ground(&self, land: Option<&crate::terrain::Land>, p: Vec2) -> Vec3 {
        let y = land.map_or(self.floor, |l| l.height(p));
        Vec3::new(p.x, y, p.y)
    }
}

/// Where the hero's blow meets the ogre.
#[derive(Component)]
pub struct Strike;

pub fn plugin(app: &mut App) {
    app.init_resource::<Stage>()
        .add_plugins((kit::plugin, flame::plugin, spell::plugin, hit::plugin, slash::plugin, mark::plugin))
        .add_systems(Startup, |mut commands: Commands, stage: Res<Stage>| {
            commands.spawn((Strike, Transform::from_translation(stage.strike())));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ruin's stage puts the blow where the ruin's knight and ogre meet, as it stood
    /// before the stage was data.
    #[test]
    fn the_ruins_strike_is_between_knight_and_ogre() {
        assert!(Stage::default().strike().distance(Vec3::new(1.0, 1.5, -0.7)) < 1e-5);
    }
}
