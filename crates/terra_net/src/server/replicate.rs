//! What each client is sent (D652): what is near it, the operations it lacks, and each state
//! packet, filled by priority within its bytes; and what its acknowledgements settle.

use super::{Audience, Config, Entity, Flight, Known, Leaving, PartSpec, Room, Sent, Session};
use crate::ack::{Header, WATCHED};
use crate::interest::{cell_of, distance_squared};
use crate::proto::{Down, EntityId, MOST_EVENTS, MOST_INTENTS, MOST_PARTS, Stamp};
use crate::transport::{Lane, Peer, ServerTransport};
use crate::wire::{Malformed, Reader, Wire, Writer};
use terra_gen::hash::QuickMap;

/// What a client's sending reads of the server.
pub(super) struct World<'a> {
    pub config: &'a Config,
    pub parts: &'a [PartSpec],
    pub entities: &'a QuickMap<EntityId, Entity>,
    pub room: &'a Room,
    pub tick: u32,
}

/// Buffers kept from call to call, so a tick allocates nothing once warm.
#[derive(Default)]
pub(super) struct Scratch {
    near: Vec<u32>,
    acked: Vec<u16>,
    owed: Vec<Owed>,
    packet: Vec<u8>,
    pub encode: Vec<u8>,
}

/// An entity with parts a client is owed, and how much it matters now.
struct Owed {
    score: f32,
    id: EntityId,
    /// Bit `n`: part `n` is owed.
    mask: u8,
}

/// A state packet up: its acknowledgements settle what was sent, and, if it is new, its
/// intents are taken.
pub(super) fn heard(
    session: &mut Session,
    bytes: &[u8],
    tick: u32,
    scratch: &mut Scratch,
) -> Result<(), Malformed> {
    let mut from = Reader::new(bytes);
    let header = Header::read(&mut from)?;
    scratch.acked.clear();
    let new = session.acks.incoming(header, &mut scratch.acked);
    for &seq in &scratch.acked {
        settle(session, seq, tick);
    }
    if new {
        intents(session, &mut from, tick)?;
    }
    Ok(())
}

/// How many intents a client may have waiting: more are a fault or a flood.
const MOST_WAITING: usize = 64;
/// How far ahead of the server's tick an intent may be stamped.
const MOST_AHEAD: u32 = 256;

/// A state packet's intents: the newest's tick, how many, then each, oldest first, a tick
/// each. Those newer than any held are kept for their ticks (D651).
fn intents(session: &mut Session, from: &mut Reader, tick: u32) -> Result<(), Malformed> {
    let newest = u32::read(from)?;
    let count = from.ranged(MOST_INTENTS as u32)?;
    if count == 0 || newest > tick.saturating_add(MOST_AHEAD) || newest < count - 1 {
        // Nothing to say, or stamped where no honest clock is: ignored.
        return Ok(());
    }
    let mut before: &[u8] = &[];
    for at in newest + 1 - count..=newest {
        // A bit says it is the one before it again.
        let bytes = if from.bool()? { before } else { from.run()? };
        before = bytes;
        if session.heard.is_none_or(|heard| at > heard) {
            session.heard = Some(at);
            session.intents.push_back((at, bytes.to_vec()));
            if session.intents.len() > MOST_WAITING {
                session.intents.pop_front();
            }
        }
    }
    Ok(())
}

/// A packet of ours arrived: the client holds what it carried, unless it began to be sent
/// the thing anew, or to forget it, after the packet left.
fn settle(session: &mut Session, seq: u16, tick: u32) {
    let Some(index) = session.flights.iter().position(|flight| flight.seq == seq) else {
        return;
    };
    let Some(flight) = session.flights.remove(index) else {
        return;
    };
    let took = tick.saturating_sub(flight.tick) as f32;
    session.round_trip = Some(
        session
            .round_trip
            .map_or(took, |held| held + (took - held) * 0.125),
    );
    for sent in flight.sent {
        match sent {
            Sent::Event(id) => session.events.retain(|event| event.id != id),
            Sent::Part { id, part, version } => {
                if let Some(known) = session
                    .known
                    .get_mut(&id)
                    .filter(|known| flight.tick >= known.since)
                {
                    let acked = &mut known.acked[usize::from(part)];
                    *acked = (*acked).max(version);
                    known.met = true;
                }
            }
            Sent::Gone(id) => {
                if session
                    .leaving
                    .get(&id)
                    .is_some_and(|leaving| flight.tick >= leaving.since)
                {
                    session.leaving.remove(&id);
                }
            }
        }
    }
}

