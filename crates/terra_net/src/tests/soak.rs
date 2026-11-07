//! The soak (docs/v0-tracks.md, Server): four clients at 100 ms and 1% loss for thirty
//! minutes of the game's time, with no lasting mismatch, and the bytes a player a second read.

use super::*;

/// Half an hour of ticks.
const SOAK_TICKS: u32 = 30 * 60 * 64;

/// The bytes of state sent down, a packet, over two seconds at rest.
fn at_rest(table: &mut Table) -> f64 {
    let before = table.network.traffic();
    table.run(128);
    let after = table.network.traffic();
    (after.down_state - before.down_state) as f64
        / (after.down_packets - before.down_packets).max(1) as f64
}

/// A state packet with nothing owed, bytes: its header (65 bits), stamp (41), no events (4)
/// and end (1).
const EMPTY_PACKET: f64 = 14.0;

fn soaked(ticks: u32) -> Table {
    let mut table = Table::new();
    table.creatures(50);
    for seat in 0..4 {
        table.sit(rough(100 + seat));
    }
    // Looked at every eight seconds on the way: nothing a client holds is older than a
    // second while packets flow.
    for _ in 0..ticks / 512 {
        table.run(512);
        for seat in &table.seats {
            let newest = seat.client.stamp().expect("packets flow").tick;
            assert!(
                table.server.tick() - newest < 64,
                "a client's newest word is {} ticks old",
                table.server.tick() - newest
            );
        }
    }
    table
}

#[test]
fn four_clients_on_a_rough_link_for_half_an_hour_end_the_same() {
    let began = std::time::Instant::now();
    let mut table = soaked(SOAK_TICKS);
    let traffic = table.network.traffic();
    let (seconds, players) = (f64::from(SOAK_TICKS) * TICK_S, 4.0);
    table.settle();
    for seat in 0..4 {
        assert_eq!(table.mismatch(seat), None, "seat {seat}");
        assert_eq!(table.seats[seat].closed, None);
    }
    assert_eq!(table.server.malformed, 0);
    // Once all is acknowledged a packet carries nothing: state at rest costs its header.
    let rest = at_rest(&mut table);
    assert!(rest <= EMPTY_PACKET, "{rest:.1} bytes a packet at rest");
    let operations: usize = table.records.values().map(Vec::len).sum();
    assert!(operations > 1000, "only {operations} operations");
    // Readings (D632: not gates until their own faults are planted), against netcode.md's
    // first targets of 8 kB/s down and 4 kB/s up.
    eprintln!(
        "soak: {:.0} B/s down and {:.0} B/s up a player (payload, 4 players, 50 creatures); {:.1} µs of server a client a tick; {operations} operations; {:.1} s",
        (traffic.down_state + traffic.down_ordered) as f64 / seconds / players,
        (traffic.up_state + traffic.up_ordered) as f64 / seconds / players,
        table.server_s / f64::from(SOAK_TICKS) / players * 1.0e6,
        began.elapsed().as_secs_f64(),
    );
}

/// The planted fault (D632): a client's acknowledgements never arrive. The server can never
/// know what that client holds, so it sends again for ever, and the rest check must fail.
#[test]
fn acknowledgements_cut_off_never_settle() {
    let mut table = soaked(1024);
    table.seats[0].tap.mute_state = true;
    table.run(64);
    table.settle();
    let rest = at_rest(&mut table);
    assert!(
        rest > EMPTY_PACKET + 1.0,
        "{rest:.1} bytes a packet at rest with no acknowledgements"
    );
}
