//! How a door or shutters hang in the hole they are fitted into (D192, D207, D225): the
//! frame each comes with, its leaves, each hinged on a side of the frame, and where each
//! stands shut, open or partway. Drawing and walking both read a prop's leaves from here,
//! so what is seen is what stops a walker, whichever way it stands.
//!
//! Each comes with its own frame, as shipped games' doors and windows do: jambs up the hole's
//! sides, a head over it and a threshold or a sill under it, lining the reveal through the
//! wall and reaching a little into the wall round it, so it fills the hole whatever the kit
//! lays round it. The frame is seated on the wall's own face, which differs by kit (a log
//! wall's round crests stand well in from its cubes' faces, a stone wall's are on them), as
//! the kit's numbers give it ([`Kit::face`]), so what hangs on it is centred on that wall
//! whatever it is built of. A door hangs inside its frame and swings a quarter turn into the
//! room; a doorway 1.5 m wide or more takes a pair. Shutters hang on the frame's outer face,
//! over the hole to the middle of its members all round, and fold back flat against it on
//! strap hinges; a hole 0.75 m wide or more takes a pair. This is how real ones hang, and
//! how Valheim's and Enshrouded's doors do.

use glam::{IVec3, Vec2, Vec3};
use terra_build::kit::{Kit, Kits};
use terra_build::{Base, CELL_M, Fill, Kind, Prop};

pub use terra_build::pieces::Hang;

/// How far a frame reaches past the hole's edges into the wall round it: so what is fitted
/// fills its hole whatever stands round it, a log wall's courses or a stone wall's dressed
/// jambs, with no gap to see through at any edge (the developers: they "can overlap w/ the
/// voxels a bit").
pub const OVERLAP_M: f32 = 0.03;
/// A frame (D225 as amended): jambs up the hole's sides and a head over it, squared timber
/// this wide across its face, lining the hole's reveal through the wall and standing this far
/// proud of the wall's face on each side; under a door a threshold this high, under shutters
/// a sill as deep as the head. A door's leaf hangs in it with this gap all round.
pub const FRAME_FACE_M: f32 = 0.09;
pub const FRAME_PROUD_M: f32 = 0.01;
pub const THRESHOLD_M: f32 = 0.03;
pub const LEAF_GAP_M: f32 = 0.004;
/// How thick a door's leaf is, and a shutter's: as the shutters' models are made, which the
/// model contract holds them to (`cargo xtask contract`).
pub const DOOR_THICK_M: f32 = 0.06;
pub const SHUTTER_THICK_M: f32 = 0.05;
/// How far a door swings open, and shutters, in degrees: shutters right back, flat against
/// the frame and the wall beside it.
const DOOR_SWING_DEGREES: f32 = 92.0;
const SHUTTER_SWING_DEGREES: f32 = 180.0;

