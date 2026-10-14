//! The kit sheet (docs/building-kits.md, D69, D71): with `--kit-sheet`, one of each
//! way log walls meet, laid in a row of three: built, going up and as a blueprint. Each
//! row's vantage is written to the log, so a picture of it is one command:
//!
//!   cargo run -p terra_sandbox -- --experiment play --level yard --kit-sheet \
//!     --vantage <from the log> --shot out/corner.png
//!
//! On the yard, the flat test level, to compare pieces; without `--level yard`, in the
//! world, among its trees and on its slopes, to judge them in a whole scene.
//!
//! Any model given with `--model` stands in a row after the sheet's, on the ground and lit
//! as the game is, so a piece or an experiment is judged beside what is built.
//!
//! The kit's file is read again when it is saved, so the sheet can be left running while
//! the look is tuned.

use super::{Build, BuildLook, Built, Shown, Site, frame_at, lifted, read_kits};
use crate::planet::Planet;
use crate::shell::Options;
use crate::start::FLOOR_CLEARANCE_M;
use bevy::prelude::*;
use std::sync::Arc;
use terra_build::kit::{Pattern, Profile};
use terra_build::ops::Op;
use terra_build::{
    Base, BuildOp, Cube, Facing, Fill, Kind, Panel, RoofSection, RoofStyle, STOREY_CUBES, Shape,
};
use terra_core::Operation;

/// How far apart the rows are, and the three states along each, in metres.
const ROW_M: f32 = 14.0;
const STATE_M: f32 = 11.0;
/// The first row's distance north of the start.
const FIRST_ROW_M: f32 = 14.0;
/// How far a going-up row has got.
const GOING_UP: f32 = 0.45;
/// How far a roof row going up has got: its walls up, its frame and battens on and its
/// covering going on, most of a roof's pieces being its covering.
const ROOF_GOING_UP: f32 = 0.72;

/// A row of wall tiles from `from` to `to` east, on the row `z` south, in cubes.
fn east_west(from: i32, to: i32, z: i32, kind: Kind) -> impl Iterator<Item = BuildOp> {
    (from..=to).map(move |x| wall([x, 0, z], Panel::Solid, kind))
}

/// A row of wall tiles from `from` to `to` south, on the row `x` east, in cubes.
fn north_south(from: i32, to: i32, x: i32, kind: Kind) -> impl Iterator<Item = BuildOp> {
    (from..=to).map(move |z| wall([x, 0, z], Panel::Solid, kind))
}

fn wall(tile: [i32; 3], panel: Panel, kind: Kind) -> BuildOp {
    BuildOp::SetWall {
        tile,
        panel: Some(panel),
        kind,
    }
}

/// Keen's test piece, as the Enshrouded wiki's galleries show every block in it
/// (research/role-pieces.md): a platform two cubes high reached by a flight of steps, a
/// wall with a window hole on it and a post beside, and a slab over them overhanging the
/// front; every role an edge can have. Solid, of blocks; of timber, walls round a deck, as
/// one is built, and a flight a cube wide.
fn keen(kind: Kind, solid: bool) -> Vec<BuildOp> {
    let fill = |min: [i32; 3], max: [i32; 3], fill: Fill| BuildOp::Fill {
        min,
        max,
        cube: Some(Cube::new(kind, fill)),
    };
    let mut ops = Vec::new();
    if solid {
        ops.push(fill([0, 0, 0], [7, 2, 5], Fill::Solid));
    } else {
        ops.push(fill([0, 0, 0], [7, 1, 0], Fill::Solid));
        ops.push(fill([0, 0, 5], [7, 1, 5], Fill::Solid));
        ops.push(fill([0, 0, 0], [0, 1, 5], Fill::Solid));
        ops.push(fill([7, 0, 0], [7, 1, 5], Fill::Solid));
        ops.push(fill([0, 2, 0], [7, 2, 5], Fill::Solid));
    }
    // The steps up the west side, from the front to the back, each a cube higher.
    let wide = if solid { -3 } else { -2 };
    for step in 0..7 {
        let z = 6 - step;
        ops.push(fill([wide, 0, z], [-1, step, z], Fill::Solid));
    }
    // The wall along the back, with a window a cube wide and two high, a cube from its end.
    ops.push(fill([0, 3, 0], [4, 7, 0], Fill::Solid));
    ops.push(fill([1, 5, 0], [2, 6, 0], Fill::Open));
    // A post at the back corner, and the slab over them, overhanging the steps and the
    // front.
    ops.push(fill([7, 3, 0], [7, 7, 0], Fill::Solid));
    ops.push(fill([-2, 8, -1], [8, 8, 3], Fill::Solid));
    ops
}

