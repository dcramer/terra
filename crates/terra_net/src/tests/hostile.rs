//! What a stranger sends (D51, D636): refusals in words, and bytes that are no game of ours.

use super::*;
use crate::proto::{Down, Up};
use crate::transport::ServerTransport;

fn closed(table: &Table, seat: usize) -> Option<Reason> {
    table.seats[seat].closed
}

/// A client's hello over a link of its own, played until the server has answered it: what
/// the client heard.
fn greeted(table: &mut Table, version: u64, identity: &Identity) -> Vec<Heard> {
    let mut tap = table.network.connect(Conditions::PERFECT);
    let mut client = Client::new();
    client.hello(&mut tap, version, identity, "guest");
    let mut heard = Vec::new();
    for _ in 0..4 {
        table.run(1);
        client.poll(&mut tap, &mut heard);
    }
    heard
}

#[test]
fn another_version_is_refused_in_words() {
    let mut table = Table::new();
    let heard = greeted(&mut table, VERSION + 1, &Identity::from_secret([9; 32]));
    assert_eq!(heard, [Heard::Closed(Reason::Version)]);
    assert_eq!(
        Reason::Version.message(),
        "This world runs another version of the game."
    );
    assert_eq!(table.server.clients(), 0);
}

/// What a raw link is told next on the ordered lane.
fn told(table: &mut Table, link: &mut impl ClientTransport) -> Down {
    let mut got = Vec::new();
    for _ in 0..4 {
        table.run(1);
        link.poll(&mut got);
        if let Some(FromServer::Packet(Lane::Ordered, bytes)) = got.first() {
            return Down::decoded(bytes).expect("the server's own message");
        }
    }
    panic!("the server said nothing: {got:?}");
}

/// The planted faults for identity (D592, D632): a hello naming one player and signed by
/// another, and an honest hello heard on one connection and sent again on the next. Each is
/// refused; the same hello signed for its own connection is let in.
#[test]
fn nobody_plays_as_another_and_a_hello_overheard_is_no_use() {
    let mut table = Table::new();
    let (ása, thief) = (
        Identity::from_secret([3; 32]),
        Identity::from_secret([4; 32]),
    );
    let hello = |signer: &Identity, challenge: &[u8; 32]| Up::Hello {
        version: VERSION,
        identity: ása.public(),
        name: "Ása".into(),
        signature: signer.sign(challenge, VERSION, "Ása"),
    };
    // Signed by the thief's key under Ása's name.
    let mut link = table.network.connect(Conditions::PERFECT);
    let Down::Challenge(first) = told(&mut table, &mut link) else {
        panic!("a server's first word is its challenge");
    };
    link.send(Lane::Ordered, &hello(&thief, &first).encoded());
    assert_eq!(told(&mut table, &mut link), Down::Refuse(Reason::Refused));
    // Ása's own hello for that connection, sent again on a new one.
    let mut link = table.network.connect(Conditions::PERFECT);
    let Down::Challenge(second) = told(&mut table, &mut link) else {
        panic!("a server's first word is its challenge");
    };
    assert_ne!(first, second, "every connection has its own challenge");
    link.send(Lane::Ordered, &hello(&ása, &first).encoded());
    assert_eq!(told(&mut table, &mut link), Down::Refuse(Reason::Refused));
    assert!(table.players.is_empty());
    // Signed for its own connection: welcomed, under Ása's public key.
    let heard = greeted(&mut table, VERSION, &ása);
    assert!(
        matches!(heard.first(), Some(Heard::Welcome { .. })),
        "{heard:?}"
    );
    assert_eq!(table.players.len(), 1);
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
/// panics, drops whoever speaks nonsense on the ordered lane (but what reads as a word before
/// any hello, which is ignored), lets none of them in, and the others play on.
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
            let mut honest = Client::new();
            honest.hello(
                &mut stranger,
                VERSION,
                &Identity::from_secret([round as u8; 32]),
                "x",
            );
            for _ in 0..3 {
                table.run(1);
                honest.poll(&mut stranger, &mut Vec::new());
            }
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
    // Unreadable, or not this server's by its first byte: either way counted and dropped.
    let dropped = table.server.malformed + table.server.strays;
    assert!(dropped >= 300, "{dropped} unreadable packets counted");
    // Every stranger was dropped, its body with it; the honest client is as the server.
    // A few whose bytes read as a word before any hello are ignored, as one handed on from
    // another server is (D640): connected and no more, as one who says nothing is.
    assert!(
        table.server.clients() <= 1 + 300 / 20,
        "{}",
        table.server.clients()
    );
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
    let dropped = client.malformed + client.strays;
    assert!(dropped > 500, "{} unreadable packets counted", dropped);
}
