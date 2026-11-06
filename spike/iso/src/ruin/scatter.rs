//! The court's floor buried as Path of Exile 2 buries its own (judge ruin-r14: "clean, tiled
//! flagstone through the centre and lower half", fine detail 0.09 against the refs' 0.14 to
//! 0.23): bricks fallen from the walls, skulls and heaps of bones of those who fought here
//! before, dense at the walls' feet, in a ring round the fight two to four and a half metres
//! out and over the lower frame, which the walls' drifts never reach; none where the
//! fighters' feet go, none in the water. The bricks are the walls' own scan
//! (castle_wall_varriation) on small turned blocks; the bones are the iso models. `strewn`
//! also thickens litter.rs's leaves and rubble and grass.rs's tufts in the same places.

use super::blocks::Blocks;
use super::ground::{TOP, paved, smooth};
use super::walls::{BACK_Z, WEST_X};
use crate::forest::Rng;
use crate::scene::{material, model};
use crate::terrain::fbm;
use bevy::prelude::*;

/// Where the fight's middle is: the ring is measured from it.
const FIGHT: Vec2 = Vec2::new(0.0, -1.4);
/// The fighters' feet (knight, ogre, tree monster) and how far round each is kept clear, a
/// step past its reach (at 1.4 m and a metre's ramp the three ate the whole ring, r15).
const FEET: [(Vec2, f32); 3] = [
    (Vec2::new(0.0, 0.0), 0.9),
    (Vec2::new(2.0, -1.4), 1.3),
    (Vec2::new(-2.8, -3.4), 1.1),
];
/// The ramp from bare to strewn past a fighter's clear radius, metres.
const RAMP: f32 = 0.5;
/// The lower frame at the ARPG camera (world x and z), where the walls' drifts never reach.
const LOWER_X: (f32, f32) = (-7.0, 9.0);
const LOWER_Z: (f32, f32) = (1.0, 7.0);
/// What already stands on the floor (braziers, wagon, barrels, camp, pillars, dress.rs's
/// bones and chest), each with the radius a skull or heap keeps from it.
const TAKEN: [(Vec2, f32); 14] = [
    (Vec2::new(-1.7, 0.9), 0.8),
    (Vec2::new(3.4, -2.9), 0.8),
    (Vec2::new(5.2, 3.2), 1.8),
    (Vec2::new(3.0, 3.8), 1.1),
    (Vec2::new(-6.2, 3.4), 1.1),
    (Vec2::new(6.6, -3.2), 0.9),
    (Vec2::new(7.2, 1.6), 0.9),
    (Vec2::new(-6.6, -5.0), 1.0),
    (Vec2::new(5.0, -0.6), 1.0),
    (Vec2::new(-0.6, -4.8), 1.0),
    (Vec2::new(-4.4, 0.2), 1.0),
    (Vec2::new(-7.9, -4.6), 0.9),
    (Vec2::new(2.8, 5.6), 1.0),
    (Vec2::new(6.9, 3.6), 0.7),
];
/// Where bricks are tried, and how many a square metre; the field keeps a share.
const AREA_X: (f32, f32) = (-9.0, 11.0);
const AREA_Z: (f32, f32) = (-6.3, 7.5);
const BRICK_TRIES: f32 = 10.0;
/// The bones placed: skulls and heaps, each at least `APART` from the others (12 and 6
/// at 1.1 m read as a few white dots, r15).
const SKULLS: usize = 20;
const HEAPS: usize = 12;
const APART: f32 = 0.9;

/// How clear the fighters' feet are kept: 0 within a fighter's radius, 1 `RAMP` past it.
fn off_feet(p: Vec2) -> f32 {
    FEET.iter()
        .map(|(f, r)| smooth(*r, r + RAMP, (p - *f).length()))
        .fold(1.0, f32::min)
}

/// How dry `p` is: nothing lies in the pool or the puddles.
fn dry(p: Vec2) -> f32 {
    1.0 - smooth(
        0.05,
        0.3,
        super::water::pool(p).max(super::water::puddled(p)),
    )
}

/// How thick the debris lies at `p` (0..1): at the walls' feet, in a ring 2 to 4.5 m off the
/// fight and over the lower frame, in heaps; none on the fighters' feet or in the water.
pub fn strewn(p: Vec2) -> f32 {
    let walls = 1.0 - smooth(0.0, 2.5, (p.y - BACK_Z).min(p.x - WEST_X).max(0.0));
    let r = (p - FIGHT).length();
    let ring = smooth(1.8, 2.4, r) * (1.0 - smooth(4.2, 5.0, r));
    let inside = (p.x - LOWER_X.0)
        .min(LOWER_X.1 - p.x)
        .min(p.y - LOWER_Z.0)
        .min(LOWER_Z.1 - p.y);
    let lower = smooth(-0.5, 0.5, inside);
    let heap = smooth(0.35, 0.65, fbm(p * 0.7, 701));
    let lie = walls.max(ring).max(0.85 * lower) * (0.45 + 0.55 * heap);
    lie * off_feet(p) * dry(p)
}

/// Whether a skull or heap at `p` stands clear of what is already there and of `placed`.
fn free(p: Vec2, placed: &[Vec2]) -> bool {
    TAKEN.iter().all(|(at, r)| (p - *at).length() > *r)
        && placed.iter().all(|q| (p - *q).length() > APART)
        && paved(p) > 0.5
}

