//! Layouts (docs/building-plan.md, D160, D181): a base's cubes laid as each kit's pieces,
//! worked out from the cubes alone, in the base's own space and never past its cubes, in
//! the order they go in. No model sizes, so a server needs no art; each piece's version is
//! a seed hashed from its place, the same on every machine.
//!
//! - [`runs`]: long members, logs or beams, laid along each run of cubes, crossing at
//!   corners, shortened under roofs, upright at posts.
//! - [`trims`]: the boards round every opening, and the doors and windows set in them.
//! - [`floors`]: a timber kind's floors and flat roofs, as a deck of boards.
//! - [`blocks`]: block kinds, a piece a cube, drawn as their surface (D197): walls,
//!   floors and footings alike.
//! - [`frames`]: a framed block kind's timber frame over its infill (D236).
//! - [`roofs`]: roof kinds' cubes as a roof, from their neighbours (D241).

pub mod blocks;
mod dressing;
mod floors;
pub mod frames;
pub mod roles;
pub mod roofs;
pub mod runs;
mod shapes;
mod stairs;
mod trims;

use crate::kit::{Kit, Kits, Pattern};
use crate::mesh::Material;
use crate::{Base, CELL_M, Fill, Kind};
use glam::{IVec3, Vec3};
pub use roles::{Dir, End, Role};
use std::collections::{BTreeMap, BTreeSet};

/// Which half of a log is kept, where it is split along its length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Split {
    Whole,
    /// The top half, its flat side down: the first course of a wall.
    Top,
    /// The bottom half, its flat side up: the last.
    Bottom,
}

/// How finely a piece is placed: in 256ths of a cube, about 2 mm (docs/building-plan.md:
/// a layout's output is in grid units, not metres).
pub const TICKS: i32 = 256;

/// A point in a base's grid, counted in [`TICKS`] of a cube east, up and south: where a
/// piece is, exact and the same on every machine, so nothing shared holds metres.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place(pub [i32; 3]);

impl Place {
    /// The place nearest a point given in metres in the base's own space.
    pub fn near(metres: Vec3) -> Place {
        Place([ticks(metres.x), ticks(metres.y), ticks(metres.z)])
    }

    /// Where it is in metres in the base's own space, for drawing.
    pub fn metres(self) -> Vec3 {
        IVec3::from_array(self.0).as_vec3() * (CELL_M / TICKS as f32)
    }
}

/// A length in metres, to the nearest tick.
pub fn ticks(metres: f32) -> i32 {
    (metres * (TICKS as f32 / CELL_M)).round() as i32
}

/// A length in ticks, in metres.
pub fn metres(ticks: i32) -> f32 {
    ticks as f32 * (CELL_M / TICKS as f32)
}

