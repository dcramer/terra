//! How a door or shutters hang in the hole they are fitted into (D192, D207): their leaves,
//! each hinged on a side of the hole, and where each stands shut, open or partway. Drawing
//! and walking both read a prop's leaves from here, so what is seen is what stops a
//! walker, whichever way it stands.
//!
//! A door hangs in its hole's reveal, between the frame the kit lays round the hole, and
//! swings a quarter turn into the room; a doorway two metres wide or more takes a pair,
//! hinged either side, meeting in the middle. Shutters hang on the wall's outside face over
//! the hole and fold back flat against the wall; a hole wider than a cube takes a pair. This
//! is how real ones hang, and how Valheim's and Enshrouded's doors do.

use crate::{CELL_M, Prop};
use glam::{IVec3, Vec2, Vec3};

/// How a prop hangs in its hole: as a door, or as shutters.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Hang {
    #[default]
    Door,
    Shutters,
}

/// How far the kit's frame stands into a hole, at its sides and head: a door's leaf fits
/// inside it, clear of the jambs as it swings (`layout::roles`' jambs and heads stand
/// about 4 cm into a log wall's hole, measured in the game).
pub const FRAME_M: f32 = 0.045;
/// How far apart a door's leaf and the frame round it stand: a finger's gap.
const GAP_M: f32 = 0.008;
/// How thick a door's leaf is, and a shutter's.
pub const DOOR_THICK_M: f32 = 0.06;
pub const SHUTTER_THICK_M: f32 = 0.035;
/// How far out from a wall's middle shutters hang: over the face of a log wall and its
/// frame, where their hinges are nailed.
const SHUTTER_OUT_M: f32 = 0.19;
/// How far a door swings open, and shutters, in degrees.
const DOOR_SWING_DEGREES: f32 = 92.0;
const SHUTTER_SWING_DEGREES: f32 = 168.0;
/// How wide a hole takes a pair of leaves rather than one, in metres: a door from 1.5 m,
/// shutters from a metre.
const DOOR_PAIR_M: f32 = 1.5;
const SHUTTER_PAIR_M: f32 = 0.75;

/// One leaf of a door or of shutters, in the base's own space, in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leaf {
    /// Its hinge's foot: on the edge it turns on, on its face toward the way it opens.
    pub hinge: Vec3,
    /// Level, from the hinge across the leaf, as it stands shut.
    pub across: Vec3,
    /// Level, the way it opens: into the room for a door, out for shutters.
    pub opens: Vec3,
    pub width: f32,
    pub height: f32,
    pub thick: f32,
    /// How far it turns open, in radians.
    pub swing: f32,
    /// Whether it is a door's: its front looks out, away from the way it opens; a
    /// shutter's looks the way it opens.
    pub door: bool,
}

/// Where a leaf stands when it has turned so far open (0 shut, 1 open): its hinge, the way
/// across it from the hinge, and the way through it from its hinge face to its other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub hinge: Vec3,
    pub across: Vec3,
    pub through: Vec3,
}

impl Leaf {
    /// Where it stands turned `open` of the way, 0 to 1.
    pub fn pose(&self, open: f32) -> Pose {
        let angle = self.swing * open.clamp(0.0, 1.0);
        let (sin, cos) = (libm::sinf(angle), libm::cosf(angle));
        // Turned in the plane of `across` and `opens`: across toward opens, and the way
        // through it, which starts as against `opens`, toward across.
        Pose {
            hinge: self.hinge,
            across: self.across * cos + self.opens * sin,
            through: -self.opens * cos + self.across * sin,
        }
    }

    /// Its middle, standing so far open.
    pub fn middle(&self, open: f32) -> Vec3 {
        let pose = self.pose(open);
        pose.hinge
            + pose.across * (self.width * 0.5)
            + pose.through * (self.thick * 0.5)
            + Vec3::Y * (self.height * 0.5)
    }
}

impl Prop {
    /// The way across the wall it is fitted in, out of the wall's face: the way it opens
    /// if it was given one, else south or east by the shape of its hole.
    fn across_wall(&self) -> IVec3 {
        let opens = IVec3::from_array(self.opens);
        if opens != IVec3::ZERO {
            return opens;
        }
        if self.max[0] > self.min[0] {
            IVec3::Z
        } else {
            IVec3::X
        }
    }

