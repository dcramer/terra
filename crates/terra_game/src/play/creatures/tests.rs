use super::gait::bolts_within;
use super::mind::{ROOSTS_OVER_M, SENSE_M, nearest};
use super::*;
use crate::play::species::Temper;
use crate::play::tick::Ticks;
use crate::play::walker::Motion;
use crate::play::{Downed, test_player};
use crate::spatial::SpatialHash;
use bevy::ecs::system::RunSystemOnce;

#[test]
fn a_creature_about_its_day_is_unaware_and_one_that_has_noticed_is_not() {
    let mut creature = Creature {
        species: Bestiary::read(&Default::default()).named("Deer").unwrap(),
        health: 10.0,
        state: State::Graze(1.0),
        home: Vec3::ZERO,
        facing: Vec3::X,
        hurt: false,
        quarry: None,
        aloft: 0.0,
        side: Vec3::X,
        bolt: Vec3::ZERO,
        pace: Pace::Startled,
    };
    assert!(creature.unaware());
    creature.state = State::Flee(2.0);
    assert!(!creature.unaware());
    creature.state = State::Idle(1.0);
    creature.hurt = true;
    assert!(!creature.unaware());
}

fn spider(world: &mut World, at: Vec3) -> Entity {
    world
        .spawn((
            Creature {
                species: world.resource::<Bestiary>().named("Spider").unwrap(),
                health: 10.0,
                state: State::Idle(1.0),
                home: at,
                facing: Vec3::X,
                hurt: false,
                quarry: None,
                aloft: 0.0,
                side: Vec3::X,
                bolt: Vec3::ZERO,
                pace: Pace::Startled,
            },
            Transform::from_translation(at),
        ))
        .id()
}

fn world() -> World {
    let mut world = World::new();
    world.init_resource::<Time>();
    world.init_resource::<Messages<Bitten>>();
    world.insert_resource(Dice(Seed(1).derive("test").rng()));
    world.insert_resource(Bestiary::read(&Default::default()));
    world.init_resource::<Ticks>();
    // What a creature asks of the world round it: the ground, its trees.
    let planet = Planet::smooth(terra_core::PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    world.insert_resource(Start::choose(&planet, true));
    world.insert_resource(planet);
    world.init_resource::<terra_world::flora::Trunks>();
    world.init_resource::<terra_world::flora::Obstacles>();
    world.init_resource::<super::super::wildlife::Warmth>();
    world.init_resource::<super::super::wildlife::Perches>();
    world.init_resource::<super::super::wildlife::Thickets>();
    world.init_resource::<terra_world::daylight::Clock>();
    world
}

#[test]
fn a_spider_goes_for_the_nearest_player_who_is_up() {
    let mut world = world();
    let ground = Vec3::Y * 2000.0;
    let near = test_player(&mut world, ground + Vec3::X * 5.0);
    let far = test_player(&mut world, ground + Vec3::X * 10.0);
    let spider = spider(&mut world, ground);
    world.run_system_once(think).unwrap();
    let chased = world.get::<Creature>(spider).unwrap();
    assert_eq!(chased.quarry, Some(near));
    assert_eq!(chased.state, State::Chase);
    // Down, the nearer one is passed over for the other.
    world.entity_mut(near).insert(Downed { lying_s: 0.0 });
    world.run_system_once(think).unwrap();
    assert_eq!(world.get::<Creature>(spider).unwrap().quarry, Some(far));
}

#[test]
fn a_bite_lands_on_the_player_bitten() {
    let mut world = world();
    let ground = Vec3::Y * 2000.0;
    let _far = test_player(&mut world, ground + Vec3::X * 20.0);
    let near = test_player(&mut world, ground + Vec3::X * 1.5);
    let spider = spider(&mut world, ground);
    let Temper::Hostile { bite_at_s, .. } = temper(&world, spider) else {
        panic!("a spider is hostile");
    };
    // Just short of where the bite lands, which the next moment passes.
    world.get_mut::<Creature>(spider).unwrap().state = State::Attack(bite_at_s - 0.01);
    world
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(20));
    world.run_system_once(think).unwrap();
    let bites: Vec<_> = world
        .resource_mut::<Messages<Bitten>>()
        .drain()
        .map(|bite| bite.player)
        .collect();
    assert_eq!(bites, [near]);
}

