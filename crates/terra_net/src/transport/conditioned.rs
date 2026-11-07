//! A client's transport made worse on purpose: whatever carries it, each way is given a
//! latency, jitter, loss and copies from a seed, as the in-memory network gives its links
//! (netem's part, for a link that is real). A soak against a running game is a client over
//! UDP through this (`tools/sandbox/net.ts --soak`).
//!
//! The unreliable lane loses, copies and reorders; the ordered lane, which the carrier under
//! it already delivers whole and in order, is only made late.

use super::loopback::{Conditions, draw};
use super::{ClientTransport, FromServer, Lane};
use std::time::Duration;

/// Something held until its time, µs, with a number that keeps equal times in order.
struct Held<T> {
    at: u64,
    order: u64,
    what: T,
}

/// A client's transport under [`Conditions`].
pub struct Conditioned<T> {
    inner: T,
    conditions: Conditions,
    now: u64,
    draws: u64,
    order: u64,
    up: Vec<Held<(Lane, Vec<u8>)>>,
    down: Vec<Held<FromServer>>,
    /// When the last ordered packet each way is let through, so the next is no sooner.
    ordered_up: u64,
    ordered_down: u64,
}

impl<T: ClientTransport> Conditioned<T> {
    pub fn new(inner: T, conditions: Conditions) -> Conditioned<T> {
        Conditioned {
            inner,
            conditions,
            now: 0,
            draws: 0,
            order: 0,
            up: Vec::new(),
            down: Vec::new(),
            ordered_up: 0,
            ordered_down: 0,
        }
    }

    /// The transport under it.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    fn draw(&mut self) -> f64 {
        draw(self.conditions.seed, &mut self.draws)
    }

    /// When something sent now arrives, by the link's latency and jitter.
    fn arrival(&mut self) -> u64 {
        let delay = self.conditions.latency_s + self.conditions.jitter_s * self.draw();
        self.now + (delay * 1.0e6) as u64
    }

    /// How many times an unreliable packet arrives: none, once or twice.
    fn copies(&mut self) -> usize {
        let lost = self.draw() < self.conditions.loss;
        let copied = self.draw() < self.conditions.copies;
        usize::from(!lost) + usize::from(!lost && copied)
    }

    fn next_order(&mut self) -> u64 {
        self.order += 1;
        self.order
    }
}

/// Takes what is due out of a list, in the order of its times.
fn due<T>(held: &mut Vec<Held<T>>, now: u64) -> Vec<T> {
    held.sort_by_key(|held| (held.at, held.order));
    let count = held.partition_point(|held| held.at <= now);
    held.drain(..count).map(|held| held.what).collect()
}

impl<T: ClientTransport> ClientTransport for Conditioned<T> {
    fn poll(&mut self, into: &mut Vec<FromServer>) {
        let mut arrived = Vec::new();
        self.inner.poll(&mut arrived);
        for arrival in arrived {
            let copies = match &arrival {
                FromServer::Packet(Lane::State, _) => self.copies(),
                _ => 1,
            };
            for _ in 0..copies {
                let mut at = self.arrival();
                if !matches!(arrival, FromServer::Packet(Lane::State, _)) {
                    at = at.max(self.ordered_down);
                    self.ordered_down = at;
                }
                let order = self.next_order();
                let what = match &arrival {
                    FromServer::Packet(lane, bytes) => FromServer::Packet(*lane, bytes.clone()),
                    FromServer::Closed => FromServer::Closed,
                };
                self.down.push(Held { at, order, what });
            }
        }
        into.extend(due(&mut self.down, self.now));
    }

    fn send(&mut self, lane: Lane, bytes: &[u8]) {
        let copies = if lane == Lane::State {
            self.copies()
        } else {
            1
        };
        for _ in 0..copies {
            let mut at = self.arrival();
            if lane == Lane::Ordered {
                at = at.max(self.ordered_up);
                self.ordered_up = at;
            }
            let order = self.next_order();
            self.up.push(Held {
                at,
                order,
                what: (lane, bytes.to_vec()),
            });
        }
    }

    fn close(&mut self) {
        self.inner.close();
    }

    fn flush(&mut self) {
        self.inner.flush();
    }

    /// Time passes: what was sent long enough ago goes on to the carrier.
    fn advance(&mut self, elapsed: Duration) {
        self.now += elapsed.as_micros() as u64;
        self.inner.advance(elapsed);
        let ready = due(&mut self.up, self.now);
        for (lane, bytes) in &ready {
            self.inner.send(*lane, bytes);
        }
        // Only when something went: a carrier frames every flush.
        if !ready.is_empty() {
            self.inner.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::loopback::Network;
    use crate::transport::{FromClients, Peer, ServerTransport};

    /// Over a perfect link made rough: the ordered lane still gives all in order, only late;
    /// the unreliable lane loses and copies; nothing arrives before the latency.
    #[test]
    fn a_perfect_link_is_made_late_lossy_and_out_of_order() {
        let network = Network::new();
        let mut server = network.server();
        let rough = Conditions {
            latency_s: 0.05,
            jitter_s: 0.03,
            loss: 0.2,
            copies: 0.1,
            seed: 5,
        };
        let mut client = Conditioned::new(network.connect(Conditions::PERFECT), rough);
        let (mut got, mut events) = (Vec::new(), Vec::new());
        let (mut ordered, mut state, mut first) = (Vec::new(), Vec::new(), None);
        for step in 0..300u32 {
            if step < 200 {
                client.send(Lane::Ordered, &[step as u8]);
                client.send(Lane::State, &[step as u8]);
            }
            client.advance(Duration::from_millis(10));
            server.poll(&mut events);
            for event in events.drain(..) {
                match event {
                    FromClients::Packet(_, Lane::Ordered, bytes) => {
                        first.get_or_insert(step);
                        ordered.push(bytes[0]);
                    }
                    FromClients::Packet(_, Lane::State, bytes) => state.push(bytes[0]),
                    _ => {}
                }
            }
            // And down: an echo of the step on the state lane.
            server.send(Peer(1), Lane::State, &[step as u8]);
            client.poll(&mut got);
        }
        assert_eq!(ordered, (0..200).map(|step| step as u8).collect::<Vec<_>>());
        assert!(
            first.is_some_and(|step| step >= 4),
            "the first arrived on step {first:?}, before 50 ms"
        );
        let mut unique = state.clone();
        unique.sort_unstable();
        unique.dedup();
        assert!(
            (130..190).contains(&unique.len()),
            "{} of 200 arrived",
            unique.len()
        );
        assert!(state.len() > unique.len(), "none was copied");
        assert!(
            state.windows(2).any(|pair| pair[0] > pair[1]),
            "none passed another"
        );
        let down = got
            .iter()
            .filter(|event| matches!(event, FromServer::Packet(Lane::State, _)))
            .count();
        assert!((200..290).contains(&down), "{down} of 300 came down");
    }
}
