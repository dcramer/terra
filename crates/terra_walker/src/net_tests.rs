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
use terra_net::transport::loopback::{Conditions, Network};
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
    wire: Box<dyn terra_net::transport::ClientTransport>,
    client: Client,
    clock: Clock,
    heard: Vec<Heard>,
    holds: Option<Action>,
}

impl Bot {
    fn join(network: &Network, holds: Option<Action>) -> Bot {
        Bot::over(Box::new(network.connect(Conditions::PERFECT)), holds)
    }

    fn over(wire: Box<dyn terra_net::transport::ClientTransport>, holds: Option<Action>) -> Bot {
        let mut bot = Bot {
            wire,
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
        use terra_net::transport::ClientTransport;
        self.wire.advance(super::TICK);
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

/// A game that joined: a world of its own on the same planet, this machine's player standing
/// anywhere on it, and a link to the host's server.
fn joined(network: &Network, conditions: Conditions) -> (World, Entity) {
    let planet = Planet::smooth(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    let somewhere = planet.ground_at(Vec3::new(40.0, 2100.0, -20.0));
    let mut world = World::new();
    world.insert_resource(planet);
    world.init_resource::<flora::Obstacles>();
    world.init_resource::<Movers>();
    world.init_resource::<Messages<crate::movement::Fell>>();
    world.init_resource::<Messages<crate::movement::Drowned>>();
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs_f32(TICK));
    world.insert_resource(time);
    let identity = Identity::from_secret([6; 32]);
    world.insert_resource(link::Link::join(
        Box::new(network.connect(conditions)),
        VERSION,
        &identity,
        "guest",
    ));
    let view = View {
        forward: Vec3::X,
        pitch: 0.0,
    };
    let player = (
        Player,
        crate::LocalPlayer,
        Transform::from_translation(somewhere),
        Motion::facing(Vec3::X),
        Stance::default(),
        Stamina::default(),
        view,
        Intent::default(),
    );
    let player = world.spawn(player).id();
    (world, player)
}

/// What the joined player's keys do on a step: forward, a sprint, a jump on the way, across,
/// a stop, back.
fn keys(step: u32) -> Intent {
    let mut intent = Intent::default();
    let held: &[Action] = match step {
        40..=167 => &[Action::Forward],
        168..=231 => &[Action::Forward, Action::Sprint],
        232..=295 => &[Action::Left],
        330..=420 => &[Action::Back],
        _ => &[],
    };
    for &action in held {
        intent.hold(action);
    }
    if step == 120 || step == 250 {
        intent.hold(Action::Jump);
    }
    intent
}

/// A host's world and a joined game's, run side by side: where the joined game predicted its
/// player on each tick, and where the host's walk put that player's body on the same tick.
struct Together {
    host: World,
    client: World,
    host_player: Entity,
    player: Entity,
    network: Network,
    budget: f64,
    predicted: std::collections::BTreeMap<u32, bevy::math::DVec3>,
    truth: std::collections::BTreeMap<u32, bevy::math::DVec3>,
}

impl Together {
    fn new(conditions: Conditions) -> Together {
        let (host, host_player, network) = hosted();
        let (client, player) = joined(&network, conditions);
        Together {
            host,
            client,
            host_player,
            player,
            network,
            budget: 0.0,
            predicted: Default::default(),
            truth: Default::default(),
        }
    }

    fn run(&mut self, steps: std::ops::Range<u32>) {
        for step in steps {
            self.network.advance(f64::from(TICK));
            // The joined game's ticks, at its clock's rate.
            self.budget += self.client.resource::<link::Link>().rate();
            while self.budget >= 1.0 {
                self.budget -= 1.0;
                *self.client.get_mut::<Intent>(self.player).unwrap() = keys(step);
                self.client.run_system_once(link::listen).unwrap();
                self.client.run_system_once(walk).unwrap();
                self.client.run_system_once(link::record).unwrap();
                self.client.run_system_once(link::show).unwrap();
                let tick = self.client.resource::<link::Link>().tick();
                let body = self.client.resource::<Movers>().body(self.player);
                if let (Some(tick), Some(body)) = (tick, body) {
                    self.predicted.entry(tick).or_insert(body.position);
                }
            }
            // The host's tick.
            let tick = self.host.resource::<Host>().server().tick();
            self.host.run_system_once(hear).unwrap();
            self.host.run_system_once(walk).unwrap();
            self.host.run_system_once(tell).unwrap();
            let mut guests = self.host.query_filtered::<Entity, With<Guest>>();
            if let Some(guest) = guests.iter(&self.host).next()
                && let Some(body) = self.host.resource::<Movers>().body(guest)
            {
                self.truth.insert(tick, body.position);
            }
        }
    }

    /// The check (D632's faults are beside it): the ticks from one on where the joined game's
    /// first prediction of its player was over a millimetre from where the host put it.
    fn mispredicted(&self, from: u32) -> Vec<u32> {
        let wrong = self
            .predicted
            .range(from..)
            .filter_map(|(tick, predicted)| {
                let truth = self.truth.get(tick)?;
                (predicted.distance(*truth) > 1.0e-3).then_some(*tick)
            });
        wrong.collect()
    }

    fn checked(&self, from: u32) -> usize {
        self.predicted
            .range(from..)
            .filter(|(tick, _)| self.truth.contains_key(tick))
            .count()
    }
}

/// A link of 100 ms there and back, with jitter, 1% lost and 1% copied.
fn rough() -> Conditions {
    Conditions {
        latency_s: 0.05,
        jitter_s: 0.01,
        loss: 0.01,
        copies: 0.01,
        seed: 77,
    }
}

/// The ticks before this are the start: joining, and the clock finding its margin.
const WARM: u32 = 150;

#[test]
fn a_joined_games_walk_is_the_hosts_walk_and_it_sees_the_host() {
    let mut together = Together::new(rough());
    together.run(0..480);
    // It was taken to where the host starts guests, and walked from there by its own keys:
    // far, off the ground once, and every tick it predicted was the host's.
    let start = together
        .host
        .resource::<Planet>()
        .ground_at(Vec3::new(0.0, 2100.0, 3.0));
    let path: Vec<_> = together.truth.values().copied().collect();
    let far = path
        .iter()
        .map(|at| at.as_vec3().distance(start))
        .fold(0.0, f32::max);
    let high = path
        .iter()
        .map(|at| at.length() - f64::from(start.length()))
        .fold(0.0, f64::max);
    assert!(
        far > 8.0 && high > 0.3,
        "the guest went {far:.1} m and {high:.2} m up"
    );
    assert!(
        together.checked(WARM) > 250,
        "only {} ticks were checked",
        together.checked(WARM)
    );
    assert_eq!(together.mispredicted(WARM), Vec::<u32>::new());
    let link = together.client.resource::<link::Link>();
    assert!(
        link.corrections() < 8,
        "{} corrections, all at the start",
        link.corrections()
    );
    // The host's player is a puppet in the joined world, where the host stands.
    let host_feet = together
        .host
        .resource::<Movers>()
        .body(together.host_player)
        .expect("the host's body")
        .position;
    let mut puppets = together.client.query::<(&link::Puppet, &Transform)>();
    let (puppet, transform) = puppets
        .single(&together.client)
        .expect("one puppet: the host");
    assert!(transform.translation.distance(host_feet.as_vec3()) < 0.01);
    assert!(puppet.shown.feet.distance(host_feet) < 0.004);
}

/// The planted fault (D632): a joined game whose man is a tenth faster than the host's. Its
/// predictions are wrong whenever it walks; the check must say so, and the corrections keep
/// it within a round trip of the host all the same.
#[test]
fn a_joined_game_with_another_mover_fails_the_check_and_is_held_to_the_host() {
    let mut together = Together::new(rough());
    let mut movers = together.client.resource_mut::<Movers>();
    movers.kind_mut().jog *= 1.1;
    movers.kind_mut().sprint *= 1.1;
    together.run(0..480);
    let wrong = together.mispredicted(WARM).len();
    assert!(wrong > 100, "only {wrong} ticks were found mispredicted");
    let corrections = together.client.resource::<link::Link>().corrections();
    assert!(corrections > 50, "only {corrections} corrections");
    // Wrong by a tenth for a round trip at most: never a metre from the host's word.
    let worst = together
        .predicted
        .iter()
        .filter_map(|(tick, predicted)| Some(predicted.distance(*together.truth.get(tick)?)));
    let worst = worst.skip(WARM as usize).fold(0.0, f64::max);
    assert!(worst < 1.0, "{worst} m from the host at worst");
}

/// The same host over this machine's own sockets (D34): the transport's time is the tick's,
/// said by `hear`, so a hosted game needs nothing more than a UDP server for its wire.
#[test]
fn a_guest_joins_a_hosted_world_over_real_sockets() {
    use terra_net::transport::udp::{UdpClient, UdpServer};
    let (mut world, _, _) = hosted();
    let wire = UdpServer::bind((std::net::Ipv4Addr::LOCALHOST, 0).into(), 4).expect("a free port");
    let address = wire.address();
    let start = world
        .resource::<Planet>()
        .ground_at(Vec3::new(0.0, 2100.0, 3.0));
    world.insert_resource(Host::new(
        Config::new(VERSION),
        parts(),
        Box::new(wire),
        start,
        Vec::new(),
    ));
    let mut bot = Bot::over(
        Box::new(UdpClient::connect(address, 7).expect("a socket")),
        Some(Action::Forward),
    );
    for _ in 0..200 {
        bot.tick();
        world.run_system_once(hear).unwrap();
        world.run_system_once(walk).unwrap();
        world.run_system_once(tell).unwrap();
        // The machine's own sockets deliver at once, given a moment.
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut guests = world.query::<(&Guest, &Transform)>();
    let (_, transform) = guests.single(&world).expect("one guest");
    let walked = (transform.translation - start).dot(Vec3::X);
    assert!(walked > 5.0, "the guest walked {walked} m");
    let (mine, _) = bot.mine_and_other();
    assert!(mine.part(OWN).is_some());
}
