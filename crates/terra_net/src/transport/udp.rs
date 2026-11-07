//! The transport over UDP (D34): `renet` 2 for connections, the two lanes as its channels and
//! fragments, and `renet_netcode` for the netcode standard over a socket. For friends by
//! direct address; Steam's relay is `renet_steam` behind the same seam, later.
//!
//! Neither end reads a clock (a shared crate's rule): the caller says how much time has
//! passed ([`UdpServer::advance`], [`UdpClient::advance`]), as it does for the in-memory
//! network. The connection is netcode's "unsecure" kind: who a player is, is proven above
//! the transport (D592), not by it.

use super::{ClientTransport, FromClients, FromServer, Lane, Peer, ServerTransport};
use renet::{ChannelConfig, ConnectionConfig, RenetClient, RenetServer, SendType, ServerEvent};
use renet_netcode::{
    ClientAuthentication, NetcodeClientTransport, NetcodeServerTransport, ServerAuthentication,
    ServerConfig,
};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::time::Duration;

/// What netcode tells one game's packets from another's by. The game's own version is checked
/// by the hello, which can say why in words.
const PROTOCOL: u64 = 0x7E22_A000_0001;
const ORDERED: u8 = 0;
const STATE: u8 = 1;
/// How long a lost ordered message waits to be sent again: a round trip of a poor link.
const RESEND: Duration = Duration::from_millis(200);
/// How long a peer being dropped is kept, so the reason sent before it arrives.
const LINGER: Duration = Duration::from_millis(500);

fn channels() -> ConnectionConfig {
    let lanes = vec![
        ChannelConfig {
            channel_id: ORDERED,
            max_memory_usage_bytes: 8 * 1024 * 1024,
            send_type: SendType::ReliableOrdered {
                resend_time: RESEND,
            },
        },
        ChannelConfig {
            channel_id: STATE,
            max_memory_usage_bytes: 1024 * 1024,
            send_type: SendType::Unreliable,
        },
    ];
    ConnectionConfig {
        server_channels_config: lanes.clone(),
        client_channels_config: lanes,
        ..ConnectionConfig::default()
    }
}

fn channel(lane: Lane) -> u8 {
    match lane {
        Lane::Ordered => ORDERED,
        Lane::State => STATE,
    }
}

fn failed(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(error.to_string())
}

/// A server's end over UDP.
pub struct UdpServer {
    renet: RenetServer,
    netcode: NetcodeServerTransport,
    now: Duration,
    /// Peers being dropped, and when each goes.
    leaving: Vec<(u64, Duration)>,
}

impl UdpServer {
    /// Listens at an address (port 0 for any free one) for at most so many clients.
    pub fn bind(address: SocketAddr, most_clients: usize) -> std::io::Result<UdpServer> {
        let socket = UdpSocket::bind(address)?;
        let config = ServerConfig {
            current_time: Duration::ZERO,
            max_clients: most_clients,
            protocol_id: PROTOCOL,
            public_addresses: vec![socket.local_addr()?],
            authentication: ServerAuthentication::Unsecure,
        };
        Ok(UdpServer {
            renet: RenetServer::new(channels()),
            netcode: NetcodeServerTransport::new(config, socket)?,
            now: Duration::ZERO,
            leaving: Vec::new(),
        })
    }

    /// The address it listens at.
    pub fn address(&self) -> SocketAddr {
        self.netcode.addresses()[0]
    }

    /// Time has passed: resends and timeouts run on it, and the socket is read.
    pub fn advance(&mut self, elapsed: Duration) {
        self.now += elapsed;
        self.renet.update(elapsed);
        // A socket's error on one packet is that packet lost, which the lanes already bear.
        let _ = self.netcode.update(elapsed, &mut self.renet);
        let now = self.now;
        let renet = &mut self.renet;
        self.leaving.retain(|&(peer, at)| {
            if at <= now {
                renet.disconnect(peer);
            }
            at > now
        });
    }
}

impl ServerTransport for UdpServer {
    fn poll(&mut self, into: &mut Vec<FromClients>) {
        while let Some(event) = self.renet.get_event() {
            into.push(match event {
                ServerEvent::ClientConnected { client_id } => FromClients::Joined(Peer(client_id)),
                ServerEvent::ClientDisconnected { client_id, .. } => {
                    FromClients::Left(Peer(client_id))
                }
            });
        }
        for peer in self.renet.clients_id() {
            for lane in [Lane::Ordered, Lane::State] {
                while let Some(bytes) = self.renet.receive_message(peer, channel(lane)) {
                    into.push(FromClients::Packet(Peer(peer), lane, bytes.to_vec()));
                }
            }
        }
    }

    fn send(&mut self, to: Peer, lane: Lane, bytes: &[u8]) {
        if self.renet.is_connected(to.0) {
            self.renet.send_message(to.0, channel(lane), bytes.to_vec());
        }
    }

    fn drop_peer(&mut self, peer: Peer) {
        self.leaving.push((peer.0, self.now + LINGER));
    }

    fn flush(&mut self) {
        self.netcode.send_packets(&mut self.renet);
    }
}

/// A client's end over UDP.
pub struct UdpClient {
    renet: RenetClient,
    netcode: NetcodeClientTransport,
    told_closed: bool,
}

