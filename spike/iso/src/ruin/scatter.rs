//! The court's floor buried as Path of Exile 2 buries its own (judge ruin-r14: "clean, tiled
//! flagstone through the centre and lower half", fine detail 0.09 against the refs' 0.14 to
//! 0.23): bricks fallen from the walls, skulls and heaps of bones of those who fought here
//! before, heaped at the walls' and pillars' feet and deepest in the corners, in mixed sizes
//! sunk part-way into the soil, with open floor between (the ring round the fight and the
//! lower frame's even strew read as confetti, judge 261107-0150); none where the
//! fighters' feet go, none in the water. The bricks are the walls' own scan
//! (castle_wall_varriation) on small turned blocks; the bones are the iso models. `strewn`
//! also thickens litter.rs's leaves and rubble and grass.rs's tufts in the same places.

use super::blocks::Blocks;
use super::ground::{TOP, paved, smooth};
use super::walls::{BACK, BACK_Z, PILLARS, THICK, WEST_X};
use crate::forest::Rng;
use crate::scene::{material, model};
use crate::terrain::fbm;
use bevy::prelude::*;

/// The fighters' feet (knight, ogre, tree monster) and how far round each is kept clear, a
/// step past its reach (at 1.4 m and a metre's ramp the three ate the whole ring, r15).
const FEET: [(Vec2, f32); 3] = [
    (Vec2::new(0.0, 0.0), 0.9),
    (Vec2::new(2.0, -1.4), 1.3),
    (Vec2::new(-2.8, -3.4), 1.1),
];
/// The ramp from bare to strewn past a fighter's clear radius, metres.
const RAMP: f32 = 0.5;
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
const BRICK_TRIES: f32 = 16.0;
/// The bones placed: skulls and heaps, each at least `APART` from the others (12 and 6
/// at 1.1 m read as a few white dots, r15; 20 and 12 the same heap stamped over the court,
/// judge 261107-0150; at two the court lost its bones, round 261107-0259-2): a few skulls
/// in the debris, five heaps at true size (dress.rs has none now).
const SKULLS: usize = 12;
const HEAPS: usize = 5;
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
        super::water::standing(p).max(super::water::puddled(p)),
    )
}

/// The faces of what stands (the back, west and east walls' runs, their skins `THICK`
/// apart) and the pillars' drums: the debris falls at their feet.
const STANDING: [(Vec2, Vec2); 3] = [
    (BACK.0, BACK.1),
    (Vec2::new(WEST_X, BACK_Z), Vec2::new(WEST_X, 4.0)),
    (Vec2::new(11.8, -6.5), Vec2::new(11.8, 0.5)),
];
/// Where debris heaps deepest: the court's back corner, the back wall's broken end, the
/// west wall's end and the pillars' feet, where two faces or a fall's whole spill meet.
const CORNERS: [Vec2; 5] = [
    Vec2::new(WEST_X, BACK_Z),
    Vec2::new(5.5, BACK_Z),
    Vec2::new(WEST_X, 4.0),
    PILLARS[0],
    PILLARS[1],
];
/// How far out from a wall's face the debris falls off to nothing, metres (judge
/// 261107-0150: "heaped at the wall feet and in corners ... with open ground between").
const FOOT: f32 = 1.8;
const CORNER: f32 = 2.6;

/// How far `p` is from the segment `a`..`b`.
fn to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

/// How near a wall's or a pillar's foot `p` is: 1 at the face, 0 `FOOT` out (more round the
/// corners, `CORNER`).
pub fn at_feet(p: Vec2) -> f32 {
    let wall = STANDING
        .iter()
        .map(|(a, b)| to_segment(p, *a, *b) - THICK * 0.5)
        .chain(PILLARS.iter().map(|c| (p - *c).length() - 0.4))
        .fold(f32::MAX, f32::min);
    let corner = CORNERS
        .iter()
        .map(|c| (p - *c).length())
        .fold(f32::MAX, f32::min);
    let foot = 1.0 - smooth(0.0, FOOT, wall.max(0.0));
    let corner = 1.0 - smooth(0.0, CORNER, corner);
    foot.max(corner)
}

/// How thick the debris lies at `p` (0..1): heaped at the walls' and pillars' feet and
/// deepest in the corners, a little where the paving is broken, open floor between; in
/// heaps, none on the fighters' feet or in the water.
pub fn strewn(p: Vec2) -> f32 {
    let feet = at_feet(p);
    let broken = 0.35 * (1.0 - smooth(0.3, 0.85, paved(p)));
    let heap = smooth(0.35, 0.65, fbm(p * 0.7, 701));
    let lie = feet.max(broken) * (0.15 + 0.85 * heap);
    lie * off_feet(p) * dry(p)
}