/// Every shape a cube can be cut to (D193), in a kind: a platform on four posts, reached by
/// a flight of stairs a metre wide and a ramp beside it, each column whole cubes under its
/// cut top; a step of halves on the ground, and halves beside the ramp's foot; and a post
/// standing alone.
fn shapes(kind: Kind) -> Vec<BuildOp> {
    let put = |min: [i32; 3], max: [i32; 3], shape: Shape| BuildOp::Fill {
        min,
        max,
        cube: Some(Cube {
            shape,
            ..Cube::new(kind, Fill::Solid)
        }),
    };
    let mut ops = Vec::new();
    for [x, z] in [[0, 0], [5, 0], [0, 3], [5, 3]] {
        ops.push(put([x, 0, z], [x, 3, z], Shape::Post));
    }
    ops.push(put([0, 4, 0], [5, 4, 3], Shape::Whole));
    // Up from the south, rising north onto the platform.
    for step in 0..5 {
        let z = 8 - step;
        if step > 0 {
            ops.push(put([0, 0, z], [3, step - 1, z], Shape::Whole));
        }
        ops.push(put([0, step, z], [1, step, z], Shape::Stair(Facing::North)));
        ops.push(put([2, step, z], [3, step, z], Shape::Slope(Facing::North)));
    }
    ops.push(put([-4, 0, 4], [-2, 0, 8], Shape::Half));
    // A run of halves beside the foot of the ramp, so its side shows over them.
    ops.push(put([4, 0, 6], [4, 0, 8], Shape::Half));
    ops.push(put([7, 0, 6], [7, 5, 6], Shape::Post));
    ops
}

/// The shapes of D231, in a kind: a mound hipped all round, slopes along its sides and
/// outer corners at its corners; an L hipped the same, an inner corner where its arms meet;
/// and a wall with a doorway under a lintel of upper halves, corbelled out at its top on
/// inverted slopes to the north and inverted stairs to the south under a coping a cube
/// wider each side.
fn more_shapes(kind: Kind) -> Vec<BuildOp> {
    let put = |min: [i32; 3], max: [i32; 3], shape: Shape| BuildOp::Fill {
        min,
        max,
        cube: Some(Cube {
            shape,
            ..Cube::new(kind, Fill::Solid)
        }),
    };
    use Facing::{East, North, South, West};
    let mut ops = vec![
        put([0, 0, 0], [5, 0, 5], Shape::Whole),
        put([1, 1, 1], [4, 1, 4], Shape::Whole),
        put([1, 1, 0], [4, 1, 0], Shape::Slope(South)),
        put([1, 1, 5], [4, 1, 5], Shape::Slope(North)),
        put([0, 1, 1], [0, 1, 4], Shape::Slope(East)),
        put([5, 1, 1], [5, 1, 4], Shape::Slope(West)),
        put([0, 1, 0], [0, 1, 0], Shape::OuterCorner(East)),
        put([5, 1, 0], [5, 1, 0], Shape::OuterCorner(South)),
        put([5, 1, 5], [5, 1, 5], Shape::OuterCorner(West)),
        put([0, 1, 5], [0, 1, 5], Shape::OuterCorner(North)),
    ];
    // An L two cubes wide, hipped: slopes either side of each arm meeting at a ridge,
    // outer corners at its ends and an inner corner, a valley's, where the arms meet.
    ops.push(put([7, 0, 0], [11, 0, 1], Shape::Whole));
    ops.push(put([7, 0, 2], [8, 0, 4], Shape::Whole));
    for (min, max, shape) in [
        ([8, 1, 0], [10, 1, 0], Shape::Slope(South)),
        ([9, 1, 1], [10, 1, 1], Shape::Slope(North)),
        ([7, 1, 1], [7, 1, 3], Shape::Slope(East)),
        ([8, 1, 2], [8, 1, 3], Shape::Slope(West)),
        ([7, 1, 0], [7, 1, 0], Shape::OuterCorner(East)),
        ([11, 1, 0], [11, 1, 0], Shape::OuterCorner(South)),
        ([11, 1, 1], [11, 1, 1], Shape::OuterCorner(West)),
        ([7, 1, 4], [7, 1, 4], Shape::OuterCorner(North)),
        ([8, 1, 4], [8, 1, 4], Shape::OuterCorner(West)),
        ([8, 1, 1], [8, 1, 1], Shape::InnerCorner(West)),
    ] {
        ops.push(put(min, max, shape));
    }
    // The wall, its doorway a metre wide, its lintel upper halves.
    ops.push(put([12, 0, 4], [19, 4, 4], Shape::Whole));
    ops.push(BuildOp::Fill {
        min: [14, 0, 4],
        max: [15, 3, 4],
        cube: None,
    });
    ops.push(put([14, 3, 4], [15, 3, 4], Shape::UpperHalf));
    ops.push(put([12, 4, 3], [19, 4, 3], Shape::InvertedSlope(South)));
    ops.push(put([12, 4, 5], [19, 4, 5], Shape::InvertedStair(North)));
    ops.push(put([12, 5, 3], [19, 5, 5], Shape::Whole));
    ops
}

