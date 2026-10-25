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
//!
//! Boxes each square to the ground at its own spot are turned a little from each other on
//! a curved planet (a thousandth of a radian 2 m apart on a 2 km one), so their shared face
//! is two faces crossing, a sliver of ground mm thick between them on one side. A turned
//! box joins as the box square to the other's frame that it holds (`Room::square_to`), its
//! faces then within 2 cm, and the bridge spans the sliver: digs it out, never more than
//! the faces' touch past the two. Rooms turned too far (2 cm of lean) are not one face.
//! Bridges are found when a box is dug, among the boxes and bridges within reach of it;
//! one with a turned box is kept to the dug box's neighbourhood, so a trench dug box by box
//! along a curved planet holds one bridge a seam, not ever more reaching back along it.

use super::{Edit, box_distance};
use glam::{Quat, Vec3};

/// Faces this close are one face: two boxes laid in one frame meet within rounding.
const TOUCH_M: f32 = 0.01;
/// The least overlap across a shared face that is a way through, not a crack.
const LEAST_M: f32 = 0.02;
/// How far inside a room turned a little from another's frame the box square to that frame
/// may lie, at most: past it the two are turned too far for their faces to be one face.
/// Rooms each square to the ground at its own spot lean 1 mm a metre apart on a 1 km planet.
const MOST_INSET_M: f32 = 0.02;
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

    /// The largest box square to a frame that this room holds round its middle (`grow`
    /// -1), or the least that holds it (`grow` 1), and how far that box's faces lie from its
    /// own at most: for a room turned a little from the frame (as two rooms each square to
    /// the ground at its own spot on a curved planet are), or none when it is turned so far
    /// that its faces are not the frame's.
    fn square_to(&self, rotation: Quat, grow: f32) -> Option<(Room, f32)> {
        if self.rotation.abs_diff_eq(rotation, 1e-6) || self.rotation.abs_diff_eq(-rotation, 1e-6) {
            return Some((*self, 0.0));
        }
        // Its axes in the frame's terms; off the diagonal is how far each leans into another.
        let turn = glam::Mat3::from_quat(rotation.inverse() * self.rotation);
        let lean = (0..3)
            .flat_map(|i| (0..3).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| turn.col(j)[i].abs())
            .fold(0.0, f32::max);
        // Along each axis a turned box reaches its own half size (times at most 1) and at
        // most `lean` times each other; a box square to the frame of half sizes `inner` lies
        // inside it when, along each of its axes, what those reach is no more than its own.
        let half = (self.high - self.low) * 0.5;
        let sized = Vec3::from_array(std::array::from_fn(|i| {
            half[i] + grow * lean * (half.element_sum() - half[i])
        }));
        let moved = (half - sized).abs().max_element();
        if moved > MOST_INSET_M || sized.min_element() <= 0.0 {
            return None;
        }
        let middle = rotation.inverse() * (self.rotation * ((self.low + self.high) * 0.5));
        let room = Room::new(
            rotation,
            middle - sized,
            middle + sized,
            self.after,
            self.since,
        );
        Some((room, moved))
    }

    /// Whether another room lies inside it (one turned a little, by the box that holds it).
    pub fn holds(&self, other: &Room) -> bool {
        other
            .square_to(self.rotation, 1.0)
            .is_some_and(|(other, _)| {
                (self.low - TOUCH_M).cmple(other.low).all()
                    && other.high.cmple(self.high + TOUCH_M).all()
            })
    }

    /// Whether it may meet another room: their spheres touch.
    fn near(&self, other: &Room) -> bool {
        let (at, reach) = self.edit.centre_and_radius();
        let (there, far) = other.edit.centre_and_radius();
        at.distance(there) <= reach + far + TOUCH_M + MOST_INSET_M
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

    /// The boxes it and another room hold together across a face they share, in its frame,
    /// dug after the given edit. A room turned a little from its frame is taken as the box
    /// square to the frame inside it, so their faces meet within that box's inset too.
    pub fn bridges(&self, other: &Room, after: usize) -> Vec<Room> {
        let Some((other, inset)) = other.square_to(self.rotation, -1.0) else {
            return Vec::new();
        };
        let other = &other;
        let (low, high) = (self.low.max(other.low), self.high.min(other.high));
        let across = high - low;
        let touch = TOUCH_M + inset;
        (0..3)
            .filter(|&k| across[k] >= -touch && (0..3).all(|j| j == k || across[j] >= LEAST_M))
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
    // Every bridge it makes is in its frame. One made with a room turned from that frame
    // is kept to the room's own neighbourhood (a box its size again each way): rooms each
    // turned a little from the last (a trench along a curved planet) would otherwise make
    // bridges reaching back along it as far as the turns allow, a few more every dig, where
    // one across each seam is all a way through needs.
    let size = fresh.high - fresh.low;
    let (near_low, near_high) = (fresh.low - size, fresh.high + size);
    let mut work = vec![fresh];
    let mut added = 0;
    while let Some(room) = work.pop() {
        let others: Vec<Room> = dug
            .iter()
            .chain([&fresh])
            .chain(rooms.iter())
            .filter(|other| room.near(other))
            .copied()
            .collect();
        for other in &others {
            let turned = other
                .square_to(fresh.rotation, 1.0)
                .is_some_and(|(_, by)| by > 0.0);
            for mut bridge in room.bridges(other, after) {
                if turned {
                    bridge = Room::new(
                        bridge.rotation,
                        bridge.low.max(near_low),
                        bridge.high.min(near_high),
                        bridge.after,
                        bridge.since,
                    );
                }
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

    /// A room as `room` makes it, turned about its middle by a rotation.
    fn turned(low: [f32; 3], high: [f32; 3], index: usize, by: Quat) -> Room {
        let (low, high) = (Vec3::from(low), Vec3::from(high));
        let edit = Edit::DigBox {
            centre: (low + high) * 0.5,
            half: (high - low) * 0.5,
            rotation: by,
        };
        Room::of(&edit, index).unwrap()
    }

    #[test]
    fn rooms_turned_a_little_hold_a_box_across_their_faces_that_lies_inside_both() {
        // Two thousandths of a radian about the face's own up: two rooms each square to the
        // ground at its spot, 4 m apart on a 2 km planet.
        let by = Quat::from_rotation_z(0.002);
        let first = room([0.0, 0.0, 0.0], [2.0, 2.0, 2.0], 0);
        let second = turned([2.0, 0.0, 0.0], [4.0, 2.0, 2.0], 1, by);
        let rooms = joined(&[first, second]);
        assert_eq!(rooms.len(), 1, "{:?}", spans(&rooms));
        let bridge = rooms[0];
        // Across the face and along it, from one room's far wall to the other's, within what
        // the turn leans (2 mm a metre).
        let corners = |r: &Room| [r.rotation * r.low, r.rotation * r.high];
        let [low, high] = corners(&bridge);
        assert!(low.abs_diff_eq(Vec3::ZERO, 0.02), "{low}");
        assert!(high.x > 3.97 && high.y > 1.97 && high.z > 1.97, "{high}");
        // Nowhere outside the two rooms by more than the faces' touch.
        let outside = |p: Vec3| {
            let d = |r: &Room| {
                let Edit::DigBox {
                    centre,
                    half,
                    rotation,
                } = r.edit
                else {
                    unreachable!()
                };
                box_distance(p, centre, half, rotation)
            };
            d(&first).min(d(&second))
        };
        let steps = 8;
        for i in 0..=steps {
            for j in 0..=steps {
                for k in 0..=steps {
                    let t = Vec3::new(i as f32, j as f32, k as f32) / steps as f32;
                    let p = bridge.rotation * (bridge.low + (bridge.high - bridge.low) * t);
                    assert!(outside(p) < TOUCH_M + MOST_INSET_M, "{p} {}", outside(p));
                }
            }
        }
    }

    #[test]
    fn a_trench_of_rooms_each_turned_from_the_last_holds_a_bridge_a_seam() {
        // Rooms 2 m long one after another, each a thousandth of a radian on from the last
        // about the trench's side (square to a 2 km planet's ground at its spot): the field
        // has room across every seam, and a dig costs a bridge, not ever more.
        let n = 40;
        let boxes: Vec<Room> = (0..n)
            .map(|i| {
                let x = 2.0 * i as f32;
                let by = Quat::from_rotation_z(0.001 * i as f32);
                turned([x, 0.0, 0.0], [x + 2.0, 2.0, 2.0], i, by)
            })
            .collect();
        let rooms = joined(&boxes);
        assert!(rooms.len() <= n, "{} bridges for {n} rooms", rooms.len());
        for seam in 1..n {
            let at = Vec3::new(2.0 * seam as f32, 1.0, 1.0);
            let deepest = boxes
                .iter()
                .chain(rooms.iter())
                .map(|r| {
                    let Edit::DigBox {
                        centre,
                        half,
                        rotation,
                    } = r.edit
                    else {
                        unreachable!()
                    };
                    -box_distance(at, centre, half, rotation)
                })
                .fold(f32::MIN, f32::max);
            assert!(deepest > 0.9, "{deepest} m of room at seam {seam}");
        }
    }

    #[test]
    fn a_fill_between_two_rooms_turned_a_little_keeps_their_bridge_out() {
        let by = Quat::from_rotation_z(0.002);
        let first = room([0.0, 0.0, 0.0], [2.0, 2.0, 2.0], 0);
        let second = turned([2.0, 0.0, 0.0], [4.0, 2.0, 2.0], 2, by);
        let mut rooms = Vec::new();
        join(second, &[first], &mut rooms, |from, _| from < 1);
        assert!(rooms.is_empty());
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
