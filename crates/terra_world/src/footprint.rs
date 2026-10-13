//! Where something built stands on the ground, as the world sees it: the places on its grid
//! it covers, and its flights of steps. What grows keeps out of it (`flora`), and whatever
//! builds it keeps it up to date, so the world never needs to know how building works.

use bevy::{platform::collections::HashSet, prelude::*};

/// The ground under something built, in its own space: `frame` places it on the planet,
/// with the grid's x and z along the ground, `cell_m` apart.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Footprint {
    pub frame: Transform,
    pub cell_m: f32,
    /// Places in a room, or with a floor, on the ground storey.
    pub ground: HashSet<IVec2>,
    /// Places where anything stands, a lone wall as much as a room.
    pub standing: HashSet<IVec2>,
    pub steps: Vec<Steps>,
}

/// A flight of steps down from a door, in the footprint's own space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Steps {
    /// The middle of the top step.
    pub top: Vec3,
    /// Level, pointing away from the door.
    pub outward: Vec3,
    pub length: f32,
    pub width: f32,
}

/// Further than this from a footprint's origin, a point is on none of it.
const FAR_M: f32 = 100_000.0;

impl Footprint {
    /// Whether a point on the planet is on the footprint, or within a margin of it: in a
    /// room or on a floor, within the margin of anything standing, or on or by the steps.
    /// Every place in reach is looked at, so a wall one place thick is never missed.
    pub fn covers(&self, point: Vec3, margin: f32) -> bool {
        let local = self
            .frame
            .compute_affine()
            .inverse()
            .transform_point3(point);
        // Nowhere near it, or no point at all, as ground searched for under a dug cliff
        // can give: not on it, and no sum below overflows.
        if !local.is_finite() || local.abs().max_element() > FAR_M {
            return false;
        }
        let at = (local / self.cell_m).floor().as_ivec3();
        let reach = (margin / self.cell_m).ceil() as i32;
        self.ground.contains(&IVec2::new(at.x, at.z))
            || (-reach..=reach).any(|dx| {
                (-reach..=reach).any(|dz| self.standing.contains(&IVec2::new(at.x + dx, at.z + dz)))
            })
            || self.steps.iter().any(|steps| {
                let offset = local - steps.top;
                let along = offset.dot(steps.outward);
                let across = offset.dot(steps.outward.cross(Vec3::Y));
                (-margin..=steps.length + margin).contains(&along)
                    && across.abs() <= steps.width * 0.5 + margin
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_footprint_covers_its_rooms_the_reach_of_its_walls_and_its_steps() {
        // Somewhere on a planet, its up the ground's and turned about it, as a base is.
        let up = Vec3::new(0.3, 0.9, 0.5).normalize();
        let frame = Transform::from_translation(up * 2000.0)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(0.7));
        let footprint = Footprint {
            frame,
            cell_m: 0.5,
            ground: [IVec2::new(0, 0)].into_iter().collect(),
            standing: [IVec2::new(4, 0)].into_iter().collect(),
            steps: vec![Steps {
                top: Vec3::new(0.0, 0.0, 5.0),
                outward: Vec3::Z,
                length: 2.0,
                width: 1.0,
            }],
        };
        // A place on the planet, given in the footprint's own space.
        let at = |x: f32, z: f32| frame.transform_point(Vec3::new(x, 0.0, z));
        assert!(footprint.covers(at(0.25, 0.25), 0.0));
        assert!(!footprint.covers(at(1.25, 0.25), 0.0));
        // A wall at x 2 to 2.5 m: covered within half a metre of it, not beyond.
        assert!(footprint.covers(at(1.75, 0.25), 0.5));
        assert!(!footprint.covers(at(1.25, 0.25), 0.5));
        assert!(footprint.covers(at(0.0, 6.5), 0.0));
        assert!(!footprint.covers(at(0.0, 7.5), 0.25));
    }
}
