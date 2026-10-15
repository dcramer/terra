//! The junctions family (docs/building-voxels.md, gap 9): a small base for each junction of
//! the voxel contract's matrix that no other inspected base has, in a wall kind under its
//! roof, so the mesh checker's ratchet (`inspect::cases`) holds each one.

use crate::roofer::Style;
use crate::{BuildOp, Cube, Facing, Fill, Kind, STOREY_CUBES, Shape, roof_over};

/// Each junction's base, by name, in `kind` under `roof`, and `other`, another roof kind, for
/// where two roof kinds meet.
pub fn junctions(kind: Kind, roof: Kind, other: Kind) -> Vec<(String, Vec<BuildOp>)> {
    let top = STOREY_CUBES - 1;
    let fill = |min: [i32; 3], max: [i32; 3], cube: Option<Cube>| BuildOp::Fill { min, max, cube };
    let solid = Some(Cube::new(kind, Fill::Solid));
    let shaped = |shape: Shape| {
        Some(Cube {
            shape,
            ..Cube::new(kind, Fill::Solid)
        })
    };
    let room = BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [9, 0, 7],
        kind,
    };
    let gable = |ops: &[BuildOp], min: [i32; 2], max: [i32; 2]| {
        roof_over(ops, 0, &vec![(min, max, Style::Gable, true)], roof)
    };
    let mut out: Vec<(String, Vec<BuildOp>)> = Vec::new();

    // Two walls meeting only along an upright edge, the end of one at the corner of the
    // other's end, and a wall over another stepped back a cube, meeting it only along a
    // level edge (gap 3).
    out.push((
        "cubes meeting along an upright edge".into(),
        vec![
            fill([0, 0, 0], [3, 3, 0], solid),
            fill([4, 0, 1], [4, 3, 4], solid),
        ],
    ));
    out.push((
        "cubes meeting along a level edge".into(),
        vec![
            fill([0, 0, 0], [5, 1, 0], solid),
            fill([0, 2, 1], [5, 3, 1], solid),
            fill([0, 0, 2], [5, 3, 2], solid),
        ],
    ));

    // A room floored a storey up, the floor reaching a cube and a half past its south wall,
    // a balcony, with a stairwell through it (gap 7).
    let floor = vec![
        room,
        fill([0, STOREY_CUBES, 0], [9, STOREY_CUBES, 9], solid),
        fill([3, STOREY_CUBES, 2], [4, STOREY_CUBES, 4], None),
    ];
    out.push(("a floor with a stairwell and a balcony".into(), floor));

    // A wall two cubes thick with a window through it (gap 7).
    out.push((
        "a thick wall with a window".into(),
        vec![
            fill([0, 0, 0], [7, 3, 1], solid),
            fill([3, 1, 0], [4, 2, 1], Some(Cube::new(kind, Fill::Open))),
        ],
    ));

    // A roof on four posts, and a hip on a room whose walls' top course is cut to a half,
    // standing on them (gap 9).
    let mut posts = Vec::new();
    for [x, z] in [[0, 0], [5, 0], [0, 5], [5, 5]] {
        posts.push(fill([x, 0, z], [x, top, z], shaped(Shape::Post)));
    }
    let roofed = gable(&posts, [0, 0], [5, 5]);
    posts.extend(roofed);
    out.push(("a roof on posts".into(), posts));
    let mut halves = vec![room];
    for (min, max) in [
        ([0, top, 0], [9, top, 0]),
        ([0, top, 7], [9, top, 7]),
        ([0, top, 0], [0, top, 7]),
        ([9, top, 0], [9, top, 7]),
    ] {
        halves.push(fill(min, max, shaped(Shape::Half)));
    }
    let roofed = roof_over(&halves, 0, &vec![([0, 0], [9, 7], Style::Hip, true)], roof);
    halves.extend(roofed);
    out.push(("a roof on cut wall cubes".into(), halves));

    // An L whose wing is roofed in another roof kind, the two meeting in its valleys (gap 9).
    let mut two = vec![
        room,
        BuildOp::AddRoom {
            min: [0, 0, 7],
            max: [7, 0, 15],
            kind,
        },
    ];
    let roofed = roof_over(
        &two,
        0,
        &vec![
            ([0, 0], [9, 7], Style::Gable, true),
            ([0, 4], [7, 15], Style::Gable, false),
        ],
        roof,
    );
    two.extend(roofed.into_iter().map(|op| match op {
        BuildOp::Fill {
            min,
            max,
            cube: Some(cube),
        } if cube.kind == roof && min[2] > 7 => BuildOp::Fill {
            min,
            max,
            cube: Some(Cube {
                kind: other,
                ..cube
            }),
        },
        op => op,
    }));
    out.push(("two roof kinds meeting".into(), two));

    // A flight of stairs up along a wall's face, each stair on the cubes under it (gap 9).
    let mut stairs = vec![fill([0, 0, 0], [7, 3, 0], solid)];
    for step in 0..4 {
        if step > 0 {
            stairs.push(fill([step + 1, 0, 1], [step + 1, step - 1, 1], solid));
        }
        stairs.push(fill(
            [step + 1, step, 1],
            [step + 1, step, 1],
            shaped(Shape::Stair(Facing::East)),
        ));
    }
    out.push(("stairs against a wall".into(), stairs));
    out
}
