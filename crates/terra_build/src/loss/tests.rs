//! Loss and repair, case by case: what a hit destroys and brings down, where it falls and
//! what its debris gives back, what a repair costs, and each undone exactly. The random
//! operations in `ops/tests.rs` hold the same over any order of acts.

use super::*;
use crate::cost::{Rules, Terms};
use crate::ops::{Planned, World};
use crate::{Fill, Prop, Shape};

const STONE: Kind = Kind(2);
const LOGS: Kind = Kind(1);
const DOOR: crate::PropKind = crate::PropKind(1);

/// Stone at a whole stone a cube, as tough as 100 with half back from its debris and half
/// its cost to repair from nothing; logs at an eighth of a log a course, two courses a
/// cube, as tough as 40.
fn rules() -> Rules {
    let mut rules = Rules::default();
    rules
        .kinds
        .insert(STONE, Terms::new(1, Bill::of("Stone", Amount::whole(1))));
    rules
        .kinds
        .insert(LOGS, Terms::new(2, Bill::of("Log", Amount::new(1, 8))));
    rules.toughness.insert(
        LOGS,
        Toughness {
            health: 40,
            salvage: Amount::new(1, 4),
            repair: Amount::new(1, 2),
        },
    );
    rules.props.insert(DOOR, Bill::of("Door", Amount::whole(1)));
    rules
}

/// A stone wall `long` cubes east and `high` up from the ground at 0, built as a player
/// builds it, a course at a time.
fn wall(base: &mut Base, long: i32, high: i32, cx: &Context) {
    for y in 0..high {
        let cubes: Vec<Planned> = (0..long)
            .map(|x| Planned {
                at: [x, y, 0],
                shape: Shape::Whole,
            })
            .collect();
        let ground = (0..long)
            .map(|x| Sample {
                column: [x, 0],
                height_cm: 10,
            })
            .collect();
        base.act(
            &Op::LayPlan {
                part: Some(PartId(y as u32)),
                kind: STONE,
                cubes,
                ground,
            },
            cx,
        )
        .unwrap();
        let course = (0..long).map(|x| [x, y, 0]).collect();
        base.act(&Op::Fill { cubes: course }, cx).unwrap();
    }
}

/// A builder with nothing to hand.
struct Short;

impl World for Short {
    fn has(&self, bill: &Bill) -> bool {
        bill.is_empty()
    }
}

/// Everything lying about, gathered.
fn debris_value(base: &Base) -> Bill {
    let mut bill = Bill::default();
    for (_, pile) in base.debris() {
        bill.add(&pile.gives);
    }
    bill
}

#[test]
fn knocking_out_a_walls_base_course_brings_down_what_it_alone_held_into_debris() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A wall six long and four high, and a second wall beside it, apart.
    wall(&mut base, 6, 4, &cx);
    let before = base.clone();
    // The base course under the middle four: the wall's two end columns still stand on
    // the ground, and the courses above stay joined through them.
    let hit = Op::Damage {
        cubes: (1..5).map(|x| [x, 0, 0]).collect(),
        amount: 100,
    };
    let done = base.act(&hit, &cx).unwrap();
    for x in 1..5 {
        assert_eq!(base.cube(IVec3::new(x, 0, 0)), None, "{x} destroyed");
    }
    assert!(
        base.is_in(IVec3::new(2, 1, 0)),
        "the course over it is held at its ends"
    );
    assert!(base.floating().is_empty());
    // A post's worth more: the two end columns' feet too. Now nothing of it touches the
    // ground, and all of it comes down.
    let feet = Op::Damage {
        cubes: vec![[0, 0, 0], [5, 0, 0]],
        amount: 250,
    };
    let fell = base.act(&feet, &cx).unwrap();
    assert_eq!(base.cubes().count(), 0, "all of it fell");
    assert!(base.floating().is_empty());
    // Each column's cubes in one pile at its foot, on the ground, half of what went in.
    assert_eq!(base.debris().count(), 6);
    for (at, pile) in base.debris() {
        assert_eq!(at.y, 0, "on the ground at {at}");
        assert_eq!(pile.floor_cm, 10);
    }
    let piles: u32 = base.debris().map(|(_, pile)| pile.cubes()).sum();
    assert_eq!(piles, 24, "every cube fell into a pile");
    assert_eq!(debris_value(&base), Bill::of("Stone", Amount::whole(12)));
    let mut lost = done.lost.clone();
    lost.add(&fell.lost);
    assert_eq!(lost, Bill::of("Stone", Amount::whole(12)));
    // Gathered, exactly what was there.
    let mut gathered = Bill::default();
    let piles: Vec<IVec3> = base.debris().map(|(at, _)| at).collect();
    for at in piles {
        let got = base.act(&Op::Gather { at: at.to_array() }, &cx).unwrap();
        gathered.add(&got.gave);
    }
    assert_eq!(gathered, Bill::of("Stone", Amount::whole(12)));
    assert_eq!(base.debris().count(), 0);
    // And undone, each in turn, back as it was.
    let mut undone = base.clone();
    undone.act_all(&fell.undo, &cx).unwrap_err();
    // The debris was gathered: the fall cannot be undone over its taking, but the
    // gathering can be, then the fall, then the first hit.
    let mut again = before.clone();
    let first = again.act(&hit, &cx).unwrap();
    let second = again.act(&feet, &cx).unwrap();
    let after = again.clone();
    let done = again.act_all(&second.undo, &Context::free(&rules)).unwrap();
    assert_eq!(done.recovered, fell.lost);
    again.act_all(&first.undo, &cx).unwrap();
    assert_eq!(again, before, "undone, as it was");
    again.act_all(&[hit.clone(), feet.clone()], &cx).unwrap();
    assert_eq!(again, after, "done again, the same");
}

