//! What holds what (construction-001, rule 11; D193, D194): every cube with anything in
//! it is joined, face to face through others with anything in them, to the ground; a
//! roof stands on built cubes under each of its eaves; a prop is fitted into a hole of
//! built cubes. Nothing is taken out while anything rests on it or is attached to it.
//!
//! This is not load or span (D142): a tower on one post is allowed, a house in the air is
//! not.

use crate::{Base, Prop, Roof, RoofId, RoofStyle, STOREY_CUBES, wall_layer};
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

/// Something resting on a cube or attached to it, which keeps it from being taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    /// A cube resting on it, or joined to the ground only through it.
    Cube(IVec3),
    /// A roof standing on it.
    Roof(RoofId),
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
            for step in FACES {
                let next = (at + step).to_array();
                if ins.contains(&next) && reached.insert(next) {
                    frontier.push(at + step);
                }
            }
        }
        ins.difference(&reached)
            .map(|&at| IVec3::from_array(at))
            .collect()
    }

    /// The cubes that would no longer be joined to the ground if these were emptied,
    /// sorted: a flood from each neighbour of theirs, through cubes with something in
    /// them, until it finds the ground.
    pub fn unjoined_without(&self, gone: &BTreeSet<[i32; 3]>) -> Vec<IVec3> {
        let stays = |at: IVec3| !gone.contains(&at.to_array()) && self.is_in(at);
        let mut joined: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut lost: BTreeSet<[i32; 3]> = BTreeSet::new();
        for &at in gone {
            for step in FACES {
                let start = IVec3::from_array(at) + step;
                let key = start.to_array();
                if !stays(start) || joined.contains(&key) || lost.contains(&key) {
                    continue;
                }
                let mut seen = BTreeSet::from([key]);
                let mut frontier = vec![start];
                let mut found = false;
                while let Some(here) = frontier.pop() {
                    if self.grounded(here) || joined.contains(&here.to_array()) {
                        found = true;
                        break;
                    }
                    for step in FACES {
                        let next = here + step;
                        if stays(next) && seen.insert(next.to_array()) {
                            frontier.push(next);
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

    /// The rows of cubes a roof's eaves rest on, at the top of its storey's walls: all
    /// four sides of a hip, the two long sides of a gable or a lean-to.
    pub fn eaves(&self, roof: &Roof) -> Vec<Vec<IVec3>> {
        let section = &roof.section;
        let top = wall_layer(section.storey()) + STOREY_CUBES - 1;
        let (low, high) = (section.min, section.max);
        let along_x = |z: i32| {
            (low[0]..=high[0])
                .map(move |x| IVec3::new(x, top, z))
                .collect()
        };
        let along_z = |x: i32| {
            (low[2]..=high[2])
                .map(move |z| IVec3::new(x, top, z))
                .collect()
        };
        match (section.style, section.along_x) {
            (RoofStyle::Hip, _) => vec![
                along_x(low[2]),
                along_x(high[2]),
                along_z(low[0]),
                along_z(high[0]),
            ],
            (_, true) => vec![along_x(low[2]), along_x(high[2])],
            (_, false) => vec![along_z(low[0]), along_z(high[0])],
        }
    }

    /// Whether a roof is held: every cube along its eaves built, so the walls under it
    /// are up before any of it goes on.
    pub fn roof_held(&self, roof: &Roof) -> bool {
        self.eaves(roof).iter().flatten().all(|&at| self.bears(at))
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
    /// it if anything is in that, roofs standing on it, props fitted against it.
    pub fn resting_on(&self, at: IVec3) -> Vec<Holder> {
        let mut found = Vec::new();
        if self.is_in(at + IVec3::Y) {
            found.push(Holder::Cube(at + IVec3::Y));
        }
        for (id, roof) in self.roofs() {
            if roof.standing() && self.eaves(roof).iter().flatten().any(|&eave| eave == at) {
                found.push(Holder::Roof(id));
            }
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
        for (id, roof) in self.roofs().filter(|(_, roof)| roof.standing()) {
            for at in self.eaves(roof).into_iter().flatten() {
                found
                    .entry(at.to_array())
                    .or_default()
                    .push(Holder::Roof(id));
            }
        }
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
}
