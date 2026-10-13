//! What kinds of thing grow or lie about the ground, read from `assets/data/flora.ron`
//! (D127, D136): their models and colours, how they sway, which layer of growth they are
//! in and when it chooses them, and what cutting them gives. `flora` grows them; nothing
//! names one.
//!
//! The file is read into a registry, `Flora` (`data`).

use super::data;
use bevy::prelude::{Color, Resource};
use serde::Deserialize;
use terra_gen::Rng;

/// The sorts of thing that can be cut. Each is worked by one family of tool, and some
/// only by a tool of a better material (D96).
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    Tree,
    /// A tree too great for a stone axe.
    GreatTree,
    Rock,
    /// Rock with ore in it.
    Ore,
    Plant,
    /// A carcass, butchered.
    Flesh,
}

/// A kind of flora, by its place in the file.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FloraKind(pub u16);

/// What cutting something takes and gives.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cuttable {
    pub what: Cut,
    /// So many of what its kind gives, which the file names and gameplay knows as an item
    /// (`registries::FloraGives`).
    pub gives: (FloraKind, u32),
    /// How long it takes with a tool of ordinary speed, in seconds.
    pub seconds: f32,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct FloraFile {
    shared_colours: Vec<Colour>,
    kinds: Vec<FloraDef>,
}

/// A material a model names, our colour for it, and whether it is foliage.
type Colour = (String, (f32, f32, f32), Surface);

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
    pub name: String,
    pub models: Vec<String>,
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
    /// Its place in the file.
    #[serde(skip)]
    kind: Option<FloraKind>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct CutDef {
    what: Cut,
    gives: (String, f32, f32),
    seconds: (f32, f32),
    #[serde(default)]
    greater: Option<(f32, Cut)>,
}

/// Every kind of thing that grows (`assets/data/flora.ron`). A kind is known by its place
/// here.
#[derive(Resource)]
pub struct Flora(Vec<FloraDef>);

impl Flora {
    pub const FILE: &str = "flora.ron";

    /// Reads the file, giving each kind the colours every kind shares after its own.
    pub fn read() -> Self {
        let file: FloraFile = data::read(Self::FILE);
        let mut kinds = file.kinds;
        for (place, kind) in kinds.iter_mut().enumerate() {
            kind.colours.extend(file.shared_colours.iter().cloned());
            kind.kind = Some(FloraKind(place as u16));
        }
        Self(kinds)
    }

    /// Every kind, in the file's order.
    pub fn kinds(&self) -> &[FloraDef] {
        &self.0
    }

    /// The kind a layer grows, given what is known of the place: the layer's noise there,
    /// how grassy the ground is, and how big the thing is, where those matter. The layer's
    /// kinds are tried in the file's order; the first whose conditions hold is chosen,
    /// rolling for chance last, so the same place grows the same thing every time.
    pub fn choose(
        &self,
        layer: Grows,
        noise: f32,
        grass: f32,
        size: f32,
        rng: &mut Rng,
    ) -> Option<usize> {
        self.0.iter().position(|kind| {
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
}

impl FloraDef {
    /// Our colour for a part of one of its models, by the material the part names, as it
    /// should look on screen, and whether it is foliage, which catches the wind.
    pub fn colour(&self, material: &str) -> (Color, bool) {
        let material = material.split('.').next().unwrap_or(material);
        let ((red, green, blue), surface) = self
            .colours
            .iter()
            .find(|(named, ..)| named == material || named == "*")
            .map_or(
                ((0.4, 0.56, 0.24), Surface::Foliage),
                |(_, colour, surface)| (*colour, *surface),
            );
        (Color::srgb(red, green, blue), surface == Surface::Foliage)
    }

    /// The name of the item cutting it gives, as the file has it, if it can be cut.
    pub fn gives(&self) -> Option<&str> {
        self.cut.as_ref().map(|cut| cut.gives.0.as_str())
    }

    /// What cutting one of a size takes and gives, if it can be cut.
    pub fn cuttable(&self, size: f32) -> Option<Cuttable> {
        let cut = self.cut.as_ref()?;
        let what = match cut.greater {
            Some((from, greater)) if size >= from => greater,
            _ => cut.what,
        };
        let (_, base, per_m) = cut.gives;
        Some(Cuttable {
            what,
            gives: (self.kind?, (base + per_m * size).round() as u32),
            seconds: cut.seconds.0 + cut.seconds.1 * size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_gen::Seed;

    fn flora() -> Flora {
        Flora::read()
    }

    fn kind<'a>(flora: &'a Flora, name: &str) -> &'a FloraDef {
        flora.kinds().iter().find(|kind| kind.name == name).unwrap()
    }

    #[test]
    fn every_layer_has_something_to_grow_and_every_kind_a_model() {
        let flora = flora();
        let mut rng = Seed(1).derive("flora").rng();
        for layer in [
            Grows::Trees,
            Grows::Rocks,
            Grows::Bushes,
            Grows::Plants,
            Grows::Flowers,
        ] {
            assert!(
                flora.choose(layer, 0.0, 0.0, 0.0, &mut rng).is_some(),
                "nothing in {layer:?}"
            );
        }
        for kind in flora.kinds() {
            assert!(!kind.models.is_empty(), "{} has no models", kind.name);
        }
    }

    #[test]
    fn a_layer_chooses_by_its_noise_and_falls_back_on_its_last() {
        let flora = flora();
        let mut rng = Seed(1).derive("flora").rng();
        let mut name = |layer, noise, size| {
            flora
                .choose(layer, noise, 0.0, size, &mut rng)
                .map(|index| flora.kinds()[index].name.as_str())
        };
        assert_eq!(name(Grows::Trees, -0.5, 0.0), Some("Pine"));
        assert_eq!(name(Grows::Trees, 0.5, 0.0), Some("Birch"));
        assert_eq!(name(Grows::Trees, 0.0, 0.0), Some("Leafy tree"));
        // Small rocks on bare ground hold no ore and grow no moss.
        assert_eq!(name(Grows::Rocks, 0.0, 0.3), Some("Rock"));
    }

    #[test]
    fn a_great_tree_takes_iron_and_gives_more_wood() {
        let flora = flora();
        let pine = kind(&flora, "Pine");
        let small = pine.cuttable(9.0).unwrap();
        let great = pine.cuttable(15.0).unwrap();
        assert_eq!(small.what, Cut::Tree);
        assert_eq!(great.what, Cut::GreatTree);
        assert_eq!((small.gives.1, great.gives.1), (3, 5));
        // Ore rounds up, as one ore comes of any ore rock.
        let ore_rock = kind(&flora, "Ore rock");
        assert_eq!(ore_rock.gives(), Some("Ore"));
        assert_eq!(ore_rock.cuttable(0.8).unwrap().gives.1, 1);
        let (_, foliage) = pine.colour("Green.001");
        assert!(foliage);
        let (_, foliage) = kind(&flora, "Ore rock").colour("Anything");
        assert!(!foliage);
    }
}
