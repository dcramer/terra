//! The ground across the network (D21, D29, D652): the host's operation log is the truth.
//! Whatever shapes the host's ground, by anyone, is ordered to every game near it, in the
//! log's order, and a game that comes later is sent the log: the same path. A joined game's
//! own dig shows at once in its own world and is asked of the host, which checks it is
//! within the player's reach (D51) before making it; when the host's order of it comes back,
//! the joined game knows it for its own and does not dig twice.
//!
//! Nothing here knows who dug or why: every change to the ground goes through
//! `Planet::apply` and is in its log, and the log is what is watched.
//!
//! Not yet: a dig the host refuses stays dug in the joined game that made it (a refusal comes
//! only of a game out of step or dishonest), and what a guest's dig yields is its own game's
//! to count, until the pack crosses (step 5).

use super::link::Link;
use super::{Guest, Host};
use bevy::prelude::*;
use terra_core::Operation;
use terra_net::client::Event as Heard;
use terra_net::wire::Wire;
use terra_world::planet::Planet;

/// The first byte of an operation of the ground's, among the operations of every layer.
pub const GROUND: u8 = 0;
/// How far from a guest's body a change it asks for may be, m: a tool's reach and what a
/// round trip of walking adds.
const REACH_M: f32 = 8.0;
/// How many of its own digs a joined game waits on at most; more, and the oldest are taken
/// as refused.
const MOST_PENDING: usize = 64;

fn tagged(operation: &Operation) -> Vec<u8> {
    let mut bytes = vec![GROUND];
    bytes.extend(operation.encoded());
    bytes
}

fn untagged(bytes: &[u8]) -> Option<Operation> {
    let (&GROUND, rest) = bytes.split_first()? else {
        return None;
    };
    Operation::decoded(rest).ok()
}

/// How much of the ground's log has been told or taken, and a joined game's own digs not yet
/// come back from the host.
#[derive(Resource, Default)]
pub struct Ground {
    seen: usize,
    pending: Vec<Operation>,
    /// A link is up and the log's length was taken when it came: what was dug before is this
    /// game's own past, not something to ask of the host.
    joined: bool,
}

impl Ground {
    /// How many of a joined game's digs the host has not yet ordered back.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

/// On the host, at the tick's end: what guests asked for is made where it is within their
/// reach, and everything the log has gained is ordered.
pub fn host(
    mut host: ResMut<Host>,
    mut planet: ResMut<Planet>,
    mut ground: ResMut<Ground>,
    guests: Query<(&Guest, &Transform)>,
) {
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        let Some(operation) = untagged(&bytes) else {
            // Another layer's.
            host.asked.push((peer, bytes));
            continue;
        };
        let body = guests.iter().find(|(guest, _)| guest.0 == peer);
        let near = operation
            .place()
            .zip(body)
            .is_some_and(|(place, (_, transform))| {
                place.distance(transform.translation) <= REACH_M
            });
        if near {
            planet.apply(&operation);
        } else {
            warn!("A guest asked to change the ground out of its reach: refused (D51).");
        }
    }
    let count = planet.operation_count();
    if count > ground.seen {
        for operation in planet.operations_from(ground.seen) {
            if let Some(place) = operation.place() {
                host.order(place, &tagged(&operation));
            }
        }
    }
    ground.seen = count;
}

/// On a joined game, at the tick's end: its own new digs are asked of the host, and the
/// host's orders are made, but for those that are its own come back.
pub fn joined(mut link: ResMut<Link>, mut planet: ResMut<Planet>, mut ground: ResMut<Ground>) {
    let count = planet.operation_count();
    if !ground.joined {
        (ground.joined, ground.seen) = (true, count);
        ground.pending.clear();
    }
    if count > ground.seen {
        for operation in planet.operations_from(ground.seen) {
            link.ask(&tagged(&operation));
            ground.pending.push(operation);
        }
        if ground.pending.len() > MOST_PENDING {
            let refused = ground.pending.len() - MOST_PENDING;
            warn!("The host has not made {refused} of this game's digs: they stay dug here alone.");
            ground.pending.drain(..refused);
        }
    }
    let (orders, rest): (Vec<Heard>, Vec<Heard>) =
        std::mem::take(&mut link.events).into_iter().partition(
            |event| matches!(event, Heard::Op { bytes, .. } if bytes.first() == Some(&GROUND)),
        );
    link.events = rest;
    for order in orders {
        let Heard::Op { bytes, .. } = order else {
            continue;
        };
        let Some(operation) = untagged(&bytes) else {
            continue;
        };
        match ground.pending.iter().position(|own| *own == operation) {
            Some(own) => {
                ground.pending.remove(own);
            }
            None => {
                planet.apply(&operation);
            }
        }
    }
    ground.seen = planet.operation_count();
}

/// With no link, a joined game's count is forgotten, so the next link begins anew.
pub fn unjoined(mut ground: ResMut<Ground>) {
    if ground.joined {
        ground.joined = false;
    }
}
