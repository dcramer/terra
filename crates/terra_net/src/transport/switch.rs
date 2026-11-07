//! One carrier shared by many servers (D640): the instance host's switch. Each realm's
//! server holds a [`Branch`], its own end of the carrier; a client belongs to one branch at
//! a time. Between ticks, and never during one, the switch hands each branch what arrived
//! for its clients ([`Switch::take_in`]) and sends what each gathered ([`Switch::send_out`]),
//! so the realms tick at once, each touching only its own branch. As a gateway routes a
//! connection to one zone's server in a world of many.

use super::{FromClients, Lane, Peer, ServerTransport};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

enum Sent {
    Packet(Peer, Lane, Vec<u8>),
    Drop(Peer),
}

#[derive(Default)]
struct Buffer {
    arrived: Vec<FromClients>,
    sent: Vec<Sent>,
}

fn lock(buffer: &Mutex<Buffer>) -> MutexGuard<'_, Buffer> {
    // A panic while holding it leaves the queues whole: nothing is half-written.
    buffer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One server's end of a shared carrier: what it sends waits for the switch.
pub struct Branch {
    buffer: Arc<Mutex<Buffer>>,
}

impl ServerTransport for Branch {
    fn poll(&mut self, into: &mut Vec<FromClients>) {
        into.append(&mut lock(&self.buffer).arrived);
    }

    fn send(&mut self, to: Peer, lane: Lane, bytes: &[u8]) {
        lock(&self.buffer)
            .sent
            .push(Sent::Packet(to, lane, bytes.to_vec()));
    }

    fn drop_peer(&mut self, peer: Peer) {
        lock(&self.buffer).sent.push(Sent::Drop(peer));
    }
}

/// A carrier and the branches it is shared among, by number in the order they were made.
pub struct Switch<T> {
    carrier: T,
    branches: Vec<Arc<Mutex<Buffer>>>,
    /// Which branch each client belongs to.
    of: HashMap<Peer, usize>,
    /// The branch a new client is given to.
    home: usize,
    polled: Vec<FromClients>,
}

impl<T: ServerTransport> Switch<T> {
    pub fn new(carrier: T) -> Switch<T> {
        Switch {
            carrier,
            branches: Vec::new(),
            of: HashMap::new(),
            home: 0,
            polled: Vec::new(),
        }
    }

    /// A new branch and its number. The first made is home until another is named.
    pub fn branch(&mut self) -> (usize, Branch) {
        let buffer = Arc::new(Mutex::new(Buffer::default()));
        self.branches.push(buffer.clone());
        (self.branches.len() - 1, Branch { buffer })
    }

    /// Names the branch new clients are given to.
    pub fn home(&mut self, branch: usize) {
        assert!(branch < self.branches.len(), "no branch {branch}");
        self.home = branch;
    }

    /// The branch a client belongs to.
    pub fn branch_of(&self, peer: Peer) -> Option<usize> {
        self.of.get(&peer).copied()
    }

    /// Before the tick: time passes for the carrier, and what arrived is handed to the branch
    /// of the client it came from. A new client is home's.
    pub fn take_in(&mut self, elapsed: Duration) {
        self.carrier.advance(elapsed);
        self.carrier.poll(&mut self.polled);
        for arrived in self.polled.drain(..) {
            let branch = match &arrived {
                FromClients::Joined(peer) => {
                    self.of.insert(*peer, self.home);
                    Some(self.home)
                }
                FromClients::Left(peer) => self.of.remove(peer),
                FromClients::Packet(peer, ..) => self.of.get(peer).copied(),
            };
            if let Some(branch) = branch {
                lock(&self.branches[branch]).arrived.push(arrived);
            }
        }
    }

    /// After the tick: what each branch gathered is sent. A branch speaks only to its own
    /// clients: a word for another's (one handed on since, say) is not carried.
    pub fn send_out(&mut self) {
        for (branch, buffer) in self.branches.iter().enumerate() {
            let sent = std::mem::take(&mut lock(buffer).sent);
            for sent in sent {
                let (Sent::Packet(peer, ..) | Sent::Drop(peer)) = &sent;
                if self.of.get(peer) != Some(&branch) {
                    continue;
                }
                match sent {
                    Sent::Packet(peer, lane, bytes) => self.carrier.send(peer, lane, &bytes),
                    Sent::Drop(peer) => self.carrier.drop_peer(peer),
                }
            }
        }
        self.carrier.flush();
    }

