//! The operations' rules, case by case, and properties over random operations: after
//! every step nothing floats, nothing is taken while held, what comes out gives back
//! exactly what went in, and undo returns the state. A failure names its seed and step, so
//! it can be run again exactly.

use super::*;
use crate::cost::Amount;
use crate::cubes::Facing;
use crate::testing::random::{self, DOOR, LOGS, STONE, THATCH, Yard, plenty, rules};
use glam::Vec3;

/// A plan of a row of cubes from one place, so many along a step, on level ground.
fn row(part: u32, kind: Kind, from: [i32; 3], step: IVec3, count: i32) -> Op {
    let cubes: Vec<Planned> = (0..count)
        .map(|i| Planned {
            at: (IVec3::from_array(from) + step * i).to_array(),
            shape: Shape::Whole,
        })
        .collect();
    let ground = cubes
        .iter()
        .map(|cube| Sample {
            column: [cube.at[0], cube.at[2]],
            height_cm: 0,
        })
        .collect();
    Op::LayPlan {
        part: Some(PartId(part)),
        kind,
        cubes,
        ground,
    }
}

fn line(from: [i32; 3], step: IVec3, count: i32) -> Vec<[i32; 3]> {
    (0..count)
        .map(|i| (IVec3::from_array(from) + step * i).to_array())
        .collect()
}

/// A wall of stone four cubes long and so many high, built.
fn stone_wall(base: &mut Base, high: i32, cx: &Context) {
    for y in 0..high {
        base.act(&row(y as u32, STONE, [0, y, 0], IVec3::X, 4), cx)
            .unwrap();
        base.act(
            &Op::Fill {
                cubes: line([0, y, 0], IVec3::X, 4),
            },
            cx,
        )
        .unwrap();
    }
}

#[test]
fn nothing_goes_in_floating_or_before_what_it_stands_on() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A cube a metre up, alone: planned as a ghost, but not built.
    base.act(&row(0, STONE, [0, 2, 0], IVec3::X, 1), &cx)
        .unwrap();
    let up = Op::Fill {
        cubes: vec![[0, 2, 0]],
    };
    assert_eq!(
        base.act(&up, &cx),
        Err(Refusal::Floats(IVec3::new(0, 2, 0)))
    );
    // A column planned under it: built from the bottom only.
    base.act(&row(1, STONE, [0, 0, 0], IVec3::Y, 2), &cx)
        .unwrap();
    let middle = Op::Fill {
        cubes: vec![[0, 1, 0]],
    };
    assert_eq!(
        base.act(&middle, &cx),
        Err(Refusal::Floats(IVec3::new(0, 1, 0)))
    );
    // The whole column at once goes in: its foot is on the ground.
    let column = Op::Fill {
        cubes: line([0, 0, 0], IVec3::Y, 3),
    };
    base.act(&column, &cx).unwrap();
    assert!(base.floating().is_empty());
    // And a beam off its top, joined to it, may reach out.
    base.act(&row(2, STONE, [1, 2, 0], IVec3::X, 2), &cx)
        .unwrap();
    base.act(
        &Op::Fill {
            cubes: line([1, 2, 0], IVec3::X, 2),
        },
        &cx,
    )
    .unwrap();
    assert!(base.floating().is_empty());
    // Logs: no course over a cube with only one of its two in.
    base.act(&row(3, LOGS, [5, 0, 0], IVec3::Y, 2), &cx)
        .unwrap();
    base.act(
        &Op::Fill {
            cubes: vec![[5, 0, 0]],
        },
        &cx,
    )
    .unwrap();
    assert_eq!(
        base.act(
            &Op::Fill {
                cubes: vec![[5, 1, 0]]
            },
            &cx
        ),
        Err(Refusal::BuildUnder(IVec3::new(5, 0, 0)))
    );
}

#[test]
fn nothing_is_taken_while_something_rests_on_it() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    stone_wall(&mut base, 3, &cx);
    // The bottom course holds the next.
    let foot = Op::Empty {
        cubes: vec![[1, 0, 0]],
    };
    assert_eq!(
        base.act(&foot, &cx),
        Err(Refusal::Holds(vec![Holder::Cube(IVec3::new(1, 1, 0))]))
    );
    // A beam out from the top holds by the wall's end; take the end and it would fall.
    base.act(&row(9, STONE, [4, 2, 0], IVec3::X, 2), &cx)
        .unwrap();
    base.act(
        &Op::Fill {
            cubes: line([4, 2, 0], IVec3::X, 2),
        },
        &cx,
    )
    .unwrap();
    let end = Op::Empty {
        cubes: vec![[3, 2, 0]],
    };
    assert!(matches!(base.act(&end, &cx), Err(Refusal::Holds(_))));
    // An item on the top: refused, saying so.
    let yard = Yard {
        stock: plenty(),
        items: BTreeSet::from([[0, 2, 0]]),
        ..Yard::default()
    };
    let with_item = Context { world: &yard, ..cx };
    let top = Op::Empty {
        cubes: vec![[0, 2, 0]],
    };
    assert_eq!(
        base.act(&top, &with_item),
        Err(Refusal::Holds(vec![Holder::Item(IVec3::new(0, 2, 0))]))
    );
    assert!(base.act(&top, &cx).is_ok());
}

