//! What the forest's planting tells the log: how many of each tree, and the nearest of each to
//! the start, for scripted runs to walk up to and look at (D309).

use bevy::prelude::*;

/// How many trees of each kind went in, and where the nearest of each stands.
pub(super) struct Tally<'a> {
    pub(super) veterans: &'a [Vec2],
    /// Oaks, pines and spruces, and the nearest grown one of each.
    pub(super) by_species: [u32; 3],
    pub(super) nearest_of: [Vec2; 3],
    /// How many of each kind wear ivy and a band of moss, and the nearest of each.
    pub(super) ivied: [(u32, Vec2); 3],
    pub(super) banded: [(u32, Vec2); 3],
    /// Grown and young, and the nearest of each.
    pub(super) planted: [u32; 2],
    pub(super) nearest_to_start: [Vec2; 2],
}

impl Tally<'_> {
    pub(super) fn tell(&self) {
        if let Some(near) = self
            .veterans
            .iter()
            .min_by(|a, b| a.length().total_cmp(&b.length()))
        {
            info!(
                "lush: {} veteran oaks, the nearest at {:.1},{:.1} (east, south).",
                self.veterans.len(),
                near.x,
                near.y
            );
        }
        let [oaks, pines, spruces] = self.by_species;
        let [oak, pine, spruce] = self.nearest_of;
        info!(
            "lush: {oaks} oaks, {pines} pines and {spruces} spruces; the nearest grown of each at {:.1},{:.1}, {:.1},{:.1} and {:.1},{:.1} (east, south).",
            oak.x, oak.y, pine.x, pine.y, spruce.x, spruce.y
        );
        for (what, tally) in [("ivy", &self.ivied), ("a band of moss", &self.banded)] {
            let [oak, pine, spruce] = tally.map(|(count, at)| {
                if count == 0 {
                    "0".to_string()
                } else {
                    format!("{count} (the nearest at {:.1},{:.1})", at.x, at.y)
                }
            });
            info!("lush: {what} on oaks {oak}, pines {pine}, spruces {spruce} (east, south).");
        }
        let [grown, young] = self.nearest_to_start;
        info!(
            "lush: {} grown trees and {} young ones planted. The nearest stand at {:.1},{:.1} and \
             {:.1},{:.1} (east, south).",
            self.planted[0], self.planted[1], grown.x, grown.y, young.x, young.y
        );
    }
}
