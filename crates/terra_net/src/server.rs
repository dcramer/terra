//! The server's end (D652): who is connected, the entities it replicates and their parts, the
//! operations it has ordered, and each tick's intents. The game owns the rules; it tells the
//! server what exists and what changed, and the server sees that each client holds the newest
//! of what is near it.
//!
//! A tick, for the game: [`Server::poll`], then each player's [`Server::intent`], then the
//! tick's rules, writing what changed ([`Server::set`], [`Server::place`],
//! [`Server::order`]), then [`Server::step`], which sends and ends the tick.

mod replicate;

use crate::ack::Acks;
use crate::identity;
use crate::interest::{Grid, Place, cell_of, distance_squared};
use crate::proto::{Down, EntityId, Kind, MOST_PARTS, Reason, Region, RoomId, Up};
use crate::transport::{FromClients, Lane, Peer, ServerTransport};
use crate::wire::{Wire, Writer};
use std::collections::{BTreeMap, VecDeque};
use terra_gen::hash::QuickMap;

/// Who is sent a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    /// Everyone the entity is near.
    Public,
    /// Only the client that owns the entity: what it carries, its body unquantised.
    Owner,
}

/// A part's place in the game's schema: every entity's part of this number is this.
#[derive(Clone, Copy, Debug)]
pub struct PartSpec {
    pub audience: Audience,
    /// How much it matters against other parts when a packet cannot hold all that is owed.
    pub weight: f32,
}

/// A server's numbers (D652): first values, to be measured.
#[derive(Clone, Debug)]
pub struct Config {
    /// The protocol's version: a client of another is refused.
    pub version: u64,
    pub most_clients: usize,
    /// The side of an interest cube, m.
    pub cell_m: f64,
    /// How near a thing is before a client is sent it, m; it is sent until a cube further.
    pub radius_m: f64,
    /// The side of a region, whose operations are one record, m.
    pub region_m: f64,
    /// A state packet is sent every this many ticks.
    pub send_every: u32,
    /// What a client is near is looked up every this many ticks.
    pub refresh_every: u32,
    /// A part sent and not acknowledged is sent again after this many ticks.
    pub resend_ticks: u32,
    /// The most bytes of a state packet: under any path's MTU.
    pub packet_bytes: usize,
    /// What its challenges are drawn from ([`crate::identity`]): the game's to give, from
    /// the operating system's randomness, so no hello overheard on one run passes on another.
    pub secret: u64,
}

impl Config {
    pub fn new(version: u64) -> Config {
        Config {
            version,
            most_clients: 16,
            cell_m: 32.0,
            radius_m: 128.0,
            region_m: 256.0,
            send_every: 2,
            refresh_every: 8,
            resend_ticks: 16,
            packet_bytes: 1200,
            secret: 0,
        }
    }
}

/// What the game is told of its clients.
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    /// A client of the right version, proven to hold the identity it gives, asks to play: answer with [`Server::welcome`] or
    /// [`Server::refuse`].
    Hello {
        peer: Peer,
        identity: [u8; 32],
        name: String,
    },
    /// A welcomed client asks for an operation: check it (reach, ownership, permission, D51)
    /// and [`Server::order`] it if it stands.
    Asked { peer: Peer, bytes: Vec<u8> },
    /// A welcomed client is gone, for any reason: its body leaves the world (D637).
    Left { peer: Peer },
    /// A welcomed client said something, which everyone has been told.
    Said { peer: Peer, text: String },
}

struct Part {
    /// Counts up with each change; none is 0.
    version: u32,
    bytes: Vec<u8>,
}

struct Entity {
    kind: Kind,
    room: RoomId,
    place: Place,
    owner: Option<Peer>,
    parts: [Option<Part>; MOST_PARTS],
}

struct Room {
    grid: Grid,
    /// Each region's operations, in the order given.
    records: QuickMap<Region, Vec<Vec<u8>>>,
}

/// What a client holds of an entity, as far as the server knows.
struct Known {
    /// The tick the client began to be sent it: a packet from before says nothing of it.
    since: u32,
    /// Each part's version the client has acknowledged, and the last sent.
    acked: [u32; MOST_PARTS],
    sent: [u32; MOST_PARTS],
    sent_at: Option<u32>,
    /// The client has acknowledged a packet saying what kind of thing it is.
    met: bool,
    /// The last look at what is near that found it.
    sweep: u32,
}

