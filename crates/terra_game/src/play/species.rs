//! What kinds of creature there are, read from `assets/data/creatures.ron` (D127, D136):
//! their models and animations, how they move, how they treat players, where they live,
//! and what they sound like and leave behind. Nothing else names a creature; code asks
//! what a kind is like, such as whether it is hostile or what butchering it gives.
//!
//! The file is read into a registry, `Bestiary` (`data`).

use super::bodies::Body;
use super::data;
use super::inventory::{Cut, Items};
use super::noise::Feet;
use bevy::prelude::Resource;
use serde::Deserialize;
use terra_items::ItemKind;

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
    /// Where it lives, if it lives about the land by its rules (`Lives::About`).
    #[serde(default)]
    pub niche: Option<Niche>,
    /// How it gets about: on foot, on the wing, or afloat.
    #[serde(default)]
    pub moves: Moves,
    /// Where it goes when it flees.
    #[serde(default)]
    pub refuge: Refuge,
    /// How deep a water it walks into, in metres.
    #[serde(default)]
    pub wades_m: f32,
    /// How far off it is drawn: as far as a thing its size is seen, a songbird some tens of
    /// metres, a deer or a heron across a clearing or a lake.
    #[serde(default = "seen_by_default")]
    pub seen_m: f32,
    /// How often it calls of its own accord while at rest, between so many seconds and so
    /// many, for a kind heard as it goes about its day; none for one that only calls in
    /// alarm.
    #[serde(default)]
    pub calls_s: Option<(f32, f32)>,
    #[serde(default)]
    pub pools: bool,
    pub stride: (f32, f32),
    pub feet: Feet,
    pub voice: Voice,
    pub blood: Blood,
    #[serde(default)]
    pub carcass: Option<CarcassDef>,
    pub lies_s: f32,
    /// Where on it a blow can land (D119): which of its model's joints are its head, body
    /// and legs, and how far round each a blow still counts.
    #[serde(default)]
    pub zones: Vec<ZoneDef>,
    /// A coat moulted with the seasons, if it has one (`coats`).
    #[serde(default)]
    pub coat: Option<CoatDef>,
}

/// A coat moulted with the seasons, as a mountain hare's or an arctic fox's: the tint its
/// summer coat lays over its model's own colours, linear RGB (its winter coat is the model's
/// own, pale), and how far one animal's tone strays from another's, a share either way.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CoatDef {
    pub summer: (f32, f32, f32),
    #[serde(default)]
    pub strays: f32,
    /// Whether it turns white in winter; one that does not keeps its tint the year round.
    #[serde(default = "moults_by_default")]
    pub moults: bool,
}

fn moults_by_default() -> bool {
    true
}

fn seen_by_default() -> f32 {
    200.0
}

impl SpeciesDef {
    pub fn seen_m(&self) -> f32 {
        self.seen_m
    }
}

/// Part of a creature a blow can land on, around the model's joints whose names contain
/// `joint`, out to `radius_m`.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ZoneDef {
    pub joint: String,
    pub part: Part,
    pub radius_m: f32,
}

/// Where on a creature a blow lands (D119).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Head,
    Body,
    Legs,
}

impl Part {
    pub fn name(self) -> &'static str {
        match self {
            Part::Head => "head",
            Part::Body => "body",
            Part::Legs => "legs",
        }
    }
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
    /// Flying, for one that flies.
    #[serde(default)]
    pub fly: Option<String>,
    /// How fast its walk and its run carry it as made, metres a second, so each is played as
    /// fast as it goes and its feet keep to the ground; none for a clip of its own pace.
    #[serde(default)]
    pub paces: Option<(f32, f32)>,
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
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Lives {
    Herd(u32),
    Lair(u32),
    /// About the land wherever its niche is, so many to a hectare of it, in groups of so many:
    /// wildlife found as it is walked into, as games' ambient populations are (`wildlife`).
    About {
        per_ha: f32,
        group: (u32, u32),
    },
}

/// Where a kind of creature living about the land is found: as warm a growing season as it
/// takes (its mean, °C, D313), and the places it keeps to. Rules, not places on a map, so a
/// new land has its animals where its climate and cover put them.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Niche {
    pub warmth_c: (f32, f32),
    pub haunts: Vec<Haunt>,
}

/// A kind of place an animal keeps to.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Haunt {
    /// Among standing trees.
    Woods,
    /// Open ground with no trees near: meadow, heath, fell.
    Open,
    /// Dry ground at the water's edge, or the shallows.
    Shore,
    /// Open water deep enough to swim on.
    Water,
    /// Ground trodden by people: a settlement's streets, square and yards.
    Trodden,
}

