//! The planet's ground where a cave is (D643): what stands or grows on the land asks whether
//! the ground it found is a cave's, as it asks whether ground was dug.

use super::Planet;
use bevy::prelude::*;

/// A den's parts all lie within this of the middle between its mouth and its floor, in
/// metres over the ground (`terra_land::cave`: two legs of 26 and 16 m, a room 30 m across,
/// alcoves beside it).
const DEN_REACH_M: f32 = 70.0;

impl Planet {
    /// Whether the ground at a point is a cave's: its floor, its walls, or the cut of its
    /// mouth, not the land where its heights put it. Nothing that grows on the land grows
    /// there, and nothing that lives on it is put there.
    pub fn in_cave(&self, point: Vec3) -> bool {
        let Some(den) = self.den() else {
            return false;
        };
        let up = point.normalize_or(Vec3::Y);
        let middle = (den.mouth + den.floor) * 0.5;
        if up.angle_between(middle.normalize()) * middle.length() > DEN_REACH_M {
            return false;
        }
        (self.ground_at(point).length() - self.surface(up).length()).abs() > 0.15
    }
}
