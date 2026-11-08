//! Where a bird sits up off the ground besides the trees' crowns: the ridges of a
//! settlement's roofs, and the tops of stones that stand clear of the grass (D647: a bird on
//! a meadow's ground is under the grass and not seen).

use super::Surroundings;
use bevy::prelude::*;

/// A stone's top this far over the ground is clear of the grass, metres: a bird on it is seen.
const CLEAR_OF_GRASS_M: f32 = 0.6;

/// Where birds sit up off the ground besides the trees: the ridges of a settlement's roofs,
/// each as its two ends, which the layer that puts the buildings up fills in (the town's).
/// Pigeons and sparrows line a ridge, as they do on every town's roofs.
#[derive(Resource, Default)]
pub struct Perches(pub Vec<(Vec3, Vec3)>);

impl Surroundings<'_, '_> {
    /// A spot on a ridge roughly the way given, between so near and so far across the ground:
    /// the one most that way, somewhere along it.
    pub fn perch(
        &self,
        here: Vec3,
        way: Vec3,
        (near, far): (f32, f32),
        dice: &mut terra_gen::Rng,
    ) -> Option<Vec3> {
        let up = here.normalize_or(Vec3::Y);
        let mut best: Option<(Vec3, f32)> = None;
        for &(a, b) in &self.perches.0 {
            let point = a.lerp(b, dice.range(0.1, 0.9));
            let off = (point - here).reject_from(up);
            let apart = off.length();
            let lean = off.normalize_or_zero().dot(way);
            if (near..=far).contains(&apart) && lean > -0.2 && best.is_none_or(|(_, l)| lean > l) {
                best = Some((point, lean));
            }
        }
        best.map(|(point, _)| point)
    }

    /// The top of a stone roughly the way given, between so near and so far across the
    /// ground and standing clear of the grass: one of those there, by the dice.
    pub fn stone_perch(
        &self,
        here: Vec3,
        way: Vec3,
        (near, far): (f32, f32),
        dice: &mut terra_gen::Rng,
    ) -> Option<Vec3> {
        let up = here.normalize_or(Vec3::Y);
        let tops: Vec<Vec3> = (self.thickets.stone_tops(here, far))
            .filter(|top| {
                let off = (*top - here).reject_from(up);
                let apart = off.length();
                (near..=far).contains(&apart)
                    && off.normalize_or_zero().dot(way) > -0.2
                    && top.length() - self.ground(*top).length() > CLEAR_OF_GRASS_M
            })
            .take(12)
            .collect();
        (!tops.is_empty()).then(|| tops[(dice.unit() * tops.len() as f32) as usize % tops.len()])
    }
}