    /// Its leaves, as they hang in its hole.
    pub fn leaves(&self) -> Vec<Leaf> {
        let opens = self.across_wall().as_vec3().normalize_or(Vec3::Z);
        // Along the wall, the way from its low end to its high.
        let along = Vec3::new(opens.z.abs(), 0.0, opens.x.abs());
        let low = IVec3::from_array(self.min).as_vec3() * CELL_M;
        let high = (IVec3::from_array(self.max) + IVec3::ONE).as_vec3() * CELL_M;
        let (start, end) = (low.dot(along), high.dot(along));
        let middle = (low + high) * 0.5;
        let depth = middle.dot(opens.abs());
        let door = self.hang == Hang::Door;
        let (inset, foot, top, thick, face, swing, pair) = if door {
            (
                FRAME_M + GAP_M,
                low.y + GAP_M,
                high.y - FRAME_M - GAP_M,
                DOOR_THICK_M,
                // Its hinge face toward the room, its middle in the wall's middle.
                DOOR_THICK_M * 0.5,
                DOOR_SWING_DEGREES,
                DOOR_PAIR_M,
            )
        } else {
            (
                0.0,
                low.y,
                high.y,
                SHUTTER_THICK_M,
                SHUTTER_OUT_M + SHUTTER_THICK_M,
                SHUTTER_SWING_DEGREES,
                SHUTTER_PAIR_M,
            )
        };
        let (start, end) = (start + inset, end - inset);
        let width = end - start;
        let at = |b: f32| {
            let level = along * b + opens.abs() * depth;
            Vec3::new(level.x, foot, level.z) + opens * face
        };
        let leaf = |hinge: f32, across: Vec3, width: f32| Leaf {
            hinge: at(hinge),
            across,
            opens,
            width,
            height: top - foot,
            thick,
            swing: swing.to_radians(),
            door,
        };
        if end - start + 2.0 * inset >= pair - 1e-3 {
            let half = width * 0.5;
            vec![leaf(start, along, half), leaf(end, -along, half)]
        } else {
            vec![leaf(start, along, width)]
        }
    }
}

