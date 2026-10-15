//! Where roofs and walls meet (D343, docs/building-plan.md#where-roofs-and-walls-meet-d343):
//! a row of bases for every case, in a wall kind under a roof kind, so the kit sheet shows
//! each and the drawing checker (`inspect`, `cargo xtask artifacts`) holds each to no gap
//! and no fault: walls and chimneys through a roof, walls standing on it (a dormer's cheeks
//! and face, a chimney, a stack on its ridge), on its hips and valleys, a storey built over
//! a roof left on, a roof against a wall on every side and in a corner, shapes and posts on
//! and through a roof, and a roof going up under walls standing on it.

use crate::example::{Plan, plan, roof_over};
use crate::roofer::Style;
use crate::{Base, BuildOp, Cube, Facing, Fill, Kind, STOREY_CUBES, Shape};
use glam::IVec3;

/// The rows, each named, in `wall` under a roof of `roof`, and `other`, a kind of another
/// pattern its chimney in a gable is built of, where there is one: a timber wall's courses
/// end against it under their end boards.
pub fn rows(wall: Kind, roof: Kind, other: Option<Kind>) -> Vec<(String, Vec<BuildOp>)> {
    let mut rows = vec![
        ("through a roof".to_string(), through(wall, roof)),
        ("standing on a roof".to_string(), standing(wall, roof, None)),
        ("on hips and valleys".to_string(), creases(wall, roof)),
        ("a storey over a roof".to_string(), storey(wall, roof)),
        (
            "a roof rising to a wall".to_string(),
            against(wall, roof, Against::Rising),
        ),
        (
            "a roof falling to a wall".to_string(),
            against(wall, roof, Against::Falling),
        ),
        (
            "a verge against a wall".to_string(),
            against(wall, roof, Against::Verge),
        ),
        (
            "a roof in the corner of two walls".to_string(),
            against(wall, roof, Against::Corner),
        ),
        ("shapes on a roof".to_string(), shapes(wall, roof)),
        ("holes in a roof".to_string(), holes(wall, roof)),
        ("openings up to a roof".to_string(), openings(wall, roof)),
        (
            "going up, the covering on and walls rising".to_string(),
            standing(wall, roof, Some((12, 8))),
        ),
        (
            "going up, the frame alone under walls".to_string(),
            standing(wall, roof, Some((4, 16))),
        ),
    ];
    if let Some(other) = other {
        rows.push((
            "a chimney of another kind in a gable".to_string(),
            chimney_in_gable(wall, roof, other),
        ));
    }
    rows
}

/// Holes taken out of a roof, a cube and two cubes across, as a smoke hole or a skylight:
/// flashed round, no eaves run into them, open at every detail.
fn holes(wall: Kind, roof: Kind) -> Vec<BuildOp> {
    let mut ops = gabled(wall, roof);
    let base = Base::from_shortcuts(&ops);
    for (x, z) in [(3, 1), (6, 5), (7, 5)] {
        if let Some((y, _)) = roof_top(&base, x, z) {
            ops.push(BuildOp::Fill {
                min: [x, y, z],
                max: [x, y, z],
                cube: None,
            });
        }
    }
    ops
}

/// Openings whose tops are a roof's cubes: a window two cubes square up to the eaves and
/// one in a gable up to its verge, the roof's cube their head.
fn openings(wall: Kind, roof: Kind) -> Vec<BuildOp> {
    let mut ops = gabled(wall, roof);
    let hole = Cube::new(wall, Fill::Open);
    for at in [
        [3, 4, 0],
        [4, 4, 0],
        [3, 5, 0],
        [4, 5, 0],
        [0, 6, 2],
        [0, 7, 3],
    ] {
        put(&mut ops, IVec3::from_array(at), hole);
    }
    ops
}

/// A chimney of another kind two cubes across in the middle of a gable, from the ground up
/// through the roof past its ridge: the gable's courses end against it.
fn chimney_in_gable(wall: Kind, roof: Kind, other: Kind) -> Vec<BuildOp> {
    let mut ops = gabled(wall, roof);
    let base = Base::from_shortcuts(&ops);
    let top = |x: i32, z: i32| roof_top(&base, x, z).map_or(STOREY_CUBES, |(y, _)| y);
    let made = cube(other, Shape::Whole);
    let high = top(0, 3).max(top(0, 4)) + 2;
    for z in 3..=4 {
        column(&mut ops, 0, z, 0, high, made);
    }
    ops
}