/// One piece of a base, placed in its grid ([`Place`]); its seed is its version.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Piece {
    /// A long member from one end to the other along its middle; its seed is its version,
    /// from its place.
    Log {
        from: Place,
        to: Place,
        split: Split,
        seed: u32,
        /// The daub under it, as far along it from `from` as from the first number to
        /// the second, in ticks, if any.
        chinking: Option<(i32, i32)>,
        /// How each end, at `from` and at `to`, is cut to the slope of a roof over it, as
        /// a gable's logs are: how many ticks the slope rises for each cube it runs in from
        /// that end, from the log's foot there; none, sawn square.
        lean: [i32; 2],
        /// The role it is laid for, drawn as the kit dresses that role (`kit::Dress::Member`):
        /// a plate along a wall's top, a sill on what it stands on, a post up an opening's
        /// side or a free end, a step's tread; none, a course of the wall.
        role: Option<Role>,
    },
    /// A board round an opening: a jamb, a head or a sill.
    Board { min: Place, max: Place },
    /// A profile of the kit's pieces along one edge of one cube, as the kit dresses the
    /// edge's role (`roles`, `dress`, D215): a coping, a plinth, a sill, a lintel. `side`
    /// is the face it looks out of, `other` the edge's other face; `ends` how it meets the
    /// next, low end first along its axis; a foot is raised `lift` ticks to step with the
    /// ground and goes on down `sink` more to reach it.
    Trim {
        cube: IVec3,
        role: Role,
        side: Dir,
        other: Dir,
        ends: [End; 2],
        lift: i32,
        sink: i32,
        seed: u32,
    },
    /// Corner stones of the kit's up one upright edge of one cube, a stone a course, in the
    /// courses it shows: quoins up an outside corner, jamb stones up an opening's side.
    /// `faces` are the two it stands out of, the first the face of its first course's
    /// longer side. `shared` says, for each face, whether the cube's other edge on it has
    /// corner stones too, as a pier a cube wide has: then the stones on that face stop
    /// short of its middle and meet the other edge's at a joint, rather than through them.
    Corner {
        cube: IVec3,
        role: Role,
        faces: [Dir; 2],
        courses: [bool; 2],
        shared: [bool; 2],
        seed: u32,
    },
    /// A profile of the kit's pieces swept along a line of a cube cut to a shape (D193,
    /// `dressing`): a stair's noses, a half's or a slope's coping, a pillar's base and cap.
    /// It looks out of `side`'s face, the profile's up square to the line and to `side`;
    /// its ends meet the next as a trim's do. It belongs to `cube`.
    Band {
        cube: IVec3,
        role: Role,
        from: Place,
        to: Place,
        side: Dir,
        ends: [End; 2],
        seed: u32,
    },
    /// A sawn member of a deck from one end to the other along its middle (`floors`): a
    /// board, a joist or a rim beam, so wide across and so high, in ticks; its seed is its
    /// version.
    Plank {
        from: Place,
        to: Place,
        wide: i32,
        high: i32,
        seed: u32,
    },
    /// A block kind's cube (`blocks`), drawn as part of its kind's surface (`surface.rs`),
    /// counted in cubes.
    Block { cube: IVec3 },
    /// A framed kind's cube's core (`frames`, D236): the wattle the daub goes on, drawn as a
    /// surface of its own where its cube is in but its infill is not finished, or is being
    /// cut out. It goes in before the cube's `Block`.
    Wattle { cube: IVec3 },
    /// A squared timber of a frame (`frames`, D236): from one end to the other along its
    /// middle, so wide across the face it shows on and so deep into the wall along `face`,
    /// the way that face looks, in ticks; its seed is its version.
    Beam {
        from: Place,
        to: Place,
        face: Dir,
        wide: i32,
        deep: i32,
        seed: u32,
    },
    /// A block kind's wall carried up into a roof cube standing on it (`roofs::gables`):
    /// the roof cube's surface of the wall's stone, cut off at the roof's underside, whose
    /// height in ticks at the cube's corners, the middles of its sides and its middle, west
    /// to east and north to south, is `under`.
    Gable { cube: IVec3, under: [[i32; 3]; 3] },
    /// A square of a roof's covering (`roofs`), half a cube across: its
    /// corners on the roof's top, north-west, north-east, south-east and south-west, and
    /// its middle; its courses' lower edges, the first's height and how far apart, in
    /// ticks; which of its sides, north, east, south and west, no covering carries on past.
    Covering {
        corners: [Place; 4],
        middle: Place,
        courses: (i32, i32),
        open: [bool; 4],
        cell: roofs::RoofCell,
    },
    /// A long piece of a roof (`roofs`), along its line from one end to the other: a rafter
    /// along its middle, the rest along the roof's top. `sides` are a point on the roof
    /// either side of it; for an edge, the second is beyond it.
    Roof {
        part: roofs::RoofPart,
        from: Place,
        to: Place,
        sides: [Place; 2],
        seed: u32,
        cell: roofs::RoofCell,
    },
}