#[test]
fn nobody_beyond_its_senses_is_noticed() {
    let mut awake = SpatialHash::default();
    let here = Vec3::Y * 2000.0;
    awake.insert(here + Vec3::X * (SENSE_M + 1.0), Entity::PLACEHOLDER);
    assert_eq!(nearest(&awake, here), None);
}

fn temper(world: &World, creature: Entity) -> Temper {
    let species = world.get::<Creature>(creature).unwrap().species;
    world.resource::<Bestiary>().get(species).temper
}

/// A creature of the first kind that is shy, or hostile, at home at a place.
fn one(world: &mut World, hostile: bool, home: Vec3) -> Entity {
    let bestiary = world.resource::<Bestiary>();
    let species = bestiary
        .all()
        .find(|&species| bestiary.hostile(species) == hostile)
        .unwrap();
    world
        .spawn((
            Creature::resting(species, 10.0, home),
            Transform::from_translation(home),
        ))
        .id()
}

fn state(world: &World, creature: Entity) -> State {
    world.get::<Creature>(creature).unwrap().state
}

#[test]
fn a_shy_creature_runs_from_anyone_near() {
    let ground = Vec3::Y * 2000.0;
    let (mut world, mut far_world) = (world(), world());
    let near = one(&mut world, false, ground);
    let far = one(&mut far_world, false, ground);
    let Temper::Shy { flees_within_m, .. } = temper(&world, near) else {
        unreachable!()
    };
    test_player(&mut world, ground + Vec3::X * flees_within_m * 0.5);
    test_player(&mut far_world, ground + Vec3::X * (flees_within_m + 1.0));
    world.run_system_once(think).unwrap();
    far_world.run_system_once(think).unwrap();
    assert!(matches!(state(&world, near), State::Flee(_)));
    assert!(!matches!(state(&far_world, far), State::Flee(_)));
}

#[test]
fn a_grazer_feeds_on_while_someone_is_well_off_and_stops_to_watch_them_nearer() {
    let ground = Vec3::Y * 2000.0;
    let (mut world, mut near_world) = (world(), world());
    let far = one(&mut world, false, ground);
    let near = one(&mut near_world, false, ground);
    let Temper::Shy { flees_within_m, .. } = temper(&world, far) else {
        unreachable!()
    };
    for (world, creature) in [(&mut world, far), (&mut near_world, near)] {
        world.get_mut::<Creature>(creature).unwrap().state = State::Graze(5.0);
    }
    test_player(&mut world, ground + Vec3::X * flees_within_m * 2.0);
    test_player(&mut near_world, ground + Vec3::X * flees_within_m * 1.2);
    world.run_system_once(think).unwrap();
    near_world.run_system_once(think).unwrap();
    assert!(matches!(state(&world, far), State::Graze(_)));
    assert!(matches!(state(&near_world, near), State::Idle(_)));
}

#[test]
fn a_grazer_feeds_on_while_someone_jogs_by_wide_of_it_and_watches_one_coming_at_it() {
    let ground = Vec3::Y * 2000.0;
    let (mut world, mut toward_world) = (world(), world());
    let passed = one(&mut world, false, ground);
    let met = one(&mut toward_world, false, ground);
    let Temper::Shy { flees_within_m, .. } = temper(&world, passed) else {
        unreachable!()
    };
    let facing = Vec3::Z;
    for (world, creature) in [(&mut world, passed), (&mut toward_world, met)] {
        let mut creature = world.get_mut::<Creature>(creature).unwrap();
        (creature.state, creature.facing) = (State::Graze(5.0), facing);
    }
    // Both 1.2 times its flight distance off: one jogging along a way that keeps wider of it
    // than it bolts at, one jogging straight at it.
    let off = flees_within_m * 1.2;
    let aside = (bolts_within(flees_within_m) + 1.0).min(off * 0.9);
    let along = (off * off - aside * aside).sqrt();
    let by = test_player(&mut world, ground + Vec3::X * aside - Vec3::Z * along);
    world.get_mut::<Motion>(by).unwrap().velocity = Vec3::Z * 4.3;
    let at = test_player(&mut toward_world, ground + Vec3::X * off);
    toward_world.get_mut::<Motion>(at).unwrap().velocity = -Vec3::X * 4.3;
    world.run_system_once(think).unwrap();
    toward_world.run_system_once(think).unwrap();
    assert!(
        matches!(state(&world, passed), State::Graze(_)),
        "{:?}",
        state(&world, passed)
    );
    assert_eq!(world.get::<Creature>(passed).unwrap().facing, facing);
    assert!(
        matches!(state(&toward_world, met), State::Idle(_)),
        "{:?}",
        state(&toward_world, met)
    );
}

