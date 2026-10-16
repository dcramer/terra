//! The kit sheet's rows (docs/building-kits.md, D69, D71): every way walls, roofs, floors
//! and openings meet, as bases of shortcut operations in cubes, each named, and a
//! material's preview (D349). The game lays them in rows on the yard (`--kit-sheet`,
//! `terra_game::build::sheet`), placing and lighting them; what each row is lives here,
//! with the other scenes, so tests and tools can build it without Bevy.

use crate::kit::{Kit, Pattern};
use crate::ops::Op;
use crate::roofer::Style;
use crate::{Base, BuildOp, Cube, Facing, Fill, Kind, Panel, STOREY_CUBES, Shape};
use glam::{IVec3, Vec3};

/// A roof's parts, as [`super::example::roof_over`] takes them.
type Parts = [([i32; 2], [i32; 2], Style, bool)];

fn roof_over(ops: &[BuildOp], storey: i32, parts: &Parts, kind: Kind) -> Vec<BuildOp> {
    super::example::roof_over(ops, storey, &parts.to_vec(), kind)
}

fn with_roof(ops: Vec<BuildOp>, parts: &Parts, kind: Kind) -> Vec<BuildOp> {
    super::example::with_roof(ops, &parts.to_vec(), kind)
}

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

/// Timber more than a cube thick, which lays as skins of logs along its faces, crossing only
/// at its ends: a wall two cubes thick and one three, a storey high, and piers two cubes
/// square and three by two.
fn thick(kind: Kind) -> Vec<BuildOp> {
    let block = |min: [i32; 3], max: [i32; 3]| BuildOp::Fill {
        min,
        max,
        cube: Some(Cube::new(kind, Fill::Solid)),
    };
    let top = STOREY_CUBES - 1;
    vec![
        block([0, 0, 0], [9, top, 1]),
        block([0, 0, 4], [9, top, 6]),
        block([12, 0, 0], [13, top, 1]),
        block([12, 0, 4], [14, top, 5]),
    ]
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
/// in `kind`, with rows mixing in `stone` where there is a stone kit (D179); each roof in
/// the roof `roof` gives the walls under it.
pub fn rows(
    kind: Kind,
    stone: Option<Kind>,
    hewn: Option<Kind>,
    roof: &dyn Fn(Kind) -> Kind,
) -> Vec<(&'static str, Vec<BuildOp>)> {
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
        with_roof(ops, &[([0, 0], [9, 7], Style::Gable, true)], roof(kind))
    };
    // A room five metres by four under a roof of a style, with a door.
    let roofed = |style: Style, along_x: bool| {
        let mut ops = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind,
        }];
        ops.extend((4..=5).map(|x| wall([x, 0, 7], Panel::Door, kind)));
        with_roof(ops, &[([0, 0], [9, 7], style, along_x)], roof(kind))
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
    let mixed = stone
        .map(|stone| mixed_rows(kind, stone, roof))
        .unwrap_or_default();
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
        ("thick walls and piers", thick(kind)),
        (
            "posts",
            [0, 3, 6]
                .map(|x| wall([x, 0, 0], Panel::Solid, kind))
                .to_vec(),
        ),
        ("carved", carved),
        ("cabin", cabin),
        ("hip roof", roofed(Style::Hip, true)),
        ("lean-to", roofed(Style::LeanTo, true)),
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
    // Decks on posts and their stairs (D306), in each timber kind and on stone piers.
    if let Some(stone) = stone {
        rows.extend(deck_rows(kind, stone, hewn));
    }
    rows
}

