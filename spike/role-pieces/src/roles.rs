//! The role function: every exposed face, edge and corner of a cube classified by its
//! neighbours, as `terra_build`'s layouts are, a pure function of the cubes and the ground
//! with no Bevy and nothing stored. What a cube shows is then a kit's choice per role
//! (research/voxel-building-look.md, "A role for every exposed part of a cube").
//!
//! A cube's six faces, each by what is beyond it: the same kind (hidden), another kind (a
//! joint), the ground, the air, or the air in a hole (a carved opening, framed). Its twelve
//! edges, each by the faces either side: an outside edge where both are open, named by
//! where it is (an arris up a corner, a crest along a top, a soffit under an overhang, a
//! foot along the bottom of a wall that stands on the ground or on another kind) and by
//! whether it is round a hole (jamb, sill, head). Its eight corners, by the edges they join.
//! Runs of the same edge role along a line are what a profile is swept along.

use glam::{IVec3, Vec3};
use std::collections::BTreeMap;
use terra_build::{Base, CELL_M, Fill, Kind};

/// The faces, in `terra_build::surface`'s order: +X, -X, +Y, -Y, +Z, -Z.
pub const FACES: [(usize, i32); 6] = [(0, 1), (0, -1), (1, 1), (1, -1), (2, 1), (2, -1)];

pub fn face_index(axis: usize, side: i32) -> usize {
    axis * 2 + usize::from(side < 0)
}

pub fn step(axis: usize, side: i32) -> IVec3 {
    let mut v = IVec3::ZERO;
    v[axis] = side;
    v
}

/// What is beyond a face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beyond {
    Same,
    Other(Kind),
    Ground,
    Air,
    /// The air in a hole: a cell walled either side along a line and covered over.
    Hole,
}

impl Beyond {
    pub fn open(self) -> bool {
        matches!(self, Beyond::Air | Beyond::Hole)
    }
}

/// What an edge of a cube is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Edge {
    /// Not an outside edge: within a face, hidden, or an inside corner.
    None,
    /// Up an outside corner.
    Arris,
    /// Up the side of a hole.
    Jamb,
    /// Along a top open to the sky.
    Crest,
    /// Along the bottom of a hole: a window's sill.
    Sill,
    /// Along an underside over the air: an overhang's edge.
    Soffit,
    /// Along the top of a hole: its head, the lintel's edge.
    Head,
    /// Along the bottom of a face that stands on the ground or on another kind.
    Foot,
}

/// A cube's roles: its faces, by [`FACES`], and its edges, by [`edge_index`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubeRoles {
    pub faces: [Beyond; 6],
    pub edges: [Edge; 12],
}

/// An edge of a cube: along `axis`, at the corner of the other two given by `high`
/// (`b` then `c`, `b = axis + 1`, `c = axis + 2`, each 0 or 1).
pub fn edge_index(axis: usize, high: [bool; 2]) -> usize {
    axis * 4 + usize::from(high[0]) * 2 + usize::from(high[1])
}

/// The faces either side of an edge, as (axis, side).
pub fn edge_faces(index: usize) -> [(usize, i32); 2] {
    let axis = index / 4;
    let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
    let hb = index & 2 != 0;
    let hc = index & 1 != 0;
    [(b, if hb { 1 } else { -1 }), (c, if hc { 1 } else { -1 })]
}

/// How far a hole may span, in cubes, and be framed as one.
const HOLE_REACH: i32 = 6;

/// The neighbourhood the roles are read from: the base's cubes of one kind and the ground.
pub struct Neighbours<'a> {
    pub base: &'a Base,
    pub kind: Kind,
    /// The ground's height in metres at a point east and south.
    pub ground: &'a dyn Fn(f32, f32) -> f32,
}