/// Moves a walker's feet out of every leaf that stands where their body is, by the least
/// it takes, the way they came across a leaf's face: so a door shut stops them in its
/// doorway, and open stops them where it stands open in the room. Each leaf is given with
/// how far open it stands.
pub fn push_off(
    from: Vec3,
    to: Vec3,
    radius: f32,
    body: (f32, f32),
    leaves: impl IntoIterator<Item = (Leaf, f32)>,
) -> Vec3 {
    let mut at = to;
    for (leaf, open) in leaves {
        // Over the body's height, from what it can step over to its head.
        if leaf.hinge.y + leaf.height <= body.0 || leaf.hinge.y >= body.1 {
            continue;
        }
        let pose = leaf.pose(open);
        let middle = leaf.middle(open);
        let (u, v) = (
            Vec2::new(pose.across.x, pose.across.z),
            Vec2::new(pose.through.x, pose.through.z),
        );
        let local = |p: Vec3| {
            let d = Vec2::new(p.x - middle.x, p.z - middle.z);
            (d.dot(u), d.dot(v))
        };
        let (x, y) = local(at);
        let (over_x, over_y) = (
            leaf.width * 0.5 + radius - x.abs(),
            leaf.thick * 0.5 + radius - y.abs(),
        );
        if over_x <= 0.0 || over_y <= 0.0 {
            continue;
        }
        // Across its face, out the side the walker came from; past its edge, if that is
        // nearer.
        let came = local(from).1;
        let side = if came.abs() > 1e-4 {
            came.signum()
        } else {
            y.signum()
        };
        let face = leaf.thick * 0.5 + radius + 1e-3;
        let push = if over_x < over_y && side == y.signum() {
            u * x.signum() * over_x
        } else {
            v * (side * face - y)
        };
        at.x += push.x;
        at.z += push.y;
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PropKind;

    fn prop(min: [i32; 3], max: [i32; 3], opens: [i32; 3], hang: Hang) -> Prop {
        Prop {
            kind: PropKind(0),
            min,
            max,
            open: false,
            opens,
            hang,
        }
    }

    #[test]
    fn a_door_fills_its_hole_inside_the_frame_and_opens_a_quarter_turn_into_the_room() {
        // A doorway a metre wide and two high in a wall running east, the room south.
        let door = prop([2, 0, 0], [3, 3, 0], [0, 0, 1], Hang::Door);
        let leaves = door.leaves();
        assert_eq!(leaves.len(), 1);
        let leaf = leaves[0];
        // Inside the frame: a metre less the frame either side.
        assert!((leaf.width - (1.0 - 2.0 * (FRAME_M + GAP_M))).abs() < 1e-4);
        assert!(leaf.height > 1.85 && leaf.height < 2.0);
        // Shut, it stands across the doorway in the wall's middle; open, from its hinge
        // into the room, south, as far as it is wide.
        let shut = leaf.middle(0.0);
        assert!((shut.z - 0.25).abs() < 0.05, "{shut}");
        assert!((shut.x - 1.5).abs() < 1e-3);
        let open = leaf.middle(1.0);
        assert!(open.z > 0.25 + leaf.width * 0.4, "{open}");
        assert!((open.x - leaf.hinge.x).abs() < leaf.thick + 0.01);
    }

    #[test]
    fn wide_holes_take_a_pair_meeting_in_the_middle() {
        let double = prop([0, 0, 4], [0, 3, 7], [1, 0, 0], Hang::Door);
        let leaves = double.leaves();
        assert_eq!(leaves.len(), 2);
        // Along Z, hinged at either end, each half the clear width, meeting in the middle.
        let ends: Vec<f32> = leaves
            .iter()
            .map(|leaf| leaf.hinge.z + leaf.across.z * leaf.width)
            .collect();
        assert!((ends[0] - 3.0).abs() < 1e-4 && (ends[1] - 3.0).abs() < 1e-4);
        // Shutters over a window a metre wide take a pair; half a metre, one.
        let pair = prop([2, 2, 0], [3, 3, 0], [0, 0, -1], Hang::Shutters);
        assert_eq!(pair.leaves().len(), 2);
        let one = prop([2, 2, 0], [2, 3, 0], [0, 0, -1], Hang::Shutters);
        assert_eq!(one.leaves().len(), 1);
    }

    #[test]
    fn shutters_hang_outside_and_fold_back_against_the_wall() {
        // A window in a wall running east, the outside north.
        let shutters = prop([2, 2, 0], [3, 3, 0], [0, 0, -1], Hang::Shutters);
        for leaf in shutters.leaves() {
            let shut = leaf.middle(0.0);
            assert!(
                shut.z < 0.25 - SHUTTER_OUT_M + 0.01,
                "outside the wall: {shut}"
            );
            // Open, beside the window rather than over it, still close to the wall.
            let open = leaf.middle(1.0);
            assert!(open.x < 1.0 || open.x > 2.0, "{open}");
            assert!(open.z < 0.0 && open.z > -0.4, "{open}");
        }
    }

    #[test]
    fn a_leaf_stops_a_walker_where_it_stands_and_only_there() {
        let door = prop([2, 0, 0], [3, 3, 0], [0, 0, 1], Hang::Door);
        let leaf = door.leaves()[0];
        let body = (0.4, 1.8);
        // Shut, walking south through the doorway's middle stops short of it.
        let (from, to) = (Vec3::new(1.5, 0.0, -0.3), Vec3::new(1.5, 0.0, 0.3));
        let stopped = push_off(from, to, 0.25, body, [(leaf, 0.0)]);
        assert!(stopped.z < 0.25 - DOOR_THICK_M * 0.5 - 0.24, "{stopped}");
        // Open, the same walk goes through.
        assert_eq!(push_off(from, to, 0.25, body, [(leaf, 1.0)]), to);
        // Open, walking east along the inside of the wall, past where the leaf stands in
        // the room, stops at it.
        let (from, to) = (Vec3::new(0.4, 0.0, 0.8), Vec3::new(1.3, 0.0, 0.8));
        let stopped = push_off(from, to, 0.25, body, [(leaf, 1.0)]);
        assert!(stopped.x < leaf.hinge.x - 0.2, "{stopped}");
        // Shut, that walk is clear.
        assert_eq!(push_off(from, to, 0.25, body, [(leaf, 0.0)]), to);
    }
}