/// A floor laid alone on the ground, with no walls: its edges finished as they are.
fn floor_alone(kind: Kind) -> Vec<BuildOp> {
    vec![BuildOp::SetFloor {
        min: [0, 0, 0],
        max: [7, 0, 5],
        laid: true,
        kind,
    }]
}

/// Each row: its name, and the operations that make it, in cubes of half a metre. Laid
/// in `kind`, with rows mixing in `stone` where there is a stone kit (D179).
fn rows(kind: Kind, stone: Option<Kind>, hewn: Option<Kind>) -> Vec<(&'static str, Vec<BuildOp>)> {
    let cabin = {
        let mut ops = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind,
        }];
        // A door a metre wide in the south wall, a window beside it, and one in the east
        // wall.
        ops.extend((3..=4).map(|x| wall([x, 0, 7], Panel::Door, kind)));
        ops.extend((6..=7).map(|x| wall([x, 0, 7], Panel::WINDOW, kind)));
        ops.extend((2..=4).map(|z| wall([9, 0, z], Panel::WINDOW, kind)));
        ops.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        ops
    };
    // A room five metres by four under a roof of a style, with a door.
    let roofed = |style: RoofStyle, along_x: bool| {
        let mut ops = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind,
        }];
        ops.extend((4..=5).map(|x| wall([x, 0, 7], Panel::Door, kind)));
        ops.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style,
            along_x,
            pitch: if style == RoofStyle::LeanTo { 0.4 } else { 0.7 },
        }));
        ops
    };
    // A wall with a hole carved in it, a cube at a time: a cross, and a slot.
    let carved = {
        let mut ops: Vec<BuildOp> = east_west(0, 9, 0, kind).collect();
        let open = |x: i32, layer: i32| BuildOp::Fill {
            min: [x, layer, 0],
            max: [x, layer, 0],
            cube: Some(Cube::new(kind, Fill::Open)),
        };
        ops.extend([open(2, 3), open(3, 2), open(3, 3), open(3, 4), open(4, 3)]);
        ops.extend((6..=8).map(|x| open(x, 1)));
        ops
    };
    let mut mixed = Vec::new();
    if let Some(stone) = stone {
        // A house all of stone under a hip roof, with a doorway and windows.
        let mut house = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind: stone,
        }];
        house.extend((4..=5).map(|x| wall([x, 0, 7], Panel::Door, stone)));
        house.extend(
            (1..=2)
                .chain(7..=8)
                .map(|x| wall([x, 0, 7], Panel::WINDOW, stone)),
        );
        house.extend((3..=4).map(|z| wall([0, 0, z], Panel::WINDOW, stone)));
        house.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style: RoofStyle::Hip,
            along_x: true,
            pitch: 0.7,
        }));
        // The cabin on a stone plinth a metre high: one wall over another.
        let mut plinth = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind,
        }];
        let stone_cube = Some(Cube::new(stone, Fill::Solid));
        plinth.extend((6..=7).map(|x| wall([x, 0, 7], Panel::WINDOW, kind)));
        plinth.extend((2..=4).map(|z| wall([9, 0, z], Panel::WINDOW, kind)));
        // After the windows, which set their whole column.
        for (min, max) in [
            ([0, 0, 0], [9, 1, 0]),
            ([0, 0, 7], [9, 1, 7]),
            ([0, 0, 0], [0, 1, 7]),
            ([9, 0, 0], [9, 1, 7]),
        ] {
            plinth.push(BuildOp::Fill {
                min,
                max,
                cube: stone_cube,
            });
        }
        plinth.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        // Stone and logs meeting every way walls meet.
        let column = |x: i32, z: i32| BuildOp::Fill {
            min: [x, 0, z],
            max: [x, STOREY_CUBES - 1, z],
            cube: stone_cube,
        };
        let ell: Vec<BuildOp> = east_west(0, 5, 0, kind)
            .chain(north_south(1, 5, 0, stone))
            .collect();
        let tee_stone: Vec<BuildOp> = east_west(0, 9, 0, stone)
            .chain(north_south(1, 5, 4, kind))
            .collect();
        let tee_logs: Vec<BuildOp> = east_west(0, 9, 0, kind)
            .chain(north_south(1, 5, 4, stone))
            .collect();
        let in_line: Vec<BuildOp> = east_west(0, 4, 0, kind)
            .chain(east_west(5, 9, 0, stone))
            .collect();
        let mut pillars: Vec<BuildOp> = east_west(0, 11, 0, kind).collect();
        pillars.extend([0, 4, 8, 11].map(|x| column(x, 0)));
        // A log cabin with its east end, under the gable, of stone, as a chimney end.
        let mut end = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind,
        }];
        end.extend((3..=4).map(|x| wall([x, 0, 7], Panel::Door, kind)));
        end.extend((0..=7).map(|z| column(9, z)));
        end.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        mixed = vec![
            ("Keen's piece in stone", keen(stone, true)),
            ("shapes in stone", shapes(stone)),
            ("floor alone in stone", floor_alone(stone)),
            ("stone house", house),
            ("stone plinth", plinth),
            ("stone and logs: L", ell),
            ("stone and logs: T, stone through", tee_stone),
            ("stone and logs: T, logs through", tee_logs),
            ("stone and logs: in line", in_line),
            ("stone pillars in logs", pillars),
            ("stone end", end),
        ];
    }
    let mut rows = vec![
        ("Keen's piece", keen(kind, false)),
        ("shapes in logs", shapes(kind)),
        ("floor alone in logs", floor_alone(kind)),
        ("free end", east_west(0, 5, 0, kind).collect()),
        (
            "corner",
            east_west(0, 5, 0, kind)
                .chain(north_south(1, 5, 0, kind))
                .collect(),
        ),
        (
            "T",
            east_west(0, 9, 0, kind)
                .chain(north_south(1, 5, 4, kind))
                .collect(),
        ),
        (
            "cross",
            east_west(0, 9, 4, kind)
                .chain(north_south(0, 9, 4, kind))
                .collect(),
        ),
        ("long wall", east_west(0, 19, 0, kind).collect()),
        (
            "posts",
            [0, 3, 6]
                .map(|x| wall([x, 0, 0], Panel::Solid, kind))
                .to_vec(),
        ),
        ("carved", carved),
        ("cabin", cabin),
        ("hip roof", roofed(RoofStyle::Hip, true)),
        ("lean-to", roofed(RoofStyle::LeanTo, true)),
    ];
    rows.extend(mixed);
    if let Some(hewn) = hewn {
        rows.push(("Keen's piece in hewn timber", keen(hewn, false)));
        rows.push(("shapes in hewn timber", shapes(hewn)));
    }
    // The shapes of D231 in each kind, after the rest so those keep their places.
    rows.push(("more shapes in logs", more_shapes(kind)));
    if let Some(stone) = stone {
        rows.push(("more shapes in stone", more_shapes(stone)));
    }
    if let Some(hewn) = hewn {
        rows.push(("more shapes in hewn timber", more_shapes(hewn)));
    }
    rows
}

