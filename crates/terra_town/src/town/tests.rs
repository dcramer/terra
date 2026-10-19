use super::*;
use terra_build::Kind;

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

fn flat(_: Vec2) -> f32 {
    0.0
}

/// A hillside rising 6 in 100 toward the north-east.
fn hillside(at: Vec2) -> f32 {
    at.dot(Vec2::new(1.0, -1.0).normalize()) * 0.06
}

fn rolling(at: Vec2) -> f32 {
    4.0 * libm::sinf(at.x / 45.0) * libm::cosf(at.y / 60.0) + at.y * 0.03
}

const GROUNDS: [fn(Vec2) -> f32; 3] = [flat, hillside, rolling];

fn towns() -> Vec<Town> {
    let mut all = Vec::new();
    for seed in [1, 2, 3, 7, 11] {
        for ground in GROUNDS {
            all.push(town(seed, KINDS, &ground));
        }
    }
    all
}

#[test] // Slow: minutes.
fn the_same_seed_and_ground_make_the_same_town_and_others_differ() {
    assert_eq!(town(5, KINDS, &hillside), town(5, KINDS, &hillside));
    let a = town(5, KINDS, &hillside);
    let b = town(6, KINDS, &hillside);
    assert_ne!(a.buildings, b.buildings);
    let shapes: BTreeSet<_> = (0..12)
        .map(|seed| format!("{:?}", town(seed, KINDS, &flat).square.shape))
        .collect();
    assert_eq!(shapes.len(), 3, "every shape of square: {shapes:?}");
}

#[test]
fn a_house_covers_its_walls_and_floor_and_a_yard_before_a_u_stays_open() {
    let house = village::put_up(
        1,
        (Plan::Rectangle, Scheme::Timber, Roof::Gable, 12),
        [0.0, 0.0],
        0.0,
        KINDS,
    );
    let rects = ground_rects(&house);
    assert_eq!(rects.len(), 1, "a rectangle is one: {rects:?}");
    let (low, high) = rects[0];
    // Twelve cubes across its front and ten deep, behind its door at the origin.
    assert_eq!(high - low, Vec2::new(6.0, 5.0));
    assert_eq!(high.y, 0.0);
    let u = village::put_up(
        1,
        (Plan::U, Scheme::Masonry, Roof::Gable, 18),
        [0.0, 0.0],
        0.0,
        KINDS,
    );
    let rects = ground_rects(&u);
    let covered = |point: Vec2| {
        rects
            .iter()
            .any(|(l, h)| point.cmpge(*l).all() && point.cmplt(*h).all())
    };
    // Its wings come forward either side of the door; between them is open yard.
    assert!(covered(Vec2::new(-4.2, 2.0)) && covered(Vec2::new(4.2, 2.0)));
    assert!(
        !covered(Vec2::new(0.0, 2.0)),
        "the yard between a U's wings is open"
    );
    assert!(covered(Vec2::new(0.0, -2.0)), "its floor is covered");
}

#[test] // Slow: minutes.
fn a_town_has_its_count_of_houses_a_market_a_forge_and_gates() {
    for town in towns() {
        let shape = town.square.shape;
        assert!(
            (BUILDINGS.0..=BUILDINGS.1).contains(&town.homes.len()),
            "{shape:?}: {} homes",
            town.homes.len()
        );
        let count = |kind| town.things.iter().filter(|t| t.kind == kind).count();
        assert_eq!(count(ThingKind::Well), 1);
        assert!(
            count(ThingKind::Stall) >= 4,
            "{shape:?}: {} stalls",
            count(ThingKind::Stall)
        );
        assert_eq!(count(ThingKind::Forge), 1, "{shape:?}: a smithy");
        assert_eq!(count(ThingKind::Gate), town.streets.len());
        assert_eq!(count(ThingKind::Door), town.homes.len());
        assert!(
            count(ThingKind::Pen) + count(ThingKind::Yard) >= 4,
            "{shape:?}: yards with animals"
        );
    }
}

