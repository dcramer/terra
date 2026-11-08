//! Hosting and joining by the remote protocol (D69, tools/sandbox): how a scripted run starts
//! a server in a running game, joins another's, and reads how it went, with no picture. The
//! title screen's Host and Join (v0's Front track) will make the same `Host` and `Link`.
//!
//! - `net.host {"port": 47000}`: this world is hosted on UDP at the port; guests start two
//!   metres east of the world's start.
//! - `net.join {"address": "127.0.0.1:47000", "name": "guest"}`: this game joins the world
//!   hosted there. It must have made the same world (the same `--seed`).
//! - `net.say {"text": "…"}`: words to everyone in the world, as the host or as a guest.
//! - `net.leave`: stop hosting (every guest is told the host left) or leave the hosted world.
//! - `net.state`: who is connected, where each guest and puppet stands and how it looks, how often this
//!   game's prediction was corrected, how many operations have shaped its ground and how many
//!   of its own the host has yet to order back, and why a link ended if one did.

use super::link::{Ended, Link, Puppet};
use super::session;
use super::{Guest, Host, realm_of};
use crate::IsLocalPlayer;
use crate::creator::Features;
use bevy::prelude::*;
use bevy::remote::{BrpResult, builtin_methods::parse_some};
use serde::Deserialize;
use serde_json::{Value, json};
use std::hash::{Hash, Hasher};
use terra_net::identity::Identity;
use terra_world::planet::Planet;
use terra_world::remote::{method, refused};

pub fn plugin(app: &mut App) {
    method(app, "net.host", host);
    method(app, "net.join", join);
    method(app, "net.state", state);
    method(app, "net.say", say);
    method(app, "net.leave", leave);
    method(app, "net.place_guests", place_guests);
}

#[derive(Deserialize)]
struct HostAsk {
    port: u16,
}

fn host(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let HostAsk { port } = parse_some(params)?;
    let address = session::open(world, port, "host").map_err(refused)?;
    Ok(json!({ "hosting": address.to_string() }))
}

#[derive(Deserialize)]
struct JoinAsk {
    address: String,
    name: String,
}

/// A scripted run's identity, from its name: the same name is the same player. A player's own
/// is a secret made once and kept on their machine (D592), which the title screen will give.
pub fn scripted_identity(name: &str) -> Identity {
    let mut secret = [0; 32];
    for (index, chunk) in secret.chunks_exact_mut(8).enumerate() {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (name, index).hash(&mut hasher);
        chunk.copy_from_slice(&hasher.finish().to_le_bytes());
    }
    Identity::from_secret(secret)
}

#[derive(Deserialize)]
struct PlaceAsk {
    /// Metres east and south of the start, as `warp` takes a place.
    at: [f32; 2],
}

/// `net.place_guests {"at": [east, south]}`: on the host, puts every guest's body on the
/// ground at a place, as `warp` puts this game's own player: a guest's body is the host's to
/// walk, so a script that wants a guest somewhere asks the host. Says how many it moved.
fn place_guests(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let PlaceAsk { at } = parse_some(params)?;
    if world.get_resource::<Host>().is_none() {
        return Err(refused(
            "This game hosts no world: host one first (net.host).",
        ));
    }
    let (Some(planet), Some(start)) = (
        world.get_resource::<Planet>(),
        world.get_resource::<terra_world::start::Start>(),
    ) else {
        return Err(refused("No world is made yet."));
    };
    let feet = planet.ground_at(start.at(at[0], at[1]));
    let mut guests = world.query_filtered::<&mut Transform, With<Guest>>();
    let mut moved = 0;
    for mut guest in guests.iter_mut(world) {
        guest.translation = feet;
        moved += 1;
    }
    Ok(json!({ "moved": moved }))
}

fn join(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let JoinAsk { address, name } = parse_some(params)?;
    let server = session::address(&address).map_err(refused)?;
    // A scripted run has made its world already: the host's must be the same.
    let realm = world
        .get_resource::<Planet>()
        .map(realm_of)
        .unwrap_or_default();
    session::join(world, server, &scripted_identity(&name), &name, realm).map_err(refused)?;
    Ok(json!({ "joining": address }))
}

#[derive(Deserialize)]
struct SayAsk {
    text: String,
}

fn say(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let SayAsk { text } = parse_some(params)?;
    if let Some(mut host) = world.get_resource_mut::<Host>() {
        host.say(&text);
    } else if let Some(mut link) = world.get_resource_mut::<Link>() {
        link.say(&text);
    } else {
        return Err(refused(
            "This game neither hosts nor has joined a world: there is nobody to say it to.",
        ));
    }
    Ok(json!("said"))
}

fn leave(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    session::close(world)
        .map(|what| json!(what))
        .ok_or_else(|| refused("This game neither hosts nor has joined a world."))
}

fn state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let place = |transform: &Transform| json!(transform.translation.to_array());
    let mut mine = world.query_filtered::<&Transform, IsLocalPlayer>();
    let mine = mine.iter(world).next().map(place);
    let mut guests = world.query::<(&Guest, &Transform, Option<&Features>)>();
    let guests: Vec<Value> = guests
        .iter(world)
        .map(|(guest, transform, looks)| {
            json!({ "peer": guest.0.0, "at": place(transform), "looks": looks })
        })
        .collect();
    let mut puppets = world.query::<(&Puppet, &Transform, Option<&Features>)>();
    let puppets: Vec<Value> = puppets
        .iter(world)
        .map(|(puppet, transform, looks)| {
            json!({ "id": puppet.id.0, "at": place(transform), "looks": looks })
        })
        .collect();
    let host = world.get_resource::<Host>();
    let link = world.get_resource::<Link>();
    Ok(json!({
        "mine": mine,
        "hosting": host.is_some(),
        "clients": host.map(|host| host.server().clients()),
        "tick": host.map(|host| host.server().tick()).or(link.and_then(Link::tick)),
        "guests": guests,
        "joined": link.is_some(),
        "ground": world.get_resource::<Planet>().map(Planet::operation_count),
        "ground_pending": world.get_resource::<super::ground::Ground>().map(super::ground::Ground::pending),
        "players": host.map(Host::players).or(link.map(|link| link.players().to_vec())),
        "ended": world.get_resource::<Ended>().map(|ended| ended.0.clone()),
        "corrections": link.map(Link::corrections),
        "missed": link.map(Link::missed),
        "answered": link.map(Link::answered),
        "puppets": puppets,
    }))
}
