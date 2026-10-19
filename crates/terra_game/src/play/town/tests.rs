#[test]
fn a_held_clock_leaves_the_day_going_at_its_pace() {
    // Running: the clock's hour.
    assert_eq!(super::lived_hours(8.6, 8.5, 8.5, 1.0), 8.6);
    // Held at 8.5: on by the day's pace, a minute of play an hour's 2 minutes of the day.
    let on = super::lived_hours(8.5, 8.5, 8.5, 60.0);
    assert!(
        (on - (8.5 + 60.0 * 24.0 / DAY_SECONDS as f64)).abs() < 1e-9,
        "{on}"
    );
    assert!(on > 8.5);
    // Before anything is seen, the clock's.
    assert_eq!(super::lived_hours(8.5, f64::NAN, f64::NAN, 1.0), 8.5);
}

use super::*;
use terra_town::village::{Kinds, Plan, Scheme};

#[test]
fn a_meadows_drifts_vary_smoothly_over_its_whole_range() {
    // D381: the mix is drifts, not speckle: neighbours a few centimetres apart differ
    // little, and across the meadow both ends of the range are met.
    let (mut low, mut high) = (1.0f32, 0.0f32);
    for i in 0..400 {
        let p = Vec2::new(i as f32 * 0.37, i as f32 * 0.21);
        let v = patch(p);
        assert!((0.0..=1.0).contains(&v));
        assert!((patch(p + Vec2::splat(0.01)) - v).abs() < 0.05);
        low = low.min(v);
        high = high.max(v);
    }
    assert!(low < 0.2 && high > 0.8, "{low} {high}");
}

/// D381: a plot and the square are trodden over a few seconds as the settlement rises,
/// the meadow giving way to earth by how far each is worn, not all at once.
#[test]
fn a_plot_and_the_square_are_trodden_by_how_far_they_are_worn() {
    use terra_world::cover::CoverSource;
    let planet = Planet::new(terra_core::PlanetSpec {
        seed: terra_gen::Seed(1),
        radius_m: 2000.0,
    });
    let middle = planet.start();
    let up = middle.normalize();
    let east = up.any_orthonormal_vector();
    let south = east.cross(up);
    let plot = [
        Vec2::new(10.0, 10.0),
        Vec2::new(16.0, 10.0),
        Vec2::new(16.0, 16.0),
        Vec2::new(10.0, 16.0),
    ];
    let square = vec![
        Vec2::new(-8.0, -8.0),
        Vec2::new(8.0, -8.0),
        Vec2::new(8.0, 8.0),
        Vec2::new(-8.0, 8.0),
    ];
    let cover = |plot_worn: f32, square_worn: f32| TownCover {
        middle,
        east,
        south,
        at: Vec2::ZERO,
        square: square.clone(),
        streets: Vec::new(),
        yards: Vec::new(),
        plots: vec![plot],
        doors: Vec::new(),
        plots_worn: vec![plot_worn],
        square_worn,
        meadow_m: 0.0,
        reach_m: COVER_REACH_M,
        paved: false,
        gardens: Default::default(),
    };
    let at = |cover: &TownCover, p: Vec2| {
        let ground = planet.ground_at(middle + east * p.x + south * p.y);
        cover.growing(&planet, ground, p)
    };
    let (begun, worn) = (cover(0.2, 0.2), cover(1.0, 1.0));
    for p in [Vec2::new(13.0, 13.0), Vec2::ZERO] {
        let (a, b) = (at(&begun, p), at(&worn, p));
        assert!(b.trodden > a.trodden, "{p}: {} {}", a.trodden, b.trodden);
        assert!(b.grass <= a.grass, "{p}: {} {}", a.grass, b.grass);
    }
    assert!(at(&worn, Vec2::ZERO).path && !at(&begun, Vec2::ZERO).path);
}

#[test]
fn a_hearth_burns_for_the_meals_and_is_banked_at_night() {
    for luck in [0.0, 0.5, 1.0] {
        let at = |hour| hearth_burns(hour, luck);
        assert!(
            at(6.5) > at(15.0) && at(19.0) > at(15.0),
            "cooking is the most"
        );
        assert!(at(3.0) < 0.3 && at(3.0) > 0.0, "banked, not out, at night");
    }
}

#[test]
fn a_house_smokes_from_over_its_ridge() {
    let kinds = Kinds {
        timber: terra_build::Kind(1),
        masonry: terra_build::Kind(2),
        beams: terra_build::Kind(3),
        cob: terra_build::Kind(4),
        framed: terra_build::Kind(5),
        works: terra_build::Kind(2),
        timber_roof: terra_build::Kind(0x800),
        masonry_roof: terra_build::Kind(0x801),
        beams_roof: terra_build::Kind(0x802),
    };
    let town = terra_town::town::town(3, kinds, &|_| 0.0);
    let house = town
        .buildings
        .iter()
        .find(|b| b.plan == Plan::Rectangle && b.scheme != Scheme::Masonry)
        .expect("a rectangular house");
    let base = Base::from_shortcuts(&house.ops);
    let top = base
        .cubes()
        .filter(|(_, c)| c.kind.is_roof())
        .map(|(at, _)| (at.y + 1) as f32 * CELL_M)
        .fold(0.0, f32::max);
    let ridge = ridge(&base).expect("a roof");
    assert!(ridge.y > top, "{} under the roof's top {top}", ridge.y);
    // Within the house's own ground, not out over its door.
    assert!(ridge.z < 0.0);
}

