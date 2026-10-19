//! What the layers under building play see of a player's hammer. Building play's builder
//! (`play::tool::Builder`) holds the work in hand in its own types; the bases and the walker
//! ask only these two things of it, so they are kept here, where both can read them.

use bevy::prelude::*;
use terra_build::Kind;

/// A player's hammer as the bases and the walker see it. Building play's `Builder` requires
/// it and keeps it up to date in the same systems that change what it says.
#[derive(Component, Clone, Copy, PartialEq, Debug, Default)]
pub struct Hammer {
    /// The material every plan is laid in, if one is picked (D232): its texture sets are
    /// kept resident, so a change to it is drawn at once (D298).
    pub material: Option<Kind>,
    /// A part is in hand, so Ctrl and the wheel change the material rather than roll (D232).
    pub part_in_hand: bool,
}
