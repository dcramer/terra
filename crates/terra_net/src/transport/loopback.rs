//! A transport in memory: a server's end and any number of clients' in one process, with no
//! socket. A game alone is a client of its own server over it (D51), and tests are run on it:
//! a link's latency, jitter, loss and copies are drawn from its seed on a clock the caller
//! advances, so a run with faults repeats to the byte (as netem or Clumsy would shape a real
//! link, without the waiting).

use super::{ClientTransport, FromClients, FromServer, Lane, Peer, ServerTransport};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::{Arc, Mutex, MutexGuard};
use terra_gen::hash::splitmix64;

/// What a link does to what crosses it, each way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conditions {
    /// How long a packet takes one way, s.
    pub latency_s: f64,
    /// The most a packet is held beyond that, s: unreliable packets pass each other within it.
    pub jitter_s: f64,
    /// The share of unreliable packets lost. An ordered packet "lost" arrives late instead,
    /// by a resend, holding back those behind it as a real reliable lane does.
    pub loss: f64,
    /// The share of unreliable packets that arrive twice.
    pub copies: f64,
    pub seed: u64,
}

impl Conditions {
    /// A link that delivers everything at once.
    pub const PERFECT: Conditions = Conditions {
        latency_s: 0.0,
        jitter_s: 0.0,
        loss: 0.0,
        copies: 0.0,
        seed: 0,
    };
}

/// How long a reliable lane waits before sending a lost packet again, s (renet's default).
const RESEND_S: f64 = 0.3;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum End {
    Server,
    Client(Peer),
}

enum Cargo {
    Packet(Lane, Vec<u8>),
    Joined,
    Closed,
}

/// Something on its way: when it lands (µs), a number that keeps equal times in the order
/// sent, whom it is for and from which client's link.
struct Flight {
    at: u64,
    order: u64,
    to: End,
    link: Peer,
    cargo: Cargo,
}

impl PartialEq for Flight {
    fn eq(&self, other: &Self) -> bool {
        (self.at, self.order) == (other.at, other.order)
    }
}
impl Eq for Flight {}
impl PartialOrd for Flight {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Flight {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.at, self.order).cmp(&(other.at, other.order))
    }
}

struct Link {
    peer: Peer,
    conditions: Conditions,
    /// How many draws its faults have taken.
    draws: u64,
    /// When the last ordered packet each way lands, so the next lands no sooner.
    ordered_up: u64,
    ordered_down: u64,
    open: bool,
}

/// The next draw of a link's faults, from 0 up to 1: by its seed and how many it has taken.
pub(crate) fn draw(seed: u64, draws: &mut u64) -> f64 {
    *draws += 1;
    let bits = splitmix64(seed ^ draws.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    (bits >> 11) as f64 / (1u64 << 53) as f64
}

impl Link {
    fn draw(&mut self) -> f64 {
        draw(self.conditions.seed, &mut self.draws)
    }
}

/// How many bytes crossed a network, by lane and way: what a budget is read from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Traffic {
    pub up_state: u64,
    pub up_ordered: u64,
    pub down_state: u64,
    pub down_ordered: u64,
    /// Packets sent, both lanes, each way.
    pub up_packets: u64,
    pub down_packets: u64,
}

#[derive(Default)]
struct Inner {
    now: u64,
    order: u64,
    next_peer: u64,
    links: Vec<Link>,
    flying: BinaryHeap<Reverse<Flight>>,
    traffic: Traffic,
}

impl Inner {
    fn link(&mut self, peer: Peer) -> Option<&mut Link> {
        self.links.iter_mut().find(|link| link.peer == peer)
    }

    fn launch(&mut self, at: u64, to: End, link: Peer, cargo: Cargo) {
        self.order += 1;
        self.flying.push(Reverse(Flight {
            at,
            order: self.order,
            to,
            link,
            cargo,
        }));
    }