#[test]
fn most_of_a_towns_buildings_have_a_ridge_along_their_top_for_birds() {
    let kinds = Kinds {
        timber: terra_build::Kind(1),
        masonry: terra_build::Kind(2),
        beams: terra_build::Kind(3),
        cob: terra_build::Kind(4),
        framed: terra_build::Kind(5),
        works: terra_build::Kind(2),
        timber_roof: terra_build::Kind(0x800),
        masonry_roof: terra_build::Kind(0x801),
        beams_roof: terra_build::Kind(0x802),
    };
    let town = terra_town::town::town(3, kinds, &|_| 0.0);
    let mut ridged = 0;
    for building in &town.buildings {
        let base = Base::from_shortcuts(&building.ops);
        let Some((a, b)) = ridge_line(&base) else {
            continue;
        };
        ridged += 1;
        let top = base
            .cubes()
            .filter(|(_, c)| c.kind.is_roof())
            .map(|(at, _)| (at.y + 1) as f32 * CELL_M)
            .fold(0.0, f32::max);
        // On the roof's top, a few metres long, level.
        assert!(
            (a.y - top).abs() < 0.3 && (b.y - top).abs() < 0.3,
            "{a} {b} {top}"
        );
        assert!(a.distance(b) > 1.5, "{}", a.distance(b));
    }
    assert!(
        ridged * 2 >= town.buildings.len(),
        "{ridged} of {}",
        town.buildings.len()
    );
}

#[test] // Slow: a minute or two (the mountain world's land, the site's search, the town).
fn the_settlements_pinned_site_still_lays_its_walled_town() {
    // The settlement time-lapse's town, laid as the sandbox lays it at its pinned place
    // (`--experiment town --site settlement@2.41,2.51`, D393, D381): a change to the land or
    // the town's rules that leaves it without its wall or castle, or puts a house in the
    // lake, fails here rather than in a film (terra_land's
    // `the_settlements_pinned_site_is_still_by_its_lake_under_its_range` holds the place).
    use terra_world::start::Found;
    let planet = Planet::new(terra_core::PlanetSpec {
        seed: terra_gen::Seed(1),
        radius_m: 30_000.0,
    });
    let found = terra_world::start::find_site(&planet, "settlement@2.41,2.51");
    let Found::Settlement(site) = found else {
        panic!("not a settlement")
    };
    let start = found.start(&planet);
    let at = start.local(planet.surface(site.toward));
    let wanted = Wanted {
        seed: SEED,
        near: Vec2::new(at.x, at.z),
        within: AT_THE_SITE_M,
        arrive: true,
        lived_in: true,
    };
    let (lapse, _) = super::super::rising::Timelapse::read("settlement.ron").unwrap();
    let size = lapse.town.expect("the settlement's size").size();
    assert!(size.walled);
    let kinds = Kinds {
        timber: terra_build::Kind(1),
        masonry: terra_build::Kind(2),
        beams: terra_build::Kind(3),
        cob: terra_build::Kind(4),
        framed: terra_build::Kind(5),
        works: terra_build::Kind(2),
        timber_roof: terra_build::Kind(0x800),
        masonry_roof: terra_build::Kind(0x801),
        beams_roof: terra_build::Kind(0x802),
    };
    let (middle, east, town) = laid_out(&planet, &start, &wanted, kinds, &size);
    let (houses, works) = town.buildings.split_at(town.works);
    let gates = works
        .iter()
        .filter(|b| b.name.contains("gatehouse"))
        .count();
    let towers = works.iter().filter(|b| b.name.contains("tower")).count();
    eprintln!(
        "settlement: {} buildings, {} homes, {} works ({gates} gates, {towers} towers), a wall of {} corners",
        town.buildings.len(),
        town.homes.len(),
        works.len(),
        town.wall.len()
    );
    // As big as its time-lapse asks, walled round with towers and gates, and its castle
    // within it, last. (Not yet a gate over every street: here the north street runs into a
    // notch the wall's line makes round its last houses, a 3 m side too short for a gate.)
    assert!(
        houses.len() >= size.buildings.0,
        "{} buildings",
        houses.len()
    );
    assert!(town.wall.len() >= 3, "no wall");
    assert!(towers >= 3, "{towers} towers");
    assert!(gates >= 1, "no gate");
    assert_eq!(works.last().map(|b| b.name.as_str()), Some("the castle"));
    // Nothing built stands in the water: every corner of every building's ground over dry
    // land, as the world draws its lakes and the sea.
    let land = terra_land::Land::new(
        planet.voxels().field().clone(),
        terra_land::start_climate().unwrap(),
    );
    let waters = terra_land::water::Waters::round(&land, site.toward, 1200.0, 10.0);
    let up = middle.normalize();
    let south = east.cross(up);
    for (b, quads) in town.footprints.iter().enumerate() {
        for &p in quads.iter().flatten() {
            let ground = planet.ground_at(middle + east * p.x + south * p.y);
            assert!(
                waters.lake_level(ground.normalize()).is_none()
                    && ground.length() > planet.sea_radius_m(),
                "{} at {p} stands in the water",
                town.buildings[b].name
            );
        }
    }
}