/// A room five metres by four in a kind under a gable, with a doorway and windows.
fn house(kind: Kind) -> Vec<BuildOp> {
    let mut ops = vec![BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [9, 0, 7],
        kind,
    }];
    ops.extend((4..=5).map(|x| wall([x, 0, 7], Panel::Door, kind)));
    ops.extend((1..=2).map(|x| wall([x, 0, 7], Panel::WINDOW, kind)));
    ops.extend((3..=4).map(|z| wall([0, 0, z], Panel::WINDOW, kind)));
    ops.extend((2..=3).map(|x| wall([x, 0, 0], Panel::WINDOW, kind)));
    ops.push(BuildOp::AddRoof(RoofSection {
        min: [0, 0, 0],
        max: [9, 0, 7],
        style: RoofStyle::Gable,
        along_x: true,
        pitch: 0.75,
    }));
    ops
}

/// A further kit's rows (D236), named for it: how its walls meet, its openings, carved,
/// a house, its shapes, and mixed with stone and logs every way they meet, after the rest.
fn kit_rows(name: &str, kind: Kind, stone: Kind, logs: Kind) -> Vec<(String, Vec<BuildOp>)> {
    let stone_cube = Some(Cube::new(stone, Fill::Solid));
    let carved = {
        let mut ops: Vec<BuildOp> = east_west(0, 9, 0, kind).collect();
        let open = |x: i32, layer: i32| BuildOp::Fill {
            min: [x, layer, 0],
            max: [x, layer, 0],
            cube: Some(Cube::new(kind, Fill::Open)),
        };
        ops.extend([open(2, 3), open(3, 2), open(3, 3), open(3, 4), open(4, 3)]);
        ops.extend((6..=8).map(|x| open(x, 1)));
        ops
    };
    let openings = {
        let mut ops: Vec<BuildOp> = east_west(0, 11, 0, kind).collect();
        ops.extend((2..=3).map(|x| wall([x, 0, 0], Panel::Door, kind)));
        ops.extend((7..=8).map(|x| wall([x, 0, 0], Panel::WINDOW, kind)));
        ops
    };
    // On a stone plinth a metre high, the doorway's left open.
    let mut plinth = house(kind);
    for (min, max) in [
        ([0, 0, 0], [9, 1, 0]),
        ([0, 0, 7], [3, 1, 7]),
        ([6, 0, 7], [9, 1, 7]),
        ([0, 0, 0], [0, 1, 7]),
        ([9, 0, 0], [9, 1, 7]),
    ] {
        plinth.push(BuildOp::Fill {
            min,
            max,
            cube: stone_cube,
        });
    }
    // A stone ground floor with a storey of it over, a floor of timber between.
    let mut storeys = vec![
        BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind: stone,
        },
        BuildOp::AddRoom {
            min: [0, 1, 0],
            max: [9, 1, 7],
            kind,
        },
        BuildOp::SetFloor {
            min: [1, 1, 1],
            max: [8, 1, 6],
            laid: true,
            kind: logs,
        },
    ];
    storeys.extend((4..=5).map(|x| wall([x, 0, 7], Panel::Door, stone)));
    storeys.extend((1..=2).map(|x| wall([x, 1, 7], Panel::WINDOW, kind)));
    storeys.extend((7..=8).map(|x| wall([x, 1, 7], Panel::WINDOW, kind)));
    storeys.push(BuildOp::AddRoof(RoofSection {
        min: [0, 1, 0],
        max: [9, 1, 7],
        style: RoofStyle::Gable,
        along_x: true,
        pitch: 0.75,
    }));
    let ell = |other: Kind| -> Vec<BuildOp> {
        east_west(0, 5, 0, kind)
            .chain(north_south(1, 5, 0, other))
            .collect()
    };
    let tee = |through: Kind, stem: Kind| -> Vec<BuildOp> {
        east_west(0, 9, 0, through)
            .chain(north_south(1, 5, 4, stem))
            .collect()
    };
    let in_line = |other: Kind| -> Vec<BuildOp> {
        east_west(0, 4, 0, kind)
            .chain(east_west(5, 9, 0, other))
            .collect()
    };
    [
        ("free end", east_west(0, 9, 0, kind).collect()),
        (
            "corner",
            east_west(0, 5, 0, kind)
                .chain(north_south(1, 5, 0, kind))
                .collect(),
        ),
        ("T", tee(kind, kind)),
        ("openings", openings),
        ("carved", carved),
        ("house", house(kind)),
        ("shapes", shapes(kind)),
        ("on a stone plinth", plinth),
        ("over a stone ground floor", storeys),
        ("and stone: L", ell(stone)),
        ("and stone: T, stone through", tee(stone, kind)),
        ("and stone: in line", in_line(stone)),
        ("and logs: L", ell(logs)),
        ("and logs: T, logs through", tee(logs, kind)),
        ("and logs: in line", in_line(logs)),
    ]
    .into_iter()
    .map(|(row, ops)| (format!("{name}: {row}"), ops))
    .collect()
}