    fn send(&mut self, peer: Peer, to: End, lane: Lane, bytes: &[u8]) {
        let now = self.now;
        let up = to == End::Server;
        let Some(link) = self.link(peer).filter(|link| link.open) else {
            return;
        };
        let micros = |seconds: f64| (seconds * 1.0e6) as u64;
        let conditions = link.conditions;
        let mut at = now + micros(conditions.latency_s + conditions.jitter_s * link.draw());
        let lost = link.draw() < conditions.loss;
        let copied = link.draw() < conditions.copies;
        let copy_at = now + micros(conditions.latency_s + conditions.jitter_s * link.draw());
        let mut copies = 1;
        match lane {
            Lane::Ordered => {
                if lost {
                    at += micros(RESEND_S);
                }
                let last = if up {
                    &mut link.ordered_up
                } else {
                    &mut link.ordered_down
                };
                at = at.max(*last);
                *last = at;
            }
            Lane::State => copies = usize::from(!lost) + usize::from(!lost && copied),
        }
        let traffic = &mut self.traffic;
        let (count, packets) = match (up, lane) {
            (true, Lane::State) => (&mut traffic.up_state, &mut traffic.up_packets),
            (true, Lane::Ordered) => (&mut traffic.up_ordered, &mut traffic.up_packets),
            (false, Lane::State) => (&mut traffic.down_state, &mut traffic.down_packets),
            (false, Lane::Ordered) => (&mut traffic.down_ordered, &mut traffic.down_packets),
        };
        *count += bytes.len() as u64;
        *packets += 1;
        for copy in 0..copies {
            let at = if copy == 0 { at } else { copy_at };
            self.launch(at, to, peer, Cargo::Packet(lane, bytes.to_vec()));
        }
    }

    /// Closes a link: its far end is told after everything already on its way there.
    fn close(&mut self, peer: Peer, tell: End) {
        let now = self.now;
        let Some(link) = self.link(peer).filter(|link| link.open) else {
            return;
        };
        link.open = false;
        let last = if tell == End::Server {
            link.ordered_up
        } else {
            link.ordered_down
        };
        let at = last.max(now + (link.conditions.latency_s * 1.0e6) as u64);
        self.launch(at, tell, peer, Cargo::Closed);
    }

    /// What has landed for an end by now, in order.
    fn landed(&mut self, end: End, mut take: impl FnMut(Peer, Cargo)) {
        let mut kept = Vec::new();
        while self.flying.peek().is_some_and(|next| next.0.at <= self.now) {
            let Some(Reverse(flight)) = self.flying.pop() else {
                break;
            };
            if flight.to == end {
                take(flight.link, flight.cargo);
            } else {
                kept.push(Reverse(flight));
            }
        }
        self.flying.extend(kept);
    }
}

/// A network in memory. Clones are the same network.
#[derive(Clone, Default)]
pub struct Network {
    inner: Arc<Mutex<Inner>>,
}

impl Network {
    pub fn new() -> Network {
        Network::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A panic while holding it leaves the queues whole: nothing is half-written.
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The server's end.
    pub fn server(&self) -> LoopbackServer {
        LoopbackServer {
            network: self.clone(),
        }
    }

    /// A new client's end, over a link of these conditions; the server is told it joined.
    pub fn connect(&self, conditions: Conditions) -> LoopbackClient {
        let mut inner = self.lock();
        inner.next_peer += 1;
        let peer = Peer(inner.next_peer);
        inner.links.push(Link {
            peer,
            conditions,
            draws: 0,
            ordered_up: 0,
            ordered_down: 0,
            open: true,
        });
        let at = inner.now + (conditions.latency_s * 1.0e6) as u64;
        inner.link(peer).expect("just made").ordered_up = at;
        inner.launch(at, End::Server, peer, Cargo::Joined);
        LoopbackClient {
            network: self.clone(),
            peer,
        }
    }

    /// Moves the network's clock on, s.
    pub fn advance(&self, seconds: f64) {
        self.lock().now += (seconds * 1.0e6) as u64;
    }

    /// The bytes that have crossed it so far.
    pub fn traffic(&self) -> Traffic {
        self.lock().traffic
    }
}

/// The server's end of a [`Network`].
pub struct LoopbackServer {
    network: Network,
}

impl ServerTransport for LoopbackServer {
    fn poll(&mut self, into: &mut Vec<FromClients>) {
        self.network.lock().landed(End::Server, |peer, cargo| {
            into.push(match cargo {
                Cargo::Packet(lane, bytes) => FromClients::Packet(peer, lane, bytes),
                Cargo::Joined => FromClients::Joined(peer),
                Cargo::Closed => FromClients::Left(peer),
            });
        });
    }