/// The rows mixing a stone kind with the kind laid: Keen's piece, the shapes and a floor in
/// stone, a stone house, the cabin on a stone plinth, stone and logs meeting every way walls
/// meet, stone pillars in a log wall, and a log cabin with a stone end.
fn mixed_rows(
    kind: Kind,
    stone: Kind,
    roof: &dyn Fn(Kind) -> Kind,
) -> Vec<(&'static str, Vec<BuildOp>)> {
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
    let house = with_roof(house, &[([0, 0], [9, 7], Style::Hip, true)], roof(stone));
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
    let plinth = with_roof(plinth, &[([0, 0], [9, 7], Style::Gable, true)], roof(kind));
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
    let end = with_roof(end, &[([0, 0], [9, 7], Style::Gable, true)], roof(kind));
    vec![
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
    ]
}

/// Decks on posts and their stairs (D306), in each timber kind and on stone piers.
fn deck_rows(kind: Kind, stone: Kind, hewn: Option<Kind>) -> Vec<(&'static str, Vec<BuildOp>)> {
    let mut out = Vec::new();
    let [on_posts, nine_and_l, against_and_high] = decks(kind, kind, stone);
    out.push(("decks on posts in logs", on_posts));
    out.push(("decks: nine posts and an L in logs", nine_and_l));
    out.push((
        "decks: against a wall, two storeys up in logs",
        against_and_high,
    ));
    if let Some(hewn) = hewn {
        let [on_posts, nine_and_l, against_and_high] = decks(hewn, hewn, stone);
        out.push(("decks on posts in hewn timber", on_posts));
        out.push(("decks: nine posts and an L in hewn timber", nine_and_l));
        out.push((
            "decks: against a wall, two storeys up in hewn timber",
            against_and_high,
        ));
        let [piers, ..] = decks(hewn, stone, stone);
        let [logs_piers, ..] = decks(kind, stone, stone);
        let mut on_piers = logs_piers;
        // The hewn decks beside the log ones, a row's width east.
        on_piers.extend(piers.into_iter().map(|op| match op {
            BuildOp::Fill { min, max, cube } => BuildOp::Fill {
                min: [min[0], min[1], min[2] + 14],
                max: [max[0], max[1], max[2] + 14],
                cube,
            },
            other => other,
        }));
        out.push(("decks on stone piers", on_piers));
    }
    out
}