impl Neighbours<'_> {
    fn solid(&self, at: IVec3) -> Option<Kind> {
        self.base
            .cube(at)
            .filter(|cube| cube.fill == Fill::Solid)
            .map(|cube| cube.kind)
    }

    /// Whether a cell is in the ground: its middle below the ground's height there.
    pub fn in_ground(&self, at: IVec3) -> bool {
        let middle = (at.as_vec3() + Vec3::splat(0.5)) * CELL_M;
        middle.y < (self.ground)(middle.x, middle.z)
    }

    fn filled(&self, at: IVec3) -> bool {
        self.solid(at).is_some() || self.in_ground(at)
    }

    /// Whether an empty cell is in a hole: filled either side of it along a level line,
    /// and filled over it, each within reach.
    pub fn in_hole(&self, at: IVec3) -> bool {
        let reach = |dir: IVec3| (1..=HOLE_REACH).any(|n| self.filled(at + dir * n));
        let covered = (1..=HOLE_REACH).any(|n| self.solid(at + IVec3::Y * n).is_some());
        covered
            && ((reach(IVec3::X) && reach(IVec3::NEG_X))
                || (reach(IVec3::Z) && reach(IVec3::NEG_Z)))
    }

    pub fn beyond(&self, at: IVec3) -> Beyond {
        match self.solid(at) {
            Some(kind) if kind == self.kind => Beyond::Same,
            Some(kind) => Beyond::Other(kind),
            None if self.in_ground(at) => Beyond::Ground,
            None if self.in_hole(at) => Beyond::Hole,
            None => Beyond::Air,
        }
    }

    /// A cube's roles.
    pub fn roles(&self, cube: IVec3) -> CubeRoles {
        let faces = FACES.map(|(axis, side)| self.beyond(cube + step(axis, side)));
        let face = |axis: usize, side: i32| faces[face_index(axis, side)];
        let below = face(1, -1);
        let mut edges = [Edge::None; 12];
        for (index, edge) in edges.iter_mut().enumerate() {
            let [(ab, sb), (ac, sc)] = edge_faces(index);
            let (fb, fc) = (face(ab, sb), face(ac, sc));
            let axis = index / 4;
            // A wall's foot: the bottom of a side face where the cube stands on the ground or
            // on another kind.
            let foot = |up_down: (usize, i32), side: Beyond| {
                up_down == (1, -1)
                    && side.open()
                    && matches!(below, Beyond::Ground | Beyond::Other(_))
            };
            if axis != 1 && (foot((ab, sb), fc) || foot((ac, sc), fb)) {
                *edge = Edge::Foot;
                continue;
            }
            if !(fb.open() && fc.open()) {
                continue;
            }
            // Both open: an outside edge, unless the cube beyond the edge fills the corner.
            let diagonal = cube + step(ab, sb) + step(ac, sc);
            if self.solid(diagonal) == Some(self.kind) {
                continue;
            }
            let hole = fb == Beyond::Hole || fc == Beyond::Hole;
            *edge = if axis == 1 {
                if hole { Edge::Jamb } else { Edge::Arris }
            } else {
                // A level edge: one of its faces is the top or the bottom.
                let (vertical, other) = if ab == 1 { (sb, fc) } else { (sc, fb) };
                let up_open = if ab == 1 { fb } else { fc };
                match (vertical > 0, up_open == Beyond::Hole || other == Beyond::Hole) {
                    (true, false) => Edge::Crest,
                    (true, true) => Edge::Sill,
                    (false, false) => Edge::Soffit,
                    (false, true) => Edge::Head,
                }
            };
        }
        CubeRoles { faces, edges }
    }

    /// Every cube of the kind's roles.
    pub fn all(&self) -> BTreeMap<[i32; 3], CubeRoles> {
        self.base
            .cubes()
            .filter(|(_, cube)| cube.kind == self.kind && cube.fill == Fill::Solid)
            .map(|(at, _)| (at.to_array(), self.roles(at)))
            .collect()
    }
}

/// A straight run of one edge role: along `axis` from one point to another in metres, the
/// two open faces' outward normals, and the cubes it runs past.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub role: Edge,
    pub axis: usize,
    pub from: Vec3,
    pub to: Vec3,
    /// Outward from the edge: square to it, each along one of its faces' normals. For a
    /// level edge the first is the side face's, the second the top's or bottom's.
    pub out: [Vec3; 2],
    pub cubes: Vec<IVec3>,
    /// Round a hole the kit already frames (D210's trims round a carved or glazed cube).
    pub framed: bool,
}

