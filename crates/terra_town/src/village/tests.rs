use super::*;
use glam::Vec3;
use std::collections::{BTreeMap, BTreeSet};
use terra_build::Base;
use terra_build::construct::{construction, finished};
use terra_build::cost::{Bill, Rules, Terms};
use terra_build::ops::Context;
use terra_build::order::Step;

const KINDS: Kinds = Kinds {
    timber: Kind(1),
    masonry: Kind(2),
    beams: Kind(3),
    cob: Kind(4),
    framed: Kind(5),
    works: Kind(2),
    timber_roof: Kind(0x800),
    masonry_roof: Kind(0x801),
    beams_roof: Kind(0x802),
};

#[test]
fn every_building_is_roofed_in_cubes_that_hold() {
    for building in village(7, KINDS) {
        let base = Base::from_shortcuts(&building.ops);
        let roofed = base.cubes().filter(|(_, c)| c.kind.is_roof()).count();
        let wants = !matches!(building.plan, Plan::Wall | Plan::Deck);
        assert_eq!(roofed > 0, wants, "{}", building.name);
        assert!(
            base.floating().is_empty(),
            "{}: {:?}",
            building.name,
            base.floating()
        );
    }
}

/// A building's cubes' columns where they stand in the village, in metres east and
/// south, a cube's middle each.
fn columns(building: &Building) -> Vec<[f32; 2]> {
    let mut base = Base::default();
    for &op in &building.ops {
        base.apply(op);
    }
    let (s, c) = (libm::sinf(building.facing), libm::cosf(building.facing));
    base.occupied_columns()
        .map(|column| {
            let (x, z) = (
                (column.x as f32 + 0.5) * terra_build::CELL_M,
                (column.y as f32 + 0.5) * terra_build::CELL_M,
            );
            // Turned so its +Z looks along `facing`.
            [
                building.at[0] + x * c + z * s,
                building.at[1] - x * s + z * c,
            ]
        })
        .collect()
}

#[test]
fn a_village_is_the_same_from_the_same_seed_and_varies_with_it() {
    assert_eq!(village(7, KINDS), village(7, KINDS));
    assert_ne!(village(7, KINDS), village(8, KINDS));
}

#[test]
fn every_scheme_and_plan_is_there_and_no_two_buildings_stand_in_one_another() {
    let all = village(7, KINDS);
    let schemes: BTreeSet<Scheme> = all.iter().map(|b| b.scheme).collect();
    let plans: BTreeSet<Plan> = all.iter().map(|b| b.plan).collect();
    assert_eq!(schemes.len(), 9, "{schemes:?}");
    assert_eq!(plans.len(), 8, "{plans:?}");
    assert!(variants() >= 150, "{}", variants());
    let placed: Vec<Vec<[f32; 2]>> = all.iter().map(columns).collect();
    for (i, a) in placed.iter().enumerate() {
        for (j, b) in placed.iter().enumerate().skip(i + 1) {
            // A metre apart at least; lengths of one garden wall may meet at a corner.
            let apart = if all[i].plan == Plan::Wall && all[j].plan == Plan::Wall {
                0.4
            } else {
                1.0
            };
            let near = a.iter().any(|p| {
                b.iter()
                    .any(|q| (p[0] - q[0]).abs() < apart && (p[1] - q[1]).abs() < apart)
            });
            assert!(!near, "{} stands in {}", all[i].name, all[j].name);
        }
    }
}

#[test]
fn every_house_looks_onto_the_street_and_stands_off_it() {
    for building in village(7, KINDS)
        .iter()
        .filter(|b| !matches!(b.plan, Plan::Wall | Plan::Deck))
    {
        // Its door looks toward the street's middle line, z = 0.
        let toward = libm::cosf(building.facing) * -building.at[1].signum();
        assert!(toward > 0.9, "{}: {toward}", building.name);
        // Nothing of it in the street, three metres either side of its middle.
        assert!(
            columns(building).iter().all(|p| p[1].abs() > 3.0),
            "{} stands in the street",
            building.name
        );
    }
}