#[test]
fn a_roof_goes_up_on_built_walls_and_holds_them_while_it_stands() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // Two walls a storey high, north and south, under a gable of roof cubes.
    for (i, z) in [0, 3].into_iter().enumerate() {
        let part = PartId(i as u32);
        let cubes = (0..4)
            .flat_map(|x| {
                (0..6).map(move |y| Planned {
                    at: [x, y, z],
                    shape: Shape::Whole,
                })
            })
            .collect();
        let plan = Op::LayPlan {
            part: Some(part),
            kind: STONE,
            cubes,
            ground: Vec::new(),
        };
        base.act(&plan, &cx).unwrap();
    }
    let footprint = (0..4).flat_map(|x| (0..4).map(move |z| [x, z])).collect();
    let roof = crate::roofer::over(&footprint, 6, crate::roofer::Style::Gable, None);
    let plan = Op::LayPlan {
        part: Some(PartId(2)),
        kind: THATCH,
        cubes: roof
            .roof
            .iter()
            .map(|&(at, shape)| Planned {
                at: at.to_array(),
                shape,
            })
            .collect(),
        ground: Vec::new(),
    };
    base.act(&plan, &cx).unwrap();
    let eaves = Op::Fill {
        cubes: (0..4).flat_map(|x| [[x, 6, 0], [x, 6, 3]]).collect(),
    };
    assert!(matches!(base.act(&eaves, &cx), Err(Refusal::Floats(_))));
    // One end of each wall up is not enough for the whole eave.
    let fill = |base: &mut Base, columns: std::ops::Range<i32>| {
        for y in 0..6 {
            let course = Op::Fill {
                cubes: columns
                    .clone()
                    .flat_map(|x| [[x, y, 0], [x, y, 3]])
                    .collect(),
            };
            base.act(&course, &cx).unwrap();
        }
    };
    fill(&mut base, 0..4);
    let done = base.act(&eaves, &cx).unwrap();
    assert_eq!(done.took, Bill::of("Thatch", Amount::whole(2)));
    // The wall top under an eave holds the roof now.
    let eave = Op::Empty {
        cubes: vec![[2, 5, 3]],
    };
    assert_eq!(
        base.act(&eave, &cx),
        Err(Refusal::Holds(vec![Holder::Cube(IVec3::new(2, 6, 3))]))
    );
    // Down again, and the walls can come apart.
    base.act_all(&done.undo, &cx).unwrap();
    assert!(base.act(&eave, &cx).is_ok());
}

#[test]
fn a_door_fits_a_hole_in_a_built_wall_and_holds_its_frame() {
    let rules = rules();
    let mut yard = Yard {
        stock: plenty(),
        ..Yard::default()
    };
    let mut base = Base::default();
    {
        let cx = Context {
            rules: &rules,
            claim: None,
            world: &yard,
        };
        // A wall five cubes long and five high with a hole a cube wide and four high.
        let cubes: Vec<Planned> = (0..5)
            .flat_map(|x| (0..5).map(move |y| [x, y, 0]))
            .filter(|&[x, y, _]| !(x == 2 && y < 4))
            .map(|at| Planned {
                at,
                shape: Shape::Whole,
            })
            .collect();
        let places: Vec<[i32; 3]> = cubes.iter().map(|cube| cube.at).collect();
        let plan = Op::LayPlan {
            part: Some(PartId(0)),
            kind: STONE,
            cubes,
            ground: Vec::new(),
        };
        base.act(&plan, &cx).unwrap();
        let door = Prop {
            kind: DOOR,
            min: [2, 0, 0],
            max: [2, 3, 0],
            open: false,
            ..Prop::default()
        };
        assert_eq!(base.act(&Op::FitProp(door), &cx), Err(Refusal::NoHole));
        for y in 0..5 {
            let course: Vec<[i32; 3]> = places.iter().copied().filter(|at| at[1] == y).collect();
            base.act(&Op::Fill { cubes: course }, &cx).unwrap();
        }
        let too_big = Prop {
            max: [2, 4, 0],
            ..door
        };
        assert_eq!(
            base.act(&Op::FitProp(too_big), &cx),
            Err(Refusal::Taken(IVec3::new(2, 4, 0)))
        );
        let done = base.act(&Op::FitProp(door), &cx).unwrap();
        assert_eq!(done.took, Bill::of("Door", Amount::whole(1)));
        // The jamb beside it, and the lintel over it, hold it.
        let lintel = Op::Empty {
            cubes: vec![[2, 4, 0]],
        };
        assert_eq!(
            base.act(&lintel, &cx),
            Err(Refusal::Holds(vec![Holder::Prop(IVec3::new(2, 0, 0))]))
        );
        base.act(
            &Op::SetOpen {
                at: [2, 0, 0],
                open: true,
            },
            &cx,
        )
        .unwrap();
        assert!(base.prop(IVec3::new(2, 0, 0)).unwrap().open);
        // Taken out, it gives back the door; fitted again as it was, open.
        let taken = base.act(&Op::RemoveProp { at: [2, 0, 0] }, &cx).unwrap();
        assert_eq!(taken.gave, Bill::of("Door", Amount::whole(1)));
        base.act_all(&taken.undo, &cx).unwrap();
        assert!(base.prop(IVec3::new(2, 0, 0)).unwrap().open);
        // Nothing can be planned where it is.
        assert_eq!(
            base.act(&row(5, STONE, [2, 1, 0], IVec3::X, 1), &cx),
            Err(Refusal::PropThere(IVec3::new(2, 0, 0)))
        );
    }
    // Someone standing in the hole keeps a door out of it.
    base.act(&Op::RemoveProp { at: [2, 0, 0] }, &Context::free(&rules))
        .unwrap();
    yard.bodies.insert([2, 1, 0]);
    let cx = Context {
        rules: &rules,
        claim: None,
        world: &yard,
    };
    let door = Prop {
        kind: DOOR,
        min: [2, 0, 0],
        max: [2, 3, 0],
        open: false,
        ..Prop::default()
    };
    assert_eq!(
        base.act(&Op::FitProp(door), &cx),
        Err(Refusal::InTheWay(IVec3::new(2, 1, 0)))
    );
}

