//! The instance host's tests (`instance`), among the hosting tests for their bot.

use super::*;

/// Two realms on one carrier (D640): a player in each is walked by its own realm and is in
/// no other; a realm nobody is in freezes after its grace, its server's tick standing, and
/// thaws when someone comes.
#[test]
fn realms_on_one_carrier_each_walk_their_own_and_an_empty_one_freezes() {
    use instance::{GRACE_TICKS, Instances, Life, walker};
    let network = Network::new();
    let mut realms = Instances::new(network.server(), Config::new(VERSION), vec![walker]);
    for seed in [1, 2] {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(seed),
            radius_m: 2000.0,
        });
        let start = planet.ground_at(Vec3::Y * 2100.0);
        realms.open(planet, start);
    }
    let players = |realms: &mut Instances<_>, realm: usize| {
        let world = realms.realm_mut(realm).world_mut();
        world.query::<&Player>().iter(world).count()
    };
    let mut bots = vec![Bot::join(&network, Some(Action::Forward))];
    let run = |realms: &mut Instances<_>, bots: &mut Vec<Bot>, ticks: u32| {
        for _ in 0..ticks {
            network.advance(f64::from(TICK));
            for bot in bots.iter_mut() {
                bot.tick();
            }
            realms.tick(Duration::from_secs_f32(TICK));
        }
    };
    run(&mut realms, &mut bots, GRACE_TICKS + 100);
    assert_eq!(realms.realm(0).life(), Life::Active);
    assert_eq!((players(&mut realms, 0), players(&mut realms, 1)), (1, 0));
    assert_eq!(realms.realm(1).life(), Life::Frozen);
    let stood = realms.realm(1).server_tick();
    let went = realms.realm(0).server_tick();
    run(&mut realms, &mut bots, 64);
    assert_eq!(
        realms.realm(1).server_tick(),
        stood,
        "a frozen realm ticked"
    );
    assert_eq!(realms.realm(0).server_tick(), went + 64);
    // What the walk says each tick is spent, not kept for ever.
    let fell = realms
        .realm(0)
        .world()
        .resource::<Messages<crate::movement::Fell>>();
    assert!(fell.len() < 8, "{} falls kept", fell.len());

    // Someone comes to the frozen realm: it thaws and walks them; the first has only its own.
    realms.home(1);
    bots.push(Bot::join(&network, None));
    run(&mut realms, &mut bots, 200);
    assert_eq!(realms.realm(1).life(), Life::Active);
    assert_eq!((players(&mut realms, 0), players(&mut realms, 1)), (1, 1));
    for bot in &bots {
        assert_eq!(
            bot.client.entities().count(),
            1,
            "a player is told of another realm's"
        );
    }
}

