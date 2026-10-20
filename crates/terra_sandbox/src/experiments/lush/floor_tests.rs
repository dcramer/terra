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