impl Piece {
    /// The box it fills in a wall or floor, in metres, if it stands in the base's cubes:
    /// not a door or window, which is not part of the plan, nor a piece of roof.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        match *self {
            Piece::Log { from, to, .. } => {
                let (from, to) = (from.metres(), to.metres());
                let r = Vec3::splat(0.15);
                Some((from.min(to) - r, from.max(to) + r))
            }
            Piece::Plank {
                from,
                to,
                wide,
                high,
                ..
            } => {
                let (from, to) = (from.metres(), to.metres());
                let along = (to - from).normalize_or_zero();
                let across = Vec3::Y.cross(along).abs() * (metres(wide) * 0.5);
                let up = Vec3::Y * (metres(high) * 0.5);
                Some((from.min(to) - across - up, from.max(to) + across + up))
            }
            Piece::Board { min, max } => Some((min.metres(), max.metres())),
            Piece::Beam {
                from,
                to,
                face,
                wide,
                deep,
                ..
            } => {
                let (from, to) = (from.metres(), to.metres());
                let normal = face.step().as_vec3().abs();
                let across = (to - from).normalize_or_zero().cross(normal).abs();
                let reach = across * (metres(wide) * 0.5) + normal * (metres(deep) * 0.5);
                Some((from.min(to) - reach, from.max(to) + reach))
            }
            Piece::Block { cube }
            | Piece::Wattle { cube }
            | Piece::Gable { cube, .. }
            | Piece::Trim { cube, .. }
            | Piece::Band { cube, .. }
            | Piece::Corner { cube, .. } => {
                let low = cube.as_vec3() * CELL_M;
                Some((low, low + Vec3::splat(CELL_M)))
            }
            Piece::Covering { .. } | Piece::Roof { .. } => None,
        }
    }

    /// The cube that owns it, which its middle is in, worked out exactly in ticks: a long
    /// member crossing from one section into another belongs to one of them only, the
    /// same on every machine (docs/building-plan.md, drawing).
    pub fn anchor(&self) -> IVec3 {
        let middle = |a: Place, b: Place| {
            [0, 1, 2].map(|i| (i64::from(a.0[i]) + i64::from(b.0[i])).div_euclid(2) as i32)
        };
        let point = match *self {
            Piece::Block { cube }
            | Piece::Wattle { cube }
            | Piece::Gable { cube, .. }
            | Piece::Trim { cube, .. }
            | Piece::Band { cube, .. }
            | Piece::Corner { cube, .. } => return cube,
            Piece::Log { from, to, .. }
            | Piece::Plank { from, to, .. }
            | Piece::Beam { from, to, .. }
            | Piece::Roof { from, to, .. } => middle(from, to),
            Piece::Board { min, max } => middle(min, max),
            Piece::Covering { middle, .. } => middle.0,
        };
        IVec3::from_array(point.map(|p| p.div_euclid(TICKS)))
    }

    /// The section that owns it: the one its [`anchor`](Piece::anchor) is in.
    pub fn section(&self) -> IVec3 {
        crate::cubes::section_of(self.anchor())
    }

    /// The roof cube it goes up with, and in which stage, if it is part of a roof.
    pub fn roof_cell(&self) -> Option<roofs::RoofCell> {
        match *self {
            Piece::Covering { cell, .. } | Piece::Roof { cell, .. } => Some(cell),
            _ => None,
        }
    }

    /// The lowest point of it, which orders the pieces as they go in.
    fn bottom(&self) -> f32 {
        match self {
            Piece::Log { from, to, .. } => from.metres().y.min(to.metres().y),
            Piece::Plank { from, to, high, .. } => {
                from.metres().y.min(to.metres().y) - metres(*high) * 0.5
            }
            Piece::Board { min, .. } => min.metres().y,
            Piece::Block { cube } | Piece::Gable { cube, .. } => cube.y as f32 * CELL_M,
            // A cube's wattle before its daub, woven a layer ahead of it: in after the
            // blocks of the layer under it.
            Piece::Wattle { cube } => (cube.y - 1) as f32 * CELL_M + 0.01,
            // A frame's members as their lowest point is reached: posts up a storey with its
            // first cubes, the plates on them when their tops are.
            Piece::Beam { .. } => self.bounds().map_or(0.0, |(min, _)| min.y),
            // Dressed once its cube is in.
            Piece::Trim { cube, .. } | Piece::Band { cube, .. } | Piece::Corner { cube, .. } => {
                cube.y as f32 * CELL_M + 0.01
            }
            // A roof after the walls under it, a stage at a time, each from its eaves up.
            Piece::Covering { corners, cell, .. } => {
                let low = corners
                    .iter()
                    .map(|c| c.metres().y)
                    .fold(f32::MAX, f32::min);
                low + 20.0 * f32::from(cell.stage)
            }
            // Its caps and flashing once its covering is on.
            Piece::Roof {
                from,
                to,
                cell,
                part,
                ..
            } => {
                let last = matches!(
                    part,
                    roofs::RoofPart::Ridge | roofs::RoofPart::Hip | roofs::RoofPart::Flashing
                );
                from.metres().y.min(to.metres().y)
                    + 20.0 * f32::from(cell.stage)
                    + if last { 10.0 } else { 0.0 }
            }
        }
    }
}

/// A base's walls in one kit's kind, as pieces.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    /// In the order they go in.
    pub pieces: Vec<Piece>,
}

/// Which way a wall runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Axis {
    /// East and west: its logs lie along X.
    X,
    /// North and south: its logs lie along Z, half a course higher.
    Z,
}

impl Axis {
    fn step(self) -> IVec3 {
        match self {
            Axis::X => IVec3::X,
            Axis::Z => IVec3::Z,
        }
    }

    fn other(self) -> Axis {
        match self {
            Axis::X => Axis::Z,
            Axis::Z => Axis::X,
        }
    }

    /// The cube so far along a row, on a layer.
    fn cube(self, row: i32, along: i32, layer: i32) -> IVec3 {
        match self {
            Axis::X => IVec3::new(along, layer, row),
            Axis::Z => IVec3::new(row, layer, along),
        }
    }

    /// A cube's row and how far along it.
    fn row_of(self, at: IVec3) -> (i32, i32) {
        match self {
            Axis::X => (at.z, at.x),
            Axis::Z => (at.x, at.z),
        }
    }

    /// A point so far along the middle of a row, at a height, in metres.
    fn point(self, row: i32, along: f32, height: f32) -> Vec3 {
        let middle = (row as f32 + 0.5) * CELL_M;
        match self {
            Axis::X => Vec3::new(along, height, middle),
            Axis::Z => Vec3::new(middle, height, along),
        }
    }

    /// Level and across the row.
    fn across(self) -> Vec3 {
        match self {
            Axis::X => Vec3::Z,
            Axis::Z => Vec3::X,
        }
    }

    /// The height of a course's middle: the walls one way lie half a course above the
    /// other's.
    fn centre(self, kit: &Kit, course: i32) -> f32 {
        match self {
            Axis::X => kit.course * (course as f32 + 0.5),
            Axis::Z => kit.course * course as f32,
        }
    }
}

/// How many cubes thick a wall may be and still be laid as skins along its faces, each
/// course a log a skin, rather than as crossing walls.
const THICK: i32 = 3;

/// What a course of logs is at one cube of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spot {
    Log(Split),
    /// Cut by an opening: framed.
    Hole,
    Nothing,
}

