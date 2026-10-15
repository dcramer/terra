//! The construction rules audited case by case (research/building-audit.md, `cargo xtask
//! rules`): the cases construction-001 and D193 name that the operations' own tests do not
//! already walk through. Each test is one rule; a rule known to fail today is `#[ignore]`d
//! with its reason, so `cargo xtask check` stays the gate for what holds and
//! `cargo xtask rules` runs the ignored ones too and reports them.

use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use terra_build::ops::{Claim, Context, Op, Planned, Refusal, World};
use terra_build::support::Holder;
use terra_build::{
    Amount, Base, Bill, CELL_M, Cube, Facing, Fill, Kind, PartId, Prop, PropKind, Rules, Sample,
    Shape, Terms, Walker,
};

const LOGS: Kind = Kind(1);
const STONE: Kind = Kind(2);
const ROOF: Kind = Kind::FIRST_ROOF;
const DOOR: PropKind = PropKind(1);

fn rules() -> Rules {
    Rules {
        kinds: BTreeMap::from([
            (LOGS, Terms::new(2, Bill::of("Log", Amount::new(1, 16)))),
            (STONE, Terms::new(1, Bill::of("Stone", Amount::new(1, 4)))),
            (ROOF, Terms::new(4, Bill::of("Shakes", Amount::new(1, 32)))),
        ]),
        props: BTreeMap::from([(DOOR, Bill::of("Door", Amount::whole(1)))]),
        toughness: BTreeMap::new(),
    }
}

/// Who stands where and what rests on what, round a base in a test.
#[derive(Default)]
struct Round {
    bodies: BTreeSet<[i32; 3]>,
    items: BTreeSet<[i32; 3]>,
}

impl World for Round {
    fn in_the_way(&self, at: IVec3) -> bool {
        self.bodies.contains(&at.to_array())
    }
    fn holds_item(&self, at: IVec3) -> bool {
        self.items.contains(&at.to_array())
    }
}

/// Plans cubes of a shape in a new part, on level ground.
fn plan(base: &mut Base, cx: &Context, kind: Kind, cubes: &[[i32; 3]], shape: Shape) -> PartId {
    let part = base.next_part();
    let op = Op::LayPlan {
        part: Some(part),
        kind,
        cubes: cubes.iter().map(|&at| Planned { at, shape }).collect(),
        ground: cubes
            .iter()
            .map(|at| Sample {
                column: [at[0], at[2]],
                height_cm: 0,
            })
            .collect(),
    };
    base.act(&op, cx)
        .unwrap_or_else(|why| panic!("plan {cubes:?}: {why:?}"));
    part
}

/// Fills cubes to built, a step at a time, bottom layer first.
fn build(base: &mut Base, cx: &Context, cubes: &[[i32; 3]]) {
    let mut sorted = cubes.to_vec();
    sorted.sort_by_key(|at| at[1]);
    for at in sorted {
        while !base.cube(IVec3::from_array(at)).unwrap().is_built() {
            base.act(&Op::Fill { cubes: vec![at] }, cx)
                .unwrap_or_else(|why| panic!("fill {at:?}: {why:?}"));
        }
    }
}

fn column(x: i32, z: i32, from: i32, to: i32) -> Vec<[i32; 3]> {
    (from..=to).map(|y| [x, y, z]).collect()
}

fn fill(at: [i32; 3]) -> Op {
    Op::Fill { cubes: vec![at] }
}

fn empty(at: [i32; 3]) -> Op {
    Op::Empty { cubes: vec![at] }
}

// Structure: nothing floats (construction-001 rule 11, D193, D194).

