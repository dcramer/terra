//! A hosted walker world and a client that is no game (D73): the world's own walk moves the
//! guest by the intents its client sends, and the client is told its own body and the host's.

use super::*;
use crate::controls::Action;
use bevy::ecs::system::RunSystemOnce;
use std::time::Duration;
use terra_core::{PlanetSpec, Seed};
use terra_motion::mover::net::{Own, Shown};
use terra_net::client::{Client, Event as Heard};
use terra_net::clock::Clock;
use terra_net::identity::Identity;
use terra_net::transport::loopback::{Conditions, LoopbackClient, Network};
use terra_world::flora;
use terra_world::planet::Planet;

const TICK: f32 = 1.0 / 64.0;
const VERSION: u64 = 9;

/// A hosted world: a smooth planet, the walk's resources, the host's own player standing on
/// the ground, and a server on an in-memory network whose guests start three metres off.
fn hosted() -> (World, Entity, Network) {
    let planet = Planet::smooth(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    let ground = planet.ground_at(Vec3::Y * 2100.0);
    let start = planet.ground_at(Vec3::new(0.0, 2100.0, 3.0));
    let network = Network::new();
    let mut world = World::new();
    world.insert_resource(planet);
    world.init_resource::<flora::Obstacles>();
    world.init_resource::<Movers>();
    world.init_resource::<Messages<crate::movement::Fell>>();
    world.init_resource::<Messages<crate::movement::Drowned>>();
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs_f32(TICK));
    world.insert_resource(time);
    world.insert_resource(Host::new(
        Config::new(VERSION),
        parts(),
        Box::new(network.server()),
        start,
        b"realm".to_vec(),
    ));
    let mut motion = Motion::facing(Vec3::X);
    motion.on_ground = true;
    let view = View {
        forward: Vec3::X,
        pitch: 0.0,
    };
    let host = (
        Player,
        Transform::from_translation(ground),
        motion,
        Stance::default(),
        Stamina::default(),
        view,
        Intent::default(),
    );
    let host = world.spawn(host).id();
    (world, host, network)
}

/// A client with no game behind it: it says hello and, each tick, what it holds down.
struct Bot {
    wire: LoopbackClient,
    client: Client,
    clock: Clock,
    heard: Vec<Heard>,
    holds: Option<Action>,
}

impl Bot {
    fn join(network: &Network, holds: Option<Action>) -> Bot {
        let mut bot = Bot {
            wire: network.connect(Conditions::PERFECT),
            client: Client::new(),
            clock: Clock::default(),
            heard: Vec::new(),
            holds,
        };
        bot.client.hello(
            &mut bot.wire,
            VERSION,
            &Identity::from_secret([5; 32]),
            "bot",
        );
        bot
    }

    fn tick(&mut self) {
        let from = self.heard.len();
        self.client.poll(&mut self.wire, &mut self.heard);
        for heard in &self.heard[from..] {
            if let Heard::Welcome { tick, .. } = heard {
                self.clock.start(*tick);
            }
        }
        if let Some(stamp) = self.client.stamp() {
            self.clock.hear(stamp);
        }
        if let Some(tick) = self.clock.advance() {
            let mut intent = Intent::default();
            if let Some(action) = self.holds {
                intent.hold(action);
            }
            let view = View {
                forward: Vec3::X,
                pitch: 0.0,
            };
            self.client
                .intent(tick, Steering { intent, view }.encoded());
        }
        self.client.send(&mut self.wire);
    }

    /// The entity that is its own, and the other it holds.
    fn mine_and_other(&self) -> (terra_net::client::Entity, terra_net::client::Entity) {
        let find = |mine: bool| {
            let found = self
                .client
                .entities()
                .find(|(_, entity)| entity.mine == mine);
            found
                .map(|(_, entity)| entity.clone())
                .expect("it holds its own body and the host's")
        };
        (find(true), find(false))
    }
}

/// Ticks of the hosted world as the game's schedule runs them (hear, the walk, tell) with the
/// bot's ticks between.
fn run(world: &mut World, network: &Network, bot: &mut Bot, ticks: u32) {
    for _ in 0..ticks {
        network.advance(f64::from(TICK));
        bot.tick();
        world.run_system_once(hear).unwrap();
        world.run_system_once(walk).unwrap();
        world.run_system_once(tell).unwrap();
    }
}

