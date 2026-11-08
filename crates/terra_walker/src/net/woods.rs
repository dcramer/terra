//! Felled trees across the network (docs/netcode.md, build step 5; D622): a tree one player
//! fells is felled for everyone. Which grown trees are felled is the world's record (their
//! feet, `terra_world::woods`); the host orders each new one where it stood, as it orders
//! the ground's changes, so a game near, or coming later, fells it again; a joined game asks
//! the host to fell the ones it fells, within its player's reach.
//!
//! Not yet: the stump and the logs a felling leaves are each game's own, so the other
//! player sees the tree gone, not what fell from it.

use super::link::Link;
use super::{Guest, Host};
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use terra_net::client::Event as Heard;
use terra_world::woods::{FellAgain, FelledWoods};

/// The first byte of a felled tree's foot, as it is asked and ordered.
pub const FELLED: u8 = 4;
/// A guest fells only a tree within this of them, in metres (D51): an axe's reach and the
/// lag of where the host has them.
const REACH_M: f32 = 10.0;
/// How often the felled are looked over for new ones, in ticks: four times a second.
const EVERY_TICKS: u32 = 16;

/// A felled tree's foot as it crosses.
fn said(foot: Vec3) -> Vec<u8> {
    let mut bytes = vec![FELLED];
    for value in foot.to_array() {
        bytes.extend(value.to_le_bytes());
    }
    bytes
}

/// The foot some bytes say, if they are a felled tree's.
fn heard(bytes: &[u8]) -> Option<Vec3> {
    let (&FELLED, rest) = bytes.split_first()? else {
        return None;
    };
    let rest: [u8; 12] = rest.try_into().ok()?;
    let value = |from: usize| f32::from_le_bytes(rest[from..from + 4].try_into().unwrap());
    let foot = Vec3::new(value(0), value(4), value(8));
    foot.is_finite().then_some(foot)
}

/// A foot as a key: its bits, as the woods keep it.
fn key(foot: Vec3) -> [u32; 3] {
    foot.to_array().map(f32::to_bits)
}

/// The felled this game has told of or been told of, so each crosses once.
#[derive(Resource, Default)]
pub struct Told {
    feet: HashSet<[u32; 3]>,
    /// Whether what was felled before the session began has been taken as known.
    begun: bool,
    since: u32,
}

/// On the host, at the tick's end: a tree a guest asks felled is felled if it stood within
/// their reach, and every tree newly felled, by anyone, is ordered where it stood.
pub fn host(
    mut host: ResMut<Host>,
    mut told: ResMut<Told>,
    felled: FelledWoods,
    guests: Query<(&Guest, &Transform)>,
    mut fell: Option<MessageWriter<FellAgain>>,
) {
    if host.is_added() {
        *told = Told::default();
    }
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        if bytes.first() != Some(&FELLED) {
            // Another layer's.
            host.asked.push((peer, bytes));
            continue;
        }
        let body = guests.iter().find(|(guest, _)| guest.0 == peer);
        match heard(&bytes).zip(body) {
            Some((foot, (_, at))) if foot.distance(at.translation) <= REACH_M => {
                if let Some(fell) = fell.as_mut() {
                    fell.write(FellAgain(vec![foot]));
                }
            }
            _ => warn!("A guest asked to fell a tree out of its reach: refused (D51)."),
        }
    }
    told.since += 1;
    if told.since < EVERY_TICKS {
        return;
    }
    told.since = 0;
    // Everything felled is ordered once: what was felled before hosting began too, so a
    // game joining later finds the woods as its host has them.
    for foot in felled.feet() {
        if told.feet.insert(key(foot)) {
            host.order(foot, &said(foot));
        }
    }
}

/// On a joined game, at the tick's end: the trees it has felled itself are asked of the
/// host, and those the host orders felled are felled here.
pub fn joined(
    mut link: ResMut<Link>,
    mut told: ResMut<Told>,
    felled: FelledWoods,
    mut fell: Option<MessageWriter<FellAgain>>,
) {
    if link.is_added() {
        *told = Told::default();
    }
    let (orders, rest): (Vec<Heard>, Vec<Heard>) =
        std::mem::take(&mut link.events).into_iter().partition(
            |event| matches!(event, Heard::Op { bytes, .. } if bytes.first() == Some(&FELLED)),
        );
    link.events = rest;
    let ordered: Vec<Vec3> = orders
        .iter()
        .filter_map(|order| match order {
            Heard::Op { bytes, .. } => heard(bytes),
            _ => None,
        })
        .filter(|foot| told.feet.insert(key(*foot)))
        .collect();
    if let (false, Some(fell)) = (ordered.is_empty(), fell.as_mut()) {
        fell.write(FellAgain(ordered));
    }
    told.since += 1;
    if told.since < EVERY_TICKS {
        return;
    }
    told.since = 0;
    // What stood felled here before the session began is this game's own past, not asked.
    let begun = std::mem::replace(&mut told.begun, true);
    for foot in felled.feet() {
        if told.feet.insert(key(foot)) && begun {
            link.ask(&said(foot));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_felled_trees_foot_crosses_to_the_bit_and_nonsense_is_no_tree() {
        let foot = Vec3::new(123.456, 30_001.5, -77.25);
        assert_eq!(heard(&said(foot)).map(key), Some(key(foot)));
        assert_eq!(heard(&[FELLED, 1, 2, 3]), None);
        assert_eq!(heard(&said(foot)[1..]), None);
        let mut not_a_place = said(foot);
        not_a_place[1..5].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(heard(&not_a_place), None);
    }
}
