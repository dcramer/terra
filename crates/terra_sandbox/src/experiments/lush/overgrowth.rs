//! What grows on the lush forest's trunks (D309): ivy climbing some of them, its leaves laid
//! against the bark from a mat round the foot up a few vines, and a band of moss round the
//! foot of others, as Sons of the Forest's boles have both. Which trunks, and how high, is
//! each kind of tree's data (`OVERGROWTH`); each kind's ivy is grown in a few variants, so
//! two trees of a kind wear different ivy (D182).

use super::species::{Species, by_height};
use super::{Card, Leaf, Limb, Shape, Tree};
use bevy::prelude::*;
use std::f32::consts::TAU;
use terra_gen::Rng;

/// What grows on a share of one kind of tree's grown trunks.
pub(super) struct Overgrowth {
    /// The share ivy climbs, and how high, in metres: least and most.
    pub ivy: f32,
    pub ivy_m: (f32, f32),
    /// The share with a band of moss round the foot, and how high it reaches, in metres.
    pub moss: f32,
    pub moss_m: (f32, f32),
}

/// Oak, pine and spruce. Ivy climbs oaks most and a spruce not at all, its skirt of boughs to
/// the ground leaving it no light; moss bands the damp feet of all of them.
pub(super) const OVERGROWTH: [Overgrowth; 3] = [
    Overgrowth {
        ivy: 0.35,
        ivy_m: (2.5, 8.0),
        moss: 0.4,
        moss_m: (1.2, 3.0),
    },
    // A third of the pines ivied and half with mossy feet (D359): fewer read as every bole
    // the same clean pole.
    Overgrowth {
        ivy: 0.32,
        ivy_m: (2.0, 8.0),
        moss: 0.5,
        moss_m: (1.0, 3.0),
    },
    Overgrowth {
        ivy: 0.0,
        ivy_m: (0.0, 0.0),
        moss: 0.35,
        moss_m: (1.0, 2.5),
    },
];

/// How many ivies each kind of tree is grown with.
pub(super) const IVY_VARIANTS: usize = 2;

/// A moss band's height in a trunk's mesh tag, as a share of the tree's height times this,
/// in its second byte (`foliage_fragment.wesl`, `MOSS_BAND_SCALE`): to a quarter of it.
pub(super) const MOSS_BAND_SCALE: f32 = 1020.0;

impl Overgrowth {
    pub(super) fn of(species: Species) -> &'static Self {
        &OVERGROWTH[species as usize]
    }
}

/// The mesh tag that bands a trunk `height` tall with moss to `band_m` (none at 0).
pub(super) fn moss_tag(band_m: f32, height: f32) -> u32 {
    let share = (band_m / height.max(1.0) * MOSS_BAND_SCALE)
        .round()
        .clamp(0.0, 255.0);
    (share as u32) << 8
}

/// A number from 0 to 1 for a place and a purpose, the same each time, so a tree's overgrowth
/// stays with where it stands.
pub(super) fn chance(at: Vec2, salt: f32) -> f32 {
    (libm::sinf(at.x * 12.9898 + at.y * 78.233 + salt * 39.425) * 43_758.547)
        .fract()
        .abs()
}

/// A point on a trunk's bark at a height and a turn round it, and the way the bark faces
/// there: its radius as the trunk is drawn (lush.rs, `tube`), flutes, lumps and roots and
/// all, so a leaf laid on it lies on the bark, not in it or off it.
fn on_bark(limb: &Limb, shape: &Shape, y: f32, angle: f32) -> (Vec3, Vec3, Vec3) {
    let (centre, radius) = by_height(&limb.points, &limb.radii, y);
    let (below, _) = by_height(&limb.points, &limb.radii, (y - 0.3).max(0.0));
    let (above, _) = by_height(&limb.points, &limb.radii, y + 0.3);
    let forward = (above - below).normalize_or(Vec3::Y);
    // The tube's own frame round it, carried up from its foot, so a turn here is the same
    // place round it as the tube's.
    let side = (limb.points[1] - limb.points[0]).any_orthonormal_vector();
    let side = (side - forward * side.dot(forward)).normalize_or(forward.any_orthonormal_vector());
    let other = forward.cross(side);
    let out = side * libm::cosf(angle) + other * libm::sinf(angle);
    let r = radius * shape.at(angle, y, y);
    (centre + out * r, out, forward)
}

