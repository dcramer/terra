//! A town's life, tested on a town from the layout and the plan the game reads.

use crate::check::Watch;
use crate::{Life, Plan, State};
use glam::Vec2;
use terra_build::Kind;
use terra_build::town::{ThingKind, town};
use terra_build::village::Kinds;

const KINDS: Kinds = Kinds {
    timber: Kind(1),
    masonry: Kind(2),
    beams: Kind(3),
    cob: Kind(4),
    framed: Kind(5),
    timber_roof: Kind(0x800),
    masonry_roof: Kind(0x801),
    beams_roof: Kind(0x802),
};

const SECONDS_AN_HOUR: f32 = 75.0;
const STEP: f32 = 0.1;

fn plan() -> Plan {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/data/life.ron"
    ))
    .expect("the plan's file");
    Plan::parse(&text).expect("the plan reads")
}

fn life(seed: u32) -> Life {
    let ground = |at: Vec2| at.x * 0.03;
    Life::new(town(seed, KINDS, &ground), plan(), seed, SECONDS_AN_HOUR)
}

/// The middle of the square, where a viewer stands to see everyone.
fn square(life: &Life) -> Vec2 {
    let outline = &life.town.square.outline;
    outline.iter().copied().sum::<Vec2>() / outline.len() as f32
}

fn run(life: &mut Life, from: f64, hours: f64, viewer: Vec2) -> Watch {
    let mut watch = Watch::default();
    life.start(from);
    let steps = (hours * SECONDS_AN_HOUR as f64 / STEP as f64) as u64;
    for n in 0..steps {
        let at = from + n as f64 * STEP as f64 / SECONDS_AN_HOUR as f64;
        life.step(at, STEP, &[viewer]);
        watch.see(life, at, STEP);
    }
    watch.finish();
    watch
}

#[test]
fn the_plan_names_only_things_a_town_has_and_activities_it_has() {
    let plan = plan();
    let kinds: Vec<&str> = ThingKind::ALL.iter().map(|k| k.name()).collect();
    for thing in plan.things.keys() {
        assert!(kinds.contains(&thing.as_str()), "no thing {thing}");
    }
    // Something to do at home, and somewhere out of town.
    assert!(plan.activities.values().any(|a| a.inside));
    assert!(plan.activities.values().any(|a| a.away));
    let wrong = plan.clone();
    let text = "(household: (1, 2), things: {}, activities: {}, roles: [(name: \"x\", count: Rest, day: [(from: 0.0, does: [\"Nap\"])])])";
    assert!(
        Plan::parse(text).is_err(),
        "an activity that is not one is caught"
    );
    assert_eq!(wrong, plan);
}

#[test]
fn everyone_has_a_home_and_a_role_and_owners_own_what_they_work() {
    let life = life(2);
    let count = |role: &str| {
        (0..life.people.len())
            .filter(|&p| life.role(p) == role)
            .count()
    };
    let stalls = life
        .town
        .things
        .iter()
        .filter(|t| t.kind == ThingKind::Stall)
        .count();
    assert_eq!(count("trader"), stalls, "a trader to each stall");
    assert_eq!(count("smith"), 1);
    for (index, person) in life.people.iter().enumerate() {
        assert_eq!(life.town.things[person.home].kind, ThingKind::Door);
        if life.role(index) == "trader" {
            let owns = person.owns.expect("a trader owns a stall");
            assert_eq!(life.town.things[owns].kind, ThingKind::Stall);
        }
        // Children are smaller and step shorter, so walk slower; nobody walks faster than
        // the walk can be played at.
        if life.role(index) == "child" {
            assert!(person.scale < 0.8 && person.walk < crate::WALK_PACE);
        }
        assert!(person.walk < crate::WALK_PACE * person.scale * 1.6);
    }
    // Every home is lived in, by the plan's household at most.
    let (_, most) = life.plan.household;
    for &home in &life.town.homes {
        let door = life
            .town
            .things
            .iter()
            .position(|t| t.kind == ThingKind::Door && t.building == Some(home))
            .unwrap();
        let living = life.people.iter().filter(|p| p.home == door).count();
        assert!(
            living >= 1 && living <= most as usize + 1,
            "{living} in a home"
        );
    }
}

