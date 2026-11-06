//! The forest's fight: the knight's chop landing on one tree creature, his bolts striking a
//! second winding up its own blow, the tree monster fallen dead before him. Each body plays
//! one clip from a fixed time (the ruin's `Pose`), held there in a still.

use super::{FOE, KNIGHT, SECOND};
use crate::ruin::fight::Pose;
use crate::scene::model;
use crate::terrain::shape;
use bevy::prelude::*;

/// Model, clip, the clip's time, played once (and held at its end), place, and what it faces.
const CAST: [(&str, &str, f32, bool, Vec2, Vec2); 4] = [
    // The chop coming down, as in the ruin (ruin/fight.rs, effects::hit's `BLOW`).
    ("knight", "attack", 1.3, false, KNIGHT, FOE),
    // The bark the blow bites, raising its arms.
    ("treant-a", "attack", 0.5, false, FOE, KNIGHT),
    // The bolts' mark, swinging at him from the wood's side.
    ("treant-b", "attack", 0.9, false, SECOND, KNIGHT),
    // One already down at his feet, toward the camera.
    (
        "tree-monster",
        "death",
        2.9,
        true,
        Vec2::new(3.4, 8.4),
        Vec2::new(6.0, 4.0),
    ),
];

pub fn spawn(mut commands: Commands, asset: Res<AssetServer>) {
    for (name, clip, at, once, place, face) in CAST {
        let path = format!("models/iso/{name}.glb");
        let d = face - place;
        // The models face +Z.
        let body = model(
            &mut commands,
            &asset,
            &path,
            place.extend(shape(place)).xzy(),
            d.x.atan2(d.y),
            1.0,
        );
        commands.entity(body).insert((
            crate::rim::Fighter::default(),
            // The oaks before the fight open about each of the cast (foliage.rs).
            crate::foliage::Seen,
            Pose {
                gltf: asset.load(path),
                clip,
                at,
                once,
            },
        ));
    }
}
