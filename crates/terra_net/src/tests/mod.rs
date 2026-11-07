//! The two ends tested together with no window (docs/netcode.md, "Tests"): a toy game on the
//! in-memory network, whose rules are a few lines, so what is tested is the carrying. Things
//! stand on a grid of half metres and step by intent; an operation marks a place; a player
//! has a purse only it is told of.
//!
//! [`Table::mismatch`] is the check the tests gate on: what a client holds against what the
//! server has. Each test that passes on it has a planted fault beside it that must fail it
//! (D632).

mod handed;
mod hostile;
mod milestone;
mod rooms;
mod soak;

use crate::client::{Client, Event as Heard};
use crate::identity::Identity;
use crate::interest::{Place, cell_of, distance_squared};
use crate::proto::{EntityId, Kind, Reason, Region, RoomId};
use crate::server::{Audience, Config, Event, PartSpec, Server};
use crate::transport::loopback::{Conditions, LoopbackClient, LoopbackServer, Network};
use crate::transport::{ClientTransport, FromServer, Lane, Peer};
use crate::wire::Wire;
use std::collections::BTreeMap;
use terra_gen::hash::splitmix64;

const VERSION: u64 = 0x7E44A;
const ROOM: RoomId = RoomId(1);
const PLAYER: Kind = Kind(1);
const CREATURE: Kind = Kind(2);
/// Where a thing stands, to everyone; and a player's purse, to its owner.
const MOTION: usize = 0;
const PURSE: usize = 1;
/// A grid step, m, and how far from the middle things roam, steps.
const STEP_M: f64 = 0.5;
const ROAM: i32 = 320;
/// How many ticks ahead of the server's a seat stamps its intents: over the round trip.
const LEAD: u32 = 12;
const TICK_S: f64 = 1.0 / 64.0;

fn place(at: [i32; 2]) -> Place {
    [f64::from(at[0]) * STEP_M, 0.0, f64::from(at[1]) * STEP_M]
}

/// A step each way from a draw: -1, 0 or 1.
fn wander(draw: u64) -> [i32; 2] {
    [(draw % 3) as i32 - 1, ((draw >> 8) % 3) as i32 - 1]
}

/// A client's transport with faults to plant: a packet down swallowed, the state lane up cut.
struct Tap {
    inner: LoopbackClient,
    /// Swallows the ordered packet down of this number that carries an operation.
    swallow_op: Option<usize>,
    ops_seen: usize,
    /// Nothing goes up the state lane: no acknowledgement reaches the server.
    mute_state: bool,
}

impl ClientTransport for Tap {
    fn poll(&mut self, into: &mut Vec<FromServer>) {
        self.inner.poll(into);
        let (swallow, seen) = (self.swallow_op, &mut self.ops_seen);
        into.retain(|arrival| {
            // An operation's message begins with its tag, 2 of 0..=4.
            let op = matches!(arrival, FromServer::Packet(Lane::Ordered, bytes) if bytes.first().is_some_and(|byte| byte & 7 == 2));
            if op {
                *seen += 1;
            }
            !(op && swallow == Some(*seen - 1))
        });
    }
    fn send(&mut self, lane: Lane, bytes: &[u8]) {
        if !(self.mute_state && lane == Lane::State) {
            self.inner.send(lane, bytes);
        }
    }
    fn close(&mut self) {
        self.inner.close();
    }
}

/// A player's end: its client, what it has been told, and its script.
struct Seat {
    tap: Tap,
    client: Client,
    you: Option<EntityId>,
    /// The operations it was sent, a region's in order.
    records: BTreeMap<Region, Vec<Vec<u8>>>,
    ready_after: Option<u32>,
    closed: Option<Reason>,
    /// The tick its next intent is for.
    next: u32,
    ticks: u32,
    seed: u64,
}

/// A server and its seats on one network.
struct Table {
    network: Network,
    wire: LoopbackServer,
    server: Server,
    seats: Vec<Seat>,
    /// The truth: where each thing stands, whose each player is, each purse, the records.
    stands: BTreeMap<EntityId, [i32; 2]>,
    players: BTreeMap<Peer, EntityId>,
    purses: BTreeMap<EntityId, u32>,
    creatures: Vec<EntityId>,
    records: BTreeMap<Region, Vec<Vec<u8>>>,
    /// Things move and players ask for operations.
    lively: bool,
    events: Vec<Event>,
    heard: Vec<Heard>,
    /// Time spent in the server's own calls, for a reading.
    server_s: f64,
}

