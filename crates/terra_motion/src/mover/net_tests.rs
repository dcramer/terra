//! Two men walking together over the network (docs/netcode.md, "Tests"): a server and two
//! clients in one process on `terra_net`'s in-memory transport, 100 ms there and back with
//! jitter and 1% loss, each client predicting its own body with the real mover and seeing the
//! other. The check ([`Walk::mispredicted`]): what a client first predicted for a tick is
//! where the server put the body on that tick, to a millimetre. It has its planted faults
//! beside it (D632).

use super::super::{Bodies, Body, Intent, Kind, Sdf, World};
use super::{Own, Shown};
use glam::{DVec3, Vec3};
use std::collections::BTreeMap;
use terra_net::client::{Client, Event as Heard};
use terra_net::clock::Clock;
use terra_net::identity::Identity;
use terra_net::predict::History;
use terra_net::proto::{EntityId, Kind as NetKind, RoomId};
use terra_net::server::{Audience, Config, Event, PartSpec, Server};
use terra_net::transport::Peer;
use terra_net::transport::loopback::{Conditions, LoopbackClient, LoopbackServer, Network};
use terra_net::wire::Wire;

const TICK: f32 = 1.0 / 64.0;
const VERSION: u64 = 1;
const ROOM: RoomId = RoomId(0);
/// The schema: a body as shown to all, and as its own player is told.
const SHOWN: usize = 0;
const OWN: usize = 1;

/// Level ground at y = 0 on a world so large it is flat here, and no water.
fn with_world<T>(run: impl FnOnce(&World) -> T) -> T {
    let floor = Sdf {
        distance: |at: DVec3| at.y as f32,
        steepest: 1.0,
    };
    let none = |_: Vec3| None;
    run(&World {
        ground: &floor,
        water: &none,
        gravity: 9.81,
        centre: Vec3::new(0.0, -1.0e6, 0.0),
    })
}

/// What a player does on a tick: eight seconds of walking, sprinting, standing, a jump and
/// turns, round again; the second player the same, the other way.
fn script(tick: u32, seat: usize) -> Intent {
    let side = if seat.is_multiple_of(2) { 1.0 } else { -1.0 };
    let (wish, sprint) = match (tick / 64) % 8 {
        0 | 1 => (Vec3::X, false),
        2 => (Vec3::Z, true),
        3 | 6 => (Vec3::ZERO, false),
        4 => (Vec3::NEG_X, false),
        5 => (Vec3::new(0.6, 0.0, -0.8), false),
        _ => (Vec3::NEG_Z, true),
    };
    Intent {
        wish: wish * side,
        sprint,
        jump: (tick / 64) % 8 == 4 && tick % 64 == 5,
        ..Intent::default()
    }
}

/// One tick of one body by an intent: the step a replay runs.
fn stepped(before: &Body, intent: &Intent, world: &World) -> Body {
    let mut alone = Bodies::default();
    alone.put(before.clone());
    alone.tick(world, &[(before.id, *intent)], TICK);
    alone.get(before.id).expect("just put").clone()
}

struct Seat {
    wire: LoopbackClient,
    client: Client,
    clock: Clock,
    history: History<Intent, Body>,
    /// The kind it predicts with: the server's, unless a fault is planted.
    kind: Kind,
    you: Option<u32>,
    body: Option<Body>,
    answered: u32,
    budget: f64,
    /// Where it first predicted its body for each tick.
    first: BTreeMap<u32, DVec3>,
}

struct Walk {
    network: Network,
    wire: LoopbackServer,
    server: Server,
    bodies: Bodies,
    players: Vec<(Peer, u32)>,
    seats: Vec<Seat>,
    /// Where the server put each body on each tick.
    truth: BTreeMap<(u32, u32), DVec3>,
    /// A shove the server gives a body on a tick, which no client can predict.
    shove: Option<(u32, u32, Vec3)>,
}

impl Walk {
    fn new() -> Walk {
        Walk::of(2)
    }

