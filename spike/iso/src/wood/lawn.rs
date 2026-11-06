//! The forest's lawns as a ground cover of clumps (the lead's blind judge, 2026-11-06: "a
//! flat, lurid, overexposed yellow-green carpet; le-river's ferns have dark depth and
//! shade"): broad-leaved rosettes, fern fronds and grass tufts of varied height and hue,
//! each leaf dark at its foot (the clump's own occlusion) over darker leaves lying flat
//! beneath, blue-green under the oaks, warmer in the open, a few flowers in the sun. As
//! Last Epoch's and Path of Exile 2's undergrowth is built: masses of cards over the ground
//! whose gaps fall to near black. The fight's trodden ground stays bare, so the cast reads.

use super::clearing::trodden;
use crate::forest::{BUTTERCUP, CAMPION, Cards, DAISY, Rng, Spot};
use crate::terrain::{self, Land, smooth};
use bevy::prelude::*;

/// The atlas's cells (materials/foliage/cards.png): a broad leaf, a fern frond (diagonal in
/// its cell, its foot at the lower left), a grass tuft.
const LEAF: Vec2 = Vec2::new(0.5, 0.25);
const FROND: Vec2 = Vec2::new(0.25, 0.25);
const TUFT: Vec2 = Vec2::new(0.75, 0.75);
/// The ground the lawn is tried over (world x, z: the frame and a margin) and how many
/// clumps a square metre are tried.
const LO: Vec2 = Vec2::new(-12.0, -10.0);
const HI: Vec2 = Vec2::new(20.0, 17.0);
const TRIES: f32 = 7.0;
/// How far the lawn keeps from each of the cast (m), beyond the trodden ground.
const CAST: f32 = 2.2;
/// A leaf's foot over its tip: the clump's own occlusion.
const FOOT: f32 = 0.2;
/// The leaves lying flat under a clump, over its tip's tint.
const UNDER: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Leaves,
    Fern,
    Tuft,
}

/// One clump: where, how wide (m), its kind, its tint and whether a flower stands in it.
#[derive(Clone, Copy, Debug)]
pub struct Clump {
    pub at: Vec2,
    pub radius: f32,
    pub kind: Kind,
    pub tint: Vec3,
    pub flower: Option<Vec2>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn.after(crate::scene::build));
}

/// Whether the lawn may grow at `p`: dry, gentle, untrodden, off the camp's bare ground and
/// clear of the cast, the oaks' feet and the moose's walk (dress::free).
pub fn grows(land: &Land, p: Vec2) -> bool {
    let cast = [super::KNIGHT, super::FOE, super::TARGET, super::SECOND];
    super::dress::free(p)
        && trodden(p) < 0.02
        && terrain::bare(p) < 0.35
        && land.slope(p) < 0.6
        && cast.iter().all(|c| c.distance(p) > CAST)
}

/// The lawn's clumps, the same every run.
pub fn clumps(land: &Land, trees: &[Spot]) -> Vec<Clump> {
    let mut rng = Rng::new(0x1a_3e5);
    let area = (HI.x - LO.x) * (HI.y - LO.y);
    let mut out = Vec::new();
    for _ in 0..(area * TRIES) as usize {
        let p = LO + Vec2::new(rng.f(), rng.f()) * (HI - LO);
        // In masses with a few gaps, never an even lawn.
        let mass = terrain::fbm(p * 0.45, 71);
        if rng.f() > 0.55 + 0.45 * smooth(0.3, 0.6, mass) || !grows(land, p) {
            continue;
        }
        // Shade under the oaks (scene::canopy: 1 in the open) turns the green blue and dark.
        let open = smooth(0.55, 0.95, crate::scene::canopy(p, trees));
        let hue = terrain::fbm(p * 0.21, 73);
        let deep = Vec3::new(0.34, 0.56, 0.5).lerp(Vec3::new(0.4, 0.6, 0.38), hue);
        let warm = Vec3::new(0.7, 0.78, 0.42).lerp(Vec3::new(0.6, 0.74, 0.4), hue);
        let tint = deep.lerp(warm, open * 0.7) * rng.range(0.45, 0.9) * (0.75 + 0.25 * open);
        let pick = rng.f();
        let kind = if pick < 0.5 {
            Kind::Leaves
        } else if pick < 0.8 {
            Kind::Fern
        } else {
            Kind::Tuft
        };
        let flower = (open > 0.6 && rng.f() < 0.08)
            .then(|| [DAISY, BUTTERCUP, CAMPION][(rng.f() * 3.0) as usize % 3]);
        out.push(Clump {
            at: p,
            radius: rng.range(0.3, 0.55) * (0.8 + 0.4 * mass),
            kind,
            tint,
            flower,
        });
    }
    out
}

