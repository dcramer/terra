//! How many wild animals are about: counted against what is allowed, the birds apart from
//! the rest, and said to a scripted run (`wildlife.state`).

use super::{MOST, MOST_BIRDS, RELEASE_M};
use crate::creatures;
use crate::species::{Bestiary, Moves, Species};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

/// Whether a kind flies: a bird, counted apart.
fn flies(species: Species, bestiary: &Bestiary) -> bool {
    matches!(bestiary.get(species).moves, Moves::Flies(_))
}

/// The wild animals about: the birds, and the rest, each against its own most.
pub(super) struct About {
    birds: usize,
    others: usize,
}

impl About {
    /// Counted from the animals the cells have put out.
    pub(super) fn of(
        looked: &HashMap<IVec3, Vec<Entity>>,
        placed: &Query<(&Transform, &creatures::Creature)>,
        bestiary: &Bestiary,
    ) -> About {
        let mut about = About {
            birds: 0,
            others: 0,
        };
        for animal in looked.values().flatten() {
            match placed.get(*animal) {
                Ok((_, creature)) if flies(creature.species, bestiary) => about.birds += 1,
                _ => about.others += 1,
            }
        }
        about
    }

    /// Whether no more of a kind may be put out.
    pub(super) fn full(&self, species: Species, bestiary: &Bestiary) -> bool {
        if flies(species, bestiary) {
            self.birds >= MOST_BIRDS
        } else {
            self.others >= MOST
        }
    }

    pub(super) fn add(&mut self, species: Species, bestiary: &Bestiary, more: usize) {
        if flies(species, bestiary) {
            self.birds += more;
        } else {
            self.others += more;
        }
    }
}

/// `wildlife.state`: the living creatures within 150 m of this machine's player, how many
/// of each kind, how many of them are birds and how many of those on the wing now, so a scripted run reads how alive a place is with
/// no picture (D69, D647).
pub(super) fn state(
    In(_): In<Option<serde_json::Value>>,
    bestiary: Res<Bestiary>,
    player: Query<&Transform, terra_walker::IsLocalPlayer>,
    placed: Query<(&Transform, &creatures::Creature)>,
) -> bevy::remote::BrpResult {
    let feet = player
        .iter()
        .next()
        .ok_or_else(|| terra_world::remote::refused("There is no player yet."))?
        .translation;
    let mut kinds = std::collections::BTreeMap::<&str, u32>::new();
    let (mut birds, mut all, mut flying) = (0, 0, 0);
    for (at, creature) in &placed {
        if creature.health <= 0.0 || at.translation.distance(feet) > RELEASE_M {
            continue;
        }
        *kinds.entry(bestiary.name(creature.species)).or_default() += 1;
        all += 1;
        if matches!(
            bestiary.get(creature.species).moves,
            crate::species::Moves::Flies(_)
        ) {
            birds += 1;
            if matches!(creature.state, creatures::State::Fly { .. }) {
                flying += 1;
            }
        }
    }
    Ok(
        serde_json::json!({ "about": all, "birds": birds, "flying": flying, "kinds": kinds, "most_birds": MOST_BIRDS, "most_others": MOST }),
    )
}