    /// Hands a client to another branch (a crossing): to the branch it leaves it has left, to
    /// the one it comes to it has joined, and what it sends from now on is the new one's.
    pub fn hand(&mut self, peer: Peer, to: usize) {
        assert!(to < self.branches.len(), "no branch {to}");
        let Some(from) = self.of.insert(peer, to) else {
            // Gone already: nobody to hand on.
            self.of.remove(&peer);
            return;
        };
        if from == to {
            return;
        }
        lock(&self.branches[from])
            .arrived
            .push(FromClients::Left(peer));
        lock(&self.branches[to])
            .arrived
            .push(FromClients::Joined(peer));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::loopback::{Conditions, Network};
    use crate::transport::{ClientTransport, FromServer};

    fn polled(end: &mut impl ServerTransport) -> Vec<FromClients> {
        let mut into = Vec::new();
        end.poll(&mut into);
        into
    }

    fn heard(client: &mut impl ClientTransport) -> Vec<FromServer> {
        let mut into = Vec::new();
        client.poll(&mut into);
        into
    }

    #[test]
    fn a_client_is_one_branchs_until_handed_on_and_only_its_branch_speaks_to_it() {
        let network = Network::new();
        let mut switch = Switch::new(network.server());
        let ((first, mut one), (second, mut two)) = (switch.branch(), switch.branch());
        let mut client = network.connect(Conditions::PERFECT);
        client.send(Lane::Ordered, b"hello");
        switch.take_in(Duration::ZERO);
        let arrived = polled(&mut one);
        let FromClients::Joined(peer) = arrived[0] else {
            panic!("home's first word of a new client is not that it joined: {arrived:?}");
        };
        assert_eq!(
            arrived[1],
            FromClients::Packet(peer, Lane::Ordered, b"hello".to_vec())
        );
        assert!(polled(&mut two).is_empty(), "another branch heard of it");
        assert_eq!(switch.branch_of(peer), Some(first));

        // Only its branch speaks to it: the other's word is not carried. Without the
        // switch's check on whose client it is, this fails (the client hears "not yours").
        one.send(peer, Lane::Ordered, b"yours");
        two.send(peer, Lane::Ordered, b"not yours");
        switch.send_out();
        assert_eq!(
            heard(&mut client),
            vec![FromServer::Packet(Lane::Ordered, b"yours".to_vec())]
        );

        // Handed on: the old branch is told it left, the new that it joined, and what it
        // sends next is the new one's; the old one's parting word is not carried.
        switch.hand(peer, second);
        client.send(Lane::State, b"onward");
        switch.take_in(Duration::ZERO);
        assert_eq!(polled(&mut one), vec![FromClients::Left(peer)]);
        assert_eq!(
            polled(&mut two),
            vec![
                FromClients::Joined(peer),
                FromClients::Packet(peer, Lane::State, b"onward".to_vec())
            ]
        );
        one.send(peer, Lane::Ordered, b"stale");
        two.send(peer, Lane::Ordered, b"welcome");
        switch.send_out();
        assert_eq!(
            heard(&mut client),
            vec![FromServer::Packet(Lane::Ordered, b"welcome".to_vec())]
        );

        // Gone: its branch is told, and it is nobody's.
        client.close();
        switch.take_in(Duration::ZERO);
        assert_eq!(polled(&mut two), vec![FromClients::Left(peer)]);
        assert_eq!(switch.branch_of(peer), None);
    }

    #[test]
    fn a_new_client_is_given_to_the_branch_named_home() {
        let network = Network::new();
        let mut switch = Switch::new(network.server());
        let ((_, mut one), (second, mut two)) = (switch.branch(), switch.branch());
        switch.home(second);
        let _client = network.connect(Conditions::PERFECT);
        switch.take_in(Duration::ZERO);
        assert!(polled(&mut one).is_empty());
        assert!(matches!(polled(&mut two)[..], [FromClients::Joined(_)]));
    }
}