impl Table {
    fn new() -> Table {
        let network = Network::new();
        let parts = vec![
            PartSpec {
                audience: Audience::Public,
                weight: 1.0,
            },
            PartSpec {
                audience: Audience::Owner,
                weight: 0.5,
            },
        ];
        Table {
            wire: network.server(),
            network,
            server: Server::new(Config::new(VERSION), parts),
            seats: Vec::new(),
            stands: BTreeMap::new(),
            players: BTreeMap::new(),
            purses: BTreeMap::new(),
            creatures: Vec::new(),
            records: BTreeMap::new(),
            lively: true,
            events: Vec::new(),
            heard: Vec::new(),
            server_s: 0.0,
        }
    }

    /// A new player connects over a link and says hello.
    fn sit(&mut self, conditions: Conditions) -> usize {
        let seed = splitmix64(self.seats.len() as u64 + 1);
        let mut seat = Seat {
            tap: Tap {
                inner: self.network.connect(conditions),
                swallow_op: None,
                ops_seen: 0,
                mute_state: false,
            },
            client: Client::new(),
            you: None,
            records: BTreeMap::new(),
            ready_after: None,
            closed: None,
            next: 0,
            ticks: 0,
            seed,
        };
        seat.client.hello(
            &mut seat.tap,
            VERSION,
            &Identity::from_secret([self.seats.len() as u8 + 1; 32]),
            "player",
        );
        self.seats.push(seat);
        self.seats.len() - 1
    }

    fn creatures(&mut self, count: usize) {
        for index in 0..count {
            let draw = splitmix64(0xC0FFEE + index as u64);
            let at = [(draw % 400) as i32 - 200, ((draw >> 16) % 400) as i32 - 200];
            let id = self.server.spawn(ROOM, CREATURE, place(at));
            self.server.set_as(id, MOTION, &at);
            self.stands.insert(id, at);
            self.creatures.push(id);
        }
    }

    fn walk(&mut self, id: EntityId, by: [i32; 2]) {
        let Some(at) = self.stands.get_mut(&id) else {
            return;
        };
        *at = [
            (at[0] + by[0]).clamp(-ROAM, ROAM),
            (at[1] + by[1]).clamp(-ROAM, ROAM),
        ];
        self.server.place(id, place(*at));
        self.server.set_as(id, MOTION, at);
    }

    /// One tick of the server and of every seat.
    fn tick(&mut self) {
        self.network.advance(TICK_S);
        let began = std::time::Instant::now();
        self.server.poll(&mut self.wire, &mut self.events);
        for event in std::mem::take(&mut self.events) {
            match event {
                Event::Hello { peer, .. } => {
                    let at = [peer.0 as i32 * 8, 0];
                    let id = self.server.spawn(ROOM, PLAYER, place(at));
                    self.server.set_as(id, MOTION, &at);
                    self.server.set_as(id, PURSE, &0u32);
                    self.stands.insert(id, at);
                    self.purses.insert(id, 0);
                    self.players.insert(peer, id);
                    self.server
                        .welcome(&mut self.wire, peer, ROOM, id, b"toy realm");
                }
                // The toy's rule: an operation is a mark at a place on the grid, nine bytes.
                Event::Asked { peer, bytes } => {
                    let (Ok(at), Some(&id)) = (
                        <[i32; 2]>::decoded(&bytes[..bytes.len().min(8)]),
                        self.players.get(&peer),
                    ) else {
                        continue;
                    };
                    let (region, number) = self.server.order(ROOM, place(at), &bytes);
                    let record = self.records.entry(region).or_default();
                    assert_eq!(record.len() as u32, number);
                    record.push(bytes);
                    let purse = self.purses.entry(id).or_insert(0);
                    *purse += 1;
                    self.server.set_as(id, PURSE, &*purse);
                }
                Event::Said { .. } | Event::Ready { .. } => {}
                Event::Left { peer } => {
                    if let Some(id) = self.players.remove(&peer) {
                        self.server.despawn(id);
                        self.stands.remove(&id);
                        self.purses.remove(&id);
                    }
                }
            }
        }
        let tick = self.server.tick();
        for (peer, id) in self.players.clone() {
            if let Some(&[x, z]) = self
                .server
                .intent(peer)
                .and_then(|bytes| bytes.first_chunk::<2>())
            {
                self.walk(id, [i32::from(x) - 1, i32::from(z) - 1]);
            }
        }
        if self.lively {
            for index in 0..self.creatures.len() {
                let id = self.creatures[index];
                // A creature moves one tick in four, each on its own.
                let draw = splitmix64(u64::from(tick) << 20 | u64::from(id.0));
                if draw >> 60 < 4 {
                    self.walk(id, wander(draw));
                }
            }
        }
        self.server.step(&mut self.wire);
        self.server_s += began.elapsed().as_secs_f64();

        for seat in &mut self.seats {
            seat.client.poll(&mut seat.tap, &mut self.heard);
            for heard in self.heard.drain(..) {
                match heard {
                    Heard::Welcome {
                        tick, you, realm, ..
                    } => {
                        assert_eq!(realm, b"toy realm");
                        seat.you = you;
                        seat.next = tick + LEAD;
                    }
                    Heard::Op {
                        region,
                        number,
                        bytes,
                    } => {
                        let record = seat.records.entry(region).or_default();
                        // The ordered lane's promise, unless a fault was planted.
                        if seat.tap.swallow_op.is_none() {
                            assert_eq!(record.len() as u32, number, "an operation out of order");
                        }
                        record.push(bytes);
                    }
                    Heard::Ready => seat.ready_after = Some(seat.ticks),
                    Heard::Closed(reason) => seat.closed = Some(reason),
                    Heard::Entered(_)
                    | Heard::Handed
                    | Heard::Gone(_)
                    | Heard::Happened { .. }
                    | Heard::Said { .. }
                    | Heard::Roster { .. } => {}
                }
            }
            seat.ticks += 1;
            if seat.you.is_none() || seat.closed.is_some() {
                continue;
            }
            let draw = splitmix64(seat.seed ^ u64::from(seat.next));
            let step = if self.lively { wander(draw) } else { [0, 0] };
            seat.client
                .intent(seat.next, vec![(step[0] + 1) as u8, (step[1] + 1) as u8]);
            seat.next += 1;
            seat.client.send(&mut seat.tap);
            // An operation about once a second, somewhere it roams.
            if self.lively && draw >> 58 == 0 {
                let at = [
                    ((draw >> 20) % 640) as i32 - 320,
                    ((draw >> 36) % 640) as i32 - 320,
                ];
                let mut ask = at.encoded();
                ask.push(draw as u8);
                seat.client.ask(&mut seat.tap, &ask);
            }
        }
    }