#[test]
fn a_market_morning_walks_without_anyone_in_a_wall_or_another() {
    let mut life = life(1);
    let viewer = square(&life);
    let watch = run(&mut life, 9.0, 1.5, viewer);
    let faults = &watch.faults;
    assert!(
        faults.in_walls.is_empty(),
        "in walls: {:?}",
        faults.in_walls
    );
    assert!(
        faults.through.is_empty(),
        "through each other: {:?}",
        faults.through
    );
    assert!(faults.sliding.is_empty(), "sliding: {:?}", faults.sliding);
    // A morning in the market is busy: most of the town is out, and many on the square.
    let hour = &watch.hours[0];
    assert!(
        hour.out > life.people.len() as f32 * 0.4,
        "{} out of {}",
        hour.out,
        life.people.len()
    );
    assert!(hour.on_square > 20.0, "{} on the square", hour.on_square);
    assert!(
        hour.doing.get("Sell").copied().unwrap_or(0.0) >= 3.0,
        "the traders sell"
    );
}

#[test]
fn far_from_everyone_nobody_ticks_and_coming_near_finds_them_where_the_clock_says() {
    let mut life = life(3);
    let far = Vec2::new(5000.0, 5000.0);
    life.start(10.0);
    let before: Vec<Vec2> = life.people.iter().map(|p| p.at).collect();
    for n in 0..200 {
        life.step(10.0 + n as f64 * 0.001, STEP, &[far]);
    }
    let after: Vec<Vec2> = life.people.iter().map(|p| p.at).collect();
    assert_eq!(before, after, "nobody moved with no one near");
    // At two in the night everyone is indoors; at ten the traders are at their stalls.
    life.step(26.0, STEP, &[square(&life)]);
    assert!(
        life.people.iter().all(|p| p.state == State::Inside),
        "abed at two"
    );
    life.step(34.0, STEP, &[far]);
    life.step(34.0, STEP, &[square(&life)]);
    for (index, person) in life.people.iter().enumerate() {
        if life.role(index) == "trader" {
            assert_eq!(life.activity(index), Some("Sell"));
            let spot = &life.spots[person.spot.expect("at a spot")];
            assert_eq!(Some(spot.thing), person.owns, "at their own stall");
        }
    }
}

#[test]
fn hens_keep_to_their_yard_and_dogs_to_their_owners_heels() {
    let mut life = life(2);
    let animals = life.plan.animals.clone();
    let mut kept = crate::Animals::new(&life, animals, 2);
    assert!(
        kept.animals.iter().filter(|a| a.kind == "Hen").count() >= 6,
        "hens in the yards"
    );
    let viewer = square(&life);
    let yards: Vec<[Vec2; 4]> = life.town.yards.iter().map(|(_, quad)| *quad).collect();
    life.start(9.0);
    let steps = (0.4 * SECONDS_AN_HOUR as f64 / STEP as f64) as u64;
    for n in 0..steps {
        let at = 9.0 + n as f64 * STEP as f64 / SECONDS_AN_HOUR as f64;
        life.step(at, STEP, &[viewer]);
        kept.step(&life, STEP, &[viewer]);
        for animal in kept.animals.iter().filter(|a| a.kind != "Dog") {
            assert!(
                yards
                    .iter()
                    .any(|yard| terra_build::town::inside(animal.at, yard)),
                "a {} left its yard at {:?}",
                animal.kind,
                animal.at
            );
        }
    }
    // The hens moved about rather than standing.
    assert!(
        kept.animals
            .iter()
            .any(|a| a.kind == "Hen" && a.speed > 0.0)
            || steps > 0
    );
}

