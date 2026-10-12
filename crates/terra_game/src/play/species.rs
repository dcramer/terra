//! What kinds of creature there are, read from `assets/data/creatures.ron` (D127, D136):
//! their models and animations, how they move, how they treat players, where they live,
//! and what they sound like and leave behind. Nothing else names a creature; code asks
//! what a kind is like, such as whether it is hostile or what butchering it gives.
//!
//! The file is built into the game, so every machine has the same kinds (D28).

use super::bodies::Body;
use super::inventory::Cut;
use super::noise::Feet;
use serde::Deserialize;
use std::{fmt, sync::LazyLock};

const CREATURES: &str = include_str!("../../../../assets/data/creatures.ron");

/// A kind of creature, as the file has it. The file says what each part means.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct SpeciesDef {
    pub name: String,
    pub model: (String, f32),
    pub clips: ClipNames,
    pub health: f32,
    pub walk_mps: f32,
    pub run_mps: f32,
    pub body: BodyDef,
    pub temper: Temper,
    pub lives: Lives,
    #[serde(default)]
    pub pools: bool,
    pub stride: (f32, f32),
    pub feet: Feet,
    pub voice: Voice,
    pub blood: Blood,
    #[serde(default)]
    pub carcass: Option<Carcass>,
    pub lies_s: f32,
}

/// Its animations, by the names the model gives them.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct ClipNames {
    pub idle: String,
    pub walk: String,
    pub run: String,
    #[serde(default)]
    pub eat: Option<String>,
    pub attack: String,
    pub death: String,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct BodyDef {
    pub radius: f32,
    pub weight: f32,
}

/// How a kind of creature treats players.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Temper {
    /// Wildlife: runs from anyone near, or from being hurt.
    Shy { flees_within_m: f32, flees_s: f32 },
    /// A monster: chases anyone near, bites within reach, and goes home if led too far.
    Hostile {
        notices_m: f32,
        leash_m: f32,
        attacks_within_m: f32,
        bite: f32,
        bite_at_s: f32,
        bite_reach_m: f32,
        attack_s: f32,
    },
}

/// Where a kind of creature is put in a world, and how many.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lives {
    Herd(u32),
    Lair(u32),
}

/// What a creature sounds like when it is alarmed.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    Snort,
    Hiss,
}

/// What sprays from a creature when it is struck.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blood {
    Red,
    Ichor,
}

/// What butchering a creature takes, and what it gives, by item name.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Carcass {
    pub needs: Cut,
    pub gives: Vec<(String, u32)>,
}

static SPECIES: LazyLock<Vec<SpeciesDef>> = LazyLock::new(|| {
    ron::from_str(CREATURES)
        .unwrap_or_else(|problem| panic!("assets/data/creatures.ron: {problem}"))
});

/// A kind of creature: its place in `assets/data/creatures.ron`. It prints as its name.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Species(u8);

impl Species {
    /// Every kind of creature.
    pub fn all() -> impl Iterator<Item = Species> {
        (0..SPECIES.len()).map(|index| Species(index as u8))
    }

    /// The kind of creature called a name. Only tests name one.
    #[cfg(test)]
    pub fn named(name: &str) -> Option<Species> {
        Species::all().find(|species| species.name() == name)
    }

    pub fn def(self) -> &'static SpeciesDef {
        &SPECIES[self.0 as usize]
    }

    pub fn name(self) -> &'static str {
        &self.def().name
    }

    /// Whether it goes for players.
    pub fn hostile(self) -> bool {
        matches!(self.def().temper, Temper::Hostile { .. })
    }

    /// The room it takes up on the ground, and how hard it is to shove.
    pub fn body(self) -> Body {
        let BodyDef { radius, weight } = self.def().body;
        Body::new(radius, weight)
    }
}

impl fmt::Display for Species {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl fmt::Debug for Species {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::inventory::Items;

    #[test]
    fn a_kind_prints_as_its_name() {
        let deer = Species::named("Deer").unwrap();
        assert_eq!(format!("{deer} {deer:?}"), "Deer Deer");
    }

    #[test]
    fn the_file_reads_and_what_it_names_exists() {
        let items = Items::default();
        assert!(Species::all().count() > 0);
        for species in Species::all() {
            assert_eq!(Species::named(species.name()), Some(species));
            for (name, _) in species
                .def()
                .carcass
                .iter()
                .flat_map(|carcass| &carcass.gives)
            {
                assert!(
                    items.find(name).is_some(),
                    "{species} gives {name}, not a kind of item"
                );
            }
        }
    }

    #[test]
    fn something_carried_can_butcher_whatever_leaves_a_carcass() {
        let items = Items::default();
        for species in Species::all() {
            if let Some(carcass) = &species.def().carcass {
                let can = items
                    .kinds()
                    .filter_map(|kind| items.tool(kind))
                    .any(|tool| tool.cuts(carcass.needs).is_some());
                assert!(can, "nothing butchers a {species}");
            }
        }
    }
}
