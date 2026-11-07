//! Rooms, crossings and things that happen once (instances.md, D652): a client moved from one
//! room to another holds the new room and nothing of the old, though packets of the old are
//! still on their way; an event is heard once by those near it and by nobody else.

use super::*;

const HERE: RoomId = RoomId(1);
const THERE: RoomId = RoomId(2);

struct Guest {
    tap: LoopbackClient,
    client: Client,
    heard: Vec<Heard>,
}

impl Guest {
    /// What it has heard since it was last let into a room.
    fn since_welcome(&self) -> &[Heard] {
        let last = self
            .heard
            .iter()
            .rposition(|heard| matches!(heard, Heard::Welcome { .. }));
        &self.heard[last.expect("welcomed")..]
    }

    fn holds(&self) -> Vec<EntityId> {
        let mut ids: Vec<EntityId> = self.client.entities().map(|(id, _)| id).collect();
        ids.sort();
        ids
    }

    /// The operations it was sent since its last welcome, as (number, bytes).
    fn operations(&self) -> Vec<(u32, Vec<u8>)> {
        let ops = self.since_welcome().iter().filter_map(|heard| match heard {
            Heard::Op { number, bytes, .. } => Some((*number, bytes.clone())),
            _ => None,
        });
        ops.collect()
    }
}

/// A server of one part and its guests, each over a rough link.
struct Hall {
    network: Network,
    wire: LoopbackServer,
    server: Server,
    guests: Vec<Guest>,
    hellos: Vec<Peer>,
    said: Vec<(Peer, String)>,
}

impl Hall {
    fn new() -> Hall {
        let network = Network::new();
        let part = PartSpec {
            audience: Audience::Public,
            weight: 1.0,
        };
        Hall {
            wire: network.server(),
            network,
            server: Server::new(Config::new(VERSION), vec![part]),
            guests: Vec::new(),
            hellos: Vec::new(),
            said: Vec::new(),
        }
    }

    fn join(&mut self, conditions: Conditions) {
        let mut guest = Guest {
            tap: self.network.connect(conditions),
            client: Client::new(),
            heard: Vec::new(),
        };
        let identity = Identity::from_secret([self.guests.len() as u8 + 1; 32]);
        guest
            .client
            .hello(&mut guest.tap, VERSION, &identity, "guest");
        self.guests.push(guest);
    }

    /// A body for a peer at a place in a room, and the peer let into the room as it.
    fn seat(&mut self, peer: Peer, room: RoomId, at: Place) -> EntityId {
        let you = self.server.spawn(room, PLAYER, at);
        self.server.set(you, 0, b"body");
        self.server.welcome(
            &mut self.wire,
            peer,
            room,
            you,
            format!("room {}", room.0).as_bytes(),
        );
        you
    }

    /// Ticks, with something the test does to the server before each tick ends. Hellos are
    /// gathered for the test to answer.
    fn run(&mut self, ticks: u32, mut each: impl FnMut(&mut Server, u32)) {
        for _ in 0..ticks {
            self.network.advance(TICK_S);
            let mut events = Vec::new();
            self.server.poll(&mut self.wire, &mut events);
            for event in events {
                match event {
                    Event::Hello { peer, .. } => self.hellos.push(peer),
                    Event::Said { peer, text } => self.said.push((peer, text)),
                    _ => {}
                }
            }
            let tick = self.server.tick();
            each(&mut self.server, tick);
            self.server.step(&mut self.wire);
            for guest in &mut self.guests {
                guest.client.poll(&mut guest.tap, &mut guest.heard);
                guest.client.send(&mut guest.tap);
            }
        }
    }
}