/// The cubes of a base a kit lays, and which way each runs.
struct Walls<'a> {
    base: &'a Base,
    kind: Kind,
    /// What each cube asked about is, asked of the base once: a row's courses ask of each
    /// of its cubes many times.
    known: std::cell::RefCell<crate::mesh::QuickMap<IVec3, Option<Fill>>>,
    /// Its flights' cubes (`stairs`), which are laid as steps, not walls.
    stairs: BTreeSet<[i32; 3]>,
}

/// What a timber kind's cube is laid as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cell {
    /// Courses of its members, as a wall.
    Wall,
    /// A deck of boards on joists (`floors`): a floor, a flat roof, or the top of a solid
    /// mass of the kind, as a platform or a slab is.
    Deck,
    /// Inside a solid mass, hidden by what is round it: nothing.
    Inside,
}

impl Walls<'_> {
    /// Whether a cube is a whole, solid cube of the kind: what a wall or a deck is made of.
    fn whole(&self, at: IVec3) -> bool {
        self.base.cube(at).is_some_and(|cube| {
            cube.kind == self.kind && drawn_fill(&cube) == Fill::Solid && !cube.shape.is_cut()
        })
    }

    /// Whether a cube is in a wall (`Base::in_wall`), but for the top of a column over a
    /// post or another cube cut to a shape, which is what it carries: a deck or a beam, not
    /// a wall's top.
    fn walled(&self, at: IVec3) -> bool {
        if !self.base.in_wall(at) {
            return false;
        }
        let top = !self.base.in_wall(at + IVec3::Y);
        !(top
            && self
                .base
                .cube(at - IVec3::Y)
                .is_some_and(|cube| cube.fill == Fill::Solid && cube.shape.is_cut()))
    }

    /// Whether a whole cube of a wall is part of a solid mass of the kind: in a block of
    /// two by two of them side by side on its layer, as a platform or a slab is and a wall
    /// a cube thick never is; a wall's top beside a flat roof is no mass.
    fn mass(&self, at: IVec3) -> bool {
        let walled = |at: IVec3| self.whole(at) && self.walled(at);
        walled(at)
            && [(1, 1), (1, -1), (-1, 1), (-1, -1)].iter().any(|&(x, z)| {
                walled(at + IVec3::new(x, 0, 0))
                    && walled(at + IVec3::new(0, 0, z))
                    && walled(at + IVec3::new(x, 0, z))
            })
    }

    /// What a cube of the kind is laid as, if it is one this layout lays: a floor or flat
    /// roof is a deck; a solid mass's top is a deck, its inside nothing and its sides walls,
    /// so a platform is boards on a frame, not a pile of log ends; the rest of a wall's
    /// cubes are walls. Cubes cut to a shape and a flight's are laid apart (`shapes`,
    /// `stairs`).
    pub(crate) fn cell(&self, at: IVec3) -> Option<Cell> {
        if self.stairs.contains(&at.to_array()) || !self.whole(at) {
            return None;
        }
        if !self.walled(at) {
            return Some(Cell::Deck);
        }
        if !self.mass(at) {
            return Some(Cell::Wall);
        }
        let covered = self
            .base
            .cube(at + IVec3::Y)
            .is_some_and(|cube| cube.fill != Fill::Open);
        let inside = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
            .iter()
            .all(|&side| self.whole(at + side));
        Some(match (covered, inside) {
            (false, _) => Cell::Deck,
            (true, true) => Cell::Inside,
            (true, false) => Cell::Wall,
        })
    }

    /// Whether a cube is a roof's standing on a whole cube of a wall of the kind: the wall
    /// is carried up into it to the roof's underside, as a gable is (D241).
    fn gable(&self, at: IVec3) -> bool {
        self.base
            .cube(at)
            .is_some_and(|cube| cube.kind.is_roof() && cube.fill != Fill::Open)
            && self.whole(at - IVec3::Y)
            && self.walled(at - IVec3::Y)
    }

    /// A cube of the kit's kind that is part of a wall, or a roof's cube its wall is carried
    /// up into.
    fn ours(&self, at: IVec3) -> Option<Fill> {
        if self.stairs.contains(&at.to_array()) {
            return None;
        }
        if let Some(&known) = self.known.borrow().get(&at) {
            return known;
        }
        let ours = self
            .base
            .cube(at)
            .filter(|cube| {
                cube.kind == self.kind
                    && self.walled(at)
                    && !cube.shape.is_cut()
                    && (drawn_fill(cube) != Fill::Solid || self.cell(at) == Some(Cell::Wall))
            })
            .map(|cube| drawn_fill(&cube))
            .or_else(|| self.gable(at).then_some(Fill::Solid));
        self.known.borrow_mut().insert(at, ours);
        ours
    }

    /// Whether a cube of ours runs along an axis: it has another of ours either side, and
    /// is not across a thick wall, a wall two or three cubes thick, whose courses lie along
    /// its faces only, as two skins of logs side by side, so its faces show logs and not a
    /// grid of sawn ends (research/goals-review-2026-10-13.md, finding 6). At a thick wall's
    /// end, or where it turns or another meets it, its cubes cross as any corner does.
    fn runs(&self, at: IVec3, axis: Axis) -> bool {
        let beside = |at: IVec3, axis: Axis| {
            self.ours(at + axis.step()).is_some() || self.ours(at - axis.step()).is_some()
        };
        self.ours(at).is_some()
            && beside(at, axis)
            && !(beside(at, axis.other()) && self.across_thick(at, axis))
    }

    /// Whether the run of ours along an axis through a cube is across a thick wall: no more
    /// than [`THICK`] cubes, each with ours either side of it the other way, along the wall.
    fn across_thick(&self, at: IVec3, axis: Axis) -> bool {
        let step = axis.step();
        let along = axis.other().step();
        let mut run = vec![at];
        for dir in [step, -step] {
            let mut next = at + dir;
            while self.ours(next).is_some() {
                if run.len() >= THICK as usize {
                    return false;
                }
                run.push(next);
                next += dir;
            }
        }
        run.iter()
            .all(|&c| self.ours(c + along).is_some() && self.ours(c - along).is_some())
    }

    /// What a course is at a cube of a row: a log where the cubes it lies in are solid and
    /// run its way, split where it is at the foot or top of them, cut where one is open.
    fn spot(&self, kit: &Kit, axis: Axis, row: i32, along: i32, course: i32) -> Spot {
        let centre = axis.centre(kit, course);
        let half = kit.course * 0.5;
        let layer = |height: f32| (height / CELL_M).floor() as i32;
        let (low, high) = (layer(centre - half + 1e-4), layer(centre + half - 1e-4));
        let state = |layer: i32| {
            let at = axis.cube(row, along, layer);
            if !self.runs(at, axis) {
                return None;
            }
            self.ours(at)
        };
        match (state(low), state(high)) {
            _ if low == high => match state(low) {
                Some(Fill::Solid) => Spot::Log(Split::Whole),
                Some(_) => Spot::Hole,
                None => Spot::Nothing,
            },
            (Some(Fill::Solid), Some(Fill::Solid)) => Spot::Log(Split::Whole),
            (None, Some(Fill::Solid)) => Spot::Log(Split::Top),
            (Some(Fill::Solid), None) => Spot::Log(Split::Bottom),
            (None, None) => Spot::Nothing,
            _ => Spot::Hole,
        }
    }

    /// Whether the open air is beyond a cube, so a log may run past into it.
    fn open_beyond(&self, at: IVec3) -> bool {
        self.base.cube(at).is_none() && !self.base.contains(at)
    }
}

