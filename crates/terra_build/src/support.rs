//! What holds what (construction-001, rule 11; D193, D194): every cube with anything in
//! it is joined, face to face through others with anything in them, to the ground; or
//! along a ramp, where one cube's sloping top runs on from another's edge a cube lower
//! and over, as a roof's slopes climb a cube a cube (D241); a prop is fitted into a hole of
//! built cubes. Nothing is taken out while anything rests on it or is attached to it.
//!
//! This is not load or span (D142): a tower on one post is allowed, a house in the air is
//! not.

use crate::{Base, Prop, Shape};
use glam::IVec3;
use std::collections::{BTreeMap, BTreeSet};

/// The six neighbours of a cube, face to face.
pub const FACES: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];

/// The eight ways along the floor, square on and corner to corner, a ramp may run.
const LEVEL_WAYS: [[i32; 2]; 8] = [
    [1, 0],
    [-1, 0],
    [0, 1],
    [0, -1],
    [1, 1],
    [1, -1],
    [-1, 1],
    [-1, -1],
];

/// Whether a cube's sloping top runs on from the edge of the one a cube lower and `way`
/// over from it, so the two are joined though no face of theirs is shared (D241): the lower
/// full height along the edge or corner they meet at, the upper cut to a plane that starts
/// there from nothing, as each slope of a roof climbs from the one below it.
pub fn ramp(lower: Shape, upper: Shape, way: [i32; 2]) -> bool {
    if !upper.planed_top() {
        return false;
    }
    // Points along where they meet, in each one's own fractions of its footprint.
    let points: Vec<([f32; 2], [f32; 2])> = match way {
        [dx, 0] => [0.0, 0.5, 1.0]
            .map(|v| {
                let (a, b) = if dx > 0 { (1.0, 0.0) } else { (0.0, 1.0) };
                ([a, v], [b, v])
            })
            .to_vec(),
        [0, dz] => [0.0, 0.5, 1.0]
            .map(|u| {
                let (a, b) = if dz > 0 { (1.0, 0.0) } else { (0.0, 1.0) };
                ([u, a], [u, b])
            })
            .to_vec(),
        [dx, dz] => {
            let a = [f32::from(dx > 0), f32::from(dz > 0)];
            vec![(a, [1.0 - a[0], 1.0 - a[1]])]
        }
    };
    points
        .iter()
        .all(|(low, high)| lower.top(low[0], low[1]) > 0.999 && upper.top(high[0], high[1]) < 0.001)
}

/// Something resting on a cube or attached to it, which keeps it from being taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    /// A cube resting on it, or joined to the ground only through it.
    Cube(IVec3),
    /// A prop fitted against it, by its lowest cube.
    Prop(IVec3),
    /// An item standing on it or hanging from it.
    Item(IVec3),
}

impl Base {
    /// Whether a cube has anything in it, so it joins its neighbours and holds weight.
    pub fn is_in(&self, at: IVec3) -> bool {
        self.cube(at).is_some_and(|cube| cube.is_in())
    }

    /// Whether a cube is built whole and solid, so a roof or prop may rest on it.
    pub fn bears(&self, at: IVec3) -> bool {
        self.cube(at)
            .is_some_and(|cube| cube.is_built() && cube.is_in())
    }

    /// Whether the ground reaches a cube.
    pub fn grounded(&self, at: IVec3) -> bool {
        self.ground.reaches(at)
    }

    /// The places a cube is joined through, whatever is in them: its six faces, and a cube
    /// a layer up or down and over whose top runs on from its own along a ramp ([`ramp`]).
    pub fn joins(&self, at: IVec3) -> Vec<IVec3> {
        let mut out: Vec<IVec3> = FACES.iter().map(|&step| at + step).collect();
        let Some(here) = self.cube(at) else {
            return out;
        };
        for way in LEVEL_WAYS {
            let over = IVec3::new(way[0], 0, way[1]);
            for (other, lower, upper) in [
                (at + over + IVec3::Y, Some(here.shape), None),
                (at - over - IVec3::Y, None, Some(here.shape)),
            ] {
                let Some(there) = self.cube(other) else {
                    continue;
                };
                let (lower, upper) = (lower.unwrap_or(there.shape), upper.unwrap_or(there.shape));
                // Either way round, the upper is `way` over from the lower.
                if ramp(lower, upper, way) {
                    out.push(other);
                }
            }
        }
        out
    }

