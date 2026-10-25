//! Boxes dug face to face: the room their union makes across the face they share.
//!
//! Each dig takes its shape's distance from the ground's (`max(ground, -box)`, D21), which
//! is `max(ground, -min(box, box, ...))`: the union of the digs as one `min`, the standard
//! CSG difference. A `min` union is exact outside the shapes but only a bound inside them
//! (Quilez, "Interior SDFs", iquilezles.org/articles/interiordistance), and where two boxes
//! meet face to face the bound falls to nothing: each box is 0 on the face they share, so
//! the field holds a wall of no thickness there, which the mover (D53) and the mesher both
//! meet. Engines that keep the field in voxels lose it between samples (Godot Voxel Tools'
//! `VoxelTool::sdf_blend`, MODE_REMOVE, writes `max(sdf, -shape)` into stored voxels and
//! collides with the mesh); ours is asked at any point (D21), so the union's inside is
//! built instead. Two boxes dug in one frame that touch or overlap along one of its axes
//! and overlap across the other two hold a third box (a bridge): their span along that
//! axis by their overlap across it. Every bridge is dug too, with the bridges of bridges,
//! keeping only the largest. A point inside the union then lies inside some box with room
//! around it, and every box lies inside the union, so the field stays a bound that never
//! says the ground is farther than it is (the crease rule, D53).

use super::{Edit, box_distance};
use glam::{Quat, Vec3};

/// Faces this close are one face: two boxes laid in one frame meet within rounding.
const TOUCH_M: f32 = 0.01;
/// The least overlap across a shared face that is a way through, not a crack.
const LEAST_M: f32 = 0.02;
/// The most bridges one dig adds, so a dig never costs more than a bounded amount.
const MOST_PER_DIG: usize = 64;

/// A box dug in a frame, as its corners in that frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Room {
    /// The edit it is dug after.
    pub after: usize,
    /// The earliest edit whose ground it takes: no fill since may touch it.
    since: usize,
    rotation: Quat,
    low: Vec3,
    high: Vec3,
    /// The dig it is, as the field asks it.
    pub edit: Edit,
}

impl Room {
    /// A dug box as a room, at its place in the edits.
    pub fn of(edit: &Edit, index: usize) -> Option<Room> {
        let Edit::DigBox {
            centre,
            half,
            rotation,
        } = *edit
        else {
            return None;
        };
        let middle = rotation.inverse() * centre;
        Some(Room {
            after: index,
            since: index,
            rotation,
            low: middle - half,
            high: middle + half,
            edit: *edit,
        })
    }

    fn new(rotation: Quat, low: Vec3, high: Vec3, after: usize, since: usize) -> Room {
        Room {
            after,
            since,
            rotation,
            low,
            high,
            edit: Edit::DigBox {
                centre: rotation * ((low + high) * 0.5),
                half: (high - low) * 0.5,
                rotation,
            },
        }
    }

    fn same_frame(&self, other: &Room) -> bool {
        self.rotation.abs_diff_eq(other.rotation, 1e-6)
            || self.rotation.abs_diff_eq(-other.rotation, 1e-6)
    }

    /// Whether another room of its frame lies inside it.
    pub fn holds(&self, other: &Room) -> bool {
        self.same_frame(other)
            && (self.low - TOUCH_M).cmple(other.low).all()
            && other.high.cmple(self.high + TOUCH_M).all()
    }

    /// Whether a sphere reaches inside it.
    pub fn touched_by(&self, centre: Vec3, radius: f32) -> bool {
        let Edit::DigBox {
            centre: middle,
            half,
            rotation,
        } = self.edit
        else {
            return false;
        };
        box_distance(centre, middle, half, rotation) < radius
    }

    /// The boxes it and another room of its frame hold together across a face they share,
    /// dug after the given edit.
    pub fn bridges(&self, other: &Room, after: usize) -> Vec<Room> {
        if !self.same_frame(other) {
            return Vec::new();
        }
        let (low, high) = (self.low.max(other.low), self.high.min(other.high));
        let across = high - low;
        (0..3)
            .filter(|&k| across[k] >= -TOUCH_M && (0..3).all(|j| j == k || across[j] >= LEAST_M))
            .map(|k| {
                let (mut l, mut h) = (low, high);
                l[k] = self.low[k].min(other.low[k]);
                h[k] = self.high[k].max(other.high[k]);
                Room::new(self.rotation, l, h, after, self.since.min(other.since))
            })
            .filter(|room| !self.holds(room) && !other.holds(room))
            .collect()
    }
}