#[test]
fn a_guest_is_walked_by_the_worlds_own_walk_and_told_its_body_and_the_hosts() {
    let (mut world, host_player, network) = hosted();
    let mut bot = Bot::join(&network, Some(Action::Forward));
    run(&mut world, &network, &mut bot, 16 + 128);
    // The guest is a player of this world, and its client's intents walked it: two seconds
    // forward at a jog, less the start.
    let mut guests = world.query::<(Entity, &Guest, &Transform)>();
    let (_, _, transform) = guests.single(&world).expect("one guest");
    let start = world
        .resource::<Planet>()
        .ground_at(Vec3::new(0.0, 2100.0, 3.0));
    let walked = (transform.translation - start).dot(Vec3::X);
    assert!(walked > 6.0 && walked < 10.0, "the guest walked {walked} m");
    assert!(matches!(bot.heard.first(), Some(Heard::Welcome { realm, .. }) if realm == b"realm"));
    // It lets go and comes to rest, so the newest word of each body is of where it is now.
    bot.holds = None;
    run(&mut world, &network, &mut bot, 64);
    bot.client.poll(&mut bot.wire, &mut bot.heard);
    let mut guests = world.query::<(Entity, &Guest)>();
    let (guest, _) = guests.single(&world).expect("one guest");
    // The client is told its own body exactly, as the mover has it, and the host's to the
    // wire's step, while the host stands where it stood.
    let (mine, other) = bot.mine_and_other();
    let movers = world.resource::<Movers>();
    let own = Own::decoded(&mine.part(OWN).expect("its own body").newer.bytes).expect("reads");
    assert!(
        own.position
            .distance(movers.body(guest).expect("the guest's body").position)
            < 1.0e-9
    );
    let shown =
        Shown::decoded(&other.part(SHOWN).expect("the host, shown").newer.bytes).expect("reads");
    let host_feet = movers.body(host_player).expect("the host's body").position;
    assert!(
        shown.feet.distance(host_feet) < 0.004,
        "the host shown {} m off",
        shown.feet.distance(host_feet)
    );
    assert!(
        other.part(OWN).is_none(),
        "the host's exact body is the host's alone"
    );

    // It leaves: its body leaves the world at once (D637), on the wire too.
    use terra_net::transport::ClientTransport;
    bot.wire.close();
    run(&mut world, &network, &mut bot, 4);
    assert!(world.get_entity(guest).is_err(), "the guest's body stayed");
    assert!(world.resource::<Movers>().body(guest).is_none());
    assert_eq!(world.resource::<Host>().server().clients(), 0);
    assert!(world.get_entity(host_player).is_ok());
}

/// The control beside it (D632): a guest whose client holds nothing stands where it started,
/// so the walking above was the intents' doing; and a steering that is no number is refused.
#[test]
fn a_guest_sending_nothing_stands_still_and_nonsense_steering_is_refused() {
    let (mut world, _, network) = hosted();
    let mut bot = Bot::join(&network, None);
    run(&mut world, &network, &mut bot, 16 + 128);
    let mut guests = world.query::<(&Guest, &Transform)>();
    let (_, transform) = guests.single(&world).expect("one guest");
    let start = world
        .resource::<Planet>()
        .ground_at(Vec3::new(0.0, 2100.0, 3.0));
    assert!(transform.translation.distance(start) < 0.05);

    let steering = Steering {
        intent: Intent::default(),
        view: View {
            forward: Vec3::new(f32::NAN, 0.0, 0.0),
            pitch: 0.0,
        },
    };
    assert!(Steering::decoded(&steering.encoded()).is_err());
    let mut held = Intent::default();
    held.hold(Action::Forward);
    held.wheel = -3;
    let good = Steering {
        intent: held,
        view: View {
            forward: Vec3::X,
            pitch: 0.25,
        },
    };
    assert_eq!(Steering::decoded(&good.encoded()), Ok(good));
}
