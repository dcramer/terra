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
//! - [`ground`]: door steps down to the ground.

pub mod blocks;
mod floors;
pub mod ground;
pub mod roofs;
pub mod runs;
mod trims;

use crate::kit::{Kit, Kits, Pattern};
use crate::mesh::Material;
use crate::{Base, CELL_M, Fill, Kind};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// Which half of a log is kept, where it is split along its length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq)]
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
    },
    /// A board round an opening: a jamb, a head or a sill.
    Board { min: Place, max: Place },
    /// A door or window set into an opening, filling the box from `min` to `max`, its face
    /// across the wall, which runs along X or along Z.
    Opening {
        min: Place,
        max: Place,
        along_x: bool,
        door: bool,
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
    /// A block kind's cube over the top of its wall, under a roof (`roofs::gables`): its
    /// surface cut off at the roof's underside, whose height in ticks over the cube's top,
    /// at its corners, the middles of its sides and its middle, west to east and north to
    /// south, is `under`.
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
            Piece::Block { cube } | Piece::Gable { cube, .. } => {
                let low = cube.as_vec3() * CELL_M;
                Some((low, low + Vec3::splat(CELL_M)))
            }
            Piece::Opening { .. } | Piece::Covering { .. } | Piece::Roof { .. } => None,
        }
    }

    /// The roof cell it goes up with, and in which stage, if it is part of a roof.
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
            Piece::Board { min, .. } | Piece::Opening { min, .. } => min.metres().y,
            Piece::Block { cube } | Piece::Gable { cube, .. } => cube.y as f32 * CELL_M,
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
}

impl Walls<'_> {
    /// A cube of the kit's kind that is part of a wall.
    fn ours(&self, at: IVec3) -> Option<Fill> {
        self.base
            .cube(at)
            .filter(|cube| cube.kind == self.kind && self.base.in_wall(at))
            .map(|cube| drawn_fill(&cube))
    }

    /// Whether a cube of ours runs along an axis: it has another of ours either side.
    fn runs(&self, at: IVec3, axis: Axis) -> bool {
        self.ours(at).is_some()
            && (self.ours(at + axis.step()).is_some() || self.ours(at - axis.step()).is_some())
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
        self.base.cube(at).is_none()
            && !self
                .base
                .contains(IVec3::new(at.x, crate::storey_of(at.y), at.z))
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
    let mut layout = lay_walls(base, kit, kits);
    roofs::lay(base, kit, kits, &mut layout.pieces);
    sort(&mut layout);
    layout
}

/// A kit's walls and their gables.
fn lay_walls(base: &Base, kit: &Kit, kits: &Kits) -> Layout {
    if kit.pattern == Pattern::Masonry {
        let mut layout = Layout {
            pieces: blocks::lay(base, kit),
        };
        roofs::gables(base, kit, kits, &mut layout.pieces);
        return layout;
    }
    let walls = Walls {
        base,
        kind: kit.kind(),
    };
    let ours: Vec<IVec3> = base
        .cubes()
        .map(|(at, _)| at)
        .filter(|&at| walls.ours(at).is_some())
        .collect();
    let mut layout = Layout::default();
    if ours.is_empty() {
        if kit.textures.contains_key(&Material::Floor) {
            floors::decks(base, kit, &mut layout);
            sort(&mut layout);
        }
        return layout;
    }
    let mut gabled = Vec::new();
    for axis in [Axis::X, Axis::Z] {
        // Each row of cubes running this way, with how far along it and how high its
        // cubes go.
        let mut rows: BTreeMap<i32, (i32, i32, i32, i32)> = BTreeMap::new();
        for &at in ours.iter().filter(|&&at| walls.runs(at, axis)) {
            let (row, along) = axis.row_of(at);
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
            gabled.push((axis, row, (first, last)));
        }
    }
    runs::posts(&walls, &ours, &mut layout);
    if kit.textures.contains_key(&Material::Floor) {
        floors::decks(base, kit, &mut layout);
    }
    let fields = roofs::Field::all(base, kits);
    runs::under_roofs(&fields, kit, &mut layout);
    // Gables after, as their logs are cut to the roof already.
    for (axis, row, ends) in gabled {
        runs::gables(&walls, kit, &fields, axis, row, ends, &mut layout);
    }
    layout
}

/// Bottom up, keeping walls together within a course; doors and windows last, fitted
/// into their openings once the rest is up, as they are not part of the plan.
fn sort(layout: &mut Layout) {
    layout.pieces.sort_by(|a, b| {
        let fitted = |p: &Piece| matches!(p, Piece::Opening { .. });
        fitted(a).cmp(&fitted(b)).then(
            (a.bottom() * 100.0)
                .round()
                .total_cmp(&(b.bottom() * 100.0).round()),
        )
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
    use crate::{BuildOp, Panel};

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
        };
        Kit {
            textures: BTreeMap::from([(Material::Floor, texture("floor"))]),
            ..kit()
        }
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
