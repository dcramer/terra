//! The forest floor's tests (`floor.rs`), apart so the floor stays readable.

use super::*;

/// A map with these trees and the forest's own paths, and a place well off them.
fn map_with(stands: &[Stand]) -> (Map, Vec2) {
    let mut canopy = Crowns::default();
    for stand in stands {
        canopy.add(*stand);
    }
    let map = Map {
        path: Path::new(Seed(3)),
        canopy,
        lying: Lying::default(),
        feet: Lying::default(),
        noise: Noise::new(Seed(4)),
    };
    let away = (0..400)
        .map(|k| Vec2::new((k % 20) as f32 * 9.0 - 90.0, (k / 20) as f32 * 9.0 - 90.0))
        .find(|at| map.path.edge_distance(*at) > 11.0)
        .expect("somewhere off the paths");
    (map, away)
}

/// Trees of a kind in a ring round `at`, closing the canopy over it.
fn stand_round(at: Vec2, species: Species) -> Vec<Stand> {
    (0..6)
        .map(|k| {
            let angle = k as f32 / 6.0 * TAU;
            let foot = at + Vec2::new(libm::cosf(angle), libm::sinf(angle)) * 3.5;
            Stand::of(foot, foot, 7.0, species, false, 0.5)
        })
        .collect()
}

#[test]
fn grass_grows_thick_in_the_open_and_thinner_under_a_closed_canopy() {
    let (open, at) = map_with(&[]);
    let lit = open.growing(at);
    assert!(lit.grass > 0.4, "{lit:?}");
    assert!(lit.shade < 0.05, "{lit:?}");
    let (shaded, _) = map_with(&stand_round(at, Species::Oak));
    let growing = shaded.growing(at);
    // Thinner, but still covering most of the floor, as shade grass does (D293).
    assert!(
        growing.grass < lit.grass * 0.85,
        "{growing:?} against {lit:?}"
    );
    assert!(growing.grass > 0.25, "{growing:?}");
    assert!(growing.shade > 0.6, "{growing:?}");
    assert!(growing.litter > 0.5, "leaves under oaks: {growing:?}");
    assert!(growing.needles < 0.1, "{growing:?}");
}

#[test]
fn needles_lie_under_conifers() {
    let (_, at) = map_with(&[]);
    let (map, _) = map_with(&stand_round(at, Species::Spruce));
    let growing = map.growing(at);
    assert!(growing.needles > 0.6, "{growing:?}");
    assert!(growing.litter < 0.4, "{growing:?}");
}

#[test]
fn grass_is_pushed_back_from_a_trunk_by_duff_and_roots() {
    let (_, at) = map_with(&[]);
    let young = Stand::of(at, at, 2.0, Species::Oak, true, 0.4);
    let (map, _) = map_with(&[young]);
    let foot = map.growing(at + Vec2::X * 0.55);
    assert!(foot.grass < 0.1, "{foot:?}");
    assert!(foot.roots > 0.5, "{foot:?}");
}

#[test]
fn a_trunk_stands_near_its_foot_and_not_metres_off() {
    let (_, at) = map_with(&[]);
    let young = Stand::of(at, at, 2.0, Species::Oak, true, 0.4);
    let (map, _) = map_with(&[young]);
    let foot = map.growing(at + Vec2::X * 0.55);
    let off = map.growing(at + Vec2::X * 4.0);
    assert!(foot.stands > 0.6, "a tree well lies at its foot: {foot:?}");
    assert_eq!(off.stands, 0.0, "nothing stands four metres off: {off:?}");
}