#[test]
fn a_shy_creature_hurt_with_nobody_near_runs_and_then_settles() {
    let mut world = world();
    let deer = one(&mut world, false, Vec3::Y * 2000.0);
    world.get_mut::<Creature>(deer).unwrap().hurt = true;
    world.run_system_once(think).unwrap();
    assert!(matches!(state(&world, deer), State::Flee(left) if left > 0.0));
    assert!(!world.get::<Creature>(deer).unwrap().hurt);
    // Once it has run its course, it goes back to its day.
    world.get_mut::<Creature>(deer).unwrap().state = State::Flee(0.0);
    world.run_system_once(think).unwrap();
    assert!(world.get::<Creature>(deer).unwrap().unaware());
}

#[test]
fn a_hostile_creature_led_too_far_from_home_goes_back() {
    let home = Vec3::Y * 2000.0;
    let mut world = world();
    let spider = one(&mut world, true, home);
    let Temper::Hostile {
        notices_m,
        leash_m,
        attacks_within_m,
        ..
    } = temper(&world, spider)
    else {
        unreachable!()
    };
    // Someone it would chase, near enough to notice and too far to bite.
    let quarry = Vec3::X * (notices_m + attacks_within_m) * 0.5;
    let player = test_player(&mut world, home + quarry);
    world.get_mut::<Creature>(spider).unwrap().state = State::Chase;
    world.run_system_once(think).unwrap();
    assert_eq!(
        state(&world, spider),
        State::Chase,
        "at home it gives chase"
    );
    let beyond = home + Vec3::NEG_Z * (leash_m + 1.0);
    world.get_mut::<Transform>(spider).unwrap().translation = beyond;
    world.get_mut::<Transform>(player).unwrap().translation = beyond + quarry;
    world.run_system_once(think).unwrap();
    assert_eq!(state(&world, spider), State::Return);
    // It keeps going home with nobody about, until it is there.
    world.despawn(player);
    world.run_system_once(think).unwrap();
    assert_eq!(state(&world, spider), State::Return);
    world.get_mut::<Transform>(spider).unwrap().translation = home;
    world.run_system_once(think).unwrap();
    assert_ne!(state(&world, spider), State::Return);
}

#[test]
fn a_hostile_creature_goes_for_whoever_hurt_it_from_beyond_its_notice() {
    let home = Vec3::Y * 2000.0;
    let mut world = world();
    let spider = one(&mut world, true, home);
    let Temper::Hostile { notices_m, .. } = temper(&world, spider) else {
        unreachable!()
    };
    assert!(notices_m < SENSE_M);
    test_player(&mut world, home + Vec3::X * (notices_m + SENSE_M) * 0.5);
    world.run_system_once(think).unwrap();
    assert_ne!(
        state(&world, spider),
        State::Chase,
        "unhurt, it has not noticed"
    );
    world.get_mut::<Creature>(spider).unwrap().hurt = true;
    world.run_system_once(think).unwrap();
    assert_eq!(state(&world, spider), State::Chase);
}

/// One of a kind, named, resting at home.
fn kind(world: &mut World, name: &str, home: Vec3) -> Entity {
    let species = world.resource::<Bestiary>().named(name).unwrap();
    world
        .spawn((
            Creature::resting(species, 2.0, home),
            Transform::from_translation(home),
        ))
        .id()
}

