use super::Element::*;
use super::*;
use crate::registry::Materials;

fn baseline() -> Materials {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    Materials::read(&assets, true).expect("the baseline's materials")
}

#[test]
fn a_seed_builds_the_same_base_and_seeds_differ() {
    let materials = baseline();
    let kits = &materials.kits;
    assert_eq!(elements(7, kits), elements(7, kits));
    let a = build(kits, &elements(7, kits));
    let b = build(kits, &elements(7, kits));
    assert_eq!(a.cubes().collect::<Vec<_>>(), b.cubes().collect::<Vec<_>>());
    let kinds: BTreeSet<usize> = (0..16).map(|seed| elements(seed, kits).len()).collect();
    assert!(kinds.len() > 1, "every seed the same length");
    assert_ne!(elements(1, kits), elements(2, kits));
}

#[test]
fn what_the_elements_lay_is_built_through_the_rules() {
    // A room of logs with a door, a gable over it: laid by the hammer's plans and built
    // whole, its doorway open.
    let materials = baseline();
    let kits = &materials.kits;
    let named = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, file)| file.as_str() == name)
            .map(|(&kind, _)| kind)
            .unwrap()
    };
    let base = build(
        kits,
        &[
            Room {
                min: [0, 0],
                max: [5, 4],
                storey: 0,
                kind: named("logs"),
            },
            Opening {
                from: [2, 0],
                to: [3, 0],
                storey: 0,
                door: true,
            },
            Roof {
                tile: [2, 2],
                storey: 0,
                style: Style::Gable,
                along_x: None,
                kind: named("shakes"),
            },
        ],
    );
    let waiting = base
        .cubes()
        .filter(|(_, cube)| cube.fill != terra_build::Fill::Open && !cube.is_built())
        .count();
    assert_eq!(waiting, 0, "everything laid is built");
    assert!(base.kinds().any(|kind| kind == named("shakes")), "the roof");
    let door = base.cube(IVec3::new(2, 1, 0)).map(|cube| cube.fill);
    assert_eq!(door, Some(terra_build::Fill::Open), "the doorway");
}

#[test]
fn fuzz_seeds_0_3_and_37_every_boarded_timber_cube_alone_is_drawn_without_coplanar() {
    // Found by `cargo xtask fuzz` at seeds 0, 3, 4, 37, 38, 98, 99, 140 and 148, each
    // shrunk to one cube: a timber cube cut upside down, to an upper half or a corner,
    // boarded as a box whose boards' sawn ends lay in the plane of its other boards'
    // faces (the voxel contract's gap 10). Every such shape, each way, in logs and hewn
    // timber, alone on the sloping ground, each its own box.
    let materials = baseline();
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .unwrap()
    };
    let boarded: Vec<Shape> = Shape::ALL
        .into_iter()
        .filter(|shape| {
            matches!(
                shape,
                Shape::UpperHalf
                    | Shape::InvertedStair(_)
                    | Shape::InvertedSlope(_)
                    | Shape::OuterCorner(_)
                    | Shape::InnerCorner(_)
            )
        })
        .collect();
    let mut elements = Vec::new();
    for (row, timber) in ["logs", "hewn"].into_iter().enumerate() {
        for (i, &shape) in boarded.iter().enumerate() {
            elements.push(Blocks {
                at: [i as i32 * 3, 0, row as i32 * 3],
                size: 1,
                shape,
                kind: kind(timber),
            });
        }
    }
    let base = build(&materials.kits, &elements);
    let report = terra_build_mesh::inspect::inspect(
        &base,
        &materials.kits,
        &terra_build_mesh::relief::Maps::worst(&materials.kits, false),
    );
    let fought: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.fault == terra_build_mesh::inspect::Fault::Coplanar)
        .collect();
    assert!(fought.is_empty(), "{fought:#?}");
}

#[test]
fn fuzz_seed_16_a_log_walls_top_split_log_ends_inside_the_end_board_against_stone() {
    // Found by `cargo xtask fuzz` at seeds 16, 20, 46, 56, 96, 113, 137 and 158, shrunk:
    // a stone wall run into a log room. A wall running north and south lies half a
    // course higher, with a split log on its top whose centre is on the next layer's
    // floor; asked there whether it ends against an end board, it found nothing and ran
    // on to the stone's face, its sawn end in the plane of the board's (gap 14).
    let materials = baseline();
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .unwrap()
    };
    let base = build(
        &materials.kits,
        &[
            Wall {
                from: [0, 2],
                to: [5, 2],
                storey: 0,
                kind: kind("stone"),
            },
            Room {
                min: [4, 1],
                max: [7, 4],
                storey: 0,
                kind: kind("logs"),
            },
        ],
    );
    let report = terra_build_mesh::inspect::inspect(
        &base,
        &materials.kits,
        &terra_build_mesh::relief::Maps::worst(&materials.kits, false),
    );
    let fought: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.fault == terra_build_mesh::inspect::Fault::Coplanar)
        .collect();
    assert!(fought.is_empty(), "{fought:#?}");
}