/// Decks on posts as a player lays them (D306), each on a stone footing a cube into the
/// ground under every post, in three rows: on four posts and on six, a storey up, posts the
/// hammer's height into the deck's layer, stairs up the south side of one and the east of
/// the other; on nine posts and an L, posts laid by hand under the deck, stairs from the
/// north and the south; against a wall two storeys high, posts only on its open side, and
/// a deck two storeys up, stairs up each from the south.
fn decks(deck: Kind, posts: Kind, footing: Kind) -> [Vec<BuildOp>; 3] {
    let put = |min: [i32; 3], max: [i32; 3], kind: Kind, shape: Shape| BuildOp::Fill {
        min,
        max,
        cube: Some(Cube {
            shape,
            ..Cube::new(kind, Fill::Solid)
        }),
    };
    // Posts at columns, up to the layer `top`, each on a footing.
    let posted = |ops: &mut Vec<BuildOp>, at: &[[i32; 2]], top: i32| {
        for &[x, z] in at {
            ops.push(put([x, -1, z], [x, -1, z], footing, Shape::Whole));
            ops.push(put([x, 0, z], [x, top, z], posts, Shape::Post));
        }
    };
    // A deck over a box of columns on a layer, around posts already in it.
    let floor = |ops: &mut Vec<BuildOp>, min: [i32; 2], max: [i32; 2], layer: i32| {
        for x in min[0]..=max[0] {
            for z in min[1]..=max[1] {
                if !ops.iter().any(|op| {
                    matches!(op, BuildOp::Fill { min: m, max: n, .. }
                        if m[0] == x && m[2] == z && n[1] >= layer && m[1] <= layer)
                }) {
                    ops.push(put([x, layer, z], [x, layer, z], deck, Shape::Whole));
                }
            }
        }
    };
    // A flight two columns wide from `foot` rising toward `facing` to the layer `top`, on a
    // footing.
    let flight = |ops: &mut Vec<BuildOp>, foot: [i32; 2], facing: Facing, top: i32| {
        let (forward, across) = (facing.step(), facing.turned(1).step());
        for i in 0..=top {
            for w in 0..2 {
                let at = IVec3::new(foot[0], 0, foot[1]) + forward * i + across * w;
                ops.push(put(
                    [at.x, -1, at.z],
                    [at.x, -1, at.z],
                    footing,
                    Shape::Whole,
                ));
                if i > 0 {
                    ops.push(put(
                        [at.x, 0, at.z],
                        [at.x, i - 1, at.z],
                        deck,
                        Shape::Whole,
                    ));
                }
                ops.push(put(
                    [at.x, i, at.z],
                    [at.x, i, at.z],
                    deck,
                    Shape::Stair(facing),
                ));
            }
        }
    };
    let up = STOREY_CUBES - 1;
    let mut on_posts = Vec::new();
    posted(&mut on_posts, &[[0, 0], [4, 0], [0, 3], [4, 3]], up);
    floor(&mut on_posts, [0, 0], [4, 3], up);
    flight(&mut on_posts, [1, 4 + STOREY_CUBES], Facing::North, up);
    posted(
        &mut on_posts,
        &[[8, 0], [13, 0], [8, 5], [13, 5], [8, 9], [13, 9]],
        up,
    );
    floor(&mut on_posts, [8, 0], [13, 9], up);
    flight(&mut on_posts, [14 + up, 2], Facing::West, up);
    let mut nine_and_l = Vec::new();
    let nine: Vec<[i32; 2]> = [0, 4, 7]
        .into_iter()
        .flat_map(|x| [0, 4, 7].map(|z| [x, z]))
        .collect();
    posted(&mut nine_and_l, &nine, up - 1);
    floor(&mut nine_and_l, [0, 0], [7, 7], up);
    flight(&mut nine_and_l, [3, -1 - up], Facing::South, up);
    posted(
        &mut nine_and_l,
        &[[10, 0], [19, 0], [19, 3], [13, 3], [13, 9], [10, 9]],
        up - 1,
    );
    floor(&mut nine_and_l, [10, 0], [19, 3], up);
    floor(&mut nine_and_l, [10, 4], [13, 9], up);
    flight(&mut nine_and_l, [11, 10 + up], Facing::North, up);
    let mut against_and_high = vec![BuildOp::Fill {
        min: [0, 0, 0],
        max: [7, 2 * STOREY_CUBES - 1, 0],
        cube: Some(Cube::new(deck, Fill::Solid)),
    }];
    against_and_high.push(put([0, -1, 0], [7, -1, 0], footing, Shape::Whole));
    posted(&mut against_and_high, &[[0, 4], [7, 4]], up);
    floor(&mut against_and_high, [0, 1], [7, 4], up);
    flight(&mut against_and_high, [3, 5 + up], Facing::North, up);
    let high = 2 * STOREY_CUBES - 1;
    posted(
        &mut against_and_high,
        &[[10, 0], [15, 0], [10, 4], [15, 4]],
        high,
    );
    floor(&mut against_and_high, [10, 0], [15, 4], high);
    flight(&mut against_and_high, [12, 5 + high], Facing::North, high);
    [on_posts, nine_and_l, against_and_high]
}

/// A room five metres by four in a kind under a gable of cubes in `roof`, with a doorway
/// and windows.
fn house(kind: Kind, roof: Kind) -> Vec<BuildOp> {
    let mut ops = vec![BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [9, 0, 7],
        kind,
    }];
    ops.extend((4..=5).map(|x| wall([x, 0, 7], Panel::Door, kind)));
    ops.extend((1..=2).map(|x| wall([x, 0, 7], Panel::WINDOW, kind)));
    ops.extend((3..=4).map(|z| wall([0, 0, z], Panel::WINDOW, kind)));
    ops.extend((2..=3).map(|x| wall([x, 0, 0], Panel::WINDOW, kind)));
    with_roof(ops, &[([0, 0], [9, 7], Style::Gable, true)], roof)
}