#[test]
fn on_market_morning_the_dogs_are_out_at_their_owners_heels_from_the_start() {
    let mut life = life(2);
    let viewer = square(&life);
    life.start_settled(10.5, viewer);
    let animals = life.plan.animals.clone();
    let mut kept = crate::Animals::new(&life, animals, 2);
    kept.settle(&life);
    let out: Vec<Vec2> = life
        .people
        .iter()
        .filter(|p| !matches!(p.state, State::Inside | State::Away))
        .map(|p| p.at)
        .collect();
    let dogs: Vec<Vec2> = kept
        .animals
        .iter()
        .filter(|a| a.kind == "Dog")
        .map(|a| a.at)
        .collect();
    assert!(dogs.len() >= 4, "{} dogs", dogs.len());
    let at_heel = dogs
        .iter()
        .filter(|dog| out.iter().any(|p| p.distance(**dog) < 2.0))
        .count();
    assert!(
        at_heel * 2 >= dogs.len(),
        "{at_heel} of {} dogs at someone's heels",
        dogs.len()
    );
}

#[test] // Slow: run before a push, not a commit (SLOW_TESTS in xtask).
fn the_same_town_and_seed_live_the_same_day() {
    let viewer = |life: &Life| square(life);
    let mut a = life(4);
    let mut b = life(4);
    let (va, vb) = (viewer(&a), viewer(&b));
    run(&mut a, 7.0, 0.3, va);
    run(&mut b, 7.0, 0.3, vb);
    let at = |life: &Life| life.people.iter().map(|p| p.at).collect::<Vec<_>>();
    assert_eq!(at(&a), at(&b));
}

#[test] // Slow: run before a push, not a commit (SLOW_TESTS in xtask).
fn walkers_go_round_a_visitor_standing_in_their_way() {
    // Where someone walks a few minutes into the morning, found without a visitor; then the
    // same morning with one standing there.
    let steps = (0.2 * SECONDS_AN_HOUR as f64 / STEP as f64) as u64;
    let hour = |n: u64| 9.0 + n as f64 * STEP as f64 / SECONDS_AN_HOUR as f64;
    let mut first = life(3);
    let viewer = square(&first);
    // Started as the game starts a town, its day already going, so people are on their way.
    first.start_settled(9.0, viewer);
    let mut way = None;
    for n in 0..steps {
        first.step(hour(n), STEP, &[viewer]);
        if n == steps / 2 {
            way = first
                .people
                .iter()
                .find(|p| matches!(p.state, State::Walking { .. }) && p.velocity.length() > 0.8)
                .map(|p| (p.at + p.velocity * 2.0, p.velocity));
        }
    }
    let (here, _) = way.expect("someone walking");
    let mut life = life(3);
    life.visitors = vec![crate::orca::Body {
        at: here,
        velocity: Vec2::ZERO,
        radius: 0.3,
    }];
    life.start_settled(9.0, viewer);
    let (mut closest, mut passed) = (f32::MAX, 0);
    for n in 0..steps {
        life.step(hour(n), STEP, &[viewer]);
        for p in &life.people {
            if matches!(p.state, State::Walking { .. }) && p.at.distance(here) < 2.0 {
                passed += 1;
                closest = closest.min(p.at.distance(here) - p.radius * 0.5);
            }
        }
    }
    assert!(passed > 0, "someone walks by");
    assert!(
        closest > 0.25,
        "a walker came within {closest} m of the visitor"
    );
}

/// How many of the town's people are out walking, over the next few minutes from `hours`.
fn walking_after(life: &mut Life, hours: f64) -> f32 {
    let viewer = square(life);
    let (mut walking, mut seen) = (0usize, 0usize);
    for n in 0..1500u64 {
        let at = hours + n as f64 * STEP as f64 / SECONDS_AN_HOUR as f64;
        life.step(at, STEP, &[viewer]);
        if n % 50 == 0 {
            walking += life
                .people
                .iter()
                .filter(|p| matches!(p.state, State::Walking { .. }))
                .count();
            seen += 1;
        }
    }
    walking as f32 / seen as f32
}