#[test]
fn a_claim_bounds_plans_and_their_budget() {
    let rules = rules();
    let claim = Claim {
        min: [0, 0],
        max: [9, 9],
        budget: 6,
    };
    let cx = Context {
        claim: Some(&claim),
        ..Context::free(&rules)
    };
    let mut base = Base::default();
    assert_eq!(
        base.act(&row(0, STONE, [8, 0, 0], IVec3::X, 4), &cx),
        Err(Refusal::OutsideClaim(IVec3::new(10, 0, 0)))
    );
    base.act(&row(0, STONE, [0, 0, 0], IVec3::X, 4), &cx)
        .unwrap();
    assert_eq!(
        base.act(&row(1, STONE, [0, 0, 1], IVec3::X, 4), &cx),
        Err(Refusal::OverBudget { budget: 6 })
    );
    // Building some frees the budget.
    base.act(
        &Op::Fill {
            cubes: line([0, 0, 0], IVec3::X, 4),
        },
        &cx,
    )
    .unwrap();
    assert!(
        base.act(&row(1, STONE, [0, 0, 1], IVec3::X, 4), &cx)
            .is_ok()
    );
    // A part is of one kind.
    assert_eq!(
        base.act(&row(1, LOGS, [0, 0, 2], IVec3::X, 1), &cx),
        Err(Refusal::WrongPart(PartId(1)))
    );
}

#[test]
fn materials_are_taken_and_given_back_exactly() {
    let rules = rules();
    let yard = Yard {
        stock: Bill::of("Log", Amount::new(1, 4)),
        ..Yard::default()
    };
    let cx = Context {
        rules: &rules,
        claim: None,
        world: &yard,
    };
    let mut base = Base::default();
    base.act(&row(0, LOGS, [0, 0, 0], IVec3::X, 3), &cx)
        .unwrap();
    // Three cubes' course is three eighths of a log: more than there is.
    let course = Op::Fill {
        cubes: line([0, 0, 0], IVec3::X, 3),
    };
    assert_eq!(
        base.act(&course, &cx),
        Err(Refusal::Materials(Bill::of("Log", Amount::new(3, 8))))
    );
    let two = Op::Fill {
        cubes: line([0, 0, 0], IVec3::X, 2),
    };
    let done = base.act(&two, &cx).unwrap();
    assert_eq!(done.took, Bill::of("Log", Amount::new(1, 4)));
    let back = base.act_all(&done.undo, &cx).unwrap();
    assert_eq!(back.gave, done.took);
}

#[test]
fn a_stone_wall_is_changed_to_logs_old_out_new_in() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    stone_wall(&mut base, 1, &cx);
    let cubes = line([0, 0, 0], IVec3::X, 4);
    base.act(
        &Op::Replace {
            cubes: cubes.clone(),
            kind: LOGS,
            part: Some(PartId(7)),
        },
        &cx,
    )
    .unwrap();
    let marked = base.cube(IVec3::ZERO).unwrap();
    assert_eq!((marked.kind, marked.pending), (STONE, Pending::Replace));
    let swap = Op::Swap {
        cubes: cubes.clone(),
    };
    assert_eq!(base.act(&swap, &cx), Err(Refusal::InProgress(IVec3::ZERO)));
    let out = base
        .act(
            &Op::Empty {
                cubes: cubes.clone(),
            },
            &cx,
        )
        .unwrap();
    assert_eq!(out.gave.0["Stone"], Amount::whole(4));
    base.act(&swap, &cx).unwrap();
    let swapped = base.cube(IVec3::ZERO).unwrap();
    assert_eq!((swapped.kind, swapped.part), (LOGS, Some(PartId(7))));
    let fill = Op::Fill {
        cubes: cubes.clone(),
    };
    base.act(&fill, &cx).unwrap();
    base.act(&fill, &cx).unwrap();
    base.act(&Op::Cancel { cubes }, &cx).unwrap();
    let done = base.cube(IVec3::ZERO).unwrap();
    assert_eq!(
        (done.kind, done.pending, done.is_built()),
        (LOGS, Pending::None, true)
    );
    // The stone's part is gone with its last cube.
    assert!(base.part(PartId(0)).is_none());
}

