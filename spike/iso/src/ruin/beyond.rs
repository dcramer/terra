//! What lies beyond the court, out to the frame's edges: boulders and stones of the game's
//! rock set (assets/models/nature/Rock_*.glb, the mossed ones in the shade), more dead trees,
//! and (walls.rs) the fragments of the outer walls, so the frame fills as the refs' do and
//! frames the fight, never a margin of bare earth.

use super::ground::{TOP, paved};
use crate::forest::Rng;
use crate::scene::model;
use bevy::prelude::*;

/// Rock sets by size: the large boulders, the knee-high stones and the small ones.
/// Only the mossed set: the bare Rock_1..10 render near white under the moon (pale scans,
/// scene-r2/step3 first try), white blobs in a night frame.
const BIG: [&str; 5] = [
    "Rock_Moss_1",
    "Rock_Moss_2",
    "Rock_Moss_3",
    "Rock_Moss_4",
    "Rock_Moss_5",
];
const MID: [&str; 3] = ["Rock_Moss_3", "Rock_Moss_4", "Rock_Moss_5"];
const SMALL: [&str; 3] = ["Rock_Moss_1", "Rock_Moss_2", "Rock_Moss_5"];

/// Clusters of rock: where, how far they spread, how many of each size, how big.
const CLUSTERS: [(Vec2, f32, usize, usize, f32); 9] = [
    (Vec2::new(-13.5, -10.0), 2.5, 2, 3, 0.9),
    (Vec2::new(-12.5, 2.0), 2.0, 1, 3, 0.8),
    (Vec2::new(-11.5, 6.0), 2.0, 1, 2, 0.7),
    (Vec2::new(12.8, -9.5), 2.5, 2, 3, 0.9),
    (Vec2::new(13.5, -2.5), 2.0, 1, 3, 0.8),
    (Vec2::new(13.0, 4.0), 2.0, 1, 2, 0.7),
    (Vec2::new(3.5, -11.5), 3.0, 1, 3, 0.8),
    (Vec2::new(-5.5, -11.0), 3.0, 1, 2, 0.7),
    (Vec2::new(-1.0, 7.5), 2.5, 0, 2, 0.6),
];

/// Dead trees at the frame's edges, beside dress.rs's two.
const TREES: [(&str, Vec2, f32, f32); 3] = [
    ("models/iso/dead-tree-b.glb", Vec2::new(-14.5, -2.5), 250.0, 0.9),
    ("models/iso/dead-tree-a.glb", Vec2::new(8.0, -12.0), 100.0, 1.1),
    ("models/iso/dead-tree-a.glb", Vec2::new(14.5, 1.0), 300.0, 0.8),
];

pub fn spawn(mut commands: Commands, asset: Res<AssetServer>) {
    let mut rng = Rng::new(0xb0_17de5);
    for (at, spread, big, mid, scale) in CLUSTERS {
        let mut put = |set: &[&str], size: f32, spread: f32, rng: &mut Rng| {
            let name = set[(rng.f() * set.len() as f32) as usize % set.len()];
            let p = at + Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)) * spread;
            // Set into the earth, not on it.
            let sunk = -0.12 * size;
            let path = format!("models/nature/{name}.glb");
            let yaw = rng.range(0.0, std::f32::consts::TAU);
            model(&mut commands, &asset, &path, p.extend(sunk).xzy(), yaw, size);
        };
        for _ in 0..big {
            put(&BIG, scale * rng.range(0.7, 1.1), spread * 0.4, &mut rng);
        }
        for _ in 0..mid {
            put(&MID, scale * rng.range(0.5, 0.9), spread, &mut rng);
        }
        for _ in 0..mid * 2 {
            put(&SMALL, rng.range(0.12, 0.28), spread * 1.3, &mut rng);
        }
    }
    // Stones strewn over the earth and the broken paving's edge.
    let mut strewn = 0;
    while strewn < 40 {
        let p = Vec2::new(rng.range(-16.0, 16.0), rng.range(-13.0, 8.0));
        if paved(p) > 0.5 {
            continue;
        }
        strewn += 1;
        let name = SMALL[(rng.f() * 3.0) as usize % 3];
        let path = format!("models/nature/{name}.glb");
        let yaw = rng.range(0.0, std::f32::consts::TAU);
        let size = rng.range(0.1, 0.3);
        model(&mut commands, &asset, &path, Vec3::new(p.x, -0.03, p.y), yaw, size);
    }
    for (path, at, yaw, scale) in TREES {
        model(
            &mut commands,
            &asset,
            path,
            at.extend(TOP).xzy(),
            yaw.to_radians(),
            scale,
        );
    }
}
