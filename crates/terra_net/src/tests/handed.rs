//! A client handed from one server of a host to another, mid-session, over a slow link
//! (D640): the crossing between realms as the wire sees it.

use super::*;
use crate::transport::switch::Switch;

/// A server of a realm: one creature in it, whose one part says which realm it is.
fn realm(epoch: u8, says: &[u8]) -> Server {
    let mut config = Config::new(VERSION);
    config.epoch = epoch;
    let parts = vec![PartSpec {
        audience: Audience::Public,
        weight: 1.0,
    }];
    let mut server = Server::new(config, parts);
    let creature = server.spawn(ROOM, CREATURE, place([2, 0]));
    server.set(creature, MOTION, says);
    server
}

/// A link that keeps a copy of the newest state packet each way, to deliver again late: the
/// state lane may deliver a copy long after.
struct Late {
    inner: LoopbackClient,
    kept: Option<Vec<u8>>,
    kept_down: Option<Vec<u8>>,
    /// What the client is given at its next poll, before what the link has.
    late: Vec<FromServer>,
}

impl ClientTransport for Late {
    fn poll(&mut self, into: &mut Vec<FromServer>) {
        into.append(&mut self.late);
        let from = into.len();
        self.inner.poll(into);
        for arrived in &into[from..] {
            if let FromServer::Packet(Lane::State, bytes) = arrived {
                self.kept_down = Some(bytes.clone());
            }
        }
    }
    fn send(&mut self, lane: Lane, bytes: &[u8]) {
        if lane == Lane::State {
            self.kept = Some(bytes.to_vec());
        }
        self.inner.send(lane, bytes);
    }
    fn close(&mut self) {
        self.inner.close();
    }
}

#[test]
fn a_client_handed_on_is_let_into_the_next_server_and_believes_nothing_of_the_last() {
    let network = Network::new();
    let mut switch = Switch::new(network.server());
    let mut servers = [realm(0, b"first"), realm(1, b"second")];
    let mut wires = [switch.branch().1, switch.branch().1];
    // 100 ms there and back: a second's worth of words is always on its way.
    let mut conditions = Conditions::PERFECT;
    conditions.latency_s = 0.05;
    let mut wire = Late {
        inner: network.connect(conditions),
        kept: None,
        kept_down: None,
        late: Vec::new(),
    };
    // The first realm has run for a while: its ticks and its packets' numbers are far past
    // those of the second, so its word believed late would outlast the second's.
    for _ in 0..500 {
        servers[0].step(&mut wires[0]);
    }
    let (mut stale, mut stale_down): (Option<Vec<u8>>, Option<Vec<u8>>) = (None, None);
    let mut client = Client::new();
    client.hello(
        &mut wire,
        VERSION,
        &Identity::from_secret([9; 32]),
        "walker",
    );

    let (mut events, mut heard) = (Vec::new(), Vec::new());
    let (mut next, mut handed, mut welcomes, mut closed) = (None::<u32>, 0, Vec::new(), None);
    let mut peer = None;
    for step in 0..600 {
        network.advance(TICK_S);
        switch.take_in(std::time::Duration::from_secs_f64(TICK_S));
        for (server, wire) in servers.iter_mut().zip(&mut wires) {
            server.poll(wire, &mut events);
            for event in events.drain(..) {
                match event {
                    Event::Hello { peer: who, .. } => {
                        peer = Some(who);
                        server.greet(wire, who, ROOM, b"realm");
                    }
                    Event::Ready { peer } => {
                        let you = server.spawn(ROOM, PLAYER, place([0, 0]));
                        server.set(you, MOTION, b"you");
                        server.welcome(wire, peer, ROOM, you, b"realm");
                    }
                    _ => {}
                }
            }
            server.step(wire);
        }
        switch.send_out();

        client.poll(&mut wire, &mut heard);
        for heard in heard.drain(..) {
            match heard {
                Heard::Welcome { tick, you, .. } => {
                    welcomes.push((handed, you.is_some()));
                    next = Some(tick + LEAD);
                    if you.is_none() {
                        client.ready(&mut wire);
                    } else if let Some(stale) = stale.take() {
                        // In the second realm: a copy of its last state packet to the first
                        // comes only now.
                        wire.inner.send(Lane::State, &stale);
                        let down = stale_down.take().expect("the first realm told it state");
                        wire.late.push(FromServer::Packet(Lane::State, down));
                    }
                }
                Heard::Handed => {
                    handed += 1;
                    next = None;
                }
                Heard::Closed(reason) => closed = Some(reason),
                _ => {}
            }
        }
        if let Some(tick) = next.as_mut() {
            client.intent(*tick, vec![if handed == 0 { 7 } else { 1 }]);
            *tick += 1;
        }
        client.send(&mut wire);
        if step == 450 {
            // Settled in the first: a word to it is on its way as it is handed on, and so is
            // the first's state, 50 ms of it, with acknowledgements far past a new server's.
            let said = |client: &Client| -> Vec<Vec<u8>> {
                let parts = client
                    .entities()
                    .filter_map(|(_, entity)| entity.part(MOTION));
                parts.map(|part| part.newer.bytes.clone()).collect()
            };
            let mut before = said(&client);
            before.sort();
            assert_eq!(before, [b"first".to_vec(), b"you".to_vec()]);
            (stale, stale_down) = (wire.kept.clone(), wire.kept_down.clone());
            assert!(stale.is_some());
            client.say(&mut wire, "one last thing");
            client.ask(&mut wire, b"dig");
            switch.hand(peer.expect("it said hello"), 1);
        }
    }

    assert_eq!(closed, None, "handed on, its connection was ended");
    assert_eq!(handed, 1);
    // Greeted and given a body by each, in turn.
    assert_eq!(welcomes, [(0, false), (0, true), (1, false), (1, true)]);
    // It holds the second realm and nothing of the first.
    let mut holds: Vec<Vec<u8>> = client
        .entities()
        .filter_map(|(_, entity)| entity.part(MOTION))
        .map(|part| part.newer.bytes.clone())
        .collect();
    holds.sort();
    assert_eq!(holds, [b"second".to_vec(), b"you".to_vec()]);
    // And the second server hears it: its intents arrive and are held, tick by tick.
    let peer = peer.expect("it said hello");
    assert_eq!(servers[1].intent(peer), Some(&[1u8][..]));
    assert!(
        servers[1].round_trip_ticks(peer).is_some(),
        "nothing it sent was acknowledged"
    );
    assert_eq!(servers[0].clients(), 0);
}
