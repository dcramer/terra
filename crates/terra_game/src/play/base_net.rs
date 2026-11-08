//! The base across the network (docs/netcode.md, build step 5; D21): a base is its
//! operations, in order, so it crosses as they do: the host orders the base whole as hosting
//! begins (its frame, its snapshot and the operations since), then every act done to it, by
//! anyone, where the base stands; a game near, or coming later, makes them in order. A
//! joined game's own acts are asked of the host, which does them as it does its own
//! player's, at no cost on the host: what they took was taken in the guest's game, the pack
//! not yet the server's (D592).
//!
//! Not yet: an act of a guest's the host refuses stays done in the guest's game alone;
//! what stands in a base (props, storage) and its fire and damage do not cross.

use crate::build::act::Building;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use terra_build::ops::Op;
use terra_build::save::Snapshot;
use terra_net::client::Event as Heard;
use terra_walker::net::link::Link;
use terra_walker::net::{Host, tell};
use terra_world::tick::Sim;

/// The first byte of what is said of the base, asked or ordered.
pub const BASE: u8 = 22;
/// The longest a word of the base is, in bytes: a base whose whole is longer is not
/// ordered whole, and is said so.
const MOST_BYTES: usize = 48 * 1024;

/// Where the base stands: its place and its turn.
type Frame = ([f32; 3], [f32; 4]);

/// What is said of the base.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum Word {
    /// The base as it stands: where, its snapshot, and the operations since.
    Whole(Frame, Box<Snapshot>, Vec<Op>),
    /// One act's operations.
    Act(Vec<Op>),
    /// The base was moved.
    Frame(Frame),
}

impl Word {
    fn said(&self) -> Option<Vec<u8>> {
        let mut bytes = vec![BASE];
        bytes.extend(ron::to_string(self).ok()?.into_bytes());
        (bytes.len() <= MOST_BYTES).then_some(bytes)
    }

    fn heard(bytes: &[u8]) -> Option<Word> {
        let (&BASE, rest) = bytes.split_first()? else {
            return None;
        };
        (rest.len() <= MOST_BYTES)
            .then(|| ron::de::from_bytes(rest).ok())
            .flatten()
    }
}

fn frame_of(at: &Transform) -> Frame {
    (at.translation.to_array(), at.rotation.to_array())
}

fn placed(frame: Frame) -> Transform {
    Transform::from_translation(Vec3::from(frame.0)).with_rotation(Quat::from_array(frame.1))
}

/// What of the base this game has told or been told: how many of its operations, where it
/// stood, whether the session has begun, and a joined game's own acts not yet come back.
#[derive(Resource, Default)]
pub(super) struct Shared {
    seen: usize,
    frame: Option<Frame>,
    begun: bool,
    pending: Vec<Vec<Op>>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Shared>().add_systems(
        FixedUpdate,
        (
            host.before(tell).run_if(resource_exists::<Host>),
            joined
                .after(terra_walker::net::link::show)
                .run_if(resource_exists::<Link>),
            alone.run_if(not(resource_exists::<Link>)),
        )
            .in_set(Sim::End)
            .run_if(terra_world::instance::in_world),
    );
}

/// The operations a build has gained since so many were seen: none if it has fewer, its
/// snapshot having taken them in.
fn gained(since: &[Op], seen: usize) -> &[Op] {
    since.get(seen..).unwrap_or(&[])
}