/// A built cube of a kind and shape.
fn cube(kind: Kind, shape: Shape) -> Cube {
    Cube {
        shape,
        ..Cube::new(kind, Fill::Solid)
    }
}

fn put(ops: &mut Vec<BuildOp>, at: IVec3, cube: Cube) {
    ops.push(BuildOp::Fill {
        min: at.to_array(),
        max: at.to_array(),
        cube: Some(cube),
    });
}

/// A plan's rooms in `wall` under its roof of cubes in `roof`.
fn house((rooms, parts): Plan, wall: Kind, roof: Kind) -> Vec<BuildOp> {
    let mut ops: Vec<BuildOp> = rooms
        .iter()
        .map(|&(min, max)| BuildOp::AddRoom {
            min,
            max,
            kind: wall,
        })
        .collect();
    let over = roof_over(&ops, 0, &parts, roof);
    ops.extend(over);
    ops
}

/// The highest roof cube in a column, and its shape.
fn roof_top(base: &Base, x: i32, z: i32) -> Option<(i32, Shape)> {
    base.column(x, z)
        .filter(|&y| {
            base.cube(IVec3::new(x, y, z))
                .is_some_and(|c| c.kind.is_roof())
        })
        .max()
        .and_then(|y| Some((y, base.cube(IVec3::new(x, y, z))?.shape)))
}

/// A column of cubes from one layer to another, in place of whatever is there.
fn column(ops: &mut Vec<BuildOp>, x: i32, z: i32, from: i32, to: i32, made: Cube) {
    for y in from..=to {
        put(ops, IVec3::new(x, y, z), made);
    }
}

/// A gabled house five metres by four, its ridge along X: walls a storey high, slopes rising
/// a cube a cube from layer 6 at its eaves to 9 at its ridge.
fn gabled(wall: Kind, roof: Kind) -> Vec<BuildOp> {
    house(plan("rectangle", Style::Gable, Style::Gable), wall, roof)
}

/// A chimney a cube across and one two across, and a
/// wall across the whole roof, each rising from the ground through the roof: the covering
/// tucked into them and flashed round them.
fn through(wall: Kind, roof: Kind) -> Vec<BuildOp> {
    let mut ops = gabled(wall, roof);
    let base = Base::from_shortcuts(&ops);
    let top = |x: i32, z: i32| roof_top(&base, x, z).map_or(STOREY_CUBES, |(y, _)| y);
    let made = cube(wall, Shape::Whole);
    column(&mut ops, 2, 1, 0, top(2, 1) + 2, made);
    let high = (5..=6)
        .flat_map(|x| (1..=2).map(move |z| (x, z)))
        .map(|(x, z)| top(x, z))
        .max()
        .unwrap_or(STOREY_CUBES);
    for x in 5..=6 {
        for z in 1..=2 {
            column(&mut ops, x, z, 0, high + 2, made);
        }
    }
    let ridge = (0..=7).map(|z| top(8, z)).max().unwrap_or(STOREY_CUBES);
    for z in 0..=7 {
        column(&mut ops, 8, z, 0, ridge + 1, made);
    }
    ops
}