#[test]
fn nothing_is_built_in_the_air_off_an_overhang_into_nothing_or_off_an_unbuilt_post() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // In the air: a cube two metres up, alone.
    plan(&mut base, &cx, STONE, &[[0, 4, 0]], Shape::Whole);
    assert_eq!(
        base.act(&fill([0, 4, 0]), &cx),
        Err(Refusal::Floats(IVec3::new(0, 4, 0)))
    );
    // Off an overhang: a wall three high with a beam two out from its top. A cube hung
    // under the beam's end is joined through it (a cantilever is allowed while joined,
    // D142); one a cube below that, with air between, is not.
    let wall = column(10, 0, 0, 2);
    plan(&mut base, &cx, STONE, &wall, Shape::Whole);
    build(&mut base, &cx, &wall);
    let beam = [[11, 2, 0], [12, 2, 0]];
    plan(&mut base, &cx, STONE, &beam, Shape::Whole);
    build(&mut base, &cx, &beam);
    plan(&mut base, &cx, STONE, &[[12, 1, 0]], Shape::Whole);
    assert!(base.act(&fill([12, 1, 0]), &cx).is_ok());
    plan(&mut base, &cx, STONE, &[[13, 0, 0]], Shape::Half);
    assert!(base.act(&fill([13, 0, 0]), &cx).is_ok(), "on the ground");
    plan(&mut base, &cx, STONE, &[[14, 2, 0]], Shape::Whole);
    assert!(
        base.act(&fill([14, 2, 0]), &cx).is_err(),
        "nothing joins it"
    );
    // Off a post with nothing under it: a post planned a storey high, only its foot in;
    // neither its top nor a deck beside its top goes in.
    let post = column(20, 0, 0, 5);
    plan(&mut base, &cx, LOGS, &post, Shape::Post);
    build(&mut base, &cx, &post[..1]);
    assert_eq!(
        base.act(&fill([20, 5, 0]), &cx),
        Err(Refusal::Floats(IVec3::new(20, 5, 0)))
    );
    plan(&mut base, &cx, LOGS, &[[21, 5, 0]], Shape::Whole);
    assert_eq!(
        base.act(&fill([21, 5, 0]), &cx),
        Err(Refusal::Floats(IVec3::new(21, 5, 0)))
    );
    // Built up, the deck goes in on it.
    build(&mut base, &cx, &post);
    assert!(base.act(&fill([21, 5, 0]), &cx).is_ok());
    assert!(base.floating().is_empty());
}

#[test]
fn a_roof_cube_over_a_missing_wall_cube_hangs_from_its_neighbours_as_any_cube_does() {
    // D199 asked a built cube under every eave; D241 made a roof cubes that hold as any
    // cube does, so an eave over a gap in the wall is joined along the eave, as a lintel.
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    let wall: Vec<[i32; 3]> = (0..4).flat_map(|x| column(x, 0, 0, 2)).collect();
    plan(&mut base, &cx, STONE, &wall, Shape::Whole);
    let gap: Vec<[i32; 3]> = wall.iter().copied().filter(|at| at[0] != 2).collect();
    build(&mut base, &cx, &gap);
    let eave: Vec<[i32; 3]> = (0..4).map(|x| [x, 3, 0]).collect();
    plan(&mut base, &cx, ROOF, &eave, Shape::Slope(Facing::South));
    let done = base.act(
        &Op::Fill {
            cubes: eave.clone(),
        },
        &cx,
    );
    assert!(done.is_ok(), "{done:?}");
    // With no wall under it at all, it floats.
    let mut bare = Base::default();
    plan(
        &mut bare,
        &cx,
        ROOF,
        &eave.iter().map(|at| [at[0], 6, 0]).collect::<Vec<_>>(),
        Shape::Slope(Facing::South),
    );
    assert!(matches!(
        bare.act(
            &Op::Fill {
                cubes: (0..4).map(|x| [x, 6, 0]).collect()
            },
            &cx
        ),
        Err(Refusal::Floats(_))
    ));
}

// Structure: nothing is taken while something rests on it (D193).