#[test]
fn a_crossing_leaves_the_old_room_behind_and_coming_back_finds_it_again() {
    let mut hall = Hall::new();
    // A thing and a record in each room, at the same place: only the room tells them apart.
    let lamp = hall.server.spawn(HERE, CREATURE, [0.0; 3]);
    let well = hall.server.spawn(THERE, CREATURE, [0.0; 3]);
    hall.server.set(well, 0, b"well");
    hall.server.order(HERE, [0.0; 3], b"here 0");
    hall.server.order(HERE, [0.0; 3], b"here 1");
    hall.server.order(THERE, [0.0; 3], b"there 0");
    // The lamp changes every tick, so packets of it are always on their way.
    let flicker = |server: &mut Server, tick: u32| server.set(lamp, 0, &tick.to_le_bytes());
    hall.join(rough(21));
    hall.join(rough(22));
    hall.run(16, flicker);
    let (crosser, watcher) = (hall.hellos[0], hall.hellos[1]);
    // Which guest is the crosser's: hellos arrive in the links' order.
    let first = hall.seat(crosser, HERE, [1.0, 0.0, 0.0]);
    let stayed = hall.seat(watcher, HERE, [2.0, 0.0, 0.0]);
    hall.run(64, flicker);
    let me = hall
        .guests
        .iter()
        .position(|guest| guest.client.entity(first).is_some_and(|entity| entity.mine))
        .expect("the crosser's guest");
    let other = 1 - me;
    assert_eq!(hall.guests[me].holds(), [lamp, first, stayed]);
    assert_eq!(
        hall.guests[me].operations(),
        [(0, b"here 0".to_vec()), (1, b"here 1".to_vec())]
    );

    // Across: its body leaves here and is made there, as a crossing does (instances.md).
    hall.server.despawn(first);
    let second = hall.seat(crosser, THERE, [1.0, 0.0, 0.0]);
    hall.run(64, flicker);
    let guest = &hall.guests[me];
    assert!(
        matches!(guest.since_welcome()[0], Heard::Welcome { room: THERE, you, ref realm, .. } if you == Some(second) && realm == b"room 2")
    );
    assert_eq!(
        guest.holds(),
        [well, second],
        "it holds the new room and nothing of the old"
    );
    assert_eq!(guest.operations(), [(0, b"there 0".to_vec())]);
    assert!(guest.since_welcome().contains(&Heard::Ready));
    // The one who stayed sees it gone, and nothing of the other room.
    assert_eq!(hall.guests[other].holds(), [lamp, stayed]);

    // And back: the old room again, whole, its record from the start.
    hall.server.despawn(second);
    let third = hall.seat(crosser, HERE, [1.0, 0.0, 0.0]);
    hall.run(64, flicker);
    let guest = &hall.guests[me];
    assert_eq!(guest.holds(), [lamp, stayed, third]);
    assert_eq!(
        guest.operations(),
        [(0, b"here 0".to_vec()), (1, b"here 1".to_vec())]
    );
    let lit = &guest
        .client
        .entity(lamp)
        .expect("the lamp")
        .part(0)
        .expect("its part")
        .newer;
    assert!(
        hall.server.tick() - u32::from_le_bytes(lit.bytes[..4].try_into().expect("a tick")) < 16
    );
    assert_eq!(hall.guests[other].holds(), [lamp, stayed, third]);
}

