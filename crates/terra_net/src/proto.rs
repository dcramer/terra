//! What crosses the lanes (D652): the names a server gives things, the messages of the ordered
//! lane, and the shape of a state packet each way.
//!
//! A state packet down: [`Header`](crate::ack::Header), the tick it is the state after, the
//! newest of the client's intents the server holds and how many ticks early it came, then
//! items to the packet's end. An item is an entity gone, or some of its parts' bytes, with its
//! kind while the client may not know it yet. A state packet up: the header, the newest
//! intent's tick and the intents for the ticks up to it not yet known to have arrived, one
//! equal to the one before it as a single bit.

use crate::interest::Cell;
use crate::wire::{Malformed, Reader, Wire, Writer};

/// The most parts an entity is made of.
pub const MOST_PARTS: usize = 8;
/// The most intents a state packet repeats (D651): 125 ms of ticks.
pub const MOST_INTENTS: usize = 8;
/// The longest name a player gives, bytes.
pub const MOST_NAME: usize = 32;

/// A thing the server replicates, by the number the server gave it. Never given twice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub u32);

/// What kind of thing an entity is: the game's data says what each number means (D136).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Kind(pub u16);

/// A room: a realm's instance on this server (instances.md). A connection is in one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoomId(pub u16);

/// A cube of a room whose operations are one numbered record.
pub type Region = Cell;

/// Why a connection was refused or ended, each with a plain message for the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The game's version is not the server's.
    Version,
    /// The world is full.
    Full,
    /// The server will not have this player.
    Refused,
    /// The host left; the world was saved.
    HostLeft,
    /// What the client sent could not be read.
    Malformed,
    /// The connection was lost.
    Lost,
}

impl Reason {
    const ALL: [Reason; 6] = [
        Reason::Version,
        Reason::Full,
        Reason::Refused,
        Reason::HostLeft,
        Reason::Malformed,
        Reason::Lost,
    ];

    /// What a player is told.
    pub fn message(self) -> &'static str {
        match self {
            Reason::Version => "This world runs another version of the game.",
            Reason::Full => "This world is full.",
            Reason::Refused => "This world's host has not let you in.",
            Reason::HostLeft => "The host left. The world was saved.",
            Reason::Malformed => "The connection was closed: the server could not read this game.",
            Reason::Lost => "The connection was lost.",
        }
    }
}

impl Wire for Reason {
    fn write(&self, to: &mut Writer) {
        to.ranged(*self as u32, Reason::ALL.len() as u32 - 1);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        Ok(Reason::ALL[from.ranged(Reason::ALL.len() as u32 - 1)? as usize])
    }
}

/// A client's messages on the ordered lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Up {
    /// The first: the protocol's version, who this is (a public key, D592) and their name.
    Hello {
        version: u64,
        identity: [u8; 32],
        name: String,
    },
    /// An operation asked for: the server checks it and, if it stands, orders it.
    Ask(Vec<u8>),
}

impl Wire for Up {
    fn write(&self, to: &mut Writer) {
        match self {
            Up::Hello {
                version,
                identity,
                name,
            } => {
                to.ranged(0, 1);
                version.write(to);
                identity.write(to);
                name.write(to);
            }
            Up::Ask(bytes) => {
                to.ranged(1, 1);
                to.run(bytes);
            }
        }
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        Ok(match from.ranged(1)? {
            0 => {
                let (version, identity) = (u64::read(from)?, <[u8; 32]>::read(from)?);
                let name = String::read(from)?;
                if name.len() > MOST_NAME {
                    return Err(Malformed::Long);
                }
                Up::Hello {
                    version,
                    identity,
                    name,
                }
            }
            _ => Up::Ask(from.run()?.to_vec()),
        })
    }
}

