//! What each player holds in hand, across the network (docs/netcode.md, build step 5): the
//! items in a player's two hands ([`Hands`]), by name, are a part of its entity
//! (`terra_walker::net::HELD`), told to everyone near, so a guest on the host and a puppet in
//! a joined game carry what their players do: a torch, an axe, a shield. The host's own
//! come from its bar; a joined game says what its player holds, which the host takes at
//! its word, as it does what they wear: it is what is shown, not what is owned, until the
//! pack is the server's (D592).

use crate::held::Hands;
use crate::inventory::Items;
use bevy::prelude::*;
use terra_walker::net::link::{Link, Puppet};
use terra_walker::net::{Guest, Host, tell};
use terra_walker::{IsLocalPlayer, Player};
use terra_world::tick::Sim;

/// The first byte of what a joined player holds, as it is asked of the host.
pub const HANDS: u8 = 21;
/// No item's name is longer.
const MOST_NAME: usize = 64;

pub fn plugin(app: &mut App) {
    app.init_resource::<Said>().add_systems(
        FixedUpdate,
        (
            host.before(tell).run_if(resource_exists::<Host>),
            joined
                .after(terra_walker::net::link::show)
                .run_if(resource_exists::<Link>),
            unjoined.run_if(not(resource_exists::<Link>)),
        )
            .in_set(Sim::End),
    );
    terra_world::remote::method(app, "hands.state", state);
}

/// `hands.state`: what each player in this game holds, by name: this machine's own first,
/// then the others it has (a host's guests, a joined game's puppets).
fn state(
    In(_): In<Option<serde_json::Value>>,
    items: Res<Items>,
    players: Query<(&Hands, Has<terra_walker::LocalPlayer>), With<Player>>,
) -> bevy::remote::BrpResult {
    let name = |kind: Option<terra_items::ItemKind>| kind.map(|kind| items.name(kind).to_string());
    let hands = |mine: bool| -> Vec<serde_json::Value> {
        players
            .iter()
            .filter(|(_, local)| *local == mine)
            .map(|(hands, _)| serde_json::json!({ "main": name(hands.main), "off": name(hands.off) }))
            .collect()
    };
    Ok(serde_json::json!({ "mine": hands(true).first(), "others": hands(false) }))
}

/// Two hands' items as they cross: each a name, empty for an empty hand.
fn said(items: &Items, hands: &Hands) -> Vec<u8> {
    let mut bytes = Vec::new();
    for kind in [hands.main, hands.off] {
        let name = kind.map_or("", |kind| items.name(kind));
        let name = &name.as_bytes()[..name.len().min(MOST_NAME)];
        bytes.push(name.len() as u8);
        bytes.extend_from_slice(name);
    }
    bytes
}

/// The hands some bytes say, if they are two hands': a name not an item's is an empty hand.
fn heard(items: &Items, mut bytes: &[u8]) -> Option<Hands> {
    let mut hand = || {
        let (&length, rest) = bytes.split_first()?;
        let (name, rest) = rest.split_at_checked(length as usize)?;
        bytes = rest;
        Some(
            std::str::from_utf8(name)
                .ok()
                .and_then(|name| items.find(name)),
        )
    };
    let (main, off) = (hand()?, hand()?);
    bytes.is_empty().then_some(Hands {
        main,
        off,
        given: true,
    })
}

/// On the host, before the tick's telling: a guest holds what its game last said, and what
/// every player holds is its entity's part.
fn host(
    mut host: ResMut<Host>,
    items: Res<Items>,
    mut players: Query<(Entity, &mut Hands, Has<Guest>), With<Player>>,
) {
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        let Some((&HANDS, rest)) = bytes.split_first() else {
            // Another layer's.
            host.asked.push((peer, bytes));
            continue;
        };
        let guest = host
            .guest(peer)
            .and_then(|guest| players.get_mut(guest).ok());
        if let (Some((_, mut hands, true)), Some(held)) = (guest, heard(&items, rest))
            && *hands != held
        {
            *hands = held;
        }
    }
    for (player, hands, _) in &players {
        host.set_held(player, &said(&items, hands));
    }
}

/// What a joined game last said its player holds, so it says so again only on a change.
#[derive(Resource, Default)]
struct Said(Option<Vec<u8>>);

/// On a joined game: what its own player holds is said to the host when it changes, and
/// each puppet holds what the host says its player does.
fn joined(
    mut link: ResMut<Link>,
    mut last: ResMut<Said>,
    items: Res<Items>,
    mine: Query<&Hands, IsLocalPlayer>,
    mut puppets: Query<(&Puppet, &mut Hands), Without<terra_walker::LocalPlayer>>,
) {
    if let Some(hands) = mine.iter().next() {
        let mut bytes = vec![HANDS];
        bytes.extend(said(&items, hands));
        if link.tick().is_some() && last.0.as_ref() != Some(&bytes) {
            link.ask(&bytes);
            last.0 = Some(bytes);
        }
    }
    for (puppet, mut hands) in &mut puppets {
        let Some(held) = link.held(puppet).and_then(|bytes| heard(&items, bytes)) else {
            continue;
        };
        if *hands != held {
            *hands = held;
        }
    }
}

/// With no link, what was said is forgotten, so the next host is told anew.
fn unjoined(mut last: ResMut<Said>) {
    if last.0.is_some() {
        last.0 = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_hands_cross_by_name_and_nonsense_is_no_hands() {
        let items = Items::default();
        let (axe, torch) = (items.find("Stone axe"), items.find("Torch"));
        assert!(
            axe.is_some() && torch.is_some(),
            "the catalogue has an axe and a torch"
        );
        let hands = Hands {
            main: axe,
            off: torch,
            given: false,
        };
        let back = heard(&items, &said(&items, &hands)).unwrap();
        assert_eq!((back.main, back.off), (axe, torch));
        // Empty hands, and a name that is no item's, are empty.
        let empty = heard(&items, &said(&items, &Hands::default())).unwrap();
        assert_eq!((empty.main, empty.off), (None, None));
        assert_eq!(
            heard(&items, &[3, b'n', b'o', b'!', 0]).map(|hands| hands.main),
            Some(None)
        );
        // Cut short, or with more after it, it is not two hands.
        assert!(heard(&items, &[9, b'a']).is_none());
        assert!(heard(&items, &[0, 0, 7]).is_none());
    }
}
