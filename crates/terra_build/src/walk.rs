//! Walking in and around a base: what holds a walker up and what stops them.
//!
//! This is deliberately simple. A base is a grid, so its walls are on tile edges and its
//! floors on storey heights, and a walker can be handled with arithmetic instead of a
//! physics engine. Coordinates are the base's own, in metres, with `y` at the feet.

use crate::{Base, DOOR_HEIGHT_M, DOOR_WIDTH_M, Face, Panel, STOREY_M, Side, TILE_M};
use glam::{IVec3, Vec3};

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

/// The tile a point is in. Feet exactly on a floor count as being in the tile above it.
fn tile_at(point: Vec3) -> IVec3 {
    IVec3::new(
        (point.x / TILE_M).floor() as i32,
        ((point.y + 0.05) / STOREY_M).floor() as i32,
        (point.z / TILE_M).floor() as i32,
    )
}

impl Base {
    /// The height of whatever would hold up feet at `point`: the highest floor or roof in
    /// that column that is no more than a step above them. `None` if the base offers
    /// nothing there, in which case the ground decides.
    pub fn support(&self, point: Vec3, walker: &Walker) -> Option<f32> {
        let column = tile_at(point);
        self.tiles()
            .filter(|(tile, _)| tile.x == column.x && tile.z == column.z)
            .flat_map(|(tile, _)| {
                // A tile's floor, and its roof when nothing is built on top.
                let floor = tile.y as f32 * STOREY_M;
                let roof = (!self.contains(tile + IVec3::Y)).then_some(floor + STOREY_M);
                [Some(floor), roof]
            })
            .flatten()
            .filter(|&surface| surface <= point.y + walker.step_up)
            .max_by(|a, b| a.total_cmp(b))
    }

    /// The height of the ceiling over a point inside the base, if it is inside.
    pub fn ceiling(&self, point: Vec3) -> Option<f32> {
        let tile = tile_at(point);
        self.contains(tile)
            .then_some((tile.y + 1) as f32 * STOREY_M)
    }