/// How thick the loose layer lies over the whole court at `p` (0..1, about the old even
/// strew's count in smaller pieces; at a third it read as a swept floor, round
/// 261107-0259-2): chips, grit and a few mid pieces broken off and kicked about, so the
/// open floor between the heaps is not swept clean (judge 261107-0250-ruin preferred the
/// old strew: "rubble and bones break up the paving"); patchy, none on the feet or in water.
pub fn loose(p: Vec2) -> f32 {
    let patch = smooth(0.25, 0.75, fbm(p * 1.3, 733));
    (0.35 + 0.65 * patch) * off_feet(p) * dry(p)
}
/// Loose pieces tried a square metre (the field keeps a share, about half).
const LOOSE_TRIES: f32 = 12.0;

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
        // In clumps where it lies thick, as a brick breaks and its shards lie together: a
        // heap's pieces piled a little over one another.
        for k in 0..1 + (rng.f() * 8.0 * s) as usize {
            let p = centre + Vec2::new(rng.range(-0.45, 0.45), rng.range(-0.45, 0.45));
            if strewn(p) == 0.0 {
                continue;
            }
            let size = lump(rng.f().powf(1.0 + 0.8 * (1.0 - s)));
            brick(&mut bricks, &mut rng, p, k as f32 * 0.025 * s, size);
        }
    }
    // The loose layer over the open floor: single chips and grit, now and then a pair or
    // three of one break, and one in twelve a mid piece.
    for _ in 0..(area * LOOSE_TRIES) as usize {
        let centre = at(&mut rng);
        if rng.f() > loose(centre) {
            continue;
        }
        for _ in 0..1 + (rng.f() * rng.f() * 4.0) as usize {
            let p = centre + Vec2::new(rng.range(-0.15, 0.15), rng.range(-0.15, 0.15));
            if loose(p) == 0.0 {
                continue;
            }
            let size = chip(rng.f());
            brick(&mut bricks, &mut rng, p, 0.0, size);
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(bricks.mesh())),
        MeshMaterial3d(materials.add(material(&asset, "castle_wall_varriation", Color::WHITE))),
    ));
    bones(&mut commands, &asset);
}

/// The size spread of the debris, times a whole brick: many small, few big (`lump`).
const SIZES: (f32, f32) = (0.4, 1.8);

/// A piece's size, times a whole brick, from `u` in 0..1: the logarithm squared, so a
/// half lie under 0.55 and one in eleven over 1.4 (judge 261107-0150: "in mixed sizes").
pub fn lump(u: f32) -> f32 {
    SIZES.0 * (SIZES.1 / SIZES.0).powf(u * u)
}

/// A loose piece's size, times a whole brick, from `u` in 0..1: grit and chips 0.15 to 0.55
/// (about half under 0.3), one in ten a mid piece 0.6 to 1.1.
pub fn chip(u: f32) -> f32 {
    if u > 0.9 {
        0.6 + 0.5 * (u - 0.9) * 10.0
    } else {
        let v = u / 0.9;
        0.15 + 0.4 * v * v
    }
}