    /// Every cube with something in it that is not joined to the ground, sorted: none,
    /// so long as the rules held.
    pub fn floating(&self) -> Vec<IVec3> {
        self.floating_on(&self.ground)
    }

    /// Every cube with something in it that would not be joined to the ground were the
    /// ground as given, sorted.
    pub fn floating_on(&self, ground: &crate::Ground) -> Vec<IVec3> {
        let ins: BTreeSet<[i32; 3]> = self
            .cubes()
            .filter(|(_, cube)| cube.is_in())
            .map(|(at, _)| at.to_array())
            .collect();
        let mut reached = BTreeSet::new();
        let mut frontier: Vec<IVec3> = ins
            .iter()
            .map(|&at| IVec3::from_array(at))
            .filter(|&at| ground.reaches(at))
            .collect();
        reached.extend(frontier.iter().map(|at| at.to_array()));
        while let Some(at) = frontier.pop() {
            for next in self.joins(at) {
                if ins.contains(&next.to_array()) && reached.insert(next.to_array()) {
                    frontier.push(next);
                }
            }
        }
        ins.difference(&reached)
            .map(|&at| IVec3::from_array(at))
            .collect()
    }

    /// The cubes that would no longer be joined to the ground if these were emptied,
    /// sorted: a flood from each neighbour of theirs, through cubes with something in
    /// them, until it finds the ground. The flood goes lowest first, so from the top of a
    /// castle's wall it finds the ground down the wall rather than wandering the castle.
    pub fn unjoined_without(&self, gone: &BTreeSet<[i32; 3]>) -> Vec<IVec3> {
        use std::cmp::Reverse;
        let stays = |at: IVec3| !gone.contains(&at.to_array()) && self.is_in(at);
        let mut joined: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut lost: BTreeSet<[i32; 3]> = BTreeSet::new();
        for &at in gone {
            for start in self.joins(IVec3::from_array(at)) {
                let key = start.to_array();
                if !stays(start) || joined.contains(&key) || lost.contains(&key) {
                    continue;
                }
                let mut seen = BTreeSet::from([key]);
                let mut frontier = std::collections::BinaryHeap::from([(Reverse(start.y), key)]);
                let mut found = false;
                while let Some((_, here)) = frontier.pop() {
                    let here = IVec3::from_array(here);
                    if self.grounded(here) || joined.contains(&here.to_array()) {
                        found = true;
                        break;
                    }
                    for next in self.joins(here) {
                        if stays(next) && seen.insert(next.to_array()) {
                            frontier.push((Reverse(next.y), next.to_array()));
                        }
                    }
                }
                if found {
                    joined.extend(seen);
                } else {
                    lost.extend(seen);
                }
            }
        }
        lost.into_iter().map(IVec3::from_array).collect()
    }

    /// The cubes a prop's hole is made of, if it fits one: built cubes across its top,
    /// down both its sides one way or the other, and under its foot unless the ground is
    /// there.
    pub fn hole_for(&self, prop: &Prop) -> Option<Vec<IVec3>> {
        let bears = |at: IVec3| self.bears(at);
        let (low, high) = (IVec3::from_array(prop.min), IVec3::from_array(prop.max));
        let mut holders = Vec::new();
        for x in low.x..=high.x {
            for z in low.z..=high.z {
                holders.push(IVec3::new(x, high.y + 1, z));
                let under = IVec3::new(x, low.y - 1, z);
                if !self.grounded(IVec3::new(x, low.y, z)) {
                    holders.push(under);
                }
            }
        }
        let sides = |axis: usize| -> Vec<IVec3> {
            let mut found = Vec::new();
            for y in low.y..=high.y {
                let across = if axis == 0 {
                    low.z..=high.z
                } else {
                    low.x..=high.x
                };
                for a in across {
                    let (first, last) = if axis == 0 {
                        (IVec3::new(low.x - 1, y, a), IVec3::new(high.x + 1, y, a))
                    } else {
                        (IVec3::new(a, y, low.z - 1), IVec3::new(a, y, high.z + 1))
                    };
                    found.extend([first, last]);
                }
            }
            found
        };
        let jambs = [0, 2]
            .into_iter()
            .map(sides)
            .find(|jambs| jambs.iter().all(|&at| bears(at)))?;
        holders.extend(jambs);
        holders.iter().all(|&at| bears(at)).then_some(holders)
    }