/// What a state packet carried, for when it is acknowledged.
enum Sent {
    Part {
        id: EntityId,
        part: u8,
        version: u32,
    },
    Gone(EntityId),
    Event(u32),
}

struct Flight {
    seq: u16,
    tick: u32,
    sent: Vec<Sent>,
}

/// Something that happened near a client, once: a noise, a blow landing. Sent until
/// acknowledged or too old to matter.
struct Outgoing {
    id: u32,
    tick: u32,
    sent_at: Option<u32>,
    bytes: Vec<u8>,
}

/// An entity a client is to forget: since when, and when it was last told.
struct Leaving {
    since: u32,
    sent_at: Option<u32>,
}

struct Session {
    /// The name it gave, once its hello was proven.
    name: String,
    /// The number its hello must be signed with.
    challenge: [u8; 32],
    welcomed: bool,
    room: RoomId,
    /// The entity it sees the world from: its own.
    view: Option<EntityId>,
    acks: Acks,
    known: QuickMap<EntityId, Known>,
    leaving: QuickMap<EntityId, Leaving>,
    flights: VecDeque<Flight>,
    /// The regions round it, and how much of each region's record it has been sent.
    watching: Vec<Region>,
    records: QuickMap<Region, u32>,
    settled: bool,
    sweep: u32,
    /// Intents for ticks to come, oldest first, and the last one taken.
    intents: VecDeque<(u32, Vec<u8>)>,
    last: Option<Vec<u8>>,
    heard: Option<u32>,
    events: VecDeque<Outgoing>,
    next_event: u32,
    /// How long its acknowledgements take to come back, ticks, smoothed.
    round_trip: Option<f32>,
}

impl Session {
    fn new(challenge: [u8; 32]) -> Session {
        Session {
            name: String::new(),
            challenge,
            welcomed: false,
            room: RoomId(0),
            view: None,
            acks: Acks::default(),
            known: QuickMap::default(),
            leaving: QuickMap::default(),
            flights: VecDeque::new(),
            watching: Vec::new(),
            records: QuickMap::default(),
            settled: false,
            sweep: 0,
            intents: VecDeque::new(),
            last: None,
            heard: None,
            events: VecDeque::new(),
            next_event: 0,
            round_trip: None,
        }
    }
}

/// The server's end of the protocol.
pub struct Server {
    config: Config,
    parts: Vec<PartSpec>,
    tick: u32,
    next_entity: u32,
    /// How many connections it has had: each one's challenge is its own.
    joined: u64,
    entities: QuickMap<EntityId, Entity>,
    rooms: QuickMap<RoomId, Room>,
    sessions: BTreeMap<Peer, Session>,
    inbox: Vec<FromClients>,
    scratch: replicate::Scratch,
    /// Packets that could not be read, both lanes.
    pub malformed: u64,
    /// The host's own player's name, where the host plays too (D638): told to whoever
    /// comes, with the others'.
    pub host_name: Option<String>,
}

impl Server {
    /// A server of a schema: part `n` of every entity is `parts[n]`.
    pub fn new(config: Config, parts: Vec<PartSpec>) -> Server {
        assert!(
            parts.len() <= MOST_PARTS,
            "an entity has at most {MOST_PARTS} parts"
        );
        Server {
            config,
            parts,
            tick: 0,
            next_entity: 0,
            joined: 0,
            entities: QuickMap::default(),
            rooms: QuickMap::default(),
            sessions: BTreeMap::new(),
            inbox: Vec::new(),
            scratch: replicate::Scratch::default(),
            malformed: 0,
            host_name: None,
        }
    }

    /// The tick under way.
    pub fn tick(&self) -> u32 {
        self.tick
    }

    /// How many clients are connected, welcomed or not.
    pub fn clients(&self) -> usize {
        self.sessions.len()
    }

