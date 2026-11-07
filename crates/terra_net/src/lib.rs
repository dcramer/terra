//! How a server and its clients talk (docs/netcode.md; D34, D651, D652, D636). Plain Rust that
//! knows no game type: the rules crates say what their state is on the wire ([`wire::Wire`]),
//! and this crate carries it.
//!
//! - [`wire`]: the bit packer, with a reader that trusts nothing.
//! - [`transport`]: the seam to whatever carries bytes, and the in-memory carrier.
//! - [`ack`]: which unreliable packets arrived.
//! - [`interest`]: what is near a place.

pub mod ack;
pub mod interest;
pub mod transport;
pub mod wire;