#[test]
fn a_cube_in_a_wall_is_changed_in_place_under_what_stays_and_each_step_undone() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    stone_wall(&mut base, 3, &cx);
    let before = base.clone();
    let cube = IVec3::new(1, 1, 0);
    base.act(
        &Op::Replace {
            cubes: vec![cube.to_array()],
            kind: LOGS,
            part: Some(PartId(9)),
        },
        &cx,
    )
    .unwrap();
    // Driven by what goes next, as the hammer is: the stone out from under the stone over
    // it, the swap, two courses of log in, and the mark cleared.
    let mut stock = plenty();
    let mut undo = Vec::new();
    let mut steps = Vec::new();
    while let Some(op) = base.next_change(Vec3::ZERO, |kind| kind == STONE) {
        let done = base
            .act(&op, &cx)
            .unwrap_or_else(|why| panic!("{op:?}: {why:?}"));
        stock.add(&done.gave);
        stock.take(&done.took).expect("in stock");
        steps.push(
            format!("{op:?}")
                .split([' ', '{'])
                .next()
                .unwrap()
                .to_string(),
        );
        undo.push(done.undo);
        // What stays over it is held all along.
        assert!(base.floating().is_empty());
        assert!(base.is_in(cube + IVec3::Y));
        assert!(steps.len() < 10);
    }
    assert_eq!(steps, ["Empty", "Swap", "Fill", "Fill", "Cancel"]);
    let changed = base.cube(cube).unwrap();
    assert_eq!(
        (
            changed.kind,
            changed.part,
            changed.pending,
            changed.is_built()
        ),
        (LOGS, Some(PartId(9)), Pending::None, true)
    );
    // The stone given back, the logs taken.
    let mut want = plenty();
    want.add(&Bill::of("Stone", Amount::whole(1)));
    want.add(&Bill::of("Mortar", Amount::new(3, 8)));
    want.take(&Bill::of("Log", Amount::new(1, 4))).unwrap();
    assert_eq!(stock, want);
    // Each step undone, last first, returns the wall as it was.
    for ops in undo.iter().rev() {
        base.act_all(ops, &cx).unwrap();
    }
    base.act(
        &Op::Cancel {
            cubes: vec![cube.to_array()],
        },
        &cx,
    )
    .unwrap();
    assert_eq!(base, before);
}

#[test]
fn every_shape_is_supported_costed_built_in_order_and_taken_apart_as_a_whole_cube() {
    let rules = rules();
    let cx = Context::free(&rules);
    for shape in Shape::ALL {
        let mut base = Base::default();
        // A stone cube on the ground, and the shape on it, then one over it.
        let plan = |part: u32, at: [i32; 3], shape: Shape| Op::LayPlan {
            part: Some(PartId(part)),
            kind: STONE,
            cubes: vec![Planned { at, shape }],
            ground: vec![Sample {
                column: [0, 0],
                height_cm: 0,
            }],
        };
        base.act(&plan(0, [0, 0, 0], Shape::Whole), &cx).unwrap();
        base.act(&plan(1, [0, 1, 0], shape), &cx).unwrap();
        // Not before what it rests on.
        assert_eq!(
            base.next_in(PartId(1), Vec3::ZERO, true),
            Some(vec![[0, 1, 0]])
        );
        let fill = |at: [i32; 3]| Op::Fill { cubes: vec![at] };
        assert!(base.act(&fill([0, 1, 0]), &cx).is_err(), "{shape:?} floats");
        base.act(&fill([0, 0, 0]), &cx).unwrap();
        let done = base.act(&fill([0, 1, 0]), &cx).unwrap();
        assert_eq!(done.took, rules.of(STONE).per_step, "{shape:?}");
        // Something on it holds it.
        base.act(&plan(2, [0, 2, 0], Shape::Whole), &cx).unwrap();
        base.act(&fill([0, 2, 0]), &cx).unwrap();
        let empty = Op::Empty {
            cubes: vec![[0, 1, 0]],
        };
        assert!(
            matches!(base.act(&empty, &cx), Err(Refusal::Holds(_))),
            "{shape:?}"
        );
        assert_eq!(
            base.last_in(PartId(2), Vec3::ZERO, true),
            Some(vec![[0, 2, 0]])
        );
        base.act(
            &Op::Empty {
                cubes: vec![[0, 2, 0]],
            },
            &cx,
        )
        .unwrap();
        let back = base.act(&empty, &cx).unwrap();
        assert_eq!(back.gave, done.took);
        assert_eq!(base.cube(IVec3::Y).unwrap().shape, shape);
    }
}

#[test]
fn a_cube_carved_out_comes_out_step_by_step_and_its_undo_puts_it_back() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    stone_wall(&mut base, 2, &cx);
    let before = base.clone();
    let carve = [
        Op::Carve {
            cubes: vec![[1, 1, 0]],
        },
        Op::Empty {
            cubes: vec![[1, 1, 0]],
        },
        Op::Unplan {
            cubes: vec![[1, 1, 0]],
        },
    ];
    let done = base.act_all(&carve, &cx).unwrap();
    assert_eq!(base.cube(IVec3::new(1, 1, 0)), None);
    base.act_all(&done.undo, &cx).unwrap();
    assert_eq!(base, before);
}

/// A log wall eight cubes long and six high, built course by course on level ground.
fn log_wall(base: &mut Base, cx: &Context) {
    for y in 0..6 {
        base.act(&row(y as u32, LOGS, [0, y, 0], IVec3::X, 8), cx)
            .unwrap();
        let course = Op::Fill {
            cubes: line([0, y, 0], IVec3::X, 8),
        };
        base.act(&course, cx).unwrap();
        base.act(&course, cx).unwrap();
    }
}

