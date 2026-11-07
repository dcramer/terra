//! Hosting and joining by the remote protocol (D69, tools/sandbox): how a scripted run starts
//! a server in a running game, joins another's, and reads how it went, with no picture. The
//! title screen's Host and Join (v0's Front track) will make the same `Host` and `Link`.
//!
//! - `net.host {"port": 47000}`: this world is hosted on UDP at the port; guests start two
//!   metres east of the world's start.
//! - `net.join {"address": "127.0.0.1:47000", "name": "guest"}`: this game joins the world
//!   hosted there. It must have made the same world (the same `--seed`).
//! - `net.state`: who is connected, where each guest and puppet stands, and how often this
//!   game's prediction was corrected.

use super::link::{Link, Puppet};
use super::{Guest, Host, parts};
use crate::IsLocalPlayer;
use bevy::prelude::*;
use bevy::remote::{BrpResult, builtin_methods::parse_some};
use serde::Deserialize;
use serde_json::{Value, json};
use std::hash::{BuildHasher, Hash, Hasher};
use std::net::{Ipv4Addr, SocketAddr};
use terra_net::identity::Identity;
use terra_net::server::Config;
use terra_net::transport::udp::{UdpClient, UdpServer};
use terra_world::remote::{method, refused};
use terra_world::start::Start;

/// The protocol's version: a game of another is refused. A hash of the wire's schema and the
/// game's data is to take its place (docs/netcode.md).
pub const VERSION: u64 = 1;
/// The most clients a hosted world lets in.
const MOST_GUESTS: usize = 16;

pub fn plugin(app: &mut App) {
    method(app, "net.host", host);
    method(app, "net.join", join);
    method(app, "net.state", state);
}

#[derive(Deserialize)]
struct HostAsk {
    port: u16,
}

fn host(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let HostAsk { port } = parse_some(params)?;
    if world.contains_resource::<Host>() || world.contains_resource::<Link>() {
        return Err(refused("This game already hosts or has joined a world."));
    }
    let Some(start) = world.get_resource::<Start>() else {
        return Err(refused(
            "There is no world to host yet: its start is not chosen.",
        ));
    };
    let beside = start.translation + start.right() * 2.0;
    let wire = UdpServer::bind((Ipv4Addr::UNSPECIFIED, port).into(), MOST_GUESTS)
        .map_err(|why| refused(format!("Port {port} could not be opened: {why}.")))?;
    let address = wire.address();
    let mut config = Config::new(VERSION);
    // The server's challenges are drawn from this (D592): the operating system's randomness,
    // by the keys the standard library seeds its hashers with.
    config.secret = std::collections::hash_map::RandomState::new().hash_one(port);
    world.insert_resource(Host::new(
        config,
        parts(),
        Box::new(wire),
        beside,
        Vec::new(),
    ));
    Ok(json!({ "hosting": address.to_string() }))
}

#[derive(Deserialize)]
struct JoinAsk {
    address: String,
    name: String,
}

/// A scripted run's identity, from its name: the same name is the same player. A player's own
/// is a secret made once and kept on their machine (D592), which the title screen will give.
fn scripted_identity(name: &str) -> Identity {
    let mut secret = [0; 32];
    for (index, chunk) in secret.chunks_exact_mut(8).enumerate() {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (name, index).hash(&mut hasher);
        chunk.copy_from_slice(&hasher.finish().to_le_bytes());
    }
    Identity::from_secret(secret)
}

fn join(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let JoinAsk { address, name } = parse_some(params)?;
    if world.contains_resource::<Host>() || world.contains_resource::<Link>() {
        return Err(refused("This game already hosts or has joined a world."));
    }
    let server: SocketAddr = address
        .parse()
        .map_err(|_| refused(format!("{address} is not an address like 127.0.0.1:47000.")))?;
    let identity = scripted_identity(&name);
    let public = identity.public();
    let number = u64::from_le_bytes(public[..8].try_into().expect("eight of thirty-two bytes"));
    let wire = UdpClient::connect(server, number)
        .map_err(|why| refused(format!("No socket could be opened to {address}: {why}.")))?;
    world.insert_resource(Link::join(Box::new(wire), VERSION, &identity, &name));
    Ok(json!({ "joining": address }))
}

fn state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let place = |transform: &Transform| json!(transform.translation.to_array());
    let mut mine = world.query_filtered::<&Transform, IsLocalPlayer>();
    let mine = mine.iter(world).next().map(place);
    let mut guests = world.query::<(&Guest, &Transform)>();
    let guests: Vec<Value> = guests
        .iter(world)
        .map(|(guest, transform)| json!({ "peer": guest.0 .0, "at": place(transform) }))
        .collect();
    let mut puppets = world.query::<(&Puppet, &Transform)>();
    let puppets: Vec<Value> = puppets
        .iter(world)
        .map(|(puppet, transform)| json!({ "id": puppet.id.0, "at": place(transform) }))
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
        "corrections": link.map(Link::corrections),
        "puppets": puppets,
    }))
}
