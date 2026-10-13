//! The operations' rules, case by case, and properties over random operations: after
//! every step nothing floats, nothing is taken while held, what comes out gives back
//! exactly what went in, layouts are the same laid twice, and undo returns the state. A
//! failure names its seed and step, so it can be run again exactly.

use super::*;
use crate::cost::{Amount, PropKind, Terms};
use crate::cubes::Facing;
use crate::kit::{Kit, Pattern};
use crate::layout::lay;
use crate::{RoofStyle, Walker};
use glam::Vec3;
use terra_gen::{Rng, Seed};

const LOGS: Kind = Kind(1);
const STONE: Kind = Kind(2);
const DOOR: PropKind = PropKind(1);

fn rules() -> Rules {
    let mut stone = Bill::of("Stone", Amount::whole(1));
    stone.add(&Bill::of("Mortar", Amount::new(3, 8)));
    Rules {
        kinds: BTreeMap::from([
            // A log fills a cube in two courses, and spans eight cubes.
            (LOGS, Terms::new(2, Bill::of("Log", Amount::new(1, 8)))),
            (STONE, Terms::new(1, stone)),
        ]),
        roofs: BTreeMap::from([(LOGS, Bill::of("Thatch", Amount::new(1, 4)))]),
        props: BTreeMap::from([(DOOR, Bill::of("Door", Amount::whole(1)))]),
    }
}

/// The world round a base in a test: what is in stock, who stands where, what rests on
/// what.
#[derive(Default)]
struct Yard {
    stock: Bill,
    bodies: BTreeSet<[i32; 3]>,
    items: BTreeSet<[i32; 3]>,
}

impl World for Yard {
    fn in_the_way(&self, at: IVec3) -> bool {
        self.bodies.contains(&at.to_array())
    }
    fn holds_item(&self, at: IVec3) -> bool {
        self.items.contains(&at.to_array())
    }
    fn has(&self, bill: &Bill) -> bool {
        self.stock.covers(bill)
    }
}