    /// A server and so many players, each over its own rough link.
    fn of(seats: u64) -> Walk {
        let network = Network::new();
        let part = |audience| PartSpec {
            audience,
            weight: 1.0,
        };
        let mut walk = Walk {
            wire: network.server(),
            server: Server::new(
                Config::new(VERSION),
                vec![part(Audience::Public), part(Audience::Owner)],
            ),
            network,
            bodies: Bodies::default(),
            players: Vec::new(),
            seats: Vec::new(),
            truth: BTreeMap::new(),
            shove: None,
        };
        for seat in 0..seats {
            let conditions = Conditions {
                latency_s: 0.05,
                jitter_s: 0.01,
                loss: 0.01,
                copies: 0.01,
                seed: 40 + seat,
            };
            let mut wire = walk.network.connect(conditions);
            let mut client = Client::new();
            client.hello(
                &mut wire,
                VERSION,
                &Identity::from_secret([seat as u8 + 1; 32]),
                "walker",
            );
            walk.seats.push(Seat {
                wire,
                client,
                clock: Clock::default(),
                history: History::default(),
                kind: Kind::man(),
                you: None,
                body: None,
                answered: 0,
                budget: 0.0,
                first: BTreeMap::new(),
            });
        }
        walk
    }

    fn tick(&mut self, world: &World) {
        self.network.advance(f64::from(TICK));
        let mut events = Vec::new();
        self.server.poll(&mut self.wire, &mut events);
        for event in events {
            if let Event::Hello { peer, .. } = event {
                // Ten metres apart, so neither pushes the other: pushes are not predicted.
                let feet = Vec3::new(0.0, 0.0, 10.0 * self.players.len() as f32);
                let id = self
                    .server
                    .spawn(ROOM, NetKind(1), feet.as_dvec3().to_array());
                self.bodies.add(id.0, Kind::man(), feet, Vec3::X, world);
                self.players.push((peer, id.0));
                self.server.welcome(&mut self.wire, peer, ROOM, id, b"");
            }
        }
        let tick = self.server.tick();
        let mut intents: Vec<(u32, Intent)> = self
            .players
            .iter()
            .map(|&(peer, id)| {
                let intent = self
                    .server
                    .intent(peer)
                    .and_then(|bytes| Intent::decoded(bytes).ok());
                (id, intent.unwrap_or_default())
            })
            .collect();
        intents.sort_by_key(|(id, _)| *id);
        if let Some((_, id, velocity)) = self.shove.filter(|(at, ..)| *at == tick) {
            let feet = self.bodies.get(id).expect("a player").feet;
            self.bodies.place(id, feet, velocity, world);
        }
        self.bodies.tick(world, &intents, TICK);
        for body in self.bodies.iter() {
            let id = EntityId(body.id);
            self.server.place(id, body.position.to_array());
            self.server.set_as(id, SHOWN, &body.shown());
            self.server.set_as(id, OWN, &body.own());
            self.truth.insert((body.id, tick), body.position);
        }
        self.server.step(&mut self.wire);

        for (index, seat) in self.seats.iter_mut().enumerate() {
            let mut heard = Vec::new();
            seat.client.poll(&mut seat.wire, &mut heard);
            for event in heard {
                if let Heard::Welcome { tick, you, .. } = event {
                    seat.clock.start(tick);
                    seat.you = you.map(|you| you.0);
                }
            }
            let Some(you) = seat.you else { continue };
            if seat
                .client
                .stamp()
                .is_some_and(|stamp| seat.clock.hear(stamp))
            {
                seat.history.clear();
            }
            // The server's newest answer for its body: taken up at first, checked after.
            let answer = seat
                .client
                .entity(EntityId(you))
                .and_then(|entity| entity.part(OWN));
            if let Some(sample) = answer
                .map(|samples| &samples.newer)
                .filter(|sample| sample.tick > seat.answered)
            {
                seat.answered = sample.tick;
                let own = Own::decoded(&sample.bytes).expect("the server's own bytes");
                match &mut seat.body {
                    None => {
                        let mut alone = Bodies::default();
                        alone.add(you, seat.kind, own.position.as_vec3(), own.facing, world);
                        let mut body = alone.remove(you).expect("just added");
                        body.correct(&own, world);
                        seat.body = Some(body);
                    }
                    Some(body) => {
                        let replayed = seat.history.reconcile(
                            sample.tick,
                            &own,
                            Body::agrees,
                            |body, own| body.correct(own, world),
                            |before, intent| stepped(before, intent, world),
                        );
                        if let Some(replayed) = replayed {
                            *body = replayed;
                        }
                    }
                }
            }
            seat.budget += seat.clock.rate();
            while seat.budget >= 1.0 {
                seat.budget -= 1.0;
                let (Some(tick), Some(body)) = (seat.clock.advance(), &mut seat.body) else {
                    continue;
                };
                let intent = script(tick, index).wired();
                *body = stepped(body, &intent, world);
                seat.first.entry(tick).or_insert(body.position);
                seat.history.record(tick, intent, body.clone());
                seat.client.intent(tick, intent.encoded());
            }
            seat.client.send(&mut seat.wire);
        }
    }

