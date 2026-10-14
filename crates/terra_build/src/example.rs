//! The example base, laid by the build experiment and the yard, and the footprint the start
//! is chosen for, so the place every test position is measured from stays where it is.

use crate::roofer::{self, Rectangle, Style};
use crate::{Base, BuildOp, CELL_M, Cube, Fill, Kind, Panel, STOREY_CUBES, Side};
use glam::IVec3;

/// The example base: two rooms joined by a corridor, a small upper storey, windows on
/// every side and a door. Written in metres, so it is the same base whatever the grid.
pub fn example(kind: Kind) -> Vec<BuildOp> {
    // A block of tiles from one outside corner in metres to the other, on a storey. Rooms
    // that meet share the row of tiles between them, so they open into one another.
    let room = |west: f32, east: f32, north: f32, south: f32, storey: i32| BuildOp::AddRoom {
        min: [cells_from(west), storey, cells_from(north)],
        max: [cells_from(east) - 1, storey, cells_from(south) - 1],
        kind,
    };
    let mut ops = vec![
        room(-6.0, 9.0, -6.0, 3.0, 0),
        room(8.5, 15.5, -3.0, 0.0, 0),
        room(15.0, 21.0, -6.0, 3.0, 0),
        room(-3.0, 3.0, -6.0, 0.0, 1),
    ];
    // An opening in a wall: which way the wall faces, where the wall is, and the stretch
    // of it to open, all in metres.
    let mut cut = |side: Side, wall_at: f32, from: f32, to: f32, storey: i32, panel: Panel| {
        // The tile of the wall, just inside its outer face.
        let inside = match side {
            Side::East | Side::South => cells_from(wall_at) - 1,
            _ => cells_from(wall_at),
        };
        for along in cells_from(from)..cells_from(to) {
            let cell = match side {
                Side::East | Side::West => IVec3::new(inside, storey, along),
                _ => IVec3::new(along, storey, inside),
            };
            ops.push(BuildOp::SetWall {
                tile: cell.to_array(),
                panel: Some(panel),
                kind,
            });
        }
    };
    cut(Side::South, 3.0, -3.0, 6.0, 0, Panel::WINDOW);
    cut(Side::South, 3.0, 7.0, 8.5, 0, Panel::Door);
    cut(Side::West, -6.0, -2.5, -0.5, 0, Panel::WINDOW);
    cut(Side::South, 0.0, -2.5, 2.5, 1, Panel::TALL_WINDOW);
    cut(Side::South, 0.0, 12.5, 14.5, 0, Panel::WINDOW);
    cut(Side::South, 3.0, 18.5, 20.5, 0, Panel::WINDOW);
    cut(Side::East, 21.0, -2.5, -0.5, 0, Panel::TALL_WINDOW);
    cut(Side::North, -6.0, -5.5, -3.5, 0, Panel::WINDOW);
    cut(Side::North, -6.0, 6.5, 8.5, 0, Panel::WINDOW);
    cut(Side::North, -6.0, 18.5, 20.5, 0, Panel::WINDOW);
    cut(Side::North, -6.0, -2.5, -0.5, 1, Panel::WINDOW);
    ops
}

/// The grid line a distance in metres falls on. Unlike [`crate::cells`], which counts, this
/// places, so it can be negative or zero.
fn cells_from(metres: f32) -> i32 {
    (metres / CELL_M).round() as i32
}

/// A wall's tile with an opening in it.
fn opening(tile: [i32; 3], panel: Panel, kind: Kind) -> BuildOp {
    BuildOp::SetWall {
        tile,
        panel: Some(panel),
        kind,
    }
}

/// A roof's parts: each a rectangle of tiles, from one to another east and south, of a
/// style, its ridge or rise along X or not.
pub type RoofParts = Vec<([i32; 2], [i32; 2], Style, bool)>;

/// A plan's rooms, each from one tile to another, and its roof's parts.
pub type Plan = (Vec<([i32; 3], [i32; 3])>, RoofParts);

/// A roof of cubes put up at once, built (D241), over parts of a roof on a storey, the
/// highest where they overlap, as the roofer lays it over what `ops` build: in `kind`, the
/// walls under its gable ends carried up to it.
pub fn roof_over(ops: &[BuildOp], storey: i32, parts: &RoofParts, kind: Kind) -> Vec<BuildOp> {
    let parts: Vec<(Rectangle, Style, bool)> = parts
        .iter()
        .map(|&(min, max, style, along_x)| (Rectangle { min, max }, style, along_x))
        .collect();
    let (footprint, height) = roofer::sections(&parts);
    let base = Base::from_shortcuts(ops);
    roofer::put_up(&base, &footprint, storey, kind, &height)
}

/// `ops` with a roof of cubes put up over them, on the ground storey ([`roof_over`]).
pub fn with_roof(mut ops: Vec<BuildOp>, parts: &RoofParts, kind: Kind) -> Vec<BuildOp> {
    let roof = roof_over(&ops, 0, parts, kind);
    ops.extend(roof);
    ops
}