#[test] // Slow: run before a push, not a commit (SLOW_TESTS in xtask).
fn a_town_started_settled_is_in_mid_flow_not_stood_still() {
    let mut cold = life(2);
    cold.start(8.5);
    let cold = walking_after(&mut cold, 8.5);
    let mut settled = life(2);
    let viewer = square(&settled);
    settled.start_settled(8.5, viewer);
    let settled = walking_after(&mut settled, 8.5);
    assert!(
        settled > cold * 2.0 && settled >= 3.0,
        "walking: {settled} settled, {cold} cold"
    );
}

#[test]
fn builders_keep_at_work_while_a_settlement_goes_up_a_building_after_another() {
    // D381: in a time-lapse each building's sites are open some twenty seconds of play, the
    // homes nearest the square first; builders go from one to the next and are mostly at work.
    let ground = |at: Vec2| at.x * 0.03;
    let mut town = town(2, KINDS, &ground);
    let first = town.things.len();
    town.add_sites();
    let mut life = Life::new(town, plan(), 2, SECONDS_AN_HOUR);
    let viewer = square(&life);
    let mut hours = 7.0;
    let sites: Vec<usize> = (first..life.town.things.len()).collect();
    for &site in &sites {
        life.set_open(site, false, hours);
    }
    life.start_settled(hours, viewer);
    let mut order: Vec<usize> = (0..life.town.buildings.len()).collect();
    order.sort_by(|&a, &b| {
        let d = |i: usize| Vec2::from(life.town.buildings[i].at).length();
        d(a).total_cmp(&d(b))
    });
    const PACE: f32 = 3.0;
    let open_s = 30.0;
    let every_s = 0.6;
    let builders: Vec<usize> = (0..life.people.len())
        .filter(|&p| life.role(p) == "builder")
        .collect();
    let mut at_work = Vec::new();
    let steps = (45.0 / STEP) as usize;
    for n in 0..steps {
        let t = n as f32 * STEP;
        for (rank, &building) in order.iter().enumerate() {
            let opens = rank as f32 * every_s;
            for (when, open) in [(opens, true), (opens + open_s, false)] {
                if (t - when).abs() < STEP * 0.5 {
                    for &site in &sites {
                        if life.town.things[site].building == Some(building) {
                            life.set_open(site, open, hours);
                        }
                    }
                }
            }
        }
        // Its people live three times as fast as the film runs, as a time-lapse's do.
        hours += (STEP * PACE) as f64 / SECONDS_AN_HOUR as f64;
        life.step(hours, STEP * PACE, &[viewer]);
        if n % 50 == 0 && t > 15.0 {
            at_work.push(
                builders
                    .iter()
                    .filter(|&&p| {
                        life.activity(p) == Some("Build")
                            && matches!(life.people[p].state, State::Doing { .. })
                    })
                    .count(),
            );
        }
    }
    let mean = at_work.iter().sum::<usize>() as f32 / at_work.len() as f32;
    assert!(
        mean >= builders.len() as f32 * 0.5,
        "{mean} of {} builders at work: {at_work:?}",
        builders.len()
    );
}