#[test]
fn a_window_is_cut_into_a_built_wall_under_the_logs_that_span_it() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    log_wall(&mut base, &cx);
    let before = base.clone();
    // A window two cubes wide and two high, a metre up, with two layers of wall over it.
    let window: Vec<[i32; 3]> = [3, 4]
        .into_iter()
        .flat_map(|x| [2, 3].map(|y| [x, y, 0]))
        .collect();
    base.act(
        &Op::Carve {
            cubes: window.clone(),
        },
        &cx,
    )
    .unwrap();
    // Out from the top, a course at a time, under the logs that stay: each gives back
    // what went in.
    let mut gave = Bill::default();
    let mut undos = Vec::new();
    for layer in [3, 2] {
        let cubes: Vec<[i32; 3]> = window.iter().copied().filter(|at| at[1] == layer).collect();
        for _ in 0..2 {
            let done = base
                .act(
                    &Op::Empty {
                        cubes: cubes.clone(),
                    },
                    &cx,
                )
                .unwrap();
            gave.add(&done.gave);
            undos.push(done.undo);
            assert!(base.floating().is_empty());
        }
    }
    // Four cubes, each two courses of an eighth of a log.
    assert_eq!(gave, Bill::of("Log", Amount::whole(1)));
    assert!(base.is_in(IVec3::new(3, 4, 0)) && base.is_in(IVec3::new(4, 5, 0)));
    // Each step undone puts it back, in turn.
    let mut undone = base.clone();
    for undo in undos.iter().rev() {
        undone.act_all(undo, &cx).unwrap();
    }
    undone
        .act(
            &Op::Cancel {
                cubes: window.clone(),
            },
            &cx,
        )
        .unwrap();
    assert_eq!(undone, before);
    // Made an opening, a window fits it, the wall either side and over it its frame.
    base.act(
        &Op::Open {
            cubes: window.clone(),
        },
        &cx,
    )
    .unwrap();
    let prop = Prop {
        kind: DOOR,
        min: [3, 2, 0],
        max: [4, 3, 0],
        open: false,
        ..Prop::default()
    };
    base.act(&Op::FitProp(prop), &cx).unwrap();
}

#[test]
fn a_cut_is_refused_where_it_would_leave_the_wall_over_it_hanging_or_a_roof_unheld() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    log_wall(&mut base, &cx);
    // Across the whole wall a metre up: what is over it would hang from nothing.
    let across = line([0, 2, 0], IVec3::X, 8);
    base.act(
        &Op::Carve {
            cubes: across.clone(),
        },
        &cx,
    )
    .unwrap();
    let both = Op::Empty {
        cubes: across.clone(),
    };
    base.act(&both, &cx).unwrap();
    assert!(matches!(base.act(&both, &cx), Err(Refusal::Holds(_))));
    // The top of a wall under a roof's eaves: a cube of it may come out from under the
    // roof, which its neighbours hold, but not the whole top, which would leave the roof
    // hanging.
    let mut roofed = Base::default();
    log_wall(&mut roofed, &cx);
    let eaves: Vec<[i32; 3]> = (0..8).map(|x| [x, 6, 0]).collect();
    roofed
        .act(
            &Op::LayPlan {
                part: Some(PartId(9)),
                kind: THATCH,
                cubes: eaves
                    .iter()
                    .map(|&at| Planned {
                        at,
                        shape: Shape::Slope(Facing::South),
                    })
                    .collect(),
                ground: Vec::new(),
            },
            &cx,
        )
        .unwrap();
    roofed.act(&Op::Fill { cubes: eaves }, &cx).unwrap();
    let top = line([0, 5, 0], IVec3::X, 8);
    roofed.act(&Op::Carve { cubes: top.clone() }, &cx).unwrap();
    // Its first course out, then the last would leave the roof hanging.
    let out = Op::Empty { cubes: top };
    roofed.act(&out, &cx).unwrap();
    assert!(matches!(
        roofed.act(&out, &cx),
        Err(Refusal::Holds(holders)) if holders.contains(&Holder::Cube(IVec3::new(2, 6, 0)))
    ));
    assert!(
        roofed
            .act(
                &Op::Empty {
                    cubes: vec![[2, 5, 0]]
                },
                &cx
            )
            .is_ok()
    );
}

#[test]
fn a_plan_on_a_slope_keeps_the_ground_it_was_laid_on() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A footing down a slope: the ground at 0 under the first column and a metre down
    // under the second.
    let plan = Op::LayPlan {
        part: Some(PartId(0)),
        kind: STONE,
        cubes: vec![
            Planned {
                at: [0, 0, 0],
                shape: Shape::Whole,
            },
            Planned {
                at: [1, -2, 0],
                shape: Shape::Whole,
            },
            Planned {
                at: [1, -1, 0],
                shape: Shape::Half,
            },
        ],
        ground: vec![
            Sample {
                column: [0, 0],
                height_cm: 0,
            },
            Sample::metres([1, 0], -1.0),
        ],
    };
    base.act(&plan, &cx).unwrap();
    assert_eq!(base.ground().sample([1, 0]), Some(-100));
    assert!(base.grounded(IVec3::new(1, -2, 0)) && !base.grounded(IVec3::new(1, -1, 0)));
    let done = base
        .act(
            &Op::SetShape {
                cubes: vec![[1, -1, 0], [0, 0, 0]],
                shape: Shape::Stair(Facing::East),
            },
            &cx,
        )
        .unwrap();
    assert_eq!(done.undo.len(), 2);
    // Unplanned, the ground under it is forgotten, and laid again, remembered.
    let all = vec![[0, 0, 0], [1, -2, 0], [1, -1, 0]];
    let before = base.clone();
    let gone = base.act(&Op::Unplan { cubes: all }, &cx).unwrap();
    assert!(base.is_empty() && base.ground().samples().next().is_none());
    base.act_all(&gone.undo, &cx).unwrap();
    assert_eq!(base, before);
}