/// A plan's shape: the rooms it is drawn as, from one tile to another, and the parts of
/// its roof, its main block's in one style and its wings' in another. Arms are four metres
/// wide, but a U's, three.
pub fn plan(shape: &str, main: Style, wings: Style) -> Plan {
    // A wing off the main block reaches into it: to its ridge for a gable, so its ridge
    // dies into the main roof's; across it for a hip, whose end lies under the main roof's
    // slope.
    let wing = |x: (i32, i32), z_end: i32| {
        let from = if wings == Style::Gable { 4 } else { 0 };
        ([x.0, from], [x.1, z_end], wings, false)
    };
    match shape {
        "L" => (
            vec![([0, 0, 0], [11, 0, 7]), ([0, 0, 7], [7, 0, 15])],
            vec![([0, 0], [11, 7], main, true), wing((0, 7), 15)],
        ),
        "T" => (
            vec![([0, 0, 0], [13, 0, 7]), ([3, 0, 7], [10, 0, 15])],
            vec![([0, 0], [13, 7], main, true), wing((3, 10), 15)],
        ),
        "U" => (
            vec![
                ([0, 0, 0], [15, 0, 7]),
                ([0, 0, 7], [5, 0, 13]),
                ([10, 0, 7], [15, 0, 13]),
            ],
            vec![
                ([0, 0], [15, 7], main, true),
                wing((0, 5), 13),
                wing((10, 15), 13),
            ],
        ),
        _ => (
            vec![([0, 0, 0], [9, 0, 7])],
            vec![([0, 0], [9, 7], main, true)],
        ),
    }
}

/// The roof rows: every sensible roof over every shape of plan, in a kind, or mixed: the
/// main block in the second kind and its wings in the first, so their roofs meet. Each with
/// a door in its north wall and a window in its west, under the roof of cubes `roof` gives
/// its main walls (D241).
pub fn roof_rows(
    kinds: &[Kind],
    mixed: bool,
    roof: &dyn Fn(Kind) -> Kind,
) -> Vec<(String, Vec<BuildOp>)> {
    let mut rows = Vec::new();
    for shape in ["rectangle", "L", "T", "U"] {
        // A lean-to only over a rectangle: over a wing it makes a tall gable wall where it
        // is high, so an L, a T or a U takes a hip over its main block with gabled wings.
        let third = if shape == "rectangle" {
            (Style::LeanTo, Style::LeanTo, "lean-to")
        } else {
            (Style::Hip, Style::Gable, "hip and gable")
        };
        for (style, wings, name) in [
            (Style::Gable, Style::Gable, "gable"),
            (Style::Hip, Style::Hip, "hip"),
            third,
        ] {
            let (rooms, parts) = plan(shape, style, wings);
            let mut ops = Vec::new();
            for (index, &(min, max)) in rooms.iter().enumerate() {
                let kind = if mixed && kinds.len() > 1 && index == 0 && rooms.len() > 1 {
                    kinds[1]
                } else {
                    kinds[0]
                };
                ops.push(BuildOp::AddRoom { min, max, kind });
            }
            let main = ops.first().map_or(kinds[0], |op| match op {
                BuildOp::AddRoom { kind, .. } => *kind,
                _ => kinds[0],
            });
            ops.extend((4..=5).map(|x| opening([x, 0, 0], Panel::Door, main)));
            ops.extend((3..=4).map(|z| opening([0, 0, z], Panel::WINDOW, main)));
            // A plain rectangle mixed is a log cabin with stone ends, which carry
            // gables of stone.
            if mixed && rooms.len() == 1 && kinds.len() > 1 {
                for x in [0, 9] {
                    ops.push(BuildOp::Fill {
                        min: [x, 0, 0],
                        max: [x, STOREY_CUBES - 1, 7],
                        cube: Some(Cube::new(kinds[1], Fill::Solid)),
                    });
                }
            }
            let ops = with_roof(ops, &parts, roof(main));
            rows.push((format!("{shape}, {name} roof"), ops));
        }
    }
    rows
}

/// How many buildings the village has.
pub const VILLAGE_HOUSES: usize = 50;

/// The bench's village (docs/building-plan.md, checking it): every roof over every plan in each of
/// a log, a stone and a hewn kind, and stone mixed with each timber, fifty, by name, each
/// roof of cubes in the roof `roof` gives its walls (D241).
pub fn bench_village(
    logs: Kind,
    stone: Kind,
    hewn: Kind,
    roof: &dyn Fn(Kind) -> Kind,
) -> Vec<(String, Vec<BuildOp>)> {
    let mut all = Vec::new();
    for (name, group, mixed) in [
        ("logs", vec![logs], false),
        ("stone", vec![stone], false),
        ("hewn", vec![hewn], false),
        ("stone and logs", vec![logs, stone], true),
        ("stone and hewn", vec![hewn, stone], true),
    ] {
        for (row, ops) in roof_rows(&group, mixed, roof) {
            all.push((format!("{name}: {row}"), ops));
        }
    }
    all.truncate(VILLAGE_HOUSES);
    all
}
