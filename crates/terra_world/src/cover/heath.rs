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
use terra_land::round::Round;

/// What grows on a world's land round the start, by its rules.
struct HeathCover {
    round: Arc<Round>,
}

impl CoverSource for HeathCover {
    fn growing(&self, planet: &Planet, ground: Vec3, _at: Vec2) -> Growing {
        let heath = self.round.heath.at(ground);
        let wild = planet.cover(ground, ground.normalize());
        // The planet's grass already gives way to the heath past the trees (`Planet::cover`).
        let meadow = wild.grass;
        // Sedge and cotton grass stand in tussocks of blades with gaps between; among the
        // dwarf shrubs, the lichen and the moss a few grasses only.
        let blades =
            0.55 * heath.sedge + 0.15 * heath.shrub + 0.15 * heath.moss + 0.04 * heath.lichen;
        Growing {
            grass: (meadow + blades).min(1.0),
            lush: wild.lush * (1.0 - heath.open) + 0.3 * heath.open,
            tall: (heath.sedge / heath.open.max(1e-3)).min(1.0) * heath.open * 0.8,
            heath: heath.open,
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
        }),
        seed: planet.seed().derive("land cover"),
    });
}
