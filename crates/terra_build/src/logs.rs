//! Log walls (docs/kits.md, D177, D178): each wall laid as logs by the stacked pattern, and
//! each log made from the kit's numbers and a seed, so it looks hewn and grown rather
//! than turned.
//!
//! **The layout** is worked out from the grid's walls alone. A wall is every wall edge in
//! a line on one storey, unbroken. Walls running east and west lay whole courses; walls
//! running north and south lie half a course higher, with a split log at the foot and on
//! top, so where two meet their logs interlock without either knowing of the other. Each
//! end of a wall is cut flush, or, where another wall crosses it and the open air lies
//! beyond, runs past as a notched corner. Openings cut the courses they cross, which end
//! at a frame. The pieces come out in the order they go in: from the bottom up.
//!
//! **A log** is a lathe of rings along its length, each ring an uneven polygon, tapered,
//! bent and turned a little, with knots now and then and axe-cut ends showing their
//! grain. Every face takes one of the kit's colours.

use crate::mesh::ROOF_THICK_M;
use crate::{Base, CELL_M, DOOR_HEIGHT_M, Face, Panel, STOREY_M, Side, Surface};
use glam::{IVec3, Vec3};
use std::collections::BTreeMap;
use std::f32::consts::TAU;

/// A kind of log wall: its sizes, how much each log varies, and its colours, from a file
/// in `assets/data/kits/` (D136). Colours are as they should appear on screen (sRGB),
/// named from the palette's roles where there is one.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogKit {
    /// How thick a log is, and so the wall.
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

/// A base's walls as pieces, and the glass in their windows.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    /// In the order they go in.
    pub pieces: Vec<Piece>,
    pub glass: Surface,
}

/// Which way a wall runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Axis {
    /// East and west: walls on the south side of cells.
    X,
    /// North and south: walls on the east side of cells.
    Z,
}

impl Axis {
    fn along(self) -> Vec3 {
        match self {
            Axis::X => Vec3::X,
            Axis::Z => Vec3::Z,
        }
    }

    /// The wall cell at a place along a line: the edge from grid point `at` to `at + 1`.
    fn face(self, line: i32, at: i32, storey: i32) -> Face {
        match self {
            Axis::X => Face::new(IVec3::new(at, storey, line - 1), Side::South),
            Axis::Z => Face::new(IVec3::new(line - 1, storey, at), Side::East),
        }
    }

    fn other(self) -> Axis {
        match self {
            Axis::X => Axis::Z,
            Axis::Z => Axis::X,
        }
    }
}

/// A gap a wall's courses are cut round, in metres along the wall and up from its storey.
struct Opening {
    start: f32,
    end: f32,
    bottom: f32,
    top: f32,
    window: bool,
}

/// Lays every wall of a base as logs.
pub fn lay(base: &Base, kit: &LogKit) -> Layout {
    // Every wall cell, by the line it is on, in order along it.
    let mut lines: BTreeMap<(i32, Axis, i32), BTreeMap<i32, Panel>> = BTreeMap::new();
    for (face, panel) in base.walls() {
        let tile = face.tile();
        let (axis, line, at) = match face.side {
            Side::South => (Axis::X, tile.z + 1, tile.x),
            _ => (Axis::Z, tile.x + 1, tile.z),
        };
        lines
            .entry((tile.y, axis, line))
            .or_default()
            .insert(at, panel);
    }
    let mut layout = Layout::default();
    for ((storey, axis, line), cells) in lines {
        let mut run: Vec<(i32, Panel)> = Vec::new();
        for (at, panel) in cells {
            if run.last().is_some_and(|&(last, _)| last + 1 != at) {
                wall(base, kit, storey, axis, line, &run, &mut layout);
                run.clear();
            }
            run.push((at, panel));
        }
        wall(base, kit, storey, axis, line, &run, &mut layout);
    }
    // Bottom up, keeping walls together within a course.
    layout.pieces.sort_by(|a, b| {
        (a.bottom() * 100.0)
            .round()
            .total_cmp(&(b.bottom() * 100.0).round())
    });
    layout
}

