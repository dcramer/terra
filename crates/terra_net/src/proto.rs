//! What crosses the lanes (D652): the names a server gives things, the messages of the ordered
//! lane, and the shape of a state packet each way.
//!
//! A state packet down: [`Header`](crate::ack::Header), the tick it is the state after, the
//! newest of the client's intents the server holds and how many ticks early it came, the
//! events it has not acknowledged (each with its number and how many ticks ago it happened),
//! then items to the packet's end. An item is an entity gone, or some of its parts' bytes, with its
//! kind while the client may not know it yet. A state packet up: the header, the newest
//! intent's tick and the intents for the ticks up to it not yet known to have arrived, one
//! equal to the one before it as a single bit.

use crate::interest::Cell;
use crate::wire::{Malformed, Reader, Wire, Writer};

/// The most parts an entity is made of.
pub const MOST_PARTS: usize = 8;
/// The most intents a state packet repeats (D651): 125 ms of ticks.
pub const MOST_INTENTS: usize = 8;
/// The most events a state packet carries.
pub const MOST_EVENTS: usize = 15;
/// The longest thing a player says at once, bytes.
pub const MOST_SAY: usize = 256;
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
    /// The first, in answer to the server's challenge: the protocol's version, who this is
    /// (a public key, D592), their name, and all of it signed with the challenge.
    Hello {
        version: u64,
        identity: [u8; 32],
        name: String,
        signature: [u8; 64],
    },
    /// An operation asked for: the server checks it and, if it stands, orders it.
    Ask(Vec<u8>),
    /// Words for everyone in the world.
    Say(String),
    /// The room it was let into is made here (its land loaded): it can take a body now.
    Ready,
}

impl Wire for Up {
    fn write(&self, to: &mut Writer) {
        match self {
            Up::Hello {
                version,
                identity,
                name,
                signature,
            } => {
                to.ranged(0, 3);
                version.write(to);
                identity.write(to);
                name.write(to);
                signature.write(to);
            }
            Up::Ask(bytes) => {
                to.ranged(1, 3);
                to.run(bytes);
            }
            Up::Say(text) => {
                to.ranged(2, 3);
                text.write(to);
            }
            Up::Ready => to.ranged(3, 3),
        }
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        Ok(match from.ranged(3)? {
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
                    signature: <[u8; 64]>::read(from)?,
                }
            }
            1 => Up::Ask(from.run()?.to_vec()),
            2 => {
                let text = String::read(from)?;
                if text.len() > MOST_SAY {
                    return Err(Malformed::Long);
                }
                Up::Say(text)
            }
            _ => Up::Ready,
        })
    }
}

/// A server's messages on the ordered lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Down {
    /// You are in: the tick, your room, the entity that is yours, and what the game says of
    /// the room (its realm and seed: all a client needs to make the land). With no entity
    /// yet it is the greeting: make the room and say [`Up::Ready`], and the welcome with
    /// your entity follows.
    Welcome {
        tick: u32,
        room: RoomId,
        you: Option<EntityId>,
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
    /// The first thing a server says: the number a hello must be signed with
    /// ([`crate::identity`]).
    Challenge([u8; 32]),
    /// What a player said, to everyone.
    Said {
        name: String,
        text: String,
    },
    /// A player is in the world, or has left it: the list of who is here, kept by its
    /// changes.
    Roster {
        name: String,
        here: bool,
    },
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
                to.ranged(0, 7);
                tick.write(to);
                room.0.write(to);
                to.bool(you.is_some());
                if let Some(you) = you {
                    you.0.write(to);
                }
                to.run(realm);
            }
            Down::Refuse(reason) => {
                to.ranged(1, 7);
                reason.write(to);
            }
            Down::Op {
                region,
                number,
                bytes,
            } => {
                to.ranged(2, 7);
                region.write(to);
                to.varint(u64::from(*number));
                to.run(bytes);
            }
            Down::Settled => to.ranged(3, 7),
            Down::Closing(reason) => {
                to.ranged(4, 7);
                reason.write(to);
            }
            Down::Challenge(challenge) => {
                to.ranged(5, 7);
                challenge.write(to);
            }
            Down::Said { name, text } => {
                to.ranged(6, 7);
                name.write(to);
                text.write(to);
            }
            Down::Roster { name, here } => {
                to.ranged(7, 7);
                name.write(to);
                to.bool(*here);
            }
        }
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        Ok(match from.ranged(7)? {
            0 => Down::Welcome {
                tick: u32::read(from)?,
                room: RoomId(u16::read(from)?),
                you: if from.bool()? {
                    Some(EntityId(u32::read(from)?))
                } else {
                    None
                },
                realm: from.run()?.to_vec(),
            },
            1 => Down::Refuse(Reason::read(from)?),
            2 => Down::Op {
                region: Region::read(from)?,
                number: u32::try_from(from.varint()?).map_err(|_| Malformed::Range)?,
                bytes: from.run()?.to_vec(),
            },
            3 => Down::Settled,
            4 => Down::Closing(Reason::read(from)?),
            5 => Down::Challenge(<[u8; 32]>::read(from)?),
            6 => Down::Said {
                name: String::read(from)?,
                text: String::read(from)?,
            },
            _ => Down::Roster {
                name: String::read(from)?,
                here: from.bool()?,
            },
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
                signature: [5; 64],
            },
            Up::Ask(vec![1, 2, 3]),
            Up::Say("hæ".to_string()),
        ];
        for up in ups {
            assert_eq!(Up::decoded(&up.encoded()), Ok(up));
        }
        let downs = [
            Down::Welcome {
                tick: 77,
                room: RoomId(2),
                you: Some(EntityId(40)),
                realm: b"seed".to_vec(),
            },
            Down::Welcome {
                tick: 78,
                room: RoomId(2),
                you: None,
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
            Down::Challenge([3; 32]),
            Down::Said {
                name: "Ása".to_string(),
                text: "hæ".to_string(),
            },
            Down::Roster {
                name: "Ása".to_string(),
                here: false,
            },
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
            signature: [0; 64],
        };
        assert_eq!(Up::decoded(&hello.encoded()), Err(Malformed::Long));
    }
}