/// One leaf of a door or of shutters, in the base's own space, in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leaf {
    /// Its hinge's foot as it stands shut: on the edge it turns on, on its face toward the
    /// way it opens.
    pub hinge: Vec3,
    /// Level, from the hinge across the leaf, as it stands shut.
    pub across: Vec3,
    /// Level, the way it opens: into the room for a door, out for shutters.
    pub opens: Vec3,
    pub width: f32,
    pub height: f32,
    pub thick: f32,
    /// How far behind its hinge face the pin it turns on is: none for a door's, hung on
    /// butts at its face; half its thickness for a shutter's, on strap hinges whose pintles
    /// stand off the frame so it folds back flat on the face it closed against.
    pub pivot: f32,
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
    /// The pin it turns on, at its hinge's foot.
    pub fn pin(&self) -> Vec3 {
        self.hinge - self.opens * self.pivot
    }

    /// Where it stands turned `open` of the way, 0 to 1.
    pub fn pose(&self, open: f32) -> Pose {
        let angle = self.swing * open.clamp(0.0, 1.0);
        let (sin, cos) = (libm::sinf(angle), libm::cosf(angle));
        // Turned about its pin in the plane of `across` and `opens`: across toward opens,
        // and the way through it, which starts as against `opens`, toward across.
        let through = -self.opens * cos + self.across * sin;
        Pose {
            hinge: self.pin() - through * self.pivot,
            across: self.across * cos + self.opens * sin,
            through,
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

/// A prop's frame and leaves as they hang in its hole. A trait, as the prop is the rules' type
/// and its hanging the layout's (D407).
pub trait Hanging {
    /// Its frame on a wall whose face stands `face` out from its middle ([`wall_face`]), as
    /// boxes in the base's own space, each its low and high corners.
    fn frame(&self, face: f32) -> Vec<(Vec3, Vec3)>;
    /// Its leaves, as they hang in its frame on a wall whose face stands `face` out from its
    /// middle.
    fn leaves(&self, face: f32) -> Vec<Leaf>;
}

impl Hanging for Prop {
    /// Its frame on a wall whose face stands `face` out from its middle ([`wall_face`]), as
    /// boxes in the base's own space, each its low and high corners: a jamb up each side and
    /// a head over the hole, their outer edges `OVERLAP_M` into the wall past the hole's,
    /// through the wall's thickness and `FRAME_PROUD_M` proud of its face either side; and
    /// between the jambs a door's threshold on the floor, or under shutters a sill reaching
    /// down into the wall as the head reaches up.
    fn frame(&self, face: f32) -> Vec<(Vec3, Vec3)> {
        let (opens, along, low, high, depth) = self.hole();
        let through = opens.abs();
        let (start, end) = (low.dot(along) - OVERLAP_M, high.dot(along) + OVERLAP_M);
        let top = high.y + OVERLAP_M;
        let (bottom, sill) = match self.hang {
            Hang::Door => (low.y, THRESHOLD_M),
            Hang::Shutters => (low.y - OVERLAP_M, FRAME_FACE_M),
        };
        let half = face + FRAME_PROUD_M;
        let boxed = |a: f32, b: f32, y0: f32, y1: f32| {
            let one = along * a + through * (depth - half) + Vec3::Y * y0;
            let other = along * b + through * (depth + half) + Vec3::Y * y1;
            (one.min(other), one.max(other))
        };
        let (inner_start, inner_end) = (start + FRAME_FACE_M, end - FRAME_FACE_M);
        vec![
            boxed(start, inner_start, bottom, top),
            boxed(inner_end, end, bottom, top),
            boxed(inner_start, inner_end, top - FRAME_FACE_M, top),
            boxed(inner_start, inner_end, bottom, bottom + sill),
        ]
    }

    /// Its leaves, as they hang in its frame on a wall whose face stands `face` out from its
    /// middle: a door's inside the frame, `LEAF_GAP_M` clear of the jambs, the head and the
    /// threshold all round, hinged on a jamb's inner face, in the middle of the wall's
    /// thickness; shutters shut against the frame's outer face, over the hole to the middle
    /// of its members all round, hinged on the jambs. A pair meets in the middle.
    fn leaves(&self, face: f32) -> Vec<Leaf> {
        let (opens, along, low, high, depth) = self.hole();
        let door = self.hang == Hang::Door;
        // How far each leaf's edges stand in from the hole's, the frame's outer edges being
        // `OVERLAP_M` out from them.
        let inset = if door {
            FRAME_FACE_M + LEAF_GAP_M
        } else {
            FRAME_FACE_M * 0.5
        } - OVERLAP_M;
        let (start, end) = (low.dot(along) + inset, high.dot(along) - inset);
        let top = high.y - inset;
        let (foot, thick, out, pivot, swing) = if door {
            (
                low.y + THRESHOLD_M + LEAF_GAP_M,
                DOOR_THICK_M,
                // Its hinge face toward the room, its middle in the wall's middle.
                DOOR_THICK_M * 0.5,
                0.0,
                DOOR_SWING_DEGREES,
            )
        } else {
            (
                low.y + inset,
                SHUTTER_THICK_M,
                // Its back on the frame's outer face.
                face + FRAME_PROUD_M + SHUTTER_THICK_M,
                SHUTTER_THICK_M * 0.5,
                SHUTTER_SWING_DEGREES,
            )
        };
        let at = |b: f32| {
            let level = along * b + opens.abs() * depth;
            Vec3::new(level.x, foot, level.z) + opens * out
        };
        let leaf = |hinge: f32, across: Vec3, width: f32| Leaf {
            hinge: at(hinge),
            across,
            opens,
            width,
            height: top - foot,
            thick,
            pivot,
            swing: swing.to_radians(),
            door,
        };
        if self.leaf_count() == 2 {
            let half = (end - start - if door { LEAF_GAP_M } else { 0.0 }) * 0.5;
            vec![leaf(start, along, half), leaf(end, -along, half)]
        } else {
            vec![leaf(start, along, end - start)]
        }
    }
}

/// The kind of the wall a prop is fitted in: the kind most of the wall's cubes round its hole
/// are, beside its sides, over it and under it; none where nothing stands round it.
pub fn wall_kind(base: &Base, prop: &Prop) -> Option<Kind> {
    let opens = prop.across_wall();
    let along = IVec3::new(opens.z.abs(), 0, opens.x.abs());
    let (low, high) = (IVec3::from_array(prop.min), IVec3::from_array(prop.max));
    let mut round: Vec<IVec3> = Vec::new();
    for y in low.y..=high.y {
        round.push(IVec3::new(low.x, y, low.z) - along);
        round.push(IVec3::new(high.x, y, high.z) + along);
    }
    for at in prop.cubes() {
        if at.y == low.y {
            round.push(at - IVec3::Y);
        }
        if at.y == high.y {
            round.push(at + IVec3::Y);
        }
    }
    let mut counts: std::collections::BTreeMap<Kind, usize> = Default::default();
    for at in round {
        if let Some(cube) = base.cube(at)
            && cube.fill == Fill::Solid
            && cube.kind != Kind::PLAIN
            && !cube.kind.is_roof()
        {
            *counts.entry(cube.kind).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(kind, count)| (count, std::cmp::Reverse(kind)))
        .map(|(kind, _)| kind)
}

/// How far the face of the wall a prop is fitted in stands out from its middle, in metres,
/// by its kit ([`Kit::face`]); a whole cube's half where there are no kits, its kind has none
/// or nothing stands round it.
pub fn wall_face(base: &Base, prop: &Prop, kits: Option<&Kits>) -> f32 {
    wall_kind(base, prop)
        .and_then(|kind| kits?.of(kind))
        .map_or(CELL_M * 0.5, Kit::face)
}

/// A prop's leaves as they hang in the wall it is fitted in, on that wall's face
/// ([`wall_face`]): both what is drawn and what a walker meets (`walk`), so the two are one.
pub fn hung(base: &Base, prop: &Prop, kits: Option<&Kits>) -> Vec<Leaf> {
    prop.leaves(wall_face(base, prop, kits))
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
    use crate::walk::Walking;
    use terra_build::PropKind;

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

    /// A wall's face a whole cube thick, stone's.
    const CUBE_FACE: f32 = CELL_M * 0.5;

    /// The shipped materials' kits, by name, as the game reads them.
    fn shipped(name: &str) -> Kit {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        Kit::read_folder(&assets)
            .unwrap()
            .into_iter()
            .find(|(named, _)| named == name)
            .map(|(_, kit)| kit)
            .unwrap_or_else(|| panic!("no {name} kit"))
    }

    /// The low and high ends of what a prop's leaves cover along its wall, shut, and their
    /// foot and top.
    fn covered(leaves: &[Leaf], along: Vec3) -> (f32, f32, f32, f32) {
        let mut spans: Vec<(f32, f32)> = leaves
            .iter()
            .map(|leaf| {
                let a = leaf.hinge.dot(along);
                let b = (leaf.hinge + leaf.across * leaf.width).dot(along);
                (a.min(b), a.max(b))
            })
            .collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Each meeting the next, with no more than a door's gap between.
        for pair in spans.windows(2) {
            assert!(pair[1].0 <= pair[0].1 + LEAF_GAP_M + 1e-4, "{spans:?}");
        }
        let foot = leaves.iter().map(|l| l.hinge.y).fold(f32::MAX, f32::min);
        let top = leaves
            .iter()
            .map(|l| l.hinge.y + l.height)
            .fold(f32::MIN, f32::max);
        (spans[0].0, spans[spans.len() - 1].1, foot, top)
    }

    /// Every hole a door fits, and every one shutters fit, in walls running east and south,
    /// with the way along each wall.
    fn holes() -> Vec<(Prop, Vec3)> {
        let mut out = Vec::new();
        for wide in 1..=4 {
            for (hang, foot, highs) in [(Hang::Door, 0, 4..=5), (Hang::Shutters, 2, 2..=4)] {
                for high in highs {
                    let top = foot + high - 1;
                    out.push((
                        prop([2, foot, 3], [1 + wide, top, 3], [0, 0, 1], hang),
                        Vec3::X,
                    ));
                    out.push((
                        prop([3, foot, 2], [3, top, 1 + wide], [-1, 0, 0], hang),
                        Vec3::Z,
                    ));
                }
            }
        }
        out
    }

    /// A hole's low and high corners, in metres.
    fn corners(prop: &Prop) -> (Vec3, Vec3) {
        (
            IVec3::from_array(prop.min).as_vec3() * CELL_M,
            (IVec3::from_array(prop.max) + IVec3::ONE).as_vec3() * CELL_M,
        )
    }

    #[test]
    fn a_door_fills_its_hole_inside_the_frame_and_opens_a_quarter_turn_into_the_room() {
        // A doorway a metre wide and two high in a wall running east, the room south.
        let door = prop([2, 0, 0], [3, 3, 0], [0, 0, 1], Hang::Door);
        let leaves = door.leaves(CUBE_FACE);
        assert_eq!(leaves.len(), 1);
        let leaf = leaves[0];
        // A metre less what the frame's jambs stand into the hole, and the gap either side.
        let inside = FRAME_FACE_M - OVERLAP_M + LEAF_GAP_M;
        assert!((leaf.width - (1.0 - 2.0 * inside)).abs() < 1e-4);
        assert!(leaf.height > 1.8 && leaf.height < 2.0);
        // Shut, it stands across the doorway in the wall's middle; open, from its hinge
        // into the room, south, as far as it is wide.
        let shut = leaf.middle(0.0);
        assert!((shut.z - 0.25).abs() < 0.05, "{shut}");
        assert!((shut.x - 1.5).abs() < 1e-3);
        let open = leaf.middle(1.0);
        assert!(open.z > 0.25 + leaf.width * 0.4, "{open}");
        assert!((open.x - leaf.hinge.x).abs() < leaf.thick + 0.01);
        // A door hangs in the wall's middle whatever the wall.
        assert_eq!(door.leaves(0.15), leaves);
    }

    #[test]
    fn a_frame_fills_its_hole_and_its_leaves_are_centred_in_it() {
        for (fitted, along) in holes() {
            let door = fitted.hang == Hang::Door;
            let (low, high) = corners(&fitted);
            let (start, end) = (low.dot(along), high.dot(along));
            let frame = fitted.frame(CUBE_FACE);
            assert_eq!(frame.len(), 4);
            // Its outer edges past the hole's by the overlap, at both sides, the head and,
            // under shutters, the sill.
            let outer = |pick: &dyn Fn(&(Vec3, Vec3)) -> f32, most: bool| {
                frame
                    .iter()
                    .map(pick)
                    .fold(if most { f32::MIN } else { f32::MAX }, |a, b| {
                        if most { a.max(b) } else { a.min(b) }
                    })
            };
            let outer_low = outer(&|(min, _)| min.dot(along), false);
            let outer_high = outer(&|(_, max)| max.dot(along), true);
            let outer_top = outer(&|(_, max)| max.y, true);
            let outer_foot = outer(&|(min, _)| min.y, false);
            assert!((start - outer_low - OVERLAP_M).abs() < 1e-4);
            assert!((outer_high - end - OVERLAP_M).abs() < 1e-4);
            assert!((outer_top - high.y - OVERLAP_M).abs() < 1e-4);
            let below = if door { 0.0 } else { OVERLAP_M };
            assert!((low.y - outer_foot - below).abs() < 1e-4);
            // The rebate: the inner faces of the jambs and head, and the sill's top.
            let (jamb_a, jamb_b) = (outer_low + FRAME_FACE_M, outer_high - FRAME_FACE_M);
            let head = outer_top - FRAME_FACE_M;
            let sill = outer_foot + if door { THRESHOLD_M } else { FRAME_FACE_M };
            let leaves = fitted.leaves(CUBE_FACE);
            assert_eq!(leaves.len(), fitted.leaf_count());
            let (left, right, foot, top) = covered(&leaves, along);
            // A door the same gap inside its frame at either side, at the head and over the
            // threshold; shutters over its opening to the middle of its members all round.
            let margins = [left - jamb_a, jamb_b - right, head - top, foot - sill];
            let want = if door {
                LEAF_GAP_M
            } else {
                -FRAME_FACE_M * 0.5
            };
            for margin in margins {
                assert!(
                    (margin - want).abs() < 1e-4,
                    "{fitted:?}: margins {margins:?}"
                );
            }
        }
    }

    #[test]
    fn shutters_are_centred_in_their_frame_and_seated_on_the_walls_face() {
        // On logs, on hewn beams and on stone: each wall's face where its kit's numbers put
        // it, and the shutters on it.
        let faces: Vec<(String, f32)> = ["logs", "hewn", "stone"]
            .into_iter()
            .map(|name| (name.to_string(), shipped(name).face()))
            .collect();
        // A round log's crest stands well in from its cube's face, a squared beam's further
        // in still; stone's is its cubes'.
        assert!(faces[0].1 > 0.13 && faces[0].1 < 0.2, "{faces:?}");
        assert!(faces[1].1 > 0.1 && faces[1].1 < faces[0].1, "{faces:?}");
        assert!((faces[2].1 - CUBE_FACE).abs() < 1e-6, "{faces:?}");
        for (name, face) in &faces {
            for (shutters, along) in holes() {
                if shutters.hang != Hang::Shutters {
                    continue;
                }
                let opens = shutters.across_wall().as_vec3();
                let middle = ((corners(&shutters).0 + corners(&shutters).1) * 0.5).dot(opens);
                let out = |p: Vec3| p.dot(opens) - middle;
                let frame = shutters.frame(*face);
                // The frame through the wall, proud of its face either side by as much.
                for (min, max) in &frame {
                    let (a, b) = (out(*min), out(*max));
                    let (back, front) = (a.min(b), a.max(b));
                    assert!(
                        (front - face - FRAME_PROUD_M).abs() < 1e-4
                            && (back + face + FRAME_PROUD_M).abs() < 1e-4,
                        "{name} {shutters:?}: frame {back}..{front}"
                    );
                }
                // Centred along the wall and up it: the frame on the hole, the leaves on the
                // frame.
                let (low, high) = corners(&shutters);
                let hole_middle = ((low + high) * 0.5).dot(along);
                let frame_low = frame
                    .iter()
                    .map(|(min, _)| min.dot(along))
                    .fold(f32::MAX, f32::min);
                let frame_high = frame
                    .iter()
                    .map(|(_, max)| max.dot(along))
                    .fold(f32::MIN, f32::max);
                let frame_foot = frame.iter().map(|(min, _)| min.y).fold(f32::MAX, f32::min);
                let frame_top = frame.iter().map(|(_, max)| max.y).fold(f32::MIN, f32::max);
                assert!(((frame_low + frame_high) * 0.5 - hole_middle).abs() < 1e-4);
                let leaves = shutters.leaves(*face);
                let (left, right, foot, top) = covered(&leaves, along);
                assert!(
                    ((left + right) * 0.5 - hole_middle).abs() < 1e-4,
                    "{name} {shutters:?}: across {left}..{right} about {hole_middle}"
                );
                assert!(
                    ((foot + top) - (frame_foot + frame_top)).abs() < 2e-4,
                    "{name} {shutters:?}: up {foot}..{top} in {frame_foot}..{frame_top}"
                );
                // Shut, each leaf's back on the frame's outer face, its front a leaf's
                // thickness out.
                for leaf in &leaves {
                    let pose = leaf.pose(0.0);
                    let back = out(pose.hinge + pose.through * leaf.thick);
                    assert!(
                        (back - face - FRAME_PROUD_M).abs() < 1e-4,
                        "{name} {shutters:?}: its back {back} off a face at {face}"
                    );
                    assert!((out(pose.hinge) - back - leaf.thick).abs() < 1e-4);
                    // Open, folded right back, flat on the same face beside the hole.
                    let open = leaf.pose(1.0);
                    for p in [open.hinge, open.hinge + open.across * leaf.width] {
                        let near = out(p).min(out(p + open.through * leaf.thick));
                        assert!(
                            (near - face - FRAME_PROUD_M).abs() < 1e-4,
                            "{name} {shutters:?}: open, its back at {near}"
                        );
                    }
                    let beside = leaf.middle(1.0).dot(along);
                    assert!(
                        beside < left - 1e-3 || beside > right + 1e-3,
                        "{name} {shutters:?}: open over the hole at {beside}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_leaf_swings_about_its_hinge_without_crossing_its_frame_or_its_wall() {
        for face in [0.135, 0.155, CUBE_FACE] {
            for (fitted, along) in holes() {
                let frame = fitted.frame(face);
                let opens = fitted.across_wall().as_vec3();
                let (low, high) = corners(&fitted);
                let middle = ((low + high) * 0.5).dot(opens);
                for leaf in fitted.leaves(face) {
                    for step in 0..=20 {
                        let open = step as f32 / 20.0;
                        let pose = leaf.pose(open);
                        // Points through the leaf's body, its edges and middle.
                        for i in 0..=8 {
                            for j in 0..=8 {
                                for k in 0..=2 {
                                    let p = pose.hinge
                                        + pose.across
                                            * (leaf.width * (0.01 + 0.98 * i as f32 / 8.0))
                                        + Vec3::Y * (leaf.height * j as f32 / 8.0)
                                        + pose.through
                                            * (leaf.thick * (0.05 + 0.9 * k as f32 / 2.0));
                                    for (min, max) in &frame {
                                        let inside = p.cmpgt(*min + Vec3::splat(1e-3)).all()
                                            && p.cmplt(*max - Vec3::splat(1e-3)).all();
                                        assert!(
                                            !inside,
                                            "{fitted:?} on {face} open {open}: the leaf crosses its frame at {p}"
                                        );
                                    }
                                    // Within the wall's face, only in the hole.
                                    let a = p.dot(along);
                                    let in_hole =
                                        a > low.dot(along) - 1e-3 && a < high.dot(along) + 1e-3;
                                    let in_wall = (p.dot(opens) - middle).abs() < face - 1e-3;
                                    assert!(
                                        in_hole || !in_wall,
                                        "{fitted:?} on {face} open {open}: the leaf in the wall beside its hole at {p}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn wide_holes_take_a_pair_meeting_in_the_middle() {
        let double = prop([0, 0, 4], [0, 3, 7], [1, 0, 0], Hang::Door);
        let leaves = double.leaves(CUBE_FACE);
        assert_eq!(leaves.len(), 2);
        // Along Z, hinged at either end, meeting in the middle with the leaves' gap between.
        let ends: Vec<f32> = leaves
            .iter()
            .map(|leaf| leaf.hinge.z + leaf.across.z * leaf.width)
            .collect();
        let half = LEAF_GAP_M * 0.5;
        assert!(
            (ends[0] - (3.0 - half)).abs() < 1e-4 && (ends[1] - (3.0 + half)).abs() < 1e-4,
            "{ends:?}"
        );
        // Shutters over a window a metre wide take a pair; half a metre, one.
        let pair = prop([2, 2, 0], [3, 3, 0], [0, 0, -1], Hang::Shutters);
        assert_eq!(pair.leaves(CUBE_FACE).len(), 2);
        let one = prop([2, 2, 0], [2, 3, 0], [0, 0, -1], Hang::Shutters);
        assert_eq!(one.leaves(CUBE_FACE).len(), 1);
    }

    #[test]
    fn a_leaf_stops_a_walker_where_it_stands_and_only_there() {
        let door = prop([2, 0, 0], [3, 3, 0], [0, 0, 1], Hang::Door);
        let leaf = door.leaves(CUBE_FACE)[0];
        let body = (0.4, 1.8);
        // Shut, walking south through the doorway's middle stops short of it.
        let (from, to) = (Vec3::new(1.5, 0.0, -0.3), Vec3::new(1.5, 0.0, 0.3));
        let stopped = push_off(from, to, 0.25, body, [(leaf, 0.0)]);
        assert!(stopped.z < 0.25 - DOOR_THICK_M * 0.5 - 0.24, "{stopped}");
        // Open, the same walk goes through.
        assert_eq!(push_off(from, to, 0.25, body, [(leaf, 1.0)]), to);
        // Open, walking east along the inside of the wall, past where the leaf stands in
        // the room, stops at it.
        let (from, to) = (Vec3::new(0.4, 0.0, 0.8), Vec3::new(1.15, 0.0, 0.8));
        let stopped = push_off(from, to, 0.25, body, [(leaf, 1.0)]);
        assert!(stopped.x < leaf.hinge.x - 0.2, "{stopped}");
        // Shut, that walk is clear.
        assert_eq!(push_off(from, to, 0.25, body, [(leaf, 0.0)]), to);
    }

    #[test]
    fn a_props_wall_is_the_kind_round_its_hole() {
        // A window in a wall of logs, with a course of stone under it.
        let (logs, stone) = (Kind(1), Kind(2));
        let mut base = Base::default();
        for x in 0..6 {
            for y in 0..6 {
                let kind = if y == 0 { stone } else { logs };
                let fill = if (2..=3).contains(&x) && (2..=3).contains(&y) {
                    Fill::Open
                } else {
                    Fill::Solid
                };
                base.apply(terra_build::BuildOp::Fill {
                    min: [x, y, 0],
                    max: [x, y, 0],
                    cube: Some(terra_build::Cube::new(kind, fill)),
                });
            }
        }
        let window = prop([2, 2, 0], [3, 3, 0], [0, 0, -1], Hang::Shutters);
        assert_eq!(wall_kind(&base, &window), Some(logs));
        let kits = Kits(vec![shipped("logs"), shipped("stone")]);
        assert_eq!(
            wall_face(&base, &window, Some(&kits)),
            shipped("logs").face()
        );
        // Standing alone, or with no kits, it is on a cube's face.
        assert_eq!(wall_face(&Base::default(), &window, Some(&kits)), CUBE_FACE);
        assert_eq!(wall_face(&base, &window, None), CUBE_FACE);
    }

    #[test]
    fn a_walker_meets_shutters_where_they_are_drawn_on_every_wall() {
        let walker = terra_build::Walker {
            radius: 0.3,
            height: 0.8,
            step_up: 0.1,
        };
        for name in ["logs", "hewn", "stone"] {
            let kit = shipped(name);
            let kits = Kits(vec![kit.clone()]);
            // A wall of it along x, its middle at z = 0.25, a window a metre square in it
            // from a metre up, shutters opening out to -z.
            let mut base = Base::default();
            for x in 0..6 {
                for y in 0..6 {
                    let hole = (2..=3).contains(&x) && (2..=3).contains(&y);
                    let fill = if hole { Fill::Open } else { Fill::Solid };
                    base.apply(terra_build::BuildOp::Fill {
                        min: [x, y, 0],
                        max: [x, y, 0],
                        cube: Some(terra_build::Cube::new(kit.kind(), fill)),
                    });
                }
            }
            let mut window = prop([2, 2, 0], [3, 3, 0], [0, 0, -1], Hang::Shutters);
            terra_build::testing::put_prop(&mut base, window);
            // Where they are drawn: the frame on the wall's face, its outer face nearest
            // the walker, the leaves' backs on it.
            let frame_out = window
                .frame(wall_face(&base, &window, Some(&kits)))
                .iter()
                .map(|(min, _)| min.z)
                .fold(f32::MAX, f32::min);
            let drawn = frame_out - SHUTTER_THICK_M;
            // In small steps, as the game walks a frame at a time.
            let walk = |base: &Base, x: f32| {
                let mut at = Vec3::new(x, 1.0, -1.5);
                for i in 1..=200 {
                    let to = Vec3::new(x, 1.0, -1.5 + 1.75 * i as f32 / 200.0);
                    at = base.walk(at, to, &walker, Some(&kits));
                }
                at.z
            };
            // Shut, walking at the window stops a walker touching the leaves' fronts.
            let stopped = walk(&base, 1.5);
            assert!(
                (stopped - (drawn - walker.radius)).abs() < 5e-3,
                "{name}: shut, stopped at {stopped}, drawn at {drawn}"
            );
            // Open, they pass into the window, and beside it meet the leaf folded back
            // on the wall, or the wall's cubes where those stand further out.
            window.open = true;
            terra_build::testing::put_prop(&mut base, window);
            assert!(walk(&base, 1.5) > 0.2, "{name}: open");
            let stopped = walk(&base, 0.75);
            let nearest = drawn.min(0.0);
            assert!(
                (stopped - (nearest - walker.radius)).abs() < 5e-3,
                "{name}: open, stopped at {stopped}, drawn at {drawn}"
            );
        }
    }
}