/// A joined game handed from one realm to the other (a crossing, D640): it holds, walking
/// nothing and with no body anywhere, until its game says the next land is made; then it has
/// a body in the second realm, none in the first, and its walk there is the realm's.
#[test]
fn a_joined_game_handed_on_walks_in_the_next_realm_once_its_land_is_made() {
    use instance::{Instances, walker};
    let network = Network::new();
    let mut realms = Instances::new(network.server(), Config::new(VERSION), vec![walker]);
    for _ in 0..2 {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        let start = planet.ground_at(Vec3::Y * 2100.0);
        realms.open(planet, start);
    }
    let (mut game, player) = joined(&network, rough());
    // Joined to whatever realm the host has, as from the title screen (D639), not to one it
    // made beforehand; the link `joined` made is left greeted and never ready, with no body.
    game.insert_resource(link::Link::join(
        Box::new(network.connect(rough())),
        VERSION,
        &Identity::from_secret([7; 32]),
        "crosser",
        Vec::new(),
    ));
    let guests = |realms: &mut Instances<_>, realm: usize| -> Vec<Vec3> {
        let world = realms.realm_mut(realm).world_mut();
        let mut guests = world.query_filtered::<&Transform, With<Guest>>();
        guests.iter(world).map(|at| at.translation).collect()
    };
    let mut budget = 0.0;
    let mut run = |realms: &mut Instances<_>, game: &mut World, steps: std::ops::Range<u32>| {
        for step in steps {
            network.advance(f64::from(TICK));
            budget += game.resource::<link::Link>().rate();
            while budget >= 1.0 {
                budget -= 1.0;
                *game.get_mut::<Intent>(player).unwrap() = keys(step);
                game.run_system_once(link::listen).unwrap();
                game.run_system_once(walk).unwrap();
                game.run_system_once(link::record).unwrap();
                game.run_system_once(link::show).unwrap();
            }
            realms.tick(Duration::from_secs_f32(TICK));
        }
    };
    run(&mut realms, &mut game, 0..WARM);
    assert_eq!(
        (guests(&mut realms, 0).len(), guests(&mut realms, 1).len()),
        (1, 0)
    );
    let peer = realms
        .realm(0)
        .world()
        .resource::<Host>()
        .guests
        .keys()
        .copied()
        .next();
    realms.hand(peer.expect("the first realm has its guest"), 1);

    // Handed on: greeted by the second realm, it holds until its land is made.
    run(&mut realms, &mut game, WARM..WARM + 200);
    assert!(
        game.get_resource::<link::Ended>().is_none(),
        "the link ended"
    );
    let link = game.resource::<link::Link>();
    assert!(
        link.crossing().is_some(),
        "not told the realm it crosses to"
    );
    assert_eq!(link.tick(), None, "it walked between realms");
    assert_eq!(
        (guests(&mut realms, 0).len(), guests(&mut realms, 1).len()),
        (0, 0)
    );

    // Its land made: it takes its body in the second realm and walks as that realm has it.
    game.resource_mut::<link::Link>().arrived();
    run(&mut realms, &mut game, WARM + 200..WARM + 600);
    assert!(
        game.get_resource::<link::Ended>().is_none(),
        "the link ended"
    );
    let theirs = guests(&mut realms, 1);
    assert_eq!((guests(&mut realms, 0).len(), theirs.len()), (0, 1));
    let mine = game
        .resource::<Movers>()
        .body(player)
        .expect("it walks")
        .position;
    let off = mine.as_vec3().distance(theirs[0]);
    assert!(off < 0.5, "{off} m from where the second realm has it");
    assert!(game.resource::<link::Link>().tick().is_some());
}

/// A realm with the trees' layer holds what its world's trees stop, by the world's own rule:
/// a body put at a tree's foot is pushed out of its trunk there, and is left where it is in
/// a realm with the walker's layer alone, which is the fault this would hide (a player
/// walked by a realm through the trunks their own game shows, 23 cm apart in crossing.ts).
#[test]
fn a_realm_with_the_trees_layer_stops_a_body_at_a_trunk() {
    use instance::{Instances, trees, walker};
    use terra_world::flora::Obstacles;
    use terra_world::flora_kinds::{Flora, Season};
    let planet = || {
        Planet::smooth(PlanetSpec {
            seed: Seed(3),
            radius_m: 2000.0,
        })
    };
    let world = planet();
    let start = terra_world::start::Start::choose(&world, false);
    let stood =
        terra_world::flora::trees::stops((&world, &start.0), &Flora::read(), &Season::Unknown);
    let (foot, radius) = stood[stood.len() / 2];
    let pushed = |layers: Vec<instance::Layer>| {
        let mut realms = Instances::new(Network::new().server(), Config::new(VERSION), layers);
        realms.open(planet(), start.translation);
        let obstacles = realms.realm(0).world().resource::<Obstacles>();
        obstacles.push_out(foot, 0.3, 0.3).distance(foot)
    };
    let with = pushed(vec![walker, trees]);
    assert!(
        with > radius,
        "at a trunk {radius} m round the body moved {with} m"
    );
    assert_eq!(pushed(vec![walker]), 0.0);
}