#[test]
fn damage_short_of_its_health_wears_a_cube_and_a_repair_costs_a_share_by_quarters() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    wall(&mut base, 2, 2, &cx);
    let at = IVec3::new(0, 1, 0);
    let before = base.clone();
    let hit = |amount| Op::Damage {
        cubes: vec![at.to_array()],
        amount,
    };
    let done = base.act(&hit(30), &cx).unwrap();
    assert_eq!(base.damage(at), 30);
    assert!(done.lost.is_empty() && base.debris().count() == 0);
    // Three tenths lost: two quarters, rounded up; half its stone's worth from nothing,
    // so a quarter of a stone.
    let tough = rules.toughness(STONE);
    assert_eq!(tough.quarters(30), 2);
    assert_eq!(
        base.repair_cost(at, &tough),
        Some(Bill::of("Stone", Amount::new(1, 4)))
    );
    // Not taken apart while damaged: mended first.
    assert_eq!(
        base.check(
            &Op::Empty {
                cubes: vec![at.to_array()]
            },
            &cx
        ),
        Err(Refusal::Damaged(at))
    );
    // Hit again, more worn; mended, whole again, for what it costs then.
    base.act(&hit(50), &cx).unwrap();
    assert_eq!(base.damage(at), 80);
    let repair = Op::Repair {
        cubes: vec![at.to_array()],
    };
    let mended = base.act(&repair, &cx).unwrap();
    assert_eq!(mended.took, Bill::of("Stone", Amount::new(1, 2)));
    assert_eq!(mended.lost, mended.took);
    assert_eq!(base.damage(at), 0);
    assert_eq!(base.check(&repair, &cx), Err(Refusal::NotDamaged(at)));
    // Its undo gives the repair back and the damage with it.
    let mut undone = base.clone();
    let back = undone.act_all(&mended.undo, &cx).unwrap();
    assert_eq!(back.gave, mended.took);
    assert_eq!(undone.damage(at), 80);
    // A repair takes what it costs, or is refused.
    let poor = Context {
        rules: &rules,
        claim: None,
        world: &Short,
    };
    assert!(matches!(
        undone.check(&repair, &poor),
        Err(Refusal::Materials(_))
    ));
    // Undone to the first hit, as it was.
    let mut first = before.clone();
    let done = first.act(&hit(30), &cx).unwrap();
    first.act_all(&done.undo, &cx).unwrap();
    assert_eq!(first, before);
}

