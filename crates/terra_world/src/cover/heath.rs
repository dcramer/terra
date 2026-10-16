//! The ground cover on a world whose land round the start was worked out (`Planet::round`):
//! a meadow's grass where the planet grows it, giving way past the trees to the heath's
//! communities (`terra_land::heath`, docs/demos.md §2), whose sedge stands as blades, thick
//! and tall in wet ground, and whose dwarf shrubs, lichen and moss lie low among a few. The
//! ground draws the communities themselves from the same map (`planet::LandMap`), so the
//! blades and the ground under them agree.

use super::{CoverRules, CoverSource, Growing};
use crate::planet::Planet;
use bevy::prelude::*;
use std::sync::Arc;
use terra_gen::Noise;
use terra_land::round::Round;

/// What grows on a world's land round the start, by its rules.
struct HeathCover {
    round: Arc<Round>,
    /// The communities' patches within a map cell (`patches`).
    mosaic: Noise,
}

/// The heath's communities as they lie in patches a few metres across, each patch mostly one
/// of them, in the shares the land's 10 m map gives: lichen's pale cushions, a mat of dwarf
/// shrubs, sedge tussocks, moss. Blended smoothly from cell to cell they were one even mix
/// everywhere, a field of the same straw (the developers, 2026-10-15: "needs more diversity").
fn patches(mosaic: &Noise, at: Vec3, shares: [f32; 4]) -> [f32; 4] {
    let total: f32 = shares.iter().sum();
    if total <= 0.0 {
        return shares;
    }
    let mut weights = [0.0; 4];
    for (k, (weight, share)) in weights.iter_mut().zip(shares).enumerate() {
        let offset = Vec3::splat(17.0 * k as f32 + 3.0);
        let here = mosaic.fbm(at / PATCH_M + offset, 3, 2.0, 0.5);
        // Each community's own noise lifts or sinks it; the rarer get a lift of their own,
        // so a share of a tenth still makes patches of its own rather than vanishing.
        let lifted = ((share + 0.1) * (1.0 + 3.0 * here)).max(0.0) * share.clamp(0.0, 0.02) / 0.02;
        *weight = lifted * lifted * lifted;
    }
    let sum: f32 = weights.iter().sum();
    if sum <= 0.0 {
        return shares;
    }
    weights.map(|w| w / sum * total)
}
/// How far across a patch of one community is, about.
const PATCH_M: f32 = 4.0;

impl CoverSource for HeathCover {
    fn growing(&self, planet: &Planet, ground: Vec3, _at: Vec2) -> Growing {
        // Nothing grows up through a pool's or a lake's water.
        if ground.length() < planet.water_near_m(ground) + 0.03 {
            return Growing {
                heath: 1.0,
                ..default()
            };
        }
        let mut heath = self.round.heath.at(ground);
        [heath.lichen, heath.shrub, heath.sedge, heath.moss] = patches(
            &self.mosaic,
            ground,
            [heath.lichen, heath.shrub, heath.sedge, heath.moss],
        );
        let wild = planet.cover(ground, ground.normalize());
        // The planet's grass already gives way to the heath past the trees (`Planet::cover`).
        let meadow = wild.grass;
        // Sedge and cotton grass stand in tussocks of blades with gaps between; among the
        // dwarf shrubs, the lichen and the moss a few grasses only.
        // The dwarf shrubs a dense mat of short leafy shoots (`Growing::shrub`), so the heath
        // stands a hand high and has its texture rather than lying as a painted floor.
        let blades = 0.55 * heath.sedge + 0.6 * heath.shrub + 0.7 * heath.moss + 0.9 * heath.lichen;
        Growing {
            grass: (meadow + blades).min(1.0),
            lush: wild.lush * (1.0 - heath.open) + 0.3 * heath.open,
            tall: (heath.sedge / heath.open.max(1e-3)).min(1.0) * heath.open * 0.8,
            heath: heath.open,
            shrub: 0.6 * heath.shrub,
            lichen: 0.9 * heath.lichen,
            heath_moss: 0.7 * heath.moss,
            ..default()
        }
    }
}

/// Grows the land's cover, unless the layer above grows its own.
pub(super) fn grow_the_lands(
    mut commands: Commands,
    planet: Res<Planet>,
    rules: Option<Res<CoverRules>>,
) {
    let Some(round) = planet.round() else {
        return;
    };
    if rules.is_some() {
        return;
    }
    commands.insert_resource(CoverRules {
        source: Arc::new(HeathCover {
            round: round.clone(),
            mosaic: Noise::new(planet.seed().derive("heath patches")),
        }),
        seed: planet.seed().derive("land cover"),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_gen::Seed;

    #[test]
    fn an_even_mix_lies_in_patches_of_one_community_each_in_its_share() {
        let mosaic = Noise::new(Seed(4));
        let shares = [0.2, 0.4, 0.3, 0.1];
        let (mut means, mut mostly_one, n) = ([0.0_f32; 4], 0, 2000);
        for i in 0..n {
            let at = Vec3::new(i as f32 * 0.37, 0.0, (i % 50) as f32 * 1.3);
            let patch = patches(&mosaic, at, shares);
            let total: f32 = patch.iter().sum();
            assert!((total - 1.0).abs() < 1e-4, "{patch:?}");
            for (mean, share) in means.iter_mut().zip(patch) {
                *mean += share / n as f32;
            }
            if patch.iter().cloned().fold(0.0, f32::max) > 0.6 {
                mostly_one += 1;
            }
        }
        // Most places are mostly one community, and over the ground each keeps about its
        // share, the commoner a little more and the rarer a little less, never none.
        assert!(mostly_one > n * 6 / 10, "{mostly_one} of {n}");
        for (mean, share) in means.iter().zip(shares) {
            assert!(*mean > share * 0.4 && *mean < share * 1.8, "{means:?}");
        }
    }
}