/// Whether any two buildings stand within a metre of one another, by the half-metre
/// cells their columns are in.
fn crowded(all: &[Building]) -> Option<(String, String)> {
    let mut taken: std::collections::HashMap<[i32; 2], usize> = Default::default();
    for (index, building) in all.iter().enumerate() {
        for p in columns(building) {
            let cell = [(p[0] / 0.5).floor() as i32, (p[1] / 0.5).floor() as i32];
            for dx in -2..=2 {
                for dz in -2..=2 {
                    if let Some(&other) = taken.get(&[cell[0] + dx, cell[1] + dz])
                        && other != index
                    {
                        return Some((building.name.clone(), all[other].name.clone()));
                    }
                }
            }
        }
        for p in columns(building) {
            taken.insert(
                [(p[0] / 0.5).floor() as i32, (p[1] / 0.5).floor() as i32],
                index,
            );
        }
    }
    None
}

#[test]
fn a_town_mixes_every_scheme_and_plan_and_no_two_buildings_stand_in_one_another() {
    let all = town(7, 90, KINDS);
    assert_eq!(all.len(), 90);
    let schemes: BTreeSet<Scheme> = all.iter().map(|b| b.scheme).collect();
    let plans: BTreeSet<Plan> = all.iter().map(|b| b.plan).collect();
    assert_eq!(schemes.len(), 9, "{schemes:?}");
    assert!(plans.len() >= 5, "{plans:?}");
    assert_eq!(crowded(&all), None);
    assert!(every_building().len() >= 200, "{}", every_building().len());
}

#[test]
fn a_town_of_many_materials_builds_each_house_of_one_and_uses_every_one() {
    // Six stones, each with its own roof; one of every other role.
    let stones: Vec<Kind> = (10..16).map(Kind).collect();
    let roofs = stones
        .iter()
        .map(|&k| (k, Kind(0x800 + k.0)))
        .chain([
            (KINDS.timber, KINDS.timber_roof),
            (KINDS.beams, KINDS.beams_roof),
        ])
        .collect();
    let palette = Palette {
        roles: [
            vec![KINDS.timber],
            stones.clone(),
            vec![KINDS.beams],
            vec![KINDS.cob],
            vec![KINDS.framed],
            vec![KINDS.works],
        ],
        roofs,
    };
    let all = town(7, 90, &palette);
    let mut used = BTreeSet::new();
    for building in &all {
        let mut base = Base::default();
        for &op in &building.ops {
            base.apply(op);
        }
        let walls: BTreeSet<Kind> = base
            .cubes()
            .map(|(_, cube)| cube.kind)
            .filter(|k| stones.contains(k))
            .collect();
        // A house is one stone throughout, and its roof the one that stone takes.
        assert!(walls.len() <= 1, "{}: {walls:?}", building.name);
        if let Some(&stone) = walls.first()
            && building.scheme == Scheme::Masonry
        {
            assert!(
                base.cubes()
                    .any(|(_, cube)| cube.kind == Kind(0x800 + stone.0)),
                "{}",
                building.name
            );
        }
        used.extend(walls);
    }
    assert_eq!(used.len(), stones.len(), "{used:?}");
    assert_eq!(palette.first().masonry, stones[0]);
}

#[test]
fn shortcuts_applied_together_lay_what_they_do_one_by_one() {
    for building in village(7, KINDS) {
        let mut one_by_one = Base::default();
        for &op in &building.ops {
            one_by_one.apply(op);
        }
        assert!(
            Base::from_shortcuts(&building.ops) == one_by_one,
            "{}",
            building.name
        );
    }
}

#[test]
fn hamlets_spread_out_to_their_radius() {
    let all = hamlets(7, 60, 2000.0, KINDS);
    assert_eq!(all.len(), 60);
    let furthest = all
        .iter()
        .map(|b| libm::hypotf(b.at[0], b.at[1]))
        .fold(0.0, f32::max);
    assert!((1200.0..2050.0).contains(&furthest), "{furthest}");
}

#[test]
fn a_two_storey_house_has_its_stair_up_to_a_landing_at_its_upper_door() {
    let all = village(7, KINDS);
    let two = all.iter().find(|b| b.plan == Plan::TwoStoreys).unwrap();
    let mut base = Base::default();
    for &op in &two.ops {
        base.apply(op);
    }
    // A stair's top step a storey up, the landing beside it, and the door off that.
    let stairs: Vec<_> = base
        .cubes()
        .filter(|(_, cube)| matches!(cube.shape, Shape::Stair(_)))
        .map(|(at, _)| at.y)
        .collect();
    assert_eq!(stairs.iter().max(), Some(&(STOREY_CUBES - 1)));
    assert!(
        base.walls()
            .any(|(tile, panel)| tile.y == 1 && panel == Panel::Door)
    );
}