#[test]
fn a_climber_startled_goes_up_the_nearest_trunk_and_down_once_nobody_is_near() {
    let mut world = world();
    let ground = Vec3::Y * 2000.0;
    let near = Trunk {
        foot: ground + Vec3::X * 3.0,
        radius_m: 0.3,
        height_m: 20.0,
        solid: false,
    };
    let far = Trunk {
        foot: ground + Vec3::NEG_X * 9.0,
        ..near
    };
    let mut trunks = world.resource_mut::<terra_world::flora::Trunks>();
    trunks.add(near);
    trunks.add(far);
    let squirrel = kind(&mut world, "Red squirrel", ground);
    let player = test_player(&mut world, ground + Vec3::Z * 3.0);
    world.run_system_once(think).unwrap();
    let State::Climb { trunk, height } = state(&world, squirrel) else {
        panic!("it should climb, not {:?}", state(&world, squirrel));
    };
    assert_eq!(trunk, near);
    assert!(height > 1.0 && height < near.height_m);
    // Up there, it waits while anyone is near, and comes down once they have gone.
    world.get_mut::<Creature>(squirrel).unwrap().aloft = height;
    world.run_system_once(think).unwrap();
    assert!(matches!(state(&world, squirrel), State::Perch { .. }));
    world.get_mut::<Creature>(squirrel).unwrap().state = State::Perch { trunk, left: 0.0 };
    world.run_system_once(think).unwrap();
    assert!(matches!(state(&world, squirrel), State::Perch { left, .. } if left > 0.0));
    world.despawn(player);
    world.get_mut::<Creature>(squirrel).unwrap().state = State::Perch { trunk, left: 0.0 };
    world.run_system_once(think).unwrap();
    assert_eq!(state(&world, squirrel), State::Climb { trunk, height: 0.0 });
}

#[test]
fn a_bird_startled_takes_wing_away_from_whoever_startled_it() {
    let mut world = world();
    let ground = Vec3::Y * 2000.0;
    let bird = kind(&mut world, "Chaffinch", ground);
    test_player(&mut world, ground + Vec3::X * 2.0);
    world.run_system_once(think).unwrap();
    let State::Fly { from, to, .. } = state(&world, bird) else {
        panic!("it should fly, not {:?}", state(&world, bird));
    };
    assert_eq!(from, ground);
    let species = world.get::<Creature>(bird).unwrap().species;
    let Refuge::Flies(near, far) = world.resource::<Bestiary>().get(species).refuge else {
        panic!("a chaffinch flies off");
    };
    let across = (to - from).reject_from(Vec3::Y).length();
    assert!(across >= near * 0.9 && across <= far * 1.1, "{across}");
    // Away: farther from the player than it was.
    assert!(to.x < from.x, "{to} from {from}");
}

#[test]
fn a_heron_someone_passes_walks_off_and_takes_wing_only_once_they_come_close() {
    let mut world = world();
    let ground = Vec3::Y * 2000.0;
    let heron = kind(&mut world, "Grey heron", ground);
    let species = world.get::<Creature>(heron).unwrap().species;
    let Temper::Shy { flees_within_m, .. } = world.resource::<Bestiary>().get(species).temper
    else {
        panic!("a heron is shy");
    };
    let player = test_player(&mut world, ground + Vec3::X * flees_within_m * 0.7);
    world.run_system_once(think).unwrap();
    assert!(
        matches!(state(&world, heron), State::Flee(_)),
        "{:?}",
        state(&world, heron)
    );
    let creature = world.get::<Creature>(heron).unwrap();
    assert_eq!(creature.pace, Pace::Wary);
    assert!(creature.bolt.x < 0.0, "not away: {}", creature.bolt);
    world.get_mut::<Transform>(player).unwrap().translation =
        ground + Vec3::X * bolts_within(flees_within_m) * 0.5;
    world.run_system_once(think).unwrap();
    assert!(
        matches!(state(&world, heron), State::Fly { .. }),
        "{:?}",
        state(&world, heron)
    );
}

#[test]
fn a_bird_takes_wing_away_from_something_frightful_and_not_from_one_far_off() {
    let mut world = world();
    let ground = Vec3::Y * 2000.0;
    let bird = kind(&mut world, "Chaffinch", ground);
    let calm = kind(&mut world, "Chaffinch", ground + Vec3::Z * 500.0);
    world.spawn((
        Frightful { within_m: 30.0 },
        GlobalTransform::from_translation(ground + Vec3::X * 20.0),
    ));
    world.run_system_once(think).unwrap();
    let State::Fly { from, to, .. } = state(&world, bird) else {
        panic!(
            "it should fly from the fright, not {:?}",
            state(&world, bird)
        );
    };
    assert!(to.x < from.x, "{to} from {from}: not away from it");
    assert!(!matches!(
        state(&world, calm),
        State::Fly { .. } | State::Flee(_)
    ));
}

