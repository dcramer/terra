use super::*;
use terra_build::kit::Kit;
use terra_build::{BuildOp, Cube};

const STONE: Kind = Kind(2);
const LOGS: Kind = Kind(1);

fn kits() -> Kits {
    Kits(vec![
        Kit::default(),
        Kit {
            id: 2,
            pattern: Pattern::Masonry,
            ..Kit::default()
        },
    ])
}

fn fill(base: &mut Base, min: [i32; 3], max: [i32; 3], kind: Kind) {
    base.apply(BuildOp::Fill {
        min,
        max,
        cube: Some(Cube::new(kind, Fill::Solid)),
    });
}

fn base(boxes: &[([i32; 3], [i32; 3])]) -> Base {
    let mut base = Base::default();
    for &(min, max) in boxes {
        fill(&mut base, min, max, STONE);
    }
    base
}

/// Every dressed edge of the kind's cubes.
fn all(base: &Base) -> Vec<Dressed> {
    let kits = kits();
    let roles = Roles::new(base, STONE, &kits);
    roles.all(base.cubes().map(|(at, _)| at))
}

fn count(dressed: &[Dressed], role: Role) -> usize {
    dressed.iter().filter(|d| d.edge.role == role).count()
}

fn at(dressed: &[Dressed], cube: [i32; 3], role: Role) -> Vec<&Dressed> {
    dressed
        .iter()
        .filter(|d| d.cube == IVec3::from_array(cube) && d.edge.role == role)
        .collect()
}

#[test]
fn a_wall_top_under_a_roofs_cubes_is_no_crest_and_one_open_to_the_sky_is() {
    let mut base = Base::default();
    base.apply(BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [7, 0, 5],
        kind: STONE,
    });
    // Open to the sky: its walls' tops are crests.
    base.apply(BuildOp::SetRoof {
        min: [1, 0, 1],
        max: [6, 0, 4],
        laid: false,
        kind: STONE,
    });
    assert!(count(&all(&base), Role::Crest) > 0);
    // Under a gable of roof cubes, none: the roof is over them, and a coping there
    // would stand up through it.
    let footprint = (0..8).flat_map(|x| (0..6).map(move |z| [x, z])).collect();
    let plan = terra_build::roofer::over(&footprint, 6, terra_build::roofer::Style::Gable, None);
    for (at, shape) in plan.roof {
        base.apply(BuildOp::Fill {
            min: at.to_array(),
            max: at.to_array(),
            cube: Some(Cube {
                shape,
                ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
            }),
        });
    }
    for at in plan.gables {
        fill(&mut base, at.to_array(), at.to_array(), STONE);
    }
    assert_eq!(count(&all(&base), Role::Crest), 0);
}

#[test]
fn a_gable_carried_up_into_its_roof_has_no_edge_at_its_steps() {
    // A gable of stone under roof cubes of a roof kit: the wall runs on into the roof
    // cubes standing on it, so the steps the gable's cubes climb in have no corner up
    // them, no cap or top along them, nothing to stand out under the roof.
    let mut kits = kits();
    kits.0.push(Kit {
        id: Kind::FIRST_ROOF.0,
        pattern: Pattern::Roof,
        ..Kit::default()
    });
    let mut base = Base::default();
    base.apply(BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [7, 0, 5],
        kind: STONE,
    });
    let footprint = (0..8).flat_map(|x| (0..6).map(move |z| [x, z])).collect();
    let plan = terra_build::roofer::over(&footprint, 6, terra_build::roofer::Style::Gable, None);
    for (at, shape) in plan.roof {
        base.apply(BuildOp::Fill {
            min: at.to_array(),
            max: at.to_array(),
            cube: Some(Cube {
                shape,
                ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
            }),
        });
    }
    for at in plan.gables {
        fill(&mut base, at.to_array(), at.to_array(), STONE);
    }
    let roles = Roles::new(&base, STONE, &kits);
    let gable: Vec<IVec3> = base
        .cubes()
        .filter(|(at, c)| c.kind == STONE && at.y >= 6)
        .map(|(at, _)| at)
        .collect();
    assert!(gable.len() > 10);
    for at in gable {
        for edge in roles.edges(at) {
            assert!(
                edge.other != Dir::Up && edge.role != Role::Arris,
                "{at}: {edge:?} on a gable under its roof"
            );
        }
    }
}

