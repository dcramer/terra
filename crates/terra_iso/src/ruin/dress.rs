//! What the ruin holds: dead trees standing over the walls, a broken-down wagon and its
//! barrels at the east edge, a camp gone cold by the west wall, and bones and skulls about
//! the floor where others fought before. The iso models (assets/models/iso/, life size on
//! y=0) and a few of the game's own props.

use super::ground::TOP;
use crate::scene::model;
use bevy::prelude::*;

/// Model, place, yaw in degrees, and a tip over (degrees about X, for a barrel on its side).
const PROPS: [(&str, Vec2, f32, f32); 19] = [
    (
        "models/iso/dead-tree-a.glb",
        Vec2::new(-11.0, -8.4),
        20.0,
        0.0,
    ),
    (
        "models/iso/dead-tree-b.glb",
        Vec2::new(10.4, -6.2),
        140.0,
        0.0,
    ),
    // The wagon's long axis lies 45 degrees off X in its file: turned to lie along the
    // east edge, a little askew.
    (
        "models/iso/wagon.glb",
        Vec2::new(5.2, 3.2),
        45.0 + 15.0,
        0.0,
    ),
    ("models/iso/barrel.glb", Vec2::new(3.4, 3.6), 10.0, 0.0),
    ("models/iso/barrel.glb", Vec2::new(3.0, 4.3), 70.0, 0.0),
    ("models/iso/barrel.glb", Vec2::new(2.4, 3.3), 30.0, 90.0),
    ("models/iso/skull.glb", Vec2::new(1.4, 1.5), 200.0, 0.0),
    ("models/iso/skull.glb", Vec2::new(-1.7, -0.8), 60.0, 0.0),
    ("models/iso/skull.glb", Vec2::new(3.6, 0.6), 320.0, 0.0),
    (
        "models/props/barrow/Debris.glb",
        Vec2::new(-4.4, -5.4),
        0.0,
        0.0,
    ),
    (
        "models/props/barrow/UrnFallen.glb",
        Vec2::new(-7.8, 1.2),
        40.0,
        0.0,
    ),
    (
        "models/props/barrow/BoneHeap.glb",
        Vec2::new(-0.6, -4.8),
        0.0,
        0.0,
    ),
    ("models/props/Campfire.glb", Vec2::new(-6.2, 3.4), 0.0, 0.0),
    ("models/props/Chest.glb", Vec2::new(-7.9, -4.6), 75.0, 0.0),
    ("models/props/Sack.glb", Vec2::new(6.9, 3.6), 20.0, 0.0),
    // More of the fallen: a skull at the pool's bank, one by the east wall, a heap of
    // bones west of the fight and the walls' debris at the front edge.
    ("models/iso/skull.glb", Vec2::new(-3.0, 1.3), 140.0, 0.0),
    ("models/iso/skull.glb", Vec2::new(5.8, -4.6), 30.0, 0.0),
    (
        "models/props/barrow/BoneHeap.glb",
        Vec2::new(-4.4, 0.2),
        60.0,
        0.0,
    ),
    (
        "models/props/barrow/Debris.glb",
        Vec2::new(2.8, 5.6),
        90.0,
        0.0,
    ),
];

pub fn spawn(mut commands: Commands, asset: Res<AssetServer>) {
    for (path, at, yaw, tip) in PROPS {
        let mut place = at.extend(TOP).xzy();
        // A barrel on its side lies on its staves, its middle at its radius.
        if tip != 0.0 {
            place.y += 0.3;
        }
        let e = model(&mut commands, &asset, path, place, 0.0, 1.0);
        commands
            .entity(e)
            .insert(Transform::from_translation(place).with_rotation(
                Quat::from_rotation_y(yaw.to_radians()) * Quat::from_rotation_x(tip.to_radians()),
            ));
    }
}