/// Walls standing on a roof's cubes: a dormer's cheeks and face, its inside open to the
/// roof, a chimney on a slope and a stack on the ridge. `going`: the roof's cubes so far in
/// (sixteenths) and the walls' highest cubes so far, for a roof going up under them.
fn standing(wall: Kind, roof: Kind, going: Option<(u8, u8)>) -> Vec<BuildOp> {
    let mut ops = gabled(wall, roof);
    if let Some((level, _)) = going {
        let base = Base::from_shortcuts(&ops);
        let roofs: Vec<(IVec3, Cube)> = base.cubes().filter(|(_, c)| c.kind.is_roof()).collect();
        for (at, c) in roofs {
            put(&mut ops, at, c.at_level(level));
        }
    }
    let base = Base::from_shortcuts(&ops);
    let top = |x: i32, z: i32| roof_top(&base, x, z).map_or(STOREY_CUBES, |(y, _)| y);
    let made = cube(wall, Shape::Whole);
    let crown = top(0, 3) + 1;
    let stand = |ops: &mut Vec<BuildOp>, x: i32, z: i32, to: i32| {
        let from = top(x, z) + 1;
        for y in from..=to {
            let level = match going {
                Some((_, last)) if y == to => last,
                _ => 16,
            };
            put(ops, IVec3::new(x, y, z), made.at_level(level));
        }
    };
    // The dormer: cheeks up the slope, its face across the eaves.
    for z in 0..=2 {
        stand(&mut ops, 2, z, crown);
        stand(&mut ops, 5, z, crown);
    }
    for x in 3..=4 {
        stand(&mut ops, x, 0, crown);
    }
    // A chimney on the slope, and a stack across the ridge.
    let chimney = top(7, 1) + 2;
    stand(&mut ops, 7, 1, chimney);
    for z in 3..=4 {
        stand(&mut ops, 8, z, crown + 1);
    }
    ops
}

/// An L under hips: a stack standing on a hip, one on the valley, one on the ridge, and a
/// chimney through the valley.
fn creases(wall: Kind, roof: Kind) -> Vec<BuildOp> {
    let mut ops = house(plan("L", Style::Hip, Style::Hip), wall, roof);
    let base = Base::from_shortcuts(&ops);
    let tops: Vec<(IVec3, Shape)> = base
        .cubes()
        .filter(|(at, c)| {
            c.kind.is_roof() && roof_top(&base, at.x, at.z).is_some_and(|(y, _)| y == at.y)
        })
        .map(|(at, c)| (at, c.shape))
        .collect();
    let made = cube(wall, Shape::Whole);
    let pick = |want: &dyn Fn(Shape) -> bool, skip: usize| {
        tops.iter()
            .filter(|(at, shape)| want(*shape) && at.x > 0 && at.z > 0)
            .nth(skip)
            .map(|(at, _)| *at)
    };
    let hip = pick(&|s| matches!(s, Shape::OuterCorner(_)), 1);
    let valleys: Vec<IVec3> = (0..4)
        .filter_map(|n| pick(&|s| matches!(s, Shape::InnerCorner(_)), n))
        .collect();
    let ridge = tops
        .iter()
        .map(|(at, _)| *at)
        .max_by_key(|at| (at.y, at.x, at.z));
    for at in [hip, valleys.first().copied(), ridge].into_iter().flatten() {
        column(&mut ops, at.x, at.z, at.y + 1, at.y + 2, made);
    }
    if let Some(at) = valleys.last().filter(|_| valleys.len() > 2) {
        column(&mut ops, at.x, at.z, 0, at.y + 2, made);
    }
    ops
}

/// A storey built round the house's walls with its roof left on, the walls standing on the
/// eaves and the gables, and a roof over the storey: the old roof inside it.
fn storey(wall: Kind, roof: Kind) -> Vec<BuildOp> {
    let (rooms, parts) = plan("rectangle", Style::Gable, Style::Gable);
    let mut ops = house((rooms.clone(), parts.clone()), wall, roof);
    let base = Base::from_shortcuts(&ops);
    let made = cube(wall, Shape::Whole);
    let (min, max) = rooms[0];
    for x in min[0]..=max[0] {
        for z in min[2]..=max[2] {
            if x != min[0] && x != max[0] && z != min[2] && z != max[2] {
                continue;
            }
            for y in STOREY_CUBES..2 * STOREY_CUBES {
                if base.cube(IVec3::new(x, y, z)).is_none() {
                    put(&mut ops, IVec3::new(x, y, z), made);
                }
            }
        }
    }
    let over = roof_over(&ops, 1, &parts, roof);
    ops.extend(over);
    ops
}