#[test]
fn nothing_is_taken_from_under_a_cube_an_eave_an_item_or_a_prop() {
    let rules = rules();
    let mut round = Round::default();
    let mut base = Base::default();
    {
        let cx = Context {
            rules: &rules,
            claim: None,
            world: &round,
        };
        // A wall five long and five high with a hole a cube wide and four high, a roof's
        // eave along its top.
        let wall: Vec<[i32; 3]> = (0..5)
            .flat_map(|x| column(x, 0, 0, 4))
            .filter(|&[x, y, _]| !(x == 2 && y < 4))
            .collect();
        plan(&mut base, &cx, STONE, &wall, Shape::Whole);
        build(&mut base, &cx, &wall);
        let eave: Vec<[i32; 3]> = (0..5).map(|x| [x, 5, 0]).collect();
        plan(&mut base, &cx, ROOF, &eave, Shape::Slope(Facing::South));
        build(&mut base, &cx, &eave);
        // Under a cube.
        assert_eq!(
            base.act(&empty([0, 3, 0]), &cx),
            Err(Refusal::Holds(vec![Holder::Cube(IVec3::new(0, 4, 0))]))
        );
        // Under the roof's eave.
        assert_eq!(
            base.act(&empty([4, 4, 0]), &cx),
            Err(Refusal::Holds(vec![Holder::Cube(IVec3::new(4, 5, 0))]))
        );
        // Under a prop: a door fitted into the hole holds its lintel and jambs.
        let door = Prop {
            kind: DOOR,
            min: [2, 0, 0],
            max: [2, 3, 0],
            open: false,
            ..Prop::default()
        };
        base.act(&Op::FitProp(door), &cx).unwrap();
        // The eave over the lintel comes off first; then the lintel still holds the door.
        base.act(&empty([2, 5, 0]), &cx).unwrap();
        while base.is_in(IVec3::new(2, 5, 0)) {
            base.act(&empty([2, 5, 0]), &cx).unwrap();
        }
        assert_eq!(
            base.act(&empty([2, 4, 0]), &cx),
            Err(Refusal::Holds(vec![Holder::Prop(IVec3::new(2, 0, 0))]))
        );
    }
    // Under an item standing on the wall's top, once the eave over it is off.
    round.items.insert([0, 4, 0]);
    let cx = Context {
        rules: &rules,
        claim: None,
        world: &round,
    };
    while base.is_in(IVec3::new(0, 5, 0)) {
        base.act(&empty([0, 5, 0]), &cx).unwrap();
    }
    assert_eq!(
        base.act(&empty([0, 4, 0]), &cx),
        Err(Refusal::Holds(vec![Holder::Item(IVec3::new(0, 4, 0))]))
    );
}

#[test]
fn carving_and_replacing_in_place_never_leave_anything_unjoined() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A post three high with a beam two out from its top: all of the beam hangs from the
    // post's middle.
    let post = column(0, 0, 0, 2);
    plan(&mut base, &cx, STONE, &post, Shape::Whole);
    build(&mut base, &cx, &post);
    let beam = [[1, 2, 0], [2, 2, 0]];
    plan(&mut base, &cx, STONE, &beam, Shape::Whole);
    build(&mut base, &cx, &beam);
    // Carved: marked, then refused when its last step would leave the top hanging.
    base.act(
        &Op::Carve {
            cubes: vec![[0, 1, 0]],
        },
        &cx,
    )
    .unwrap();
    assert!(matches!(
        base.act(&empty([0, 1, 0]), &cx),
        Err(Refusal::Holds(_))
    ));
    base.act(
        &Op::Cancel {
            cubes: vec![[0, 1, 0]],
        },
        &cx,
    )
    .unwrap();
    // Replaced in place: the old taken out from under what stays, refused the same.
    base.act(
        &Op::Replace {
            cubes: vec![[0, 0, 0]],
            kind: LOGS,
            part: Some(base.next_part()),
        },
        &cx,
    )
    .unwrap();
    assert!(matches!(
        base.act(&empty([0, 0, 0]), &cx),
        Err(Refusal::Holds(_))
    ));
    base.act(
        &Op::Cancel {
            cubes: vec![[0, 0, 0]],
        },
        &cx,
    )
    .unwrap();
    // With a second post under the beam's end, the first's middle can be carved out and the
    // top of it stays, held through the beam.
    let other = column(2, 0, 0, 1);
    plan(&mut base, &cx, STONE, &other, Shape::Whole);
    build(&mut base, &cx, &other);
    base.act(
        &Op::Carve {
            cubes: vec![[0, 1, 0]],
        },
        &cx,
    )
    .unwrap();
    base.act(&empty([0, 1, 0]), &cx).unwrap();
    assert!(base.floating().is_empty());
    // And its foot replaced: out, swapped, and in again as logs.
    base.act(
        &Op::Replace {
            cubes: vec![[0, 0, 0]],
            kind: LOGS,
            part: Some(base.next_part()),
        },
        &cx,
    )
    .unwrap();
    base.act(&empty([0, 0, 0]), &cx).unwrap();
    assert!(base.floating().is_empty());
}

// Structure: the claim, its budget and the ground (D193, D131).