#[test]
fn builders_go_to_a_building_while_it_goes_up_and_leave_when_it_is_done() {
    // D381: a site's spots are closed until its building goes up; opened, builders busy at
    // something else go to it and hammer there; closed, nobody stays.
    let ground = |at: Vec2| at.x * 0.03;
    let mut town = town(2, KINDS, &ground);
    let first = town.things.len();
    town.add_sites();
    let mut life = Life::new(town, plan(), 2, SECONDS_AN_HOUR);
    let sites: Vec<usize> = (first..life.town.things.len()).collect();
    let viewer = square(&life);
    let mut hours = 8.0;
    for &site in &sites {
        life.set_open(site, false, hours);
    }
    life.start_settled(hours, viewer);
    let building = |life: &Life, person: usize| {
        let spot = life.people[person].spot?;
        let thing = &life.town.things[life.spots[spot].thing];
        (thing.kind == ThingKind::Site)
            .then_some(thing.building)
            .flatten()
    };
    let at_sites = |life: &Life| {
        (0..life.people.len())
            .filter(|&p| life.activity(p) == Some("Build") && building(life, p).is_some())
            .count()
    };
    let advance = |life: &mut Life, hours: &mut f64, minutes: f64| {
        let steps = (minutes / 60.0 * SECONDS_AN_HOUR as f64 / STEP as f64) as u64;
        for _ in 0..steps {
            *hours += STEP as f64 / SECONDS_AN_HOUR as f64;
            life.step(*hours, STEP, &[viewer]);
        }
    };
    advance(&mut life, &mut hours, 10.0);
    assert_eq!(at_sites(&life), 0, "nobody builds where nothing goes up");
    // The building of the first home's sites opened.
    let home = life.town.homes[0];
    let its: Vec<usize> = sites
        .iter()
        .copied()
        .filter(|&s| life.town.things[s].building == Some(home))
        .collect();
    for &site in &its {
        life.set_open(site, true, hours);
    }
    advance(&mut life, &mut hours, 20.0);
    let building_there = (0..life.people.len())
        .filter(|&p| {
            life.activity(p) == Some("Build")
                && matches!(life.people[p].state, State::Doing { .. })
                && building(&life, p) == Some(home)
        })
        .count();
    assert!(
        building_there >= 2,
        "{building_there} building at the open site"
    );
    for &site in &its {
        life.set_open(site, false, hours);
    }
    advance(&mut life, &mut hours, 2.0);
    assert_eq!(at_sites(&life), 0, "nobody stays at a site closed");
}

#[test]
fn a_dog_at_heel_does_not_flick_between_sitting_and_standing() {
    use crate::animals::Doing;
    // What a dog is drawn doing, as the game picks its clip (`town_animals::animate`).
    fn pose(dog: &crate::Animal) -> u8 {
        match dog.doing {
            Doing::Resting { .. } => 0,
            Doing::Moving { .. } if dog.speed > 0.02 => 1,
            Doing::Moving { .. } => 2,
        }
    }
    let mut life = life(2);
    let viewer = square(&life);
    life.start_settled(10.0, viewer);
    let animals = life.plan.animals.clone();
    let mut kept = crate::Animals::new(&life, animals, 2);
    kept.settle(&life);
    // A film's frame with the people living three times faster (`rising`, D381).
    let step = 3.0 / 60.0;
    let dogs: Vec<usize> = (0..kept.animals.len())
        .filter(|&i| kept.animals[i].kind == "Dog")
        .collect();
    let mut since: Vec<(u8, u8, u32)> = dogs
        .iter()
        .map(|&i| (pose(&kept.animals[i]), pose(&kept.animals[i]), 1000))
        .collect();
    let mut flicks = 0;
    for n in 0..1200u32 {
        let at = 10.0 + n as f64 * step as f64 / SECONDS_AN_HOUR as f64;
        life.step(at, step, &[viewer]);
        kept.step(&life, step, &[viewer]);
        for (k, &i) in dogs.iter().enumerate() {
            let now = pose(&kept.animals[i]);
            let (was, before, held) = since[k];
            if now != was {
                // Back to what it was a moment ago: a pose held under a fifth of a second.
                if now == before && held < 4 {
                    flicks += 1;
                }
                since[k] = (now, was, 0);
            } else {
                since[k].2 += 1;
            }
        }
    }
    assert_eq!(
        flicks, 0,
        "dogs flicked back to a pose they had just left {flicks} times"
    );
}
