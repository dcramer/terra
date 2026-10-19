//! The operations' cases that walk what they build (D407): moved whole from the rules'
//! own tests (`terra_build`'s `ops/tests.rs`) when walking became the layout's, with the
//! few fixtures they lay.

use glam::{IVec3, Vec3};
use std::collections::BTreeMap;
use terra_build::ops::{Context, Op, Planned, Refusal};
use terra_build::{
    Amount, Base, Bill, CELL_M, Fill, Kind, PartId, Prop, PropKind, Rules, Sample, Shape, Terms,
    Walker,
};
use terra_build_layout::walk::Walking;

const LOGS: Kind = Kind(1);
const STONE: Kind = Kind(2);
const THATCH: Kind = Kind::FIRST_ROOF;
const DOOR: PropKind = PropKind(1);

fn rules() -> Rules {
    let mut stone = Bill::of("Stone", Amount::whole(1));
    stone.add(&Bill::of("Mortar", Amount::new(3, 8)));
    Rules {
        kinds: BTreeMap::from([
            // A log fills a cube in two courses, and spans eight cubes.
            (LOGS, Terms::new(2, Bill::of("Log", Amount::new(1, 8)))),
            (STONE, Terms::new(1, stone)),
            // A roof's cube in four stages: frame, battens, covering, trims.
            (THATCH, Terms::new(4, Bill::of("Thatch", Amount::new(1, 4)))),
        ]),
        props: BTreeMap::from([(DOOR, Bill::of("Door", Amount::whole(1)))]),
        // Logs weaker than stone; thatch as any kind.
        toughness: BTreeMap::from([
            (
                LOGS,
                terra_build::Toughness {
                    health: 60,
                    repair: Amount::new(1, 2),
                },
            ),
            (
                STONE,
                terra_build::Toughness {
                    health: 150,
                    repair: Amount::new(1, 4),
                },
            ),
        ]),
    }
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
    assert!((base.walk(from, to, &walker, None) - to).length() < 1e-3);
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
fn a_post_holds_up_what_rests_on_it() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A post three cubes high, and a beam of stone across its top, out into the air.
    let post: Vec<Planned> = (0..3)
        .map(|y| Planned {
            at: [0, y, 0],
            shape: Shape::Post,
        })
        .collect();
    base.act(
        &Op::LayPlan {
            part: Some(PartId(0)),
            kind: STONE,
            cubes: post,
            ground: vec![Sample {
                column: [0, 0],
                height_cm: 0,
            }],
        },
        &cx,
    )
    .unwrap();
    base.act(&row(1, STONE, [0, 3, 0], IVec3::X, 3), &cx)
        .unwrap();
    // The beam waits for the post under it.
    assert!(
        base.act(
            &Op::Fill {
                cubes: vec![[0, 3, 0]]
            },
            &cx
        )
        .is_err()
    );
    for y in 0..3 {
        base.act(
            &Op::Fill {
                cubes: vec![[0, y, 0]],
            },
            &cx,
        )
        .unwrap();
    }
    base.act(
        &Op::Fill {
            cubes: line([0, 3, 0], IVec3::X, 3),
        },
        &cx,
    )
    .unwrap();
    assert!(base.floating().is_empty());
    // Nothing of the post comes out while the beam rests on it.
    let refused = base.act(
        &Op::Empty {
            cubes: vec![[0, 2, 0]],
        },
        &cx,
    );
    assert!(refused.is_err(), "{refused:?}");
    // A walker meets it as a whole cube.
    let walker = Walker::default();
    assert_eq!(
        base.support(Vec3::new(0.25, 0.3, 0.25), &walker),
        Some(CELL_M)
    );
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
        opens: [0, 0, 1],
        ..Prop::default()
    };
    base.act(&Op::FitProp(door), &cx).unwrap();
    // Shut, it stops a walker in the doorway; open, it lets them in as far as where it
    // stands open.
    let walker = Walker {
        radius: 0.2,
        ..Walker::default()
    };
    let (from, to) = (Vec3::new(1.25, 0.0, -0.3), Vec3::new(1.25, 0.0, 0.05));
    assert!(base.walk(from, to, &walker, None).z < 0.04);
    base.act(
        &Op::SetOpen {
            at: [2, 0, 0],
            open: true,
        },
        &cx,
    )
    .unwrap();
    assert!((base.walk(from, to, &walker, None) - to).length() < 1e-3);
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