/// A roof against a wall taller than it: a lean-to rising to it (an apron), one falling
/// to it (a back gutter), a gable's verge against it (steps), or a lean-to in the corner
/// two walls make.
fn against(wall: Kind, roof: Kind, side: Against) -> Vec<BuildOp> {
    let mut ops = Vec::new();
    let made = cube(wall, Shape::Whole);
    // A room's walls a storey high, from one corner to another.
    let room = |ops: &mut Vec<BuildOp>, x0: i32, x1: i32, z0: i32, z1: i32| {
        ops.push(BuildOp::AddRoom {
            min: [x0, 0, z0],
            max: [x1, 0, z1],
            kind: wall,
        });
    };
    // Slopes over a room from x0 to x1, rising toward `facing` a cube a cube from the eave
    // at layer 6, with gables of the wall under them at its ends.
    let slopes = |ops: &mut Vec<BuildOp>, x0: i32, x1: i32, zs: &[(i32, i32, Facing)]| {
        for &(z, y, facing) in zs {
            for x in x0..=x1 {
                put(ops, IVec3::new(x, y, z), cube(roof, Shape::Slope(facing)));
            }
            for x in [x0, x1] {
                for g in STOREY_CUBES..y {
                    put(ops, IVec3::new(x, g, z), made);
                }
            }
        }
    };
    let tall = |ops: &mut Vec<BuildOp>, min: [i32; 3], max: [i32; 3]| {
        ops.push(BuildOp::Fill {
            min,
            max,
            cube: Some(made),
        });
    };
    let lean = |rising: Facing| -> Vec<(i32, i32, Facing)> {
        (0..=5)
            .map(|z| {
                let up = if rising == Facing::South { z } else { 5 - z };
                (z, STOREY_CUBES + up, rising)
            })
            .collect()
    };
    match side {
        Against::Rising => {
            room(&mut ops, 0, 9, 0, 5);
            slopes(&mut ops, 0, 9, &lean(Facing::South));
            tall(&mut ops, [-1, 0, 6], [10, 13, 6]);
        }
        Against::Falling => {
            room(&mut ops, 0, 9, 0, 5);
            slopes(&mut ops, 0, 9, &lean(Facing::North));
            tall(&mut ops, [-1, 0, 6], [10, 13, 6]);
        }
        Against::Verge => {
            room(&mut ops, 0, 9, 0, 7);
            let gable: Vec<(i32, i32, Facing)> = (0..=7)
                .map(|z| {
                    if z < 4 {
                        (z, STOREY_CUBES + z, Facing::South)
                    } else {
                        (z, STOREY_CUBES + 7 - z, Facing::North)
                    }
                })
                .collect();
            slopes(&mut ops, 0, 9, &gable);
            tall(&mut ops, [10, 0, -1], [10, 13, 8]);
        }
        Against::Corner => {
            room(&mut ops, 0, 9, 0, 5);
            slopes(&mut ops, 0, 9, &lean(Facing::South));
            tall(&mut ops, [-1, 0, 6], [10, 13, 6]);
            tall(&mut ops, [10, 0, -1], [10, 13, 5]);
        }
    }
    ops
}

/// Which way a roof meets a wall taller than it.
#[derive(Clone, Copy)]
enum Against {
    /// A lean-to rising to the wall: an apron.
    Rising,
    /// A lean-to falling to it: a back gutter.
    Falling,
    /// A gable's verge against it: steps.
    Verge,
    /// A lean-to in the corner of two walls.
    Corner,
}

/// Cubes cut to shapes standing on a slope: a half, a stair, a slope and a
/// post, and a post rising from the ground through the roof.
fn shapes(wall: Kind, roof: Kind) -> Vec<BuildOp> {
    let mut ops = gabled(wall, roof);
    let base = Base::from_shortcuts(&ops);
    let top = |x: i32, z: i32| roof_top(&base, x, z).map_or(STOREY_CUBES, |(y, _)| y);
    for (x, shape) in [
        (1, Shape::Half),
        (3, Shape::Stair(Facing::South)),
        (5, Shape::Slope(Facing::South)),
        (7, Shape::Post),
    ] {
        put(&mut ops, IVec3::new(x, top(x, 1) + 1, 1), cube(wall, shape));
    }
    column(&mut ops, 8, 2, 0, top(8, 2) + 1, cube(wall, Shape::Post));
    ops
}