/// A flight of stairs in stone, a cube wide, rising north from the ground three cubes,
/// planned as one part: each column whole cubes under a stair.
fn flight(part: u32, top: Shape) -> Op {
    let mut cubes = Vec::new();
    for i in 0..3 {
        for y in 0..=i {
            cubes.push(Planned {
                at: [0, y, -i],
                shape: if y == i { top } else { Shape::Whole },
            });
        }
    }
    let ground = (0..3)
        .map(|i| Sample {
            column: [0, -i],
            height_cm: 0,
        })
        .collect();
    Op::LayPlan {
        part: Some(PartId(part)),
        kind: STONE,
        cubes,
        ground,
    }
}

#[test]
fn shaped_cubes_are_built_held_and_taken_apart_as_whole_ones_are() {
    let rules = rules();
    let cx = Context::free(&rules);
    for top in [
        Shape::Stair(Facing::North),
        Shape::Slope(Facing::North),
        Shape::Half,
    ] {
        let mut base = Base::default();
        base.act(&flight(0, top), &cx).unwrap();
        // Bottom up, a cube at a time, each joined to the ground or to cubes in.
        let mut took = Bill::default();
        while let Some((at, _)) = base
            .cubes()
            .filter(|(_, cube)| !cube.is_built())
            .min_by_key(|(at, _)| (at.y, at.z))
        {
            let done = base
                .act(
                    &Op::Fill {
                        cubes: vec![at.to_array()],
                    },
                    &cx,
                )
                .unwrap_or_else(|why| panic!("{top:?} at {at}: {why:?}"));
            took.add(&done.took);
            assert!(base.floating().is_empty(), "{top:?}");
        }
        // A shaped cube costs what a whole one of its kind does: six cubes of stone.
        assert_eq!(took, rules.kinds[&STONE].per_step.times(6), "{top:?}");
        // The whole cube under the top step holds it, so it is not taken out.
        let under = Op::Empty {
            cubes: vec![[0, 1, -2]],
        };
        assert!(base.act(&under, &cx).is_err(), "{top:?}");
        // The top step comes out, and gives back what it took.
        let out = base
            .act(
                &Op::Empty {
                    cubes: vec![[0, 2, -2]],
                },
                &cx,
            )
            .unwrap();
        assert_eq!(out.gave, rules.kinds[&STONE].per_step, "{top:?}");
        assert_eq!(base.cube(IVec3::new(0, 2, -2)).unwrap().shape, top);
    }
}

/// What is in a base: each cube's steps, each prop.
fn value(base: &Base, rules: &Rules) -> Bill {
    let mut bill = Bill::default();
    for (_, cube) in base.cubes().filter(|(_, cube)| cube.fill != Fill::Open) {
        let terms = base.terms(&cube);
        let steps = u64::from(cube.level() / terms.step());
        bill.add(&terms.per_step.times(steps));
    }
    for prop in base.props() {
        bill.add(&rules.prop(prop.kind));
    }
    bill
}

/// What the rules say must hold of every base after every operation.
fn check_rules(base: &Base, at: &str) {
    let floating = base.floating();
    assert!(floating.is_empty(), "{at}: {floating:?} float");
    for (cube, _) in base.cubes().filter(|(_, cube)| cube.is_in()) {
        let under = base.cube(cube - IVec3::Y);
        assert!(
            under.is_none_or(|under| {
                !under.is_in() || under.is_built() || under.pending.in_place()
            }),
            "{at}: {cube} is in over a cube partly in"
        );
    }
    for prop in base.props() {
        assert!(
            base.hole_for(prop).is_some(),
            "{at}: the prop at {:?} is in no hole",
            prop.min
        );
    }
    // Only what is in is damaged, and never past its health.
    for (cube, _) in base.damaged() {
        let found = base.cube(cube).filter(Cube::is_in);
        assert!(found.is_some(), "{at}: {cube} damaged but not in");
    }
}

/// Whether operations filled cubes that, now in, hold up these: an undo of them is
/// rightly refused.
fn fills_under_what_rests(ops: &[Op], after: &Base, holders: &[Holder]) -> bool {
    let [Op::Fill { cubes }] = ops else {
        return false;
    };
    let resting: Vec<Holder> = cubes
        .iter()
        .flat_map(|&at| after.resting_on(IVec3::from_array(at)))
        .collect();
    holders.iter().all(|holder| resting.contains(holder))
}

// Random operations (`testing::random`, which the layout's tests lay too).

