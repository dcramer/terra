//! Log walls (docs/building-kits.md, D177, D178): each wall of log cubes laid as logs by the stacked
//! pattern, and each log made from the kit's numbers and a seed, so it looks hewn and
//! grown rather than turned.
//!
//! **The layout** is worked out from the cubes alone (D181). A row of cubes running east
//! and west lays whole courses, two to a cube; a row running north and south lies half a
//! course higher, with a split log at its foot and on top, so where two meet their logs
//! interlock without either knowing of the other. A cube with rows both ways is a corner,
//! a T or a cross, and its logs run through it to its outer face, no further, so nothing
//! leaves the grid; a row ending with nothing across it is cut flush. Openings, cubes cut open or glazed, cut the courses through
//! them, which end at a frame of boards. A cube with nothing beside it either way is a
//! post. Under a pitched roof the courses carry on up, shortening to the ridge. The
//! pieces come out in the order they go in: from the bottom up.
//!
//! **A log** is a lathe of rings along its length, each ring an uneven polygon, tapered,
//! bent and turned a little, with knots now and then and axe-cut ends showing their
//! grain. Every face takes one of the kit's colours.

#[cfg(test)]
use crate::DOOR_HEIGHT_M;
use crate::mesh::{Material, ROOF_THICK_M, Surfaces};
use crate::{
    Base, CELL_M, Fill, Kind, RoofStyle, STOREY_CUBES, STOREY_M, Surface, storey_of, wall_layer,
};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::f32::consts::TAU;

/// A kind of construction's kit: how its walls are laid, its sizes, how much each piece
/// varies, and its colours and textures, from a file in `assets/data/kits/` (D136).
/// Colours are as they should appear on screen (sRGB), named from the palette's roles
/// where there is one. What a file leaves out is pine's, as `logs.ron` has it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LogKit {
    /// The kind of construction it lays (D181): the number the cubes it lays carry.
    pub id: u16,
    /// How its walls are laid: as logs, or as masonry.
    pub pattern: Pattern,
    /// How thick a log is.
    pub diameter: f32,
    /// How far apart the courses are; less than the diameter, as logs are scribed to sit
    /// into one another. Must divide the storey.
    pub course: f32,
    /// How far a log runs past the corner cube it crosses at, beyond the grid. None, so
    /// that every piece stays inside its cubes and anything can be built against it.
    pub overhang: f32,
    /// The longest wall one log spans, the longest a tree gives (D174); the ends that run
    /// past corners come with it.
    pub longest: f32,
    /// How many facets a log's section has, from the first to the second.
    pub facets: (u32, u32),
    /// How much thicker one end is than the middle, and the other thinner, as a fraction.
    pub taper: f32,
    /// How far a log bows from straight over its length, in metres, at most.
    pub bend: f32,
    /// How uneven a log's section is, as a fraction of its radius.
    pub roughness: f32,
    /// Knots for every metre of log, on average.
    pub knots: f32,
    /// How far short of the others' a corner log's end may stop, in metres.
    pub ragged: f32,
    /// Whether there is daub in the groove between courses.
    pub chinking: bool,
    /// The boards round an opening: how wide and how far proud of the logs.
    pub frame: f32,
    /// How far a roof over its walls overhangs them, at the eaves and the gable ends (D175).
    pub eaves: f32,
    /// A block kind's surface: the radius its outside edges are rounded to, and how far
    /// its material's relief stands out (`surface.rs`).
    pub bevel: f32,
    pub relief: f32,
    /// The models its pieces are drawn with (D177), as asset paths; a piece with none is
    /// drawn in code, as a stand-in.
    #[serde(default)]
    pub models: KitModels,
    /// Its materials' textures (D14): where each is, how many metres across it is, and how
    /// deep its parallax reads; a material with none is drawn in its plain colour.
    #[serde(default)]
    pub textures: BTreeMap<Material, Texture>,
    pub colours: LogColours,
}

/// How a kit lays its walls (D160: a few patterns in code, kits as data).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Pattern {
    /// Courses of logs, crossing at the corners (this file).
    #[default]
    Logs,
    /// Blocks (`masonry.rs`): stone, brick, plaster or earth, drawn as the cubes' own
    /// surface, rounded and displaced by the kit's `Masonry` material (D197). Where it
    /// meets logs, it runs through and the logs end flush against it (D179).
    Masonry,
}

/// Every kit a base is laid by, one for each kind of construction.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kits(pub Vec<LogKit>);

impl Kits {
    /// The kit that lays a kind, if any does.
    pub fn of(&self, kind: Kind) -> Option<&LogKit> {
        self.0.iter().find(|kit| kit.kind() == kind)
    }
}

/// A material's textures: a folder under `assets` with `color.jpg`, `normal.jpg` and
/// `arm.jpg` (occlusion, roughness, metal), and `height.jpg` for parallax.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texture {
    pub path: String,
    /// How many metres across and down one copy of it covers.
    pub metres: [f32; 2],
    /// How deep its parallax reads, as a fraction of its size; none if not given.
    #[serde(default)]
    pub depth: f32,
}

/// The models of a kit's pieces, as asset paths (docs/building-kits.md). A list gives versions, one
/// picked for each place by its seed.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct KitModels {
    /// Logs of an everyday length, laid along X, stretched to fit.
    pub logs: Vec<String>,
    /// A long log, for a course longer than `long_from` metres.
    pub long_log: Option<String>,
    pub long_from: f32,
    /// A board round openings, laid along X.
    pub board: Option<String>,
    /// A door and a window in their frames, set into openings of their shape.
    pub door: Option<String>,
    pub window: Option<String>,
    /// Stretches of footing under walls on the ground, front to +Z.
    pub footing: Vec<String>,
    /// A panel of a roof's covering, its rows' lower edges to +Z; and a ridge cap along X.
    pub shakes: Option<String>,
    pub ridge: Option<String>,
    /// A panel of floor.
    pub floor: Option<String>,
}

/// The colours a log wall is drawn in.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogColours {
    /// The log's surface, mostly.
    pub wood: [f32; 3],
    /// Streaks of a lighter shade along it, and its weathered top.
    pub light: [f32; 3],
    /// Streaks of a darker shade.
    pub dark: [f32; 3],
    /// Bark left on in patches, and the ring round a cut end.
    pub bark: [f32; 3],
    /// The cut end's sapwood and its heart.
    pub end: [f32; 3],
    pub heart: [f32; 3],
    /// Knots.
    pub knot: [f32; 3],
    /// The daub between courses.
    pub chinking: [f32; 3],
    /// Boards round openings, and behind a gable.
    pub board: [f32; 3],
    /// The roof over log walls, until roofs have kits of their own.
    pub roof: [f32; 3],
}

impl Default for LogKit {
    /// Pine, as `assets/data/kits/logs.ron` has it; for tests.
    fn default() -> Self {
        LogKit {
            id: 1,
            pattern: Pattern::Logs,
            diameter: 0.3,
            course: 0.25,
            overhang: 0.0,
            longest: 4.0,
            facets: (9, 11),
            taper: 0.07,
            bend: 0.035,
            roughness: 0.07,
            knots: 0.5,
            ragged: 0.04,
            chinking: true,
            frame: 0.08,
            eaves: 0.5,
            bevel: 0.04,
            relief: 0.05,
            models: KitModels::default(),
            textures: BTreeMap::new(),
            colours: LogColours {
                wood: [0.62, 0.43, 0.26],
                light: [0.74, 0.56, 0.37],
                dark: [0.5, 0.34, 0.22],
                bark: [0.37, 0.26, 0.19],
                end: [0.8, 0.64, 0.44],
                heart: [0.66, 0.47, 0.3],
                knot: [0.33, 0.22, 0.15],
                chinking: [0.66, 0.6, 0.5],
                board: [0.45, 0.32, 0.22],
                roof: [0.47, 0.42, 0.37],
            },
        }
    }
}