    /// Takes what the transport has for it; the game answers the events.
    pub fn poll(&mut self, transport: &mut impl ServerTransport, events: &mut Vec<Event>) {
        let mut inbox = std::mem::take(&mut self.inbox);
        transport.poll(&mut inbox);
        for arrival in inbox.drain(..) {
            match arrival {
                FromClients::Joined(peer) => {
                    if self.sessions.len() >= self.config.most_clients {
                        transport.send(peer, Lane::Ordered, &Down::Refuse(Reason::Full).encoded());
                        transport.drop_peer(peer);
                    } else {
                        self.joined += 1;
                        let challenge = identity::challenge(self.config.secret, self.joined);
                        transport.send(peer, Lane::Ordered, &Down::Challenge(challenge).encoded());
                        self.sessions.insert(peer, Session::new(challenge));
                    }
                }
                FromClients::Left(peer) => self.forget(transport, peer, events),
                FromClients::Packet(peer, Lane::Ordered, bytes) => {
                    self.ordered(transport, peer, &bytes, events);
                }
                FromClients::Packet(peer, Lane::State, bytes) => self.state(peer, &bytes),
            }
        }
        self.inbox = inbox;
    }

    /// A connection is gone: if it was let in, the others are told it left.
    fn forget(
        &mut self,
        transport: &mut impl ServerTransport,
        peer: Peer,
        events: &mut Vec<Event>,
    ) {
        let Some(session) = self
            .sessions
            .remove(&peer)
            .filter(|session| session.welcomed)
        else {
            return;
        };
        self.tell_all(
            transport,
            &Down::Roster {
                name: session.name,
                here: false,
            },
        );
        events.push(Event::Left { peer });
    }

    /// A message on the ordered lane to every client let in.
    fn tell_all(&self, transport: &mut impl ServerTransport, message: &Down) {
        let bytes = message.encoded();
        for (&peer, _) in self.sessions.iter().filter(|(_, session)| session.welcomed) {
            transport.send(peer, Lane::Ordered, &bytes);
        }
    }

    /// The host's own words, or the game's, to everyone: as a player's are.
    pub fn say(&mut self, transport: &mut impl ServerTransport, name: &str, text: &str) {
        let said = Down::Said {
            name: name.to_string(),
            text: text.to_string(),
        };
        self.tell_all(transport, &said);
    }

