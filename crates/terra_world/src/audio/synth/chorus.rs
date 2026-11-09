//! A place's own calls (`Sound::Calls`): which sounds make each chorus.

use super::Sound;

/// Whose calls a place has, besides the songbirds (`Birds`): what a player hears of the life
/// there they mostly do not see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chorus {
    /// A wood: woodpeckers drumming and wood pigeons cooing, far off among the trees.
    Woods,
    /// Open fells and heath: plovers whistling, now and then a grouse's croak.
    Fells,
    /// A lake's shore: ducks quacking out on the water, a heron's croak, fish rising.
    Shore,
    /// A town's yards and streets: hens clucking, sparrows, a dog now and then.
    Yards,
}

impl Chorus {
    /// The calls it is made of, each with how often it comes of them all, and its length.
    pub(super) fn calls(self) -> &'static [(Sound, f32, f32)] {
        match self {
            Chorus::Woods => &[(Sound::Drum, 0.45, 0.8), (Sound::Coo, 0.55, 2.5)],
            Chorus::Fells => &[(Sound::Whistle, 0.8, 0.6), (Sound::Croak, 0.2, 0.5)],
            Chorus::Shore => &[
                (Sound::Quack, 0.5, 0.75),
                (Sound::Plop, 0.35, 0.3),
                (Sound::Croak, 0.15, 0.5),
            ],
            Chorus::Yards => &[
                (Sound::Cluck, 0.5, 0.9),
                (Sound::Chirp, 0.35, 0.45),
                (Sound::Bark, 0.15, 0.3),
            ],
        }
    }
}
