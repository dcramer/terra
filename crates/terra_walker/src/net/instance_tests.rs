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
