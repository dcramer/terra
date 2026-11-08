//! Natural caves (D591, D643, docs/caves.md): systems of chambers and passages carved from
//! the ground's distance. A system is made of parts (a passage's stretch a capsule, a chamber
//! a spheroid with a level floor), its walls roughened by a noise of some decimetres; what a
//! planet carries is subtracted from its ground with a smooth maximum, as Voxel Tools'
//! `SdfSmoothSubtract` does (Zylann's godot_voxel, MIT: the method, no code of it), so the
//! field stays a distance: the streaming's chunk rule (`lod::may_hold_surface`), the mover
//! and digging read it as any other ground.
//!
//! Where a system goes and what shape it takes on a seed's land is the land's to say
//! (`terra_land::cave`); this is only the carve.

use crate::mountains::smooth_max;
use glam::Vec3;
use terra_core::Seed;
use terra_gen::Noise;

/// The carve eases into the ground over this, either side of where a cave's wall meets it:
/// a mouth's lip is rounded, not a knife's edge.
pub const BLEND_M: f32 = 1.5;
/// How far a wall is pushed in or out by its roughness, and how far apart its bumps are:
/// knobs of some decimetres on swells of half a metre and more, so a passage is no tube
/// (looked at in the game: with the knobs alone it was one).
const ROUGH_M: f32 = 0.35;
const ROUGH_ACROSS_M: f32 = 3.0;
const SWELL_M: f32 = 0.6;
const SWELL_ACROSS_M: f32 = 8.0;
/// Seen more coarsely than this, in metres a voxel, the ground has no caves: a passage is
/// narrower than two such voxels, and a far chunk's caves could only be seen as holes.
pub const FINEST_SEEN_M: f32 = 2.0;

/// One part of a system, in the planet's space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    /// A passage's stretch: everything within a radius of the line between two points.
    Passage { from: Vec3, to: Vec3, radius_m: f32 },
    /// A chamber: a spheroid so far across and so high about its middle, upright as the
    /// planet is there, with a level floor so far below its middle.
    Chamber {
        middle: Vec3,
        across_m: f32,
        high_m: f32,
        floor_m: f32,
    },
}

impl Part {
    /// How far a point is from the part's wall: negative in the open, inside it.
    fn distance(&self, point: Vec3) -> f32 {
        match *self {
            Part::Passage { from, to, radius_m } => {
                let along = to - from;
                let share =
                    ((point - from).dot(along) / along.length_squared().max(1e-6)).clamp(0.0, 1.0);
                point.distance(from + along * share) - radius_m
            }
            Part::Chamber {
                middle,
                across_m,
                high_m,
                floor_m,
            } => {
                let up = middle.normalize_or(Vec3::Y);
                let from = point - middle;
                let rise = from.dot(up);
                let out = (from - up * rise).length();
                // Inigo Quilez's bound on an ellipsoid's distance, exact on its surface.
                let (a, b) = (across_m * 0.5, high_m * 0.5);
                let scaled = Vec3::new(out / a, rise / b, 0.0).length();
                let steep = Vec3::new(out / (a * a), rise / (b * b), 0.0).length();
                let round = if steep > 1e-6 {
                    scaled * (scaled - 1.0) / steep
                } else {
                    -a.min(b)
                };
                // Open only above its floor.
                round.max(-(rise + floor_m))
            }
        }
    }

    /// A ball that holds the part.
    fn ball(&self) -> (Vec3, f32) {
        match *self {
            Part::Passage { from, to, radius_m } => {
                ((from + to) * 0.5, from.distance(to) * 0.5 + radius_m)
            }
            Part::Chamber {
                middle,
                across_m,
                high_m,
                ..
            } => (middle, across_m.max(high_m) * 0.5),
        }
    }
}

/// A cave system: its parts as one open space, roughened.
#[derive(Clone, Debug)]
pub struct System {
    parts: Vec<Part>,
    /// A ball that holds it all, walls' roughness and the carve's easing with it.
    middle: Vec3,
    reach_m: f32,
    rough: Noise,
}

impl System {
    /// A system of parts, its walls' roughness by a seed.
    pub fn new(parts: Vec<Part>, seed: Seed) -> Self {
        let balls: Vec<_> = parts.iter().map(Part::ball).collect();
        let middle = balls.iter().map(|(at, _)| *at).sum::<Vec3>() / balls.len().max(1) as f32;
        let reach = balls
            .iter()
            .map(|(at, radius)| at.distance(middle) + radius)
            .fold(0.0, f32::max);
        Self {
            parts,
            middle,
            reach_m: reach + ROUGH_M + SWELL_M + BLEND_M,
            rough: Noise::new(seed.derive("cave walls")),
        }
    }

    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// The ball that holds the system and everything its carve changes.
    pub fn ball(&self) -> (Vec3, f32) {
        (self.middle, self.reach_m)
    }

    /// How far a point is from the system's walls: negative in the open. The roughness is
    /// asked from the system's middle, where a single still tells decimetres apart on a
    /// world tens of kilometres across.
    pub fn distance(&self, point: Vec3) -> f32 {
        let smooth = self
            .parts
            .iter()
            .map(|part| part.distance(point))
            .fold(f32::MAX, f32::min);
        let from = point - self.middle;
        smooth
            + ROUGH_M * self.rough.sample(from / ROUGH_ACROSS_M)
            + SWELL_M * self.rough.sample(from / SWELL_ACROSS_M + Vec3::splat(17.3))
    }
}

/// The caves a planet carries.
#[derive(Clone, Debug, Default)]
pub struct Caves(Vec<System>);

impl Caves {
    pub fn new(systems: Vec<System>) -> Self {
        Self(systems)
    }

    pub fn systems(&self) -> &[System] {
        &self.0
    }

    /// The ground's distance at a point with the caves carved from it: `ground` where no
    /// cave is near, the open space's own distance inside one. Air above the ground stays
    /// air: a cave only takes rock away.
    pub fn carve(&self, point: Vec3, ground: f32) -> f32 {
        if ground > BLEND_M {
            return ground;
        }
        let open = self
            .0
            .iter()
            .filter(|system| {
                point.distance_squared(system.middle) < system.reach_m * system.reach_m
            })
            .map(|system| system.distance(point))
            .fold(f32::MAX, f32::min);
        if open == f32::MAX {
            ground
        } else {
            smooth_max(ground, -open, BLEND_M)
        }
    }
}

#[cfg(test)]
mod tests;
