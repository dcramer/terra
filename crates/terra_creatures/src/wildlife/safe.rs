//! Where players arrive is safe to stand in (D250, D647): a kind that goes for players is not
//! put near its world's start, the village by it among it, and none is staged as a meeting
//! ahead of a walker, which is for the eye. A Worman on the village's paths killed a player
//! standing at the start.

use super::Wild;
use crate::species::Bestiary;

/// How far from a world's start a hostile kind keeps, metres.
const HOSTILE_FROM_START_M: f32 = 150.0;

/// Whether a kind may stand so far from its world's start: any may but one that goes for
/// players, which keeps its distance.
pub(super) fn keeps_from_start(hostile: bool, from_start_m: f32) -> bool {
    !hostile || from_start_m >= HOSTILE_FROM_START_M
}

/// The kinds a meeting may be staged with: a hostile kind stands where it lives, never ahead
/// of a walker for them to come on.
pub(super) fn for_the_eye<'a>(wild: &[Wild<'a>], bestiary: &Bestiary) -> Vec<Wild<'a>> {
    (wild.iter().copied())
        .filter(|(species, ..)| !bestiary.hostile(*species))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_hostile_lives_where_players_arrive() {
        assert!(!keeps_from_start(true, 0.0) && !keeps_from_start(true, 149.0));
        assert!(keeps_from_start(true, 150.0) && keeps_from_start(true, 900.0));
        assert!(keeps_from_start(false, 0.0) && keeps_from_start(false, 900.0));
    }
}
