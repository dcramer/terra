//! What a stranger sends (D51, D636): refusals in words, and bytes that are no game of ours.

use super::*;
use crate::proto::Up;
use crate::transport::ServerTransport;

fn closed(table: &Table, seat: usize) -> Option<Reason> {
    table.seats[seat].closed
}

#[test]
fn another_version_is_refused_in_words() {
    let mut table = Table::new();
    let mut tap = table.network.connect(Conditions::PERFECT);
    let mut client = Client::new();
    client.hello(&mut tap, VERSION + 1, [0; 32], "old");
    table.run(2);
    let mut heard = Vec::new();
    client.poll(&mut tap, &mut heard);
    assert_eq!(heard, [Heard::Closed(Reason::Version)]);
    assert_eq!(
        Reason::Version.message(),
        "This world runs another version of the game."
    );
    assert_eq!(table.server.clients(), 0);
}

#[test]
fn a_full_world_refuses_and_the_host_leaving_ends_it_for_all() {
    let mut table = Table::new();
    for _ in 0..Config::new(VERSION).most_clients {
        table.sit(Conditions::PERFECT);
    }
    table.run(4);
    let extra = table.sit(Conditions::PERFECT);
    table.run(4);
    assert_eq!(closed(&table, extra), Some(Reason::Full));
    assert_eq!(closed(&table, 0), None);
    table.server.close(&mut table.wire, Reason::HostLeft);
    table.run(2);
    assert_eq!(closed(&table, 0), Some(Reason::HostLeft));
    assert_eq!(closed(&table, 7), Some(Reason::HostLeft));
}

/// Random bytes on both lanes, from a stranger and from a welcomed client: the server never
/// panics, drops whoever speaks nonsense on the ordered lane, and the others play on.
#[test]
fn nonsense_never_panics_the_server_and_costs_only_its_sender() {
    let mut table = Table::new();
    table.creatures(10);
    table.sit(rough(1));
    table.run(64);
    for round in 0..300u64 {
        let mut stranger = table.network.connect(Conditions::PERFECT);
        // Half of them are welcomed first, so the state lane's reader is reached.
        if round % 2 == 0 {
            stranger.send(
                Lane::Ordered,
                &Up::Hello {
                    version: VERSION,
                    identity: [1; 32],
                    name: "x".into(),
                }
                .encoded(),
            );
            table.run(1);
        }
        for packet in 0..8u64 {
            let draw = splitmix64(round << 8 | packet);
            let bytes: Vec<u8> = (0..draw % 48).map(|i| splitmix64(draw + i) as u8).collect();
            stranger.send(
                if packet < 6 {
                    Lane::State
                } else {
                    Lane::Ordered
                },
                &bytes,
            );
        }
        table.run(2);
    }
    assert!(
        table.server.malformed >= 300,
        "{} unreadable packets counted",
        table.server.malformed
    );
    // Every stranger was dropped, its body with it; the honest client is as the server.
    assert_eq!(table.server.clients(), 1);
    assert_eq!(table.players.len(), 1);
    table.settle();
    assert_eq!(table.mismatch(0), None);
}

/// A client fed nonsense: it never panics, whatever it makes of it.
#[test]
fn nonsense_never_panics_a_client() {
    let network = Network::new();
    let (mut wire, mut tap) = (network.server(), network.connect(Conditions::PERFECT));
    let mut client = Client::new();
    let mut heard = Vec::new();
    for round in 0..2000u64 {
        let draw = splitmix64(round);
        let bytes: Vec<u8> = (0..draw % 64)
            .map(|i| splitmix64(draw ^ (i << 32)) as u8)
            .collect();
        wire.send(
            Peer(1),
            if round % 3 == 0 {
                Lane::Ordered
            } else {
                Lane::State
            },
            &bytes,
        );
        client.poll(&mut tap, &mut heard);
    }
    assert!(
        client.malformed > 500,
        "{} unreadable packets counted",
        client.malformed
    );
}