#[test]
fn a_free_wall_has_arrises_up_its_ends_crests_along_its_top_and_a_foot() {
    // Four cubes long, three high, one thick, standing on the ground at the base's level.
    let base = base(&[([0, 0, 0], [3, 2, 0])]);
    let d = all(&base);
    // Four uprights at its ends, each cube of them.
    assert_eq!(count(&d, Role::Arris), 4 * 3);
    // Along the top, both faces and both ends.
    assert_eq!(count(&d, Role::Crest), 4 * 2 + 2);
    // Along the foot likewise: it stands on the ground.
    assert_eq!(count(&d, Role::Foot), 4 * 2 + 2);
    for role in [
        Role::Soffit,
        Role::Jamb,
        Role::Inside,
        Role::Border,
        Role::Nose,
    ] {
        assert_eq!(count(&d, role), 0, "{role:?}");
    }
    // The crest along the south face runs on between cubes and turns out round the
    // corners.
    let south = |x: i32| {
        at(&d, [x, 2, 0], Role::Crest)
            .into_iter()
            .find(|e| e.edge.side == Dir::South)
            .unwrap()
            .ends
    };
    assert_eq!(south(0), [End::Out, End::On]);
    assert_eq!(south(1), [End::On, End::On]);
    assert_eq!(south(3), [End::On, End::Out]);
    // Each edge belongs to one cube only.
    let mut seen = std::collections::BTreeSet::new();
    for e in &d {
        let line = line(e.cube, e.edge);
        assert!(seen.insert((line, e.edge.side, e.edge.other)), "{e:?}");
    }
}

#[test]
fn an_l_has_an_inside_corner_whose_crests_mitre_back_into_it() {
    // An L: along x from the corner, and along z from it.
    let base = base(&[([0, 0, 0], [3, 1, 0]), ([0, 0, 1], [0, 1, 3])]);
    let d = all(&base);
    // The inside corner, up the line at x = 1, z = 1, belongs to the cube looking east
    // into it.
    let inside: Vec<_> = d.iter().filter(|e| e.edge.role == Role::Inside).collect();
    assert_eq!(inside.len(), 2, "{inside:?}");
    assert!(
        inside
            .iter()
            .all(|e| e.cube.x == 0 && e.cube.z == 1 && e.edge.side == Dir::East)
    );
    // The crest along the inner face of the x arm stops mitred back at the corner.
    let crest = at(&d, [1, 1, 0], Role::Crest)
        .into_iter()
        .find(|e| e.edge.side == Dir::South)
        .unwrap();
    assert_eq!(crest.ends[0], End::Back);
    // The outer corner's crests mitre out.
    let outer = at(&d, [0, 1, 0], Role::Crest);
    assert!(
        outer
            .iter()
            .any(|e| e.edge.side == Dir::North && e.ends == [End::Out, End::On])
    );
}

#[test]
fn a_hole_has_jambs_a_sill_and_a_head() {
    let mut base = base(&[([0, 0, 0], [5, 5, 0])]);
    // A window carved out of it: an opening's cubes.
    base.apply(BuildOp::Fill {
        min: [2, 2, 0],
        max: [3, 3, 0],
        cube: Some(Cube::new(STONE, Fill::Open)),
    });
    let d = all(&base);
    // Jambs up both sides of it, both faces, two cubes high.
    assert_eq!(count(&d, Role::Jamb), 2 * 2 * 2);
    // A sill on both faces under it, a head over it, each two cubes long.
    assert_eq!(count(&d, Role::Sill), 2 * 2);
    assert_eq!(count(&d, Role::Head), 2 * 2);
    let sill = at(&d, [2, 1, 0], Role::Sill);
    assert!(sill.iter().all(|e| e.edge.other == Dir::Up));
    // The sill runs on into the next cube's and stops square at the jambs.
    assert!(sill.iter().all(|e| e.ends == [End::Square, End::On]));
}

#[test]
fn an_overhang_has_soffits_and_steps_have_noses() {
    // A pillar under a slab overhanging it.
    let overhang = base(&[([0, 0, 0], [0, 3, 0]), ([-2, 4, -1], [2, 4, 1])]);
    let d = all(&overhang);
    assert!(count(&d, Role::Soffit) >= 4 * 2);
    // A flight of three steps, each a cube higher, rising north.
    let mut steps = Base::default();
    for step in 0..3 {
        fill(&mut steps, [0, 0, -step], [2, step, -step], STONE);
    }
    let d = all(&steps);
    // Each step's south edge is a nose but the top one's, which nothing rises behind.
    assert_eq!(count(&d, Role::Nose), 2 * 3, "{:?}", d);
    assert!(
        d.iter()
            .filter(|e| e.edge.role == Role::Nose)
            .all(|e| e.edge.side == Dir::South)
    );
    // A riser's foot on the step under it is no floor border: nothing runs on beyond.
    assert_eq!(count(&d, Role::Border), 0);
}