    fn run(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.tick();
        }
    }

    /// Lets everything come to rest: nobody moves or asks, for long enough that every packet
    /// lands, every resend is made and what is near is looked up again.
    fn settle(&mut self) {
        self.lively = false;
        self.run(192);
    }

    /// The check: the first thing a seat holds that is not as the server has it, or none. A
    /// thing within the radius must be held as it stands; one more than a cube beyond must
    /// not be; a player's purse is held by its owner alone; the records of the regions round
    /// it are whole.
    fn mismatch(&self, seat: usize) -> Option<String> {
        let seat = &self.seats[seat];
        let you = seat.you?;
        let eye = place(*self.stands.get(&you)?);
        let config = Config::new(VERSION);
        let (near, far) = (config.radius_m, config.radius_m + config.cell_m);
        for (&id, &at) in &self.stands {
            let away = distance_squared(eye, place(at)).sqrt();
            let held = seat.client.entity(id);
            if away > far {
                if held.is_some() {
                    return Some(format!("{id:?} is {away:.0} m away and still held"));
                }
                continue;
            }
            let Some(held) = held else {
                if away <= near {
                    return Some(format!("{id:?} is {away:.0} m away and not held"));
                }
                continue;
            };
            let motion = held.part(MOTION).map(|samples| samples.newer.bytes.clone());
            if motion != Some(at.encoded()) {
                return Some(format!("{id:?} stands at {at:?}, held as {motion:?}"));
            }
            let purse = held.part(PURSE).map(|samples| samples.newer.bytes.clone());
            let truth = (id == you).then(|| self.purses[&id].encoded());
            if purse != truth {
                return Some(format!("{id:?}'s purse is {truth:?}, held as {purse:?}"));
            }
            if held.mine != (id == you) {
                return Some(format!(
                    "{id:?} is held as {}",
                    if held.mine { "its own" } else { "another's" }
                ));
            }
        }
        if let Some((id, _)) = seat
            .client
            .entities()
            .find(|(id, _)| !self.stands.contains_key(id))
        {
            return Some(format!("{id:?} is no more and still held"));
        }
        let low = cell_of(eye.map(|along| along - near), config.region_m);
        let high = cell_of(eye.map(|along| along + near), config.region_m);
        for (region, record) in &self.records {
            let round = (0..3).all(|axis| (low[axis]..=high[axis]).contains(&region[axis]));
            if round && seat.records.get(region) != Some(record) {
                let held = seat.records.get(region).map_or(0, Vec::len);
                return Some(format!(
                    "region {region:?}'s record has {} operations, {held} held or not the same",
                    record.len()
                ));
            }
        }
        None
    }
}

/// A link as v0's soak asks: 100 ms there and back, 1% lost, some jitter and copies.
fn rough(seed: u64) -> Conditions {
    Conditions {
        latency_s: 0.05,
        jitter_s: 0.01,
        loss: 0.01,
        copies: 0.01,
        seed,
    }
}
