//! Log walls (docs/building-kits.md, D177, D178): each wall of log cubes laid as logs by the stacked
//! pattern, and each log made from the kit's numbers and a seed, so it looks hewn and
//! grown rather than turned.
//!
//! **The layout** is worked out from the cubes alone (D181). A row of cubes running east
//! and west lays whole courses, two to a cube; a row running north and south lies half a
//! course higher, with a split log at its foot and on top, so where two meet their logs
//! interlock without either knowing of the other. A cube with rows both ways is a corner,
//! a T or a cross, and its logs run past it into the open air; a row ending with nothing
//! across it is cut flush. Openings, cubes cut open or glazed, cut the courses through
//! them, which end at a frame of boards. A cube with nothing beside it either way is a
//! post. Under a pitched roof the courses carry on up, shortening to the ridge. The
//! pieces come out in the order they go in: from the bottom up.
//!
//! **A log** is a lathe of rings along its length, each ring an uneven polygon, tapered,
//! bent and turned a little, with knots now and then and axe-cut ends showing their
//! grain. Every face takes one of the kit's colours.

#[cfg(test)]
use crate::DOOR_HEIGHT_M;
use crate::mesh::ROOF_THICK_M;
use crate::{Base, CELL_M, Fill, Kind, STOREY_CUBES, STOREY_M, Surface, storey_of, wall_layer};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::f32::consts::TAU;

/// A kind of log wall: its sizes, how much each log varies, and its colours, from a file
/// in `assets/data/kits/` (D136). Colours are as they should appear on screen (sRGB),
/// named from the palette's roles where there is one.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogKit {
    /// The kind of construction it lays (D181): the number the cubes it lays carry.
    pub id: u16,
    /// How thick a log is.
    pub diameter: f32,
    /// How far apart the courses are; less than the diameter, as logs are scribed to sit
    /// into one another. Must divide the storey.
    pub course: f32,
    /// How far a log runs past the wall it crosses at a corner.
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
    /// How far a corner log's end may differ from the others', in metres.
    pub ragged: f32,
    /// Whether there is daub in the groove between courses.
    pub chinking: bool,
    /// The boards round an opening: how wide and how far proud of the logs.
    pub frame: f32,
    /// The model each log is drawn with (D177), an asset path, its length along X; until
    /// the kit has one, logs are drawn in code.
    #[serde(default)]
    pub log_model: Option<String>,
    pub colours: LogColours,
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
            diameter: 0.3,
            course: 0.25,
            overhang: 0.25,
            longest: 4.0,
            facets: (9, 11),
            taper: 0.07,
            bend: 0.035,
            roughness: 0.07,
            knots: 0.5,
            ragged: 0.04,
            chinking: true,
            frame: 0.08,
            log_model: None,
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
}