    fn run(&mut self, ticks: u32) {
        with_world(|world| {
            for _ in 0..ticks {
                self.tick(world);
            }
        });
    }

    /// A seat's body, by the number the server gave it. Players are welcomed in the order
    /// their hellos arrive, which is the links', not the seats'.
    fn id(&self, seat: usize) -> u32 {
        self.seats[seat].you.expect("welcomed")
    }

    /// The check: the ticks from one on where a seat's first prediction of its body was more
    /// than a millimetre from where the server put it.
    fn mispredicted(&self, seat: usize, from: u32) -> Vec<u32> {
        let seat = &self.seats[seat];
        let you = seat.you.expect("welcomed");
        seat.first
            .range(from..)
            .filter_map(|(&tick, &predicted)| {
                let truth = self.truth.get(&(you, tick))?;
                (predicted.distance(*truth) > 1.0e-3).then_some(tick)
            })
            .collect()
    }
}

/// The ticks before this are the start: the clock finding its margin.
const WARM: u32 = 192;
const WALKED: u32 = 20 * 64;

#[test]
fn a_predicted_walk_is_the_servers_walk_and_each_sees_the_other() {
    let mut walk = Walk::new();
    // Ended mid-stride: a body at rest is sent nothing, so its newest word would be old.
    let walked = WALKED + 40;
    walk.run(walked);
    for seat in 0..2 {
        // It walked: tens of metres, with a jump off the ground on the way.
        let you = walk.seats[seat].you.expect("welcomed");
        let path: Vec<DVec3> = walk
            .truth
            .range((you, 0)..(you, u32::MAX))
            .map(|(_, at)| *at)
            .collect();
        let far = path
            .iter()
            .map(|at| at.distance(path[0]))
            .fold(0.0, f64::max);
        let high = path.iter().map(|at| at.y).fold(0.0, f64::max);
        assert!(
            far > 10.0 && high > 0.3,
            "seat {seat} went {far:.1} m and {high:.2} m up"
        );
        // Checked on at least fifteen seconds of ticks, and predicted right on every one,
        // with no correction once warm.
        let checked = walk.seats[seat]
            .first
            .range(WARM..)
            .filter(|(tick, _)| walk.truth.contains_key(&(you, **tick)))
            .count();
        assert!(checked > 15 * 64, "only {checked} ticks were checked");
        assert_eq!(
            walk.mispredicted(seat, WARM),
            Vec::<u32>::new(),
            "seat {seat}"
        );
        // The other, as shown: where the server had it on the tick the newest word is of,
        // to the wire's 2 mm a axis.
        let other = walk.id(1 - seat);
        let entity = walk.seats[seat]
            .client
            .entity(EntityId(other))
            .expect("the other is near");
        let sample = &entity.part(SHOWN).expect("shown").newer;
        let shown = Shown::decoded(&sample.bytes).expect("the server's bytes");
        let truth = walk.truth[&(other, sample.tick)];
        assert!(
            shown.feet.distance(truth) < 0.004,
            "shown {:?}, was at {truth:?}",
            shown.feet
        );
        assert!(
            walk.server.tick() - sample.tick < 16,
            "the newest word of the other is {} ticks old",
            walk.server.tick() - sample.tick
        );
        // Between its last two words the other is drawn where the server had it on the tick
        // between, to a centimetre: a walk is straight over two ticks.
        let samples = entity.part(SHOWN).expect("shown");
        let older = samples.older.as_ref().expect("two words of a walking body");
        assert!(
            samples.newer.tick - older.tick >= 2,
            "sent every second tick"
        );
        let mid = older.tick + 1;
        let (from, to, share) = samples.around(f64::from(mid));
        let (from, to) = (
            Shown::decoded(&from.bytes).expect("reads"),
            Shown::decoded(&to.bytes).expect("reads"),
        );
        assert!(share > 0.0 && share < 1.0);
        let drawn = from.between(&to, share);
        assert!(
            drawn.feet.distance(walk.truth[&(other, mid)]) < 0.01,
            "drawn {:?}",
            drawn.feet
        );
        // Its own exact body is told to it alone.
        assert!(entity.part(OWN).is_none());
    }
    let corrections: u64 = walk.seats.iter().map(|seat| seat.history.corrections).sum();
    let traffic = walk.network.traffic();
    let seconds = f64::from(walked) * f64::from(TICK);
    // Readings, against netcode.md's first targets (8 kB/s down, 4 kB/s up).
    eprintln!(
        "walk: {:.0} B/s down and {:.0} B/s up a player (payload, two men, 100 ms, 1% loss); {corrections} corrections in all",
        (traffic.down_state + traffic.down_ordered) as f64 / seconds / 2.0,
        (traffic.up_state + traffic.up_ordered) as f64 / seconds / 2.0,
    );
}

