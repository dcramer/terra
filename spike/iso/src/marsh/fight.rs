//! The marsh's fight: the knight's chop landing on the ogre, his bolts striking a treant
//! winding up its blow (the tree monster wading behind them is life.rs's). Each body plays
//! one clip from a fixed time (the ruin's `Pose`), held there in a still.

use super::land::shape;
use super::{FOE, KNIGHT, LEVEL, TARGET};
use crate::ruin::fight::Pose;
use crate::scene::model;
use bevy::prelude::*;

/// Model, clip, the clip's time, place, and what it faces.
const CAST: [(&str, &str, f32, Vec2, Vec2); 3] = [
    // The chop coming down, as in the ruin (effects::hit's `BLOW`).
    ("knight", "attack", 1.3, KNIGHT, FOE),
    // The ogre the blow bites, its own swing just begun.
    ("ogre", "attack", 0.45, FOE, KNIGHT),
    // The bolts' mark, swinging at him from the left.
    ("treant-a", "attack", 0.9, TARGET, KNIGHT),
];

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn);
}

/// How deep a body stands in water at most (its feet on the mud under the face).
pub fn footing(p: Vec2, wade: f32) -> f32 {
    shape(p).max(LEVEL - wade)
}

fn spawn(mut commands: Commands, asset: Res<AssetServer>) {
    for (name, clip, at, place, face) in CAST {
        let path = format!("models/iso/{name}.glb");
        let d = face - place;
        // The models face +Z.
        let body = model(
            &mut commands,
            &asset,
            &path,
            place.extend(footing(place, 0.35)).xzy(),
            d.x.atan2(d.y),
            1.0,
        );
        commands.entity(body).insert(Pose {
            gltf: asset.load(path),
            clip,
            at,
            once: false,
        });
    }
}