#[test]
fn a_claim_bounds_what_is_planned_and_the_ground_is_kept_only_under_what_stands() {
    let rules = rules();
    let claim = Claim {
        min: [0, 0],
        max: [9, 9],
        budget: 1000,
    };
    let cx = Context {
        rules: &rules,
        claim: Some(&claim),
        world: &Round::default(),
    };
    let mut base = Base::default();
    // A plan reaching one cube past the claim is refused whole.
    let op = Op::LayPlan {
        part: Some(PartId(0)),
        kind: STONE,
        cubes: [[9, 0, 0], [10, 0, 0]]
            .map(|at| Planned {
                at,
                shape: Shape::Whole,
            })
            .to_vec(),
        ground: Vec::new(),
    };
    assert_eq!(
        base.act(&op, &cx),
        Err(Refusal::OutsideClaim(IVec3::new(10, 0, 0)))
    );
    // The ground sampled again under columns where nothing stands is not taken.
    plan(&mut base, &cx, STONE, &[[1, 0, 1]], Shape::Whole);
    let samples = vec![
        Sample {
            column: [1, 1],
            height_cm: -10,
        },
        Sample {
            column: [5, 5],
            height_cm: 80,
        },
    ];
    base.act(&Op::Ground { samples }, &cx).unwrap();
    assert_eq!(base.ground().sample([1, 1]), Some(-10));
    assert_eq!(base.ground().sample([5, 5]), None);
    // Taken out of the plan, its column's sample goes with it.
    base.act(
        &Op::Unplan {
            cubes: vec![[1, 0, 1]],
        },
        &cx,
    )
    .unwrap();
    assert_eq!(base.ground().sample([1, 1]), None);
}

#[test]
fn the_plan_budget_holds_however_a_base_is_built_and_taken_apart() {
    let rules = rules();
    let claim = Claim {
        min: [-20, -20],
        max: [20, 20],
        budget: 8,
    };
    let cx = Context {
        rules: &rules,
        claim: Some(&claim),
        world: &Round::default(),
    };
    let mut base = Base::default();
    // Two rows of eight, each planned within the budget and built.
    for z in 0..2 {
        let row: Vec<[i32; 3]> = (0..8).map(|x| [x, 0, z]).collect();
        plan(&mut base, &cx, STONE, &row, Shape::Whole);
        build(&mut base, &cx, &row);
    }
    assert_eq!(base.store().planned(), 0);
    // Taken apart as the hammer takes apart, each cube goes with its plan (D193, the
    // developers on 2026-10-15: "it should deconstruct and return materials").
    for x in 0..8 {
        for z in 0..2 {
            let ops = base.taking_apart(&[IVec3::new(x, 0, z)]);
            let _ = base.act_all(&ops, &cx);
        }
    }
    assert!(base.is_empty(), "something was left behind");
    assert!(
        base.store().planned() <= claim.budget,
        "{} cubes planned in a claim with a budget of {}",
        base.store().planned(),
        claim.budget
    );
}

// Physics: what a walker meets (D192, D220, D231), and building round them.

#[test]
fn building_where_someone_stands_or_shutting_a_door_on_them_is_refused() {
    let rules = rules();
    let mut round = Round::default();
    let mut base = Base::default();
    let wall: Vec<[i32; 3]> = (0..5)
        .flat_map(|x| column(x, 0, 0, 4))
        .filter(|&[x, y, _]| !(x == 2 && y < 4))
        .collect();
    {
        let cx = Context {
            rules: &rules,
            claim: None,
            world: &round,
        };
        plan(&mut base, &cx, STONE, &wall, Shape::Whole);
        build(&mut base, &cx, &wall);
        base.act(
            &Op::FitProp(Prop {
                kind: DOOR,
                min: [2, 0, 0],
                max: [2, 3, 0],
                open: true,
                ..Prop::default()
            }),
            &cx,
        )
        .unwrap();
        plan(&mut base, &cx, STONE, &[[0, 0, 3], [0, 1, 3]], Shape::Whole);
    }
    // Someone stands in the doorway and on the planned cubes.
    round.bodies.extend([
        [2, 0, 0],
        [2, 1, 0],
        [2, 2, 0],
        [2, 3, 0],
        [0, 0, 3],
        [0, 1, 3],
    ]);
    let cx = Context {
        rules: &rules,
        claim: None,
        world: &round,
    };
    assert_eq!(
        base.act(&fill([0, 0, 3]), &cx),
        Err(Refusal::InTheWay(IVec3::new(0, 0, 3)))
    );
    assert_eq!(
        base.act(
            &Op::SetOpen {
                at: [2, 0, 0],
                open: false
            },
            &cx
        ),
        Err(Refusal::InTheWay(IVec3::new(2, 0, 0)))
    );
    // Gone, both go.
    let cx = Context::free(&rules);
    base.act(&fill([0, 0, 3]), &cx).unwrap();
    base.act(
        &Op::SetOpen {
            at: [2, 0, 0],
            open: false,
        },
        &cx,
    )
    .unwrap();
}