/// The Server track's bar (docs/v0-tracks.md), on the real mover: four clients for thirty
/// minutes at 100 ms and 1% loss, with no lasting mismatch. Here no mismatch at all: every
/// tick each client predicted is the server's, though the players walk out of each other's
/// sight and the clocks run for half an hour.
#[test]
fn four_men_for_half_an_hour_are_never_mispredicted() {
    let began = std::time::Instant::now();
    let mut walk = Walk::of(4);
    let ticks = 30 * 60 * 64;
    walk.run(ticks);
    for seat in 0..4 {
        let you = walk.seats[seat].you.expect("welcomed");
        let checked = walk.seats[seat]
            .first
            .keys()
            .filter(|tick| **tick >= WARM && walk.truth.contains_key(&(you, **tick)))
            .count();
        assert!(
            checked > 29 * 60 * 64,
            "seat {seat}: only {checked} ticks were checked"
        );
        assert_eq!(
            walk.mispredicted(seat, WARM),
            Vec::<u32>::new(),
            "seat {seat}"
        );
    }
    // Those who walk the same way stay in sight: seat 0 holds seat 2 where the server has it.
    let other = walk.id(2);
    let entity = walk.seats[0]
        .client
        .entity(EntityId(other))
        .expect("seat 2 is near seat 0");
    let sample = &entity.part(SHOWN).expect("shown").newer;
    let shown = Shown::decoded(&sample.bytes).expect("the server's bytes");
    assert!(shown.feet.distance(walk.truth[&(other, sample.tick)]) < 0.004);
    // Those who walk apart are out of it, and forgotten.
    let gone = walk.id(1);
    assert!(walk.seats[0].client.entity(EntityId(gone)).is_none());
    let corrections: u64 = walk.seats.iter().map(|seat| seat.history.corrections).sum();
    let traffic = walk.network.traffic();
    let seconds = f64::from(ticks) * f64::from(TICK);
    eprintln!(
        "soak on the mover: {:.0} B/s down and {:.0} B/s up a player (payload, four men, 100 ms, 1% loss, 30 min); {corrections} corrections; {:.1} s",
        (traffic.down_state + traffic.down_ordered) as f64 / seconds / 4.0,
        (traffic.up_state + traffic.up_ordered) as f64 / seconds / 4.0,
        began.elapsed().as_secs_f64(),
    );
}

