//! A gate's state across a hosted session (D590: opened, it "stays open for everyone in that
//! world"; v0-goals: "decided once, for everyone"). The host's gate is the truth: once open,
//! the host orders it to every game, now and when one comes later, as it orders the ground's
//! changes (`terra_walker::net::ground`), at the gate and where each player stands. A joined player at a shut gate asks the host to
//! open it, saying the marks they bear; the host opens it if one of them opens gates.
//!
//! The host takes a joined game's marks at its word, as it does what a player wears and how
//! they look, until the server keeps characters (D592). And nobody crosses while a world is
//! shared: a hosted world is one world (D638) until its realms run as instances (D640), so
//! the gate says so in words.

use super::Portal;
use super::gates::WorldGate;
use crate::play::marks::{Kinds, Marks};
use bevy::prelude::*;
use terra_net::client::Event as Heard;
use terra_walker::net::link::Link;
use terra_walker::net::{Host, tell};
use terra_world::tick::Sim;

/// The first byte of the game's word on a gate, among every layer's operations and asks.
pub const GATE: u8 = 16;
/// The most bytes of marks an ask may carry.
const MOST_MARKS: usize = 512;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        FixedUpdate,
        (
            host.before(tell).run_if(resource_exists::<Host>),
            joined.run_if(resource_exists::<Link>),
        )
            .in_set(Sim::End),
    );
}

/// What a joined player says to have a shut gate opened: the marks they bear.
pub(super) fn ask(marks: &Marks) -> Vec<u8> {
    let mut bytes = vec![GATE];
    let keys: Vec<&str> = marks.0.iter().map(String::as_str).collect();
    bytes.extend(keys.join("\n").bytes().take(MOST_MARKS));
    bytes
}

/// The marks an ask says its player bears.
fn asked_with(rest: &[u8]) -> Marks {
    let text = std::str::from_utf8(&rest[..rest.len().min(MOST_MARKS)]).unwrap_or_default();
    Marks(text.lines().map(str::to_string).collect())
}

/// On the host, before the tick's telling: a joined player's ask opens the gate if they bear
/// a mark that opens gates, and an open gate is ordered to everyone once.
fn host(
    mut host: ResMut<Host>,
    kinds: Res<Kinds>,
    mut gates: Query<(&mut Portal, &Transform), With<WorldGate>>,
    players: Query<&Transform, With<crate::play::Player>>,
    mut ordered: Local<bool>,
) {
    if host.is_added() {
        *ordered = false;
    }
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        let Some((&GATE, rest)) = bytes.split_first() else {
            // Another layer's.
            host.asked.push((peer, bytes));
            continue;
        };
        if let Some(mark) = kinds.opens_gates(&asked_with(rest)) {
            for (mut gate, _) in &mut gates {
                if !gate.open {
                    gate.open = true;
                    info!("The gate opens to {}, borne by a joined player.", mark.name);
                }
            }
        }
    }
    if !*ordered && let Some((_, at)) = gates.iter().find(|(gate, _)| gate.open) {
        // Ordered at the gate, for whoever comes to it later, and where each player stands
        // now: an operation is told by area, and a gate may stand some hundreds of metres
        // from players who should know it is open (the two-game check failed on a world
        // whose gate was out of the joined player's area). Told twice it opens once.
        host.order(at.translation, &[GATE]);
        for player in &players {
            host.order(player.translation, &[GATE]);
        }
        *ordered = true;
    }
}

/// On a joined game: the host's order opens the gate here.
fn joined(mut link: ResMut<Link>, mut gates: Query<&mut Portal, With<WorldGate>>) {
    let (orders, rest): (Vec<Heard>, Vec<Heard>) =
        std::mem::take(&mut link.events).into_iter().partition(
            |event| matches!(event, Heard::Op { bytes, .. } if bytes.first() == Some(&GATE)),
        );
    link.events = rest;
    if orders.is_empty() {
        return;
    }
    for mut gate in &mut gates {
        if !gate.open {
            gate.open = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ask_says_the_marks_borne_and_nonsense_bears_none() {
        let marks = Marks(["troll".to_string(), "crown".to_string()].into());
        let bytes = ask(&marks);
        assert_eq!(bytes[0], GATE);
        assert_eq!(asked_with(&bytes[1..]), marks);
        assert_eq!(asked_with(&[0xff, 0xfe, 0x00]), Marks::default());
        assert_eq!(ask(&Marks::default()), [GATE]);
    }
}
