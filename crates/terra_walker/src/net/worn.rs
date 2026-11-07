//! What each player wears, across the network (D652): a player's outfit, the items it shows
//! (`character::Outfit`), is a part of its entity, told to everyone near, so a guest on the
//! host and a puppet in a joined game are dressed as their players are, not in the body's
//! look whole. The host's players' come from their own gear; a joined game says what its
//! player wears, which the host takes at its word: it is what is shown, not what is owned,
//! until the pack is the server's (D592, step 5).
//!
//! Not yet: the colours a piece is dyed (`Clothing::tint`), which go as the default.

use super::link::{Link, Puppet};
use super::{Guest, Host};
use crate::character::{Clothing, Outfit};
use crate::{IsLocalPlayer, Player};
use bevy::prelude::*;
use terra_net::wire::{Malformed, Reader, Wire, Writer};

/// The walker's third part of an entity: what it wears.
pub const WORN: usize = 2;
/// The first byte of a joined game's word on what its player wears, among what it asks.
pub const WEAR: u8 = 1;
/// The most pieces an outfit names on the wire, and the longest name.
const MOST_PIECES: usize = 32;
const MOST_NAME: usize = 64;

/// An outfit's items, by name, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wearing(pub Vec<String>);

impl Wearing {
    pub fn of(outfit: &Outfit) -> Wearing {
        let items = outfit.0.iter().take(MOST_PIECES);
        Wearing(items.map(|piece| piece.item.clone()).collect())
    }

    pub fn outfit(&self) -> Outfit {
        Outfit(self.0.iter().map(Clothing::new).collect())
    }
}

impl Wire for Wearing {
    fn write(&self, to: &mut Writer) {
        to.ranged(self.0.len().min(MOST_PIECES) as u32, MOST_PIECES as u32);
        for item in self.0.iter().take(MOST_PIECES) {
            item.write(to);
        }
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        let count = from.ranged(MOST_PIECES as u32)?;
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let item = String::read(from)?;
            if item.len() > MOST_NAME {
                return Err(Malformed::Long);
            }
            items.push(item);
        }
        Ok(Wearing(items))
    }
}

/// On the host, before the tick's telling: a guest wears what its game last said, and every
/// player's outfit is its entity's part.
pub fn host(
    mut host: ResMut<Host>,
    mut commands: Commands,
    guests: Query<(Entity, &Guest, Option<&Outfit>)>,
    players: Query<(Entity, &Outfit), With<Player>>,
) {
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        let wearing = match bytes.split_first() {
            Some((&WEAR, rest)) => Wearing::decoded(rest).ok(),
            _ => {
                // Another layer's.
                host.asked.push((peer, bytes));
                continue;
            }
        };
        let guest = guests.iter().find(|(_, guest, _)| guest.0 == peer);
        if let (Some(wearing), Some((entity, _, worn))) = (wearing, guest) {
            let outfit = wearing.outfit();
            if worn != Some(&outfit) {
                commands.entity(entity).insert(outfit);
            }
        }
    }
    for (player, outfit) in &players {
        host.set(player, WORN, &Wearing::of(outfit));
    }
}

/// What a joined game last said its player wears, so it says so again only on a change.
#[derive(Resource, Default)]
pub struct Said(Option<Vec<u8>>);

/// On a joined game: its own player's outfit is said to the host when it changes, and each
/// puppet wears what the host says its player does.
pub fn joined(
    mut link: ResMut<Link>,
    mut said: ResMut<Said>,
    mut commands: Commands,
    mine: Query<&Outfit, IsLocalPlayer>,
    puppets: Query<(Entity, &Puppet, Option<&Outfit>)>,
) {
    if let Some(outfit) = mine.iter().next() {
        let mut bytes = vec![WEAR];
        bytes.extend(Wearing::of(outfit).encoded());
        if link.tick().is_some() && said.0.as_ref() != Some(&bytes) {
            link.ask(&bytes);
            said.0 = Some(bytes);
        }
    }
    for (entity, puppet, worn) in &puppets {
        let told = link
            .client()
            .entity(puppet.id)
            .and_then(|entity| entity.part(WORN));
        let Some(Ok(wearing)) = told.map(|samples| Wearing::decoded(&samples.newer.bytes)) else {
            continue;
        };
        let outfit = wearing.outfit();
        if worn != Some(&outfit) {
            commands.entity(entity).insert(outfit);
        }
    }
}

/// With no link, what was said is forgotten, so the next host is told anew.
pub fn unjoined(mut said: ResMut<Said>) {
    if said.0.is_some() {
        said.0 = None;
    }
}