/// What fills a cube as a layout draws it: an opening already once any of it has been
/// carved out, so a log is cut and framed at the saw's first cut.
pub fn drawn_fill(cube: &crate::Cube) -> Fill {
    if cube.pending == crate::Pending::Carve && !cube.is_built() {
        Fill::Open
    } else {
        cube.fill
    }
}

/// Lays everything of a base in the kit's kind, as [`lay_among`] does, with the kit the
/// only one there is.
pub fn lay(base: &Base, kit: &Kit) -> Layout {
    lay_among(base, kit, &Kits(vec![kit.clone()]))
}

/// Lays everything of a base built in one of `kits`' kinds, `kit`: its walls, as blocks or
/// as long members, and their gables, and its roofs, which meet the other kits' (`roofs`).
pub fn lay_among(base: &Base, kit: &Kit, kits: &Kits) -> Layout {
    lay_scoped(base, kit, kits, Scope::ALL)
}

/// The pieces of one section of a base (docs/building-plan.md, drawing): exactly those of
/// [`lay_among`] whose [`Piece::section`] it is, in the order they go in, worked out from
/// only the rows, roofs and decks that reach it, so laying a section never doubles or
/// leaves out a piece another section owns, and does not lay the whole base.
pub fn lay_section(base: &Base, kit: &Kit, kits: &Kits, section: IVec3) -> Layout {
    let mut layout = lay_scoped(base, kit, kits, Scope::section(section));
    layout.pieces.retain(|piece| piece.section() == section);
    layout
}

/// As [`lay_section`], with the pieces a cube beyond its edges as well, in a second list:
/// what a block kind's surface needs to round and shade its edge as its neighbour does.
pub fn lay_section_and_edge(
    base: &Base,
    kit: &Kit,
    kits: &Kits,
    section: IVec3,
) -> (Layout, Vec<Piece>) {
    let mut layout = lay_scoped(base, kit, kits, Scope::section(section));
    let low = section * crate::cubes::SECTION - IVec3::ONE;
    let high = low + IVec3::splat(crate::cubes::SECTION + 1);
    let (mut own, mut edge) = (Vec::new(), Vec::new());
    for piece in layout.pieces.drain(..) {
        let at = piece.anchor();
        if crate::cubes::section_of(at) == section {
            own.push(piece);
        } else if at.cmpge(low).all() && at.cmple(high).all() {
            edge.push(piece);
        }
    }
    layout.pieces = own;
    (layout, edge)
}