    /// The names of the clients let in.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        let here = self.sessions.values().filter(|session| session.welcomed);
        here.map(|session| session.name.as_str())
    }

    fn ordered(
        &mut self,
        transport: &mut impl ServerTransport,
        peer: Peer,
        bytes: &[u8],
        events: &mut Vec<Event>,
    ) {
        let Some(session) = self.sessions.get(&peer) else {
            return;
        };
        match Up::decoded(bytes) {
            Ok(Up::Hello {
                version,
                identity,
                name,
                signature,
            }) if !session.welcomed => {
                if version != self.config.version {
                    self.refuse(transport, peer, Reason::Version);
                } else if !identity::proven(
                    &identity,
                    &session.challenge,
                    version,
                    &name,
                    &signature,
                ) {
                    // Not the holder of that key, or a hello heard on another connection.
                    self.refuse(transport, peer, Reason::Refused);
                } else {
                    if let Some(session) = self.sessions.get_mut(&peer) {
                        session.name = name.clone();
                    }
                    events.push(Event::Hello {
                        peer,
                        identity,
                        name,
                    });
                }
            }
            Ok(Up::Ask(bytes)) if session.welcomed => events.push(Event::Asked { peer, bytes }),
            Ok(Up::Say(text)) if session.welcomed => {
                let name = session.name.clone();
                self.say(transport, &name, &text);
                events.push(Event::Said { peer, text });
            }
            // Out of turn or unreadable on the lane that loses nothing: not a game of ours.
            _ => {
                self.malformed += 1;
                transport.send(
                    peer,
                    Lane::Ordered,
                    &Down::Closing(Reason::Malformed).encoded(),
                );
                transport.drop_peer(peer);
                self.forget(transport, peer, events);
            }
        }
    }

    /// A state packet up: acknowledgements, then intents. One that cannot be read is dropped,
    /// as a lost one would be.
    fn state(&mut self, peer: Peer, bytes: &[u8]) {
        let Some(session) = self
            .sessions
            .get_mut(&peer)
            .filter(|session| session.welcomed)
        else {
            return;
        };
        if replicate::heard(session, bytes, self.tick, &mut self.scratch).is_err() {
            self.malformed += 1;
        }
    }

    /// Lets a client in, to a room, seeing from an entity that is its own; or, for a client
    /// already in, moves it to another room (a crossing, instances.md): it is told the new
    /// room as it was the first, and begins it holding nothing. `realm` is the
    /// game's word on the room, passed on as it is.
    pub fn welcome(
        &mut self,
        transport: &mut impl ServerTransport,
        peer: Peer,
        room: RoomId,
        you: EntityId,
        realm: &[u8],
    ) {
        let Some(session) = self.sessions.get_mut(&peer) else {
            return;
        };
        let first = !session.welcomed;
        // Let in, or crossing from another room: either way it holds nothing of this room
        // yet, and nothing of the last is owed it.
        session.welcomed = true;
        session.room = room;
        session.view = Some(you);
        session.known.clear();
        session.leaving.clear();
        session.flights.clear();
        session.watching.clear();
        session.records.clear();
        session.events.clear();
        session.settled = false;
        session.sweep = 0;
        if let Some(entity) = self.entities.get_mut(&you) {
            entity.owner = Some(peer);
        }
        let welcome = Down::Welcome {
            tick: self.tick,
            room,
            you,
            realm: realm.to_vec(),
        };
        transport.send(peer, Lane::Ordered, &welcome.encoded());
        if first {
            // Who is here, each way: the newcomer is told of the others, and they of it.
            let name = self
                .sessions
                .get(&peer)
                .map(|session| session.name.clone())
                .unwrap_or_default();
            let came = Down::Roster { name, here: true }.encoded();
            let others = self
                .sessions
                .iter()
                .filter(|(other, session)| **other != peer && session.welcomed);
            for (&other, session) in others {
                let theirs = Down::Roster {
                    name: session.name.clone(),
                    here: true,
                };
                transport.send(peer, Lane::Ordered, &theirs.encoded());
                transport.send(other, Lane::Ordered, &came);
            }
            if let Some(host) = &self.host_name {
                let host = Down::Roster {
                    name: host.clone(),
                    here: true,
                };
                transport.send(peer, Lane::Ordered, &host.encoded());
            }
        }
    }

    /// Turns a client away, with the reason it is shown.
    pub fn refuse(&mut self, transport: &mut impl ServerTransport, peer: Peer, reason: Reason) {
        if self.sessions.remove(&peer).is_some() {
            transport.send(peer, Lane::Ordered, &Down::Refuse(reason).encoded());
            transport.drop_peer(peer);
        }
    }

    /// Ends the session for everyone: the host is leaving.
    pub fn close(&mut self, transport: &mut impl ServerTransport, reason: Reason) {
        for peer in std::mem::take(&mut self.sessions).into_keys() {
            transport.send(peer, Lane::Ordered, &Down::Closing(reason).encoded());
            transport.drop_peer(peer);
        }
    }

    fn room(&mut self, room: RoomId) -> &mut Room {
        let cell_m = self.config.cell_m;
        self.rooms.entry(room).or_insert_with(|| Room {
            grid: Grid::new(cell_m),
            records: QuickMap::default(),
        })
    }

    /// A new entity of a kind at a place in a room.
    pub fn spawn(&mut self, room: RoomId, kind: Kind, place: Place) -> EntityId {
        self.next_entity += 1;
        let id = EntityId(self.next_entity);
        self.room(room).grid.insert(id.0, place);
        self.entities.insert(
            id,
            Entity {
                kind,
                room,
                place,
                owner: None,
                parts: Default::default(),
            },
        );
        id
    }

    /// An entity is no more: every client that held it is told.
    pub fn despawn(&mut self, id: EntityId) {
        let Some(entity) = self.entities.remove(&id) else {
            return;
        };
        self.room(entity.room).grid.remove(id.0, entity.place);
        for session in self.sessions.values_mut() {
            if session.view == Some(id) {
                session.view = None;
            }
            if session.known.remove(&id).is_some() {
                session.leaving.insert(
                    id,
                    Leaving {
                        since: self.tick,
                        sent_at: None,
                    },
                );
            }
        }
    }

    /// Where an entity is now, for who is near it.
    pub fn place(&mut self, id: EntityId, place: Place) {
        if let Some(entity) = self.entities.get_mut(&id) {
            let (room, from) = (entity.room, entity.place);
            entity.place = place;
            self.room(room).grid.shift(id.0, from, place);
        }
    }

    /// An entity's part as it is now. Bytes equal to the last are no change, so a thing at
    /// rest costs nothing however often it is written.
    pub fn set(&mut self, id: EntityId, part: usize, bytes: &[u8]) {
        debug_assert!(part < self.parts.len(), "part {part} is not in the schema");
        let Some(entity) = self.entities.get_mut(&id) else {
            return;
        };
        match &mut entity.parts[part] {
            Some(held) if held.bytes == bytes => {}
            Some(held) => {
                held.version += 1;
                held.bytes.clear();
                held.bytes.extend_from_slice(bytes);
            }
            empty => {
                *empty = Some(Part {
                    version: 1,
                    bytes: bytes.to_vec(),
                });
            }
        }
    }

    /// [`Server::set`] of a value, encoded here.
    pub fn set_as(&mut self, id: EntityId, part: usize, value: &impl Wire) {
        let mut writer = Writer::reusing(std::mem::take(&mut self.scratch.encode));
        value.write(&mut writer);
        let bytes = writer.finish();
        self.set(id, part, &bytes);
        self.scratch.encode = bytes;
    }

    /// A client's intent for the tick under way: the one it sent for this tick, or, that one
    /// lost or late, its last (Overwatch). None before its first.
    pub fn intent(&mut self, peer: Peer) -> Option<&[u8]> {
        let tick = self.tick;
        let session = self.sessions.get_mut(&peer)?;
        while let Some((at, _)) = session.intents.front() {
            if *at > tick {
                break;
            }
            let (at, bytes) = session.intents.pop_front()?;
            if at == tick {
                session.last = Some(bytes);
            }
        }
        session.last.as_deref()
    }

    /// Orders an operation at a place: the next of its region's record, sent to every client
    /// the region is near, now and when they come. Returns the region and its number.
    pub fn order(&mut self, room: RoomId, place: Place, bytes: &[u8]) -> (Region, u32) {
        let region = cell_of(place, self.config.region_m);
        let record = self.room(room).records.entry(region).or_default();
        record.push(bytes.to_vec());
        (region, record.len() as u32 - 1)
    }

    /// Something happened at a place, once (a noise, a blow landing): every client near it
    /// is told, on the state lane, again until it acknowledges or the event is half a second
    /// old, when it is dropped: a noise late is worse than none.
    pub fn happen(&mut self, room: RoomId, place: Place, bytes: &[u8]) {
        let reach = self.config.radius_m * self.config.radius_m;
        let tick = self.tick;
        for session in self.sessions.values_mut() {
            let eye = session.view.and_then(|view| self.entities.get(&view));
            let near = eye.is_some_and(|eye| distance_squared(eye.place, place) <= reach);
            if session.welcomed && session.room == room && near {
                session.next_event += 1;
                session.events.push_back(Outgoing {
                    id: session.next_event,
                    tick,
                    sent_at: None,
                    bytes: bytes.to_vec(),
                });
            }
        }
    }

    /// How long a client's acknowledgements take to come back, ticks: its round trip and a
    /// tick or so of waiting to be sent. None until one has.
    pub fn round_trip_ticks(&self, peer: Peer) -> Option<f32> {
        self.sessions.get(&peer)?.round_trip
    }

    /// Ends the tick: each client is sent the operations it lacks and, on the ticks state is
    /// sent, the newest of what it is owed.
    pub fn step(&mut self, transport: &mut impl ServerTransport) {
        let tick = self.tick;
        for (&peer, session) in self
            .sessions
            .iter_mut()
            .filter(|(_, session)| session.welcomed)
        {
            let Some(room) = self.rooms.get(&session.room) else {
                continue;
            };
            let world = replicate::World {
                config: &self.config,
                parts: &self.parts,
                entities: &self.entities,
                room,
                tick,
            };
            if session.sweep == 0 || tick.is_multiple_of(self.config.refresh_every) {
                replicate::refresh(session, &world, &mut self.scratch);
            }
            replicate::operations(session, &world, peer, transport);
            if tick.is_multiple_of(self.config.send_every) {
                replicate::send(session, &world, peer, transport, &mut self.scratch);
            }
        }
        // On the ticks state goes: a carrier frames every flush, and a flush a tick was
        // half of what went down. An operation ordered on the tick between waits one tick.
        if tick.is_multiple_of(self.config.send_every) {
            transport.flush();
        }
        self.tick += 1;
    }
}
