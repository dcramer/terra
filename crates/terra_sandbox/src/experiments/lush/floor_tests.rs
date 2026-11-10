//! The forest floor's tests (`floor.rs`), apart so the floor stays readable.

use super::super::Path;
use super::super::species::Species;
use super::*;

/// The forest walk's floor, through which its paths run.
type Map = Floor<Path>;

/// A map with these trees and the forest's own paths, and a place well off them.
fn map_with(stands: &[Stand]) -> (Map, Vec2) {
    let mut canopy = Crowns::default();
    for stand in stands {
        canopy.add(*stand);
    }
    let map = Floor::new(Path::new(Seed(3)), canopy, Seed(4));
    let away = (0..400)
        .map(|k| Vec2::new((k % 20) as f32 * 9.0 - 90.0, (k / 20) as f32 * 9.0 - 90.0))
        .find(|at| map.ways.edge_distance(*at) > 11.0)
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
fn the_verge_likes_no_place_more_for_the_sun_missing_it() {
    // What lets the floor skip the sun's test where full sun would not keep a plant
    // (`Lay::likes_at_most`): a place the sun misses is liked no more than if it reached it.
    let (open, _) = map_with(&[]);
    let mut asked = 0;
    for (at, _) in grid_by_path(&open) {
        let mut here = open.here(at);
        here.sun = 1.0;
        let lit = Under::Verge.likes(&here);
        here.sun = 0.0;
        let missed = Under::Verge.likes(&here);
        assert!(
            missed <= lit,
            "at {at}: {missed} missed by the sun, {lit} lit"
        );
        asked += (lit > 0.0) as u32;
    }
    assert!(asked > 10, "the grid crosses the verge: {asked} places");
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

#[test]
fn a_squares_tries_are_as_thick_as_the_rings() {
    // The woods' ring round their middle, and squares of 16 m streamed past it: each square's
    // tries, its share of the ring's thickness with the fraction left to the dice, average to
    // the ring's tries a square metre, for the rocks' many tries and the undergrowth's few.
    let ring = Area {
        middle: Vec2::ZERO,
        from_m: 14.0,
        to_m: 220.0,
        inner_m: 14.0,
        outer_m: 220.0,
        among_trees: true,
        off_water: true,
        square: None,
        skip_within_m: 0.0,
    };
    let ground = PI * (220.0f32 * 220.0 - 14.0 * 14.0);
    let mut dice = Seed(9).rng();
    let squares = 20_000;
    for (yard, undergrowth) in [(1400, false), (70, false), (20, true)] {
        let thick = ring.tries(yard, undergrowth, &mut dice) as f32 / ground;
        let tried: u32 = (0..squares)
            .map(|k| {
                let low = Vec2::new((k % 100) as f32, (k / 100) as f32) * 16.0;
                let square = Area {
                    square: Some((low, 16.0)),
                    ..ring
                };
                square.tries(yard, undergrowth, &mut dice)
            })
            .sum();
        let square_thick = tried as f32 / (squares as f32 * 256.0);
        assert!(
            (square_thick / thick - 1.0).abs() < 0.04,
            "{yard} tries: {square_thick} a square metre in squares, {thick} in the ring"
        );
    }
}