    /// What rests on a cube or is attached to it, other than items, sorted: the cube over
    /// it if anything is in that, props fitted against it.
    pub fn resting_on(&self, at: IVec3) -> Vec<Holder> {
        let mut found = Vec::new();
        if self.is_in(at + IVec3::Y) {
            found.push(Holder::Cube(at + IVec3::Y));
        }
        for prop in self.props() {
            if self
                .hole_for(prop)
                .is_some_and(|holders| holders.contains(&at))
            {
                found.push(Holder::Prop(IVec3::from_array(prop.min)));
            }
        }
        found
    }

    /// For every cube something rests on or is attached to, what does, beyond the cube
    /// over it; for checking the rules held.
    pub fn attachments(&self) -> BTreeMap<[i32; 3], Vec<Holder>> {
        let mut found: BTreeMap<[i32; 3], Vec<Holder>> = BTreeMap::new();
        for prop in self.props() {
            for at in self.hole_for(prop).unwrap_or_default() {
                let holder = Holder::Prop(IVec3::from_array(prop.min));
                found.entry(at.to_array()).or_default().push(holder);
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildOp, Cube, Fill, Kind};

    fn post(base: &mut Base, x: i32, from: i32, to: i32) {
        base.apply(BuildOp::Fill {
            min: [x, from, 0],
            max: [x, to, 0],
            cube: Some(Cube::new(Kind(1), Fill::Solid)),
        });
    }

    #[test]
    fn a_post_on_the_ground_is_joined_and_one_in_the_air_floats() {
        let mut base = Base::default();
        post(&mut base, 0, 0, 5);
        assert!(base.floating().is_empty());
        post(&mut base, 3, 2, 5);
        assert_eq!(base.floating().len(), 4);
        // A beam across joins it through the first.
        base.apply(BuildOp::Fill {
            min: [1, 5, 0],
            max: [2, 5, 0],
            cube: Some(Cube::new(Kind(1), Fill::Solid)),
        });
        assert!(base.floating().is_empty());
        // Without the first post's foot, all of it would hang from nothing.
        let gone = BTreeSet::from([[0, 0, 0]]);
        assert_eq!(base.unjoined_without(&gone).len(), 5 + 2 + 4);
        // Without the top of the beam's far end, the second post still stands on nothing
        // and the first still on the ground.
        let gone = BTreeSet::from([[2, 5, 0]]);
        assert_eq!(base.unjoined_without(&gone).len(), 4);
    }

    #[test]
    fn slopes_climbing_a_cube_a_cube_are_joined_along_their_ramp() {
        use crate::{Facing, Shape};
        let put = |base: &mut Base, at: [i32; 3], shape: Shape| {
            base.apply(BuildOp::Fill {
                min: at,
                max: at,
                cube: Some(Cube {
                    shape,
                    ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
                }),
            });
        };
        // A wall two high, and a roof's slopes climbing south from its top, a cube a cube,
        // touching only along their edges.
        let mut base = Base::default();
        post(&mut base, 0, 0, 1);
        for step in 0..3 {
            put(&mut base, [0, 2 + step, step], Shape::Slope(Facing::South));
        }
        assert!(base.floating().is_empty(), "{:?}", base.floating());
        // Without the lowest, the rest hang from nothing.
        let gone = BTreeSet::from([[0, 2, 0]]);
        assert_eq!(base.unjoined_without(&gone).len(), 2);
        // Slopes rising the other way meet at their low edges, which is no joint: whole
        // cubes climbing are not joined either.
        let mut wrong = Base::default();
        post(&mut wrong, 0, 0, 1);
        put(&mut wrong, [0, 2, 0], Shape::Slope(Facing::South));
        put(&mut wrong, [0, 3, 1], Shape::Slope(Facing::North));
        assert_eq!(wrong.floating(), vec![IVec3::new(0, 3, 1)]);
        let mut whole = Base::default();
        post(&mut whole, 0, 0, 1);
        put(&mut whole, [0, 2, 1], Shape::Whole);
        assert_eq!(whole.floating().len(), 1);
        // A hip's outer corner climbs corner to corner from the one below it.
        assert!(ramp(
            Shape::OuterCorner(Facing::East),
            Shape::OuterCorner(Facing::East),
            [1, 1]
        ));
        assert!(!ramp(Shape::Whole, Shape::Whole, [1, 0]));
    }
}