    /// Moves a walker along the floor from `from` toward `to`, stopping at walls and
    /// windows and passing through doors. Returns where they end up. Height is taken from
    /// `to` unchanged; use [`Base::support`] for that.
    pub fn walk(&self, from: Vec3, to: Vec3, walker: &Walker) -> Vec3 {
        let mut at = from;
        // One axis at a time, so that walking into a wall at an angle slides along it.
        for (axis, positive, negative) in
            [(0, Side::East, Side::West), (2, Side::South, Side::North)]
        {
            let mut wanted = to[axis];
            let step = wanted - at[axis];
            if step != 0.0 {
                let (direction, side) = if step > 0.0 {
                    (1.0, positive)
                } else {
                    (-1.0, negative)
                };
                let tile = tile_at(at);
                let edge = (tile[axis] + i32::from(direction > 0.0)) as f32 * TILE_M;
                let reaches_edge = (wanted + direction * walker.radius - edge) * direction > 0.0;
                if reaches_edge && self.blocks(tile, side, at, walker) {
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

    /// Whether stepping out of `tile` through `side` is stopped by the hull.
    fn blocks(&self, tile: IVec3, side: Side, at: Vec3, walker: &Walker) -> bool {
        let beyond = tile + side.offset();
        // The hull face between the two tiles belongs to whichever of them is inside.
        let face = match (self.contains(tile), self.contains(beyond)) {
            (true, false) => Face::new(tile, side),
            (false, true) => Face::new(beyond, opposite(side)),
            // Both inside, or both outside: nothing in the way.
            _ => return false,
        };
        if self.panel(face) != Panel::Door {
            return true;
        }
        // A door lets a walker through only where its opening is.
        let along = if matches!(side, Side::East | Side::West) {
            at.z
        } else {
            at.x
        };
        let from_centre = (along.rem_euclid(TILE_M) - TILE_M * 0.5).abs();
        let floor = face.tile[1] as f32 * STOREY_M;
        let fits_width = from_centre + walker.radius <= DOOR_WIDTH_M * 0.5 + 0.05;
        let fits_height = at.y + walker.height <= floor + DOOR_HEIGHT_M + walker.step_up;
        !(fits_width && fits_height)
    }
}

fn opposite(side: Side) -> Side {
    match side {
        Side::East => Side::West,
        Side::West => Side::East,
        Side::South => Side::North,
        Side::North => Side::South,
        Side::Up => Side::Down,
        Side::Down => Side::Up,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BuildOp;

    /// A 3 by 3 room on the ground with a door in the middle of its south wall and a
    /// window in the middle of its east wall.
    fn room() -> Base {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [2, 0, 2],
        });
        base.apply(BuildOp::SetPanel(
            Face::new(IVec3::new(1, 0, 2), Side::South),
            Panel::Door,
        ));
        base.apply(BuildOp::SetPanel(
            Face::new(IVec3::new(2, 0, 1), Side::East),
            Panel::WIDE_WINDOW,
        ));
        base
    }

    fn walker() -> Walker {
        Walker::default()
    }

    /// Walks in small steps, as a game would frame by frame.
    fn stroll(base: &Base, from: Vec3, to: Vec3) -> Vec3 {
        let mut at = from;
        for i in 1..=200 {
            let target = from.lerp(to, i as f32 / 200.0);
            at = base.walk(at, Vec3::new(target.x, at.y, target.z), &walker());
        }
        at
    }

    #[test]
    fn a_door_lets_you_in_and_out() {
        let base = room();
        // South of the room, lined up with the door, walking north into the room.
        let inside = stroll(&base, Vec3::new(4.5, 0.0, 14.0), Vec3::new(4.5, 0.0, 4.5));
        assert!((inside.z - 4.5).abs() < 0.01, "stopped at z {}", inside.z);
        let back_out = stroll(&base, inside, Vec3::new(4.5, 0.0, 14.0));
        assert!((back_out.z - 14.0).abs() < 0.01);
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
            (at_glass.x - (9.0 - 0.31)).abs() < 0.02,
            "stopped at x {}",
            at_glass.x
        );
        // Missing the door's opening counts as hitting the wall beside it.
        let beside_door = stroll(&base, Vec3::new(3.3, 0.0, 14.0), Vec3::new(3.3, 0.0, 4.5));
        assert!(beside_door.z > 9.0);
    }

    #[test]
    fn walking_into_a_wall_at_an_angle_slides_along_it() {
        let base = room();
        let slid = stroll(&base, Vec3::new(1.5, 0.0, 4.5), Vec3::new(7.5, 0.0, -6.0));
        assert!(slid.z > 0.0, "stopped by the north wall");
        assert!((slid.x - 7.5).abs() < 0.01, "but carried on east along it");
    }

    #[test]
    fn floors_and_roofs_hold_you_up() {
        let mut base = room();
        let w = walker();
        // Inside: the floor. Just outside the door, on lower ground: the floor is still
        // within a step. Far below: nothing.
        assert_eq!(base.support(Vec3::new(4.5, 0.0, 4.5), &w), Some(0.0));
        assert_eq!(base.support(Vec3::new(4.5, -0.3, 4.5), &w), Some(0.0));
        assert_eq!(base.support(Vec3::new(4.5, -2.0, 4.5), &w), None);
        // Beside the base: nothing.
        assert_eq!(base.support(Vec3::new(-4.5, 0.0, 4.5), &w), None);
        // On the roof.
        assert_eq!(base.support(Vec3::new(4.5, 3.2, 4.5), &w), Some(3.0));
        // With an upper storey, its floor rather than the roof below.
        base.apply(BuildOp::AddRoom {
            min: [1, 1, 1],
            max: [1, 1, 1],
        });
        assert_eq!(base.support(Vec3::new(4.5, 3.0, 4.5), &w), Some(3.0));
        assert_eq!(base.support(Vec3::new(4.5, 6.5, 4.5), &w), Some(6.0));
    }

    #[test]
    fn there_is_a_ceiling_only_indoors() {
        let base = room();
        assert_eq!(base.ceiling(Vec3::new(4.5, 0.0, 4.5)), Some(3.0));
        assert_eq!(base.ceiling(Vec3::new(4.5, 0.0, 14.0)), None);
        assert_eq!(base.ceiling(Vec3::new(4.5, 3.0, 4.5)), None);
    }

    #[test]
    fn walking_under_or_over_a_base_is_not_blocked_by_its_walls() {
        let base = room();
        // On the roof, walking off the east edge.
        let off_roof = stroll(&base, Vec3::new(4.5, 3.0, 4.5), Vec3::new(14.0, 3.0, 4.5));
        assert!((off_roof.x - 14.0).abs() < 0.01);
    }
}