/// Adds the bridges a box just dug makes with those dug before it in its frame: each one no
/// box already holds, and no fill since touches (digging it later would take that fill).
/// `refilled(from, room)` says whether a fill after edit `from` reaches inside a room.
pub(super) fn join(
    fresh: Room,
    dug: &[Room],
    rooms: &mut Vec<Room>,
    refilled: impl Fn(usize, &Room) -> bool,
) {
    let after = fresh.after;
    let mut work = vec![fresh];
    let mut added = 0;
    while let Some(room) = work.pop() {
        let others: Vec<Room> = dug
            .iter()
            .chain([&fresh])
            .chain(rooms.iter())
            .copied()
            .collect();
        for other in &others {
            for bridge in room.bridges(other, after) {
                let held = |r: &Room| r.holds(&bridge);
                if dug.iter().chain([&fresh]).chain(rooms.iter()).any(held)
                    || refilled(bridge.since, &bridge)
                {
                    continue;
                }
                // A bridge it holds is dug in it, unless a fill since that one was dug
                // came between.
                rooms.retain(|r| !bridge.holds(r) || refilled(r.after, r));
                rooms.push(bridge);
                work.push(bridge);
                added += 1;
                if added >= MOST_PER_DIG {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(low: [f32; 3], high: [f32; 3], index: usize) -> Room {
        let (low, high) = (Vec3::from(low), Vec3::from(high));
        Room::of(
            &Edit::DigBox {
                centre: (low + high) * 0.5,
                half: (high - low) * 0.5,
                rotation: Quat::IDENTITY,
            },
            index,
        )
        .unwrap()
    }

    /// Joins boxes dug one after another and returns the bridges.
    fn joined(boxes: &[Room]) -> Vec<Room> {
        let mut rooms = Vec::new();
        for (i, &fresh) in boxes.iter().enumerate() {
            join(fresh, &boxes[..i], &mut rooms, |_, _| false);
        }
        rooms
    }

    fn spans(rooms: &[Room]) -> Vec<([f32; 3], [f32; 3])> {
        rooms
            .iter()
            .map(|r| {
                let round = |v: Vec3| (v * 100.0).round().to_array().map(|x| x / 100.0);
                (round(r.low), round(r.high))
            })
            .collect()
    }

    #[test]
    fn two_boxes_face_to_face_hold_the_box_they_make() {
        let rooms = joined(&[
            room([0.0, 0.0, 0.0], [2.0, 2.0, 2.0], 0),
            room([2.0, 0.0, 0.0], [4.0, 2.0, 2.0], 1),
        ]);
        assert_eq!(spans(&rooms), vec![([0.0, 0.0, 0.0], [4.0, 2.0, 2.0])]);
    }

    #[test]
    fn treads_of_different_depths_hold_their_overlap_across_and_no_more() {
        // A tread 0.6 deep then one 0.9 deep: they share the room down to 0.6.
        let rooms = joined(&[
            room([0.0, -0.6, 0.0], [3.0, 1.0, 3.0], 0),
            room([3.0, -0.9, 0.0], [6.0, 1.0, 3.0], 1),
        ]);
        assert_eq!(spans(&rooms), vec![([0.0, -0.6, 0.0], [6.0, 1.0, 3.0])]);
    }

    #[test]
    fn four_boxes_round_a_corner_hold_the_whole() {
        // Two by two, the corner line where all four meet is inside the bridge of bridges.
        let rooms = joined(&[
            room([0.0, 0.0, 0.0], [1.0, 1.0, 1.0], 0),
            room([1.0, 0.0, 0.0], [2.0, 1.0, 1.0], 1),
            room([0.0, 0.0, 1.0], [1.0, 1.0, 2.0], 2),
            room([1.0, 0.0, 1.0], [2.0, 1.0, 2.0], 3),
        ]);
        let whole = Room::new(Quat::IDENTITY, Vec3::ZERO, Vec3::new(2.0, 1.0, 2.0), 3, 0);
        assert!(rooms.iter().any(|r| r.holds(&whole)), "{:?}", spans(&rooms));
        // Only the largest are kept.
        assert_eq!(rooms.len(), 1, "{:?}", spans(&rooms));
    }

    #[test]
    fn boxes_apart_or_meeting_at_an_edge_or_turned_hold_nothing() {
        let a = room([0.0, 0.0, 0.0], [1.0, 1.0, 1.0], 0);
        assert!(joined(&[a, room([1.5, 0.0, 0.0], [2.5, 1.0, 1.0], 1)]).is_empty());
        assert!(joined(&[a, room([1.0, 1.0, 0.0], [2.0, 2.0, 1.0], 1)]).is_empty());
        let mut turned = room([1.0, 0.0, 0.0], [2.0, 1.0, 1.0], 1);
        turned.rotation = Quat::from_rotation_y(0.1);
        assert!(joined(&[a, turned]).is_empty());
    }

    #[test]
    fn a_fill_between_two_digs_keeps_their_bridge_out() {
        let boxes = [
            room([0.0, 0.0, 0.0], [2.0, 2.0, 2.0], 0),
            room([2.0, 0.0, 0.0], [4.0, 2.0, 2.0], 2),
        ];
        let mut rooms = Vec::new();
        join(boxes[1], &boxes[..1], &mut rooms, |from, _| from < 1);
        assert!(rooms.is_empty());
    }
}