/// A further kit's rows (D236), named for it: how its walls meet, its openings, carved,
/// a house, its shapes, and mixed with stone and logs every way they meet, after the rest;
/// its roofs of cubes in `roof` (D241).
pub fn kit_rows(
    name: &str,
    kind: Kind,
    stone: Kind,
    logs: Kind,
    roof: Kind,
) -> Vec<(String, Vec<BuildOp>)> {
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
    let mut plinth = house(kind, roof);
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
    let upper = roof_over(&storeys, 1, &[([0, 0], [9, 7], Style::Gable, true)], roof);
    storeys.extend(upper);
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
        ("house", house(kind, roof)),
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
pub struct Acted {
    pub name: &'static str,
    pub ops: Vec<BuildOp>,
    pub columns: [Vec<Op>; 3],
    /// Set on ground falling away to the north, the yard being level, and seen from there.
    pub sloped: bool,
    /// Where its vantage's eye is and what it looks at, from the middle state's corner on
    /// the ground, in metres east, up and south.
    pub view: [Vec3; 2],
}

/// The rows of what stands between a building and the ground, its floors and its openings
/// (construction-001's ground, floor and opening cases): a cabin on a slope, on stone
/// footings down to the ground, with stone steps from its door; floors of stone and of
/// timber, open to the sky; a window cut into a built wall, a course at a time; and a door,
/// shut and open.
pub fn acted_rows(kind: Kind, stone: Kind, roof: &dyn Fn(Kind) -> Kind) -> Vec<Acted> {
    let room = |kind: Kind| BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [9, 0, 7],
        kind,
    };
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
    let slope = with_roof(slope, &[([0, 0], [9, 7], Style::Gable, true)], roof(kind));
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
        part: Some(crate::PartId(900)),
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

/// The rows of roofs of cubes (D241) in a roof kind over walls of a kind: a gable and a hip;
/// L, T and U plans under gables whose wings die into the main ridge; a dormer on a gable's
/// slope, its front a gable carried up from the wall with a window in it; a house of two
/// storeys over half its ground floor, the lower roof against the upper storey's wall and a
/// second roof over it; and a lean-to against a wall two storeys high.
pub fn roof_cube_rows(walls: Kind, roof: Kind) -> Vec<(String, Vec<BuildOp>)> {
    let room = |min: [i32; 3], max: [i32; 3]| BuildOp::AddRoom {
        min,
        max,
        kind: walls,
    };
    let door = |ops: &mut Vec<BuildOp>, xs: [i32; 2], z: i32| {
        ops.extend(xs.map(|x| wall([x, 0, z], Panel::Door, walls)));
    };
    let mut rows = Vec::new();
    for (name, style) in [("gable", Style::Gable), ("hip", Style::Hip)] {
        let mut ops = vec![room([0, 0, 0], [9, 0, 7])];
        door(&mut ops, [4, 5], 0);
        ops.extend((3..=4).map(|z| wall([0, 0, z], Panel::WINDOW, walls)));
        rows.push((
            name.to_string(),
            with_roof(ops, &[([0, 0], [9, 7], style, true)], roof),
        ));
    }
    for shape in ["L", "T", "U"] {
        let (rooms, parts) = super::example::plan(shape, Style::Gable, Style::Gable);
        let mut ops: Vec<BuildOp> = rooms.iter().map(|&(min, max)| room(min, max)).collect();
        door(&mut ops, [4, 5], 0);
        rows.push((shape.to_string(), with_roof(ops, &parts, roof)));
    }
    // A dormer: a gable along X over a room seven metres by six, a gable along Z three metres
    // wide off its south slope, its front the south wall carried up, a window in it.
    let mut dormer = vec![room([0, 0, 0], [13, 0, 11])];
    door(&mut dormer, [3, 4], 11);
    let mut dormer = with_roof(
        dormer,
        &[
            ([0, 0], [13, 11], Style::Gable, true),
            ([4, 6], [9, 11], Style::Gable, false),
        ],
        roof,
    );
    dormer.push(BuildOp::Fill {
        min: [6, 6, 11],
        max: [7, 6, 11],
        cube: Some(Cube::new(walls, Fill::Open)),
    });
    rows.push(("dormer".to_string(), dormer));
    // Two storeys over the west half of a ground floor eight metres by five: the ground
    // floor's east half under a lean-to against the upper storey's east wall, and a hip
    // over the upper storey.
    let mut two = vec![room([0, 0, 0], [15, 0, 9]), room([0, 1, 0], [7, 1, 9])];
    door(&mut two, [11, 12], 9);
    two.extend((2..=3).map(|x| wall([x, 1, 9], Panel::WINDOW, walls)));
    let lower = roof_over(&two, 0, &[([8, 0], [15, 9], Style::Hip, true)], roof);
    two.extend(lower);
    let upper = roof_over(&two, 1, &[([0, 0], [7, 9], Style::Hip, true)], roof);
    two.extend(upper);
    rows.push(("two storeys, a roof over a roof".to_string(), two));
    // A lean-to against its own back wall carried up two storeys, rising to it.
    let mut against = vec![room([0, 0, 0], [9, 0, 6])];
    door(&mut against, [4, 5], 0);
    against.push(BuildOp::Fill {
        min: [0, 0, 6],
        max: [9, 2 * STOREY_CUBES - 1, 6],
        cube: Some(Cube::new(walls, Fill::Solid)),
    });
    let against = with_roof(against, &[([0, 0], [9, 5], Style::LeanTo, true)], roof);
    rows.push(("against a taller wall".to_string(), against));
    rows
}

/// A house's gable end, to be seen straight on: a room five metres by four in a wall kind
/// under a gable of cubes in a roof kind, its west wall the gable, with a doorway in it
/// under a lintel and a window in the long wall beside. Where the walls are carried up to
/// the roof, its verges and its ridge's end all show at once (the "gable end" rows).
pub fn gable_end(walls: Kind, roof: Kind) -> Vec<BuildOp> {
    let mut ops = vec![BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [9, 0, 7],
        kind: walls,
    }];
    ops.extend((3..=4).map(|z| wall([0, 0, z], Panel::Door, walls)));
    ops.extend((2..=3).map(|x| wall([x, 0, 7], Panel::WINDOW, walls)));
    with_roof(ops, &[([0, 0], [9, 7], Style::Gable, true)], roof)
}
/// How far east of a wall material's preview house its platform stands, in metres (the
/// views in `tools/kits/preview.ts` look at it there).
const PREVIEW_ASIDE_M: f32 = 8.5;

