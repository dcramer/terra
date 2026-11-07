//! The client's end (D651, D652): what the server has told it of the entities near it, the
//! operations in their order, and its own intents on their way up. It holds no game type and
//! needs no engine: the game reads parts' bytes from it, and a bot or a load test drives it
//! as it is (D73).
//!
//! State arrives unreliably, so every rule here is "the newest wins": a part is stamped with
//! the tick of the packet that brought it and an older one never replaces it, and an entity
//! told to be forgotten stays forgotten against anything older.

use crate::ack::{Acks, Header};
use crate::identity::Identity;
use crate::proto::{
    Down, EntityId, Kind, MOST_INTENTS, MOST_PARTS, Reason, Region, RoomId, Stamp, Up,
};
use crate::transport::{ClientTransport, FromServer, Lane};
use crate::wire::{Malformed, Reader, Wire, Writer};
use std::collections::{HashMap, VecDeque};

/// What the game is told.
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    /// The server let it in: the room, the entity that is its own, the game's word on the
    /// room.
    Welcome {
        tick: u32,
        room: RoomId,
        you: EntityId,
        realm: Vec<u8>,
    },
    /// The next operation of a region's record, to apply.
    Op {
        region: Region,
        number: u32,
        bytes: Vec<u8>,
    },
    /// The records of the regions round it are whole: the world round it is as the server's.
    Ready,
    /// An entity came near, and one left or is no more.
    Entered(EntityId),
    Gone(EntityId),
    /// The connection is over, and why, in words for the player ([`Reason::message`]).
    Closed(Reason),
}

/// A part as a packet brought it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    /// The tick it is the state after.
    pub tick: u32,
    pub bytes: Vec<u8>,
}

/// A part's last two states: what is shown is between them (D651).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Samples {
    pub older: Option<Sample>,
    pub newer: Sample,
}

/// How many sends back of the newest word others are shown (D651): two, so one lost packet
/// leaves something to draw toward, as Source's interpolation ratio.
pub const SHOWN_SENDS_BACK: u32 = 2;

impl Samples {
    /// The two states to draw between at a tick of the server's (it may fall between ticks),
    /// and how far from the first to the second: 0 at or before the older, 1 at or past the
    /// newer, where it is held, never run on past what the server said.
    pub fn around(&self, tick: f64) -> (&Sample, &Sample, f32) {
        let Some(older) = &self.older else {
            return (&self.newer, &self.newer, 1.0);
        };
        let span = f64::from(self.newer.tick - older.tick);
        let share = ((tick - f64::from(older.tick)) / span.max(1.0)).clamp(0.0, 1.0);
        (older, &self.newer, share as f32)
    }
}

/// An entity as the client holds it.
#[derive(Clone, Debug)]
pub struct Entity {
    pub kind: Kind,
    /// It is this client's own.
    pub mine: bool,
    /// The newest tick anything of it came from.
    seen: u32,
    parts: [Option<Samples>; MOST_PARTS],
}

impl Entity {
    pub fn part(&self, part: usize) -> Option<&Samples> {
        self.parts.get(part)?.as_ref()
    }
}

/// How long a forgotten entity is remembered as forgotten, ticks: longer than any packet is
/// taken (the acknowledgements refuse one 33 packets old).
const TOMB_TICKS: u32 = 256;

/// The client's end of the protocol.
#[derive(Default)]
pub struct Client {
    acks: Acks,
    entities: HashMap<EntityId, Entity>,
    /// Entities told to be forgotten, and as of which tick.
    tombs: HashMap<EntityId, u32>,
    stamp: Option<Stamp>,
    /// Its intents the server is not known to hold, oldest first, a tick each.
    intents: VecDeque<(u32, Vec<u8>)>,
    closed: bool,
    /// The server's challenge, and the hello waiting for it.
    challenge: Option<[u8; 32]>,
    asking: Option<(u64, Identity, String)>,
    inbox: Vec<FromServer>,
    acked: Vec<u16>,
    /// Packets that could not be read.
    pub malformed: u64,
}

impl Client {
    pub fn new() -> Client {
        Client::default()
    }