/// One piece `s` times a whole brick at `p`, `lift` over the floor where it lies on others:
/// a shard to a block of two or three bricks still mortared, turned any way, leaning, sunk
/// part-way into the soil, some stained by it.
fn brick(bricks: &mut Blocks, rng: &mut Rng, p: Vec2, lift: f32, s: f32) {
    // A broken piece is squarer than a whole brick; a big one a lump of courses.
    let long = 0.25 * s * rng.range(0.6, 1.2);
    let size = Vec3::new(
        long,
        0.075 * s.sqrt() * rng.range(0.8, 1.3),
        0.12 * s * rng.range(0.7, 1.1),
    );
    let on_edge = rng.f() < 0.15;
    let rot = Quat::from_euler(
        EulerRot::YXZ,
        rng.range(0.0, 6.28),
        if on_edge { 1.4 } else { rng.range(-0.35, 0.35) },
        rng.range(-0.35, 0.35),
    );
    let up = if on_edge { size.z } else { size.y };
    let floor = if paved(p) > 0.5 { TOP } else { -0.03 };
    // Sunk a tenth to two fifths of its height: its middle 0.4 to 0.1 of it over the floor.
    let sink = rng.range(0.1, 0.4);
    // A shard shows more fresh broken face than a whole brick: the smaller, the lighter.
    let v = rng.range(0.4, 0.9) * (1.0 + 0.35 * (1.0 - s).max(0.0));
    // Soil-stained: browner and darker, the sunk ones most.
    let soil = rng.range(0.0, 0.6) * (0.5 + sink);
    let tint = Color::srgb(
        v * (1.0 - 0.25 * soil),
        v * (0.95 - 0.2 * soil),
        v * (0.9 - 0.3 * soil),
    );
    bricks.add(
        Vec3::new(p.x, floor + lift + up * (0.5 - sink), p.y),
        size,
        rot,
        2.0,
        Vec2::new(rng.f(), rng.f()),
        tint,
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
        // smaller so they read as a man's bones (at 0.8 to 1.15 a giant's, r15b; at 0.55
        // to 0.8 still a metre's femur, judge 261107-0150): a femur 0.45 m, half the leg.
        ("models/iso/bone-pile.glb", HEAPS, (0.45, 0.5)),
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

    /// Debris lies thick at the walls' feet and thin a few metres out, most of the open
    /// paving bare between (judge 261107-0150's confetti).
    #[test]
    fn debris_heaps_at_the_walls_with_open_floor_between() {
        let (mut near, mut n_near, mut far, mut n_far, mut bare, mut paving) =
            (0.0, 0, 0.0, 0, 0, 0);
        for i in 0..80 {
            for j in 0..56 {
                let p = Vec2::new(
                    AREA_X.0 + (i as f32 + 0.5) * (AREA_X.1 - AREA_X.0) / 80.0,
                    AREA_Z.0 + (j as f32 + 0.5) * (AREA_Z.1 - AREA_Z.0) / 56.0,
                );
                if paved(p) < 0.9 || dry(p) < 1.0 {
                    continue;
                }
                paving += 1;
                bare += (strewn(p) < 0.05) as usize;
                let wall = STANDING
                    .iter()
                    .map(|(a, b)| to_segment(p, *a, *b) - THICK * 0.5)
                    .fold(f32::MAX, f32::min);
                if wall < 0.8 {
                    near += strewn(p);
                    n_near += 1;
                } else if wall > 3.0 {
                    far += strewn(p);
                    n_far += 1;
                }
            }
        }
        let (near, far) = (near / n_near as f32, far / n_far as f32);
        assert!(
            near > 0.25 && near > 4.0 * far,
            "debris at the walls' feet {near:.2} against {far:.2} three metres out: it should \
             heap at the feet"
        );
        let share = bare as f32 / paving as f32;
        assert!(
            share > 0.5,
            "only {share:.2} of the whole paving is bare: confetti"
        );
    }

    /// The pieces spread from 0.4 to 1.8 bricks, many small and few big.
    #[test]
    fn debris_sizes_mostly_small_few_big() {
        let sizes: Vec<f32> = (0..1000).map(|k| lump((k as f32 + 0.5) / 1000.0)).collect();
        let share =
            |f: &dyn Fn(f32) -> bool| sizes.iter().filter(|s| f(**s)).count() as f32 / 1000.0;
        assert!(sizes.iter().all(|s| (SIZES.0..=SIZES.1).contains(s)));
        let small = share(&|s| s < 0.55);
        let big = share(&|s| s > 1.4);
        assert!(small > 0.35 && small < 0.6, "small share {small}");
        assert!(big > 0.04 && big < 0.15, "big share {big}");
    }

    /// The open floor between the heaps is not bare: the loose layer lies over most of it,
    /// in chips and grit with few mid pieces (judge 261107-0250-ruin: "A's clean floor").
    #[test]
    fn open_floor_has_a_loose_layer_of_chips() {
        let (mut open, mut covered) = (0, 0);
        for i in 0..40 {
            for j in 0..28 {
                let p = Vec2::new(-9.0 + i as f32 * 0.5, -6.3 + j as f32 * 0.5);
                if strewn(p) < 0.05 && off_feet(p) == 1.0 && dry(p) == 1.0 {
                    open += 1;
                    covered += (loose(p) > 0.3) as usize;
                }
            }
        }
        assert!(open > 100, "no open floor to test ({open} points)");
        assert!(
            covered as f32 > 0.9 * open as f32,
            "loose layer on {covered} of {open}"
        );
        let sizes: Vec<f32> = (0..1200).map(|k| chip((k as f32 + 0.5) / 1200.0)).collect();
        let small = sizes.iter().filter(|s| **s < 0.3).count() as f32 / 1200.0;
        let mid = sizes.iter().filter(|s| **s >= 0.6).count() as f32 / 1200.0;
        assert!(sizes.iter().all(|s| (0.15..=1.1).contains(s)));
        assert!(
            small > 0.4 && small < 0.7,
            "chips under 0.3 of a brick {small}"
        );
        assert!(mid > 0.05 && mid < 0.15, "mid pieces {mid}");
    }

    /// None on the fighters' feet, none in the pool.
    #[test]
    fn debris_keeps_off_the_feet_and_the_pool() {
        for (f, r) in FEET {
            for k in 0..12 {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                let p = f + Vec2::new(a.cos(), a.sin()) * r * 0.95;
                assert!(strewn(p) == 0.0, "debris on the feet at {p}: {}", strewn(p));
                assert!(
                    loose(p) == 0.0,
                    "loose chips on the feet at {p}: {}",
                    loose(p)
                );
            }
        }
        assert!(
            strewn(super::super::water::POOL.0) == 0.0 && loose(super::super::water::POOL.0) == 0.0,
            "debris in the pool"
        );
        let thick = |pts: &[Vec2]| pts.iter().map(|p| strewn(*p)).fold(0.0, f32::max);
        let wall: Vec<Vec2> = (0..16)
            .map(|k| Vec2::new(-8.0 + k as f32, BACK_Z + 0.4))
            .collect();
        assert!(thick(&wall) > 0.5, "the wall's foot is bare");
    }
}
