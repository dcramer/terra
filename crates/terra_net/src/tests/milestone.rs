//! Milestone D (docs/engine.md): two peers end in the same state, and one that joins late
//! catches up, with no window.

use super::*;

/// Two players for five seconds, a third joining then, five seconds more, and rest.
fn played(fault: impl Fn(&mut Table)) -> Table {
    let mut table = Table::new();
    table.creatures(20);
    table.sit(rough(11));
    table.sit(rough(12));
    table.run(320);
    fault(&mut table);
    table.sit(rough(13));
    table.run(320);
    table.settle();
    table
}

#[test]
fn two_peers_end_the_same_and_a_late_joiner_catches_up() {
    let table = played(|_| {});
    for seat in 0..3 {
        assert_eq!(table.mismatch(seat), None, "seat {seat}");
    }
    // The run was a real one: operations were ordered, and the late joiner was sent those
    // from before it came, and was ready within a second of saying hello.
    let before: usize = table.records.values().map(Vec::len).sum();
    assert!(before >= 10, "only {before} operations were ordered");
    let late = &table.seats[2];
    assert!(late.records.values().map(Vec::len).sum::<usize>() >= 10);
    let ready = late.ready_after.expect("the late joiner was never ready");
    assert!(ready < 64, "ready after {ready} ticks");
    // Each player's purse counted its own asks, told to it alone.
    assert!(table.purses.values().all(|&purse| purse > 0));
}

/// The planted fault (D632): one operation of a record never reaches a client. The check must
/// say so, by the region.
#[test]
fn an_operation_dropped_from_a_record_fails_the_check() {
    let table =
        played(|table| table.seats[0].tap.swallow_op = Some(table.seats[0].tap.ops_seen + 2));
    let found = table
        .mismatch(0)
        .expect("the dropped operation went unseen");
    assert!(found.contains("record"), "{found}");
    assert_eq!(table.mismatch(1), None);
}

/// The planted fault for state: a thing moved on the server after the client's last word of
/// it, with nothing sent since. The check must name it.
#[test]
fn a_thing_moved_unsent_fails_the_check() {
    let mut table = played(|_| {});
    let you = table.seats[0].you.expect("seated");
    let near = *table
        .creatures
        .iter()
        .min_by_key(|&&id| {
            let (a, b) = (table.stands[&id], table.stands[&you]);
            (a[0] - b[0]).pow(2) + (a[1] - b[1]).pow(2)
        })
        .expect("creatures");
    table.walk(near, [1, 0]);
    let found = table.mismatch(0).expect("the move went unseen");
    assert!(found.contains("stands at"), "{found}");
    // Sent, it passes again.
    table.run(64);
    assert_eq!(table.mismatch(0), None);
}

#[test]
fn a_player_who_leaves_is_gone_for_the_others() {
    let mut table = played(|_| {});
    let left = table.seats[1].you.expect("seated");
    table.seats[1].tap.close();
    table.run(64);
    assert!(
        !table.stands.contains_key(&left),
        "its body stayed in the world (D637)"
    );
    assert!(table.seats[0].client.entity(left).is_none());
    assert_eq!(table.mismatch(0), None);
    assert_eq!(table.mismatch(2), None);
}