// ---- Put up by the rules (from terra_build's construct.rs and put_up.rs, whose tests
// can no longer see the village, D407).

/// The kits' terms as the game's files give their steps: timber two courses a cube,
/// blocks one, roofs four stages.
fn rules() -> Rules {
    let steps = [
        (1, 2),
        (2, 1),
        (3, 2),
        (4, 1),
        (5, 1),
        (0x800, 4),
        (0x801, 4),
        (0x802, 4),
    ];
    Rules {
        kinds: steps
            .iter()
            .map(|&(kind, steps)| (Kind(kind), Terms::new(steps, Bill::default())))
            .collect::<BTreeMap<_, _>>(),
        props: BTreeMap::new(),
        toughness: BTreeMap::new(),
    }
}

#[test]
fn a_town_built_step_by_step_ends_as_the_shortcuts_say_from_the_bottom_up() {
    // D381: each building of a town, put up a step at a time in the hammer's order,
    // ends with every cube built and the same cubes as laid whole; no step puts a part's
    // cube in over one of the same part still to go in below it; a log wall goes in a
    // whole course at a time and a stone one a block at a time; and a roof's stage is
    // never put on before the one under it is on across its layer.
    for building in town(3, 12, KINDS) {
        let whole = Base::from_shortcuts(&building.ops);
        let steps = construction(&whole, &rules(), Vec3::ZERO);
        let name = &building.name;
        let (_, last) = steps.last().unwrap_or_else(|| panic!("{name}: no steps"));
        assert!(finished(last), "{name}: not all built");
        assert!(last.same_cubes(&whole), "{name}: not the same cubes");
        let mut before = Base::default();
        let rules = rules();
        let cx = Context::free(&rules);
        for op in whole.plan_operations() {
            before.act(&op, &cx).unwrap();
        }
        for (step, after) in &steps {
            if let Step::Put {
                part, kind, cubes, ..
            } = step
            {
                let lowest_waiting = before
                    .cubes_of(*part)
                    .filter(|(_, cube)| cube.fill == Fill::Solid && !cube.is_built())
                    .map(|(at, _)| at.y)
                    .min();
                assert_eq!(
                    Some(cubes[0][1]),
                    lowest_waiting,
                    "{name}: a cube of part {} put in over one still to go in",
                    part.0
                );
                if *kind == KINDS.masonry {
                    assert_eq!(cubes.len(), 1, "{name}: stone goes in a block at a time");
                }
                if *kind == KINDS.timber {
                    let layer = before
                        .cubes_of(*part)
                        .filter(|(at, cube)| {
                            at.y == cubes[0][1] && cube.fill == Fill::Solid && !cube.is_built()
                        })
                        .count();
                    assert!(cubes.len() == layer, "{name}: a log course spans its wall");
                }
            }
            before = after.clone();
        }
    }
}

#[test]
fn the_built_share_rises_with_every_step_to_one() {
    // D381: a time-lapse paces a building by how far it is built, so that share must rise
    // with each step and be whole at the end.
    let building = &town(5, 3, KINDS)[0];
    let whole = Base::from_shortcuts(&building.ops);
    let steps = construction(&whole, &rules(), Vec3::ZERO);
    let mut last = 0.0;
    for (_, base) in &steps {
        let share = base.built_share();
        assert!(share > last, "{share} after {last}");
        last = share;
    }
    assert!((last - 1.0).abs() < 1e-6);
}

#[test]
fn every_village_building_town_and_castle_stands_put_up_by_the_operations() {
    // D108: put up by the same checked operations as a player's buildings, every rule
    // holding, the cubes the same as the shortcuts say.
    let mut buildings = village(7, KINDS);
    buildings.extend(town(3, 40, KINDS));
    let mut all: Vec<(String, Vec<BuildOp>)> = buildings
        .into_iter()
        .map(|building| (building.name, building.ops))
        .collect();
    all.push((
        "castle".into(),
        terra_build::castle::castle(KINDS.masonry, KINDS.beams, KINDS.masonry_roof),
    ));
    for (name, ops) in all {
        let shortcuts = Base::from_shortcuts(&ops);
        let built = shortcuts
            .put_up(&rules())
            .unwrap_or_else(|(op, why)| panic!("{name}: {why:?} at {op:?}"));
        assert!(built.same_cubes(&shortcuts), "{name}: not the same cubes");
        assert!(built.floating().is_empty(), "{name}");
        assert_eq!(built.store().planned(), 0, "{name}: not all built");
    }
}