#[test] // Slow: minutes.
fn nothing_built_stands_on_another_or_in_a_street_or_on_the_square() {
    for town in towns() {
        let quads: Vec<(usize, &[Vec2; 4])> = town
            .footprints
            .iter()
            .enumerate()
            .flat_map(|(b, quads)| quads.iter().map(move |q| (b, q)))
            .collect();
        for (i, &(a, qa)) in quads.iter().enumerate() {
            for &(b, qb) in &quads[i + 1..] {
                if a != b {
                    assert!(
                        polygons(qa, qb) > 0.05,
                        "{} and {} overlap",
                        town.buildings[a].name,
                        town.buildings[b].name
                    );
                }
            }
            for street in &town.streets {
                for pair in street.points.windows(2) {
                    assert!(
                        segment_polygon(pair[0], pair[1], qa) >= street.wide * 0.5,
                        "{} stands in a street",
                        town.buildings[a].name
                    );
                }
            }
            assert!(
                polygons(qa, &town.square.outline) > 0.5,
                "{} on the square",
                town.buildings[a].name
            );
        }
    }
}

#[test] // Slow: minutes.
fn streets_follow_the_contour_rather_than_climb() {
    // Straight up the hillside a street would climb 6 in 100. The main street runs along
    // it; a side street leaves the square up or down it, then turns along it.
    let climb = |points: &[Vec2]| {
        points
            .windows(2)
            .map(|pair| (hillside(pair[1]) - hillside(pair[0])).abs() / pair[0].distance(pair[1]))
            .sum::<f32>()
            / (points.len() - 1) as f32
    };
    for seed in 0..8 {
        let town = town(seed, KINDS, &hillside);
        for (index, street) in town.streets.iter().enumerate() {
            let points = &street.points;
            let (main, rise) = if index < 2 {
                (true, climb(&points[3..]))
            } else {
                (false, climb(&points[points.len() * 3 / 4..]))
            };
            assert!(
                rise < 0.025,
                "seed {seed}: {} street climbs {rise:.3} a metre",
                if main { "a main" } else { "a side" }
            );
        }
    }
}

#[test] // Slow: minutes.
fn every_door_opens_onto_a_street_or_the_square() {
    for town in towns() {
        for door in town.things.iter().filter(|t| t.kind == ThingKind::Door) {
            let before = door.place(Vec2::new(0.0, 1.0));
            let to_street = town
                .streets
                .iter()
                .map(|street| {
                    street
                        .points
                        .windows(2)
                        .map(|pair| point_segment(before, pair[0], pair[1]) - street.wide * 0.5)
                        .fold(f32::MAX, f32::min)
                })
                .fold(f32::MAX, f32::min);
            let outline = &town.square.outline;
            let to_square = if inside(before, outline) {
                0.0
            } else {
                (0..outline.len())
                    .map(|i| point_segment(before, outline[i], outline[(i + 1) % outline.len()]))
                    .fold(f32::MAX, f32::min)
            };
            // Its front stands back from the way by at most its wings' and a step's depth.
            assert!(
                to_street.min(to_square) < 5.5,
                "a door {:.1} m from any way",
                to_street.min(to_square)
            );
        }
    }
}

#[test] // Slow: minutes.
fn the_path_graph_is_one_and_every_walk_on_it_is_clear() {
    for town in towns() {
        let parts = town.paths.parts();
        let shape = town.square.shape;
        let main = parts[0];
        for thing in &town.things {
            assert_eq!(
                parts[thing.node as usize], main,
                "{shape:?}: the {:?} at {:?} is cut off",
                thing.kind, thing.at
            );
        }
        for &[a, b] in &town.paths.edges {
            let (pa, pb) = (
                town.paths.nodes[a as usize].0,
                town.paths.nodes[b as usize].0,
            );
            for (index, quads) in town.footprints.iter().enumerate() {
                for quad in quads {
                    assert!(
                        segment_polygon(pa, pb, quad) >= CLEAR_M - 1e-3,
                        "{shape:?}: a walk from {pa:?} to {pb:?} goes through {}",
                        town.buildings[index].name
                    );
                }
            }
        }
        // Most homes with a yard have a lane to it.
        let lanes = town
            .paths
            .nodes
            .iter()
            .filter(|(_, on)| matches!(on, On::Lane(_)))
            .count();
        assert!(
            lanes / 3 * 3 >= town.yards.len() * 2,
            "{shape:?}: {} yards, {lanes} lane nodes",
            town.yards.len()
        );
    }
}