#[test]
fn a_bird_on_the_wing_flies_its_way_and_lands_there() {
    let mut world = world();
    let ground = Vec3::Y * 2000.0;
    let bird = kind(&mut world, "Chaffinch", ground);
    let to = ground + Vec3::Z * 12.0;
    world.get_mut::<Creature>(bird).unwrap().state = State::Fly {
        from: ground,
        to,
        along: 0.0,
    };
    let mut highest: f32 = 0.0;
    for _ in 0..200 {
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(16));
        world.run_system_once(walk).unwrap();
        world.run_system_once(think).unwrap();
        let at = world.get::<Transform>(bird).unwrap().translation;
        highest = highest.max(at.length() - 2000.0);
        if !matches!(state(&world, bird), State::Fly { .. }) {
            break;
        }
    }
    assert!(highest > 1.0, "it rose off the ground: {highest}");
    let at = world.get::<Transform>(bird).unwrap().translation;
    assert!(at.distance(to) < 0.5, "landed at {at}, not {to}");
    assert!(world.get::<Creature>(bird).unwrap().home().distance(to) < 0.1);
}

#[test]
fn a_bird_flown_into_a_crown_sits_there_and_later_leaves_it() {
    let mut world = world();
    let ground = Vec3::Y * 2000.0;
    let bird = kind(&mut world, "Chaffinch", ground);
    let up_there = ground + Vec3::Y * 12.0 + Vec3::X * 5.0;
    world.get_mut::<Creature>(bird).unwrap().state = State::Fly {
        from: ground,
        to: up_there,
        along: 1.0,
    };
    world.run_system_once(think).unwrap();
    let State::Roost { at, left } = state(&world, bird) else {
        panic!("it should sit in the crown, not {:?}", state(&world, bird));
    };
    assert_eq!(at, up_there);
    assert!(left > 0.0);
    // While it sits it stays where it is, off the ground.
    world.run_system_once(walk).unwrap();
    let sits = world.get::<Transform>(bird).unwrap().translation;
    assert!(sits.distance(up_there) < 1e-3, "{sits}");
    // Its home is the ground under it, where it comes down to.
    let home = world.get::<Creature>(bird).unwrap().home();
    assert!((home.length() - 2000.0).abs() < 0.1, "{home}");
    world.get_mut::<Creature>(bird).unwrap().state = State::Roost {
        at: up_there,
        left: 0.0,
    };
    world.run_system_once(think).unwrap();
    assert!(
        matches!(state(&world, bird), State::Fly { from, .. } if from == up_there),
        "{:?}",
        state(&world, bird)
    );
}

#[test]
fn a_startled_bird_among_trees_may_go_up_into_a_crown_away_from_the_player() {
    let mut world = world();
    let ground = Vec3::Y * 2000.0;
    for i in 0..8 {
        let trunk = Trunk {
            foot: ground + Vec3::new(-6.0 - i as f32 * 2.0, 0.0, (i % 3) as f32 * 3.0 - 3.0),
            radius_m: 0.3,
            height_m: 22.0,
            solid: false,
        };
        world
            .resource_mut::<terra_world::flora::Trunks>()
            .add(trunk);
    }
    let mut crowns = 0;
    for seed in 0..20 {
        world.insert_resource(Dice(Seed(seed).derive("test").rng()));
        let bird = kind(&mut world, "Chaffinch", ground);
        let player = test_player(&mut world, ground + Vec3::X * 2.0);
        world.run_system_once(think).unwrap();
        let State::Fly { to, .. } = state(&world, bird) else {
            panic!("it should fly, not {:?}", state(&world, bird));
        };
        assert!(to.x < ground.x, "away from the player: {to}");
        if to.length() - 2000.0 > ROOSTS_OVER_M {
            crowns += 1;
            assert!(
                to.length() - 2000.0 > 22.0 * 0.45,
                "up among the branches: {to}"
            );
        }
        world.despawn(bird);
        world.despawn(player);
    }
    assert!(crowns > 3 && crowns < 17, "{crowns} of 20 went up a tree");
}
