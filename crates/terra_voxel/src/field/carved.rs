//! A generated planet's caves (D643): carried by the planet and carved from its ground's
//! distance wherever it is asked finely enough to hold them.

use super::{GeneratedPlanet, HeightMemo, Sample, material};
use crate::caves::{Caves, FINEST_SEEN_M};
use glam::Vec3;

impl GeneratedPlanet {
    /// The same planet with caves carved from its ground.
    pub fn with_caves(mut self, caves: Caves) -> Self {
        self.caves = (!caves.systems().is_empty()).then_some(caves);
        self
    }

    /// Its caves, if it has any.
    pub fn caves(&self) -> Option<&Caves> {
        self.caves.as_ref()
    }

    /// The ground's distance at a point with its caves carved from it, for a memo's
    /// coarseness: ground seen coarsely has none (`FINEST_SEEN_M`).
    pub(super) fn carved(&self, point: Vec3, ground: f32, memo: &HeightMemo) -> f32 {
        match &self.caves {
            Some(caves) if memo.step as f32 * Self::LATTICE_M <= FINEST_SEEN_M => {
                caves.carve(point, ground)
            }
            _ => ground,
        }
    }

    /// The ground at a point with its caves carved from it: where a cave's carve reaches,
    /// its wall and the lip of its mouth are rock, whatever lay at that depth, so nothing
    /// that grows on soil grows in a cave.
    pub(super) fn carved_sample(&self, point: Vec3, ground: Sample, memo: &HeightMemo) -> Sample {
        let distance = self.carved(point, ground.distance, memo);
        Sample {
            distance,
            material: if distance == ground.distance {
                ground.material
            } else {
                material::STONE
            },
        }
    }
}