/// A shove from the server, which no client can foresee: mispredicted, corrected, and right
/// again within a round trip and a half.
#[test]
fn a_shove_is_corrected_within_a_round_trip() {
    let mut walk = Walk::new();
    walk.run(32);
    let shoved = walk.id(0);
    let at = 640;
    walk.shove = Some((at, shoved, Vec3::new(0.0, 4.0, 6.0)));
    walk.run(WALKED);
    let wrong = walk.mispredicted(0, WARM);
    assert!(
        !wrong.is_empty(),
        "the shove was predicted, which cannot be"
    );
    assert!(
        wrong.iter().all(|tick| (at..at + 24).contains(tick)),
        "mispredicted on {wrong:?}, shoved on {at}"
    );
    assert!(walk.seats[0].history.corrections >= 1);
    // The other was not shoved and never wrong.
    assert_eq!(walk.mispredicted(1, WARM), Vec::<u32>::new());
}

/// The planted fault (D632): a client whose mover is not the server's (a man a tenth faster).
/// Its predictions are wrong whenever it walks, and the check must say so.
#[test]
fn a_client_with_another_mover_fails_the_check() {
    let mut walk = Walk::new();
    walk.seats[0].kind.jog *= 1.1;
    walk.seats[0].kind.sprint *= 1.1;
    walk.run(WALKED);
    let wrong = walk.mispredicted(0, WARM).len();
    assert!(wrong > 300, "only {wrong} ticks were found mispredicted");
    assert!(walk.seats[0].history.corrections > 300);
    assert_eq!(walk.mispredicted(1, WARM), Vec::<u32>::new());
}

#[test]
fn a_body_and_an_intent_read_back() {
    let shown = Shown {
        feet: DVec3::new(-14_999.123, 250.5, 0.001),
        velocity: Vec3::new(4.5, -9.0, 0.0),
        facing: Vec3::new(0.6, 0.0, -0.8),
        ground: super::Ground::Airborne,
    };
    let back = Shown::decoded(&shown.encoded()).expect("reads");
    assert!(
        back.feet.distance(shown.feet) < 0.002 && back.velocity.distance(shown.velocity) < 0.02
    );
    assert!(back.facing.distance(shown.facing) < 0.003 && back.ground == shown.ground);
    assert_eq!(shown.encoded().len(), 19);
    // Standing still is a byte; an intent is the same after one trip as after two.
    assert_eq!(Intent::default().encoded().len(), 1);
    assert_eq!(Intent::default().wired().wish, Vec3::ZERO);
    let intent = Intent {
        wish: Vec3::new(0.6, 0.0, -0.8),
        sprint: true,
        scale: 0.5,
        pitch: -0.3,
        drive: Some(Vec3::new(1.0, 0.0, 2.0)),
        ..Intent::default()
    };
    let (once, twice) = (intent.wired(), intent.wired().wired());
    assert_eq!(once.encoded(), twice.encoded());
    assert!(once.wish.distance(intent.wish) < 0.003 && once.sprint && once.drive == intent.drive);
    assert!((once.scale - 0.5).abs() < 0.002 && (once.pitch + 0.3).abs() < 0.004);
    // A drive that is no number is refused.
    let bad = Intent {
        drive: Some(Vec3::new(f32::NAN, 0.0, 0.0)),
        ..Intent::default()
    };
    assert!(Intent::decoded(&bad.encoded()).is_err());
}
