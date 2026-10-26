//! What a blow meets, as it is heard (docs/audio.md, D113): the surfaces a contact is between
//! and how heavy the blow, the facts the sound module chooses a contact's sound by from its
//! table in data (D447). What a fighter strikes with and guards with is its move set's kit; a
//! body is flesh until it wears something.

use crate::moves::{Move, Taken};
use serde::Deserialize;

/// What a thing is made of, as a blow meets it.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Surface {
    Steel,
    Iron,
    Wood,
    Flesh,
}

/// What a set fights with, as a blow meets it: its blade, its shield's face (where a blow on
/// the guard lands) and its boss (where a parry turns one).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Kit {
    pub blade: Surface,
    pub face: Surface,
    pub boss: Surface,
}

/// A blow meeting something: how it was taken, the blade's surface and the one it met, and
/// whether the move was heavy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Struck {
    pub taken: Taken,
    pub by: Surface,
    pub on: Surface,
    pub heavy: bool,
}

impl Struck {
    /// A blow of `blow`, struck with `by`'s blade on a fighter guarding with `on` and with a
    /// body of `body`: on the guard it meets the shield's face, parried its boss, a hit the
    /// body. A dodged blow meets nothing.
    pub fn of(taken: Taken, blow: &Move, by: &Kit, on: &Kit, body: Surface) -> Option<Self> {
        let on = match taken {
            Taken::Hit => body,
            Taken::Blocked | Taken::GuardBroken => on.face,
            Taken::Parried => on.boss,
            Taken::Dodged => return None,
        };
        Some(Self {
            taken,
            by: by.blade,
            on,
            heavy: blow.heavy,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moves::MoveSet;

    fn set() -> MoveSet {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/data/moves/sword.shielded.ron"
        );
        MoveSet::read(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn a_blow_meets_the_shields_face_on_the_guard_its_boss_parried_and_the_body_on_a_hit() {
        let set = set();
        let (light, heavy) = (set.get("slash").unwrap(), set.get("heavy").unwrap());
        let shield = Kit {
            blade: Surface::Steel,
            face: Surface::Wood,
            boss: Surface::Iron,
        };
        let of = |taken, blow| Struck::of(taken, blow, &set.kit, &shield, Surface::Flesh);
        let met = |taken, blow| of(taken, blow).map(|struck| (struck.on, struck.heavy));
        assert_eq!(met(Taken::Blocked, light), Some((Surface::Wood, false)));
        assert_eq!(met(Taken::GuardBroken, heavy), Some((Surface::Wood, true)));
        assert_eq!(met(Taken::Parried, light), Some((Surface::Iron, false)));
        assert_eq!(met(Taken::Hit, heavy), Some((Surface::Flesh, true)));
        assert_eq!(of(Taken::Dodged, light), None);
        // The blade is the striker's own.
        assert_eq!(of(Taken::Hit, light).unwrap().by, set.kit.blade);
    }
}
