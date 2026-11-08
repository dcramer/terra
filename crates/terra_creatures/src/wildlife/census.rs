//! How many wild animals are about: counted against what is allowed, the birds apart from
//! the rest, and said to a scripted run (`wildlife.state`).

use super::{MOST, MOST_BIRDS, RELEASE_M};
use crate::creatures;
use crate::species::{Bestiary, Moves, Species};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

/// A creature in the picture is seen when it is at least so many pixels tall: less is a
/// speck nobody takes for an animal. What stands between is not asked: grass and leaves hide
/// some of those counted.
const SEEN_PIXELS: f32 = 6.0;
/// A bird's wings, spread, are so many times its body across.
const WINGS: f32 = 4.0;

/// Whether a kind flies: a bird, counted apart.
fn flies(species: Species, bestiary: &Bestiary) -> bool {
    matches!(bestiary.get(species).moves, Moves::Flies(_))
}

/// Whether a kind is each game's own, not the host's to tell: the small life that is there
/// for the eye (D647), which is the birds.
pub fn own_to_each(species: Species, bestiary: &Bestiary) -> bool {
    flies(species, bestiary)
}

/// The kinds a game puts out itself: every kind, or in a joined game only its own.
pub(super) fn kinds_here(bestiary: &Bestiary, joined: bool) -> impl Iterator<Item = Species> + '_ {
    bestiary
        .all()
        .filter(move |species| !joined || own_to_each(*species, bestiary))
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
    cameras: Query<(&Camera, &GlobalTransform), With<terra_world::planet::MainCamera>>,
) -> bevy::remote::BrpResult {
    let feet = player
        .iter()
        .next()
        .ok_or_else(|| terra_world::remote::refused("There is no player yet."))?
        .translation;
    let mut kinds = std::collections::BTreeMap::<&str, u32>::new();
    let (mut birds, mut all, mut flying) = (0, 0, 0);
    // Those the camera has in its picture, each at least so many pixels tall: what a player
    // sees of them, where a count of those near says only that they are there.
    let (mut shown, mut birds_shown) = (0, 0);
    let pixels = |at: Vec3, tall_m: f32| -> Option<f32> {
        let (camera, from) = cameras.iter().next()?;
        let height = camera.physical_viewport_size()?.y as f32;
        let foot = camera.world_to_ndc(from, at)?;
        let head = camera.world_to_ndc(from, at + at.normalize() * tall_m)?;
        let inside = foot.x.abs() <= 1.0 && foot.y.abs() <= 1.0 && (0.0..=1.0).contains(&foot.z);
        inside.then_some((head.y - foot.y).abs() * height / 2.0)
    };
    for (at, creature) in &placed {
        if creature.health <= 0.0 || at.translation.distance(feet) > RELEASE_M {
            continue;
        }
        *kinds.entry(bestiary.name(creature.species)).or_default() += 1;
        all += 1;
        let about = bestiary.get(creature.species);
        // On the wing it is seen by its wings, some four times its body across.
        let spread = if matches!(creature.state, creatures::State::Fly { .. }) {
            WINGS
        } else {
            1.0
        };
        let seen = pixels(
            at.translation,
            about.body.radius * 2.0 * at.scale.y * spread,
        )
        .is_some_and(|tall| tall >= SEEN_PIXELS);
        if seen {
            shown += 1;
            if flies(creature.species, &bestiary) {
                birds_shown += 1;
            }
        }
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
        serde_json::json!({ "about": all, "birds": birds, "flying": flying, "in_view": shown, "birds_in_view": birds_shown, "kinds": kinds, "most_birds": MOST_BIRDS, "most_others": MOST }),
    )
}