/// The pieces of some sections of a base: exactly those of [`lay_among`] whose
/// [`Piece::section`] is one of them, worked out from what reaches them alone, as
/// [`lay_section`] does for one, so an edit lays again only the building it is in.
pub fn lay_sections(base: &Base, kit: &Kit, kits: &Kits, sections: &BTreeSet<[i32; 3]>) -> Layout {
    let Some((low, high)) = sections.iter().map(|&at| IVec3::from_array(at)).fold(
        None,
        |bounds: Option<(IVec3, IVec3)>, at| {
            Some(bounds.map_or((at, at), |(l, h)| (l.min(at), h.max(at))))
        },
    ) else {
        return Layout::default();
    };
    let side = crate::cubes::SECTION;
    let scope = Scope {
        low: low * side - IVec3::splat(Scope::MARGIN),
        high: (high + IVec3::ONE) * side - IVec3::ONE + IVec3::splat(Scope::MARGIN),
    };
    let mut layout = lay_scoped(base, kit, kits, scope);
    layout
        .pieces
        .retain(|piece| sections.contains(&piece.section().to_array()));
    layout
}

fn lay_scoped(base: &Base, kit: &Kit, kits: &Kits, scope: Scope) -> Layout {
    let mut layout = if kit.pattern == Pattern::Roof {
        Layout::default()
    } else {
        lay_walls(base, kit, kits, scope)
    };
    roofs::lay(base, kit, kits, scope, &mut layout.pieces);
    sort(&mut layout);
    layout
}

/// The part of a base a layout is worked out for, in cubes, from `low` to `high`: a
/// section and a margin round it, or everything. Each part of a layout takes in whatever
/// could give a piece there (a whole row of logs, a whole roof, a whole deck), so what it
/// lays there is what laying the whole base would.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Scope {
    pub low: IVec3,
    pub high: IVec3,
}

impl Scope {
    pub const ALL: Scope = Scope {
        low: IVec3::splat(i32::MIN / 4),
        high: IVec3::splat(i32::MAX / 4),
    };
    /// How far round a section a layout looks, in cubes: further than any piece reaches
    /// past the cube that owns it.
    const MARGIN: i32 = 2;

    fn section(section: IVec3) -> Scope {
        let low = section * crate::cubes::SECTION;
        Scope {
            low: low - IVec3::splat(Self::MARGIN),
            high: low + IVec3::splat(crate::cubes::SECTION - 1 + Self::MARGIN),
        }
    }

    /// Whether a box of cubes reaches into it.
    pub fn reaches(&self, low: IVec3, high: IVec3) -> bool {
        low.cmple(self.high).all() && high.cmpge(self.low).all()
    }

    /// Its cubes in a base, sorted, within a box as well if given.
    pub fn cubes(&self, base: &Base, within: Option<(IVec3, IVec3)>) -> Vec<(IVec3, crate::Cube)> {
        let Some((low, high)) = base.store().bounds() else {
            return Vec::new();
        };
        let (mut low, mut high) = (low.max(self.low), high.min(self.high));
        if let Some((a, b)) = within {
            (low, high) = (low.max(a), high.min(b));
        }
        if low.cmpgt(high).any() {
            return Vec::new();
        }
        base.store().within(low, high)
    }

    /// A row's cubes of the base along an axis, wherever they are along it: the row
    /// whose cross-coordinate is `row`.
    fn row_cubes(base: &Base, axis: Axis, row: i32) -> Vec<(IVec3, crate::Cube)> {
        let (low, high) = match axis {
            Axis::X => (
                IVec3::new(i32::MIN / 4, i32::MIN / 4, row),
                IVec3::new(i32::MAX / 4, i32::MAX / 4, row),
            ),
            Axis::Z => (
                IVec3::new(row, i32::MIN / 4, i32::MIN / 4),
                IVec3::new(row, i32::MAX / 4, i32::MAX / 4),
            ),
        };
        Scope::ALL.cubes(base, Some((low, high)))
    }

    /// Whether a row across an axis is one whose pieces may be ours.
    fn has_row(&self, axis: Axis, row: i32) -> bool {
        let (low, high) = match axis {
            Axis::X => (self.low.z, self.high.z),
            Axis::Z => (self.low.x, self.high.x),
        };
        (low..=high).contains(&row)
    }
}

