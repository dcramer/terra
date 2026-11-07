//! The transport seam (D34): all `terra_net` asks of whatever carries its bytes. Two lanes, a
//! reliable ordered one and an unreliable one, and who joined and left. [`loopback`] carries
//! them in memory, for a game alone with its own server and for tests; UDP (`renet`) and
//! Steam's relay are other carriers of the same seam.

pub mod loopback;
#[cfg(feature = "udp")]
pub mod udp;

/// How a packet is carried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Arrives, once, in the order sent: operations, connecting, what must not be missed.
    Ordered,
    /// May be lost, copied or arrive out of order, and is never resent: state and intent,
    /// where only the newest matters.
    State,
}

/// A client as its server's transport names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Peer(pub u64);

/// What a server's transport tells it.
#[derive(Debug, PartialEq, Eq)]
pub enum FromClients {
    Joined(Peer),
    Left(Peer),
    Packet(Peer, Lane, Vec<u8>),
}

/// What a client's transport tells it.
#[derive(Debug, PartialEq, Eq)]
pub enum FromServer {
    /// The connection is gone, by either end's choice or by silence.
    Closed,
    Packet(Lane, Vec<u8>),
}

/// A server's end of a transport.
pub trait ServerTransport {
    /// What has arrived since the last call, in order.
    fn poll(&mut self, into: &mut Vec<FromClients>);
    fn send(&mut self, to: Peer, lane: Lane, bytes: &[u8]);
    /// Ends a client's connection, after what was sent to it on the ordered lane.
    fn drop_peer(&mut self, peer: Peer);
    /// Puts what was sent on the wire, where a carrier gathers a tick's sends into packets.
    fn flush(&mut self) {}
}

impl<T: ServerTransport + ?Sized> ServerTransport for Box<T> {
    fn poll(&mut self, into: &mut Vec<FromClients>) {
        (**self).poll(into);
    }
    fn send(&mut self, to: Peer, lane: Lane, bytes: &[u8]) {
        (**self).send(to, lane, bytes);
    }
    fn drop_peer(&mut self, peer: Peer) {
        (**self).drop_peer(peer);
    }
    fn flush(&mut self) {
        (**self).flush();
    }
}

/// A client's end of a transport.
pub trait ClientTransport {
    fn poll(&mut self, into: &mut Vec<FromServer>);
    fn send(&mut self, lane: Lane, bytes: &[u8]);
    fn close(&mut self);
    /// As [`ServerTransport::flush`].
    fn flush(&mut self) {}
}

impl<T: ClientTransport + ?Sized> ClientTransport for Box<T> {
    fn poll(&mut self, into: &mut Vec<FromServer>) {
        (**self).poll(into);
    }
    fn send(&mut self, lane: Lane, bytes: &[u8]) {
        (**self).send(lane, bytes);
    }
    fn close(&mut self) {
        (**self).close();
    }
    fn flush(&mut self) {
        (**self).flush();
    }
}
