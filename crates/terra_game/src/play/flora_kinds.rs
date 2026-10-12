//! What kinds of thing grow or lie about the ground, read from `assets/data/flora.ron`
//! (D127, D136): their models and colours, how they sway, which layer of growth they are
//! in and when it chooses them, and what cutting them gives. `flora` grows them; nothing
//! names one.
//!
//! The file is built into the game, so every machine grows the same world (D28).

use super::inventory::{Cut, Cuttable};
use bevy::prelude::Color;
use serde::Deserialize;
use std::sync::LazyLock;
use terra_gen::Rng;

const FLORA: &str = include_str!("../../../../assets/data/flora.ron");

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields, bound(deserialize = "'de: 'static"))]
struct FloraFile {
    shared_colours: Vec<Colour>,
    kinds: Vec<FloraDef>,
}

/// A material a model names, our colour for it, and whether it is foliage.
type Colour = (&'static str, (f32, f32, f32), Surface);

/// Whether a part of a model catches the wind.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Foliage,
    Firm,
}

/// How a kind moves in the wind.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sway {
    Tree,
    Bush,
    Plant,
    Still,
}

/// The layer of growth a kind is in. Each is worked out in its own way (`flora`).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Grows {
    Trees,
    Rocks,
    Bushes,
    Plants,
    Flowers,
    /// One placed by the base's door, so many metres east and south, so tall.
    ByTheDoor(f32, f32, f32),
}

impl Grows {
    fn same_layer(self, other: Grows) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }
}

/// One kind, as the file has it. The file says what each part means.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct FloraDef {
    pub name: &'static str,
    pub models: Vec<&'static str>,
    #[serde(default)]
    colours: Vec<Colour>,
    pub sway: Sway,
    pub grows: Grows,
    #[serde(default)]
    below: Option<f32>,
    #[serde(default)]
    above: Option<f32>,
    #[serde(default)]
    grass_above: Option<f32>,
    #[serde(default)]
    size_above: Option<f32>,
    #[serde(default)]
    chance: Option<f32>,
    #[serde(default)]
    pub size: Option<(f32, f32)>,
    #[serde(default)]
    cut: Option<CutDef>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields, bound(deserialize = "'de: 'static"))]
struct CutDef {
    what: Cut,
    gives: (&'static str, f32, f32),
    seconds: (f32, f32),
    #[serde(default)]
    greater: Option<(f32, Cut)>,
}

static FLORA_FILE: LazyLock<FloraFile> = LazyLock::new(|| {
    ron::from_str(FLORA).unwrap_or_else(|problem| panic!("assets/data/flora.ron: {problem}"))
});

/// Every kind, in the file's order. A kind is known by its place here.
pub fn kinds() -> &'static [FloraDef] {
    &FLORA_FILE.kinds
}

/// The kind a layer grows, given what is known of the place: the layer's noise there, how
/// grassy the ground is, and how big the thing is, where those matter. The layer's kinds are
/// tried in the file's order; the first whose conditions hold is chosen, rolling for chance
/// last, so the same place grows the same thing every time.
pub fn choose(layer: Grows, noise: f32, grass: f32, size: f32, rng: &mut Rng) -> Option<usize> {
    kinds().iter().position(|kind| {
        kind.grows.same_layer(layer)
            && kind.below.is_none_or(|below| noise < below)
            && kind.above.is_none_or(|above| noise > above)
            && kind
                .grass_above
                .is_none_or(|grass_above| grass > grass_above)
            && kind.size_above.is_none_or(|size_above| size > size_above)
            && kind.chance.is_none_or(|chance| rng.unit() < chance)
    })
}

impl FloraDef {
    /// Our colour for a part of one of its models, by the material the part names, as it
    /// should look on screen, and whether it is foliage, which catches the wind.
    pub fn colour(&self, material: &str) -> (Color, bool) {
        let material = material.split('.').next().unwrap_or(material);
        let (_, (red, green, blue), surface) = self
            .colours
            .iter()
            .chain(&FLORA_FILE.shared_colours)
            .find(|(named, ..)| *named == material || *named == "*")
            .copied()
            .unwrap_or(("*", (0.4, 0.56, 0.24), Surface::Foliage));
        (Color::srgb(red, green, blue), surface == Surface::Foliage)
    }

    /// What cutting one of a size takes and gives, if it can be cut.
    pub fn cuttable(&self, size: f32) -> Option<Cuttable> {
        let cut = self.cut.as_ref()?;
        let what = match cut.greater {
            Some((from, greater)) if size >= from => greater,
            _ => cut.what,
        };
        let (gives, base, per_m) = cut.gives;
        Some(Cuttable {
            what,
            gives: (gives, (base + per_m * size).round() as u32),
            seconds: cut.seconds.0 + cut.seconds.1 * size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::inventory::Items;
    use terra_gen::Seed;

    fn kind(name: &str) -> &'static FloraDef {
        kinds().iter().find(|kind| kind.name == name).unwrap()
    }

    #[test]
    fn every_layer_has_something_to_grow_and_every_cut_gives_an_item() {
        let items = Items::default();
        let mut rng = Seed(1).derive("flora").rng();
        for layer in [
            Grows::Trees,
            Grows::Rocks,
            Grows::Bushes,
            Grows::Plants,
            Grows::Flowers,
        ] {
            assert!(
                choose(layer, 0.0, 0.0, 0.0, &mut rng).is_some(),
                "nothing in {layer:?}"
            );
        }
        for kind in kinds() {
            assert!(!kind.models.is_empty(), "{} has no models", kind.name);
            if let Some(cut) = kind.cuttable(1.0) {
                assert!(
                    items.find(cut.gives.0).is_some(),
                    "{} gives {}",
                    kind.name,
                    cut.gives.0
                );
            }
        }
    }

    #[test]
    fn a_layer_chooses_by_its_noise_and_falls_back_on_its_last() {
        let mut rng = Seed(1).derive("flora").rng();
        let name = |index: Option<usize>| index.map(|index| kinds()[index].name);
        assert_eq!(
            name(choose(Grows::Trees, -0.5, 0.0, 0.0, &mut rng)),
            Some("Pine")
        );
        assert_eq!(
            name(choose(Grows::Trees, 0.5, 0.0, 0.0, &mut rng)),
            Some("Birch")
        );
        assert_eq!(
            name(choose(Grows::Trees, 0.0, 0.0, 0.0, &mut rng)),
            Some("Leafy tree")
        );
        // Small rocks on bare ground hold no ore and grow no moss.
        assert_eq!(
            name(choose(Grows::Rocks, 0.0, 0.0, 0.3, &mut rng)),
            Some("Rock")
        );
    }

    #[test]
    fn a_great_tree_takes_iron_and_gives_more_wood() {
        let pine = kind("Pine");
        let small = pine.cuttable(9.0).unwrap();
        let great = pine.cuttable(15.0).unwrap();
        assert_eq!(small.what, Cut::Tree);
        assert_eq!(great.what, Cut::GreatTree);
        assert_eq!((small.gives.1, great.gives.1), (3, 5));
        // Ore rounds up, as one ore comes of any ore rock.
        assert_eq!(kind("Ore rock").cuttable(0.8).unwrap().gives, ("Ore", 1));
        let (_, foliage) = pine.colour("Green.001");
        assert!(foliage);
        let (_, foliage) = kind("Ore rock").colour("Anything");
        assert!(!foliage);
    }
}