    fn send(&mut self, to: Peer, lane: Lane, bytes: &[u8]) {
        self.network.lock().send(to, End::Client(to), lane, bytes);
    }

    fn drop_peer(&mut self, peer: Peer) {
        self.network.lock().close(peer, End::Client(peer));
    }
}

/// A client's end of a [`Network`].
pub struct LoopbackClient {
    network: Network,
    peer: Peer,
}

impl ClientTransport for LoopbackClient {
    fn poll(&mut self, into: &mut Vec<FromServer>) {
        self.network
            .lock()
            .landed(End::Client(self.peer), |_, cargo| match cargo {
                Cargo::Packet(lane, bytes) => into.push(FromServer::Packet(lane, bytes)),
                Cargo::Closed => into.push(FromServer::Closed),
                Cargo::Joined => {}
            });
    }

    fn send(&mut self, lane: Lane, bytes: &[u8]) {
        self.network
            .lock()
            .send(self.peer, End::Server, lane, bytes);
    }

    fn close(&mut self) {
        self.network.lock().close(self.peer, End::Server);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rough(seed: u64) -> Conditions {
        Conditions {
            latency_s: 0.05,
            jitter_s: 0.03,
            loss: 0.2,
            copies: 0.1,
            seed,
        }
    }

    /// What a client receives of 200 packets a lane over a rough link, a step a packet.
    fn run(seed: u64) -> (Vec<u8>, Vec<u8>) {
        let network = Network::new();
        let (mut server, mut client) = (network.server(), network.connect(rough(seed)));
        let (mut joined, mut got) = (Vec::new(), Vec::new());
        let (mut ordered, mut state) = (Vec::new(), Vec::new());
        for step in 0..260u32 {
            network.advance(0.01);
            server.poll(&mut joined);
            if step < 200 {
                server.send(Peer(1), Lane::Ordered, &[step as u8]);
                server.send(Peer(1), Lane::State, &[step as u8]);
            }
            client.poll(&mut got);
            for event in got.drain(..) {
                match event {
                    FromServer::Packet(Lane::Ordered, bytes) => ordered.push(bytes[0]),
                    FromServer::Packet(Lane::State, bytes) => state.push(bytes[0]),
                    FromServer::Closed => panic!("nobody closed it"),
                }
            }
        }
        assert_eq!(joined, [FromClients::Joined(Peer(1))]);
        (ordered, state)
    }

    #[test]
    fn the_ordered_lane_delivers_all_in_order_and_the_state_lane_does_not() {
        let (ordered, state) = run(7);
        assert_eq!(ordered, (0..200).map(|step| step as u8).collect::<Vec<_>>());
        // A fifth lost, a tenth of the rest copied: well short of all, and out of order.
        let mut unique = state.clone();
        unique.sort_unstable();
        unique.dedup();
        assert!(
            (130..190).contains(&unique.len()),
            "{} arrived",
            unique.len()
        );
        assert!(state.len() > unique.len(), "no copy arrived");
        assert!(
            state.windows(2).any(|pair| pair[0] > pair[1]),
            "none passed another"
        );
    }

    #[test]
    fn a_seed_repeats_and_another_differs() {
        assert_eq!(run(7), run(7));
        assert_ne!(run(7).1, run(8).1);
    }

    #[test]
    fn closing_tells_the_far_end_after_what_was_sent() {
        let network = Network::new();
        let (mut server, mut client) = (network.server(), network.connect(rough(1)));
        client.send(Lane::Ordered, b"bye");
        client.close();
        client.send(Lane::Ordered, b"too late");
        network.advance(1.0);
        let mut events = Vec::new();
        server.poll(&mut events);
        assert_eq!(
            events,
            [
                FromClients::Joined(Peer(1)),
                FromClients::Packet(Peer(1), Lane::Ordered, b"bye".to_vec()),
                FromClients::Left(Peer(1)),
            ]
        );
        // The server's drop reaches a client the same way.
        let mut other = network.connect(Conditions::PERFECT);
        server.poll(&mut events);
        server.drop_peer(Peer(2));
        let mut got = Vec::new();
        other.poll(&mut got);
        assert_eq!(got, [FromServer::Closed]);
    }
}