#[test]
fn fuzz_seed_12_an_end_board_over_a_doorway_stands_on_the_doors_head() {
    // Found by `cargo xtask fuzz` at seed 12, shrunk: a log room's doorway beside the stone
    // room it is built into. Over the doorway the logs end against the stone, and the end
    // board up their ends started at the doorway's top, through the head board lapping past
    // the opening, in the plane of its face.
    let materials = baseline();
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .unwrap()
    };
    let base = build(
        &materials.kits,
        &[
            Room {
                min: [0, 1],
                max: [6, 6],
                storey: 0,
                kind: kind("stone"),
            },
            Room {
                min: [0, 0],
                max: [4, 3],
                storey: 0,
                kind: kind("logs"),
            },
            Opening {
                from: [1, 3],
                to: [2, 3],
                storey: 0,
                door: true,
            },
        ],
    );
    let report = terra_build_mesh::inspect::inspect(
        &base,
        &materials.kits,
        &terra_build_mesh::relief::Maps::worst(&materials.kits, false),
    );
    let fought: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.fault == terra_build_mesh::inspect::Fault::Coplanar)
        .collect();
    assert!(fought.is_empty(), "{fought:#?}");
}

#[test]
fn a_failing_base_shrinks_to_the_element_that_fails_it() {
    // Fails while a box of hewn timber is in it: the room and the wall go, the box
    // stays, made one cube.
    let materials = baseline();
    let kits = &materials.kits;
    let named = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, file)| file.as_str() == name)
            .map(|(&kind, _)| kind)
            .unwrap()
    };
    let (hewn, logs) = (named("hewn"), named("logs"));
    let all = vec![
        Room {
            min: [0, 0],
            max: [4, 4],
            storey: 0,
            kind: logs,
        },
        Blocks {
            at: [8, 0, 8],
            size: 2,
            shape: Shape::Whole,
            kind: hewn,
        },
        Wall {
            from: [0, 7],
            to: [4, 7],
            storey: 0,
            kind: logs,
        },
    ];
    let fails = |base: &Base| base.kinds().any(|kind| kind == hewn);
    let (kept, tries) = shrink(&all, kits, &fails);
    assert_eq!(
        kept,
        vec![Blocks {
            at: [8, 0, 8],
            size: 1,
            shape: Shape::Whole,
            kind: hewn,
        }]
    );
    assert!(tries >= 3);
    let rust = as_rust(&kept, &|_| "hewn".into());
    assert!(
        rust.contains(
            "Blocks { at: [8, 0, 8], size: 1, shape: Shape::Whole, kind: kind(\"hewn\") }"
        ),
        "{rust}"
    );
}

#[test]
fn fuzz_seed_36_a_roof_cube_laid_whole_on_the_ground_has_no_battens_crossing() {
    // Found by `cargo xtask fuzz` at seeds 36, 68 and 128, shrunk: a whole shakes cube
    // on its own. Its top is level, and a course's level line cut through that plane
    // laid battens along its triangles' edges and diagonals, crossing one another in one
    // plane (gap 11).
    let materials = baseline();
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .unwrap()
    };
    let base = build(
        &materials.kits,
        &[Blocks {
            at: [6, -1, 4],
            size: 1,
            shape: Shape::Whole,
            kind: kind("shakes"),
        }],
    );
    let report = terra_build_mesh::inspect::inspect(
        &base,
        &materials.kits,
        &terra_build_mesh::relief::Maps::worst(&materials.kits, false),
    );
    assert_eq!(
        report.count(terra_build_mesh::inspect::Fault::Coplanar),
        0,
        "{report:?}"
    );
}

#[test]
fn fuzz_seed_72_inner_corners_stepping_keep_their_beams_and_trims_on_their_own_roof() {
    // Found by `cargo xtask fuzz` at seeds 61 and 72, shrunk: a 2x2x2 box of slate whose
    // top layer is four inner corners alike, so its roof steps down a cube between them
    // (gap 11). A valley's beam reached past its high end into the lower cube beside it
    // (9 cm over that roof), and one cube's eaves, carried round its corner past a side
    // open over the lower cube, stood a step over that cube's own eaves, their verge's
    // trim 21 cm over its roof.
    let materials = baseline();
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .unwrap()
    };
    let base = build(
        &materials.kits,
        &[Blocks {
            at: [9, -1, 3],
            size: 2,
            shape: Shape::InnerCorner(Facing::South),
            kind: kind("slate"),
        }],
    );
    let report = terra_build_mesh::inspect::inspect(
        &base,
        &materials.kits,
        &terra_build_mesh::relief::Maps::worst(&materials.kits, false),
    );
    let out: Vec<_> = report
        .findings
        .iter()
        .filter(|f| {
            f.fault == terra_build_mesh::inspect::Fault::PokesOut
                && ["Log", "Trim"].iter().any(|what| f.what.contains(what))
        })
        .collect();
    assert!(out.is_empty(), "{out:#?}");
}