/// One unbroken wall: its cells in order along its line, from grid point `run[0].0`.
fn wall(
    base: &Base,
    kit: &LogKit,
    storey: i32,
    axis: Axis,
    line: i32,
    run: &[(i32, Panel)],
    layout: &mut Layout,
) {
    let (Some(&(first, _)), Some(&(last, _))) = (run.first(), run.last()) else {
        return;
    };
    let radius = kit.radius();
    let floor = storey as f32 * STOREY_M;
    let along = axis.along();
    // The wall's middle line, as a point at a distance along it and a height.
    let line_m = line as f32 * CELL_M;
    let at = |distance: f32, height: f32| match axis {
        Axis::X => Vec3::new(distance, height, line_m),
        Axis::Z => Vec3::new(line_m, height, distance),
    };

    // How far past each end its logs reach: notched corners where a wall crosses and the
    // open air is beyond, flush otherwise.
    let crossed = |point: i32| {
        let other = axis.other();
        [line - 1, line]
            .into_iter()
            .any(|cell| base.filling(other.face(point, cell, storey)).is_some())
    };
    let open_beyond = |cell: i32| {
        let face = axis.face(line, cell, storey);
        let tile = face.tile();
        !base.contains(tile) && !base.contains(tile + face.side.offset())
    };
    let start_point = first;
    let end_point = last + 1;
    let start_corner = crossed(start_point) && open_beyond(start_point - 1);
    let end_corner = crossed(end_point) && open_beyond(end_point);

    let openings = openings(run);
    let (courses, _) = kit.courses();
    let low_courses = axis == Axis::Z;
    for course in 0..=courses {
        // Walls one way lie half a course above the other's, split at foot and top.
        let (centre, split) = match (low_courses, course) {
            (false, c) if c == courses => break,
            (false, c) => (kit.course * (c as f32 + 0.5), Split::Whole),
            (true, 0) => (0.0, Split::Top),
            (true, c) if c == courses => (STOREY_M, Split::Bottom),
            (true, c) => (kit.course * c as f32, Split::Whole),
        };
        let (low, high) = match split {
            Split::Whole => (centre - radius * 0.9, centre + radius * 0.9),
            Split::Top => (centre, centre + radius * 0.9),
            Split::Bottom => (centre - radius * 0.9, centre),
        };
        let seed = hash(&[storey as u32, axis as u32, line as u32, course as u32]);
        let ragged = |end: u32| (unit(hash(&[seed, end])) - 0.5) * 2.0 * kit.ragged;
        let reach = |corner: bool, end: u32| {
            if corner {
                radius + kit.overhang + ragged(end)
            } else {
                radius
            }
        };
        let start = start_point as f32 * CELL_M - reach(start_corner, 0);
        let end = end_point as f32 * CELL_M + reach(end_corner, 1);

        // The course, less what openings cut from it.
        let mut stretches = vec![(start, end)];
        for opening in openings.iter().filter(|o| o.bottom < high && o.top > low) {
            let (cut_start, cut_end) = (opening.start - kit.frame, opening.end + kit.frame);
            stretches = stretches
                .into_iter()
                .flat_map(|(a, b)| {
                    let mut kept = Vec::new();
                    if cut_start > a {
                        kept.push((a, cut_start.min(b)));
                    }
                    if cut_end < b {
                        kept.push((cut_end.max(a), b));
                    }
                    kept
                })
                .filter(|(a, b)| b - a > 0.05)
                .collect();
        }
        // Daub between this course and the one under it, along the wall itself.
        let daub = kit.chinking && split != Split::Top && course > 0;
        let (wall_start, wall_end) = (
            start_point as f32 * CELL_M - radius * 0.5,
            end_point as f32 * CELL_M + radius * 0.5,
        );
        for (a, b) in stretches {
            for (index, (a, b)) in joined(
                a,
                b,
                kit.longest + 2.0 * (radius + kit.overhang + kit.ragged),
                course,
            )
            .into_iter()
            .enumerate()
            {
                let chinking = daub.then(|| (wall_start.max(a) - a, wall_end.min(b) - a));
                layout.pieces.push(Piece::Log {
                    from: at(a, floor + centre),
                    to: at(b, floor + centre),
                    split,
                    seed: hash(&[seed, index as u32, (a * 100.0) as i32 as u32]),
                    chinking: chinking.filter(|(near, far)| far > near),
                });
            }
        }
    }

    // Under a pitched roof, courses carry on up, each log as long as fits under it, so
    // they shorten toward the ridge as a gable's do.
    let rise = |distance: f32| {
        let point = at(distance, 0.0);
        base.roof_rise(storey, glam::Vec2::new(point.x, point.z))
            .map(|rise| rise - ROOF_THICK_M)
    };
    let (wall_from, wall_to) = (start_point as f32 * CELL_M, end_point as f32 * CELL_M);
    for step in 0.. {
        let centre = STOREY_M + kit.course * (step as f32 + 0.5);
        let needed = centre - STOREY_M + radius * 0.9;
        let mut spans: Vec<(f32, f32)> = Vec::new();
        let mut open: Option<f32> = None;
        let samples = ((wall_to - wall_from) / GABLE_STEP_M).round() as i32;
        for sample in 0..=samples {
            let distance = wall_from + sample as f32 * GABLE_STEP_M;
            let fits = rise(distance).is_some_and(|rise| rise >= needed);
            match (fits, open) {
                (true, None) => open = Some(distance),
                (false, Some(from)) => {
                    spans.push((from, distance - GABLE_STEP_M));
                    open = None;
                }
                _ => {}
            }
        }
        if let Some(from) = open {
            spans.push((from, wall_to));
        }
        spans.retain(|(a, b)| b - a > 0.15);
        if spans.is_empty() {
            break;
        }
        let seed = hash(&[storey as u32, axis as u32, line as u32, 100 + step as u32]);
        for (index, (a, b)) in spans.into_iter().enumerate() {
            layout.pieces.push(Piece::Log {
                from: at(a, floor + centre),
                to: at(b, floor + centre),
                split: Split::Whole,
                seed: hash(&[seed, index as u32]),
                chinking: kit.chinking.then_some((0.0, b - a)),
            });
        }
    }

    // A frame round each opening, and the glass in a window.
    let depth = Vec3::Y.cross(along) * (radius + 0.02);
    let course_centres: Vec<f32> = (0..=courses)
        .map(|c| {
            if low_courses {
                kit.course * c as f32
            } else {
                kit.course * (c as f32 + 0.5)
            }
        })
        .collect();
    for opening in &openings {
        // The head reaches up into the first log over the opening; a sill down into the
        // first under it.
        let head = course_centres
            .iter()
            .copied()
            .find(|&c| c - radius * 0.9 >= opening.top)
            .unwrap_or(STOREY_M)
            .max(opening.top + 0.08);
        let foot = if opening.window {
            course_centres
                .iter()
                .copied()
                .rev()
                .find(|&c| c + radius * 0.9 <= opening.bottom)
                .unwrap_or(0.0)
                .min(opening.bottom - 0.06)
        } else {
            opening.bottom
        };
        let (start, end) = (opening.start, opening.end);
        let mut board = |a: f32, b: f32, bottom: f32, top: f32, proud: f32| {
            let low = at(a, floor + bottom) - depth - depth.normalize() * proud;
            let high = at(b, floor + top) + depth + depth.normalize() * proud;
            layout.pieces.push(Piece::Board {
                min: low.min(high),
                max: low.max(high),
            });
        };
        board(start - kit.frame, start, foot, head, 0.0);
        board(end, end + kit.frame, foot, head, 0.0);
        board(start - kit.frame, end + kit.frame, opening.top, head, 0.01);
        if opening.window {
            board(
                start - kit.frame,
                end + kit.frame,
                foot,
                opening.bottom,
                0.03,
            );
            let corners = [
                at(start, floor + opening.bottom),
                at(end, floor + opening.bottom),
                at(end, floor + opening.top),
                at(start, floor + opening.top),
            ];
            let normal = along.cross(Vec3::Y);
            layout
                .glass
                .positions
                .extend([0, 1, 2, 0, 2, 3].map(|i| corners[i]));
            layout.glass.normals.extend([normal; 6]);
            layout.glass.colours.extend([[0.55, 0.8, 0.9]; 6]);
        }
    }
}

