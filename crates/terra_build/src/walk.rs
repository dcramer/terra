//! Walking in and around a base: what holds a walker up and what stops them.
//!
//! This is deliberately simple. A base is cubes on a grid, so a walker can be handled with
//! arithmetic instead of a physics engine: the tops of cubes hold them up, and solid and
//! glass cubes stop them, while openings let them through. Coordinates are the base's own,
//! in metres, with `y` at the feet.

use crate::{Base, CELL_M, Fill, STOREY_M};
use glam::{IVec2, IVec3, Vec3};

/// The size of something walking through a base.
#[derive(Clone, Copy, Debug)]
pub struct Walker {
    pub radius: f32,
    pub height: f32,
    /// The tallest step that can be walked up without jumping.
    pub step_up: f32,
}

impl Default for Walker {
    fn default() -> Self {
        Walker {
            radius: 0.3,
            height: 1.8,
            step_up: 0.4,
        }
    }
}

/// The column of cubes a point is over.
fn column_at(point: Vec3) -> IVec2 {
    IVec2::new(
        (point.x / CELL_M).floor() as i32,
        (point.z / CELL_M).floor() as i32,
    )
}

impl Base {
    /// The layers of a column of cubes that are not openings, with their bottoms and tops.
    fn solid_layers(&self, column: IVec2) -> impl Iterator<Item = (f32, f32)> + '_ {
        self.column(column.x, column.y)
            .filter(move |&layer| {
                self.cube(IVec3::new(column.x, layer, column.y))
                    .is_some_and(|cube| cube.fill != Fill::Open)
            })
            .map(|layer| (layer as f32 * CELL_M, (layer + 1) as f32 * CELL_M))
    }

    /// The height of whatever would hold up feet at `point`: the highest top of a cube, or
    /// a pitched roof, in that column that is no more than a step above them. `None` if
    /// the base offers nothing there, in which case the ground decides.
    pub fn support(&self, point: Vec3, walker: &Walker) -> Option<f32> {
        let tops = self.solid_layers(column_at(point)).map(|(_, top)| top);
        // A pitched roof is stood on where it is.
        let pitched = self.sections().iter().filter_map(|section| {
            let rise = section.rise(glam::Vec2::new(point.x, point.z))?;
            Some((section.storey() + 1) as f32 * STOREY_M + rise)
        });
        tops.chain(pitched)
            .filter(|&surface| surface <= point.y + walker.step_up)
            .max_by(|a, b| a.total_cmp(b))
    }

    /// The height of the ceiling over a point: the lowest cube, or pitched roof's
    /// underside, over it.
    pub fn ceiling(&self, point: Vec3) -> Option<f32> {
        let above = point.y + 0.1;
        let bottoms = self
            .solid_layers(column_at(point))
            .map(|(bottom, _)| bottom);
        let pitched = self.sections().iter().filter_map(|section| {
            let rise = section.rise(glam::Vec2::new(point.x, point.z))?;
            Some((section.storey() + 1) as f32 * STOREY_M + rise - crate::mesh::ROOF_THICK_M)
        });
        bottoms
            .chain(pitched)
            .filter(|&bottom| bottom >= above)
            .min_by(|a, b| a.total_cmp(b))
    }

    /// Moves a walker along the floor from `from` toward `to`, stopping at walls and
    /// windows and passing through openings. Returns where they end up. Height is taken
    /// from `to` unchanged; use [`Base::support`] for that.
    pub fn walk(&self, from: Vec3, to: Vec3, walker: &Walker) -> Vec3 {
        let mut at = from;
        // One axis at a time, so that walking into a wall at an angle slides along it.
        for axis in [0, 2] {
            let mut wanted = to[axis];
            let step = wanted - at[axis];
            if step != 0.0 {
                let direction = step.signum();
                let column = column_at(at);
                let index = if axis == 0 { 0 } else { 1 };
                let edge = (column[index] + i32::from(direction > 0.0)) as f32 * CELL_M;
                let reaches_edge = (wanted + direction * walker.radius - edge) * direction > 0.0;
                // A walker is wider than a point: everything across their width has to
                // be open, so a doorway one narrow cell wide does not let them through.
                let sideways = 2 - axis;
                let stopped = [-walker.radius, 0.0, walker.radius].iter().any(|&shift| {
                    let mut beside = at;
                    beside[sideways] += shift;
                    let mut next = column_at(beside);
                    next[index] += direction as i32;
                    self.blocks(next, at.y, walker)
                });
                if reaches_edge && stopped {
                    wanted = edge - direction * (walker.radius + 0.01);
                    // Never be pushed backward by a wall already too close.
                    if (wanted - at[axis]) * direction < 0.0 {
                        wanted = at[axis];
                    }
                }
            }
            at[axis] = wanted;
        }
        at.y = to.y;
        at
    }

    /// Whether a column of cubes has something solid where a walker's body would be,
    /// above what they can step up.
    fn blocks(&self, column: IVec2, feet: f32, walker: &Walker) -> bool {
        let (low, high) = (feet + walker.step_up, feet + walker.height);
        self.solid_layers(column)
            .any(|(bottom, top)| top > low && bottom < high)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildOp, Kind, Panel, cells};

    /// A room nine metres square outside on the ground, with a doorway a metre wide in its
    /// south wall and a window in its east wall. These tests are written in metres and
    /// assume the grid divides half a metre.
    fn room() -> Base {
        let mut base = Base::default();
        let far = cells(9.0) - 1;
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [far, 0, far],
            kind: Kind::PLAIN,
        });
        for x in cells(4.0)..cells(5.0) {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, far],
                panel: Some(Panel::Door),
                kind: Kind::PLAIN,
            });
        }
        for z in cells(3.0)..cells(6.0) {
            base.apply(BuildOp::SetWall {
                tile: [far, 0, z],
                panel: Some(Panel::WINDOW),
                kind: Kind::PLAIN,
            });
        }
        base
    }

    fn walker() -> Walker {
        Walker::default()
    }

    /// Walks in small steps, as a game would frame by frame.
    fn stroll(base: &Base, from: Vec3, to: Vec3) -> Vec3 {
        let mut at = from;
        for i in 1..=400 {
            let target = from.lerp(to, i as f32 / 400.0);
            at = base.walk(at, Vec3::new(target.x, at.y, target.z), &walker());
        }
        at
    }

    #[test]
    fn a_door_lets_you_in_and_out() {
        let base = room();
        let inside = stroll(&base, Vec3::new(4.5, 0.0, 14.0), Vec3::new(4.5, 0.0, 4.5));
        assert!((inside.z - 4.5).abs() < 0.01, "stopped at z {}", inside.z);
        let back_out = stroll(&base, inside, Vec3::new(4.5, 0.0, 14.0));
        assert!((back_out.z - 14.0).abs() < 0.01);
    }

    #[test]
    fn a_doorway_narrower_than_a_walker_keeps_them_out() {
        let mut base = room();
        // One cell of doorway in the north wall.
        base.apply(BuildOp::SetWall {
            tile: [cells(4.5), 0, 0],
            panel: Some(Panel::Door),
            kind: Kind::PLAIN,
        });
        let at = Vec3::new(4.5 + CELL_M * 0.5, 0.0, 4.5);
        let stopped = stroll(&base, at, Vec3::new(at.x, 0.0, -6.0));
        assert!(stopped.z > 0.0, "squeezed through at z {}", stopped.z);
    }

    #[test]
    fn walls_and_windows_stop_you_from_both_sides() {
        let base = room();
        // Walking north at a solid part of the south wall, from outside.
        let outside = stroll(&base, Vec3::new(1.5, 0.0, 14.0), Vec3::new(1.5, 0.0, 4.5));
        assert!(
            (outside.z - (9.0 + 0.31)).abs() < 0.02,
            "stopped at z {}",
            outside.z
        );
        // Walking east at the window, from inside.
        let at_glass = stroll(&base, Vec3::new(4.5, 0.0, 4.5), Vec3::new(14.0, 0.0, 4.5));
        assert!(
            (at_glass.x - (8.5 - 0.31)).abs() < 0.02,
            "stopped at x {}",
            at_glass.x
        );
        // Clipping the side of the doorway counts as hitting the wall beside it.
        let beside_door = stroll(&base, Vec3::new(3.9, 0.0, 14.0), Vec3::new(3.9, 0.0, 4.5));
        assert!(beside_door.z > 9.0);
    }

    #[test]
    fn walking_into_a_wall_at_an_angle_slides_along_it() {
        let base = room();
        let slid = stroll(&base, Vec3::new(1.5, 0.0, 4.5), Vec3::new(7.5, 0.0, -6.0));
        assert!(slid.z > 0.5, "stopped by the north wall");
        assert!((slid.x - 7.5).abs() < 0.01, "but carried on east along it");
    }

    #[test]
    fn floors_and_roofs_hold_you_up() {
        let mut base = room();
        let w = walker();
        // Inside: the floor. A little below it: the floor is still within a step. Far
        // below: nothing.
        assert_eq!(base.support(Vec3::new(4.5, 0.0, 4.5), &w), Some(0.0));
        assert_eq!(base.support(Vec3::new(4.5, -0.3, 4.5), &w), Some(0.0));
        assert_eq!(base.support(Vec3::new(4.5, -2.0, 4.5), &w), None);
        // Beside the base: nothing. In the doorway: its threshold.
        assert_eq!(base.support(Vec3::new(-4.5, 0.0, 4.5), &w), None);
        assert_eq!(base.support(Vec3::new(4.25, 0.0, 8.75), &w), Some(0.0));
        // On the roof.
        assert_eq!(base.support(Vec3::new(4.5, 3.2, 4.5), &w), Some(3.0));
        // With an upper storey, its floor rather than the roof below.
        base.apply(BuildOp::AddRoom {
            min: [cells(3.0), 1, cells(3.0)],
            max: [cells(6.0), 1, cells(6.0)],
            kind: Kind::PLAIN,
        });
        assert_eq!(base.support(Vec3::new(4.5, 3.0, 4.5), &w), Some(3.0));
        assert_eq!(base.support(Vec3::new(4.5, 6.5, 4.5), &w), Some(6.0));
    }

    #[test]
    fn there_is_a_ceiling_only_under_a_roof() {
        let base = room();
        assert_eq!(base.ceiling(Vec3::new(4.5, 0.0, 4.5)), Some(2.5));
        assert_eq!(base.ceiling(Vec3::new(4.5, 0.0, 14.0)), None);
        assert_eq!(base.ceiling(Vec3::new(4.5, 3.0, 4.5)), None);
    }

    #[test]
    fn walking_over_a_base_is_not_blocked_by_its_walls() {
        let base = room();
        // On the roof, walking off the east edge.
        let off_roof = stroll(&base, Vec3::new(4.5, 3.0, 4.5), Vec3::new(14.0, 3.0, 4.5));
        assert!((off_roof.x - 14.0).abs() < 0.01);
    }

    #[test]
    fn an_interior_wall_stops_you_and_its_doorway_lets_you_through() {
        let mut base = room();
        // A wall right across the room, two and a half metres from its west side.
        let wall = |z: i32, panel| BuildOp::SetWall {
            tile: [cells(2.5), 0, z],
            panel: Some(panel),
            kind: Kind::PLAIN,
        };
        let (west, east) = (Vec3::new(1.5, 0.0, 4.5), Vec3::new(4.5, 0.0, 4.5));
        assert!((stroll(&base, west, east).x - east.x).abs() < 0.01);
        for z in 1..cells(9.0) - 1 {
            base.apply(wall(z, Panel::Solid));
        }
        assert!(stroll(&base, west, east).x < 2.5);
        assert!(stroll(&base, east, west).x > 3.0);
        for z in cells(4.0)..cells(5.0) {
            base.apply(wall(z, Panel::Door));
        }
        assert!((stroll(&base, west, east).x - east.x).abs() < 0.01);
    }
}
