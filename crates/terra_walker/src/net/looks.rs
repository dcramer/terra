//! How each player looks, across the network (D111, D652): a player's own features (their
//! hair, beard and the colours of hair, skin and eyes, `creator::Features`) are a part of
//! its entity, told to everyone near, so a guest on the host and a puppet in a joined game
//! are the people their players made, and two players tell each other apart. The host's
//! players' are their own; a joined game says its player's, which the host takes at its
//! word, as it does what is worn (`worn`): a look, nothing owned. A name the data here does
//! not have shows as the first on offer (`creator::Creator::paint`), never as nothing.

use super::link::{Link, Puppet};
use super::{Guest, Host};
use crate::creator::Features;
use crate::{IsLocalPlayer, Player};
use bevy::prelude::*;
use terra_net::wire::{Malformed, Reader, Wire, Writer};

/// The walker's fourth part of an entity: how its player looks.
pub const LOOKS: usize = 3;
/// The first byte of a joined game's word on how its player looks, among what it asks.
pub const LOOK: u8 = 2;
/// The longest name of a feature on the wire.
const MOST_NAME: usize = 64;

fn name(from: &mut Reader) -> Result<String, Malformed> {
    let name = String::read(from)?;
    if name.len() > MOST_NAME {
        return Err(Malformed::Long);
    }
    Ok(name)
}

impl Wire for Features {
    fn write(&self, to: &mut Writer) {
        self.body.write(to);
        for piece in [&self.hair, &self.beard] {
            to.bool(piece.is_some());
            if let Some(piece) = piece {
                piece.write(to);
            }
        }
        for colour in [&self.hair_colour, &self.skin, &self.eyes] {
            colour.write(to);
        }
    }

    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        let body = name(from)?;
        let mut piece = || {
            Ok(if from.bool()? {
                Some(name(from)?)
            } else {
                None
            })
        };
        let (hair, beard) = (piece()?, piece()?);
        Ok(Features {
            body,
            hair,
            beard,
            hair_colour: name(from)?,
            skin: name(from)?,
            eyes: name(from)?,
        })
    }
}

/// On the host, before the tick's telling: a guest looks as its game last said, and every
/// player's features are its entity's part.
pub fn host(
    mut host: ResMut<Host>,
    mut commands: Commands,
    guests: Query<(Entity, &Guest, Option<&Features>)>,
    players: Query<(Entity, &Features), With<Player>>,
) {
    let asked = std::mem::take(&mut host.asked);
    for (peer, bytes) in asked {
        let features = match bytes.split_first() {
            Some((&LOOK, rest)) => Features::decoded(rest).ok(),
            _ => {
                // Another layer's.
                host.asked.push((peer, bytes));
                continue;
            }
        };
        let guest = guests.iter().find(|(_, guest, _)| guest.0 == peer);
        if let (Some(features), Some((entity, _, had))) = (features, guest)
            && had != Some(&features)
        {
            commands.entity(entity).insert(features);
        }
    }
    for (player, features) in &players {
        host.set(player, LOOKS, features);
    }
}

/// What a joined game last said of its player's looks, so it says so again only on a change.
#[derive(Resource, Default)]
pub struct Said(Option<Vec<u8>>);

/// On a joined game: its own player's features are said to the host when they change, and
/// each puppet looks as the host says its player does.
pub fn joined(
    mut link: ResMut<Link>,
    mut said: ResMut<Said>,
    mut commands: Commands,
    mine: Query<&Features, IsLocalPlayer>,
    puppets: Query<(Entity, &Puppet, Option<&Features>)>,
) {
    if let Some(features) = mine.iter().next() {
        let mut bytes = vec![LOOK];
        bytes.extend(features.encoded());
        if link.tick().is_some() && said.0.as_ref() != Some(&bytes) {
            link.ask(&bytes);
            said.0 = Some(bytes);
        }
    }
    for (entity, puppet, had) in &puppets {
        let told = link
            .client()
            .entity(puppet.id)
            .and_then(|entity| entity.part(LOOKS));
        let Some(Ok(features)) = told.map(|samples| Features::decoded(&samples.newer.bytes)) else {
            continue;
        };
        if had != Some(&features) {
            commands.entity(entity).insert(features);
        }
    }
}

/// With no link, what was said is forgotten, so the next host is told anew.
pub fn unjoined(mut said: ResMut<Said>) {
    if said.0.is_some() {
        said.0 = None;
    }
}
