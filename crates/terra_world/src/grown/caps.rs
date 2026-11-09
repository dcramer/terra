//! Cap trees (docs/realm-two.md, D642): what stands where trees would in the far realm. A
//! great fungus grown by rule as a tree is: a stalk flared at its foot and leaning a little,
//! a wide domed cap, smaller caps in tiers down the stalk and young ones at its foot, each a
//! limb turned round its own axis (`build::tube`), so it wears a scanned surface, is drawn
//! at the trees' levels of detail and costs what a trunk does. Its numbers are its row's
//! (`CapDef`, `data/flora.ron`).

use super::build::Limb;
use super::tree::Tree;
use super::{CapDef, TreeDef};
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;
use terra_gen::Rng;

/// How a stalk's radius goes from its foot to its neck, against its `trunk_m`.
const STALK: [f32; 7] = [1.7, 1.25, 1.0, 0.9, 0.82, 0.76, 0.72];
/// A cap's rounds from its rim to its crown.
const ROUNDS: usize = 7;

/// A cap over a point: from the neck out under it to its rim, then over the dome to its
/// crown, a limb whose radius is the cap's own outline.
fn cap(at: Vec3, across: f32, depth: f32, neck: f32, level: u32) -> Limb {
    let rim = across / 2.0;
    let mut points = vec![at - Vec3::Y * depth * 0.08, at + Vec3::Y * depth * 0.02];
    let mut radii = vec![neck, rim * 0.97];
    for round in 0..=ROUNDS {
        let turn = round as f32 / ROUNDS as f32 * FRAC_PI_2;
        points.push(at + Vec3::Y * depth * (0.06 + 0.94 * libm::sinf(turn)));
        radii.push((rim * libm::cosf(turn)).max(0.01));
    }
    Limb {
        points,
        radii,
        depth: level,
    }
}

/// A stalk from a foot to its neck, leaning as it rises: its points, and its radius there.
fn stalk(foot: Vec3, lean: Vec3, rise: f32, girth: f32, level: u32) -> Limb {
    let points = (0..STALK.len())
        .map(|i| {
            let t = i as f32 / (STALK.len() - 1) as f32;
            foot + Vec3::Y * rise * t + lean * t * t
        })
        .collect();
    Limb {
        points,
        radii: STALK.iter().map(|share| girth * share).collect(),
        depth: level,
    }
}