pub fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let at =
        |rng: &mut Rng| Vec2::new(rng.range(AREA_X.0, AREA_X.1), rng.range(AREA_Z.0, AREA_Z.1));

    // Bricks fallen from the walls: whole ones, halves and shards, some on edge.
    let mut bricks = Blocks::default();
    let mut rng = Rng::new(0xb_71c5);
    let area = (AREA_X.1 - AREA_X.0) * (AREA_Z.1 - AREA_Z.0);
    for _ in 0..(area * BRICK_TRIES) as usize {
        let centre = at(&mut rng);
        let s = strewn(centre);
        if rng.f() > s {
            continue;
        }
        // In clumps where it lies thick, as a brick breaks and its shards lie together.
        for _ in 0..1 + (rng.f() * 3.0 * s) as usize {
            let p = centre + Vec2::new(rng.range(-0.25, 0.25), rng.range(-0.25, 0.25));
            if strewn(p) == 0.0 {
                continue;
            }
            brick(&mut bricks, &mut rng, p);
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(bricks.mesh())),
        MeshMaterial3d(materials.add(material(&asset, "castle_wall_varriation", Color::WHITE))),
    ));
    bones(&mut commands, &asset);
}

/// One brick at `p`: whole, a half or a shard, lying flat or on its edge.
fn brick(bricks: &mut Blocks, rng: &mut Rng, p: Vec2) {
    let whole = rng.f() < 0.45;
    let long = if whole {
        rng.range(0.22, 0.28)
    } else {
        rng.range(0.08, 0.16)
    };
    let size = Vec3::new(long, rng.range(0.06, 0.09), rng.range(0.1, 0.13));
    let on_edge = rng.f() < 0.2;
    let rot = Quat::from_euler(
        EulerRot::YXZ,
        rng.range(0.0, 6.28),
        if on_edge { 1.4 } else { rng.range(-0.2, 0.2) },
        rng.range(-0.2, 0.2),
    );
    let up = if on_edge { size.z } else { size.y };
    let floor = if paved(p) > 0.5 { TOP } else { -0.03 };
    let v = rng.range(0.55, 0.95);
    bricks.add(
        Vec3::new(p.x, floor + up * 0.42, p.y),
        size,
        rot,
        2.0,
        Vec2::new(rng.f(), rng.f()),
        Color::srgb(v, v * 0.95, v * 0.9),
    );
}

/// Skulls and heaps of bones, where the debris is thick and the floor free.
fn bones(commands: &mut Commands, asset: &AssetServer) {
    let at =
        |rng: &mut Rng| Vec2::new(rng.range(AREA_X.0, AREA_X.1), rng.range(AREA_Z.0, AREA_Z.1));
    let mut placed: Vec<Vec2> = Vec::new();
    let mut rng = Rng::new(0x5_c011);
    for (path, count, scale) in [
        // The heap's file is a metre across, its femurs as long as the knight's leg: drawn
        // smaller so they read as a man's bones (at 0.8 to 1.15 a giant's, r15b).
        ("models/iso/bone-pile.glb", HEAPS, (0.55, 0.8)),
        ("models/iso/skull.glb", SKULLS, (0.85, 1.1)),
    ] {
        let mut left = count;
        for _ in 0..4000 {
            if left == 0 {
                break;
            }
            let p = at(&mut rng);
            if rng.f() > strewn(p) || !free(p, &placed) {
                continue;
            }
            placed.push(p);
            left -= 1;
            let s = rng.range(scale.0, scale.1);
            model(
                commands,
                asset,
                path,
                p.extend(TOP).xzy(),
                rng.range(0.0, 6.28),
                s,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Debris lies thick at the walls, in the ring round the fight and in the lower frame;
    /// none on the fighters' feet, none in the pool.
    #[test]
    fn debris_rings_the_fight_and_keeps_off_its_feet() {
        for (f, r) in FEET {
            for k in 0..12 {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                let p = f + Vec2::new(a.cos(), a.sin()) * r * 0.95;
                assert!(strewn(p) == 0.0, "debris on the feet at {p}: {}", strewn(p));
            }
        }
        assert!(
            strewn(super::super::water::POOL.0) == 0.0,
            "debris in the pool"
        );
        // Somewhere in the ring, at the wall and in the lower frame it lies thick.
        let thick = |pts: &[Vec2]| pts.iter().map(|p| strewn(*p)).fold(0.0, f32::max);
        let ring: Vec<Vec2> = (0..24)
            .map(|k| {
                let a = k as f32 / 24.0 * std::f32::consts::TAU;
                FIGHT + Vec2::new(a.cos(), a.sin()) * 3.3
            })
            .collect();
        assert!(thick(&ring) > 0.5, "the ring is bare: {}", thick(&ring));
        let wall: Vec<Vec2> = (0..16)
            .map(|k| Vec2::new(-8.0 + k as f32, BACK_Z + 0.4))
            .collect();
        assert!(thick(&wall) > 0.5, "the wall's foot is bare");
        let lower: Vec<Vec2> = (0..16).map(|k| Vec2::new(-6.0 + k as f32, 5.0)).collect();
        assert!(thick(&lower) > 0.5, "the lower frame is bare");
    }
}
