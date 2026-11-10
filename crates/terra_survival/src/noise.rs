//! Noises: something audible happened, and where. The shared type between whatever makes
//! a noise and whatever hears it.
//!
//! A noise says what happened (a step on wood, a blow, a tree coming down), not what it
//! sounds like: `sounds.rs` decides that, and plays it. Creatures could hear the same
//! noises later, which is why it is a fact about the world and not a request to play a
//! file.
//!
//! A module sends one when it does something its state does not show afterwards, such
//! as finishing a building job. What its state does show, such as walking, a creature
//! turning to chase, or work going on at a station, `noises.rs` hears from the outside.

use crate::crafting::Station;
use crate::inventory::Cut;
use bevy::prelude::*;
use terra_motion::struck::Struck;

/// Something heard, at a place in the world, or with none, by the player alone (menus).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Noise {
    pub cue: Cue,
    pub at: Option<Vec3>,
}

impl Noise {
    pub fn at(cue: Cue, at: Vec3) -> Self {
        Noise { cue, at: Some(at) }
    }

    /// A noise only the player hears, such as a menu opening.
    pub fn near(cue: Cue) -> Self {
        Noise { cue, at: None }
    }
}

/// What made a noise.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cue {
    /// A foot set down.
    Step(Feet),
    /// A swing through the air, and whether it is a heavy one.
    Swing {
        heavy: bool,
    },
    /// A blow landing on a creature, and one that killed it.
    Hit,
    Kill,
    /// A blow in a fight meeting something: what struck what, how it was taken and how heavy
    /// it was (a blade on a shield's wooden face, parried on its iron boss, on flesh).
    Struck(Struck),
    /// The player hurt, and the player going down.
    Hurt,
    Down,
    /// A tool striking what it cuts, and the thing coming down.
    Strike(Cut),
    Felled {
        tall: bool,
    },
    /// A hammer at work on something being built or taken apart.
    Hammer,
    /// A building job finished: a part built, an item placed, something taken apart.
    Built,
    Placed,
    TakenApart,
    /// A station at work, and something made at one.
    Working(Station),
    Made,
    /// A startled deer; a spider about to strike.
    Snort,
    Hiss,
    /// Small animals alarmed: a squirrel, a songbird, a duck, a heron or grouse, a fox or dog;
    /// and a bird taking off.
    Chatter,
    Chirp,
    Quack,
    Croak,
    Bark,
    Wings,
    /// A kind's own recorded voice, by the name of its set of clips (`data/creatures.ron`'s
    /// `cry` and `call`): what is heard in place of the sounds above for a kind that has one.
    Voiced(&'static str),
    /// A big creature heard at a moment of what it does.
    Cry(Cry),
    /// A settlement's bell.
    Bell,
    /// The interface: a click, a panel opened or closed.
    Click,
    Open,
    Close,
}

/// What a step is on, or made by.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Deserialize)]
pub enum Feet {
    Grass,
    Wood,
    Water,
    Hoof,
    Skitter,
    /// A foot the ground is felt under: a body of some tonnes.
    Heavy,
}

/// What a big creature is heard to do at a moment of one of its clips, when it is hurt, or
/// at rest (`data/creatures.ron`'s `heard`, `pained` and `voice`): its roar, the growl it
/// makes to itself, the grunt of a blow begun, its groan at a wound, a great limb through
/// the air, and a fist, a foot or its whole weight meeting the ground.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Deserialize)]
pub enum Cry {
    Roar,
    Growl,
    Grunt,
    Groan,
    Whoosh,
    Slam,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Noise>();
}