#[test]
fn a_wall_on_a_floor_has_a_border_inside_and_a_foot_outside() {
    // A floor three cubes deep, a wall standing on its north edge.
    let base = base(&[([0, 0, 0], [3, 0, 2]), ([0, 1, 0], [3, 2, 0])]);
    let d = all(&base);
    // Along the wall's south face, where the floor runs on south of it.
    let border: Vec<_> = d.iter().filter(|e| e.edge.role == Role::Border).collect();
    assert_eq!(border.len(), 4, "{border:?}");
    assert!(
        border
            .iter()
            .all(|e| e.cube.y == 1 && e.edge.side == Dir::South)
    );
}

#[test]
fn a_wall_over_a_lean_to_has_no_border_on_its_roof() {
    // A wall three cubes high, a lean-to's roof cubes against its south face two cubes
    // deep, standing on the stone under it: where the wall's face rises out of the
    // roof, it meets the roof's slope, no floor it borders.
    let mut base = base(&[([0, 0, 0], [3, 2, 0]), ([0, 0, 1], [3, 0, 2])]);
    fill(&mut base, [0, 1, 1], [3, 1, 2], Kind::FIRST_ROOF);
    let d = all(&base);
    assert_eq!(count(&d, Role::Border), 0, "{d:?}");
}

#[test]
fn a_wall_with_a_plinth_on_a_footing_with_its_own_takes_none() {
    // An earth wall on a stone footing a cube high on the ground: the footing's plinth
    // is the foot there, not a second one stacked on it and run into it; on the
    // ground, the earth has its own.
    let plinth = |id: u16| {
        let mut kit = Kit {
            id,
            pattern: Pattern::Masonry,
            ..Kit::default()
        };
        kit.dress.roles.insert(
            Role::Foot,
            terra_build::kit::Dress::Profile {
                name: "plinth".into(),
                extend: 0.0,
                joints: true,
                material: None,
            },
        );
        kit
    };
    let kits = Kits(vec![plinth(2), plinth(7)]);
    let earth = Kind(7);
    let mut base = base(&[([0, 0, 0], [3, 0, 0])]);
    fill(&mut base, [0, 1, 0], [3, 3, 0], earth);
    fill(&mut base, [5, 0, 0], [6, 3, 0], earth);
    let feet = |at: [i32; 3]| {
        Roles::new(&base, earth, &kits)
            .dressed(IVec3::from_array(at))
            .into_iter()
            .filter(|e| e.edge.role == Role::Foot)
            .count()
    };
    assert_eq!(feet([1, 1, 0]), 0);
    assert_eq!(feet([5, 0, 0]), 3);
    // The footing keeps its own.
    let stone = Roles::new(&base, STONE, &kits);
    assert!(
        stone
            .dressed(IVec3::new(1, 0, 0))
            .iter()
            .any(|e| e.edge.role == Role::Foot)
    );
}

#[test]
fn logs_on_stone_meet_it_with_a_cap_and_stone_quoined_beside_logs() {
    // A stone plinth a cube high, a log wall on it, the stone running on east past it.
    let mut base = base(&[([0, 0, 0], [5, 0, 0])]);
    fill(&mut base, [0, 1, 0], [3, 3, 0], LOGS);
    fill(&mut base, [4, 1, 0], [5, 3, 0], STONE);
    let d = all(&base);
    // The plinth's top under the logs meets them on both faces.
    assert_eq!(at(&d, [1, 0, 0], Role::Meet).len(), 2);
    // The stone beside the logs is quoined up its end against them: its faces show
    // past the thinner logs.
    assert_eq!(at(&d, [4, 2, 0], Role::Arris).len(), 2);
    // Seen from the logs, they stand on another kind: a foot.
    let kits = kits();
    let logs = Roles::new(&base, LOGS, &kits);
    let foot = logs
        .dressed(IVec3::new(1, 1, 0))
        .into_iter()
        .filter(|e| e.edge.role == Role::Foot)
        .count();
    assert_eq!(foot, 2);
}