/// The openings along a wall, in metres along its line, from its cells in order.
fn openings(run: &[(i32, Panel)]) -> Vec<Opening> {
    let mut found: Vec<Opening> = Vec::new();
    let mut previous: Option<(i32, Panel)> = None;
    for &(at, panel) in run {
        let (bottom, top, window) = match panel {
            Panel::Solid => {
                previous = None;
                continue;
            }
            Panel::Door => (0.0, DOOR_HEIGHT_M, false),
            Panel::Window { sill, top } => (sill, STOREY_M - top, true),
        };
        let start = at as f32 * CELL_M;
        let end = start + CELL_M;
        match (previous, found.last_mut()) {
            (Some((last, last_panel)), Some(opening)) if last + 1 == at && last_panel == panel => {
                opening.end = end;
            }
            _ => found.push(Opening {
                start,
                end,
                bottom,
                top,
                window,
            }),
        }
        previous = Some((at, panel));
    }
    found
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
    let shift = if course % 2 == 1 { each * 0.5 } else { 0.0 };
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
    use crate::BuildOp;

    fn cabin() -> Base {
        let mut base = Base::default();
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [7, 0, 5],
        });
        base
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

    #[test]
    fn the_kit_fills_a_storey_with_whole_courses() {
        assert_eq!(LogKit::default().courses(), (12, true));
    }

    #[test]
    fn a_room_has_a_dozen_logs_a_wall_a_side_and_its_corners_run_past() {
        let kit = LogKit::default();
        let layout = lay(&cabin(), &kit);
        let logs = logs(&layout);
        // Walls running east and west lay 12 courses; the others 11 and a split log at
        // the foot and on top.
        assert_eq!(logs.len(), 2 * 12 + 2 * 13);
        assert_eq!(logs.iter().filter(|l| l.2 != Split::Whole).count(), 4);
        // The south wall, 4 m long from x = 0 to 4, runs past both corners.
        let south: Vec<_> = logs
            .iter()
            .filter(|(from, to, _)| from.z == 3.0 && to.z == 3.0)
            .collect();
        assert_eq!(south.len(), 12);
        for (from, to, _) in south {
            assert!(from.x < -0.3 && to.x > 4.3, "{from} {to}");
        }
    }

    #[test]
    fn walls_either_way_never_share_a_course() {
        let kit = LogKit::default();
        let logs = logs(&lay(&cabin(), &kit));
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
        for x in 0..4 {
            base.apply(BuildOp::SetWall(
                Face::new(IVec3::new(x, 0, 0), Side::South),
                Some(Panel::Solid),
            ));
        }
        let kit = LogKit::default();
        for (from, to, _) in logs(&lay(&base, &kit)) {
            assert!((from.x + 0.15).abs() < 1e-4 && (to.x - 2.15).abs() < 1e-4);
        }
    }

    #[test]
    fn a_door_cuts_the_courses_it_crosses_and_is_framed() {
        let mut base = cabin();
        // A door a metre wide in the south wall.
        for x in 3..5 {
            base.apply(BuildOp::SetWall(
                Face::new(IVec3::new(x, 0, 5), Side::South),
                Some(Panel::Door),
            ));
        }
        let kit = LogKit::default();
        let layout = lay(&base, &kit);
        let south: Vec<_> = logs(&layout)
            .into_iter()
            .filter(|(from, to, _)| from.z == 3.0 && to.z == 3.0)
            .collect();
        // Every course the door crosses is two logs, ending at the jambs.
        let crossed = south
            .iter()
            .filter(|(from, _, _)| from.y < DOOR_HEIGHT_M + 0.13)
            .count();
        let whole = south.len() - crossed;
        assert_eq!(crossed % 2, 0);
        assert_eq!(crossed / 2 + whole, 12);
        for (from, to, _) in &south {
            let clear = to.x <= 1.5 - kit.frame + 1e-4 || from.x >= 2.5 + kit.frame - 1e-4;
            assert!(clear || from.y > DOOR_HEIGHT_M, "{from} {to}");
        }
        assert_eq!(
            layout
                .pieces
                .iter()
                .filter(|p| matches!(p, Piece::Board { .. }))
                .count(),
            3
        );
    }

    #[test]
    fn a_gable_end_is_logs_that_shorten_toward_the_ridge() {
        let mut base = cabin();
        base.apply(BuildOp::AddRoof(crate::RoofSection {
            min: [0, 0, 0],
            max: [7, 0, 5],
            style: crate::RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        let kit = LogKit::default();
        // The west gable, on the line x = 0, above the walls.
        let gable: Vec<_> = logs(&lay(&base, &kit))
            .into_iter()
            .filter(|(from, to, _)| from.x == 0.0 && to.x == 0.0 && from.y > STOREY_M)
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
        for x in 0..20 {
            base.apply(BuildOp::SetWall(
                Face::new(IVec3::new(x, 0, 0), Side::South),
                Some(Panel::Solid),
            ));
        }
        let kit = LogKit::default();
        let logs = logs(&lay(&base, &kit));
        let limit = kit.longest + kit.diameter + 2.0 * (kit.overhang + kit.ragged);
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
    fn logs_are_the_same_on_every_machine_and_differ_from_one_another() {
        let kit = LogKit::default();
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