/// Looks up what a client is near: things within the radius begin to be sent, things a cube
/// beyond it stop (hysteresis, so a thing at the edge does not flicker), and the regions
/// round it are watched for operations.
pub(super) fn refresh(session: &mut Session, world: &World, scratch: &mut Scratch) {
    let Some(view) = session.view else {
        return;
    };
    let Some(eye) = world.entities.get(&view).map(|entity| entity.place) else {
        return;
    };
    let (radius, tick) = (world.config.radius_m, world.tick);
    let keep = radius + world.config.cell_m;
    session.sweep += 1;
    let sweep = session.sweep;
    scratch.near.clear();
    world.room.grid.near(eye, keep, &mut scratch.near);
    for &id in &scratch.near {
        let id = EntityId(id);
        let Some(entity) = world.entities.get(&id) else {
            continue;
        };
        let distance = distance_squared(eye, entity.place);
        match session.known.get_mut(&id) {
            Some(known) if distance <= keep * keep || id == view => known.sweep = sweep,
            Some(_) => {}
            None if distance <= radius * radius || id == view => {
                // Back before it was told to forget it: it is told the thing anew instead.
                session.leaving.remove(&id);
                session.known.insert(
                    id,
                    Known {
                        since: tick,
                        acked: [0; MOST_PARTS],
                        sent: [0; MOST_PARTS],
                        sent_at: None,
                        met: false,
                        sweep,
                    },
                );
            }
            None => {}
        }
    }
    let leaving = &mut session.leaving;
    session.known.retain(|&id, known| {
        let stays = known.sweep == sweep;
        if !stays {
            leaving.insert(
                id,
                Leaving {
                    since: tick,
                    sent_at: None,
                },
            );
        }
        stays
    });
    // The regions its radius touches: at most eight while a region is wider than the reach.
    session.watching.clear();
    let size = world.config.region_m;
    let low = cell_of(eye.map(|along| along - radius), size);
    let high = cell_of(eye.map(|along| along + radius), size);
    for x in low[0]..=high[0] {
        for y in low[1]..=high[1] {
            for z in low[2]..=high[2] {
                session.watching.push([x, y, z]);
            }
        }
    }
}

/// Sends a client the operations of the regions round it that it has not been sent: the next
/// of each record in order, on the ordered lane, so it holds exactly what was sent. A late
/// joiner's catching up is this and nothing else.
pub(super) fn operations(
    session: &mut Session,
    world: &World,
    peer: Peer,
    transport: &mut impl ServerTransport,
) {
    for &region in &session.watching {
        let Some(record) = world.room.records.get(&region) else {
            continue;
        };
        let sent = session.records.entry(region).or_insert(0);
        for (number, bytes) in record.iter().enumerate().skip(*sent as usize) {
            let op = Down::Op {
                region,
                number: number as u32,
                bytes: bytes.clone(),
            };
            transport.send(peer, Lane::Ordered, &op.encoded());
        }
        *sent = record.len() as u32;
    }
    if !session.settled && session.sweep > 0 {
        session.settled = true;
        transport.send(peer, Lane::Ordered, &Down::Settled.encoded());
    }
}

/// Whether something sent at a tick and not acknowledged is due again.
fn due(sent_at: Option<u32>, tick: u32, resend: u32) -> bool {
    sent_at.is_none_or(|at| tick.saturating_sub(at) >= resend)
}

/// How long something sent and not acknowledged waits to be sent again, ticks: half again
/// the client's measured round trip and a send, so a packet is called lost only once its
/// acknowledgement is truly overdue; the config's number until one is measured, and at most.
fn resend_after(session: &Session, config: &Config) -> u32 {
    session
        .round_trip
        .map_or(config.resend_ticks, |round_trip| {
            ((round_trip * 1.5) as u32 + config.send_every).clamp(4, config.resend_ticks)
        })
}

/// How long an event is worth telling, ticks: half a second.
const EVENT_LIFE: u32 = 32;

/// The events a client has not acknowledged, oldest first, as many as the packet and
/// [`MOST_EVENTS`] allow; those past their life are dropped untold.
fn events(
    session: &mut Session,
    tick: u32,
    resend: u32,
    budget: usize,
    to: &mut Writer,
    sent: &mut Vec<Sent>,
) {
    session
        .events
        .retain(|event| tick.saturating_sub(event.tick) <= EVENT_LIFE);
    let mut room = budget.saturating_sub(to.len() + 1);
    let chosen: Vec<usize> = session
        .events
        .iter()
        .enumerate()
        .filter(|(_, event)| due(event.sent_at, tick, resend))
        .filter(|(_, event)| {
            let fits = event.bytes.len() + 8 <= room;
            if fits {
                room -= event.bytes.len() + 8;
            }
            fits
        })
        .map(|(index, _)| index)
        .take(MOST_EVENTS)
        .collect();
    to.ranged(chosen.len() as u32, MOST_EVENTS as u32);
    for index in chosen {
        let event = &mut session.events[index];
        to.varint(u64::from(event.id));
        to.bits(u64::from(tick - event.tick), 6);
        to.run(&event.bytes);
        event.sent_at = Some(tick);
        sent.push(Sent::Event(event.id));
    }
}