#[test]
fn stone_over_logs_changed_into_a_wall_has_a_foot_that_does_not_sink_over_them() {
    // A stone wall with a block of logs two wide and two high changed into it, a
    // metre up: the stone over the logs stands on them, its foot level with them.
    let mut base = base(&[([0, 0, 0], [9, 5, 0])]);
    fill(&mut base, [4, 2, 0], [5, 3, 0], LOGS);
    let d = all(&base);
    let feet = at(&d, [4, 4, 0], Role::Foot);
    assert_eq!(feet.len(), 2);
    assert!(feet.iter().all(|foot| foot.sink == 0 && foot.lift == 0));
    // Its sides quoined, the stone running through, and capped under them.
    assert_eq!(at(&d, [3, 2, 0], Role::Arris).len(), 2);
    assert_eq!(at(&d, [4, 1, 0], Role::Meet).len(), 2);
}

#[test]
fn how_kinds_meet_is_the_kits_table_not_their_pattern() {
    use terra_build::kit::{Joint, Meet, Meets};
    let mut base = base(&[([0, 0, 0], [5, 0, 0])]);
    fill(&mut base, [0, 1, 0], [3, 3, 0], LOGS);
    fill(&mut base, [4, 1, 0], [5, 3, 0], STONE);
    // The defaults: stone runs through logs and caps them, logs end with a board.
    let kits = kits();
    assert!(kits.through(STONE, LOGS) && !kits.through(LOGS, STONE));
    assert_eq!(kits.carries(STONE, LOGS), Joint::Cap);
    assert_eq!(kits.end(LOGS, STONE), Joint::Board);
    // A stone whose table lets logs run through it: no quoins up the joint, no cap, and
    // the logs' table read the other way round.
    let mut table = Meets::of(Pattern::Masonry);
    table.kinds.insert(
        LOGS.0,
        Meet {
            through: Some(false),
            ..Meet::default()
        },
    );
    let mut changed = kits.clone();
    changed.0[1].meets = Some(table);
    assert!(changed.through(LOGS, STONE));
    assert_eq!(changed.end(LOGS, STONE), Joint::None);
    let roles = Roles::new(&base, STONE, &changed);
    let d = roles.all(base.cubes().map(|(at, _)| at));
    assert!(at(&d, [1, 0, 0], Role::Meet).is_empty());
    assert!(at(&d, [4, 2, 0], Role::Arris).is_empty());
    // A heavier timber, by weight alone, runs through stone the same way.
    let mut heavy = kits.clone();
    heavy.0[0].meets = Some(Meets {
        weight: 4,
        ..Meets::of(Pattern::Logs)
    });
    assert!(heavy.through(LOGS, STONE));
}

#[test]
fn a_foot_on_a_slope_steps_with_the_ground() {
    // A wall along x on footings down into ground rising a quarter cube a cube east.
    let mut base = Base::default();
    let cm = terra_build::ground::cube_cm();
    for x in 0..8 {
        fill(&mut base, [x, -1, 0], [x, 3, 0], STONE);
    }
    // Sampled once the cubes are there: a base keeps samples only under its cubes.
    for x in -1..9 {
        base.sample_ground((-1..=1).map(|z| terra_build::Sample {
            column: [x, z],
            height_cm: (x * cm / 4) as i16,
        }));
    }

    let d = all(&base);
    let feet: Vec<_> = d
        .iter()
        .filter(|e| e.edge.role == Role::Foot && e.edge.side == Dir::South)
        .collect();
    // A foot on every cube along it: the ground passes each.
    assert_eq!(feet.len(), 8, "{feet:?}");
    // Raised by whole steps, never down into the ground, and going on down to it.
    let ground = |x: i32| x * TICKS / 4;
    for e in &feet {
        let x = e.cube.x;
        let edge = e.cube.y * TICKS + e.lift;
        assert_eq!(e.lift % LIFT_STEP, 0);
        assert!(edge >= ground(x + 1).min(e.cube.y * TICKS + TICKS), "{e:?}");
        assert!(edge - e.sink <= ground(x - 1).max(ground(0)), "{e:?}");
    }
    // Where the lift changes, the ends step.
    assert!(feet.iter().any(|e| e.ends.contains(&End::Step)));
    // Nothing is dressed under the ground.
    assert!(d.iter().all(|e| e.edge.role == Role::Foot || e.cube.y >= 0));
}

#[test]
fn roles_are_the_same_worked_out_twice_and_only_for_the_kind() {
    let base = base(&[([0, 0, 0], [3, 2, 0])]);
    assert_eq!(all(&base), all(&base));
    let kits = kits();
    let logs = Roles::new(&base, LOGS, &kits);
    assert!(logs.dressed(IVec3::ZERO).is_empty());
}

