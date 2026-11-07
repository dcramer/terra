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
                if let Event::Hello { peer, .. } = event {
                    self.hellos.push(peer);
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
        matches!(guest.since_welcome()[0], Heard::Welcome { room: THERE, you, ref realm, .. } if you == second && realm == b"room 2")
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