impl UdpClient {
    /// Begins connecting to a server's address, as a client of a number no other client of
    /// that server has (the caller's to draw: this crate reads no clock and rolls no dice).
    pub fn connect(server: SocketAddr, client_id: u64) -> std::io::Result<UdpClient> {
        let any: SocketAddr = match server {
            SocketAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
            SocketAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, 0).into(),
        };
        let authentication = ClientAuthentication::Unsecure {
            protocol_id: PROTOCOL,
            client_id,
            server_addr: server,
            user_data: None,
        };
        let socket = UdpSocket::bind(any)?;
        Ok(UdpClient {
            renet: RenetClient::new(channels()),
            netcode: NetcodeClientTransport::new(Duration::ZERO, authentication, socket)
                .map_err(failed)?,
            told_closed: false,
        })
    }

    /// Whether the server has accepted the connection: nothing sent before it is carried.
    pub fn connected(&self) -> bool {
        self.renet.is_connected()
    }

    /// Time has passed, as [`UdpServer::advance`].
    pub fn advance(&mut self, elapsed: Duration) {
        self.renet.update(elapsed);
        let _ = self.netcode.update(elapsed, &mut self.renet);
    }
}

impl ClientTransport for UdpClient {
    fn poll(&mut self, into: &mut Vec<FromServer>) {
        for lane in [Lane::Ordered, Lane::State] {
            while let Some(bytes) = self.renet.receive_message(channel(lane)) {
                into.push(FromServer::Packet(lane, bytes.to_vec()));
            }
        }
        if self.renet.is_disconnected() && !self.told_closed {
            self.told_closed = true;
            into.push(FromServer::Closed);
        }
    }

    fn send(&mut self, lane: Lane, bytes: &[u8]) {
        self.renet.send_message(channel(lane), bytes.to_vec());
    }

    fn close(&mut self) {
        self.netcode.disconnect();
        self.renet.disconnect();
    }

    fn flush(&mut self) {
        let _ = self.netcode.send_packets(&mut self.renet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Client, Event as Heard};
    use crate::proto::{Kind, Reason, RoomId};
    use crate::server::{Audience, Config, Event, PartSpec, Server};

    const TICK: Duration = Duration::from_micros(15_625);

    /// A server and a client on this machine's own sockets: the same two ends as in memory,
    /// connecting, a part and an operation arriving, another version refused in words, and a
    /// leaving client told to the game.
    #[test]
    fn the_two_ends_talk_over_real_sockets() {
        let mut wire = UdpServer::bind((Ipv4Addr::LOCALHOST, 0).into(), 4).expect("a free port");
        let parts = vec![PartSpec {
            audience: Audience::Public,
            weight: 1.0,
        }];
        let mut server = Server::new(Config::new(7), parts);
        let mut seats: Vec<(UdpClient, Client, Vec<Heard>, bool)> = [1u64, 2]
            .map(|id| {
                (
                    UdpClient::connect(wire.address(), id).expect("a socket"),
                    Client::new(),
                    Vec::new(),
                    false,
                )
            })
            .into();
        let (mut events, mut left) = (Vec::new(), Vec::new());
        for step in 0..400u32 {
            wire.advance(TICK);
            server.poll(&mut wire, &mut events);
            for event in events.drain(..) {
                match event {
                    Event::Hello { peer, name, .. } => {
                        assert_eq!(name, "first");
                        let you = server.spawn(RoomId(0), Kind(1), [0.0; 3]);
                        server.set(you, 0, b"here");
                        server.welcome(&mut wire, peer, RoomId(0), you, b"realm");
                        server.order(RoomId(0), [1.0, 2.0, 3.0], b"dig");
                    }
                    Event::Left { peer } => left.push(peer),
                    Event::Asked { .. } => {}
                }
            }
            server.step(&mut wire);
            for (index, (tap, client, heard, greeted)) in seats.iter_mut().enumerate() {
                tap.advance(TICK);
                if tap.connected() && !*greeted {
                    *greeted = true;
                    // The second is a game of another version.
                    client.hello(
                        tap,
                        7 + index as u64,
                        [0; 32],
                        if index == 0 { "first" } else { "second" },
                    );
                }
                client.poll(tap, heard);
                client.send(tap);
                // The first leaves once it has what it came for.
                if index == 0 && step == 300 {
                    tap.close();
                }
            }
            // The machine's own sockets deliver at once, given a moment.
            std::thread::sleep(Duration::from_millis(1));
        }
        let (_, first, heard, _) = &seats[0];
        assert!(
            matches!(heard[0], Heard::Welcome { ref realm, .. } if realm == b"realm"),
            "{heard:?}"
        );
        assert!(
            heard.iter().any(
                |event| matches!(event, Heard::Op { bytes, number: 0, .. } if bytes == b"dig")
            )
        );
        assert!(heard.contains(&Heard::Ready));
        let (_, entity) = first.entities().next().expect("its own entity arrived");
        assert_eq!(entity.part(0).expect("its part").newer.bytes, b"here");
        assert!(entity.mine);
        assert_eq!(seats[1].2, [Heard::Closed(Reason::Version)]);
        assert_eq!(left, [Peer(1)]);
        assert_eq!(server.clients(), 0);
    }
}