#[test]
fn a_wall_broken_by_harm_leaves_its_break_undressed() {
    // A notch cut by hand from a wall's top is a laid opening, its sides arrises and its
    // floor a crest; broken by harm, no dressed edge faces it (D305, castle film r3).
    let notch: Vec<IVec3> = [[3, 2], [4, 2], [3, 3], [4, 3]]
        .map(|[x, y]| IVec3::new(x, y, 0))
        .to_vec();
    let facing = |d: &Dressed| {
        let into = |dir: Dir| notch.contains(&(d.cube + dir.step()));
        into(d.edge.side) || into(d.edge.other)
    };
    let mut cut = base(&[([0, 0, 0], [7, 3, 0])]);
    for at in &notch {
        let at = at.to_array();
        cut.apply(BuildOp::Fill {
            min: at,
            max: at,
            cube: None,
        });
    }
    let laid = all(&cut).iter().filter(|d| facing(d)).count();
    assert!(laid >= 4, "a cut notch has {laid} dressed edges facing it");
    let mut base = base(&[([0, 0, 0], [7, 3, 0])]);
    let (rules, cubes) = (
        kits().rules(),
        notch.iter().map(|at| at.to_array()).collect(),
    );
    let op = terra_build::ops::Op::Damage {
        cubes,
        amount: u16::MAX,
    };
    base.act(&op, &terra_build::ops::Context::free(&rules))
        .unwrap();
    assert!(notch.iter().all(|at| base.cube(*at).is_none()));
    let broken: Vec<_> = all(&base).into_iter().filter(|d| facing(d)).collect();
    let dressed: Vec<_> = broken.iter().map(|d| (d.cube, d.edge.role)).collect();
    assert!(broken.is_empty(), "dressed facing the break: {dressed:?}");
}

#[test]
fn every_edge_with_a_role_is_dressed_by_one_cube_only() {
    // docs/building-voxels.md, invariant 2: a line where faces meet is dressed once, by
    // the cube that owns it, whatever is round it. Here every way a kind's cubes meet
    // along an edge but at one edge alone (that document's gap list): outside and inside
    // corners, a T, a cross, a thick wall, a pier, steps, an overhang, a wall on a floor,
    // holes of every sort, a notch at a foot and another kind standing on it and beside it.
    let mut base = Base::default();
    // A room with a floor, its walls holed by a door, a window and a carved cross.
    base.apply(BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [9, 0, 7],
        kind: STONE,
    });
    let open = |base: &mut Base, min: [i32; 3], max: [i32; 3]| {
        base.apply(BuildOp::Fill {
            min,
            max,
            cube: Some(Cube::new(STONE, Fill::Open)),
        });
    };
    open(&mut base, [4, 0, 0], [5, 3, 0]);
    open(&mut base, [0, 2, 3], [0, 3, 4]);
    open(&mut base, [9, 2, 3], [9, 2, 5]);
    open(&mut base, [9, 1, 4], [9, 3, 4]);
    open(&mut base, [2, 0, 7], [2, 1, 7]);
    // A T and a cross inside it, a wall three thick and a pier, steps and an overhang.
    fill(&mut base, [1, 0, 3], [8, 4, 3], STONE);
    fill(&mut base, [5, 0, 1], [5, 4, 6], STONE);
    fill(&mut base, [12, 0, 0], [14, 5, 6], STONE);
    fill(&mut base, [17, 0, 2], [18, 5, 3], STONE);
    for step in 0..4 {
        fill(&mut base, [20 + step, 0, 0], [20 + step, step, 2], STONE);
    }
    fill(&mut base, [26, 0, 0], [26, 3, 0], STONE);
    fill(&mut base, [26, 4, 0], [29, 4, 0], STONE);
    // Logs standing on a stone wall and against its side.
    fill(&mut base, [12, 6, 0], [14, 9, 0], LOGS);
    fill(&mut base, [15, 0, 3], [17, 5, 3], LOGS);
    let dressed = all(&base);
    assert!(dressed.len() > 200, "{}", dressed.len());
    let mut lines: BTreeMap<([i32; 3], [i32; 3]), &Dressed> = BTreeMap::new();
    for d in &dressed {
        if let Some(other) = lines.insert(line(d.cube, d.edge), d) {
            panic!("one edge dressed twice: {other:?} and {d:?}");
        }
    }
}