#[test]
fn a_shrub_stands_in_a_well_of_its_own_and_moves_nothing() {
    let (mut map, at) = map_with(&[]);
    let bare = map.growing(at + Vec2::X * 0.4);
    let lying = map.here(at + Vec2::X * 0.4).lying;
    map.feet.add(at, FOOT_M);
    let foot = map.growing(at + Vec2::X * 0.4);
    let off = map.growing(at + Vec2::X * 2.0);
    assert_eq!(bare.stands, 0.0, "nothing stands there before: {bare:?}");
    assert!(
        foot.stands > 0.5,
        "a well lies round the stool's foot: {foot:?}"
    );
    assert_eq!(off.stands, 0.0, "none two metres off: {off:?}");
    // What placement reads is the same, so every other plant stands where it did.
    assert_eq!(map.here(at + Vec2::X * 0.4).lying, lying);
    assert!(Under::Hazel.stands_in_a_well() && Under::Spruce.stands_in_a_well());
    assert!(!Under::Fern.stands_in_a_well() && !Under::Snag.stands_in_a_well());
}

#[test]
fn a_big_conifers_well_reaches_farther_than_a_saplings() {
    let (_, at) = map_with(&[]);
    let sapling = Stand::of(at, at, 0.6, Species::Spruce, true, 0.05);
    let big = Stand::of(at, at, 4.0, Species::Spruce, false, 0.5);
    let (small_map, _) = map_with(&[sapling]);
    let (big_map, _) = map_with(&[big]);
    // Round the whole ring, so no one lobe of the well's uneven edge decides it.
    let ring = |map: &Map, r: f32| {
        (0..12)
            .map(|i| {
                let turn = i as f32 * core::f32::consts::TAU / 12.0;
                map.growing(at + Vec2::new(libm::cosf(turn), libm::sinf(turn)) * r)
                    .stands
            })
            .sum::<f32>()
            / 12.0
    };
    assert!(
        ring(&big_map, 1.8) > 0.3,
        "a big spruce's well reaches under its boughs"
    );
    assert!(
        ring(&small_map, 1.8) < 0.05,
        "a sapling's well is a hand or two round it"
    );
}

#[test]
fn rocks_learn_the_snow_cover_when_it_moves_and_not_for_a_trifle() {
    use bevy::ecs::system::RunSystemOnce;
    let mut world = World::new();
    world.init_resource::<Assets<Mossy>>();
    let rock = world.resource_mut::<Assets<Mossy>>().add(Mossy {
        base: StandardMaterial::default(),
        extension: MossExtension {
            colour: Vec4::ONE,
            ground_color: None,
            ground_normal: None,
            snow: Vec4::ZERO,
        },
    });
    let told = |world: &World| {
        world
            .resource::<Assets<Mossy>>()
            .get(&rock)
            .unwrap()
            .extension
            .snow
            .x
    };
    world.insert_resource(SnowCover(0.8));
    world.run_system_once(follow_snow).unwrap();
    assert_eq!(told(&world), 0.8, "snow lying round the eye caps the rocks");
    world.insert_resource(SnowCover(0.81));
    world.run_system_once(follow_snow).unwrap();
    assert_eq!(
        told(&world),
        0.8,
        "a trifle's change leaves the material alone"
    );
    world.insert_resource(SnowCover(0.0));
    world.run_system_once(follow_snow).unwrap();
    assert_eq!(told(&world), 0.0, "bare ground bares the rocks");
}

#[test]
fn a_plants_scan_learns_the_snow_and_stays_a_plant() {
    use bevy::ecs::system::RunSystemOnce;
    let mut world = World::new();
    world.init_resource::<Assets<Mossy>>();
    let fern = world.resource_mut::<Assets<Mossy>>().add(Mossy {
        base: StandardMaterial::default(),
        extension: MossExtension {
            colour: Vec4::ZERO,
            ground_color: None,
            ground_normal: None,
            snow: Vec4::new(0.0, 1.0, 0.0, 0.0),
        },
    });
    world.insert_resource(SnowCover(0.6));
    world.run_system_once(follow_snow).unwrap();
    let snow = world
        .resource::<Assets<Mossy>>()
        .get(&fern)
        .unwrap()
        .extension
        .snow;
    assert_eq!(snow.x, 0.6, "snow lying round the eye lies on the fern");
    assert_eq!(
        snow.y, 1.0,
        "and it withers as a plant, not mossed as a rock"
    );
}