/// A kit's walls and their gables.
fn lay_walls(base: &Base, kit: &Kit, kits: &Kits, scope: Scope) -> Layout {
    if kit.pattern == Pattern::Masonry {
        let mut layout = Layout {
            pieces: blocks::lay(base, kit, kits, scope),
        };
        roofs::gables(base, kit, scope, &mut layout.pieces);
        return layout;
    }
    let steps = stairs::find(base, kit, kits);
    let walls = Walls {
        base,
        kind: kit.kind(),
        known: Default::default(),
        stairs: stairs::cubes(&steps),
    };
    // Every cube on the rows that cross the scope, either way, whole.
    let band = |axis: Axis| {
        let (mut low, mut high) = (IVec3::splat(i32::MIN / 4), IVec3::splat(i32::MAX / 4));
        match axis {
            Axis::X => (low.z, high.z) = (scope.low.z, scope.high.z),
            Axis::Z => (low.x, high.x) = (scope.low.x, scope.high.x),
        }
        Scope::ALL.cubes(base, Some((low, high)))
    };
    let mut ours: Vec<IVec3> = band(Axis::X)
        .into_iter()
        .chain(band(Axis::Z))
        .map(|(at, _)| at)
        .filter(|&at| walls.ours(at).is_some())
        .collect();
    ours.sort_by_key(|at| at.to_array());
    ours.dedup();
    let mut layout = Layout::default();
    stairs::lay(kit, &steps, &mut layout);
    if ours.is_empty() {
        if kit.textures.contains_key(&Material::Floor) {
            floors::decks(&walls, kit, scope, &mut layout);
        }
        shapes::lay(&walls, kit, kits, scope, &mut layout);
        sort(&mut layout);
        return layout;
    }
    for axis in [Axis::X, Axis::Z] {
        // Each row of cubes running this way, with how far along it and how high its
        // cubes go.
        let mut rows: BTreeMap<i32, (i32, i32, i32, i32)> = BTreeMap::new();
        for &at in ours.iter().filter(|&&at| walls.runs(at, axis)) {
            let (row, along) = axis.row_of(at);
            if !scope.has_row(axis, row) {
                continue;
            }
            let entry = rows.entry(row).or_insert((along, along, at.y, at.y));
            *entry = (
                entry.0.min(along),
                entry.1.max(along),
                entry.2.min(at.y),
                entry.3.max(at.y),
            );
        }
        for (row, (first, last, lowest, highest)) in rows {
            runs::courses(
                &walls,
                kit,
                axis,
                row,
                (first, last),
                (lowest, highest),
                &mut layout,
            );
            trims::frames(&walls, kit, axis, row, &mut layout);
        }
    }
    runs::posts(&walls, &ours, &mut layout);
    runs::corner_posts(&walls, kit, kits, &ours, &mut layout);
    trims::ends(&walls, kit, kits, &ours, &mut layout);
    if kit.textures.contains_key(&Material::Floor) {
        floors::decks(&walls, kit, scope, &mut layout);
    }
    shapes::lay(&walls, kit, kits, scope, &mut layout);
    // Last, as every log a roof comes down over is cut to it.
    runs::under_roofs(base, kits, kit, &mut layout);
    layout
}

/// Bottom up, keeping walls together within a course.
fn sort(layout: &mut Layout) {
    layout.pieces.sort_by(|a, b| {
        (a.bottom() * 100.0)
            .round()
            .total_cmp(&(b.bottom() * 100.0).round())
    });
}

/// Runs of cells in a line: grouped by the first of what `key` gives, consecutive in the
/// second, as the line, the first and the last.
fn runs_of(
    cells: &BTreeSet<(i32, i32)>,
    key: impl Fn((i32, i32)) -> (i32, i32),
) -> Vec<(i32, i32, i32)> {
    let keyed: BTreeSet<(i32, i32)> = cells.iter().map(|&cell| key(cell)).collect();
    let mut found: Vec<(i32, i32, i32)> = Vec::new();
    for (line, at) in keyed {
        match found.last_mut() {
            Some((last_line, _, high)) if *last_line == line && *high + 1 == at => *high = at,
            _ => found.push((line, at, at)),
        }
    }
    found
}