fn plenty() -> Bill {
    let mut stock = Bill::default();
    for (material, count) in [
        ("Log", 40),
        ("Stone", 40),
        ("Mortar", 20),
        ("Thatch", 20),
        ("Door", 3),
    ] {
        stock.add(&Bill::of(material, Amount::whole(count)));
    }
    stock
}

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
fn a_log_course_goes_in_across_its_cubes_and_stands_to_its_fill() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    base.act(&row(0, LOGS, [0, 0, 4], IVec3::X, 8), &cx)
        .unwrap();
    let course = Op::Fill {
        cubes: line([0, 0, 4], IVec3::X, 8),
    };
    // The plan is walked through.
    let walker = Walker::default();
    let (from, to) = (Vec3::new(2.0, 0.0, 0.5), Vec3::new(2.0, 0.0, 4.0));
    assert!((base.walk(from, to, &walker) - to).length() < 1e-3);
    // One course: a quarter of a metre, stepped over; it took one log.
    let done = base.act(&course, &cx).unwrap();
    assert_eq!(done.took, Bill::of("Log", Amount::whole(1)));
    let at = Vec3::new(2.0, 0.1, 2.25);
    assert_eq!(base.support(at, &walker), Some(0.25));
    // Two: built, a half metre, and the plan no longer planned.
    base.act(&course, &cx).unwrap();
    assert!(base.cube(IVec3::new(3, 0, 4)).unwrap().is_built());
    assert_eq!(base.store().planned(), 0);
    assert_eq!(base.support(at, &walker), Some(0.5));
    assert_eq!(
        base.act(&course, &cx),
        Err(Refusal::AlreadyBuilt(IVec3::new(0, 0, 4)))
    );
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
    // Two walls a storey high, north and south, under a gable running east and west.
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
    let section = RoofSection {
        min: [0, 0, 0],
        max: [3, 0, 3],
        style: RoofStyle::Gable,
        along_x: true,
        pitch: 0.7,
    };
    let roof = RoofId(0);
    base.act(
        &Op::AddRoof {
            roof,
            section,
            kind: Some(LOGS),
        },
        &cx,
    )
    .unwrap();
    let frame = Op::RoofStage {
        roof,
        cells: vec![[1, 1], [2, 1]],
        up: true,
    };
    assert_eq!(base.act(&frame, &cx), Err(Refusal::RoofNotHeld(roof)));
    // One end of each wall up to the eaves is not enough: the whole wall top bears it.
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
    fill(&mut base, 0..1);
    assert_eq!(base.act(&frame, &cx), Err(Refusal::RoofNotHeld(roof)));
    fill(&mut base, 1..4);
    let done = base.act(&frame, &cx).unwrap();
    assert_eq!(done.took, Bill::of("Thatch", Amount::new(1, 2)));
    // The wall top under an eave holds the roof now.
    let eave = Op::Empty {
        cubes: vec![[2, 5, 3]],
    };
    assert_eq!(
        base.act(&eave, &cx),
        Err(Refusal::Holds(vec![Holder::Roof(roof)]))
    );
    assert_eq!(
        base.act(&Op::RemoveRoof { roof }, &cx),
        Err(Refusal::RoofUp(roof))
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
    // A cube with nothing over it but a roof's eaves: the roof holds it.
    let mut roofed = Base::default();
    log_wall(&mut roofed, &cx);
    roofed
        .act(
            &Op::AddRoof {
                roof: RoofId(0),
                section: RoofSection {
                    min: [0, 0, 0],
                    max: [7, 0, 0],
                    style: RoofStyle::LeanTo,
                    along_x: true,
                    pitch: 0.4,
                },
                kind: Some(LOGS),
            },
            &cx,
        )
        .unwrap();
    roofed
        .act(
            &Op::RoofStage {
                roof: RoofId(0),
                cells: vec![[2, 0]],
                up: true,
            },
            &cx,
        )
        .unwrap();
    roofed
        .act(
            &Op::Carve {
                cubes: vec![[2, 5, 0]],
            },
            &cx,
        )
        .unwrap();
    assert!(matches!(
        roofed.act(&Op::Empty { cubes: vec![[2, 5, 0]] }, &cx),
        Err(Refusal::Holds(holders)) if holders.contains(&Holder::Roof(RoofId(0)))
    ));
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

// Random operations.

/// The ground's height under a column for a seed, in centimetres: a slope with bumps.
fn ground_at(seed: u64, column: [i32; 2]) -> i16 {
    let bump = Seed(seed).derive(&format!("{column:?}")).rng().below(40) as i16;
    (column[0] - 14) as i16 * -12 + bump - 20
}

/// Where random operations happen: across the edges of sections, east and down.
const LOW: IVec3 = IVec3::new(12, -2, 0);
const HIGH: IVec3 = IVec3::new(19, 5, 4);

fn somewhere(rng: &mut Rng) -> IVec3 {
    let span = HIGH - LOW + IVec3::ONE;
    LOW + IVec3::new(
        rng.below(span.x as u32) as i32,
        rng.below(span.y as u32) as i32,
        rng.below(span.z as u32) as i32,
    )
}

fn pick<T: Copy>(rng: &mut Rng, from: &[T]) -> Option<T> {
    (!from.is_empty()).then(|| from[rng.below(from.len() as u32) as usize])
}

/// Up to so many cubes in a row from one, along a random way, that pass a test.
fn run_from(rng: &mut Rng, start: IVec3, most: i32, keep: impl Fn(IVec3) -> bool) -> Vec<[i32; 3]> {
    let step = [IVec3::X, IVec3::Z, IVec3::Y][rng.below(3) as usize];
    (0..1 + rng.below(most as u32) as i32)
        .map(|i| start + step * i)
        .take_while(|&at| keep(at))
        .map(|at| at.to_array())
        .collect()
}

fn random_op(base: &Base, rng: &mut Rng, seed: u64, kind_of: u32) -> Op {
    let cubes: Vec<(IVec3, Cube)> = base.cubes().collect();
    let with = |test: &dyn Fn(&Cube) -> bool| -> Vec<IVec3> {
        cubes
            .iter()
            .filter(|(_, cube)| test(cube))
            .map(|&(at, _)| at)
            .collect()
    };
    let roofs: Vec<RoofId> = base.roofs().map(|(id, _)| id).collect();
    let props: Vec<[i32; 3]> = base.props().map(|prop| prop.min).collect();
    let fallback = Op::Unplan {
        cubes: vec![somewhere(rng).to_array()],
    };
    match kind_of {
        0..=2 => {
            let kind = [LOGS, STONE][rng.below(2) as usize];
            let parts: Vec<PartId> = base
                .parts()
                .filter(|(_, part)| part.kind == kind)
                .map(|(id, _)| id)
                .collect();
            let part = match pick(rng, &parts) {
                Some(part) if rng.below(4) == 0 => part,
                _ => base.next_part(),
            };
            let start = somewhere(rng);
            let cubes: Vec<Planned> = run_from(rng, start, 4, |at| base.cube(at).is_none())
                .into_iter()
                .map(|at| Planned {
                    at,
                    shape: if rng.below(6) == 0 {
                        Shape::Half
                    } else {
                        Shape::Whole
                    },
                })
                .collect();
            let ground = cubes
                .iter()
                .map(|cube| {
                    let column = [cube.at[0], cube.at[2]];
                    Sample {
                        column,
                        height_cm: ground_at(seed, column),
                    }
                })
                .collect();
            Op::LayPlan {
                part: Some(part),
                kind,
                cubes,
                ground,
            }
        }
        3..=6 => {
            let planned = with(&|cube| !cube.is_built());
            let Some(start) = pick(rng, &planned) else {
                return fallback;
            };
            let level = base.cube(start).map(|cube| cube.level());
            let cubes = run_from(rng, start, 4, |at| {
                base.cube(at).map(|cube| cube.level()) == level
            });
            Op::Fill { cubes }
        }
        7..=8 => {
            let ins = with(&|cube| cube.is_in());
            let Some(start) = pick(rng, &ins) else {
                return fallback;
            };
            let cubes = run_from(rng, start, 3, |at| base.is_in(at));
            Op::Empty { cubes }
        }
        9 => {
            let bare = with(&|cube| cube.level() == 0);
            let Some(start) = pick(rng, &bare) else {
                return fallback;
            };
            let cubes = run_from(rng, start, 3, |at| {
                base.cube(at).is_some_and(|cube| cube.level() == 0)
            });
            Op::Unplan { cubes }
        }
        10 => {
            let shapes = [
                Shape::Whole,
                Shape::Half,
                Shape::Stair(Facing::South),
                Shape::Slope(Facing::West),
            ];
            let bare = with(&|cube| cube.level() == 0);
            match pick(rng, &bare) {
                Some(at) => Op::SetShape {
                    cubes: vec![at.to_array()],
                    shape: shapes[rng.below(4) as usize],
                },
                None => fallback,
            }
        }
        11..=14 => {
            let any: Vec<IVec3> = cubes.iter().map(|&(at, _)| at).collect();
            let Some(start) = pick(rng, &any) else {
                return fallback;
            };
            let cubes = run_from(rng, start, 3, |at| base.cube(at).is_some());
            match kind_of {
                11 => Op::Carve { cubes },
                12 => Op::Cancel { cubes },
                13 => Op::Replace {
                    cubes,
                    kind: [LOGS, STONE][rng.below(2) as usize],
                    part: Some(base.next_part()),
                },
                _ => Op::Swap { cubes },
            }
        }
        15 => {
            let low = somewhere(rng);
            let size = IVec3::new(1 + rng.below(5) as i32, 0, 1 + rng.below(4) as i32);
            let section = RoofSection {
                min: [low.x, 0, low.z],
                max: [low.x + size.x, 0, low.z + size.z],
                style: [RoofStyle::Hip, RoofStyle::Gable, RoofStyle::LeanTo][rng.below(3) as usize],
                along_x: rng.below(2) == 0,
                pitch: 0.7,
            };
            Op::AddRoof {
                roof: base.next_roof(),
                section,
                kind: Some(LOGS),
            }
        }
        16 => match pick(rng, &roofs) {
            Some(roof) => Op::RemoveRoof { roof },
            None => fallback,
        },
        17 => {
            let Some(id) = pick(rng, &roofs) else {
                return fallback;
            };
            let cells: Vec<[i32; 2]> = base
                .roof(id)
                .map(|roof| roof.cells().collect())
                .unwrap_or_default();
            let first = rng.below(cells.len() as u32) as usize;
            let count = 1 + rng.below(3) as usize;
            Op::RoofStage {
                roof: id,
                cells: cells.into_iter().skip(first).take(count).collect(),
                up: rng.below(3) > 0,
            }
        }
        18 => {
            // A door into a hole of built cubes, where there is one.
            let mut holes = Vec::new();
            for x in LOW.x..=HIGH.x {
                for y in LOW.y..=HIGH.y {
                    for z in LOW.z..=HIGH.z {
                        for high in [y, y + 1] {
                            let prop = Prop {
                                kind: DOOR,
                                min: [x, y, z],
                                max: [x, high, z],
                                open: false,
                            };
                            let free = prop.cubes().all(|at| base.cube(at).is_none());
                            if free && base.hole_for(&prop).is_some() {
                                holes.push(prop);
                            }
                        }
                    }
                }
            }
            match pick(rng, &holes) {
                Some(prop) => Op::FitProp(prop),
                None => Op::FitProp(Prop {
                    kind: DOOR,
                    min: somewhere(rng).to_array(),
                    max: somewhere(rng).to_array(),
                    open: false,
                }),
            }
        }
        19 => match pick(rng, &props) {
            Some(at) => Op::RemoveProp { at },
            None => fallback,
        },
        20 => match pick(rng, &props) {
            Some(at) => Op::SetOpen {
                at,
                open: rng.below(2) == 0,
            },
            None => fallback,
        },
        21 => {
            let bare = with(&|cube| cube.level() == 0 && cube.fill == Fill::Solid);
            let Some(start) = pick(rng, &bare) else {
                return fallback;
            };
            let cubes = run_from(rng, start, 3, |at| {
                base.cube(at)
                    .is_some_and(|cube| cube.level() == 0 && cube.fill == Fill::Solid)
            });
            Op::Open { cubes }
        }
        _ => {
            let open = with(&|cube| cube.fill == Fill::Open);
            let Some(start) = pick(rng, &open) else {
                return fallback;
            };
            let cubes = run_from(rng, start, 3, |at| {
                base.cube(at).is_some_and(|cube| cube.fill == Fill::Open)
            });
            Op::Close { cubes }
        }
    }
}

/// What is in a base: each cube's steps, each roof cell's stages, each prop.
fn value(base: &Base, rules: &Rules) -> Bill {
    let mut bill = Bill::default();
    for (_, cube) in base.cubes().filter(|(_, cube)| cube.fill != Fill::Open) {
        let terms = base.terms(&cube);
        let steps = u64::from(cube.level() / terms.step());
        bill.add(&terms.per_step.times(steps));
    }
    for (_, roof) in base.roofs() {
        let stages: u64 = roof.stages.iter().map(|&stage| u64::from(stage)).sum();
        bill.add(&roof.per_stage.times(stages));
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
                !under.is_in() || under.is_built() || under.pending == Pending::Carve
            }),
            "{at}: {cube} is in over a cube partly in"
        );
    }
    for (id, roof) in base.roofs().filter(|(_, roof)| roof.standing()) {
        assert!(base.roof_held(roof), "{at}: roof {id:?} stands on nothing");
    }
    for prop in base.props() {
        assert!(
            base.hole_for(prop).is_some(),
            "{at}: the prop at {:?} is in no hole",
            prop.min
        );
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

/// How many seeds of random operations a test run tries, unless `TERRA_SEEDS` says.
const SEEDS: u64 = 4;

#[test]
fn random_operations_keep_every_rule() {
    let rules = rules();
    let kit = Kit::default();
    let stone_kit = Kit {
        id: STONE.0,
        pattern: Pattern::Masonry,
        ..Kit::default()
    };
    let claim = Claim {
        min: [LOW.x, LOW.z],
        max: [HIGH.x, HIGH.z],
        budget: 120,
    };
    let mut accepted: BTreeMap<String, usize> = BTreeMap::new();
    // A few seeds by default, enough that every operation is accepted; more for a sweep,
    // as `TERRA_SEEDS=64 cargo test -p terra_build random_operations`.
    let seeds: u64 = std::env::var("TERRA_SEEDS")
        .ok()
        .and_then(|seeds| seeds.parse().ok())
        .unwrap_or(SEEDS);
    for seed in 0..seeds {
        let mut rng = Seed(seed).derive("building").rng();
        let mut yard = Yard {
            stock: plenty(),
            ..Yard::default()
        };
        let mut base = Base::default();
        let mut log: Vec<Op> = Vec::new();
        let (mut again, mut replayed) = (Base::default(), 0);
        // A stone wall a storey high with a doorway in it and a roof planned along its top
        // to begin with, so doors are fitted and roofs go up and are held.
        let wall: Vec<[i32; 3]> = (13..18)
            .flat_map(|x| (-1..6).map(move |y| [x, y, 2]))
            .filter(|&[x, y, _]| !(x == 15 && (0..2).contains(&y)))
            .collect();
        let mut start = vec![Op::LayPlan {
            part: Some(PartId(0)),
            kind: STONE,
            cubes: wall
                .iter()
                .map(|&at| Planned {
                    at,
                    shape: Shape::Whole,
                })
                .collect(),
            ground: (13..18)
                .map(|x| Sample {
                    column: [x, 2],
                    height_cm: ground_at(seed, [x, 2]),
                })
                .collect(),
        }];
        for y in -1..6 {
            let cubes = wall.iter().copied().filter(|at| at[1] == y).collect();
            start.push(Op::Fill { cubes });
        }
        start.push(Op::AddRoof {
            roof: RoofId(0),
            section: RoofSection {
                min: [13, 0, 2],
                max: [17, 0, 2],
                style: RoofStyle::Gable,
                along_x: true,
                pitch: 0.7,
            },
            kind: Some(LOGS),
        });
        let free = Context::free(&rules);
        let begun = base.act_all(&start, &free).unwrap();
        yard.stock.take(&begun.took).unwrap();
        let (mut paid, mut refunded) = (begun.took, Bill::default());
        log.extend(start);
        let mut undos: Vec<Vec<Op>> = Vec::new();
        for step in 0..1200 {
            let at = format!("seed {seed} step {step}");
            if step % 50 == 0 {
                // Someone walks about; things are set down.
                yard.bodies = (0..3).map(|_| somewhere(&mut rng).to_array()).collect();
                yard.items = (0..3).map(|_| somewhere(&mut rng).to_array()).collect();
            }
            let roll = rng.below(26);
            let undo = roll >= 23;
            let ops = if undo {
                match undos.pop() {
                    Some(ops) => ops,
                    None => continue,
                }
            } else {
                vec![random_op(&base, &mut rng, seed, roll)]
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
                    // Nothing was taken from under anything, but a cube carved out from
                    // under what stays joined, or from under an item.
                    for op in &ops {
                        if let Op::Empty { cubes } = op {
                            for &cube in cubes {
                                let cube = IVec3::from_array(cube);
                                let carved = base
                                    .cube(cube)
                                    .is_some_and(|cube| cube.pending == Pending::Carve);
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
            check_rules(&base, &at);
            let mut held = value(&base, &rules);
            held.add(&refunded);
            assert_eq!(
                held, paid,
                "{at}: what is in and what came out is not what went in"
            );
            if step % 300 == 299 {
                // Laid twice, the same.
                for kit in [&kit, &stone_kit] {
                    assert_eq!(lay(&base, kit).pieces, lay(&base, kit).pieces, "{at}");
                }
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
        "AddRoof",
        "RemoveRoof",
        "RoofStage",
        "FitProp",
        "RemoveProp",
        "SetOpen",
        "Open",
        "Close",
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
fn an_opening_is_planned_into_a_wall_and_a_door_fits_it_once_built() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    wall_with_a_doorway(&mut base, &cx);
    // An opening is not filled, and is walked through.
    assert_eq!(
        base.act(
            &Op::Fill {
                cubes: vec![[2, 0, 0]]
            },
            &cx
        ),
        Err(Refusal::Opening(IVec3::new(2, 0, 0)))
    );
    let door = Prop {
        kind: DOOR,
        min: [2, 0, 0],
        max: [2, 3, 0],
        open: false,
    };
    base.act(&Op::FitProp(door), &cx).unwrap();
    // Shut, it stops a walker; open, it lets them through.
    let walker = Walker {
        radius: 0.2,
        ..Walker::default()
    };
    let (from, to) = (Vec3::new(1.25, 0.0, -0.3), Vec3::new(1.25, 0.0, 0.3));
    assert!(base.walk(from, to, &walker).z < 0.0);
    base.act(
        &Op::SetOpen {
            at: [2, 0, 0],
            open: true,
        },
        &cx,
    )
    .unwrap();
    assert!((base.walk(from, to, &walker) - to).length() < 1e-3);
    // A fitted opening cannot be closed, and a closed one is planned again.
    assert_eq!(
        base.act(
            &Op::Close {
                cubes: vec![[2, 0, 0]]
            },
            &cx
        ),
        Err(Refusal::PropThere(IVec3::new(2, 0, 0)))
    );
    base.act(&Op::RemoveProp { at: [2, 0, 0] }, &cx).unwrap();
    let closed = base
        .act(
            &Op::Close {
                cubes: line([2, 0, 0], IVec3::Y, 4),
            },
            &cx,
        )
        .unwrap();
    assert_eq!(base.cube(IVec3::new(2, 0, 0)).unwrap().level(), 0);
    base.act_all(&closed.undo, &cx).unwrap();
    assert_eq!(base.cube(IVec3::new(2, 0, 0)).unwrap().fill, Fill::Open);
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
fn only_walls_built_whole_close_a_room_in() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A ring of wall tiles four by four, a storey high, planned.
    let ring: Vec<[i32; 3]> = (0..4)
        .flat_map(|x| (0..4).map(move |z| (x, z)))
        .filter(|&(x, z)| x == 0 || x == 3 || z == 0 || z == 3)
        .flat_map(|(x, z)| (0..6).map(move |y| [x, y, z]))
        .collect();
    base.act(
        &Op::LayPlan {
            part: Some(PartId(0)),
            kind: STONE,
            cubes: ring
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
    assert_eq!(base.tile_count(), 0, "a plan encloses nothing");
    for y in 0..6 {
        let course: Vec<[i32; 3]> = ring.iter().copied().filter(|at| at[1] == y).collect();
        base.act(&Op::Fill { cubes: course }, &cx).unwrap();
        let expected = if y < 4 { 0 } else { 4 };
        assert_eq!(base.tile_count(), expected, "after layer {y}");
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