impl LogKit {
    /// How many courses fill a storey, and whether they fill it exactly.
    pub fn courses(&self) -> (i32, bool) {
        let count = (STOREY_M / self.course).round();
        (count as i32, (count * self.course - STOREY_M).abs() < 1e-3)
    }

    /// The kind of construction it lays.
    pub fn kind(&self) -> Kind {
        Kind(self.id)
    }

    fn radius(&self) -> f32 {
        self.diameter * 0.5
    }
}

/// How finely a gable's logs are fitted under its roof, in metres along the wall.
const GABLE_STEP_M: f32 = 0.02;

/// Which half of a log is kept, where it is split along its length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Split {
    Whole,
    /// The top half, its flat side down: the first course of a wall.
    Top,
    /// The bottom half, its flat side up: the last.
    Bottom,
}

/// One piece of a wall, in the base's own space, in metres.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    /// A log from one end to the other along its middle.
    Log {
        from: Vec3,
        to: Vec3,
        split: Split,
        seed: u32,
        /// The daub under it, as far along it from `from` as from the first number to
        /// the second, if any.
        chinking: Option<(f32, f32)>,
    },
    /// A board round an opening: a jamb, a head or a sill.
    Board { min: Vec3, max: Vec3 },
    /// A door or window set into an opening, filling the box from `min` to `max`, its face
    /// across the wall, which runs along X or along Z.
    Opening {
        min: Vec3,
        max: Vec3,
        along_x: bool,
        door: bool,
    },
    /// A footing along the foot of a wall on the ground, its top along the line from one
    /// end to the other.
    Footing { from: Vec3, to: Vec3, seed: u32 },
    /// A panel of a roof's covering, by its corners: along its lower edge, then its upper.
    Shakes { corners: [Vec3; 4] },
    /// A ridge cap along a roof's ridge, on it.
    Ridge { from: Vec3, to: Vec3 },
    /// A panel of floor, its top from one corner to the other.
    Floor { min: Vec3, max: Vec3 },
    /// A block kind's cube (`masonry.rs`), drawn as part of its kind's surface
    /// (`surface.rs`), counted in cubes.
    Block { cube: IVec3 },
}

impl Piece {
    /// The box it fills in a wall or floor, if it stands in the base's cubes: not a door
    /// or window, which is not part of the plan, nor a roof's covering or a footing in the
    /// ground.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        match *self {
            Piece::Log { from, to, .. } => {
                let r = Vec3::splat(0.15);
                Some((from.min(to) - r, from.max(to) + r))
            }
            Piece::Board { min, max } | Piece::Floor { min, max } => Some((min, max)),
            Piece::Block { cube } => {
                let low = cube.as_vec3() * CELL_M;
                Some((low, low + Vec3::splat(CELL_M)))
            }
            Piece::Opening { .. }
            | Piece::Footing { .. }
            | Piece::Shakes { .. }
            | Piece::Ridge { .. } => None,
        }
    }

    /// The lowest point of it, which orders the pieces as they go in.
    fn bottom(&self) -> f32 {
        match self {
            Piece::Log { from, to, .. } | Piece::Ridge { from, to } => from.y.min(to.y),
            Piece::Footing { from, to, .. } => from.y.min(to.y) - 1.0,
            Piece::Board { min, .. } | Piece::Opening { min, .. } => min.y,
            Piece::Floor { min, .. } => min.y - 0.5,
            Piece::Shakes { corners } => corners.iter().map(|c| c.y).fold(f32::MAX, f32::min),
            Piece::Block { cube } => cube.y as f32 * CELL_M,
        }
    }
}