/// Grows a cap tree into `tree`: its stalk, its cap, the tiers under it and its young.
pub fn grow(dice: &mut Rng, def: &TreeDef, tree: &mut Tree) {
    let CapDef {
        across,
        depth,
        tiers,
        young,
        tint,
        ..
    } = def.cap.unwrap_or_default();
    let (height, girth) = (tree.height, tree.trunk_m);
    tree.cap_tint = Vec3::from(tint);
    let lean = Vec3::new(dice.range(-0.07, 0.07), 0.0, dice.range(-0.07, 0.07)) * height;
    let rise = height * (1.0 - depth);
    tree.limbs.push(stalk(Vec3::ZERO, lean, rise, girth, 0));
    let neck = girth * STALK[STALK.len() - 1];
    tree.limbs.push(cap(
        Vec3::Y * rise + lean,
        across * height,
        depth * height,
        neck,
        1,
    ));
    // Smaller caps round the stalk below, each narrower than the one over it.
    for tier in 1..tiers {
        let t = 1.0 - tier as f32 / (tiers as f32 + 0.6);
        let at = Vec3::Y * rise * t + lean * t * t;
        let share = 0.62 - 0.12 * tier as f32;
        tree.limbs.push(cap(
            at,
            across * height * share,
            depth * height * 0.55,
            girth,
            1,
        ));
    }
    // Young ones at its foot, each a whole small one of its kind.
    for _ in 0..young {
        let turn = Quat::from_rotation_y(dice.range(0.0, std::f32::consts::TAU));
        let foot = turn * Vec3::X * girth * dice.range(2.2, 4.0);
        let small = height * dice.range(0.1, 0.24);
        let lean = (turn * Vec3::X) * small * dice.range(0.05, 0.2);
        let rise = small * (1.0 - depth);
        tree.limbs.push(stalk(foot, lean, rise, girth * 0.3, 2));
        tree.limbs.push(cap(
            foot + Vec3::Y * rise + lean,
            across * small,
            depth * small,
            girth * 0.3 * STALK[STALK.len() - 1],
            3,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grown::Habit;
    use terra_gen::Seed;

    fn def(cap: CapDef) -> TreeDef {
        TreeDef {
            habit: Habit::Cap,
            bark: "bark".into(),
            bark_m: 2.0,
            bark_tint: (1.0, 1.0, 1.0),
            leaves: "oak".into(),
            tint: (1.0, 1.0, 1.0),
            clump_m: 1.0,
            trunk: 0.05,
            cap: Some(cap),
        }
    }

    /// A cap tree is as tall as it is grown, its cap as wide as its row says, with a cap for
    /// each tier and a stalk and a cap for each young one; and it is built at every level
    /// of detail, fewer triangles each level out.
    #[test]
    fn a_cap_tree_is_its_rows_numbers_and_simpler_further_off() {
        let cap = CapDef {
            across: 0.8,
            depth: 0.25,
            tiers: 3,
            young: 2,
            tint: (0.6, 0.2, 0.4),
            ..default()
        };
        let tree = crate::grown::tree::grow(&mut Seed(3).rng(), &def(cap), 20.0);
        assert_eq!(tree.limbs.len(), 1 + 3 + 2 * 2);
        let top = (tree.limbs.iter().flat_map(|limb| &limb.points))
            .map(|point| point.y)
            .fold(0.0, f32::max);
        assert!((top - 20.0).abs() < 0.3, "{top} m tall");
        let widest = tree.limbs[1].radii.iter().copied().fold(0.0, f32::max);
        assert!(
            (widest * 2.0 - 16.0).abs() < 0.6,
            "its cap {} m across",
            widest * 2.0
        );
        assert_eq!(tree.cap_tint, Vec3::new(0.6, 0.2, 0.4));
        let pivots = crate::grown::tree::pivots(&tree);
        let triangles: Vec<usize> = (0..3)
            .map(|detail| {
                crate::grown::tree::wood(&tree, detail, 2.0, &pivots)
                    .indices
                    .len()
                    / 3
            })
            .collect();
        assert!(
            triangles[0] > triangles[1] && triangles[1] > triangles[2] && triangles[2] > 0,
            "{triangles:?}"
        );
    }

    /// A cap is lathed with its outside its front: every triangle faces as its corners'
    /// normals do, its dome up and its gills down. Wound as a tube is, its gills were
    /// culled from below and the dome's inside was seen through them.
    #[test]
    fn a_caps_outside_is_its_front_and_its_gills_face_down() {
        let limb = cap(Vec3::Y * 10.0, 8.0, 2.0, 0.3, 1);
        let pivots = crate::grown::pivots::Pivots::default();
        let mut build = crate::grown::build::Build::levelled(&pivots);
        crate::grown::build::lathe(&mut build, &limb, 12, 12.0, Vec3::ONE);
        let at = |i: u32| Vec3::from(build.positions[i as usize]);
        let normal = |i: u32| Vec3::from(build.normals[i as usize]);
        let (mut down, mut up) = (0, 0);
        for corners in build.indices.chunks(3) {
            let (a, b, c) = (corners[0], corners[1], corners[2]);
            let face = (at(b) - at(a)).cross(at(c) - at(a));
            if face.length() < 1.0e-6 {
                continue;
            }
            let mean = normal(a) + normal(b) + normal(c);
            assert!(face.dot(mean) > 0.0, "a triangle faces into the cap");
            down += usize::from(mean.y < -1.5);
            up += usize::from(mean.y > 1.5);
        }
        assert!(down >= 12 && up >= 12, "{down} down, {up} up");
        // Skin, by its texture coordinates.
        assert!(build.uvs.iter().all(|uv| uv[1] <= -1.0));
    }

    /// A tube's outside is its front, as a cap's is: wound the other way, bark drawn one
    /// sided (a grown tree's, a cap tree's stalk) showed its far wall from within.
    #[test]
    fn a_tubes_outside_is_its_front() {
        let limb = stalk(Vec3::ZERO, Vec3::X, 10.0, 0.5, 0);
        let pivots = crate::grown::pivots::Pivots::default();
        let mut build = crate::grown::build::Build::levelled(&pivots);
        crate::grown::build::tube(&mut build, &limb, 8, 12.0, 2.0, Vec3::ONE);
        let at = |i: u32| Vec3::from(build.positions[i as usize]);
        let normal = |i: u32| Vec3::from(build.normals[i as usize]);
        for corners in build.indices.chunks(3) {
            let (a, b, c) = (corners[0], corners[1], corners[2]);
            let face = (at(b) - at(a)).cross(at(c) - at(a));
            assert!(face.dot(normal(a) + normal(b) + normal(c)) > 0.0);
        }
    }
}