#[test]
fn what_happens_once_is_heard_once_by_those_near_and_by_nobody_else() {
    let mut hall = Hall::new();
    // A link that loses a twentieth and copies a fifth: the worst case for "once".
    let poor = Conditions {
        loss: 0.05,
        copies: 0.2,
        ..rough(31)
    };
    hall.join(poor);
    hall.join(Conditions { seed: 32, ..poor });
    hall.run(16, |_, _| {});
    let (near, far) = (hall.hellos[0], hall.hellos[1]);
    let near_body = hall.seat(near, HERE, [0.0; 3]);
    hall.seat(far, HERE, [1000.0, 0.0, 0.0]);
    hall.run(32, |_, _| {});
    // A noise every second tick for five seconds, each saying the tick it was made on.
    let began = hall.server.tick();
    hall.run(320, |server, tick| {
        if tick % 2 == 0 {
            server.happen(HERE, [3.0, 0.0, 0.0], &tick.to_le_bytes());
        }
    });
    hall.run(64, |_, _| {});
    let noises = |guest: &Guest| -> Vec<(u32, u32)> {
        let heard = guest.heard.iter().filter_map(|heard| match heard {
            Heard::Happened { tick, bytes } => Some((
                *tick,
                u32::from_le_bytes(bytes[..4].try_into().expect("a tick")),
            )),
            _ => None,
        });
        heard.collect()
    };
    let me = hall
        .guests
        .iter()
        .position(|guest| {
            guest
                .client
                .entity(near_body)
                .is_some_and(|entity| entity.mine)
        })
        .expect("the near guest");
    let heard = noises(&hall.guests[me]);
    // Each said to have happened on the tick it did, none twice, and nearly all of the 160:
    // one lost twice is too old to tell and is dropped.
    assert!(
        heard
            .iter()
            .all(|(said, made)| said == made && *made >= began)
    );
    let mut once: Vec<u32> = heard.iter().map(|(_, made)| *made).collect();
    once.sort_unstable();
    once.dedup();
    assert_eq!(once.len(), heard.len(), "a noise was heard twice");
    assert!(
        (150..=160).contains(&heard.len()),
        "{} of 160 noises were heard",
        heard.len()
    );
    assert_eq!(
        noises(&hall.guests[1 - me]),
        [],
        "a noise a kilometre off was heard"
    );
}

#[test]
fn the_round_trip_is_measured_and_sets_how_soon_a_lost_part_is_sent_again() {
    let mut hall = Hall::new();
    hall.join(rough(41));
    hall.run(16, |_, _| {});
    let peer = hall.hellos[0];
    assert_eq!(hall.server.round_trip_ticks(peer), None);
    hall.seat(peer, HERE, [0.0; 3]);
    hall.run(128, |_, _| {});
    // 100 ms there and back is 6.4 ticks; a client answers on its next tick.
    let measured = hall
        .server
        .round_trip_ticks(peer)
        .expect("acknowledged by now");
    assert!((6.0..9.5).contains(&measured), "{measured} ticks");
}

/// A client's link whose ordered lane is slow: what arrives on it is held back so many polls,
/// as a reliable lane's is when its packet is lost and sent again, while state flows.
struct SlowOrdered {
    inner: LoopbackClient,
    held: Vec<(u32, FromServer)>,
    polls: u32,
    delay: u32,
}

impl ClientTransport for SlowOrdered {
    fn poll(&mut self, into: &mut Vec<FromServer>) {
        self.polls += 1;
        let mut arrived = Vec::new();
        self.inner.poll(&mut arrived);
        for arrival in arrived {
            match arrival {
                FromServer::Packet(Lane::Ordered, _) => {
                    self.held.push((self.polls + self.delay, arrival))
                }
                state => into.push(state),
            }
        }
        while self.held.first().is_some_and(|(due, _)| *due <= self.polls) {
            into.push(self.held.remove(0).1);
        }
    }
    fn send(&mut self, lane: Lane, bytes: &[u8]) {
        self.inner.send(lane, bytes);
    }
    fn close(&mut self) {
        self.inner.close();
    }
}