fn rgba(c: Vec3) -> [f32; 4] {
    [c.x, c.y, c.z, 1.0]
}

/// A clump's cards: leaves into `leaves` (they flutter), tufts and flowers into `stems`
/// (they bend from the foot).
fn build(c: &Clump, land: &Land, rng: &mut Rng, leaves: &mut Cards, stems: &mut Cards) {
    let root = Vec3::new(c.at.x, land.height(c.at) - 0.02, c.at.y);
    let up = |p: Vec3| ((p - root).with_y(0.0).normalize_or_zero() * 0.5 + Vec3::Y).normalize();
    let tip = rgba(c.tint);
    let foot = rgba(c.tint * FOOT);
    let spin = rng.f() * std::f32::consts::TAU;
    // Dark leaves lying flat beneath, so the gaps between the clump's leaves fall dark.
    for k in 0..3 {
        let yaw = spin + k as f32 * 2.1 + rng.f();
        let out = Vec3::new(yaw.cos(), 0.0, yaw.sin());
        let len = c.radius * rng.range(1.1, 1.5);
        let a = Vec3::new(-out.z, 0.0, out.x) * len * 0.8;
        let b = (out + Vec3::Y * 0.08).normalize() * len;
        let dark = rgba(c.tint * UNDER * FOOT);
        leaves.graded(
            root + b * 0.5 + Vec3::Y * 0.01,
            a,
            b,
            LEAF,
            |_| Vec3::Y,
            dark,
            rgba(c.tint * UNDER),
        );
    }
    match c.kind {
        Kind::Leaves => {
            let n = 5 + (rng.f() * 4.0) as usize;
            for k in 0..n {
                let yaw = spin + k as f32 * std::f32::consts::TAU / n as f32 + rng.range(-0.3, 0.3);
                let tilt = rng.range(0.6, 1.25);
                let out = Vec3::new(yaw.cos(), 0.0, yaw.sin());
                let dir = out * tilt.sin() + Vec3::Y * tilt.cos();
                let len = c.radius * rng.range(1.0, 1.4);
                let a = Vec3::new(-out.z, 0.0, out.x) * len * 0.85;
                let b = dir * len;
                leaves.graded(root + b * 0.5, a, b, LEAF, up, foot, tip);
            }
        }
        Kind::Fern => {
            let n = 6 + (rng.f() * 4.0) as usize;
            for k in 0..n {
                let yaw =
                    spin + k as f32 * std::f32::consts::TAU / n as f32 + rng.range(-0.25, 0.25);
                let tilt = rng.range(0.8, 1.3);
                let out = Vec3::new(yaw.cos(), 0.0, yaw.sin());
                let dir = out * tilt.sin() + Vec3::Y * tilt.cos();
                let len = c.radius * rng.range(1.4, 2.0);
                // The frond runs corner to corner of its cell: a + b along it, a - b across.
                let along = dir * len;
                let across = Vec3::new(-out.z, 0.0, out.x) * len;
                let (a, b) = ((along + across) * 0.5, (along - across) * 0.5);
                leaves.graded(root + along * 0.5, a, b, FROND, up, foot, tip);
            }
        }
        Kind::Tuft => {
            for k in 0..3 {
                let yaw = spin + k as f32 * 1.05;
                let a = Vec3::new(yaw.cos(), 0.0, yaw.sin()) * c.radius * 1.6;
                let b = Vec3::Y * c.radius * rng.range(1.1, 1.6);
                stems.graded(root + b * 0.5, a, b, TUFT, up, foot, tip);
            }
        }
    }
    if let Some(cell) = c.flower {
        let yaw = rng.f() * std::f32::consts::TAU;
        let a = Vec3::new(yaw.cos(), 0.0, yaw.sin()) * 0.3;
        let b = Vec3::Y * rng.range(0.3, 0.42);
        let bloom = rgba(Vec3::splat(0.85));
        stems.graded(
            root + b * 0.5,
            a,
            b,
            cell,
            up,
            rgba(Vec3::splat(0.85 * FOOT)),
            bloom,
        );
    }
}

fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    land: Res<Land>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::foliage::FoliageMaterial>>,
    mut mats: ResMut<crate::foliage::Mats>,
) {
    let mut trees = crate::forest::tree_spots(&land);
    trees.extend(
        super::dress::OAKS
            .iter()
            .map(|&(at, scale, yaw)| Spot { at, scale, yaw }),
    );
    let mut leaves = Cards::default();
    leaves.leaf = true;
    let mut stems = Cards::default();
    let mut rng = Rng::new(0x1a_3e6);
    for c in clumps(&land, &trees) {
        build(&c, &land, &mut rng, &mut leaves, &mut stems);
    }
    // The tree crowns' cards' material, its leaves fluttering a little and its stems bending.
    let cards = StandardMaterial {
        base_color_texture: Some(asset.load("materials/foliage/cards.png")),
        alpha_mode: AlphaMode::Mask(0.45),
        cull_mode: None,
        perceptual_roughness: 0.8,
        reflectance: 0.2,
        diffuse_transmission: 0.15,
        ..default()
    };
    let flutter =
        crate::foliage::material(&mut materials, &mut mats, cards.clone(), 0.0, 0.01, false);
    let bend = crate::foliage::material(&mut materials, &mut mats, cards, 0.05, 0.0, false);
    for (mesh, material) in [(leaves.mesh(), flutter), (stems.mesh(), bend)] {
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::default(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lawn covers most of the open ground in the frame and none of the fight's: no clump
    /// on trodden earth or within reach of the cast, and most open points have one near.
    #[test]
    fn the_lawn_covers_the_open_ground_and_leaves_the_fight_bare() {
        let land = Land::new();
        let trees = crate::forest::tree_spots(&land);
        let all = clumps(&land, &trees);
        let cast = [
            super::super::KNIGHT,
            super::super::FOE,
            super::super::TARGET,
            super::super::SECOND,
        ];
        for c in &all {
            assert!(
                trodden(c.at) < 0.02,
                "a clump at {} stands on the fight's ground",
                c.at
            );
            for f in cast {
                assert!(
                    f.distance(c.at) > CAST - 1e-3,
                    "a clump at {} is on a fighter at {f}",
                    c.at
                );
            }
        }
        // Open lawn points (where it may grow) with a clump within 0.6 m.
        let (mut open, mut covered) = (0, 0);
        for i in 0..60 {
            for j in 0..60 {
                let p = LO + (HI - LO) * Vec2::new(i as f32 + 0.5, j as f32 + 0.5) / 60.0;
                if !grows(&land, p) {
                    continue;
                }
                open += 1;
                if all.iter().any(|c| c.at.distance(p) < 0.6) {
                    covered += 1;
                }
            }
        }
        assert!(open > 500, "only {open} open points");
        let share = covered as f32 / open as f32;
        assert!(share > 0.8, "the lawn covers {share} of the open ground");
    }
}