/// A material's preview (D349): every role and shape of its pattern in one place, as bases
/// of cubes, what is then done to them, and how far east of the preview each stands. A wall
/// material's house, with a doorway, windows, corners, its foot on the ground, a gable and a
/// floor, a hole carved in its front and its front's east end damaged; beside it a platform
/// reached by a flight of steps, its top, a slab over a wall with a window, and a wall
/// part-built. A roof covering's L of stone walls under a hip with a gabled wing and a
/// valley where they meet, its east end damaged and its wing's end not yet all laid.
pub fn preview(
    kit: &Kit,
    stone: Kind,
    roof: Kind,
    rules: &crate::Rules,
) -> Vec<(Vec<BuildOp>, Vec<Op>, f32)> {
    // Only the acts the rules let through, as a player's would be, each tried in turn: a
    // course of logs comes out of every cube it spans at once, so a cube may be empty before
    // its count of steps is up.
    wanted_preview(kit, stone, roof)
        .into_iter()
        .map(|(ops, acts, aside)| {
            let mut base = Base::default();
            for &op in &ops {
                base.apply(op);
            }
            let cx = crate::ops::Context::free(rules);
            let held = acts
                .into_iter()
                .filter(|act| base.act(act, &cx).is_ok())
                .collect();
            (ops, held, aside)
        })
        .collect()
}