/// A row whose three columns are each a state its cubes are in, rather than how far its
/// pieces are in: the shortcuts' cubes, then each column's operations on them, drawn by what
/// each cube holds.
struct Acted {
    name: &'static str,
    ops: Vec<BuildOp>,
    columns: [Vec<Op>; 3],
    /// Set on ground falling away to the north, the yard being level, and seen from there.
    sloped: bool,
    /// Where its vantage's eye is and what it looks at, from the middle state's corner on
    /// the ground, in metres east, up and south.
    view: [Vec3; 2],
}

/// The rows of what stands between a building and the ground, its floors and its openings
/// (construction-001's ground, floor and opening cases): a cabin on a slope, on stone
/// footings down to the ground, with stone steps from its door; floors of stone and of
/// timber, open to the sky; a window cut into a built wall, a course at a time; and a door,
/// shut and open.
fn acted_rows(kind: Kind, stone: Kind) -> Vec<Acted> {
    let room = |kind: Kind| BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [9, 0, 7],
        kind,
    };
    let gable = BuildOp::AddRoof(RoofSection {
        min: [0, 0, 0],
        max: [9, 0, 7],
        style: RoofStyle::Gable,
        along_x: true,
        pitch: 0.7,
    });
    let roofless = BuildOp::SetRoof {
        min: [1, 0, 1],
        max: [8, 0, 6],
        laid: false,
        kind,
    };
    let floor_of = |kind: Kind| BuildOp::SetFloor {
        min: [1, 0, 1],
        max: [8, 0, 6],
        laid: true,
        kind,
    };
    // A cabin on a slope, its door to the north, downhill.
    let mut slope = vec![room(kind)];
    slope.extend((3..=4).map(|x| wall([x, 0, 0], Panel::Door, kind)));
    slope.extend((6..=7).map(|x| wall([x, 0, 0], Panel::WINDOW, kind)));
    slope.push(gable);
    // Floors open to the sky, the walls a metre high so they are seen.
    let low_walls = |ops: &mut Vec<BuildOp>| {
        ops.extend([2, 3, 4, 5].map(|layer| BuildOp::Fill {
            min: [0, layer, 0],
            max: [9, layer, 7],
            cube: None,
        }));
    };
    let mut stone_floor = vec![room(kind), roofless];
    stone_floor.push(BuildOp::SetFloor {
        min: [1, 0, 1],
        max: [8, 0, 6],
        laid: false,
        kind,
    });
    stone_floor.push(floor_of(stone));
    low_walls(&mut stone_floor);
    let mut timber_floor = vec![room(kind), roofless];
    low_walls(&mut timber_floor);
    // And a deck before it, out from its south wall.
    timber_floor.push(BuildOp::SetFloor {
        min: [2, 0, 8],
        max: [7, 0, 10],
        laid: true,
        kind,
    });
    // A window cut into a log wall already built: marked, half out, and open.
    let wall_ops: Vec<BuildOp> = east_west(0, 9, 0, kind).collect();
    let window: Vec<[i32; 3]> = [4, 5]
        .into_iter()
        .flat_map(|x| [2, 3].map(|y| [x, y, 0]))
        .collect();
    let top: Vec<[i32; 3]> = window.iter().copied().filter(|at| at[1] == 3).collect();
    let low: Vec<[i32; 3]> = window.iter().copied().filter(|at| at[1] == 2).collect();
    let marked = vec![Op::Carve {
        cubes: window.clone(),
    }];
    let mut half = marked.clone();
    half.push(Op::Empty { cubes: top });
    let mut cut = half.clone();
    cut.push(Op::Empty { cubes: low });
    cut.push(Op::Open {
        cubes: window.clone(),
    });
    vec![
        Acted {
            name: "stone floor",
            ops: stone_floor,
            columns: Default::default(),
            sloped: false,
            view: [Vec3::new(3.5, 6.0, 9.0), Vec3::new(2.5, 0.0, 2.0)],
        },
        Acted {
            name: "timber floor",
            ops: timber_floor,
            columns: Default::default(),
            sloped: false,
            view: [Vec3::new(3.5, 4.5, 10.5), Vec3::new(2.5, 0.0, 3.0)],
        },
        Acted {
            name: "window cut into a built wall",
            ops: wall_ops,
            columns: [marked, half, cut],
            sloped: false,
            view: [Vec3::new(2.5, 2.4, 9.0), Vec3::new(2.5, 1.3, 0.0)],
        },
        Acted {
            name: "stone changed to logs in place",
            ops: east_west(0, 9, 0, stone).collect(),
            columns: changed(kind),
            sloped: false,
            view: [Vec3::new(2.5, 2.4, 9.0), Vec3::new(2.5, 1.3, 0.0)],
        },
        // Last, as the ground cut away falls north beyond it.
        Acted {
            name: "cabin on a slope",
            ops: slope,
            columns: Default::default(),
            sloped: true,
            view: [Vec3::new(9.0, 5.0, -12.0), Vec3::new(2.5, -0.5, 2.0)],
        },
    ]
}