#[test] // Slow: minutes.
fn builders_sites_stand_outside_every_building_facing_their_own() {
    // D381: a settlement being built has places to work round each building, out of every
    // building's ground and every thing's, each facing the wall it is by, so a builder
    // there hammers at their own building and not a neighbour's or the air.
    for mut town in towns() {
        let before = town.things.len();
        let laid = town.add_sites();
        assert!(
            laid >= town.buildings.len(),
            "{laid} sites for {}",
            town.buildings.len()
        );
        assert_eq!(town.things.len(), before + laid, "laid after the rest");
        let mut sited = std::collections::BTreeSet::new();
        for site in &town.things[before..] {
            assert_eq!(site.kind, ThingKind::Site);
            let own = site.building.expect("a site's building");
            sited.insert(own);
            assert!(
                town.footprints
                    .iter()
                    .flatten()
                    .all(|quad| !inside(site.at, quad)),
                "a site in a building"
            );
            // A little ahead of it is its own building's ground.
            let ahead = site.at + site.front() * 1.3;
            assert!(
                town.footprints[own].iter().any(|quad| inside(ahead, quad)),
                "a site at {:?} facing {:?} of {} faces away from it: {:?}",
                site.at,
                site.front(),
                town.buildings[own].name,
                town.footprints[own]
            );
            assert!((site.node as usize) < town.paths.nodes.len());
        }
        for &home in &town.homes {
            assert!(
                sited.contains(&home),
                "{} has no site",
                town.buildings[home].name
            );
        }
    }
}

#[test]
fn a_walled_town_closes_its_houses_and_castle_in_with_a_gate_over_its_streets_and_its_castle_last()
{
    let size = Size {
        buildings: (80, 90),
        streets: 1.6,
        walled: true,
    };
    for ground in [flat, rolling] {
        let town = town_sized(2, KINDS, &ground, &size);
        let (houses, works) = town.buildings.split_at(town.works);
        assert!(town.homes.len() >= 70, "{} homes", town.homes.len());
        assert_eq!(works.last().map(|b| b.name.as_str()), Some("the castle"));
        let gates = works
            .iter()
            .filter(|b| b.name.contains("gatehouse"))
            .count();
        let lengths = works.iter().filter(|b| b.name.contains("length")).count();
        eprintln!(
            "walled town: {} homes, {} buildings, {} works: {gates} gates, {lengths} lengths",
            town.homes.len(),
            houses.len(),
            works.len()
        );
        assert_eq!(gates, town.streets.len(), "a gate over each street");
        // Every house, yard and the square stands within the wall's line, and the castle too.
        let mut within: Vec<(&str, Vec2)> = Vec::new();
        for (b, quads) in town.footprints.iter().enumerate() {
            if b >= town.works && b + 1 < town.buildings.len() {
                continue;
            }
            let name = town.buildings[b].name.as_str();
            within.extend(quads.iter().flatten().map(|&p| (name, p)));
        }
        within.extend(
            town.yards
                .iter()
                .flat_map(|(_, y)| y.iter().map(|&p| ("a yard", p))),
        );
        for (name, p) in within {
            assert!(
                in_polygon(p, &town.wall),
                "{name} at {p} stands outside the wall"
            );
        }
        // And the castle is of the town, not an annex walled on: the way from its gate to
        // the square runs inside the wall all along.
        let gate = Vec2::from(works.last().expect("a castle").at);
        let square =
            town.square.outline.iter().copied().sum::<Vec2>() / town.square.outline.len() as f32;
        for k in 0..=50 {
            let p = gate.lerp(square, k as f32 / 50.0);
            assert!(
                in_polygon(p, &town.wall),
                "the way from the castle leaves the wall at {p}"
            );
        }
        // No work stands on a house, a yard or the square.
        for (w, work) in town.footprints[town.works..].iter().enumerate() {
            for (h, house) in town.footprints[..town.works].iter().enumerate() {
                for (a, b) in work.iter().flat_map(|p| house.iter().map(move |q| (p, q))) {
                    assert!(
                        polygons(a, b) > 0.0,
                        "{} at {:?} stands on {} at {:?}",
                        works[w].name,
                        works[w].at,
                        houses[h].name,
                        houses[h].at
                    );
                }
            }
            for (_, yard) in &town.yards {
                assert!(work.iter().all(|p| polygons(p, yard) > 0.0));
            }
            assert!(work.iter().all(|p| polygons(p, &town.square.outline) > 0.0));
        }
    }
}

/// Whether a point is inside a polygon, convex or not (by the crossings of a ray east).
fn in_polygon(at: Vec2, polygon: &[Vec2]) -> bool {
    let mut odd = false;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        if (a.y > at.y) != (b.y > at.y) && at.x < a.x + (at.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            odd = !odd;
        }
    }
    odd
}