/// Walks in small steps, stood each step on what holds the feet up, as the game does.
fn stroll(base: &Base, from: Vec3, to: Vec3) -> Vec3 {
    let w = Walker::default();
    let mut at = from;
    for i in 1..=400 {
        let target = from.lerp(to, i as f32 / 400.0);
        at = base.walk(at, Vec3::new(target.x, at.y, target.z), &w);
        at.y = base.support(at, &w).unwrap_or(0.0).max(0.0);
    }
    at
}

fn one(at: [i32; 3], shape: Shape) -> Base {
    let mut base = Base::default();
    base.apply(terra_build::BuildOp::Fill {
        min: at,
        max: at,
        cube: Some(Cube {
            shape,
            ..Cube::new(STONE, Fill::Solid)
        }),
    });
    base
}

#[test]
fn shapes_overhead_stop_a_walker_where_their_underside_is_lower_than_their_head() {
    let w = Walker::default();
    // Walked at from the west at ground level, a cube's column at x 0.
    let (from, to) = (Vec3::new(-1.0, 0.0, 0.25), Vec3::new(1.5, 0.0, 0.25));
    let passes = |base: &Base| (stroll(base, from, to) - to).length() < 0.01;
    for layer in 0..6 {
        let foot = layer as f32 * CELL_M;
        for shape in [
            Shape::UpperHalf,
            Shape::InvertedSlope(Facing::East),
            Shape::InvertedSlope(Facing::West),
            Shape::InvertedStair(Facing::East),
            Shape::InvertedStair(Facing::West),
        ] {
            let base = one([0, layer, 0], shape);
            // The lowest its underside comes: an upper half's half way up; an inverted
            // stair's or slope's down to the cube's foot on one side.
            let lowest = match shape {
                Shape::UpperHalf => foot + CELL_M * 0.5,
                _ => foot,
            };
            // What a walker cannot step up or duck under stops them.
            let top = foot + CELL_M;
            let stops = top > w.step_up && lowest < w.height;
            assert_eq!(passes(&base), !stops, "{shape:?} at layer {layer}");
        }
    }
}

#[test]
fn trims_standing_out_of_a_wall_never_stop_a_walker_short_of_its_cubes() {
    // A walker meets the cubes, never the pieces drawn on them (D192 as amended): walked
    // at a wall, they stop their radius short of its cubes' face, whatever its kit hangs
    // there.
    let mut base = Base::default();
    base.apply(terra_build::BuildOp::Fill {
        min: [4, 0, -4],
        max: [4, 5, 4],
        cube: Some(Cube::new(STONE, Fill::Solid)),
    });
    let w = Walker::default();
    let at = stroll(&base, Vec3::new(0.0, 0.0, 0.25), Vec3::new(4.0, 0.0, 0.25));
    let face = 4.0 * CELL_M;
    assert!(
        at.x <= face - w.radius + 1e-4 && at.x > face - w.radius - 0.02,
        "{at}"
    );
}

// The goals review's defects (research/goals-review-2026-10-13.md, findings 3 to 5), as
// rules (D260, D261).

#[test]
fn a_section_can_be_built_by_hand_a_cube_at_a_time() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    // A floor of a section's 256 columns, each cube laid alone, as the hammer lays a block:
    // in the part of the cubes of its kind beside it laid on its terms, or one of its own.
    // Every other column first, so most begin alone and are joined up later.
    let terms = rules.of(STONE);
    for (x, z) in (0..16)
        .flat_map(|x| (0..16).map(move |z| (x, z)))
        .filter(|(x, z)| (x + z) % 2 == 0)
        .chain(
            (0..16)
                .flat_map(|x| (0..16).map(move |z| (x, z)))
                .filter(|(x, z)| (x + z) % 2 == 1),
        )
    {
        let at = IVec3::new(x, 0, z);
        let (part, mut ops) = base
            .joining(&[at], STONE, &terms)
            .unwrap_or_else(|| (base.next_part(), Vec::new()));
        ops.push(Op::LayPlan {
            part: Some(part),
            kind: STONE,
            cubes: vec![Planned {
                at: at.to_array(),
                shape: Shape::Whole,
            }],
            ground: Vec::new(),
        });
        let laid = base.act_all(&ops, &cx);
        assert!(laid.is_ok(), "the block at {x}, {z}: {laid:?}");
    }
    assert_eq!(base.parts().count(), 1, "one floor, one part");
}

