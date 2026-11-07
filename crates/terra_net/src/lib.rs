//! How a server and its clients talk (docs/netcode.md; D34, D651, D652, D636). Plain Rust that
//! knows no game type: the rules crates say what their state is on the wire ([`wire::Wire`]),
//! and this crate carries it.
//!
//! - [`wire`]: the bit packer, with a reader that trusts nothing.
//! - [`transport`]: the seam to whatever carries bytes, and the in-memory carrier.
//! - [`ack`]: which unreliable packets arrived.
//! - [`interest`]: what is near a place.
//! - [`proto`]: the names and messages that cross.
//! - [`identity`]: who a player is, proven by a signed challenge.
//! - [`server`], [`client`]: the two ends.
//! - [`clock`], [`predict`]: a client's tick against its server's, and prediction by replay.

pub mod ack;
pub mod client;
pub mod clock;
pub mod identity;
pub mod interest;
pub mod predict;
pub mod proto;
pub mod server;
pub mod transport;
pub mod wire;

#[cfg(test)]
mod tests;