#[test]
fn fuzz_seeds_35_54_115_outer_corners_stepping_keep_their_hip_beams_on_their_own_roof() {
    // Found by `cargo xtask fuzz` at seeds 35, 54 and 115, shrunk: a 2x2x2 box of shakes
    // whose top layer is four outer corners alike, so the roof steps down a cube past
    // each corner's peak (gap 11), on its own or with a wall standing in half of it. A
    // hip's beam, brought back plumb at its peak or cut at the wall there, still reached
    // its own width past the corner over the lower cube beside it, its side and sawn end
    // 11 to 12 cm over that roof.
    let materials = baseline();
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .unwrap()
    };
    let corners = |at, facing| Blocks {
        at,
        size: 2,
        shape: Shape::OuterCorner(facing),
        kind: kind("shakes"),
    };
    for elements in [
        vec![corners([9, 1, -2], Facing::East)],
        vec![
            Room {
                min: [0, 4],
                max: [6, 10],
                storey: 0,
                kind: kind("stone"),
            },
            corners([7, 5, 4], Facing::West),
        ],
        vec![
            Wall {
                from: [3, 2],
                to: [10, 2],
                storey: 0,
                kind: kind("logs"),
            },
            corners([6, 3, 3], Facing::West),
        ],
    ] {
        let base = build(&materials.kits, &elements);
        let report = terra_build_mesh::inspect::inspect(
            &base,
            &materials.kits,
            &terra_build_mesh::relief::Maps::worst(&materials.kits, false),
        );
        let out: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.fault == terra_build_mesh::inspect::Fault::PokesOut)
            .collect();
        assert!(out.is_empty(), "{:?}: {out:#?}", elements.last());
    }
}

#[test]
fn fuzz_seeds_61_72_196_an_eave_at_a_steps_top_is_measured_from_its_own_roof() {
    // Found by `cargo xtask fuzz` at seeds 61, 72 and 196, shrunk: a 2x2x2 box of slate
    // inner corners or of shakes slopes, so the roof steps down a cube between them, on
    // its own or beside a wall (gap 11). The higher cube's level edge at the step's top
    // is an eave: its fascia hangs plumb out past it and the boards close the step under
    // it, as at the top of any step, up to the step's height over the lower roof. The
    // check measured them from that lower roof (12 to 20 cm over it, past a covering's
    // reach), not from the eave they hang from.
    let materials = baseline();
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .unwrap()
    };
    let blocks = |at, shape, name| Blocks {
        at,
        size: 2,
        shape,
        kind: kind(name),
    };
    for elements in [
        vec![blocks(
            [9, -1, 3],
            Shape::InnerCorner(Facing::South),
            "slate",
        )],
        vec![
            Wall {
                from: [0, 2],
                to: [3, 2],
                storey: 0,
                kind: kind("logs"),
            },
            blocks([1, 4, 0], Shape::InnerCorner(Facing::West), "slate"),
        ],
        vec![
            Room {
                min: [0, 4],
                max: [5, 7],
                storey: 0,
                kind: kind("hewn"),
            },
            blocks([2, 2, 9], Shape::Slope(Facing::North), "shakes"),
        ],
    ] {
        let base = build(&materials.kits, &elements);
        let report = terra_build_mesh::inspect::inspect(
            &base,
            &materials.kits,
            &terra_build_mesh::relief::Maps::worst(&materials.kits, false),
        );
        let out: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.fault == terra_build_mesh::inspect::Fault::PokesOut)
            .collect();
        assert!(out.is_empty(), "{:?}: {out:#?}", elements.last());
    }
}

#[test]
fn fuzz_seed_180_no_batten_stands_out_past_a_steps_top_under_a_wall() {
    // Found by `cargo xtask fuzz` at seed 180, shrunk: a 2x2x2 box of shakes slopes, so
    // the roof steps down a cube between them, with a room's log wall standing across the
    // step (gap 11). The batten under the higher face's top course was laid as on a roof
    // running on, as the lower roof rises on beyond the step: its corner stood 3 mm out past
    // the step's edge and its end 4 mm past the wall, 20 cm over the lower roof.
    let materials = baseline();
    let kind = |name: &str| {
        materials
            .names
            .iter()
            .find(|(_, n)| n.as_str() == name)
            .map(|(&k, _)| k)
            .unwrap()
    };
    let elements = vec![
        Blocks {
            at: [6, 0, 7],
            size: 2,
            shape: Shape::Slope(Facing::West),
            kind: kind("shakes"),
        },
        Room {
            min: [3, 2],
            max: [7, 6],
            storey: 0,
            kind: kind("logs"),
        },
    ];
    let base = build(&materials.kits, &elements);
    let report = terra_build_mesh::inspect::inspect(
        &base,
        &materials.kits,
        &terra_build_mesh::relief::Maps::worst(&materials.kits, false),
    );
    let out: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.fault == terra_build_mesh::inspect::Fault::PokesOut)
        .collect();
    assert!(out.is_empty(), "{out:#?}");
}