impl Piece {
    /// The lowest point of it, which orders the pieces as they go in.
    fn bottom(&self) -> f32 {
        match self {
            Piece::Log { from, to, .. } => from.y.min(to.y),
            Piece::Board { min, .. } => min.y,
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
    // Bottom up, keeping walls together within a course.
    layout.pieces.sort_by(|a, b| {
        (a.bottom() * 100.0)
            .round()
            .total_cmp(&(b.bottom() * 100.0).round())
    });
    layout
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
                    Spot::Nothing if corner && open => {
                        let ragged = (unit(hash(&[seed, end as u32, which])) - 0.5) * 2.0;
                        kit.overhang + ragged * kit.ragged
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

/// Draws a piece.
pub fn draw(piece: &Piece, kit: &LogKit, surface: &mut Surface) {
    match *piece {
        Piece::Log {
            from,
            to,
            split,
            seed,
            chinking,
        } => {
            log(surface, kit, from, to, split, seed);
            if let Some((near, far)) = chinking {
                let along = (to - from).normalize_or_zero();
                let depth = Vec3::Y.cross(along) * kit.radius() * 0.5;
                let start = from + along * near;
                let end = from + along * far;
                let gap = kit.course * 0.5;
                let low = start - depth - Vec3::Y * (gap + 0.018);
                let high = end + depth - Vec3::Y * (gap - 0.018);
                cuboid(surface, low.min(high), low.max(high), kit.colours.chinking);
            }
        }
        Piece::Board { min, max } => cuboid(surface, min, max, kit.colours.board),
    }
}

/// A box from one corner to the other, flat-faced in one colour.
fn cuboid(surface: &mut Surface, min: Vec3, max: Vec3, colour: [f32; 3]) {
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
        let [a, b, c, d] = corners.map(|(x, y, z)| corner(x, y, z));
        surface.quad([a, b, c, d], normal, colour);
    }
}

/// How far each ring of a log is from its start, as a fraction of its length: close
/// together at the ends, where the axe's chamfer is, and about every half metre between.
fn stations(length: f32) -> Vec<f32> {
    let chamfer = (0.035 / length).min(0.2);
    let middle = ((length / 0.45).ceil() as usize).max(1);
    let mut at = vec![0.0, chamfer];
    for step in 1..middle {
        at.push(chamfer + (1.0 - 2.0 * chamfer) * step as f32 / middle as f32);
    }
    at.extend([1.0 - chamfer, 1.0]);
    at
}

/// One log.
fn log(surface: &mut Surface, kit: &LogKit, from: Vec3, to: Vec3, split: Split, seed: u32) {
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
    let colours = &kit.colours;

    // The section: an uneven polygon, the same shape all along, turned a little.
    let (fewest, most) = kit.facets;
    let facets = (fewest + random.below(most.saturating_sub(fewest) + 1)) as usize;
    let turn = random.unit() * TAU;
    let section: Vec<(f32, f32)> = (0..facets)
        .map(|i| {
            let angle = turn + TAU * (i as f32 + (random.unit() - 0.5) * 0.55) / facets as f32;
            let out = 1.0 + (random.unit() - 0.5) * 2.0 * kit.roughness;
            (angle, out)
        })
        .collect();

    // A tree's shape along it: thicker at one end, bowed, swelling here and there.
    let thick_first = random.unit() < 0.5;
    let bow = (random.unit() - 0.5) * 2.0 * kit.bend;
    let sag = (random.unit() - 0.5) * kit.bend;
    let twist = (random.unit() - 0.5) * 0.3;
    let at = stations(length);
    let last = at.len() - 1;
    let rings: Vec<Vec<Vec3>> = at
        .iter()
        .enumerate()
        .map(|(index, &t)| {
            let end = index == 0 || index == last;
            let taper = if thick_first {
                1.0 - 2.0 * t
            } else {
                2.0 * t - 1.0
            };
            let swell = 1.0 + (random.unit() - 0.5) * 0.06;
            let mut scale = radius * (1.0 + kit.taper * taper) * swell;
            if end {
                // The axe's chamfer round a cut end.
                scale *= 0.86;
            }
            let arc = libm::sinf(std::f32::consts::PI * t);
            let centre = from + axis * (length * t) + side * (bow * arc) + up * (sag * arc);
            section
                .iter()
                .map(|&(angle, out)| {
                    let wobble = 1.0 + (random.unit() - 0.5) * 0.04;
                    let angle = angle + twist * t;
                    let r = scale * out * wobble;
                    let across = libm::cosf(angle) * r;
                    let mut height = libm::sinf(angle) * r * 0.94;
                    // A split log keeps one half, flat along its middle.
                    match split {
                        Split::Top => height = height.max(0.0),
                        Split::Bottom => height = height.min(0.0),
                        Split::Whole => {}
                    }
                    centre + side * across + up * height
                })
                .collect()
        })
        .collect();

    // Its surface: each log its own tone, each facet a faint streak of a lighter or darker
    // shade, now and then a patch of bark left on, the top weathered lighter.
    let tone = match random.unit() {
        x if x < 0.33 => mix(colours.wood, colours.light, random.unit() * 0.35),
        x if x < 0.66 => mix(colours.wood, colours.dark, random.unit() * 0.35),
        _ => colours.wood,
    };
    let streaks: Vec<[f32; 3]> = (0..facets)
        .map(|_| match random.unit() {
            x if x < 0.3 => mix(tone, colours.light, 0.2),
            x if x < 0.55 => mix(tone, colours.dark, 0.2),
            _ => tone,
        })
        .collect();
    for (ring, pair) in rings.windows(2).enumerate() {
        let (near, far) = (&pair[0], &pair[1]);
        let middle = from + axis * (length * (at[ring] + at[ring + 1]) * 0.5);
        for i in 0..facets {
            let j = (i + 1) % facets;
            let mut colour = streaks[i];
            if random.unit() < 0.04 {
                colour = mix(colour, colours.bark, 0.7);
            }
            for corners in [[near[i], far[i], far[j]], [near[i], far[j], near[j]]] {
                let normal = facing(corners, middle);
                if normal == Vec3::ZERO {
                    continue;
                }
                let colour = if normal.dot(up) > 0.6 {
                    mix(colour, colours.light, 0.25)
                } else {
                    colour
                };
                let corners = wound(corners, normal);
                surface.triangle(corners, normal, colour);
            }
        }
    }

    // The cut ends: a ring of bark, the sapwood, and a darker heart, a little off square.
    for (ring, outward) in [(&rings[0], -axis), (&rings[last], axis)] {
        let centre = ring.iter().copied().sum::<Vec3>() / ring.len() as f32;
        let tilt = (side * (random.unit() - 0.5) + up * (random.unit() - 0.5)) * 0.12;
        let lean = |point: Vec3| point + outward * (point - centre).dot(tilt);
        let inset = |fraction: f32| -> Vec<Vec3> {
            ring.iter()
                .map(|&point| lean(centre + (point - centre) * fraction))
                .collect()
        };
        let bands = [
            (inset(1.0), inset(0.84), colours.bark),
            (inset(0.84), inset(0.42), colours.end),
        ];
        for (outer, inner, colour) in bands {
            for i in 0..facets {
                let j = (i + 1) % facets;
                for corners in [
                    [outer[i], outer[j], inner[j]],
                    [outer[i], inner[j], inner[i]],
                ] {
                    end_triangle(surface, corners, outward, colour);
                }
            }
        }
        let heart = inset(0.42);
        let middle = lean(centre);
        for i in 0..facets {
            let j = (i + 1) % facets;
            end_triangle(
                surface,
                [middle, heart[i], heart[j]],
                outward,
                colours.heart,
            );
        }
    }

    // Knots: the stubs of branches cut off, on a side that shows.
    let knots = (length * kit.knots + random.unit()).floor() as usize;
    if split == Split::Whole {
        for _ in 0..knots {
            let t = 0.15 + random.unit() * 0.7;
            let angle =
                (random.unit() - 0.5) * 1.6 + if random.unit() < 0.5 { 0.0 } else { TAU * 0.5 };
            let out = side * libm::cosf(angle) + up * libm::sinf(angle);
            let centre = from + axis * (length * t);
            knot(
                surface,
                centre + out * radius * 0.86,
                out,
                axis,
                radius * 0.24,
                &mut random,
                colours,
            );
        }
    }
}

/// A short stub standing out from a log.
fn knot(
    surface: &mut Surface,
    base: Vec3,
    out: Vec3,
    along: Vec3,
    size: f32,
    random: &mut Random,
    colours: &LogColours,
) {
    let across = out.cross(along).normalize();
    let along = across.cross(out);
    let sides = 5;
    let height = size * (0.5 + random.unit() * 0.4);
    let ring = |scale: f32, lift: f32| -> Vec<Vec3> {
        (0..sides)
            .map(|i| {
                let angle = TAU * i as f32 / sides as f32;
                base + out * lift
                    + (across * libm::cosf(angle) + along * libm::sinf(angle)) * size * scale
            })
            .collect()
    };
    let foot = ring(1.0, 0.0);
    let top = ring(0.6, height);
    for i in 0..sides {
        let j = (i + 1) % sides;
        for corners in [[foot[i], top[i], top[j]], [foot[i], top[j], foot[j]]] {
            let normal = facing(corners, base + out * (height * 0.5));
            surface.triangle(wound(corners, normal), normal, colours.knot);
        }
    }
    let tip = base + out * height;
    for i in 0..sides {
        let j = (i + 1) % sides;
        end_triangle(surface, [tip, top[i], top[j]], out, colours.end);
    }
}

/// A triangle of a cut end, facing the way the end does.
fn end_triangle(surface: &mut Surface, corners: [Vec3; 3], outward: Vec3, colour: [f32; 3]) {
    let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
    if normal.length_squared() < 1e-12 {
        return;
    }
    let normal = normal.normalize();
    let (corners, normal) = if normal.dot(outward) < 0.0 {
        ([corners[0], corners[2], corners[1]], -normal)
    } else {
        (corners, normal)
    };
    surface.triangle(corners, normal, colour);
}

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

fn mix(a: [f32; 3], b: [f32; 3], amount: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * amount)
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
    fn a_room_has_a_dozen_logs_a_wall_a_side_and_its_corners_run_past() {
        let layout = lay(&cabin(), &kit());
        let logs = logs(&layout);
        // Walls running east and west lay 12 courses; the others 11 and a split log at
        // the foot and on top.
        assert_eq!(logs.len(), 2 * 12 + 2 * 13, "{logs:?}");
        assert_eq!(logs.iter().filter(|l| l.2 != Split::Whole).count(), 4);
        // The south wall, the row of cubes from z = 2.5 to 3, runs past both corners.
        let south: Vec<_> = logs
            .iter()
            .filter(|(from, to, _)| from.z == 2.75 && to.z == 2.75)
            .collect();
        assert_eq!(south.len(), 12);
        for (from, to, _) in south {
            assert!(from.x < -0.2 && to.x > 4.2, "{from} {to}");
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
            cube: Some(Cube {
                kind: kit().kind(),
                fill: Fill::Open,
            }),
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
            let mut surface = Surface::default();
            draw(piece, &kit, &mut surface);
            surface
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
