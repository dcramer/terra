//! The ground under a base (construction-001, gap 3): its height under each column of
//! cubes, sampled when a plan is laid and kept with it, so every machine agrees on what
//! is joined to the ground whatever its terrain does. Heights are whole centimetres in the
//! base's own space, as 2 bytes a column.
//!
//! A column nothing has sampled, as under what the shortcuts built before plans, has its
//! ground at the base's own level, the foot of its first storey's walls.

use crate::CELL_M;
use glam::IVec3;
use std::collections::BTreeMap;

/// The ground's height where nothing was sampled, in centimetres.
pub const LEVEL_CM: i16 = 0;

/// A cube's height in centimetres.
pub fn cube_cm() -> i32 {
    (CELL_M * 100.0).round() as i32
}

/// The ground's height under one column of cubes, as a plan carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    /// The column, counted in cubes east and south.
    pub column: [i32; 2],
    pub height_cm: i16,
}

impl Sample {
    /// A sample of a height in metres, rounded to the centimetre.
    pub fn metres(column: [i32; 2], height: f32) -> Sample {
        let cm = (height * 100.0)
            .round()
            .clamp(f32::from(i16::MIN), f32::from(i16::MAX));
        Sample {
            column,
            height_cm: cm as i16,
        }
    }
}

/// The ground's sampled heights under a base.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ground {
    heights: BTreeMap<[i32; 2], i16>,
}

impl Ground {
    /// The sampled height under a column, if any.
    pub fn sample(&self, column: [i32; 2]) -> Option<i16> {
        self.heights.get(&column).copied()
    }

    /// The height under a column, in centimetres: its sample, or the base's level.
    pub fn height_cm(&self, column: [i32; 2]) -> i16 {
        self.sample(column).unwrap_or(LEVEL_CM)
    }

    /// Whether the ground reaches a cube: it passes through it, or the cube is below it.
    pub fn reaches(&self, at: IVec3) -> bool {
        at.y * cube_cm() <= i32::from(self.height_cm([at.x, at.z]))
    }

    pub(crate) fn set(&mut self, column: [i32; 2], height: Option<i16>) {
        match height {
            Some(height) => self.heights.insert(column, height),
            None => self.heights.remove(&column),
        };
    }

    /// Every sample, sorted by column.
    pub fn samples(&self) -> impl Iterator<Item = Sample> + '_ {
        self.heights
            .iter()
            .map(|(&column, &height_cm)| Sample { column, height_cm })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ground_reaches_the_cube_it_passes_through_and_those_below() {
        let mut ground = Ground::default();
        // Unsampled: the base's level, so its floor layer and first wall layer.
        assert!(ground.reaches(IVec3::new(0, 0, 0)));
        assert!(ground.reaches(IVec3::new(0, -1, 0)));
        assert!(!ground.reaches(IVec3::new(0, 1, 0)));
        // A slope falling away: 80 cm down, so only cubes from -2 down.
        ground.set([3, 0], Some(Sample::metres([3, 0], -0.8).height_cm));
        assert!(ground.reaches(IVec3::new(3, -2, 0)));
        assert!(!ground.reaches(IVec3::new(3, -1, 0)));
    }
}
