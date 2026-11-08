//! A hosted walker world and a client that is no game (D73): the world's own walk moves the
//! guest by the intents its client sends, and the client is told its own body and the host's.

#[path = "net_tests/leaving.rs"]
mod leaving;

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
    world.init_resource::<ground::Ground>();
    world.init_resource::<worn::Said>();
    world.init_resource::<looks::Said>();
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
            if let Heard::Welcome { tick, you, .. } = heard {
                self.clock.start(*tick);
                // Greeted: a bot has no land to load, so it is ready at once.
                if you.is_none() {
                    self.client.ready(&mut self.wire);
                }
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
    assert!(
        matches!(bot.heard.first(), Some(Heard::Welcome { realm, .. }) if greeted(realm) == (b"realm", &[]))
    );
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
    let apart = own
        .position
        .distance(movers.body(guest).expect("the guest's body").position);
    // Exact but for what a body at rest still settles between two ticks: nanometres.
    assert!(
        apart < 1.0e-6,
        "its own body is told {apart} m from where the mover has it"
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
            // Along nothing: no way to look.
            forward: Vec3::ZERO,
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
    // It reads back to the wire's step, and the same from then on: what both ends act on.
    let once = good.wired();
    assert_eq!(once.wired(), once);
    assert_eq!(once.intent, good.intent);
    assert!(once.view.forward.distance(Vec3::X) < 0.001 && (once.view.pitch - 0.25).abs() < 0.001);
    assert!(
        good.encoded().len() <= 10,
        "a steering is {} bytes",
        good.encoded().len()
    );
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
    world.init_resource::<ground::Ground>();
    world.init_resource::<worn::Said>();
    world.init_resource::<looks::Said>();
    world.init_resource::<Messages<crate::movement::Fell>>();
    world.init_resource::<Messages<crate::movement::Drowned>>();
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs_f32(TICK));
    world.insert_resource(time);
    let identity = Identity::from_secret([6; 32]);
    let wire = Box::new(network.connect(conditions));
    world.insert_resource(link::Link::join(
        wire,
        VERSION,
        &identity,
        "guest",
        b"realm".to_vec(),
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
                self.client.run_system_once(ground::joined).unwrap();
                self.client.run_system_once(worn::joined).unwrap();
                self.client.run_system_once(looks::joined).unwrap();
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
            self.host.run_system_once(ground::host).unwrap();
            self.host.run_system_once(worn::host).unwrap();
            self.host.run_system_once(looks::host).unwrap();
            self.host.run_system_once(tell).unwrap();
            let mut guests = self.host.query_filtered::<Entity, With<Guest>>();
            if let Some(guest) = guests.iter(&self.host).next()
                && let Some(body) = self.host.resource::<Movers>().body(guest)
            {
                self.truth.insert(tick, body.position);
            }
        }
    }

    /// The joined game's clock held for so many of the host's ticks (a title screen, a load):
    /// it walks nothing and only keeps its link, a frame a tick, while the host plays on.
    fn hold(&mut self, ticks: u32, keeps: bool) {
        let mut real = Time::<Real>::default();
        for _ in 0..ticks {
            self.network.advance(f64::from(TICK));
            if keeps {
                real.advance_by(std::time::Duration::from_secs_f32(TICK));
                self.client.insert_resource(real);
                self.client.run_system_once(link::wait).unwrap();
            }
            self.host.run_system_once(hear).unwrap();
            self.host.run_system_once(walk).unwrap();
            self.host.run_system_once(tell).unwrap();
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

/// One nudge from the host, mid-walk, which the joined game cannot foresee (a shove, a late
/// intent): corrected once, within a round trip, and then the walk is the host's again. A
/// correction must not leave the body so changed that the next tick is wrong too, and the
/// next: on a loaded machine that chain made a walk of the game's 160 corrections long.
#[test]
fn a_nudge_is_corrected_within_a_round_trip_and_starts_no_chain() {
    let mut together = Together::new(rough());
    together.run(0..100);
    let before = together.client.resource::<link::Link>().corrections();
    // The guest on the host, pushed a hand's width aside while it walks.
    let mut guests = together
        .host
        .query_filtered::<&mut Transform, With<Guest>>();
    let mut guest = guests.single_mut(&mut together.host).expect("one guest");
    let aside = guest.translation.normalize().cross(Vec3::X) * 0.08;
    guest.translation += aside;
    let nudged = together.host.resource::<Host>().server().tick();
    together.run(100..300);
    let corrections = together.client.resource::<link::Link>().corrections() - before;
    let wrong = together.mispredicted(nudged);
    assert!(!wrong.is_empty(), "the nudge was foreseen, which cannot be");
    assert!(
        wrong.iter().all(|tick| *tick < nudged + 24),
        "mispredicted on {wrong:?}, nudged on {nudged}"
    );
    assert!(corrections <= 3, "{corrections} corrections for one nudge");
}

/// A game that made another world than the host's is not kept in it: the land would not be
/// the same under the two. The link ends at the welcome, with words for the player.
#[test]
fn a_game_of_another_world_is_told_so_and_leaves() {
    let (mut host, _, network) = hosted();
    let (mut client, _) = joined(&network, Conditions::PERFECT);
    let identity = Identity::from_secret([8; 32]);
    let wire = Box::new(network.connect(Conditions::PERFECT));
    client.insert_resource(link::Link::join(
        wire,
        VERSION,
        &identity,
        "lost",
        b"another".to_vec(),
    ));
    for _ in 0..40 {
        network.advance(f64::from(TICK));
        client.run_system_once(link::listen).unwrap();
        if client.contains_resource::<link::Link>() {
            client.run_system_once(link::record).unwrap();
        }
        host.run_system_once(hear).unwrap();
        host.run_system_once(walk).unwrap();
        host.run_system_once(tell).unwrap();
    }
    assert!(
        !client.contains_resource::<link::Link>(),
        "it stayed in a world that is not its own"
    );
    let why = &client.resource::<link::Ended>().0;
    assert!(why.contains("another seed"), "{why}");
    // The host sees it go: one who left is no guest.
    let mut guests = host.query::<&Guest>();
    assert_eq!(guests.iter(&host).count(), 0);
}

/// The ground across the network (D21): what the host digs, a joined game's ground has; what
/// a joined game digs, the host's has, and the joined game's has once, not twice; and a dig
/// asked for far from the player is refused (D51).
#[test]
fn a_dig_on_either_side_is_in_both_grounds_once() {
    use terra_core::Operation;
    let mut together = Together::new(rough());
    together.run(0..60);
    let count = |world: &World| world.resource::<Planet>().operation_count();
    let depth = |world: &World, at: Vec3| world.resource::<Planet>().distance(at);
    let guest_at = together
        .client
        .get::<Transform>(together.player)
        .unwrap()
        .translation;
    let host_at = together
        .host
        .get::<Transform>(together.host_player)
        .unwrap()
        .translation;

    // The host digs beside its own player, three metres from the guest's start.
    let hosts = Operation::Dig {
        centre: host_at + Vec3::X * 1.5,
        radius: 0.6,
    };
    together.host.resource_mut::<Planet>().apply(&hosts);
    together.run(60..90);
    assert_eq!(
        (count(&together.host), count(&together.client)),
        (1, 1),
        "the host's dig did not reach the joined game"
    );
    let at = host_at + Vec3::X * 1.5;
    assert!(
        depth(&together.client, at) > 0.3,
        "the joined game's ground is not dug there"
    );
    assert_eq!(depth(&together.client, at), depth(&together.host, at));

    // The joined game digs beside its own player.
    let guests = Operation::Dig {
        centre: guest_at + Vec3::X * 1.0,
        radius: 0.6,
    };
    together.client.resource_mut::<Planet>().apply(&guests);
    assert_eq!(count(&together.client), 2, "its own dig shows at once");
    together.run(90..130);
    assert_eq!(
        (count(&together.host), count(&together.client)),
        (2, 2),
        "dug once on each side"
    );
    assert_eq!(
        together.client.resource::<ground::Ground>().pending(),
        0,
        "its own dig came back"
    );
    assert_eq!(
        together.host.resource::<Planet>().operations(),
        together.client.resource::<Planet>().operations()
    );

    // And asks for one half a kilometre off, which no tool reaches: the host refuses.
    let far = Operation::Dig {
        centre: guest_at + Vec3::X * 500.0,
        radius: 0.6,
    };
    together.client.resource_mut::<Planet>().apply(&far);
    together.run(130..170);
    assert_eq!(
        count(&together.host),
        2,
        "the host made a dig out of the guest's reach"
    );
    assert_eq!(
        together.client.resource::<ground::Ground>().pending(),
        1,
        "the refused dig is still waited on"
    );
}

/// An operation from a stranger is held to what could be asked: numbers that are none, a
/// size past the most, a turn that is no turn.
#[test]
fn an_operation_that_could_not_be_asked_is_refused() {
    use terra_core::{MaterialId, Operation};
    use terra_net::wire::Wire;
    let good = [
        Operation::Dig {
            centre: Vec3::new(1.0, 2.0, 3.0),
            radius: 0.5,
        },
        Operation::DigBox {
            centre: Vec3::ONE,
            half: Vec3::splat(2.0),
            rotation: Quat::from_rotation_y(0.7),
        },
        Operation::Fill {
            centre: Vec3::ONE,
            radius: 1.0,
            material: MaterialId(3),
        },
        Operation::ClearCover {
            centre: Vec3::ONE,
            radius: 4.0,
        },
    ];
    for operation in good {
        assert_eq!(Operation::decoded(&operation.encoded()), Ok(operation));
    }
    let bad = [
        Operation::Dig {
            centre: Vec3::new(f32::NAN, 0.0, 0.0),
            radius: 0.5,
        },
        Operation::Dig {
            centre: Vec3::ONE,
            radius: 5000.0,
        },
        Operation::Dig {
            centre: Vec3::ONE,
            radius: -1.0,
        },
        Operation::DigBox {
            centre: Vec3::ONE,
            half: Vec3::ONE,
            rotation: Quat::from_xyzw(0.0, 0.0, 0.0, 9.0),
        },
    ];
    for operation in bad {
        assert!(
            Operation::decoded(&operation.encoded()).is_err(),
            "{operation:?} was let through"
        );
    }
}

/// What each player wears crosses both ways: the host's player's outfit is on its puppet in
/// the joined game, the joined player's on its guest on the host, and a change follows.
#[test]
fn each_player_is_dressed_as_its_player_is_in_the_others_game() {
    use crate::character::{Clothing, Outfit};
    let wears = |items: &[&str]| Outfit(items.iter().map(|item| Clothing::new(*item)).collect());
    let mut together = Together::new(rough());
    together
        .host
        .entity_mut(together.host_player)
        .insert(wears(&["vest", "pack"]));
    together
        .client
        .entity_mut(together.player)
        .insert(wears(&["hat"]));
    together.run(0..90);
    let mut puppets = together
        .client
        .query_filtered::<&Outfit, With<link::Puppet>>();
    assert_eq!(
        puppets.single(&together.client).expect("the host's puppet"),
        &wears(&["vest", "pack"])
    );
    let mut guests = together.host.query_filtered::<&Outfit, With<Guest>>();
    assert_eq!(
        guests.single(&together.host).expect("the guest"),
        &wears(&["hat"])
    );
    // Each takes something off, and puts something on.
    together
        .host
        .entity_mut(together.host_player)
        .insert(wears(&["vest"]));
    together
        .client
        .entity_mut(together.player)
        .insert(wears(&["hat", "cloak"]));
    together.run(90..150);
    assert_eq!(
        puppets.single(&together.client).expect("the host's puppet"),
        &wears(&["vest"])
    );
    assert_eq!(
        guests.single(&together.host).expect("the guest"),
        &wears(&["hat", "cloak"])
    );
    // An outfit of too many pieces, or a name too long, is held to the wire's most.
    use terra_net::wire::Wire;
    let long = worn::Wearing(vec!["x".repeat(65)]);
    assert!(worn::Wearing::decoded(&long.encoded()).is_err());
}

/// A game that joins from its title screen holds its clock while it makes the land the host
/// welcomed it to (v0's Front): the link is kept all the while, the realm is known, and when
/// its ticks begin again its walk is the host's from the first second.
#[test]
fn a_link_kept_while_the_clock_is_held_walks_with_the_host_again() {
    let mut pair = Together::new(rough());
    pair.run(0..128);
    assert_eq!(
        pair.client.resource::<link::Link>().realm(),
        Some(&b"realm"[..]),
        "the realm the host welcomed it to is known"
    );
    // Ten seconds held.
    pair.hold(640, true);
    assert!(
        pair.client.contains_resource::<link::Link>(),
        "the link was lost while held: {:?}",
        pair.client
            .get_resource::<link::Ended>()
            .map(|ended| &ended.0)
    );
    pair.run(768..1408);
    let resumed = pair.host.resource::<Host>().server().tick() - 640 + 64;
    assert!(pair.checked(resumed) > 400, "{}", pair.checked(resumed));
    assert_eq!(pair.mispredicted(resumed), Vec::<u32>::new());
}

/// The planted fault (D632): held without keeping its link, the game's clock is ten seconds
/// behind the host's when its ticks begin again, and the same check must fail it.
#[test]
fn a_link_not_kept_while_held_fails_the_check() {
    let mut pair = Together::new(rough());
    pair.run(0..128);
    pair.hold(640, false);
    pair.run(768..1408);
    let resumed = pair.host.resource::<Host>().server().tick() - 640 + 64;
    assert!(
        pair.checked(resumed) < 400 || !pair.mispredicted(resumed).is_empty(),
        "a clock ten seconds behind walked with the host"
    );
}

/// Two players tell each other apart (D111): each looks, in the other's game, as its player
/// made it, a change follows, and a name too long for the wire is refused.
#[test]
fn each_player_looks_in_the_others_game_as_its_player_made_it() {
    use crate::creator::{Features, creator};
    let made = |hair: Option<&str>, skin: &str, eyes: &str| Features {
        body: "male".into(),
        hair: hair.map(str::to_string),
        beard: None,
        hair_colour: creator().hair[1].name.clone(),
        skin: skin.into(),
        eyes: eyes.into(),
    };
    let (hosts, mine) = (
        made(Some("male_hair_dreads"), "Deep", "Brown"),
        made(None, "Pale", "Blue"),
    );
    let mut together = Together::new(rough());
    let (host_player, player) = (together.host_player, together.player);
    together.host.entity_mut(host_player).insert(hosts.clone());
    together.client.entity_mut(player).insert(mine.clone());
    together.run(0..90);
    let mut puppets = together
        .client
        .query_filtered::<&Features, With<link::Puppet>>();
    let mut guests = together.host.query_filtered::<&Features, With<Guest>>();
    assert_eq!(
        puppets.single(&together.client).expect("the host's puppet"),
        &hosts
    );
    assert_eq!(guests.single(&together.host).expect("the guest"), &mine);
    // The joined player grows a beard; the host shaves its head.
    let (bearded, shaved) = (
        Features {
            beard: Some("male_beard_full".into()),
            ..mine
        },
        Features {
            hair: None,
            ..hosts
        },
    );
    together.client.entity_mut(player).insert(bearded.clone());
    together.host.entity_mut(host_player).insert(shaved.clone());
    together.run(90..150);
    assert_eq!(
        puppets.single(&together.client).expect("the host's puppet"),
        &shaved
    );
    assert_eq!(guests.single(&together.host).expect("the guest"), &bearded);
    use terra_net::wire::Wire;
    assert_eq!(Features::decoded(&bearded.encoded()), Ok(bearded.clone()));
    let long = Features {
        skin: "x".repeat(65),
        ..bearded
    };
    assert!(Features::decoded(&long.encoded()).is_err());
}

/// A game that has joined and is still making the host's land (its clock held) is known to
/// the host and has no body there: nobody sees one stand idle. Its body is made when its
/// first tick is walked (D639). Before this the body stood from the hello on.
#[test]
fn a_joining_game_has_no_body_on_the_host_until_its_world_is_made() {
    let mut pair = Together::new(Conditions::PERFECT);
    // Three seconds of loading: the link kept, no tick walked.
    pair.hold(192, true);
    let mut guests = pair.host.query_filtered::<Entity, With<Guest>>();
    assert_eq!(
        guests.iter(&pair.host).count(),
        0,
        "a body stood while it loaded"
    );
    assert_eq!(
        pair.host.resource::<Host>().server().clients(),
        1,
        "but it is in"
    );
    assert_eq!(
        pair.client.resource::<link::Link>().realm(),
        Some(&b"realm"[..]),
        "and knows the land to make"
    );
    // Its world made, its ticks begin: its body stands within the second.
    pair.run(192..256);
    assert_eq!(guests.iter(&pair.host).count(), 1);
    let mut puppets = pair.client.query_filtered::<Entity, With<link::Puppet>>();
    assert_eq!(
        puppets.iter(&pair.client).count(),
        1,
        "and it sees the host"
    );
}

#[path = "net/instance_tests.rs"]
mod realms;