/// How a kind of creature gets about.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum Moves {
    /// On foot, and only on land and as deep a water as it wades.
    #[default]
    Walks,
    /// On foot, and on the wing at so many metres a second, from one place to the next further
    /// than it walks.
    Flies(f32),
    /// Afloat, keeping to the water, its feet so far under the surface.
    Swims(f32),
}

/// Where a shy creature goes when it flees.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum Refuge {
    /// Away, as fast as it runs.
    #[default]
    Runs,
    /// Up the nearest trunk, out of reach, until nobody is near.
    Climbs,
    /// Away on the wing, landing between so many metres off, somewhere it keeps to.
    Flies(f32, f32),
}

/// What a creature sounds like when it is alarmed.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    Snort,
    Hiss,
    /// A squirrel's scolding.
    Chatter,
    /// A small bird's alarm.
    Chirp,
    Quack,
    /// A heron's or a grouse's harsh call.
    Croak,
    Bark,
    /// Nothing: it flees in silence, as a hare does.
    Silent,
}

/// What sprays from a creature when it is struck.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blood {
    Red,
    Ichor,
}

/// What butchering a creature takes, and what it gives, as the file has it.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct CarcassDef {
    needs: Cut,
    gives: Vec<(String, u32)>,
}

/// What butchering a creature takes, and what it gives.
#[derive(Debug)]
pub struct Carcass {
    pub needs: Cut,
    pub gives: Vec<(ItemKind, u32)>,
}

/// A kind of creature: its place in the `Bestiary`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Species(u8);

/// Every kind of creature (`assets/data/creatures.ron`), and what butchering each gives,
/// by the kinds of item the registry of items knows.
#[derive(Resource)]
pub struct Bestiary {
    kinds: Vec<SpeciesDef>,
    carcasses: Vec<Option<Carcass>>,
}

impl Bestiary {
    pub fn read(items: &Items) -> Self {
        const FILE: &str = "creatures.ron";
        let kinds: Vec<SpeciesDef> = data::read(FILE);
        let carcasses = kinds
            .iter()
            .map(|kind| {
                kind.carcass.as_ref().map(|carcass| Carcass {
                    needs: carcass.needs,
                    gives: items.cost(&carcass.gives, FILE),
                })
            })
            .collect();
        Self { kinds, carcasses }
    }

    /// Every kind of creature.
    pub fn all(&self) -> impl Iterator<Item = Species> + use<> {
        (0..self.kinds.len()).map(|index| Species(index as u8))
    }

    /// The kind of creature called a name. Only tests name one.
    #[cfg(test)]
    pub fn named(&self, name: &str) -> Option<Species> {
        self.all().find(|&species| self.name(species) == name)
    }

    pub fn get(&self, species: Species) -> &SpeciesDef {
        &self.kinds[species.0 as usize]
    }

    pub fn name(&self, species: Species) -> &str {
        &self.get(species).name
    }

    /// What butchering it takes and gives, if it can be butchered.
    pub fn carcass(&self, species: Species) -> Option<&Carcass> {
        self.carcasses[species.0 as usize].as_ref()
    }

    /// Whether it goes for players.
    pub fn hostile(&self, species: Species) -> bool {
        matches!(self.get(species).temper, Temper::Hostile { .. })
    }

    /// The room it takes up on the ground, and how hard it is to shove.
    pub fn body(&self, species: Species) -> Body {
        let BodyDef { radius, weight } = self.get(species).body;
        Body::new(radius, weight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reading checks that what each kind gives is a kind of item there is.
    #[test]
    fn the_file_reads_and_names_each_kind_once() {
        let bestiary = Bestiary::read(&Items::default());
        assert!(bestiary.all().count() > 0);
        for species in bestiary.all() {
            assert_eq!(bestiary.named(bestiary.name(species)), Some(species));
        }
    }

    #[test]
    fn something_carried_can_butcher_whatever_leaves_a_carcass() {
        let items = Items::default();
        let bestiary = Bestiary::read(&items);
        for species in bestiary.all() {
            if let Some(carcass) = bestiary.carcass(species) {
                let can = items
                    .kinds()
                    .filter_map(|kind| items.tool(kind))
                    .any(|tool| tool.cuts(carcass.needs).is_some());
                assert!(can, "nothing butchers a {}", bestiary.name(species));
            }
        }
    }
}