/// A ring of whole stone cubes round a room four by three tiles inside, from one layer
/// to another, on a floor, under a flat roof of cubes one layer over its top.
fn ring(base: &mut Base, from: i32, to: i32) {
    let cube = Some(Cube::new(STONE, Fill::Solid));
    let fill = |base: &mut Base, min: [i32; 3], max: [i32; 3]| {
        base.apply(terra_build::BuildOp::Fill { min, max, cube });
    };
    fill(base, [0, from - 1, 0], [5, from - 1, 4]);
    for (min, max) in [
        ([0, 0], [5, 0]),
        ([0, 4], [5, 4]),
        ([0, 0], [0, 4]),
        ([5, 0], [5, 4]),
    ] {
        fill(base, [min[0], from, min[1]], [max[0], to, max[1]]);
    }
    fill(base, [0, to + 1, 0], [5, to + 1, 4]);
}

#[test]
fn a_room_is_what_its_cubes_close_in_whatever_layer_its_walls_start_on() {
    // A storey's walls from its own layer close a room in.
    let mut on = Base::default();
    ring(&mut on, 0, 4);
    assert!(!on.indoors().is_empty(), "walls from the storey's layer");
    // The same walls begun three layers up, as a hand builds on a plinth, close one too.
    let mut off = Base::default();
    ring(&mut off, 3, 7);
    assert!(
        !off.indoors().is_empty(),
        "walls from layer 3 to 7 close nothing in"
    );
}

#[test]
fn everything_a_walker_stands_on_is_a_cube() {
    // A room with a doorway, by the room shortcut, a metre over the ground: its steps down
    // to the ground are cubes of the base, which a walker stands on.
    let mut base = Base::default();
    base.apply(terra_build::BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [7, 0, 5],
        kind: STONE,
    });
    for x in 3..5 {
        base.apply(terra_build::BuildOp::SetWall {
            tile: [x, 0, 5],
            panel: Some(terra_build::Panel::Door),
            kind: STONE,
        });
    }
    assert!(base.lay_door_steps(STONE, |_| -100));
    let w = Walker::default();
    for (at, cube) in base.cubes().filter(|(_, cube)| cube.shape != Shape::Whole) {
        // Over each step, a walker stands on that cube's top.
        let point = (at.as_vec3() + Vec3::new(0.5, 1.0, 0.5)) * CELL_M;
        let top = (at.y as f32 + cube.shape.top(0.5, 0.5)) * CELL_M;
        let stood = base.support(point.with_y(top + 0.05), &w);
        assert!(
            stood.is_some_and(|y| (y - top).abs() < 1e-3),
            "{at}: {stood:?}"
        );
    }
    assert!(
        base.cubes()
            .any(|(_, cube)| matches!(cube.shape, Shape::Stair(_)))
    );
}

// Loss and repair (D193, D305, step 6): what loses its support without being asked comes
// down, and is said as it was for the game to burst into fragments; nothing floats.

/// Something to hit: a stone room's walls eight cubes high, a floor across their tops,
/// and logs standing out from one side, on level ground.
fn hall(base: &mut Base, cx: &Context) -> Vec<[i32; 3]> {
    let mut walls = Vec::new();
    for x in 0..8 {
        for z in 0..6 {
            if x == 0 || x == 7 || z == 0 || z == 5 {
                walls.extend(column(x, z, 0, 7));
            }
        }
    }
    plan(base, cx, STONE, &walls, Shape::Whole);
    build(base, cx, &walls);
    let floor: Vec<[i32; 3]> = (1..7)
        .flat_map(|x| (1..5).map(move |z| [x, 7, z]))
        .collect();
    plan(base, cx, STONE, &floor, Shape::Whole);
    build(base, cx, &floor);
    let hangs: Vec<[i32; 3]> = (1..4).flat_map(|z| [[8, 4, z], [9, 4, z]]).collect();
    plan(base, cx, LOGS, &hangs, Shape::Whole);
    build(base, cx, &hangs);
    walls
}