#[test]
fn debris_lies_on_what_stops_it_and_is_in_the_way_until_gathered() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A post three high, and a beam of two out from its top, hanging from it alone.
    wall(&mut base, 1, 3, &cx);
    let beam: Vec<[i32; 3]> = vec![[1, 2, 0], [2, 2, 0]];
    base.act(
        &Op::LayPlan {
            part: Some(PartId(9)),
            kind: STONE,
            cubes: beam
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
    base.act(
        &Op::Fill {
            cubes: beam.clone(),
        },
        &cx,
    )
    .unwrap();
    // A plan where the beam's end will fall, on the ground under it.
    base.act(
        &Op::LayPlan {
            part: Some(PartId(10)),
            kind: STONE,
            cubes: vec![Planned {
                at: [2, 0, 0],
                shape: Shape::Whole,
            }],
            ground: vec![Sample {
                column: [2, 0],
                height_cm: 10,
            }],
        },
        &cx,
    )
    .unwrap();
    // Its first cube out from the post hit: both come down, each to the ground under it.
    base.act(
        &Op::Damage {
            cubes: vec![[1, 2, 0]],
            amount: 100,
        },
        &cx,
    )
    .unwrap();
    assert!(!base.is_in(IVec3::new(2, 2, 0)));
    assert!(base.pile(IVec3::new(1, 0, 0)).is_some());
    assert!(base.pile(IVec3::new(2, 0, 0)).is_some());
    // The plan where it lies cannot be built until it is gathered.
    let fill = Op::Fill {
        cubes: vec![[2, 0, 0]],
    };
    assert_eq!(
        base.check(&fill, &cx),
        Err(Refusal::InTheWay(IVec3::new(2, 0, 0)))
    );
    base.act(&Op::Gather { at: [2, 0, 0] }, &cx).unwrap();
    base.act(&fill, &cx).unwrap();
    // The post's top taken off: the debris on the ground beside it stays where it is.
    assert!(base.pile(IVec3::new(1, 0, 0)).is_some());
    assert_eq!(
        base.check(&Op::Gather { at: [5, 0, 0] }, &cx),
        Err(Refusal::NoDebris(IVec3::new(5, 0, 0)))
    );
}

#[test]
fn debris_on_what_falls_falls_with_it_and_lies_on_the_rest() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // Two posts four high, a beam across their tops, and a block over it on the east post.
    wall(&mut base, 1, 4, &cx);
    for y in 0..4 {
        base.act(
            &Op::LayPlan {
                part: Some(PartId(20 + y as u32)),
                kind: STONE,
                cubes: vec![Planned {
                    at: [3, y, 0],
                    shape: Shape::Whole,
                }],
                ground: vec![Sample {
                    column: [3, 0],
                    height_cm: 10,
                }],
            },
            &cx,
        )
        .unwrap();
        base.act(
            &Op::Fill {
                cubes: vec![[3, y, 0]],
            },
            &cx,
        )
        .unwrap();
    }
    let beam: Vec<[i32; 3]> = vec![[1, 3, 0], [2, 3, 0]];
    base.act(
        &Op::LayPlan {
            part: Some(PartId(30)),
            kind: STONE,
            cubes: beam
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
    base.act(&Op::Fill { cubes: beam }, &cx).unwrap();
    // A cube on the beam's east half, destroyed.
    let on_beam = IVec3::new(2, 4, 0);
    base.act(
        &Op::LayPlan {
            part: Some(PartId(31)),
            kind: STONE,
            cubes: vec![Planned {
                at: on_beam.to_array(),
                shape: Shape::Whole,
            }],
            ground: Vec::new(),
        },
        &cx,
    )
    .unwrap();
    base.act(
        &Op::Fill {
            cubes: vec![on_beam.to_array()],
        },
        &cx,
    )
    .unwrap();
    base.act(
        &Op::Damage {
            cubes: vec![on_beam.to_array()],
            amount: 100,
        },
        &cx,
    )
    .unwrap();
    // Destroyed where it stood, its debris lies on the beam that held it.
    assert!(base.pile(on_beam).is_some(), "on the beam");
    // The beam taken from under it: the beam and its debris both fall to the ground.
    let before = base.clone();
    let done = base
        .act(
            &Op::Damage {
                cubes: vec![[1, 3, 0], [2, 3, 0]],
                amount: 100,
            },
            &cx,
        )
        .unwrap();
    assert!(base.pile(on_beam).is_none());
    let under = base.pile(IVec3::new(2, 0, 0)).expect("on the ground");
    assert_eq!(under.cubes(), 2, "the beam's end and what lay on it");
    assert_eq!(under.from, 4);
    base.act_all(&done.undo, &cx).unwrap();
    assert_eq!(base, before);
}

#[test]
fn a_door_falls_with_its_hole() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A wall three long and four high, its middle open three high, a door in it.
    wall(&mut base, 3, 4, &cx);
    let hole: Vec<[i32; 3]> = (0..3).map(|y| [1, y, 0]).collect();
    // Out of the wall as it was: the middle three of its column taken apart, top first.
    let mut cut = base.clone();
    for &at in hole.iter().rev() {
        cut.act(&Op::Carve { cubes: vec![at] }, &cx).unwrap();
        cut.act(&Op::Empty { cubes: vec![at] }, &cx).unwrap();
        cut.act(&Op::Open { cubes: vec![at] }, &cx).unwrap();
    }
    let mut base = cut;
    let door = Prop {
        kind: DOOR,
        min: [1, 0, 0],
        max: [1, 2, 0],
        open: false,
        ..Prop::default()
    };
    base.act(&Op::FitProp(door), &cx).unwrap();
    assert!(
        base.cube(IVec3::new(1, 1, 0))
            .is_some_and(|c| c.fill == Fill::Open)
    );
    // Its lintel hit, the door falls, half of what it cost in the debris.
    let before = base.clone();
    let done = base
        .act(
            &Op::Damage {
                cubes: vec![[1, 3, 0]],
                amount: 100,
            },
            &cx,
        )
        .unwrap();
    assert_eq!(base.props().count(), 0);
    let gives = debris_value(&base);
    assert_eq!(
        gives.0.get("Door").copied(),
        Some(Amount::new(1, 2)),
        "{gives}"
    );
    base.act_all(&done.undo, &cx).unwrap();
    assert_eq!(base, before);
}

#[test]
fn the_ground_giving_way_brings_down_what_it_alone_held() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    wall(&mut base, 3, 2, &cx);
    let before = base.clone();
    // The ground under the west column drops a metre: it hangs from the rest.
    let west = Op::Subside {
        samples: vec![Sample {
            column: [0, 0],
            height_cm: -90,
        }],
    };
    let done = base.act(&west, &cx).unwrap();
    assert_eq!(base.cubes().count(), 6, "held by the wall beside it");
    // Under all three: all of it falls, onto the ground where it is now.
    let all = Op::Subside {
        samples: (0..3)
            .map(|x| Sample {
                column: [x, 0],
                height_cm: -90,
            })
            .collect(),
    };
    let fell = base.act(&all, &cx).unwrap();
    assert_eq!(base.cubes().count(), 0);
    assert!(base.floating().is_empty());
    for (at, pile) in base.debris() {
        assert_eq!((at.y, pile.floor_cm), (-2, -90), "{at}");
    }
    // Undone, each, as it was.
    base.act_all(&fell.undo, &Context::free(&rules)).unwrap();
    base.act_all(&done.undo, &cx).unwrap();
    assert_eq!(base, before);
    // Where the ground rises, nothing falls.
    let rise = Op::Subside {
        samples: vec![Sample {
            column: [1, 0],
            height_cm: 40,
        }],
    };
    let risen = base.act(&rise, &cx).unwrap();
    assert!(risen.lost.is_empty() && base.cubes().count() == 6);
}