    /// Asks to play, as an identity: sent once the server's challenge is in hand, signed with
    /// it ([`crate::identity`]).
    pub fn hello(
        &mut self,
        transport: &mut impl ClientTransport,
        version: u64,
        identity: &Identity,
        name: &str,
    ) {
        self.asking = Some((version, identity.clone(), name.to_string()));
        self.greet(transport);
    }

    /// Sends the hello when both it and the challenge are held, whichever came first.
    fn greet(&mut self, transport: &mut impl ClientTransport) {
        let (Some(challenge), Some((version, identity, name))) = (self.challenge, &self.asking)
        else {
            return;
        };
        let hello = Up::Hello {
            version: *version,
            identity: identity.public(),
            name: name.clone(),
            signature: identity.sign(&challenge, *version, name),
        };
        transport.send(Lane::Ordered, &hello.encoded());
        transport.flush();
        self.asking = None;
    }

    /// Takes what the transport has for it.
    pub fn poll(&mut self, transport: &mut impl ClientTransport, events: &mut Vec<Event>) {
        let mut inbox = std::mem::take(&mut self.inbox);
        transport.poll(&mut inbox);
        for arrival in inbox.drain(..) {
            match arrival {
                FromServer::Closed => self.close(Reason::Lost, events),
                FromServer::Packet(Lane::Ordered, bytes) => match Down::decoded(&bytes) {
                    Ok(Down::Welcome {
                        tick,
                        room,
                        you,
                        realm,
                    }) => events.push(Event::Welcome {
                        tick,
                        room,
                        you,
                        realm,
                    }),
                    Ok(Down::Op {
                        region,
                        number,
                        bytes,
                    }) => events.push(Event::Op {
                        region,
                        number,
                        bytes,
                    }),
                    Ok(Down::Settled) => events.push(Event::Ready),
                    Ok(Down::Challenge(challenge)) => {
                        self.challenge = Some(challenge);
                        self.greet(transport);
                    }
                    Ok(Down::Refuse(reason) | Down::Closing(reason)) => self.close(reason, events),
                    Err(_) => self.malformed += 1,
                },
                FromServer::Packet(Lane::State, bytes) => {
                    if self.state(&bytes, events).is_err() {
                        self.malformed += 1;
                    }
                }
            }
        }
        self.inbox = inbox;
    }

    /// The connection ended: told once, with the first reason given.
    fn close(&mut self, reason: Reason, events: &mut Vec<Event>) {
        if !self.closed {
            self.closed = true;
            events.push(Event::Closed(reason));
        }
    }

    /// A state packet down.
    fn state(&mut self, bytes: &[u8], events: &mut Vec<Event>) -> Result<(), Malformed> {
        let mut from = Reader::new(bytes);
        let header = Header::read(&mut from)?;
        self.acked.clear();
        if !self.acks.incoming(header, &mut self.acked) {
            return Ok(());
        }
        let stamp = Stamp::read(&mut from)?;
        let tick = stamp.tick;
        if self.stamp.is_none_or(|newest| tick > newest.tick) {
            self.stamp = Some(stamp);
            if let Some(heard) = stamp.heard {
                while self.intents.front().is_some_and(|(at, _)| *at <= heard) {
                    self.intents.pop_front();
                }
            }
            self.tombs
                .retain(|_, at| tick.saturating_sub(*at) < TOMB_TICKS);
        }
        while from.bool()? {
            let id = EntityId(u32::try_from(from.varint()?).map_err(|_| Malformed::Range)?);
            if from.bool()? {
                self.forget(id, tick, events);
            } else {
                self.item(id, tick, &mut from, events)?;
            }
        }
        Ok(())
    }

    /// An entity is to be forgotten as of a tick, unless something newer of it is held.
    fn forget(&mut self, id: EntityId, tick: u32, events: &mut Vec<Event>) {
        if self
            .entities
            .get(&id)
            .is_some_and(|entity| entity.seen > tick)
        {
            return;
        }
        let tomb = self.tombs.entry(id).or_insert(tick);
        *tomb = (*tomb).max(tick);
        if self.entities.remove(&id).is_some() {
            events.push(Event::Gone(id));
        }
    }