/// How many entities that would not fit are passed over before a packet is called full.
const MOST_MISSES: usize = 8;

/// One state packet for a client: the entities it is to forget, then the parts it is owed,
/// the most pressing first (Tribes, Halo: Fiedler's priority accumulator), within the
/// packet's bytes. What does not fit waits, and presses harder for having waited.
pub(super) fn send(
    session: &mut Session,
    world: &World,
    peer: Peer,
    transport: &mut impl ServerTransport,
    scratch: &mut Scratch,
) {
    let (tick, config) = (world.tick, world.config);
    let budget = config.packet_bytes;
    let resend = resend_after(session, config);
    let mut to = Writer::reusing(std::mem::take(&mut scratch.packet));
    // Whose state this is, first: a client handed on from another server drops the rest
    // unread (D640).
    config.epoch.write(&mut to);
    let header = session.acks.outgoing();
    header.write(&mut to);
    let seq = header.seq;
    Stamp {
        tick,
        heard: session.heard,
        margin: session.heard.map_or(0, |heard| {
            (i64::from(heard) - i64::from(tick)).clamp(-128, 127) as i8
        }),
    }
    .write(&mut to);
    let mut sent = Vec::new();
    events(session, tick, resend, budget, &mut to, &mut sent);

    for (&id, leaving) in session.leaving.iter_mut() {
        if !due(leaving.sent_at, tick, resend) {
            continue;
        }
        if to.len() + 7 > budget {
            break;
        }
        to.bool(true);
        to.varint(u64::from(id.0));
        to.bool(true);
        leaving.sent_at = Some(tick);
        sent.push(Sent::Gone(id));
    }

    let eye = session
        .view
        .and_then(|view| world.entities.get(&view))
        .map(|entity| entity.place);
    // Nearness halves a part's weight at a quarter of the radius.
    let falloff = 16.0 / (config.radius_m * config.radius_m);
    scratch.owed.clear();
    for (&id, known) in session.known.iter() {
        let Some(entity) = world.entities.get(&id) else {
            continue;
        };
        let again = due(known.sent_at, tick, resend);
        let (mut mask, mut weight) = (0u8, 0.0f32);
        for (index, part) in entity.parts.iter().enumerate() {
            let Some(part) = part else { continue };
            let spec = &world.parts[index];
            let theirs = spec.audience == Audience::Public || entity.owner == Some(peer);
            if theirs
                && part.version > known.acked[index]
                && (part.version > known.sent[index] || again)
            {
                mask |= 1 << index;
                weight = weight.max(spec.weight);
            }
        }
        if mask == 0 {
            continue;
        }
        let waited = known
            .sent_at
            .map_or(64, |at| tick.saturating_sub(at).min(64));
        let score = if session.view == Some(id) {
            f32::MAX
        } else {
            let away = eye.map_or(0.0, |eye| distance_squared(eye, entity.place));
            weight * (waited + 1) as f32 / (1.0 + (away * falloff) as f32)
        };
        scratch.owed.push(Owed { score, id, mask });
    }
    // By score, then by id so equal scores fall the same way every run.
    scratch
        .owed
        .sort_unstable_by(|a, b| b.score.total_cmp(&a.score).then(a.id.cmp(&b.id)));

    let mut misses = 0;
    for owed in &scratch.owed {
        let (Some(entity), Some(known)) = (
            world.entities.get(&owed.id),
            session.known.get_mut(&owed.id),
        ) else {
            continue;
        };
        let parts = || (0..MOST_PARTS).filter(|index| owed.mask & (1 << index) != 0);
        let size: usize = 10
            + parts()
                .filter_map(|index| entity.parts[index].as_ref())
                .map(|part| part.bytes.len() + 3)
                .sum::<usize>();
        if to.len() + size > budget {
            misses += 1;
            if misses == MOST_MISSES {
                break;
            }
            continue;
        }
        to.bool(true);
        to.varint(u64::from(owed.id.0));
        to.bool(false);
        to.bool(!known.met);
        if !known.met {
            to.varint(u64::from(entity.kind.0));
            to.bool(entity.owner == Some(peer));
        }
        to.bits(u64::from(owed.mask), MOST_PARTS as u32);
        for index in parts() {
            let Some(part) = &entity.parts[index] else {
                continue;
            };
            to.run(&part.bytes);
            known.sent[index] = part.version;
            sent.push(Sent::Part {
                id: owed.id,
                part: index as u8,
                version: part.version,
            });
        }
        known.sent_at = Some(tick);
    }
    to.bool(false);

    let bytes = to.finish();
    transport.send(peer, Lane::State, &bytes);
    scratch.packet = bytes;
    if session.flights.len() == WATCHED {
        session.flights.pop_front();
    }
    session.flights.push_back(Flight { seq, tick, sent });
}