/// The welcome is on the ordered lane and state on the other, so state of a room can come
/// before the welcome to it. What came first is the room's and must be kept: the server has
/// had it acknowledged and will not say again what the things are. (Found by the walker's
/// two-world test: a client that dropped it never heard of its own body again.)
#[test]
fn state_that_comes_before_the_welcome_is_kept() {
    let network = Network::new();
    let mut wire = network.server();
    let part = PartSpec {
        audience: Audience::Public,
        weight: 1.0,
    };
    let mut server = Server::new(Config::new(VERSION), vec![part]);
    let lamp = server.spawn(HERE, CREATURE, [0.0; 3]);
    let mut tap = SlowOrdered {
        inner: network.connect(Conditions::PERFECT),
        held: Vec::new(),
        polls: 0,
        delay: 0,
    };
    let mut client = Client::new();
    client.hello(&mut tap, VERSION, &Identity::from_secret([2; 32]), "guest");
    let (mut events, mut heard, mut you) = (Vec::new(), Vec::new(), None);
    for tick in 0..96u32 {
        network.advance(TICK_S);
        server.poll(&mut wire, &mut events);
        for event in events.drain(..) {
            if let Event::Hello { peer, .. } = event {
                let body = server.spawn(HERE, PLAYER, [1.0, 0.0, 0.0]);
                server.set(body, 0, b"body");
                server.welcome(&mut wire, peer, HERE, body, b"room");
                you = Some(body);
                // From here the ordered lane is slow: the welcome comes twenty polls late.
                tap.delay = 20;
            }
        }
        server.set(lamp, 0, &tick.to_le_bytes());
        server.step(&mut wire);
        client.poll(&mut tap, &mut heard);
        client.send(&mut tap);
    }
    let you = you.expect("it said hello");
    // State came first, by twenty ticks.
    let welcome = heard
        .iter()
        .position(|heard| matches!(heard, Heard::Welcome { .. }))
        .expect("welcomed");
    let entered = heard
        .iter()
        .position(|heard| *heard == Heard::Entered(you))
        .expect("its body came");
    assert!(
        entered < welcome,
        "the test's premise: state before the welcome"
    );
    // And it is all still held, and still told: its own body, and the lamp as it is now.
    assert!(
        client.entity(you).is_some_and(|entity| entity.mine),
        "its own body was dropped at the welcome"
    );
    let lit = &client
        .entity(lamp)
        .expect("the lamp")
        .part(0)
        .expect("its part")
        .newer;
    assert!(server.tick() - u32::from_le_bytes(lit.bytes[..4].try_into().expect("a tick")) < 4);
}

/// Who is here, and what they say (docs/v0-tracks.md, Server: text chat and a player list):
/// each is told of the others as they come and go, and what one says everyone hears once, by
/// name, the host's own words among them.
#[test]
fn everyone_is_told_who_is_here_and_hears_what_is_said() {
    let mut hall = Hall::new();
    hall.server.host_name = Some("host".to_string());
    hall.join(rough(51));
    hall.run(16, |_, _| {});
    let first = hall.hellos[0];
    hall.seat(first, HERE, [0.0; 3]);
    hall.join(rough(52));
    hall.run(32, |_, _| {});
    let second = hall.hellos[1];
    hall.seat(second, HERE, [1.0, 0.0, 0.0]);
    hall.run(32, |_, _| {});
    let roster = |guest: &Guest| -> Vec<(String, bool)> {
        let told = guest.heard.iter().filter_map(|heard| match heard {
            Heard::Roster { name, here } => Some((name.clone(), *here)),
            _ => None,
        });
        told.collect()
    };
    let said = |guest: &Guest| -> Vec<(String, String)> {
        let told = guest.heard.iter().filter_map(|heard| match heard {
            Heard::Said { name, text } => Some((name.clone(), text.clone())),
            _ => None,
        });
        told.collect()
    };
    let here = |name: &str| (name.to_string(), true);
    // Both links say "guest"; the host is named to each as it comes.
    assert_eq!(roster(&hall.guests[0]), [here("host"), here("guest")]);
    assert_eq!(roster(&hall.guests[1]), [here("guest"), here("host")]);
    assert_eq!(hall.server.names().collect::<Vec<_>>(), ["guest", "guest"]);

    let Guest { tap, client, .. } = &mut hall.guests[0];
    client.say(tap, "is anyone there?");
    hall.run(16, |_, _| {});
    hall.server.say(&mut hall.wire, "host", "i am");
    hall.run(16, |_, _| {});
    let both = [
        ("guest".to_string(), "is anyone there?".to_string()),
        ("host".to_string(), "i am".to_string()),
    ];
    assert_eq!(said(&hall.guests[0]), both);
    assert_eq!(said(&hall.guests[1]), both);
    assert_eq!(hall.said, [(first, "is anyone there?".to_string())]);

    // One goes: the other is told. And words too long are no game of ours: their sender is
    // dropped, as for any nonsense on the ordered lane.
    hall.guests[1].tap.close();
    hall.run(16, |_, _| {});
    assert_eq!(
        roster(&hall.guests[0]).last(),
        Some(&("guest".to_string(), false))
    );
    let Guest { tap, client, .. } = &mut hall.guests[0];
    client.say(tap, &"x".repeat(crate::proto::MOST_SAY + 1));
    hall.run(16, |_, _| {});
    assert!(
        hall.guests[0]
            .heard
            .contains(&Heard::Closed(Reason::Malformed))
    );
    assert_eq!(hall.server.clients(), 0);
}