/// The edges of every cube gathered into straight runs of one role each.
pub fn runs(roles: &BTreeMap<[i32; 3], CubeRoles>) -> Vec<Run> {
    // Keyed by role, the edge's line (its axis and the grid point it crosses at), and its
    // two normals; then consecutive along the axis.
    let mut lines: BTreeMap<(Edge, usize, [i32; 3], [i32; 3], [i32; 3]), Vec<i32>> =
        BTreeMap::new();
    for (&cube, cube_roles) in roles {
        let cube = IVec3::from_array(cube);
        for (index, &edge) in cube_roles.edges.iter().enumerate() {
            if edge == Edge::None {
                continue;
            }
            let axis = index / 4;
            let [(ab, sb), (ac, sc)] = edge_faces(index);
            // The grid point the edge starts at, then its position along the axis apart.
            let mut corner = cube;
            if sb > 0 {
                corner[ab] += 1;
            }
            if sc > 0 {
                corner[ac] += 1;
            }
            let along = corner[axis];
            corner[axis] = 0;
            // A foot's faces: the side and down; written side first.
            let (first, second) = if ab == 1 {
                (step(ac, sc), step(ab, sb))
            } else {
                (step(ab, sb), step(ac, sc))
            };
            lines
                .entry((edge, axis, corner.to_array(), first.to_array(), second.to_array()))
                .or_default()
                .push(along);
        }
    }
    let mut out = Vec::new();
    for ((role, axis, corner, first, second), mut alongs) in lines {
        alongs.sort();
        let mut start = 0;
        for i in 0..alongs.len() {
            let last = i + 1 == alongs.len() || alongs[i + 1] != alongs[i] + 1;
            if !last {
                continue;
            }
            let (low, high) = (alongs[start], alongs[i] + 1);
            let point = |along: i32| {
                let mut p = IVec3::from_array(corner);
                p[axis] = along;
                p.as_vec3() * CELL_M
            };
            let cubes = (low..high)
                .map(|along| {
                    let mut c = IVec3::from_array(corner);
                    c[axis] = along;
                    // Back from the grid point into the cube on the outward sides.
                    for (n, o) in [first, second].iter().enumerate() {
                        let _ = n;
                        for k in 0..3 {
                            if o[k] > 0 {
                                c[k] -= 1;
                            }
                        }
                    }
                    c
                })
                .collect();
            out.push(Run {
                role,
                axis,
                from: point(low),
                to: point(high),
                out: [
                    IVec3::from_array(first).as_vec3(),
                    IVec3::from_array(second).as_vec3(),
                ],
                cubes,
                framed: false,
            });
            start = i + 1;
        }
    }
    out
}

/// How a run's end meets the runs of the same role there: square to the end if none does,
/// else mitred towards the one that turns away: at a point `o` out from the edge along
/// its first normal, the end moves `o * mitre` further along it.
pub fn mitres(runs: &[Run], index: usize) -> [f32; 2] {
    let run = &runs[index];
    let dir = (run.to - run.from).normalize();
    [(run.from, -dir), (run.to, dir)].map(|(end, outward)| {
        runs.iter()
            .enumerate()
            .filter(|&(j, other)| {
                j != index
                    && other.role == run.role
                    && other.axis != run.axis
                    && other.out[1] == run.out[1]
                    && (other.from.distance(end) < 1e-4 || other.to.distance(end) < 1e-4)
            })
            .map(|(_, other)| outward.dot(other.out[0]))
            .find(|d| d.abs() > 0.5)
            .unwrap_or(0.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_build::{BuildOp, Cube};

    fn base(cubes: &[([i32; 3], [i32; 3])]) -> Base {
        let mut base = Base::default();
        for &(min, max) in cubes {
            base.apply(BuildOp::Fill {
                min,
                max,
                cube: Some(Cube::new(Kind(2), Fill::Solid)),
            });
        }
        base
    }

    fn flat(_: f32, _: f32) -> f32 {
        0.0
    }

    #[test]
    fn a_free_wall_has_arrises_crests_and_a_foot() {
        let base = base(&[([0, 0, 0], [3, 3, 0])]);
        let n = Neighbours {
            base: &base,
            kind: Kind(2),
            ground: &flat,
        };
        let roles = n.all();
        let runs = runs(&roles);
        let count = |role: Edge| runs.iter().filter(|r| r.role == role).count();
        // Four uprights at the ends, the crests along both faces and across both ends, the
        // foot along both faces and both ends.
        assert_eq!(count(Edge::Arris), 4);
        assert_eq!(count(Edge::Crest), 4);
        assert_eq!(count(Edge::Foot), 4);
        assert_eq!(count(Edge::Soffit), 0);
        let crest = runs
            .iter()
            .find(|r| r.role == Edge::Crest && r.axis == 0)
            .unwrap();
        assert!((crest.to.x - crest.from.x - 2.0).abs() < 1e-5);
        // The crests meet at the corners, mitred outward.
        let i = runs.iter().position(|r| r == crest).unwrap();
        assert_eq!(mitres(&runs, i), [1.0, 1.0]);
    }

    #[test]
    fn a_carved_hole_is_framed() {
        let mut base = base(&[([0, 0, 0], [5, 5, 0])]);
        base.apply(BuildOp::Fill {
            min: [2, 2, 0],
            max: [3, 3, 0],
            cube: None,
        });
        let n = Neighbours {
            base: &base,
            kind: Kind(2),
            ground: &flat,
        };
        let runs = runs(&n.all());
        let has = |role: Edge| runs.iter().any(|r| r.role == role);
        assert!(has(Edge::Jamb) && has(Edge::Sill) && has(Edge::Head));
    }

    #[test]
    fn an_overhang_has_soffits() {
        let base = base(&[([0, 0, 0], [0, 3, 0]), ([-2, 4, -1], [2, 4, 1])]);
        let n = Neighbours {
            base: &base,
            kind: Kind(2),
            ground: &flat,
        };
        let runs = runs(&n.all());
        assert!(runs.iter().any(|r| r.role == Edge::Soffit));
    }
}
