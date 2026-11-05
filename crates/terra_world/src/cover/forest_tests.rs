//! The forest floor's map's tests (`forest.rs`), apart so the map stays readable.

use super::*;
use core::f32::consts::TAU;

/// A floor with these trees and no paths, and a place on it.
fn map_with(stands: &[Stand]) -> (Floor, Vec2) {
    let mut canopy = Crowns::default();
    for stand in stands {
        canopy.add(*stand);
    }
    (Floor::new(Wild, canopy, Seed(4)), Vec2::new(31.0, -17.0))
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
    let ring = |map: &Floor, r: f32| {
        (0..12)
            .map(|i| {
                let turn = i as f32 * TAU / 12.0;
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
fn a_wild_wood_has_no_paths_and_nothing_lines_one() {
    let (map, at) = map_with(&stand_round(Vec2::ZERO, Species::Pine));
    let mut dice = Seed(9).rng();
    assert!(Wild.beside(&mut dice, 2.0).is_none());
    for k in 0..200 {
        let p = at + Vec2::new((k % 20) as f32 * 1.7, (k / 20) as f32 * 2.3);
        assert!(!map.growing(p).path, "no path at {p}");
    }
}

#[test]
fn a_woods_floor_lies_among_its_trees_and_gives_way_to_the_land_past_them() {
    let (map, _) = map_with(&stand_round(Vec2::ZERO, Species::Spruce));
    // Among the trees the wood's floor, and in a glade well past them the land's own.
    assert_eq!(wooded(&map.here(Vec2::ZERO)), 1.0);
    assert_eq!(wooded(&map.here(Vec2::new(30.0, 0.0))), 0.0);
    // Between, the one blends into the other, never jumping from one to the other.
    let mut last = 1.0;
    for k in 0..60 {
        let w = wooded(&map.here(Vec2::new(4.0 + k as f32 * 0.25, 0.0)));
        assert!(
            w <= last + 1e-6 && last - w < 0.35,
            "{last} then {w} at step {k}"
        );
        last = w;
    }
    // A blend takes each of what grows the share it is given.
    let heath = Growing {
        grass: 0.8,
        heath: 1.0,
        lichen: 0.6,
        ..default()
    };
    let floor = map.growing(Vec2::ZERO);
    let half = mix(&heath, &floor, 0.5);
    assert!(
        (half.needles - floor.needles * 0.5).abs() < 1e-6,
        "{half:?}"
    );
    assert!((half.lichen - 0.3).abs() < 1e-6, "{half:?}");
    assert_eq!(mix(&heath, &floor, 0.0), heath);
    assert_eq!(mix(&heath, &floor, 1.0), floor);
}

#[test]
fn what_lies_is_taken_off_exactly_one_circle_at_a_time() {
    let (mut floor, at) = map_with(&[]);
    let open = floor.here(at).lying;
    floor.lying.add(at, 0.5);
    floor.lying.add(at, 0.5);
    floor.lying.add(at + Vec2::X, 0.3);
    floor.lying.remove(at, 0.5);
    assert!(
        floor.here(at).lying < 0.0,
        "one of the two still lies there"
    );
    floor.lying.remove(at, 0.4);
    floor.lying.remove(at, 0.5);
    floor.lying.remove(at + Vec2::X, 0.3);
    assert_eq!(
        floor.here(at).lying,
        open,
        "all three gone, none other taken"
    );
}

#[test]
fn a_stump_is_taken_off_and_a_tree_on_the_same_foot_stays() {
    let foot = Vec2::new(31.0, -17.0);
    let tree = Stand::of(foot, foot, 7.0, Species::Oak, false, 0.5);
    let (mut floor, _) = map_with(&[tree]);
    floor.canopy.add(Stand::stump(foot, 0.4));
    floor.canopy.add(Stand::stump(foot + Vec2::X * 3.0, 0.4));
    assert_eq!(floor.canopy.len(), 3);
    floor.canopy.remove_stump(foot);
    assert_eq!(floor.canopy.len(), 2);
    floor.canopy.remove_stump(foot);
    assert_eq!(floor.canopy.len(), 2, "the tree is no stump");
    floor.canopy.remove_stump(foot + Vec2::X * 3.0);
    assert_eq!(floor.canopy.len(), 1);
    assert!(floor.canopy.among(foot), "the tree still stands");
}
