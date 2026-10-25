//! The faces weather reaches: each solid cube's side with air in front of it, grouped into
//! planes, a wall's face or a roof's top, over which a motif runs on from cube to cube.

use crate::Stuff;
use glam::IVec3;
use std::collections::BTreeMap;

/// A side of a cube: its outward normal, and the two axes across it, `u` along and `v` up
/// (or, on a face looking up or down, `z`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    pub normal: IVec3,
    pub u: IVec3,
    pub v: IVec3,
}

/// The six faces in the lookup's order: +x, -x, +y, -y, +z, -z.
pub const FACES: [Face; 6] = [
    Face {
        normal: IVec3::X,
        u: IVec3::Z,
        v: IVec3::Y,
    },
    Face {
        normal: IVec3::NEG_X,
        u: IVec3::Z,
        v: IVec3::Y,
    },
    Face {
        normal: IVec3::Y,
        u: IVec3::X,
        v: IVec3::Z,
    },
    Face {
        normal: IVec3::NEG_Y,
        u: IVec3::X,
        v: IVec3::Z,
    },
    Face {
        normal: IVec3::Z,
        u: IVec3::X,
        v: IVec3::Y,
    },
    Face {
        normal: IVec3::NEG_Z,
        u: IVec3::X,
        v: IVec3::Y,
    },
];

/// The exposed faces of one side and depth: a wall's face, a roof's top.
#[derive(Clone, Debug)]
pub struct Plane {
    /// Which of [`FACES`].
    pub face: usize,
    /// Its cubes, each with its place across the plane, `(u, v)` in cubes.
    pub cubes: Vec<(IVec3, [i32; 2])>,
    /// The least and greatest `(u, v)` of its cubes.
    pub min: [i32; 2],
    pub max: [i32; 2],
}

impl Plane {
    /// Its size in cubes across and up.
    pub fn size(&self) -> [i32; 2] {
        [self.max[0] - self.min[0] + 1, self.max[1] - self.min[1] + 1]
    }
}

/// Where a cube is across a face.
pub fn across(face: &Face, at: IVec3) -> [i32; 2] {
    [at.dot(face.u), at.dot(face.v)]
}

/// Every exposed face of the cubes in the box, by plane, in a fixed order.
pub fn planes(stuff: &dyn Fn(IVec3) -> Stuff, low: IVec3, high: IVec3) -> Vec<Plane> {
    let mut by: BTreeMap<(usize, i32), Plane> = BTreeMap::new();
    for z in low.z..=high.z {
        for y in low.y..=high.y {
            for x in low.x..=high.x {
                let at = IVec3::new(x, y, z);
                let own = stuff(at);
                if !own.solid() {
                    continue;
                }
                for (index, face) in FACES.iter().enumerate() {
                    // Undersides are out of the weather; a roof weathers only on top and
                    // where its edges show.
                    if face.normal.y < 0 || stuff(at + face.normal).solid() {
                        continue;
                    }
                    let place = across(face, at);
                    let plane = by.entry((index, at.dot(face.normal))).or_insert(Plane {
                        face: index,
                        cubes: Vec::new(),
                        min: place,
                        max: place,
                    });
                    plane.min = [plane.min[0].min(place[0]), plane.min[1].min(place[1])];
                    plane.max = [plane.max[0].max(place[0]), plane.max[1].max(place[1])];
                    plane.cubes.push((at, place));
                }
            }
        }
    }
    by.into_values().collect()
}