/// A server's messages on the ordered lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Down {
    /// You are in: the tick, your room, the entity that is yours, and what the game says of
    /// the room (its realm and seed: all a client needs to make the land).
    Welcome {
        tick: u32,
        room: RoomId,
        you: EntityId,
        realm: Vec<u8>,
    },
    Refuse(Reason),
    /// The next operation of a region's record.
    Op {
        region: Region,
        number: u32,
        bytes: Vec<u8>,
    },
    /// The records of the regions round you are whole as of now.
    Settled,
    /// The connection ends, and why.
    Closing(Reason),
}

impl Wire for Down {
    fn write(&self, to: &mut Writer) {
        match self {
            Down::Welcome {
                tick,
                room,
                you,
                realm,
            } => {
                to.ranged(0, 4);
                tick.write(to);
                room.0.write(to);
                you.0.write(to);
                to.run(realm);
            }
            Down::Refuse(reason) => {
                to.ranged(1, 4);
                reason.write(to);
            }
            Down::Op {
                region,
                number,
                bytes,
            } => {
                to.ranged(2, 4);
                region.write(to);
                to.varint(u64::from(*number));
                to.run(bytes);
            }
            Down::Settled => to.ranged(3, 4),
            Down::Closing(reason) => {
                to.ranged(4, 4);
                reason.write(to);
            }
        }
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        Ok(match from.ranged(4)? {
            0 => Down::Welcome {
                tick: u32::read(from)?,
                room: RoomId(u16::read(from)?),
                you: EntityId(u32::read(from)?),
                realm: from.run()?.to_vec(),
            },
            1 => Down::Refuse(Reason::read(from)?),
            2 => Down::Op {
                region: Region::read(from)?,
                number: u32::try_from(from.varint()?).map_err(|_| Malformed::Range)?,
                bytes: from.run()?.to_vec(),
            },
            3 => Down::Settled,
            _ => Down::Closing(Reason::read(from)?),
        })
    }
}

/// What a state packet down says before its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    /// The tick this is the state after.
    pub tick: u32,
    /// The newest of this client's intents the server holds, when within 126 ticks of the
    /// tick.
    pub heard: Option<u32>,
    /// How many ticks before it was needed that intent came (negative: late), for the
    /// client's clock (D651).
    pub margin: i8,
}

impl Wire for Stamp {
    fn write(&self, to: &mut Writer) {
        // The newest intent held is the tick and the margin; a margin at its bounds says
        // only "far", which is all a clock that far out needs.
        self.tick.write(to);
        to.bool(self.heard.is_some());
        (self.margin as u8).write(to);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        let tick = u32::read(from)?;
        let any = from.bool()?;
        let margin = u8::read(from)? as i8;
        let exact = any && margin > i8::MIN && margin < i8::MAX;
        Ok(Stamp {
            tick,
            heard: exact.then(|| tick.wrapping_add_signed(i32::from(margin))),
            margin,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_reads_back() {
        let ups = [
            Up::Hello {
                version: 0xFEED_F00D_1234_5678,
                identity: [9; 32],
                name: "Ása".to_string(),
            },
            Up::Ask(vec![1, 2, 3]),
        ];
        for up in ups {
            assert_eq!(Up::decoded(&up.encoded()), Ok(up));
        }
        let downs = [
            Down::Welcome {
                tick: 77,
                room: RoomId(2),
                you: EntityId(40),
                realm: b"seed".to_vec(),
            },
            Down::Refuse(Reason::Full),
            Down::Op {
                region: [-1, 0, 3],
                number: 300,
                bytes: vec![7; 20],
            },
            Down::Settled,
            Down::Closing(Reason::HostLeft),
        ];
        for down in downs {
            assert_eq!(Down::decoded(&down.encoded()), Ok(down));
        }
        let stamp = Stamp {
            tick: 12,
            heard: Some(9),
            margin: -3,
        };
        assert_eq!(Stamp::decoded(&stamp.encoded()), Ok(stamp));
    }

    #[test]
    fn a_name_too_long_is_refused() {
        let hello = Up::Hello {
            version: 1,
            identity: [0; 32],
            name: "x".repeat(MOST_NAME + 1),
        };
        assert_eq!(Up::decoded(&hello.encoded()), Err(Malformed::Long));
    }
}