/// Places of the map's ground in a grid, each with how far it is from a path's frayed edge,
/// as the placement rules read it (`Map::here`), not the path's own line.
fn grid_by_path(map: &Map) -> Vec<(Vec2, f32)> {
    (0..3600)
        .map(|k| Vec2::new((k % 60) as f32 * 1.5 - 45.0, (k / 60) as f32 * 1.5 - 45.0))
        .map(|at| (at, map.here(at).path))
        .collect()
}

#[test]
fn no_shrub_stands_in_a_paths_line_and_low_growth_still_lines_it() {
    let (map, _) = map_with(&[]);
    let grid = grid_by_path(&map);
    let beside: Vec<_> = grid
        .iter()
        .filter(|(_, d)| (0.0..2.0).contains(d))
        .collect();
    assert!(beside.len() > 20, "the grid crosses a path's verge");
    for (at, _) in &beside {
        let here = map.here(*at);
        for shrub in [Under::Hazel, Under::Coppice, Under::Bramble, Under::Sapling] {
            assert!(
                !shrub.keeps_path_line(here.path),
                "{shrub:?} at {at} in the path's line"
            );
        }
        for low in [Under::Bracken, Under::Verge, Under::Fern, Under::Flowers] {
            assert!(
                low.keeps_path_line(here.path),
                "{low:?} kept off the verge at {at}"
            );
        }
    }
    let off = grid
        .iter()
        .find(|(_, d)| *d > PATH_LINE_M + 1.0)
        .expect("ground off the path");
    assert!(Under::Hazel.keeps_path_line(map.here(off.0).path));
}

#[test]
fn verge_bracken_grows_on_a_sunlit_path_side_not_in_the_deep_wood() {
    let (open, away) = map_with(&[]);
    let grid = grid_by_path(&open);
    // The verge: the metre or so beside the path's edge, full at it and gone by 2.2 m.
    let verge = grid
        .iter()
        .find(|(_, d)| (0.5..1.1).contains(d))
        .expect("a path's verge")
        .0;
    let beyond = grid
        .iter()
        .find(|(_, d)| (3.0..5.0).contains(d))
        .expect("lit ground past the verge")
        .0;
    let (shaded, _) = map_with(&stand_round(away, Species::Spruce));
    let lit = Under::Verge.likes(&open.here(verge));
    let deep = Under::Verge.likes(&shaded.here(away));
    assert!(lit > 0.5, "a sunlit verge takes verge bracken: {lit}");
    assert!(deep < 0.05, "the deep wood off the path takes none: {deep}");
    let (under_spruce, _) = map_with(&stand_round(verge, Species::Spruce));
    let dim = Under::Verge.likes(&under_spruce.here(verge));
    assert!(
        dim < 0.5 * lit,
        "a verge under spruce takes less than a lit one: {dim} vs {lit}"
    );
    let past = Under::Verge.likes(&open.here(beyond));
    assert!(past < 0.05, "lit ground past the verge takes none: {past}");
}

#[test]
fn verge_tries_start_just_outside_a_paths_edge() {
    // Tried over the whole floor, a path's verge, about a hundredth of it, took 31 plants of
    // 2600 tries (forest fix r2); tried beside the paths, every start is on a verge.
    let path = Path::new(Seed(3));
    let mut dice = Seed(5).rng();
    let starts: Vec<f32> = (0..500)
        .map(|_| path.edge_distance(path.beside(&mut dice, VERGE_OUT_M)))
        .collect();
    let outside = starts
        .iter()
        .filter(|d| (-0.01..=VERGE_OUT_M + 0.05).contains(*d))
        .count();
    // Only where two paths cross is a start on the other's way.
    assert!(outside >= 490, "{outside} of 500 just outside the edge");
    assert!(
        starts.iter().all(|d| *d <= VERGE_OUT_M + 0.05),
        "none further out"
    );
    let far = starts.iter().filter(|d| **d > VERGE_OUT_M * 0.5).count();
    assert!(far > 150, "spread across the verge, not on the edge: {far}");
}