/// Ivy on a tree's trunk, climbing to about `top_m`: a mat of leaves round its foot, and from
/// it a few vines winding up the bole, thick at the foot and giving out in single leaves as
/// they climb. Its leaves are cards of the litter atlas's ivy (`leaves`), each laid on the
/// bark a little off it, turned at random about the way the bark faces, some standing out from
/// it at their top as ivy's leaves do.
pub(super) fn ivy(
    dice: &mut Rng,
    tree: &Tree,
    height: f32,
    top_m: f32,
    leaves: &[Leaf],
) -> Vec<Card> {
    let Some(trunk) = tree
        .limbs
        .iter()
        .find(|limb| limb.depth == 0 && limb.points[0].y < 0.01)
    else {
        return Vec::new();
    };
    let shape = Shape::of(trunk);
    let top_m = top_m.min(trunk.points[trunk.points.len() - 1].y * 0.9);
    let mut cards = Vec::new();
    let mut leaf_at = |dice: &mut Rng, y: f32, angle: f32, size: f32, lean: f32| {
        let (at, out, forward) = on_bark(trunk, &shape, y, angle);
        let across = forward.cross(out);
        let turn = dice.range(-0.9, 0.9);
        let up = forward * libm::cosf(turn) + across * libm::sinf(turn);
        // Standing out from the bark at its top by `lean`, as a leaf on its stalk does.
        let up = (up + out * lean).normalize();
        let right = up.cross(out).normalize_or(across);
        let half = size * 0.5;
        let bend = (y / height).clamp(0.0, 1.0);
        // Ivy's leaves a deep green, darker and greener than the floor's pale ivy.
        let light = dice.range(0.55, 0.85) * if dice.unit() < 0.25 { 0.75 } else { 1.0 };
        cards.push(Card {
            middle: at + out * (0.025 + half * lean * 0.8),
            right,
            up,
            half,
            normal: (out * 0.8 + Vec3::Y * 0.2).normalize(),
            tint: Vec3::new(0.75, 0.95, 0.55) * light,
            sway: Vec2::new(bend * bend, 0.06),
            flip: if dice.unit() < 0.5 { -1.0 } else { 1.0 },
            // Low on a trunk in a wood it sees little of the sky, more as it climbs (D216).
            sky: 0.32 + 0.35 * (y / 8.0).clamp(0.0, 1.0),
            leaf: leaves[dice.below(leaves.len() as u32) as usize],
        });
    };
    // The mat round the foot, on the side the vines rise from.
    let from = dice.range(0.0, TAU);
    let reach = dice.range(0.5, 1.1);
    for _ in 0..22 + dice.below(10) {
        let angle = from + dice.range(-reach, reach);
        let y = dice.range(0.05, 1.1);
        let size = dice.range(0.28, 0.4);
        let lean = dice.range(0.0, 0.35);
        leaf_at(dice, y, angle, size, lean);
    }
    // The vines, each winding up the bole as far as it got, a leaf or two either side of it at
    // each step, closer at its foot, so the ivy reads as a mass up the bark, not a line of
    // stickers.
    for _ in 0..3 + dice.below(3) {
        let mut angle = from + dice.range(-reach, reach) * 0.6;
        let drift = dice.range(-0.08, 0.08);
        let top = top_m * dice.range(0.45, 1.0);
        let mut y = dice.range(0.3, 0.9);
        let mut side = 1.0;
        while y < top {
            let t = y / top.max(0.1);
            let (_, radius) = by_height(&trunk.points, &trunk.radii, y);
            for _ in 0..1 + (dice.unit() < 0.6 - 0.4 * t) as u32 {
                let size = dice.range(0.24, 0.36) * (1.0 - 0.3 * t);
                let off = side * dice.range(0.03, 0.14) / radius.max(0.1);
                let lean = if dice.unit() < 0.5 {
                    dice.range(0.35, 0.7)
                } else {
                    dice.range(0.0, 0.15)
                };
                let at = y + dice.range(-0.05, 0.05);
                leaf_at(dice, at, angle + off, size, lean);
                side = -side;
            }
            let step = dice.range(0.08, 0.14) * (1.0 + 0.9 * t);
            y += step;
            angle += drift * step + dice.range(-0.04, 0.04);
        }
    }
    cards
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_gen::Seed;

    fn leaf() -> Leaf {
        Leaf {
            cell: Vec2::ZERO,
            across: 0.25,
            outline: [(0.0, 0.0); 8],
        }
    }

    /// Ivy lies on the bark as the trunk is drawn: every leaf's middle a little off the bark,
    /// none inside it nor standing off in the air, and it climbs about as high as asked, its
    /// leaves thinning out as it climbs.
    #[test]
    fn ivy_lies_on_the_bark_up_to_its_height() {
        let mut dice = Seed(3).rng();
        let tree =
            super::super::species::oak(&mut dice, 22.0, super::super::species::Form::Tall, false);
        let trunk = tree.limbs.iter().find(|l| l.depth == 0).expect("a trunk");
        let shape = Shape::of(trunk);
        let cards = ivy(&mut dice, &tree, 22.0, 6.0, &[leaf()]);
        assert!(cards.len() > 25, "{} leaves", cards.len());
        for card in &cards {
            // The height up the trunk whose ring the leaf lies off: where it is square to the
            // trunk's axis, which leans.
            let mut y = card.middle.y;
            for _ in 0..4 {
                let (centre, _) = by_height(&trunk.points, &trunk.radii, y);
                let (_, _, forward) = on_bark(trunk, &shape, y, 0.0);
                y += (card.middle - centre).dot(forward) / forward.y;
            }
            let (centre, radius) = by_height(&trunk.points, &trunk.radii, y);
            // Its turn round the trunk in the tube's frame, from the bark's way out at a quarter
            // turn and none, and how far out the bark is there.
            let (_, side, _) = on_bark(trunk, &shape, y, 0.0);
            let (_, other, _) = on_bark(trunk, &shape, y, TAU / 4.0);
            let to = card.middle - centre;
            let turn = libm::atan2f(to.dot(other), to.dot(side));
            let (on, out, _) = on_bark(trunk, &shape, y, turn);
            let off = (card.middle - on).dot(out);
            assert!(off > 0.0, "a leaf {off} m into the bark at {y} m");
            assert!(
                off < 0.2 + radius * 0.1,
                "a leaf {off} m off the bark at {y} m"
            );
        }
        let highest = cards.iter().map(|c| c.middle.y).fold(0.0, f32::max);
        assert!(
            highest > 6.0 * 0.5 && highest < 6.0 + 0.5,
            "climbs to {highest} m"
        );
        let low = cards.iter().filter(|c| c.middle.y < 3.0).count();
        let high = cards.iter().filter(|c| c.middle.y >= 3.0).count();
        assert!(low > high, "{low} leaves below 3 m, {high} above");
    }

    /// A moss band's tag holds its height as a share of the tree's, as the shader reads it
    /// back, in the byte above the camera's fade.
    #[test]
    fn a_moss_band_is_tagged_as_a_share_of_the_height() {
        let tag = moss_tag(2.0, 20.0);
        assert_eq!(tag & 0xff, 0);
        let share = ((tag >> 8) & 0xff) as f32 / MOSS_BAND_SCALE;
        assert!((share - 0.1).abs() < 0.002, "{share}");
        assert_eq!(moss_tag(0.0, 20.0), 0);
    }
}