#[test]
fn random_operations_keep_every_rule() {
    let rules = rules();
    let claim = random::claim();
    let mut accepted: BTreeMap<String, usize> = BTreeMap::new();
    for seed in 0..random::seeds() {
        // `random::bases` draws as this does, so the bases it gives are those checked
        // here; a draw added here is added there.
        let mut rng = random::rng(seed);
        let mut yard = Yard {
            stock: plenty(),
            ..Yard::default()
        };
        let mut base = Base::default();
        let mut log: Vec<Op> = Vec::new();
        let (mut again, mut replayed) = (Base::default(), 0);
        let start = random::start(seed);
        let free = Context::free(&rules);
        let begun = base.act_all(&start, &free).unwrap();
        yard.stock.take(&begun.took).unwrap();
        let (mut paid, mut refunded) = (begun.took, Bill::default());
        // What harm destroyed and repairs used up, and what undoing them brought back.
        let (mut lost, mut recovered) = (Bill::default(), Bill::default());
        log.extend(start);
        let mut undos: Vec<Vec<Op>> = Vec::new();
        for step in 0..1200 {
            let at = format!("seed {seed} step {step}");
            if step % 50 == 0 {
                // Someone walks about; things are set down.
                yard.bodies = (0..3)
                    .map(|_| random::somewhere(&mut rng).to_array())
                    .collect();
                yard.items = (0..3)
                    .map(|_| random::somewhere(&mut rng).to_array())
                    .collect();
            }
            let roll = rng.below(31);
            let undo = roll >= 27;
            let ops = if undo {
                match undos.pop() {
                    Some(ops) => ops,
                    None => continue,
                }
            } else {
                vec![random::random_op(&base, &mut rng, seed, roll)]
            };
            let before = base.clone();
            let cx = Context {
                rules: &rules,
                claim: Some(&claim),
                world: &yard,
            };
            match base.act_all(&ops, &cx) {
                Ok(done) => {
                    if !undo {
                        let name = format!("{:?}", ops[0]);
                        let name = name.split([' ', '(', '{']).next().unwrap_or("").to_string();
                        *accepted.entry(name).or_default() += 1;
                    }
                    assert!(
                        yard.stock.take(&done.took).is_some(),
                        "{at}: took what was not there"
                    );
                    yard.stock.add(&done.gave);
                    paid.add(&done.took);
                    refunded.add(&done.gave);
                    lost.add(&done.lost);
                    recovered.add(&done.recovered);
                    // The columns, kept cube by cube, are those of every cube counted afresh.
                    let mut counted = base.clone();
                    counted.count_columns();
                    assert!(counted == base, "{at}: {ops:?} left the columns wrong");
                    // Nothing was taken from under anything, but a cube carved out or changed from
                    // under what stays joined, or from under an item.
                    for op in &ops {
                        if let Op::Empty { cubes } = op {
                            for &cube in cubes {
                                let cube = IVec3::from_array(cube);
                                // Marked to be carved out, now or by the act itself, as
                                // the undo of a cube put in whole under what stands is.
                                let carved =
                                    base.cube(cube).is_some_and(|cube| cube.pending.in_place())
                                        || ops.iter().any(|op| {
                                            matches!(op, Op::Carve { cubes } if cubes.contains(&cube.to_array()))
                                        });
                                assert!(
                                    !base.is_in(cube + IVec3::Y) || base.is_in(cube) || carved,
                                    "{at}: {cube} taken from under what rests on it"
                                );
                                assert!(
                                    !yard.items.contains(&cube.to_array()),
                                    "{at}: {cube} taken from under an item"
                                );
                            }
                        }
                    }
                    log.extend(ops.iter().cloned());
                    // Undo at once, now and then: back to as it was, and on again.
                    if rng.below(3) == 0 {
                        let after = base.clone();
                        let cx = Context {
                            rules: &rules,
                            claim: Some(&claim),
                            world: &Free,
                        };
                        match base.act_all(&done.undo, &cx) {
                            Ok(_) => {
                                assert_eq!(base, before, "{at}: {ops:?} undone is not as it was");
                                base.act_all(&ops, &cx)
                                    .unwrap_or_else(|why| panic!("{at}: redo refused: {why:?}"));
                                assert_eq!(base, after, "{at}: {ops:?} redone is not as it was");
                            }
                            // Over budget, as taking apart plans what it takes; or a
                            // cube put in under what is already up, which then rests on
                            // it and keeps it (D193).
                            Err(Refusal::OverBudget { .. }) => assert_eq!(base, after),
                            Err(Refusal::Holds(holders))
                                if fills_under_what_rests(&ops, &after, &holders) =>
                            {
                                assert_eq!(base, after)
                            }
                            Err(why) => panic!("{at}: undo of {ops:?} refused: {why:?}"),
                        }
                    }
                    if !undo {
                        undos.push(done.undo);
                    }
                }
                Err(why) => {
                    assert_eq!(base, before, "{at}: {ops:?} refused ({why:?}) yet changed");
                    assert!(!why.message().is_empty());
                }
            }
            check_rules(&base, &format!("{at} after {ops:?}"));
            // What is in, came out and was lost is what went in and was
            // brought back, exactly.
            let mut held = value(&base, &rules);
            held.add(&refunded);
            held.add(&lost);
            let mut put = paid.clone();
            put.add(&recovered);
            assert_eq!(
                held, put,
                "{at}: what is in and what came out is not what went in"
            );
            if step % 300 == 299 {
                // A snapshot reads back the same.
                let snapshot = base.snapshot();
                assert_eq!(Base::from_snapshot(&snapshot).as_ref(), Ok(&base), "{at}");
                // The same operations, in order, build the same base: replayed on a base
                // of their own from the start, carried on from where the last check left
                // it.
                let cx = Context {
                    rules: &rules,
                    claim: Some(&claim),
                    world: &Free,
                };
                for op in &log[replayed..] {
                    again
                        .act(op, &cx)
                        .unwrap_or_else(|why| panic!("{at}: replaying {op:?}: {why:?}"));
                }
                replayed = log.len();
                assert_eq!(again, base, "{at}: replayed, not the same");
            }
        }
    }
    eprintln!("accepted: {accepted:?}");
    let every = [
        "LayPlan",
        "Unplan",
        "Fill",
        "Empty",
        "SetShape",
        "Replace",
        "Swap",
        "Carve",
        "Cancel",
        "FitProp",
        "RemoveProp",
        "SetOpen",
        "Open",
        "Close",
        "Damage",
        "Repair",
        "Subside",
    ];
    for name in every {
        assert!(accepted.contains_key(name), "{name} was never accepted");
    }
}