#[test]
fn whatever_is_hit_nothing_is_left_floating_and_what_came_down_is_said_as_it_was() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    let walls = hall(&mut base, &cx);
    let value = |base: &Base| {
        let mut bill = Bill::default();
        for (_, cube) in base.cubes() {
            bill.add(&base.put_in(&cube));
        }
        bill
    };
    // Every cube of its walls in turn, from the bottom, hit hard enough to destroy it.
    for at in walls.iter().filter(|at| at[1] < 3) {
        if !base.is_in(IVec3::from_array(*at)) {
            continue;
        }
        let (was, before) = (value(&base), base.clone());
        let done = base
            .act(
                &Op::Damage {
                    cubes: vec![*at],
                    amount: u16::MAX,
                },
                &cx,
            )
            .unwrap();
        assert!(
            base.floating().is_empty(),
            "{at:?} left {:?}",
            base.floating()
        );
        // What came down is every cube gone, as it was; what went into it is lost, nothing
        // made or lost besides.
        let mut fell = was.clone();
        fell.take(&value(&base)).expect("less is in");
        assert_eq!(done.lost, fell, "{at:?}");
        let gone = before.cubes().count() - base.cubes().count();
        assert_eq!(done.fell.len(), gone, "{at:?}");
        for (place, cube) in &done.fell {
            assert_eq!(before.cube(*place), Some(*cube));
            assert_eq!(base.cube(*place), None);
        }
    }
}

#[test]
fn a_damaged_cube_is_mended_before_anything_more_goes_in_or_comes_out() {
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    let post = column(0, 0, 0, 2);
    plan(&mut base, &cx, LOGS, &post, Shape::Whole);
    // The post's foot one course in of two, hit.
    base.act(&fill([0, 0, 0]), &cx).unwrap();
    base.act(
        &Op::Damage {
            cubes: vec![[0, 0, 0]],
            amount: 10,
        },
        &cx,
    )
    .unwrap();
    let at = IVec3::new(0, 0, 0);
    assert_eq!(base.check(&fill([0, 0, 0]), &cx), Err(Refusal::Damaged(at)));
    assert_eq!(
        base.check(&empty([0, 0, 0]), &cx),
        Err(Refusal::Damaged(at))
    );
    base.act(
        &Op::Repair {
            cubes: vec![[0, 0, 0]],
        },
        &cx,
    )
    .unwrap();
    base.act(&fill([0, 0, 0]), &cx).unwrap();
    assert!(base.cube(at).unwrap().is_built());
}

#[test]
fn a_breach_knocked_under_a_standing_wall_is_built_again() {
    // A log wall three long and three high; the middle of its first course knocked out,
    // the wall over it held by the courses either side. Laid again, the breach goes in
    // whole in one act, worked in from the side, both its courses' logs taken; undone, it
    // comes out from under the wall as it went in.
    let rules = rules();
    let cx = Context::free(&rules);
    let mut base = Base::default();
    let wall: Vec<[i32; 3]> = (0..3).flat_map(|x| column(x, 0, 0, 2)).collect();
    plan(&mut base, &cx, LOGS, &wall, Shape::Whole);
    build(&mut base, &cx, &wall);
    base.act(
        &Op::Damage {
            cubes: vec![[1, 0, 0]],
            amount: u16::MAX,
        },
        &cx,
    )
    .unwrap();
    let breach = IVec3::new(1, 0, 0);
    assert_eq!(base.cube(breach), None);
    assert!(base.is_in(breach + IVec3::Y), "the wall over it stands");
    plan(&mut base, &cx, LOGS, &[[1, 0, 0]], Shape::Whole);
    let before = base.clone();
    let done = base.act(&fill([1, 0, 0]), &cx).unwrap();
    assert!(base.cube(breach).unwrap().is_built());
    assert_eq!(
        done.took,
        Bill::of("Log", Amount::new(1, 8)),
        "both courses"
    );
    assert!(base.floating().is_empty());
    let back = base.act_all(&done.undo, &cx).unwrap();
    assert_eq!(back.gave, done.took);
    assert_eq!(base, before, "undone, as it was");
}
