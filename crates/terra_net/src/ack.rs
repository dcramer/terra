//! Acknowledgements for the unreliable lane (D636), as Fiedler's "Reliability and Congestion
//! Avoidance over UDP": every packet carries its own number, the newest number its sender has
//! received, and a bit for each of the 32 before that. A packet is thus acknowledged 33 times
//! over, so losing acknowledgements costs nothing, and a sender learns which of its packets
//! arrived without a reliable lane.

use crate::wire::{Malformed, Reader, Wire, Writer};
use std::collections::VecDeque;

/// What every packet on the unreliable lane begins with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// This packet's number.
    pub seq: u16,
    /// The newest number its sender has received, if any.
    pub ack: Option<u16>,
    /// Bit `i`: it has received `ack - 1 - i` too.
    pub bits: u32,
}

impl Wire for Header {
    fn write(&self, to: &mut Writer) {
        self.seq.write(to);
        to.bool(self.ack.is_some());
        self.ack.unwrap_or(0).write(to);
        self.bits.write(to);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        let seq = u16::read(from)?;
        let any = from.bool()?;
        let ack = u16::read(from)?;
        Ok(Header {
            seq,
            ack: any.then_some(ack),
            bits: u32::read(from)?,
        })
    }
}

/// How many sent packets are watched for their acknowledgement; one older is taken as lost.
pub const WATCHED: usize = 64;

/// One end's numbering: what it has sent that is not yet acknowledged, and what it has
/// received.
#[derive(Default)]
pub struct Acks {
    next: u16,
    /// The newest number received and the 32 before it, as [`Header::bits`].
    latest: Option<u16>,
    seen: u32,
    flying: VecDeque<u16>,
}

impl Acks {
    /// The header of the next packet sent.
    pub fn outgoing(&mut self) -> Header {
        let seq = self.next;
        self.next = self.next.wrapping_add(1);
        if self.flying.len() == WATCHED {
            self.flying.pop_front();
        }
        self.flying.push_back(seq);
        Header {
            seq,
            ack: self.latest,
            bits: self.seen,
        }
    }

    /// Takes a packet's header: the numbers of ours it newly acknowledges are pushed to
    /// `acked`, and whether the packet itself is new (not a copy of one already taken, nor
    /// too old to tell) is returned. A stale packet's acknowledgements still count.
    pub fn incoming(&mut self, header: Header, acked: &mut Vec<u16>) -> bool {
        if let Some(ack) = header.ack {
            self.flying.retain(|&seq| {
                let back = ack.wrapping_sub(seq);
                let hit = back == 0 || (back <= 32 && header.bits & (1 << (back - 1)) != 0);
                if hit {
                    acked.push(seq);
                }
                !hit
            });
        }
        let Some(latest) = self.latest else {
            self.latest = Some(header.seq);
            return true;
        };
        let ahead = i32::from(header.seq.wrapping_sub(latest) as i16);
        if ahead > 0 {
            // The old newest becomes bit `ahead - 1`, the rest move up behind it.
            self.seen = match ahead {
                1..=31 => (self.seen << ahead) | (1 << (ahead - 1)),
                32 => 1 << 31,
                _ => 0,
            };
            self.latest = Some(header.seq);
            return true;
        }
        let back = ahead.unsigned_abs();
        if back == 0 || back > 32 {
            return false;
        }
        let bit = 1 << (back - 1);
        let new = self.seen & bit == 0;
        self.seen |= bit;
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two ends, the packets from one to the other delivered by a rule: every packet that
    /// arrived is acknowledged once, none that was lost is, and a copy is told from a first.
    #[test]
    fn what_arrived_is_acknowledged_once_and_what_was_lost_never() {
        let (mut here, mut there) = (Acks::default(), Acks::default());
        let (mut acked, mut delivered) = (Vec::new(), Vec::new());
        // Across the number's wrap.
        here.next = u16::MAX - 40;
        there.next = 7;
        for step in 0..200u32 {
            let header = here.outgoing();
            if step % 5 != 3 {
                delivered.push(header.seq);
                assert!(there.incoming(header, &mut Vec::new()));
                // A copy is refused.
                assert!(!there.incoming(header, &mut Vec::new()));
            }
            // An answer only every third packet: each still acknowledges all that arrived.
            if step % 3 == 0 {
                assert!(here.incoming(there.outgoing(), &mut acked));
            }
        }
        here.incoming(there.outgoing(), &mut acked);
        assert_eq!(acked, delivered);
    }

    #[test]
    fn a_packet_out_of_order_is_new_once() {
        let mut there = Acks::default();
        let header = |seq| Header {
            seq,
            ack: None,
            bits: 0,
        };
        let mut none = Vec::new();
        assert!(there.incoming(header(10), &mut none));
        assert!(there.incoming(header(12), &mut none));
        assert!(there.incoming(header(11), &mut none));
        assert!(!there.incoming(header(11), &mut none));
        // Too old to tell from a copy: refused.
        assert!(there.incoming(header(60), &mut none));
        assert!(!there.incoming(header(12), &mut none));
        assert_eq!(there.outgoing().bits & 1, 0);
    }
}
