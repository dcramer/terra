//! Beginning and ending a session: this game's world hosted over UDP, or another's joined
//! (docs/netcode.md, D34, D638). One way for whoever asks: the title screen's Host and Join
//! (the game's) and a scripted run's `net.host` and `net.join` (`remote`). What goes wrong is
//! said in words a player can act on (D556).

use super::link::{Ended, Link, Puppet};
use super::{Host, parts, realm_of};
use bevy::prelude::*;
use std::hash::BuildHasher;
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};
use terra_net::identity::Identity;
use terra_net::server::Config;
use terra_net::transport::udp::{UdpClient, UdpServer};
use terra_world::planet::Planet;
use terra_world::start::Start;

/// The protocol's version: a game of another is refused. A hash of the wire's schema and the
/// game's data is to take its place (docs/netcode.md).
pub const VERSION: u64 = 6;
/// The UDP port a world is hosted on, and joined at when an address names none.
pub const PORT: u16 = 46_464;
/// The most clients a hosted world lets in.
const MOST_GUESTS: usize = 16;

const ALREADY: &str = "This game already hosts or has joined a world.";

/// Hosts this game's world at a port, its player named; guests start two metres east of the
/// world's start. Returns the address it listens on.
pub fn open(world: &mut World, port: u16, name: &str) -> Result<SocketAddr, String> {
    if world.contains_resource::<Host>() || world.contains_resource::<Link>() {
        return Err(ALREADY.into());
    }
    let Some(start) = world.get_resource::<Start>() else {
        return Err("There is no world to host yet: its start is not chosen.".into());
    };
    let beside = start.translation + start.right() * 2.0;
    let wire =
        UdpServer::bind((Ipv4Addr::UNSPECIFIED, port).into(), MOST_GUESTS).map_err(|why| {
            format!("Port {port} could not be opened: {why}. Is another game hosting on it?")
        })?;
    let address = wire.address();
    let mut config = Config::new(VERSION);
    // The server's challenges are drawn from this (D592): the operating system's randomness,
    // by the keys the standard library seeds its hashers with.
    config.secret = std::collections::hash_map::RandomState::new().hash_one(port);
    let realm = world
        .get_resource::<Planet>()
        .map(realm_of)
        .unwrap_or_default();
    world.insert_resource(Host::new(config, parts(), Box::new(wire), beside, realm).named(name));
    Ok(address)
}

/// An address as a player types it: a name or a number, the port only when it is not
/// [`PORT`]. A name is looked up, which waits on the network.
pub fn address(text: &str) -> Result<SocketAddr, String> {
    let text = text.trim();
    let not_one = || format!("\"{text}\" is not an address like 203.0.113.7 or a host's name.");
    if text.is_empty() {
        return Err("Type the host's address first.".into());
    }
    if let Ok(whole) = text.parse::<SocketAddr>() {
        return Ok(whole);
    }
    let with_port = if text.parse::<std::net::IpAddr>().is_ok() || !text.contains(':') {
        format!("{text}:{PORT}")
    } else {
        text.to_string()
    };
    let mut found = with_port.to_socket_addrs().map_err(|_| not_one())?;
    // The carrier's socket is IPv4.
    found.find(SocketAddr::is_ipv4).ok_or_else(not_one)
}

/// Joins the world hosted at an address, as an identity. `realm` is the world this game has
/// made, which the host's must then be (`realm_of`); empty when it will make whichever the
/// host welcomes it to ([`Link::realm`]).
pub fn join(
    world: &mut World,
    server: SocketAddr,
    identity: &Identity,
    name: &str,
    realm: Vec<u8>,
) -> Result<(), String> {
    if world.contains_resource::<Host>() || world.contains_resource::<Link>() {
        return Err(ALREADY.into());
    }
    let public = identity.public();
    let number = u64::from_le_bytes(public[..8].try_into().expect("eight of thirty-two bytes"));
    let wire = UdpClient::connect(server, number)
        .map_err(|why| format!("No socket could be opened to {server}: {why}."))?;
    world.remove_resource::<Ended>();
    world.insert_resource(Link::join(Box::new(wire), VERSION, identity, name, realm));
    Ok(())
}

/// Ends the session, if there is one: a host's guests are each told the host left, a joined
/// game leaves and its puppets go. Says which it was.
pub fn close(world: &mut World) -> Option<&'static str> {
    if let Some(mut host) = world.remove_resource::<Host>() {
        host.close();
        return Some("stopped hosting: every guest was told the host left");
    }
    let mut link = world.remove_resource::<Link>()?;
    link.leave();
    let mut puppets = world.query_filtered::<Entity, With<Puppet>>();
    for puppet in puppets.iter(world).collect::<Vec<_>>() {
        world.despawn(puppet);
    }
    Some("left the hosted world")
}

/// The seed and radius a realm's bytes name ([`realm_of`]), if they are a realm's.
pub fn realm_from(realm: &[u8]) -> Option<(u64, f32)> {
    let seed = u64::from_le_bytes(realm.get(..8)?.try_into().ok()?);
    let radius = f32::from_bits(u32::from_le_bytes(realm.get(8..12)?.try_into().ok()?));
    (realm.len() == 12 && radius.is_finite() && radius > 0.0).then_some((seed, radius))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_takes_the_port_only_when_it_is_given() {
        assert_eq!(
            address(" 203.0.113.7 "),
            Ok(SocketAddr::from(([203, 0, 113, 7], PORT)))
        );
        assert_eq!(
            address("203.0.113.7:5000"),
            Ok(SocketAddr::from(([203, 0, 113, 7], 5000)))
        );
        assert_eq!(
            address("localhost"),
            Ok(SocketAddr::from(([127, 0, 0, 1], PORT)))
        );
        for wrong in ["", "   ", "not an address", "1.2.3.4:port"] {
            assert!(address(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn a_realm_reads_back_as_its_seed_and_radius_and_nothing_else_does() {
        let mut realm = 77u64.to_le_bytes().to_vec();
        realm.extend_from_slice(&30_000f32.to_bits().to_le_bytes());
        assert_eq!(realm_from(&realm), Some((77, 30_000.0)));
        assert_eq!(realm_from(&realm[..11]), None);
        assert_eq!(realm_from(&[realm.clone(), vec![0]].concat()), None);
        realm[8..].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
        assert_eq!(realm_from(&realm), None);
    }
}