/// A number from a list of numbers, the same on every machine: a piece's version from its
/// place.
pub(crate) fn hash(parts: &[u32]) -> u32 {
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    for &part in parts {
        state ^= part as u64;
        state = splitmix(state);
    }
    state as u32
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// A number from 0 up to 1, from a hash.
pub(crate) fn unit(hash: u32) -> f32 {
    (hash >> 8) as f32 / (1u32 << 24) as f32
}

/// A seeded run of numbers, for one piece.
pub(crate) struct Random(pub u32);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 = hash(&[self.0]);
        self.0
    }

    /// From 0 up to 1.
    pub fn unit(&mut self) -> f32 {
        unit(self.next())
    }

    /// From -1 up to 1.
    pub fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::kit::Texture;
    use crate::{BuildOp, Cube, Panel};

    pub(crate) fn kit() -> Kit {
        Kit::default()
    }

    /// A cabin four metres by three outside.
    pub(crate) fn cabin() -> Base {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [7, 0, 5],
            kind: kit().kind(),
        });
        base
    }

    pub(crate) fn wall(base: &mut Base, tiles: impl IntoIterator<Item = [i32; 3]>, panel: Panel) {
        for tile in tiles {
            base.apply(BuildOp::SetWall {
                tile,
                panel: Some(panel),
                kind: kit().kind(),
            });
        }
    }

    pub(crate) fn logs(layout: &Layout) -> Vec<(Vec3, Vec3, Split)> {
        layout
            .pieces
            .iter()
            .filter_map(|piece| match *piece {
                Piece::Log {
                    from, to, split, ..
                } => Some((from.metres(), to.metres(), split)),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn boards(layout: &Layout) -> usize {
        layout
            .pieces
            .iter()
            .filter(|p| matches!(p, Piece::Board { .. }))
            .count()
    }

    /// A kit with a texture for floors, so it lays them.
    pub(crate) fn textured() -> Kit {
        let texture = |path: &str| Texture {
            path: path.into(),
            metres: [1.0, 1.0],
            depth: 0.0,
            reflectance: crate::kit::matte(),
            strips: None,
            cells: 1,
        };
        Kit {
            textures: BTreeMap::from([(Material::Floor, texture("floor"))]),
            ..kit()
        }
    }

    #[test]
    fn a_walkway_a_cube_wide_draws_alike_at_any_layer() {
        // A timber walkway a cube wide on a post at each end, at layer 3, partway up what
        // a storey was, and at layer 5, where a storey's floor was (D277; the second goals
        // review, finding 6): a deck both times, never a log wall, and the same pieces.
        let kit = kit();
        let walkway = |y: i32| {
            let mut base = Base::default();
            let post = Cube {
                shape: crate::Shape::Post,
                ..Cube::new(kit.kind(), Fill::Solid)
            };
            for x in [0, 7] {
                base.apply(BuildOp::Fill {
                    min: [x, 0, 0],
                    max: [x, y - 1, 0],
                    cube: Some(post),
                });
            }
            base.apply(BuildOp::Fill {
                min: [0, y, 0],
                max: [7, y, 0],
                cube: Some(Cube::new(kit.kind(), Fill::Solid)),
            });
            base
        };
        let kinds = |layout: &Layout| {
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for piece in &layout.pieces {
                let name = format!("{piece:?}");
                let name = name.split([' ', '(', '{']).next().unwrap_or("").to_string();
                *counts.entry(name).or_default() += 1;
            }
            counts
        };
        let (low, high) = (lay(&walkway(3), &kit), lay(&walkway(5), &kit));
        for (layout, y) in [(&low, 3), (&high, 5)] {
            // Logs lying along the walkway's own layer, as a wall's courses would.
            let band = (y as f32 * CELL_M)..=((y + 1) as f32 * CELL_M);
            let level: Vec<_> = logs(layout)
                .into_iter()
                .filter(|(from, to, _)| (from.y - to.y).abs() < 1e-4 && band.contains(&from.y))
                .collect();
            assert!(level.is_empty(), "logs laid along the walkway: {level:?}");
        }
        assert_eq!(kinds(&low), kinds(&high));
    }

    #[test]
    fn versions_come_from_places_and_are_the_same_on_every_machine() {
        // Pinned, so a change to the hash, which would move every log in every base, is
        // seen (D30).
        assert_eq!(hash(&[]), 0x7f4a_7c15);
        assert_eq!(
            [hash(&[0, 3, 7]), hash(&[1, 3, 7]), hash(&[0, 3, 8])],
            [0x85a8_47f3, 0x94d9_ad8c, 0xd1c1_928d]
        );
        let mut random = Random(42);
        let first: Vec<f32> = (0..3).map(|_| random.unit()).collect();
        assert!(first.iter().all(|u| (0.0..1.0).contains(u)));
        let mut again = Random(42);
        assert_eq!(first, (0..3).map(|_| again.unit()).collect::<Vec<_>>());
    }

    #[test]
    fn a_wall_being_carved_is_cut_and_framed_at_the_first_cut() {
        use crate::ops::{Context, Op};
        let rules = crate::Rules::default();
        let cx = Context::free(&rules);
        let mut base = cabin();
        let whole = lay(&base, &kit());
        let window = vec![[0, 2, 2], [0, 3, 2]];
        base.act(
            &Op::Carve {
                cubes: window.clone(),
            },
            &cx,
        )
        .unwrap();
        // Marked, nothing out yet: drawn as it was.
        assert_eq!(lay(&base, &kit()).pieces, whole.pieces);
        base.act(
            &Op::Empty {
                cubes: vec![[0, 3, 2]],
            },
            &cx,
        )
        .unwrap();
        let cut = lay(&base, &kit());
        assert!(boards(&cut) > boards(&whole), "framed");
        assert!(logs(&cut).len() > logs(&whole).len(), "its logs cut in two");
    }

    #[test]
    fn pieces_are_placed_in_whole_ticks_of_the_grid() {
        // A cube is so many ticks, and metres come back as they went in to within half a
        // tick; a place on the grid's lines is exact.
        assert_eq!(Place::near(Vec3::new(0.5, 3.0, -0.25)).0, [256, 1536, -128]);
        let point = Vec3::new(1.4213, 0.1337, 2.9707);
        let error = (Place::near(point).metres() - point).abs().max_element();
        assert!(error <= CELL_M / TICKS as f32 * 0.5, "{error}");
        assert_eq!(metres(ticks(0.125)), 0.125);
        // Every piece of a cabin is in the grid, with its version from its place.
        let mut base = cabin();
        wall(&mut base, [[3, 0, 5], [4, 0, 5]], Panel::Door);
        let layout = lay(&base, &textured());
        assert!(layout.pieces.iter().any(|p| matches!(p, Piece::Log { .. })));
    }

    #[test]
    fn a_base_laid_twice_is_laid_the_same() {
        let mut base = cabin();
        wall(&mut base, [[3, 0, 5], [4, 0, 5]], Panel::Door);
        let kit = textured();
        let (a, b) = (lay(&base, &kit), lay(&base, &kit));
        assert_eq!(a.pieces, b.pieces);
        assert!(!a.pieces.is_empty());
    }
}