/// A base's log walls as pieces.
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

    /// A point so far along the middle of a row, at a height, in metres.
    fn point(self, row: i32, along: f32, height: f32) -> Vec3 {
        let middle = (row as f32 + 0.5) * CELL_M;
        match self {
            Axis::X => Vec3::new(along, height, middle),
            Axis::Z => Vec3::new(middle, height, along),
        }
    }

    /// The height of a course's middle: the walls one way lie half a course above the
    /// other's.
    fn centre(self, kit: &LogKit, course: i32) -> f32 {
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
            .map(|cube| cube.fill)
    }

    /// Whether a cube of ours runs along an axis: it has another of ours either side.
    fn runs(&self, at: IVec3, axis: Axis) -> bool {
        self.ours(at).is_some()
            && (self.ours(at + axis.step()).is_some() || self.ours(at - axis.step()).is_some())
    }

    /// What a course is at a cube of a row: a log where the cubes it lies in are solid and
    /// run its way, split where it is at the foot or top of them, cut where one is open.
    fn spot(&self, kit: &LogKit, axis: Axis, row: i32, along: i32, course: i32) -> Spot {
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

/// Lays every wall of a base in the kit's kind as logs.
pub fn lay(base: &Base, kit: &LogKit) -> Layout {
    if kit.pattern == Pattern::Masonry {
        let mut layout = Layout {
            pieces: crate::masonry::lay(base, kit),
        };
        sort(&mut layout);
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
        return layout;
    }
    for axis in [Axis::X, Axis::Z] {
        // Each row of cubes running this way, with how far along it and how high its
        // cubes go.
        let mut rows: BTreeMap<i32, (i32, i32, i32, i32)> = BTreeMap::new();
        for &at in ours.iter().filter(|&&at| walls.runs(at, axis)) {
            let (row, along) = match axis {
                Axis::X => (at.z, at.x),
                Axis::Z => (at.x, at.z),
            };
            let entry = rows.entry(row).or_insert((along, along, at.y, at.y));
            *entry = (
                entry.0.min(along),
                entry.1.max(along),
                entry.2.min(at.y),
                entry.3.max(at.y),
            );
        }
        for (row, (first, last, lowest, highest)) in rows {
            courses(
                &walls,
                kit,
                axis,
                row,
                (first, last),
                (lowest, highest),
                &mut layout,
            );
            frames(&walls, kit, axis, row, &mut layout);
            gables(&walls, kit, axis, row, (first, last), &mut layout);
        }
    }
    posts(&walls, &ours, &mut layout);
    if !kit.models.footing.is_empty() || kit.textures.contains_key(&Material::Stone) {
        footings(&walls, &ours, &mut layout);
    }
    if kit.models.shakes.is_some() {
        coverings(&walls, kit, &mut layout);
    }
    if kit.models.floor.is_some() || kit.textures.contains_key(&Material::Floor) {
        floors(&walls, &mut layout);
    }
    under_roofs(base, kit, &mut layout);
    sort(&mut layout);
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

/// The courses of one row, each split into logs where it runs unbroken.
fn courses(
    walls: &Walls,
    kit: &LogKit,
    axis: Axis,
    row: i32,
    (first, last): (i32, i32),
    (lowest, highest): (i32, i32),
    layout: &mut Layout,
) {
    let per_cube = (CELL_M / kit.course).round() as i32;
    for course in lowest * per_cube - 1..=(highest + 1) * per_cube + 1 {
        let spots: Vec<Spot> = (first..=last)
            .map(|along| walls.spot(kit, axis, row, along, course))
            .collect();
        let centre = axis.centre(kit, course);
        let seed = hash(&[axis as u32, row as u32, course as u32]);
        let mut index = 0;
        while index < spots.len() {
            let Spot::Log(split) = spots[index] else {
                index += 1;
                continue;
            };
            let start = index;
            while index + 1 < spots.len() && spots[index + 1] == spots[start] {
                index += 1;
            }
            let (low, high) = (first + start as i32, first + index as i32);
            index += 1;
            // How each end finishes: cut at an opening's frame, run past a corner into the
            // open air, or flush with the cube.
            let layer = ((centre + 1e-3) / CELL_M).floor() as i32;
            let reach = |end: i32, outward: i32, which: u32| -> f32 {
                let beyond = end + outward;
                let next = if (first..=last).contains(&beyond) {
                    spots[(beyond - first) as usize]
                } else {
                    Spot::Nothing
                };
                let cube = axis.cube(row, end, layer.max(layer_of_split(centre, split)));
                let corner = walls.runs(cube, axis.other());
                let open = walls.open_beyond(axis.cube(row, beyond, cube.y));
                match next {
                    Spot::Hole => -kit.frame,
                    // Uneven, but never past the overhang: short of it by up to `ragged`.
                    Spot::Nothing if corner && open => {
                        kit.overhang - unit(hash(&[seed, end as u32, which])) * kit.ragged
                    }
                    _ => 0.0,
                }
            };
            let from = low as f32 * CELL_M - reach(low, -1, 0);
            let to = (high + 1) as f32 * CELL_M + reach(high, 1, 1);
            // Daub between this course and the one under it, along the wall itself.
            let below = walls.spot(kit, axis, row, low, course - 1);
            let daub = kit.chinking && split != Split::Top && matches!(below, Spot::Log(_));
            let (wall_from, wall_to) = (low as f32 * CELL_M, (high + 1) as f32 * CELL_M);
            let limit = kit.longest + 2.0 * (kit.overhang + kit.ragged);
            for (part, (a, b)) in joined(from, to, limit, course).into_iter().enumerate() {
                let chinking = daub.then(|| (wall_from.max(a) - a, wall_to.min(b) - a));
                layout.pieces.push(Piece::Log {
                    from: axis.point(row, a, centre),
                    to: axis.point(row, b, centre),
                    split,
                    seed: hash(&[seed, part as u32, (a * 100.0) as i32 as u32]),
                    chinking: chinking.filter(|(near, far)| far > near),
                });
            }
        }
    }
}

/// The layer a split log's cubes are on: the one its kept half lies in.
fn layer_of_split(centre: f32, split: Split) -> i32 {
    let layer = |height: f32| (height / CELL_M).floor() as i32;
    match split {
        Split::Top => layer(centre + 1e-3),
        Split::Bottom => layer(centre - 1e-3),
        Split::Whole => layer(centre + 1e-3),
    }
}

/// A frame of boards round every opening in a row, of whatever shape: a board along each
/// edge where an open or glass cube meets the wall, edges in a line one board. Up the
/// sides they are jambs; over the top a head, reaching up into the first log above; under
/// it a sill, unless the opening is at the foot of its wall, as a doorway is.
fn frames(walls: &Walls, kit: &LogKit, axis: Axis, row: i32, layout: &mut Layout) {
    let mut holes: BTreeSet<(i32, i32)> = BTreeSet::new();
    for (at, cube) in walls.base.cubes() {
        let (on_row, along) = match axis {
            Axis::X => (at.z, at.x),
            Axis::Z => (at.x, at.z),
        };
        if on_row == row && cube.fill != Fill::Solid && walls.runs(at, axis) {
            holes.insert((along, at.y));
        }
    }
    // A door or window the kit has a model of fills its opening, when the opening is the
    // shape of one: a rectangle, at the foot of its wall for a door, above it for a window.
    let mut framed: BTreeSet<(i32, i32)> = BTreeSet::new();
    for region in regions(&holes) {
        let (low, high) = region.iter().fold(
            ((i32::MAX, i32::MAX), (i32::MIN, i32::MIN)),
            |(lo, hi), &(a, l)| ((lo.0.min(a), lo.1.min(l)), (hi.0.max(a), hi.1.max(l))),
        );
        let whole = region.len() as i32 == (high.0 - low.0 + 1) * (high.1 - low.1 + 1);
        let at_foot = walls.ours(axis.cube(row, low.0, low.1 - 1)).is_none();
        let glazed = region
            .iter()
            .any(|&(a, l)| walls.ours(axis.cube(row, a, l)) == Some(Fill::Glass));
        let modelled = whole
            && if at_foot && !glazed {
                kit.models.door.is_some()
            } else {
                !at_foot && glazed && kit.models.window.is_some()
            };
        if !modelled {
            framed.extend(region);
            continue;
        }
        let depth = kit.radius() + 0.02;
        let (start, end) = (
            low.0 as f32 * CELL_M - kit.frame,
            (high.0 + 1) as f32 * CELL_M + kit.frame,
        );
        let bottom = low.1 as f32 * CELL_M - if at_foot { 0.0 } else { kit.frame };
        let top = (high.1 + 1) as f32 * CELL_M + kit.frame;
        let out = match axis {
            Axis::X => Vec3::Z,
            Axis::Z => Vec3::X,
        } * depth;
        let (a, b) = (
            axis.point(row, start, bottom) - out,
            axis.point(row, end, top) + out,
        );
        layout.pieces.push(Piece::Opening {
            min: a.min(b),
            max: a.max(b),
            along_x: axis == Axis::X,
            door: at_foot,
        });
    }
    let holes = framed;
    let solid =
        |along: i32, layer: i32| walls.ours(axis.cube(row, along, layer)) == Some(Fill::Solid);
    let per_cube = (CELL_M / kit.course).round() as i32;
    let depth = kit.radius() + 0.02;
    let across = match axis {
        Axis::X => Vec3::Z,
        Axis::Z => Vec3::X,
    };
    let mut board = |a: f32, b: f32, bottom: f32, top: f32, proud: f32| {
        let (low, high) = (axis.point(row, a, bottom), axis.point(row, b, top));
        let out = across * (depth + proud);
        layout.pieces.push(Piece::Board {
            min: (low - out).min(high + out),
            max: (low - out).max(high + out),
        });
    };
    // Up each side: a jamb along every run of hole cubes with wall beside them.
    for outward in [-1, 1] {
        let edges: BTreeSet<(i32, i32)> = holes
            .iter()
            .copied()
            .filter(|&(along, layer)| solid(along + outward, layer))
            .collect();
        for (along, low, high) in runs_of(&edges, |(along, layer)| (along, layer)) {
            let face = (along + i32::from(outward > 0)) as f32 * CELL_M;
            let (a, b) = if outward > 0 {
                (face, face + kit.frame)
            } else {
                (face - kit.frame, face)
            };
            board(a, b, low as f32 * CELL_M, (high + 1) as f32 * CELL_M, 0.0);
        }
    }
    // Over the top and under the bottom: a head and a sill along every run of hole cubes
    // with wall over or under them.
    for upward in [1, -1] {
        let edges: BTreeSet<(i32, i32)> = holes
            .iter()
            .copied()
            .filter(|&(along, layer)| solid(along, layer + upward))
            .collect();
        for (layer, low, high) in runs_of(&edges, |(along, layer)| (layer, along)) {
            let (a, b) = (
                low as f32 * CELL_M - kit.frame,
                (high + 1) as f32 * CELL_M + kit.frame,
            );
            let near = layer * per_cube;
            if upward > 0 {
                let edge = (layer + 1) as f32 * CELL_M;
                // Up to the middle of the first course wholly above it.
                let head = (near - 2..near + 2 * per_cube + 2)
                    .map(|course| axis.centre(kit, course))
                    .find(|&centre| centre - kit.course * 0.5 >= edge - 1e-3)
                    .map_or(edge + 0.1, |centre| centre.max(edge + 0.06));
                board(a, b, edge, head, 0.01);
            } else {
                let edge = layer as f32 * CELL_M;
                let foot = (near - 2 * per_cube - 2..near + 2)
                    .rev()
                    .map(|course| axis.centre(kit, course))
                    .find(|&centre| centre + kit.course * 0.5 <= edge + 1e-3)
                    .map_or(edge - 0.1, |centre| centre.min(edge - 0.06));
                board(a, b, foot, edge, 0.03);
            }
        }
    }
}

/// The groups of cells that touch, each in order.
fn regions(cells: &BTreeSet<(i32, i32)>) -> Vec<Vec<(i32, i32)>> {
    let mut left = cells.clone();
    let mut found = Vec::new();
    while let Some(&start) = left.iter().next() {
        left.remove(&start);
        let mut group = vec![start];
        let mut frontier = vec![start];
        while let Some((a, l)) = frontier.pop() {
            for next in [(a + 1, l), (a - 1, l), (a, l + 1), (a, l - 1)] {
                if left.remove(&next) {
                    group.push(next);
                    frontier.push(next);
                }
            }
        }
        group.sort();
        found.push(group);
    }
    found
}

/// A footing under every straight run of wall that stands on the ground: the cubes of the
/// first layer, with nothing of the wall under them.
fn footings(walls: &Walls, ours: &[IVec3], layout: &mut Layout) {
    for axis in [Axis::X, Axis::Z] {
        let mut rows: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();
        for &at in ours.iter().filter(|at| at.y == 0) {
            let alone = !walls.runs(at, Axis::X) && !walls.runs(at, Axis::Z);
            if walls.runs(at, axis) || (alone && axis == Axis::X) {
                let (row, along) = match axis {
                    Axis::X => (at.z, at.x),
                    Axis::Z => (at.x, at.z),
                };
                rows.entry(row).or_default().insert(along);
            }
        }
        for (row, cells) in rows {
            let keyed: BTreeSet<(i32, i32)> = cells.iter().map(|&a| (0, a)).collect();
            for (_, first, last) in runs_of(&keyed, |cell| cell) {
                let reach = 0.05;
                layout.pieces.push(Piece::Footing {
                    from: axis.point(row, first as f32 * CELL_M - reach, 0.0),
                    to: axis.point(row, (last + 1) as f32 * CELL_M + reach, 0.0),
                    seed: hash(&[axis as u32, row as u32, first as u32]),
                });
            }
        }
    }
}

/// How large a panel of roof covering or floor is, about.
const PANEL_M: f32 = 1.0;

/// The covering of each gable and lean-to roof over walls of the kit's kind: each face,
/// eaves and all, in panels of about a metre, as many as fit it exactly; and a ridge cap
/// along a gable's ridge. A hip's faces are not rectangles, and are left to the plain roof.
fn coverings(walls: &Walls, kit: &LogKit, layout: &mut Layout) {
    for section in walls.base.sections() {
        if walls.base.kind(IVec3::from_array(section.min)) != Some(walls.kind) {
            continue;
        }
        let (low, high) = section.bounds();
        let eaves = (section.storey() + 1) as f32 * STOREY_M;
        let e = kit.eaves;
        let along_x = section.along_x;
        // Along the ridge and across it, as points.
        let point = |along: f32, across: f32, height: f32| {
            if along_x {
                Vec3::new(along, height, across)
            } else {
                Vec3::new(across, height, along)
            }
        };
        let (start, end) = if along_x {
            (low.x - e, high.x + e)
        } else {
            (low.y - e, high.y + e)
        };
        let (near, far) = if along_x {
            (low.y, high.y)
        } else {
            (low.x, high.x)
        };
        let height = |across: f32, from_eave: f32| eaves + from_eave * section.pitch + 0.0 * across;
        // Each face: its eave's edge across the ridge, and where it rises to.
        let faces: Vec<(f32, f32)> = match section.style {
            RoofStyle::Gable => {
                let middle = (near + far) * 0.5;
                vec![(near - e, middle), (far + e, middle)]
            }
            RoofStyle::LeanTo => vec![(near - e, far + e)],
            RoofStyle::Hip => continue,
        };
        for (eave, ridge) in faces {
            let toward = (ridge - eave).signum();
            let run = (ridge - eave).abs();
            let slope = run * (1.0 + section.pitch * section.pitch).sqrt();
            let (rows, columns) = (
                (slope / PANEL_M).round().max(1.0) as i32,
                ((end - start) / PANEL_M).round().max(1.0) as i32,
            );
            let at = |column: i32, row: i32| {
                let along = start + (end - start) * column as f32 / columns as f32;
                let across = eave + toward * run * row as f32 / rows as f32;
                let from_eave = run * row as f32 / rows as f32 - e;
                point(along, across, height(across, from_eave))
            };
            for row in 0..rows {
                for column in 0..columns {
                    layout.pieces.push(Piece::Shakes {
                        corners: [
                            at(column, row),
                            at(column + 1, row),
                            at(column + 1, row + 1),
                            at(column, row + 1),
                        ],
                    });
                }
            }
        }
        if section.style == RoofStyle::Gable {
            let middle = (near + far) * 0.5;
            let top = eaves + (far - near) * 0.5 * section.pitch;
            layout.pieces.push(Piece::Ridge {
                from: point(start, middle, top),
                to: point(end, middle, top),
            });
        }
    }
}

/// The floors of rooms in the kit's kind, in panels: two tiles square where four make one,
/// else a tile.
fn floors(walls: &Walls, layout: &mut Layout) {
    let ours = |tile: IVec3| {
        walls.base.contains(tile)
            && walls
                .base
                .cube(IVec3::new(tile.x, crate::floor_layer(tile.y), tile.z))
                .is_some_and(|cube| cube.kind == walls.kind)
    };
    let mut done: BTreeSet<[i32; 3]> = BTreeSet::new();
    for (tile, _) in walls.base.tiles() {
        if done.contains(&tile.to_array()) || !ours(tile) {
            continue;
        }
        let block = [IVec3::ZERO, IVec3::X, IVec3::Z, IVec3::new(1, 0, 1)];
        let size = if block
            .iter()
            .all(|step| ours(tile + *step) && !done.contains(&(tile + *step).to_array()))
        {
            2
        } else {
            1
        };
        for x in 0..size {
            for z in 0..size {
                done.insert((tile + IVec3::new(x, 0, z)).to_array());
            }
        }
        let y = tile.y as f32 * STOREY_M;
        let low = Vec3::new(tile.x as f32 * CELL_M, y, tile.z as f32 * CELL_M);
        layout.pieces.push(Piece::Floor {
            min: low,
            max: low + Vec3::new(size as f32 * CELL_M, 0.0, size as f32 * CELL_M),
        });
    }
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

/// Under a pitched roof, a row's courses carry on up, each log as long as fits under it,
/// so they shorten toward the ridge as a gable's do.
fn gables(
    walls: &Walls,
    kit: &LogKit,
    axis: Axis,
    row: i32,
    (first, last): (i32, i32),
    layout: &mut Layout,
) {
    let radius = kit.radius();
    // Each storey whose walls' top layer this row reaches.
    let tops: BTreeSet<i32> = walls
        .base
        .cubes()
        .map(|(at, _)| at)
        .filter(|&at| {
            let on_row = match axis {
                Axis::X => at.z,
                Axis::Z => at.x,
            };
            on_row == row
                && at.y - wall_layer(storey_of(at.y)) == STOREY_CUBES - 1
                && walls.runs(at, axis)
        })
        .map(|at| storey_of(at.y))
        .collect();
    for storey in tops {
        let eaves = (storey + 1) as f32 * STOREY_M;
        let rise = |distance: f32| {
            let point = axis.point(row, distance, 0.0);
            walls
                .base
                .roof_rise(storey, glam::Vec2::new(point.x, point.z))
                .map(|rise| rise - ROOF_THICK_M)
        };
        let (from, to) = (first as f32 * CELL_M, (last + 1) as f32 * CELL_M);
        for step in 0.. {
            let centre = kit.course * (step as f32 + 0.5);
            let needed = centre + radius * 0.9;
            let mut spans: Vec<(f32, f32)> = Vec::new();
            let mut open: Option<f32> = None;
            let samples = ((to - from) / GABLE_STEP_M).round() as i32;
            for sample in 0..=samples {
                let distance = from + sample as f32 * GABLE_STEP_M;
                let fits = rise(distance).is_some_and(|rise| rise >= needed);
                match (fits, open) {
                    (true, None) => open = Some(distance),
                    (false, Some(start)) => {
                        spans.push((start, distance - GABLE_STEP_M));
                        open = None;
                    }
                    _ => {}
                }
            }
            if let Some(start) = open {
                spans.push((start, to));
            }
            spans.retain(|(a, b)| b - a > 0.15);
            if spans.is_empty() {
                break;
            }
            let seed = hash(&[axis as u32, row as u32, storey as u32, 100 + step as u32]);
            for (index, (a, b)) in spans.into_iter().enumerate() {
                layout.pieces.push(Piece::Log {
                    from: axis.point(row, a, eaves + centre),
                    to: axis.point(row, b, eaves + centre),
                    split: Split::Whole,
                    seed: hash(&[seed, index as u32]),
                    chinking: kit.chinking.then_some((0.0, b - a)),
                });
            }
        }
    }
}

/// Cuts back every log that would pass through a roof, its eaves too: the ends of the top
/// courses that run past a corner, under a hip's slope, end where the roof begins, as a
/// builder saws them. A log the roof cuts in the middle is left in two.
fn under_roofs(base: &Base, kit: &LogKit, layout: &mut Layout) {
    let radius = kit.radius() + kit.ragged;
    // Where the slab of a roof is at a point, its underside and its top, if one is there.
    let slab = |at: Vec3| {
        base.sections().iter().find_map(|section| {
            let eaves = (section.storey() + 1) as f32 * STOREY_M;
            let rise = section.rise_beyond(glam::Vec2::new(at.x, at.z), kit.eaves)?;
            let top = eaves + rise;
            (at.y + radius > top - ROOF_THICK_M && at.y - radius < top).then_some(())
        })
    };
    let pieces = std::mem::take(&mut layout.pieces);
    for piece in pieces {
        let Piece::Log {
            from,
            to,
            split,
            seed,
            chinking,
        } = piece
        else {
            layout.pieces.push(piece);
            continue;
        };
        let length = from.distance(to);
        if base.sections().is_empty() || from.y != to.y || length < 1e-3 {
            layout.pieces.push(piece);
            continue;
        }
        let along = (to - from) / length;
        let samples = (length / GABLE_STEP_M).ceil() as i32;
        let clear: Vec<bool> = (0..=samples)
            .map(|i| slab(from + along * (length * i as f32 / samples as f32)).is_none())
            .collect();
        if clear.iter().all(|&c| c) {
            layout.pieces.push(piece);
            continue;
        }
        // The runs of it that are clear of every roof.
        let mut start = None;
        let mut spans = Vec::new();
        for (i, &c) in clear.iter().enumerate() {
            let at = length * i as f32 / samples as f32;
            match (c, start) {
                (true, None) => start = Some(at),
                (false, Some(a)) => {
                    spans.push((a, at - length / samples as f32));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(a) = start {
            spans.push((a, length));
        }
        for (index, (a, b)) in spans.into_iter().enumerate() {
            if b - a < 0.15 {
                continue;
            }
            layout.pieces.push(Piece::Log {
                from: from + along * a,
                to: from + along * b,
                split,
                seed: if index == 0 {
                    seed
                } else {
                    hash(&[seed, index as u32])
                },
                chinking: chinking.and_then(|(c0, c1)| {
                    let (c0, c1) = (c0.max(a) - a, c1.min(b) - a);
                    (c1 > c0).then_some((c0, c1))
                }),
            });
        }
    }
}

/// A cube of ours with none beside it either way is a post: an upright log up each
/// stack of them.
fn posts(walls: &Walls, ours: &[IVec3], layout: &mut Layout) {
    let alone = |at: IVec3| {
        walls.ours(at) == Some(Fill::Solid) && !walls.runs(at, Axis::X) && !walls.runs(at, Axis::Z)
    };
    for &at in ours {
        // From the bottom of each stack.
        if !alone(at) || alone(at - IVec3::Y) {
            continue;
        }
        let mut top = at;
        while alone(top + IVec3::Y) {
            top += IVec3::Y;
        }
        let middle = |layer: i32| {
            Vec3::new(
                (at.x as f32 + 0.5) * CELL_M,
                layer as f32 * CELL_M,
                (at.z as f32 + 0.5) * CELL_M,
            )
        };
        layout.pieces.push(Piece::Log {
            from: middle(at.y),
            to: middle(top.y + 1),
            split: Split::Whole,
            seed: hash(&[at.x as u32, at.y as u32, at.z as u32, 7]),
            chinking: None,
        });
    }
}

/// A stretch of a course in logs no longer than `longest`, the joins staggered from one
/// course to the next so no seam runs up the wall.
fn joined(start: f32, end: f32, longest: f32, course: i32) -> Vec<(f32, f32)> {
    let length = end - start;
    if length <= longest {
        return vec![(start, end)];
    }
    let count = (length / longest).ceil();
    let each = length / count;
    // Every other course moves its joins along by half a log, with a short log at each end.
    let shift = if course.rem_euclid(2) == 1 {
        each * 0.5
    } else {
        0.0
    };
    let mut cuts = vec![start];
    let mut at = start + if shift > 0.0 { shift } else { each };
    while at < end - 0.3 {
        cuts.push(at);
        at += each;
    }
    cuts.push(end);
    cuts.windows(2).map(|pair| (pair[0], pair[1])).collect()
}

/// The size in metres of a kit's texture for a material, or a metre square if it has none.
fn metres(kit: &LogKit, material: Material) -> [f32; 2] {
    kit.textures
        .get(&material)
        .map_or([1.0, 1.0], |texture| texture.metres)
}

/// Draws a piece, each part in its material.
pub fn draw(piece: &Piece, kit: &LogKit, surfaces: &mut Surfaces) {
    match *piece {
        Piece::Log {
            from,
            to,
            split,
            seed,
            ..
        } => {
            log(surfaces, kit, from, to, split, seed);
            draw_chinking(piece, kit, surfaces);
        }
        Piece::Board { min, max } => cuboid(
            surfaces.of(Material::Board),
            min,
            max,
            metres(kit, Material::Board),
            [1.0; 3],
        ),
        Piece::Footing { from, to, .. } => {
            // A wall of stone under the wall's line, its top a little over the foot.
            let along = (to - from).normalize_or_zero();
            let across = Vec3::Y.cross(along) * FOOTING_DEEP_M * 0.5;
            let low = from - across - Vec3::Y * (FOOTING_HIGH_M - 0.02);
            let high = to + across + Vec3::Y * 0.02;
            cuboid(
                surfaces.of(Material::Stone),
                low.min(high),
                low.max(high),
                metres(kit, Material::Stone),
                [1.0; 3],
            );
        }
        Piece::Floor { min, max } => cuboid(
            surfaces.of(Material::Floor),
            min - Vec3::Y * 0.03,
            max + Vec3::Y * 0.005,
            metres(kit, Material::Floor),
            [1.0; 3],
        ),
        // Laid from a model.
        Piece::Opening { .. }
        | Piece::Shakes { .. }
        | Piece::Ridge { .. }
        | Piece::Block { .. } => {}
    }
}

/// How tall and deep a footing of stone is, its top at the wall's foot.
const FOOTING_HIGH_M: f32 = 0.5;
const FOOTING_DEEP_M: f32 = 0.6;

/// The daub under a log, alone.
pub fn draw_chinking(piece: &Piece, kit: &LogKit, surfaces: &mut Surfaces) {
    if let Piece::Log {
        from,
        to,
        chinking: Some((near, far)),
        ..
    } = *piece
    {
        let along = (to - from).normalize_or_zero();
        let depth = Vec3::Y.cross(along) * kit.radius() * 0.5;
        let start = from + along * near;
        let end = from + along * far;
        let gap = kit.course * 0.5;
        let low = start - depth - Vec3::Y * (gap + 0.018);
        let high = end + depth - Vec3::Y * (gap - 0.018);
        cuboid(
            surfaces.of(Material::Daub),
            low.min(high),
            low.max(high),
            metres(kit, Material::Daub),
            kit.colours.chinking,
        );
    }
}

/// A box from one corner to the other, each face's texture mapped from its plane.
fn cuboid(surface: &mut Surface, min: Vec3, max: Vec3, metres: [f32; 2], colour: [f32; 3]) {
    let corner = |x: bool, y: bool, z: bool| {
        Vec3::new(
            if x { max.x } else { min.x },
            if y { max.y } else { min.y },
            if z { max.z } else { min.z },
        )
    };
    let faces = [
        (
            Vec3::X,
            [
                (true, false, true),
                (true, false, false),
                (true, true, false),
                (true, true, true),
            ],
        ),
        (
            Vec3::NEG_X,
            [
                (false, false, false),
                (false, false, true),
                (false, true, true),
                (false, true, false),
            ],
        ),
        (
            Vec3::Y,
            [
                (false, true, true),
                (true, true, true),
                (true, true, false),
                (false, true, false),
            ],
        ),
        (
            Vec3::NEG_Y,
            [
                (false, false, false),
                (true, false, false),
                (true, false, true),
                (false, false, true),
            ],
        ),
        (
            Vec3::Z,
            [
                (false, false, true),
                (true, false, true),
                (true, true, true),
                (false, true, true),
            ],
        ),
        (
            Vec3::NEG_Z,
            [
                (true, false, false),
                (false, false, false),
                (false, true, false),
                (true, true, false),
            ],
        ),
    ];
    for (normal, corners) in faces {
        let corners = corners.map(|(x, y, z)| corner(x, y, z));
        surface.planar_quad(corners, normal, metres, colour);
    }
}

/// How far each ring of a log is from its start, as a fraction of its length: close
/// together at the ends, where the axe's chamfer is, and every two thirds of a metre or so
/// between, enough for its bow.
fn stations(length: f32) -> Vec<f32> {
    let chamfer = (0.035 / length).min(0.2);
    let middle = ((length / 0.7).ceil() as usize).max(1);
    let mut at = vec![0.0, chamfer];
    for step in 1..middle {
        at.push(chamfer + (1.0 - 2.0 * chamfer) * step as f32 / middle as f32);
    }
    at.extend([1.0 - chamfer, 1.0]);
    at
}

/// One log: round, a little uneven, tapered and bowed in one sweep, shaded smooth round it,
/// in the kit's wood, its texture placed anew on each log by its seed so no two repeat;
/// its sawn ends in the kit's cut face.
fn log(surfaces: &mut Surfaces, kit: &LogKit, from: Vec3, to: Vec3, split: Split, seed: u32) {
    let length = from.distance(to);
    if length < 1e-3 {
        return;
    }
    let mut random = Random(seed);
    let axis = (to - from) / length;
    let up = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let side = axis.cross(up).normalize();
    let up = side.cross(axis);
    let radius = kit.radius();

    // The section: round, a little uneven, the same shape all along.
    let (fewest, most) = kit.facets;
    let facets = (fewest + random.below(most.saturating_sub(fewest) + 1)) as usize;
    let turn = random.unit() * TAU;
    let section: Vec<(f32, f32)> = (0..facets)
        .map(|i| {
            let angle = turn + TAU * (i as f32 + (random.unit() - 0.5) * 0.2) / facets as f32;
            let out = 1.0 + (random.unit() - 0.5) * 2.0 * kit.roughness;
            (angle, out)
        })
        .collect();

    // A tree's shape along it: thicker at one end and bowed, in one sweep.
    let thick_first = random.unit() < 0.5;
    let bow = (random.unit() - 0.5) * 2.0 * kit.bend;
    let sag = (random.unit() - 0.5) * kit.bend;
    let at = stations(length);
    let last = at.len() - 1;
    let centres: Vec<Vec3> = at
        .iter()
        .map(|&t| {
            let arc = libm::sinf(std::f32::consts::PI * t);
            from + axis * (length * t) + side * (bow * arc) + up * (sag * arc)
        })
        .collect();
    let rings: Vec<Vec<Vec3>> = at
        .iter()
        .enumerate()
        .map(|(index, &t)| {
            let taper = if thick_first {
                1.0 - 2.0 * t
            } else {
                2.0 * t - 1.0
            };
            let mut scale = radius * (1.0 + kit.taper * taper);
            if index == 0 || index == last {
                // The axe's chamfer round a sawn end.
                scale *= 0.88;
            }
            section
                .iter()
                .map(|&(angle, out)| {
                    let r = scale * out;
                    let across = libm::cosf(angle) * r;
                    let mut height = libm::sinf(angle) * r * 0.94;
                    // A split log keeps one half, flat along its middle.
                    match split {
                        Split::Top => height = height.max(0.0),
                        Split::Bottom => height = height.min(0.0),
                        Split::Whole => {}
                    }
                    centres[index] + side * across + up * height
                })
                .collect()
        })
        .collect();

    // Its side: the wood's grain along it, its texture shifted by the seed, its shade a
    // little lighter or darker log to log.
    let wood = metres(kit, Material::Log);
    let (shift_round, shift_along) = (random.unit() * 7.0, random.unit() * 7.0);
    let shade = 0.82 + random.unit() * 0.18;
    let colour = [shade; 3];
    let around = TAU * radius;
    let surface = surfaces.of(Material::Log);
    for (ring, pair) in rings.windows(2).enumerate() {
        let (near, far) = (&pair[0], &pair[1]);
        let (near_centre, far_centre) = (centres[ring], centres[ring + 1]);
        let round = |point: Vec3, centre: Vec3| {
            let out = point - centre;
            (out - axis * out.dot(axis)).normalize_or_zero()
        };
        let uv = |facet: usize, station: usize| {
            [
                shift_round + facet as f32 / facets as f32 * around / wood[0],
                shift_along + at[station] * length / wood[1],
            ]
        };
        for i in 0..facets {
            let j = (i + 1) % facets;
            for (corners, middles, uvs) in [
                (
                    [near[i], far[i], far[j]],
                    [near_centre, far_centre, far_centre],
                    [uv(i, ring), uv(i, ring + 1), uv(i + 1, ring + 1)],
                ),
                (
                    [near[i], far[j], near[j]],
                    [near_centre, far_centre, near_centre],
                    [uv(i, ring), uv(i + 1, ring + 1), uv(i + 1, ring)],
                ),
            ] {
                let middle = (near_centre + far_centre) * 0.5;
                let normal = facing(corners, middle);
                if normal == Vec3::ZERO {
                    continue;
                }
                let mut smooth = [0, 1, 2].map(|k| round(corners[k], middles[k]));
                if smooth.iter().any(|n| n.dot(normal) < CREASE) {
                    smooth = [normal; 3];
                }
                let flipped = wound(corners, normal) != corners;
                if flipped {
                    surface.textured(
                        [corners[0], corners[2], corners[1]],
                        [smooth[0], smooth[2], smooth[1]],
                        [uvs[0], uvs[2], uvs[1]],
                        colour,
                    );
                } else {
                    surface.textured(corners, smooth, uvs, colour);
                }
            }
        }
    }

    // Its sawn ends: the cut face's picture across each, turned by the seed.
    let ends = surfaces.of(Material::LogEnd);
    for (ring, outward) in [(&rings[0], -axis), (&rings[last], axis)] {
        let centre = ring.iter().copied().sum::<Vec3>() / ring.len() as f32;
        let reach = ring
            .iter()
            .map(|p| p.distance(centre))
            .fold(0.0, f32::max)
            .max(1e-3);
        let spin = random.unit() * TAU;
        let (s, c) = (libm::sinf(spin), libm::cosf(spin));
        let uv = |p: Vec3| {
            let (x, y) = ((p - centre).dot(side) / reach, (p - centre).dot(up) / reach);
            let (x, y) = (x * c - y * s, x * s + y * c);
            [0.5 + 0.48 * x, 0.5 - 0.48 * y]
        };
        for i in 0..facets {
            let j = (i + 1) % facets;
            let corners = [centre, ring[i], ring[j]];
            let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            if normal.length_squared() < 1e-12 {
                continue;
            }
            let corners = if normal.dot(outward) < 0.0 {
                [corners[0], corners[2], corners[1]]
            } else {
                corners
            };
            ends.textured(corners, [outward; 3], corners.map(uv), [1.0; 3]);
        }
    }
}

/// Faces that meet at more than about 50 degrees are shaded apart, as a crease; less, smooth
/// across (docs/art-style.md).
const CREASE: f32 = 0.64;

/// The normal of a triangle on a surface round `inside`, pointing away from it.
fn facing(corners: [Vec3; 3], inside: Vec3) -> Vec3 {
    let normal = (corners[1] - corners[0])
        .cross(corners[2] - corners[0])
        .normalize_or_zero();
    let middle = (corners[0] + corners[1] + corners[2]) / 3.0;
    if normal.dot(middle - inside) < 0.0 {
        -normal
    } else {
        normal
    }
}

/// The corners in the order that faces `normal`.
fn wound(corners: [Vec3; 3], normal: Vec3) -> [Vec3; 3] {
    let own = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
    if own.dot(normal) < 0.0 {
        [corners[0], corners[2], corners[1]]
    } else {
        corners
    }
}

/// A number from a list of numbers, the same on every machine.
fn hash(parts: &[u32]) -> u32 {
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
fn unit(hash: u32) -> f32 {
    (hash >> 8) as f32 / (1u32 << 24) as f32
}

/// A seeded run of numbers, for one log.
struct Random(u32);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 = hash(&[self.0]);
        self.0
    }

    fn unit(&mut self) -> f32 {
        unit(self.next())
    }

    fn below(&mut self, count: u32) -> u32 {
        if count == 0 { 0 } else { self.next() % count }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildOp, Cube, Panel, RoofSection, RoofStyle};

    fn kit() -> LogKit {
        LogKit::default()
    }

    /// A cabin four metres by three outside.
    fn cabin() -> Base {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [7, 0, 5],
            kind: kit().kind(),
        });
        base
    }

    fn wall(base: &mut Base, tiles: impl IntoIterator<Item = [i32; 3]>, panel: Panel) {
        for tile in tiles {
            base.apply(BuildOp::SetWall {
                tile,
                panel: Some(panel),
                kind: kit().kind(),
            });
        }
    }

    fn logs(layout: &Layout) -> Vec<(Vec3, Vec3, Split)> {
        layout
            .pieces
            .iter()
            .filter_map(|piece| match *piece {
                Piece::Log {
                    from, to, split, ..
                } => Some((from, to, split)),
                _ => None,
            })
            .collect()
    }

    fn boards(layout: &Layout) -> usize {
        layout
            .pieces
            .iter()
            .filter(|p| matches!(p, Piece::Board { .. }))
            .count()
    }

    #[test]
    fn the_kit_fills_a_storey_with_whole_courses() {
        assert_eq!(kit().courses(), (12, true));
    }

    #[test]
    fn a_room_has_a_dozen_logs_a_wall_a_side_and_stays_inside_its_cubes() {
        let layout = lay(&cabin(), &kit());
        let logs = logs(&layout);
        // Walls running east and west lay 12 courses; the others 11 and a split log at
        // the foot and on top.
        assert_eq!(logs.len(), 2 * 12 + 2 * 13, "{logs:?}");
        assert_eq!(logs.iter().filter(|l| l.2 != Split::Whole).count(), 4);
        // The south wall, the row of cubes from z = 2.5 to 3, runs through both corners
        // to their outer faces, a little short of them here and there, and no further.
        let south: Vec<_> = logs
            .iter()
            .filter(|(from, to, _)| from.z == 2.75 && to.z == 2.75)
            .collect();
        assert_eq!(south.len(), 12);
        for (from, to, _) in south {
            assert!(
                from.x >= 0.0 && from.x < 0.1 && to.x <= 4.0 && to.x > 3.9,
                "{from} {to}"
            );
        }
        // Nothing leaves the cubes: four metres by three.
        for (from, to, _) in &logs {
            for end in [from, to] {
                assert!(
                    (0.0..=4.0).contains(&end.x) && (0.0..=3.0).contains(&end.z),
                    "{end}"
                );
            }
        }
    }

    #[test]
    fn walls_either_way_never_share_a_course() {
        let logs = logs(&lay(&cabin(), &kit()));
        let height = |run_x: bool| -> Vec<i32> {
            logs.iter()
                .filter(|(from, to, _)| (from.z == to.z) == run_x)
                .map(|(from, _, _)| (from.y * 1000.0).round() as i32)
                .collect()
        };
        let (east_west, north_south) = (height(true), height(false));
        assert!(east_west.iter().all(|y| !north_south.contains(y)));
    }

    #[test]
    fn a_free_end_is_cut_flush() {
        let mut base = Base::default();
        wall(&mut base, (0..4).map(|x| [x, 0, 0]), Panel::Solid);
        for (from, to, _) in logs(&lay(&base, &kit())) {
            assert!(
                from.x.abs() < 1e-4 && (to.x - 2.0).abs() < 1e-4,
                "{from} {to}"
            );
        }
    }

    #[test]
    fn a_door_cuts_the_courses_it_crosses_and_is_framed() {
        let mut base = cabin();
        // A door a metre wide in the south wall.
        wall(&mut base, [[3, 0, 5], [4, 0, 5]], Panel::Door);
        let kit = kit();
        let layout = lay(&base, &kit);
        let south: Vec<_> = logs(&layout)
            .into_iter()
            .filter(|(from, to, _)| from.z == 2.75 && to.z == 2.75)
            .collect();
        // Every course the door crosses is two logs, ending at the jambs.
        let crossed = south
            .iter()
            .filter(|(from, _, _)| from.y < DOOR_HEIGHT_M)
            .count();
        let whole = south.len() - crossed;
        assert_eq!(crossed % 2, 0);
        assert_eq!(crossed / 2 + whole, 12);
        for (from, to, _) in &south {
            let clear = to.x <= 1.5 - kit.frame + 1e-4 || from.x >= 2.5 + kit.frame - 1e-4;
            assert!(clear || from.y > DOOR_HEIGHT_M, "{from} {to}");
        }
        // Two jambs and a head; a doorway needs no sill.
        assert_eq!(boards(&layout), 3);
    }

    #[test]
    fn a_window_has_a_sill_and_a_cube_carved_out_is_framed_too() {
        let mut base = cabin();
        wall(&mut base, [[0, 0, 2]], Panel::WINDOW);
        assert_eq!(boards(&lay(&base, &kit())), 4);
        // One cube a metre and a half up the north wall, carved out.
        base.apply(BuildOp::Fill {
            min: [3, 3, 0],
            max: [3, 3, 0],
            cube: Some(Cube::new(kit().kind(), Fill::Open)),
        });
        let layout = lay(&base, &kit());
        assert_eq!(boards(&layout), 8);
        // The two courses through it are cut, so the north wall has two more logs.
        let north = |layout: &Layout| {
            logs(layout)
                .iter()
                .filter(|(from, to, _)| from.z == 0.25 && to.z == 0.25)
                .count()
        };
        assert_eq!(north(&layout), 14);
    }

    #[test]
    fn a_gable_end_is_logs_that_shorten_toward_the_ridge() {
        let mut base = cabin();
        base.apply(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [7, 0, 5],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        // The west gable, on the row of cubes along x = 0, above the walls.
        let gable: Vec<_> = logs(&lay(&base, &kit()))
            .into_iter()
            .filter(|(from, to, _)| from.x == 0.25 && to.x == 0.25 && from.y > STOREY_M)
            .map(|(from, to, _)| (from.y, from.distance(to)))
            .collect();
        assert!(gable.len() >= 3, "{gable:?}");
        assert!(
            gable
                .windows(2)
                .all(|pair| pair[1].0 > pair[0].0 && pair[1].1 < pair[0].1)
        );
        // Under the roof: the ridge is 1.5 m from the eaves, rising 1.05 m.
        assert!(gable.iter().all(|(y, _)| *y < STOREY_M + 1.05));
    }

    #[test]
    fn no_log_passes_through_a_roof() {
        let kit = kit();
        for style in [RoofStyle::Hip, RoofStyle::Gable, RoofStyle::LeanTo] {
            let mut base = cabin();
            base.apply(BuildOp::AddRoof(RoofSection {
                min: [0, 0, 0],
                max: [7, 0, 5],
                style,
                along_x: true,
                pitch: 0.7,
            }));
            let section = base.sections()[0];
            let logs = logs(&lay(&base, &kit));
            for (from, to, _) in &logs {
                for i in 0..=20 {
                    let at = from.lerp(*to, i as f32 / 20.0);
                    let Some(rise) = section.rise_beyond(glam::Vec2::new(at.x, at.z), kit.eaves)
                    else {
                        continue;
                    };
                    let top = STOREY_M + rise;
                    let r = kit.radius();
                    assert!(
                        at.y + r <= top - ROOF_THICK_M + 1e-3 || at.y - r >= top - 1e-3,
                        "{style:?}: a log at {at} meets the roof at {top}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_long_wall_is_joined_with_the_seams_staggered() {
        let mut base = Base::default();
        wall(&mut base, (0..20).map(|x| [x, 0, 0]), Panel::Solid);
        let kit = kit();
        let logs = logs(&lay(&base, &kit));
        let limit = kit.longest + 2.0 * (kit.overhang + kit.ragged);
        assert!(
            logs.iter()
                .all(|(from, to, _)| from.distance(*to) <= limit + 1e-4)
        );
        let seams = |y: f32| -> Vec<i32> {
            logs.iter()
                .filter(|(from, _, _)| (from.y - y).abs() < 1e-3)
                .map(|(from, _, _)| (from.x * 100.0).round() as i32)
                .collect()
        };
        let (first, second) = (seams(0.125), seams(0.375));
        assert!(first.len() > 1 && second.len() > 1);
        assert!(first[1..].iter().all(|x| !second.contains(x)));
    }

    #[test]
    fn a_column_of_cubes_alone_is_a_post() {
        let mut base = Base::default();
        wall(&mut base, [[0, 0, 0]], Panel::Solid);
        let logs = logs(&lay(&base, &kit()));
        assert_eq!(
            logs,
            vec![(
                Vec3::new(0.25, 0.0, 0.25),
                Vec3::new(0.25, 3.0, 0.25),
                Split::Whole
            )]
        );
    }

    #[test]
    fn logs_are_the_same_on_every_machine_and_differ_from_one_another() {
        let kit = kit();
        let layout = lay(&cabin(), &kit);
        let drawn = |piece: &Piece| {
            let mut surfaces = Surfaces::default();
            draw(piece, &kit, &mut surfaces);
            surfaces.of(Material::Log).clone()
        };
        assert_eq!(drawn(&layout.pieces[0]), drawn(&layout.pieces[0]));
        let a = drawn(&layout.pieces[0]);
        let b = drawn(&layout.pieces[1]);
        assert!(!a.positions.is_empty());
        assert_ne!(a.positions, b.positions);
        // Faces point outward: each triangle's normal agrees with its winding.
        for (corners, normal) in a.positions.chunks(3).zip(a.normals.chunks(3)) {
            let own = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            assert!(own.dot(normal[0]) >= 0.0);
        }
    }
}