/// A client greeted with no body yet is told the room and nothing of what is in it; nobody
/// is told of it either. When it says it is ready it is given its body and welcomed with it,
/// and the room's state comes (D639: told the level, load it, then enter).
#[test]
fn a_greeted_client_takes_its_body_only_once_it_says_it_is_ready() {
    let network = Network::new();
    let mut wire = network.server();
    let part = PartSpec {
        audience: Audience::Public,
        weight: 1.0,
    };
    let mut server = Server::new(Config::new(VERSION), vec![part]);
    let lamp = server.spawn(HERE, CREATURE, [0.0; 3]);
    server.set(lamp, 0, b"lit");
    let mut tap = network.connect(Conditions::PERFECT);
    let mut client = Client::new();
    client.hello(&mut tap, VERSION, &Identity::from_secret([2; 32]), "guest");
    let (mut events, mut heard, mut you) = (Vec::new(), Vec::new(), None);
    let mut run = |ticks: u32,
                   server: &mut Server,
                   client: &mut Client,
                   tap: &mut LoopbackClient,
                   heard: &mut Vec<Heard>,
                   you: &mut Option<EntityId>| {
        for _ in 0..ticks {
            network.advance(TICK_S);
            server.poll(&mut wire, &mut events);
            for event in events.drain(..) {
                match event {
                    Event::Hello { peer, .. } => server.greet(&mut wire, peer, HERE, b"room"),
                    Event::Ready { peer } => {
                        let body = server.spawn(HERE, PLAYER, [1.0, 0.0, 0.0]);
                        server.set(body, 0, b"body");
                        server.welcome(&mut wire, peer, HERE, body, b"room");
                        *you = Some(body);
                    }
                    _ => {}
                }
            }
            server.step(&mut wire);
            client.poll(tap, heard);
            client.send(tap);
        }
    };
    // Greeted: the room's word, no entity, and for a second nothing of the room.
    run(64, &mut server, &mut client, &mut tap, &mut heard, &mut you);
    assert_eq!(
        heard
            .iter()
            .filter(|each| !matches!(each, Heard::Roster { .. }))
            .collect::<Vec<_>>(),
        [&Heard::Welcome {
            tick: 1,
            room: HERE,
            you: None,
            realm: b"room".to_vec()
        }],
        "greeted with the room and no entity, and told nothing in it"
    );
    assert_eq!(you, None, "no body was made for it");
    assert_eq!(client.entities().count(), 0);
    // Ready: its body, the welcome with it, and the room's state.
    client.ready(&mut tap);
    run(64, &mut server, &mut client, &mut tap, &mut heard, &mut you);
    let body = you.expect("its body was made when it said it was ready");
    assert!(
        heard
            .iter()
            .any(|each| matches!(each, Heard::Welcome { you: Some(you), .. } if *you == body)),
        "{heard:?}"
    );
    assert!(client.entity(body).is_some_and(|entity| entity.mine));
    assert!(client.entity(lamp).is_some(), "the room's state came");
}
