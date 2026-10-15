//! Loss and repair, case by case: what a hit destroys and brings down, what is lost with
//! it, what a repair costs, and each undone exactly. The random
//! operations in `ops/tests.rs` hold the same over any order of acts.

use super::*;
use crate::cost::{Rules, Terms};
use crate::ops::{Planned, World};
use crate::{Fill, Kind, Prop, Shape};

const STONE: Kind = Kind(2);
const LOGS: Kind = Kind(1);
const DOOR: crate::PropKind = crate::PropKind(1);

/// Stone at a whole stone a cube, as tough as 100 with half its cost to repair from
/// nothing; logs at an eighth of a log a course, two courses a cube, as tough as 40.
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

#[test]
fn knocking_out_a_walls_base_course_brings_down_what_it_alone_held() {
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
    // Every cube of it came down, said as it was for the game to burst, and what went
    // into it is lost: nothing is given back while building takes no real resources.
    assert_eq!(done.fell.len() + fell.fell.len(), 24);
    assert!(
        fell.fell
            .iter()
            .all(|(_, cube)| cube.kind == STONE && cube.is_built())
    );
    assert!(fell.gave.is_empty());
    let mut lost = done.lost.clone();
    lost.add(&fell.lost);
    assert_eq!(lost, Bill::of("Stone", Amount::whole(24)));
    // Undone, each in turn, back as it was.
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
    assert!(done.lost.is_empty() && done.fell.is_empty());
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
fn plans_hanging_from_what_falls_go_with_it_and_no_others() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A stone post two high, a roof's plan laid out from its top as a ghost, and the plan
    // of a wall on the ground beside it, apart.
    wall(&mut base, 1, 2, &cx);
    let lay = |base: &mut Base, part: u32, cubes: &[[i32; 3]]| {
        let ground = cubes
            .iter()
            .map(|at| Sample {
                column: [at[0], at[2]],
                height_cm: 10,
            })
            .collect();
        base.act(
            &Op::LayPlan {
                part: Some(PartId(part)),
                kind: STONE,
                cubes: cubes
                    .iter()
                    .map(|&at| Planned {
                        at,
                        shape: Shape::Whole,
                    })
                    .collect(),
                ground,
            },
            &cx,
        )
        .unwrap();
    };
    let ghost = [[1, 1, 0], [2, 1, 0], [2, 2, 0]];
    lay(&mut base, 40, &ghost);
    lay(&mut base, 41, &[[5, 0, 0], [6, 0, 0]]);
    let before = base.clone();
    // The post's foot destroyed: the post comes down, and the plan that hung from it.
    let done = base
        .act(
            &Op::Damage {
                cubes: vec![[0, 0, 0]],
                amount: 100,
            },
            &cx,
        )
        .unwrap();
    assert_eq!(done.fell.len(), 2, "only what was built bursts");
    for at in ghost {
        assert_eq!(
            base.cube(IVec3::from_array(at)),
            None,
            "{at:?} left floating"
        );
    }
    assert!(
        base.cube(IVec3::new(5, 0, 0)).is_some(),
        "the plan on the ground stays"
    );
    assert_eq!(base.cubes().count(), 2);
    // Undone, the plan comes back with the post.
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
    // Its lintel hit, the door comes down, what it cost lost.
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
    assert_eq!(
        done.lost.0.get("Door").copied(),
        Some(Amount::whole(1)),
        "{}",
        done.lost
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
    assert_eq!(fell.fell.len(), 6);
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
fn a_log_course_part_in_loses_what_went_into_it() {
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
    // An eighth of a log went into it, lost; the course either side stands on the ground.
    assert_eq!(done.lost, Bill::of("Log", Amount::new(1, 8)));
    assert_eq!(done.fell.len(), 1);
    assert_eq!(done.fell[0].1.level(), crate::cubes::FULL / 2);
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

#[test]
fn damage_spreads_a_little_into_the_cubes_beside_a_damaged_one_and_no_further() {
    // One cube at the origin all but gone, its face looking south (+Z) at z = 0.5 m.
    let worn = BTreeMap::from([([0, 0, 0], 4u8)]);
    let normal = glam::Vec3::Z;
    let at = |x: f32, y: f32| lost(&worn, glam::Vec3::new(x, y, 0.5), normal);
    assert_eq!(at(0.25, 0.25), 1.0);
    // In the next cube east, near their shared edge, some of it; past the spread, none.
    let near = at(0.52, 0.25);
    assert!(near > 0.3 && near < 1.0, "{near}");
    assert_eq!(at(0.9, 0.25), 0.0);
    // Above it too, and not through the face to the cube behind.
    assert!(at(0.25, 0.53) > 0.3);
    assert_eq!(lost(&worn, glam::Vec3::new(0.25, 0.25, 1.0), normal), 0.0);
}

#[test]
fn a_wall_harmed_evenly_gives_way_in_patches_not_evenly_over_every_cube() {
    // A wall twenty cubes long and high, every cube half its health lost.
    let worn: BTreeMap<[i32; 3], u8> = (0..20)
        .flat_map(|x| (0..20).map(move |y| ([x, y, 0], 2u8)))
        .collect();
    let normal = glam::Vec3::NEG_Z;
    // Read at each cube's middle on its face, so the hit's focus is the same at each.
    let at = |x: i32, y: i32| {
        struck(
            &worn,
            glam::Vec3::new(x as f32 + 0.5, y as f32 + 0.5, 0.0) * crate::CELL_M,
            normal,
        )
    };
    let all: Vec<f32> = (0..20)
        .flat_map(|x| (0..20).map(move |y| (x, y)))
        .map(|(x, y)| at(x, y))
        .collect();
    let mean = all.iter().sum::<f32>() / all.len() as f32;
    let (least, most) = all
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    // As lost as its cubes on the whole, but some places far weaker than others.
    assert!((mean - 0.5 * 1.15).abs() < 0.12, "mean {mean}");
    assert!(most > least * 1.8, "from {least} to {most}");
    // In patches: a cube is harmed much as the one beside it, far less like one across the wall.
    let (mut near, mut far) = (0.0, 0.0);
    for x in 0..19 {
        for y in 0..20 {
            near += (at(x, y) - at(x + 1, y)).abs();
            far += (at(x, y) - at((x + 10) % 20, (y + 7) % 20)).abs();
        }
    }
    assert!(near < far * 0.6, "beside {near}, across {far}");
}

#[test]
fn what_a_point_has_lost_runs_on_smoothly_across_cubes_and_is_most_at_a_hits_middle() {
    // One cube all but gone in a wall, and how much is lost along a line through it.
    let worn: BTreeMap<[i32; 3], u8> = [([4, 4, 0], 4u8)].into_iter().collect();
    let normal = glam::Vec3::NEG_Z;
    let at = |x: f32| struck(&worn, glam::Vec3::new(x, 2.25, 0.0), normal);
    let line: Vec<f32> = (0..=200).map(|k| at(1.25 + k as f32 * 0.01)).collect();
    // No step anywhere, at the cubes' edges (x = 1.5, 2.0, 2.5) or between.
    for pair in line.windows(2) {
        assert!((pair[1] - pair[0]).abs() < 0.03, "{pair:?}");
    }
    // Most at its middle, reaching a little past its edges, nothing two cubes off.
    let middle = at(2.25);
    assert!(middle > 0.4 && middle > at(2.0) && at(2.0) > at(1.8) && at(1.8) > 0.0);
    assert_eq!(at(1.4), 0.0);
}