/// What a material's preview would do to each base, before the rules have their say.
fn wanted_preview(kit: &Kit, stone: Kind, roof: Kind) -> Vec<(Vec<BuildOp>, Vec<Op>, f32)> {
    let kind = kit.kind();
    let steps = usize::from(kit.cost.steps.max(1));
    let damage = |cubes: Vec<[i32; 3]>, share: f32| Op::Damage {
        cubes,
        amount: (f32::from(kit.toughness.health) * share) as u16,
    };
    // Steps taken out of cubes, the highest first, a cube at a time.
    let empty = |mut cubes: Vec<[i32; 3]>, times: usize| -> Vec<Op> {
        cubes.sort_by_key(|at| -at[1]);
        (0..times)
            .flat_map(|_| cubes.clone())
            .map(|at| Op::Empty { cubes: vec![at] })
            .collect()
    };
    if kind.is_roof() {
        let (rooms, parts) = super::example::plan("L", Style::Hip, Style::Gable);
        let mut ops: Vec<BuildOp> = rooms
            .iter()
            .map(|&(min, max)| BuildOp::AddRoom {
                min,
                max,
                kind: stone,
            })
            .collect();
        ops.extend([4, 5].map(|x| wall([x, 0, 0], Panel::Door, stone)));
        let ops = with_roof(ops, &parts, kind);
        let mut base = Base::default();
        for &op in &ops {
            base.apply(op);
        }
        let covering: Vec<[i32; 3]> = base
            .cubes()
            .filter(|(_, cube)| cube.kind == kind)
            .map(|(at, _)| at.to_array())
            .collect();
        let east = covering.iter().map(|at| at[0]).max().unwrap_or(0);
        let south = covering.iter().map(|at| at[2]).max().unwrap_or(0);
        let mut acts = vec![damage(
            covering
                .iter()
                .copied()
                .filter(|at| at[0] >= east - 1)
                .collect(),
            0.6,
        )];
        // The wing's end at its gable: its last two rows half laid, its last none.
        acts.extend(empty(
            covering
                .iter()
                .copied()
                .filter(|at| at[2] >= south - 1)
                .collect(),
            steps / 2,
        ));
        acts.extend(empty(
            covering
                .iter()
                .copied()
                .filter(|at| at[2] == south)
                .collect(),
            steps - steps / 2,
        ));
        return vec![(ops, acts, 0.0)];
    }
    // The house's front, its south wall, with the doorway and a window in it.
    let front = |xs: [i32; 2], ys: [i32; 2]| -> Vec<[i32; 3]> {
        (xs[0]..=xs[1])
            .flat_map(|x| (ys[0]..=ys[1]).map(move |y| [x, y, 7]))
            .collect()
    };
    let mut cut = vec![Op::Carve {
        cubes: front([7, 8], [2, 3]),
    }];
    cut.extend(empty(front([7, 8], [2, 3]), steps));
    cut.push(damage(front([8, 9], [0, 1]), 0.5));
    cut.push(damage(front([9, 9], [2, 2]), 0.8));
    // The platform, and before it, beside its steps, a low wall going up: its top course not
    // yet in, the one under it half in.
    let mut platform = keen(kind, kit.pattern == Pattern::Masonry);
    platform.push(BuildOp::Fill {
        min: [0, 0, 7],
        max: [3, 3, 7],
        cube: Some(Cube::new(kind, Fill::Solid)),
    });
    let top = |y: i32| (0..=3).map(|x| [x, y, 7]).collect::<Vec<_>>();
    let mut going_up = empty(top(3), steps);
    going_up.extend(empty(top(2), steps.div_ceil(2)));
    vec![
        (house(kind, roof), cut, 0.0),
        (platform, going_up, PREVIEW_ASIDE_M),
    ]
}