/// A built wall's cubes changed to another kind in place (D232), a block of them two wide
/// and two high a metre up, as the hammer's Change material does it: marked; the old taken
/// out, swapped and a course of the new in; and all of the new in.
fn changed(to: Kind) -> [Vec<Op>; 3] {
    let block: Vec<[i32; 3]> = [4, 5]
        .into_iter()
        .flat_map(|x| [2, 3].map(|y| [x, y, 0]))
        .collect();
    let layer =
        |y: i32| -> Vec<[i32; 3]> { block.iter().copied().filter(|at| at[1] == y).collect() };
    let marked = vec![Op::Replace {
        cubes: block.clone(),
        kind: to,
        part: Some(terra_build::PartId(900)),
    }];
    let mut going = marked.clone();
    going.extend([
        Op::Empty { cubes: layer(3) },
        Op::Empty { cubes: layer(2) },
        Op::Swap {
            cubes: block.clone(),
        },
        Op::Fill { cubes: layer(2) },
    ]);
    let mut done = going.clone();
    done.extend([
        Op::Fill { cubes: layer(2) },
        Op::Fill { cubes: layer(3) },
        Op::Fill { cubes: layer(3) },
        Op::Cancel { cubes: block },
    ]);
    [marked, going, done]
}

/// The ground under the yard cut away into a slope falling north from a row's south edge,
/// a metre for every four, as a cabin on a hillside stands.
fn slope(frame: &Transform) -> Operation {
    let fall = libm::atanf(0.25);
    let (half, south) = (Vec3::new(60.0, 6.0, 8.0), 4.6);
    let turn = Quat::from_rotation_x(-fall);
    // What is over the slope is dug: the box's underside is the slope, its south edge on
    // the ground at the row's south edge.
    let edge = Vec3::new(0.0, 0.0, south);
    let centre = edge - turn * Vec3::new(0.0, -half.y, half.z);
    Operation::DigBox {
        centre: frame.transform_point(centre),
        half,
        rotation: frame.rotation * turn,
    }
}