/// A stone wall five cubes long and five high, built, with a doorway a cube wide and four
/// high planned into it as an opening.
fn wall_with_a_doorway(base: &mut Base, cx: &Context) {
    let cubes: Vec<Planned> = (0..5)
        .flat_map(|x| (0..5).map(move |y| [x, y, 0]))
        .map(|at| Planned {
            at,
            shape: Shape::Whole,
        })
        .collect();
    base.act(
        &Op::LayPlan {
            part: Some(PartId(0)),
            kind: STONE,
            cubes,
            ground: Vec::new(),
        },
        cx,
    )
    .unwrap();
    base.act(
        &Op::Open {
            cubes: line([2, 0, 0], IVec3::Y, 4),
        },
        cx,
    )
    .unwrap();
    for y in 0..5 {
        let course: Vec<[i32; 3]> = (0..5)
            .map(|x| [x, y, 0])
            .filter(|&[x, y, _]| !(x == 2 && y < 4))
            .collect();
        base.act(&Op::Fill { cubes: course }, cx).unwrap();
    }
}

#[test]
fn a_prop_gives_back_what_was_paid_for_it_whatever_it_costs_now() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    wall_with_a_doorway(&mut base, &cx);
    let door = Prop {
        kind: DOOR,
        min: [2, 0, 0],
        max: [2, 3, 0],
        open: false,
        ..Prop::default()
    };
    let fitted = base.act(&Op::FitProp(door), &cx).unwrap();
    // Doors cost more now; taking this one out gives back what went in.
    let mut dearer = rules.clone();
    dearer
        .props
        .insert(DOOR, Bill::of("Door", Amount::whole(3)));
    let taken = base
        .act(&Op::RemoveProp { at: [2, 0, 0] }, &Context::free(&dearer))
        .unwrap();
    assert_eq!(taken.gave, fitted.took);
    // And a snapshot keeps it.
    base.act(&Op::FitProp(door), &cx).unwrap();
    let read = Base::from_snapshot(&base.snapshot()).unwrap();
    assert_eq!(read, base);
}

#[test]
fn only_what_is_built_closes_a_room_in() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A box of stone four by four, its floor at the ground's level and its roof six over
    // it, planned.
    let shell: Vec<[i32; 3]> = (0..4)
        .flat_map(|x| (-1..7).flat_map(move |y| (0..4).map(move |z| [x, y, z])))
        .filter(|&[x, y, z]| x == 0 || x == 3 || z == 0 || z == 3 || y == -1 || y == 6)
        .collect();
    base.act(
        &Op::LayPlan {
            part: Some(PartId(0)),
            kind: STONE,
            cubes: shell
                .iter()
                .map(|&at| Planned {
                    at,
                    shape: Shape::Whole,
                })
                .collect(),
            ground: Vec::new(),
        },
        &cx,
    )
    .unwrap();
    assert_eq!(base.room_cubes(), 0, "a plan encloses nothing");
    for y in -1..7 {
        let course: Vec<[i32; 3]> = shell.iter().copied().filter(|at| at[1] == y).collect();
        base.act(&Op::Fill { cubes: course }, &cx).unwrap();
        let expected = if y < 6 { 0 } else { 2 * 6 * 2 };
        assert_eq!(base.room_cubes(), expected, "after layer {y}");
    }
}

#[test]
fn the_ground_is_sampled_again_unless_that_would_undermine_what_is_built() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    stone_wall(&mut base, 2, &cx);
    // Dug a little under one end: still joined through the rest.
    let one = vec![Sample {
        column: [0, 0],
        height_cm: -80,
    }];
    let done = base.act(&Op::Ground { samples: one }, &cx).unwrap();
    assert_eq!(base.ground().height_cm([0, 0]), -80);
    base.act_all(&done.undo, &cx).unwrap();
    assert_eq!(base.ground().height_cm([0, 0]), 0);
    // Dug from under all of it: refused.
    let all: Vec<Sample> = (0..4)
        .map(|x| Sample {
            column: [x, 0],
            height_cm: -80,
        })
        .collect();
    assert_eq!(
        base.act(&Op::Ground { samples: all }, &cx),
        Err(Refusal::Undermines(IVec3::new(0, 0, 0)))
    );
    // Where nothing stands, there is nothing to keep.
    let elsewhere = vec![Sample {
        column: [9, 9],
        height_cm: -80,
    }];
    assert_eq!(
        base.act(&Op::Ground { samples: elsewhere }, &cx),
        Err(Refusal::Unchanged)
    );
}

mod by_hand;