    /// Some of an entity's parts as of a tick. All of it is read whatever is kept, so the
    /// items after it are found.
    fn item(
        &mut self,
        id: EntityId,
        tick: u32,
        from: &mut Reader,
        events: &mut Vec<Event>,
    ) -> Result<(), Malformed> {
        let intro = if from.bool()? {
            let kind = Kind(u16::try_from(from.varint()?).map_err(|_| Malformed::Range)?);
            Some((kind, from.bool()?))
        } else {
            None
        };
        let mask = from.bits(MOST_PARTS as u32)?;
        // Forgotten as of this tick or a later one: this is older news.
        let buried = self.tombs.get(&id).is_some_and(|tomb| *tomb >= tick);
        let mut entity = match (self.entities.get_mut(&id), intro) {
            _ if buried => None,
            (Some(entity), _) => Some(entity),
            (None, Some((kind, mine))) => {
                events.push(Event::Entered(id));
                Some(self.entities.entry(id).or_insert(Entity {
                    kind,
                    mine,
                    seen: tick,
                    parts: Default::default(),
                }))
            }
            // Unknown and not said what it is: from before it was forgotten.
            (None, None) => None,
        };
        for index in (0..MOST_PARTS).filter(|index| mask & (1 << index) != 0) {
            let bytes = from.run()?;
            let Some(entity) = entity.as_deref_mut() else {
                continue;
            };
            entity.seen = entity.seen.max(tick);
            match &mut entity.parts[index] {
                Some(samples) if tick > samples.newer.tick => {
                    let newer = Sample {
                        tick,
                        bytes: bytes.to_vec(),
                    };
                    samples.older = Some(std::mem::replace(&mut samples.newer, newer));
                }
                // Older than the newest held: the newest wins.
                Some(_) => {}
                empty => {
                    *empty = Some(Samples {
                        older: None,
                        newer: Sample {
                            tick,
                            bytes: bytes.to_vec(),
                        },
                    });
                }
            }
        }
        Ok(())
    }

    /// Its intent for a tick, to go up with the next packets until the server holds it. Ticks
    /// are given in order, one each; a gap starts the run again.
    pub fn intent(&mut self, tick: u32, bytes: Vec<u8>) {
        if self
            .intents
            .back()
            .is_some_and(|(last, _)| last + 1 != tick)
        {
            self.intents.clear();
        }
        self.intents.push_back((tick, bytes));
        if self.intents.len() > MOST_INTENTS {
            self.intents.pop_front();
        }
    }

    /// Sends a state packet: what has arrived, and the intents not yet known to have.
    pub fn send(&mut self, transport: &mut impl ClientTransport) {
        let mut to = Writer::default();
        self.acks.outgoing().write(&mut to);
        self.intents
            .back()
            .map_or(0, |(tick, _)| *tick)
            .write(&mut to);
        to.ranged(self.intents.len() as u32, MOST_INTENTS as u32);
        // One equal to the one before it is a bit: a key held is most ticks.
        let mut before: Option<&Vec<u8>> = None;
        for (_, bytes) in &self.intents {
            to.bool(before == Some(bytes));
            if before != Some(bytes) {
                to.run(bytes);
            }
            before = Some(bytes);
        }
        transport.send(Lane::State, &to.finish());
        transport.flush();
    }

    /// Asks the server for an operation: it shows when the server orders it (D651).
    pub fn ask(&mut self, transport: &mut impl ClientTransport, bytes: &[u8]) {
        transport.send(Lane::Ordered, &Up::Ask(bytes.to_vec()).encoded());
    }

    /// The newest packet's word on time: the server's tick, the newest intent it holds and
    /// how early that came.
    pub fn stamp(&self) -> Option<Stamp> {
        self.stamp
    }

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    /// Every entity near, in no order.
    pub fn entities(&self) -> impl Iterator<Item = (EntityId, &Entity)> {
        self.entities.iter().map(|(&id, entity)| (id, entity))
    }
}