/// How far east of the walls' rows each kind's roof rows are, a kind after another.
const ROOF_GROUP_M: f32 = 40.0;

/// Lays the sheet once the site is there.
#[allow(clippy::too_many_arguments)]
pub(super) fn lay(
    mut done: Local<bool>,
    options: Res<Options>,
    site: Option<Res<Site>>,
    look: Option<Res<BuildLook>>,
    mut planet: ResMut<Planet>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<AssetServer>,
    rules: Option<Res<super::act::BuildRules>>,
) {
    if *done || !options.kit_sheet {
        return;
    }
    let (Some(site), Some(look)) = (site, look) else {
        return;
    };
    *done = true;
    let Some(kits) = read_kits().map(Arc::new) else {
        return;
    };
    let laying = kits.0.first().map_or(Kind::PLAIN, |kit| kit.kind());
    let stone = kits.footing().map(|kit| kit.kind());
    // How high the ground is at a place, in the start's own space, the yard being round:
    // what a vantage near it needs to stand on it.
    let ground_at = |planet: &Planet, east: f32, north: f32| {
        let point = site.frame.transform_point(Vec3::new(east, 0.0, north));
        let ground = point - point.normalize() * planet.distance(point);
        (
            ground,
            site.frame
                .compute_affine()
                .inverse()
                .transform_point3(ground)
                .y,
        )
    };
    // The walls' rows, then each kind's roof rows to the east of them, a group a kind.
    let hewn = kits
        .0
        .iter()
        .find(|kit| kit.pattern == Pattern::Logs && kit.timber.profile == Profile::Hewn)
        .map(|kit| kit.kind());
    let mut placed: Vec<(String, Vec<BuildOp>, f32, usize)> = rows(laying, stone, hewn)
        .into_iter()
        .enumerate()
        .map(|(row, (name, ops))| (name.to_string(), ops, 0.0, row))
        .collect();
    // The further kits' rows (D236), each by its file's name, after the rest.
    if let Some(stone) = stone {
        let names = super::kit_names();
        for name in ["earth", "wattle"] {
            let Some((&kind, _)) = names.iter().find(|(_, n)| n.as_str() == name) else {
                continue;
            };
            for (row_name, ops) in kit_rows(name, kind, stone, laying) {
                let row = placed.len();
                placed.push((row_name, ops, 0.0, row));
            }
        }
    }
    let count = placed.len();
    let groups = [
        ("logs", vec![laying], false),
        ("stone", stone.into_iter().collect(), false),
        ("hewn", hewn.into_iter().collect(), false),
        (
            "stone and logs",
            stone
                .into_iter()
                .map(|s| [laying, s])
                .next()
                .unwrap_or([laying; 2])
                .to_vec(),
            true,
        ),
    ];
    for (group, (kit_name, kinds, mixed)) in groups.into_iter().enumerate() {
        if kinds.is_empty() {
            continue;
        }
        for (row, (name, ops)) in terra_build::roof_rows(&kinds, mixed)
            .into_iter()
            .enumerate()
        {
            let east = ROOF_GROUP_M * (group + 1) as f32;
            placed.push((format!("{kit_name}: {name}"), ops, east, row));
        }
    }
    for (name, ops, group_east, row) in placed {
        // Only the rows asked for, each where it always is.
        let wanted = &options.sheet_rows;
        if !wanted.is_empty() && !wanted.iter().any(|text| name.contains(text.as_str())) {
            continue;
        }
        let north = -FIRST_ROW_M - ROW_M * row as f32;
        for (column, shown) in [
            Shown::Built,
            Shown::Plan(if group_east > 0.0 {
                ROOF_GOING_UP
            } else {
                GOING_UP
            }),
            Shown::Plan(0.0),
        ]
        .into_iter()
        .enumerate()
        {
            let east = group_east + STATE_M * column as f32;
            // On the ground there, the yard being round.
            let point = site.frame.transform_point(Vec3::new(east, 0.0, north));
            let ground = point - point.normalize() * planet.distance(point);
            let mut base = Base::default();
            for &op in &ops {
                base.apply(op);
            }
            let frame = frame_at(ground + ground.normalize() * FLOOR_CLEARANCE_M, &site.frame);
            // Walls with no floor stand on footings in the ground instead.
            let frame = if base.floors().next().is_some() {
                lifted(frame, &base, &planet)
            } else {
                frame
            };
            let label = format!("Kit sheet: {name}, {shown:?}");
            let mut build = Build::new(&mut commands, &look, frame, &label);
            build.kits = Some(kits.clone());
            build.shown = shown;
            build.replace_with(&ops, &mut commands, &planet, &mut meshes);
            commands.spawn((Name::new(label), Built(build)));
        }
        let (_, height) = ground_at(&planet, group_east + STATE_M, north);
        info!(
            "Kit sheet: {name}: --vantage {},{},{},{},{},{}",
            group_east + STATE_M + 2.0,
            height + 9.0,
            north + 22.0,
            group_east + STATE_M + 2.0,
            height + 0.5,
            north + 1.5
        );
    }
    // Rows of cubes in states of their own, after them.
    let acted = stone
        .map(|stone| acted_rows(laying, stone))
        .unwrap_or_default();
    let rules = rules.map(|rules| rules.rules.clone()).unwrap_or_default();
    for (index, row) in acted.iter().enumerate() {
        let north = -FIRST_ROW_M - ROW_M * (count + index) as f32;
        let row_frame = frame_at(
            {
                let point = site.frame.transform_point(Vec3::new(0.0, 0.0, north));
                point - point.normalize() * planet.distance(point)
            },
            &site.frame,
        );
        if row.sloped && options.yard() {
            planet.apply(&slope(&row_frame));
        }
        for (column, acts) in row.columns.iter().enumerate() {
            let east = STATE_M * column as f32;
            let point = site.frame.transform_point(Vec3::new(east, 0.0, north));
            let ground = point - point.normalize() * planet.distance(point);
            let mut base = Base::default();
            for &op in &row.ops {
                base.apply(op);
            }
            let frame = frame_at(ground + ground.normalize() * FLOOR_CLEARANCE_M, &site.frame);
            // Walls with no floor stand on footings in the ground instead.
            let frame = if base.floors().next().is_some() {
                lifted(frame, &base, &planet)
            } else {
                frame
            };
            let label = format!("Kit sheet: {}, {column}", row.name);
            let mut build = Build::new(&mut commands, &look, frame, &label);
            build.kits = Some(kits.clone());
            build.by_cubes = true;
            build.replace_with(&row.ops, &mut commands, &planet, &mut meshes);
            let cx = terra_build::ops::Context::free(&rules);
            if let Err(why) = build.base.act_all(acts, &cx) {
                warn!("Kit sheet: {}: {why:?}", row.name);
            }
            // What is marked to be carved out and not yet begun is lit, as the next work.
            build.next = build
                .base
                .cubes()
                .filter(|(_, cube)| {
                    (cube.pending == terra_build::Pending::Carve && cube.is_built())
                        || (cube.pending.changing() && !cube.is_built())
                })
                .map(|(at, _)| at.to_array())
                .collect();
            build.revision += 1;
            commands.spawn((Name::new(label), Built(build)));
        }
        let (_, height) = ground_at(&planet, STATE_M, north);
        let corner = Vec3::new(STATE_M, height, north);
        let (eye, at) = (corner + row.view[0], corner + row.view[1]);
        info!(
            "Kit sheet: {}: --vantage {},{},{},{},{},{}",
            row.name, eye.x, eye.y, eye.z, at.x, at.y, at.z
        );
    }
    let count = count + acted.len();
    // Models to judge beside the sheet, each in a row of its own after it.
    for (index, path) in options.models.iter().enumerate() {
        let (east, north) = (0.0, -FIRST_ROW_M - ROW_M * (count + index) as f32);
        let (ground, height) = ground_at(&planet, east, north);
        commands.spawn((
            Name::new(format!("Kit sheet: model {path}")),
            frame_at(ground, &site.frame),
            Visibility::default(),
            bevy::world_serialization::WorldAssetRoot(
                assets.load(GltfAssetLabel::Scene(0).from_asset(path.clone())),
            ),
        ));
        info!(
            "Kit sheet: model {path}: --vantage {},{},{},{},{},{}",
            east + 2.0,
            height + 2.2,
            north + 7.0,
            east + 2.0,
            height + 1.0,
            north
        );
    }
}