/// On the host, before the tick's telling: a guest's act is done, the base is ordered whole
/// once, and every act since, and its moving, is ordered where it stands.
fn host(mut host: ResMut<Host>, mut shared: ResMut<Shared>, mut building: Building) {
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        if bytes.first() != Some(&BASE) {
            // Another layer's.
            host.asked.push((peer, bytes));
            continue;
        }
        if let Some(Word::Act(ops)) = Word::heard(&bytes) {
            // Refused or done, it is said in the log as any act is.
            let _ = building.act_as_told("a guest's act", &ops);
        }
    }
    let at = building.site.frame.translation;
    let frame = frame_of(&building.site.frame);
    if !std::mem::replace(&mut shared.begun, true) {
        let site = &building.site;
        let snapshot = site
            .snapshot
            .clone()
            .unwrap_or_else(|| site.base.snapshot());
        let since = if site.snapshot.is_some() {
            site.since.clone()
        } else {
            Vec::new()
        };
        match Word::Whole(frame, Box::new(snapshot), since).said() {
            Some(word) => {
                info!("Base: told whole to those who join, {} bytes.", word.len());
                host.order(at, &word);
            }
            None => warn!(
                "The base is too large to be told whole to those who join ({MOST_BYTES} bytes at most): they see only what is built from now."
            ),
        }
        (shared.seen, shared.frame) = (site.since.len(), Some(frame));
        return;
    }
    if shared.frame != Some(frame) {
        shared.frame = Some(frame);
        if let Some(word) = Word::Frame(frame).said() {
            host.order(at, &word);
        }
    }
    let since = &building.site.since;
    let new = gained(since, shared.seen).to_vec();
    shared.seen = since.len();
    if !new.is_empty() {
        match Word::Act(new.clone()).said() {
            Some(word) => {
                info!("Base: told an act of {} operations.", new.len());
                host.order(at, &word);
            }
            None => warn!(
                "An act of {} operations on the base is too long to tell ({MOST_BYTES} bytes at most): those joined do not see it.",
                new.len()
            ),
        }
    }
}

/// On a joined game, at the tick's end: the host's words of the base are made, but its own
/// acts come back; and its own new acts are asked of the host.
pub(super) fn joined(
    mut link: ResMut<Link>,
    mut shared: ResMut<Shared>,
    mut building: Building,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !std::mem::replace(&mut shared.begun, true) {
        shared.seen = building.site.since.len();
    }
    let (words, rest): (Vec<Heard>, Vec<Heard>) =
        std::mem::take(&mut link.events).into_iter().partition(
            |event| matches!(event, Heard::Op { bytes, .. } if bytes.first() == Some(&BASE)),
        );
    link.events = rest;
    // Its own acts since the last look, before the host's are made over them.
    let own = gained(&building.site.since, shared.seen).to_vec();
    if !own.is_empty()
        && let Some(word) = Word::Act(own.clone()).said()
    {
        link.ask(&word);
        shared.pending.push(own);
    }
    for word in words {
        let Heard::Op { bytes, .. } = word else {
            continue;
        };
        match Word::heard(&bytes) {
            Some(Word::Whole(frame, snapshot, since)) => {
                info!(
                    "Base: the host's base taken whole, {} operations since its snapshot.",
                    since.len()
                );
                let Building {
                    site,
                    planet,
                    rules,
                    ..
                } = &mut building;
                site.move_to(placed(frame), &mut commands, planet, &mut meshes);
                site.restore(&snapshot, &since, &rules.rules);
                shared.pending.clear();
            }
            Some(Word::Frame(frame)) => {
                let Building { site, planet, .. } = &mut building;
                site.move_to(placed(frame), &mut commands, planet, &mut meshes);
            }
            Some(Word::Act(ops)) => match shared.pending.iter().position(|own| *own == ops) {
                // Its own, come back: done here already.
                Some(own) => {
                    shared.pending.remove(own);
                }
                None => {
                    let _ = building.act_as_told("the host's act", &ops);
                }
            },
            None => warn!("A word of the base from the host could not be read: left out."),
        }
    }
    shared.seen = building.site.since.len();
}

/// With no link, what a joined game was told is forgotten; a host's own count stands while
/// it hosts. (Not by the link being new: it is taken out and put back every tick.)
fn alone(mut shared: ResMut<Shared>, host: Option<Res<Host>>) {
    if host.is_none() && shared.begun {
        *shared = Shared::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_of_the_base_crosses_whole_and_nonsense_is_no_word() {
        let frame = ([1.0, 2.0, 3.0], [0.0, 0.0, 0.0, 1.0]);
        for word in [Word::Act(Vec::new()), Word::Frame(frame)] {
            let said = word.said().expect("short enough");
            assert_eq!(said[0], BASE);
            assert_eq!(Word::heard(&said), Some(word));
        }
        assert_eq!(Word::heard(&[BASE, b'?']), None);
        assert_eq!(Word::heard(b"no tag"), None);
        assert_eq!(placed(frame).translation, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn what_a_build_gained_is_what_lies_past_what_was_seen() {
        assert!(
            gained(&[], 3).is_empty(),
            "a snapshot took them in: nothing gained"
        );
        assert!(gained(&[], 0).is_empty());
    }
}