#[test]
fn logs_hit_fall_as_their_kinds_share_says_and_a_log_course_part_in_gives_its_part() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A course of logs, one course in of two, along the ground.
    let cubes: Vec<Planned> = (0..4)
        .map(|x| Planned {
            at: [x, 0, 0],
            shape: Shape::Whole,
        })
        .collect();
    base.act(
        &Op::LayPlan {
            part: Some(PartId(0)),
            kind: LOGS,
            cubes,
            ground: (0..4)
                .map(|x| Sample {
                    column: [x, 0],
                    height_cm: 0,
                })
                .collect(),
        },
        &cx,
    )
    .unwrap();
    base.act(
        &Op::Fill {
            cubes: (0..4).map(|x| [x, 0, 0]).collect(),
        },
        &cx,
    )
    .unwrap();
    let done = base
        .act(
            &Op::Damage {
                cubes: vec![[0, 0, 0]],
                amount: 40,
            },
            &cx,
        )
        .unwrap();
    // An eighth of a log went into it; a quarter of that back, the rest lost.
    assert_eq!(debris_value(&base), Bill::of("Log", Amount::new(1, 32)));
    assert_eq!(done.lost, Bill::of("Log", Amount::new(3, 32)));
}

#[test]
fn a_damaged_cubes_surface_is_drawn_darker_the_more_it_has_lost_and_no_other() {
    // A face of the cube at the origin and one of the cube two along, each a fine grid of
    // triangles, as a wall's surface is.
    let face = |x0: f32| {
        let mut surface = crate::Surface::default();
        for i in 0..10 {
            for j in 0..10 {
                let (u, v) = (x0 + i as f32 * 0.05, j as f32 * 0.05);
                let corners = [
                    glam::Vec3::new(u, v, 0.5),
                    glam::Vec3::new(u + 0.05, v, 0.5),
                    glam::Vec3::new(u + 0.05, v + 0.05, 0.5),
                    glam::Vec3::new(u, v + 0.05, 0.5),
                ];
                surface.quad(corners, glam::Vec3::Z, [0.8; 3]);
            }
        }
        surface
    };
    let mean = |surface: &crate::Surface| {
        surface.colours.iter().map(|c| c[0]).sum::<f32>() / surface.colours.len() as f32
    };
    let worn = |quarters: u8, x0: f32| {
        let mut surface = face(x0);
        wear(&mut surface, &[([0, 0, 0], quarters)]);
        mean(&surface)
    };
    // Untouched where it is not damaged; darker by quarters lost where it is.
    assert!((worn(4, 1.0) - 0.8).abs() < 1e-5);
    assert!(worn(1, 0.0) < 0.8);
    assert!(worn(2, 0.0) < worn(1, 0.0));
    assert!(worn(4, 0.0) < worn(2, 0.0));
    assert!(worn(4, 0.0) > 0.2, "still its material, only worn");
}
