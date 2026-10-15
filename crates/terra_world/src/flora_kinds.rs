//! What kinds of thing grow or lie about the ground, read from `assets/data/flora.ron`
//! (D127, D136): their models and colours, how they sway, which layer of growth they are
//! in and when it chooses them, and what cutting them gives. `flora` grows them; nothing
//! names one.
//!
//! The file is read into a registry, `Flora` (`data`).

use super::data;
use super::grown::Grown;
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

/// A material a model names, our colour for it, whether it is foliage, and the real
/// material it is drawn in, if any: a layer of `materials/flora` by name (`MATERIALS`).
#[derive(Deserialize, Debug, Clone)]
struct Colour(
    String,
    (f32, f32, f32),
    Surface,
    #[serde(default)] Option<String>,
);

/// The real materials a part of a model may be drawn in, in the order they are stacked in
/// `assets/materials/flora` (assets/materials/README.md), and how many metres each covers.
pub const MATERIALS: [(&str, f32); 6] = [
    ("pine_bark", 2.0),
    ("bark_brown_02", 1.0),
    ("leaves", 1.6),
    ("mossy_rock", 3.0),
    ("lichen_rock", 2.0),
    ("rock_boulder_dry", 1.8),
];

/// How a part of a model is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub colour: Color,
    /// Whether it catches the wind.
    pub foliage: bool,
    /// Which of [`MATERIALS`] it is drawn in, if any; plain colour if not.
    pub material: Option<u32>,
}

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
    /// The heath's dwarf shrubs past the trees, where its dwarf-shrub community grows
    /// (`terra_land::heath`).
    Heath,
    /// One placed by the base's door, so many metres east and south, so tall.
    ByTheDoor(f32, f32, f32),
}

impl Grows {
    fn same_layer(self, other: Grows) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }
}

fn four() -> u32 {
    4
}

/// One kind, as the file has it. The file says what each part means.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct FloraDef {
    pub name: String,
    #[serde(default)]
    pub models: Vec<String>,
    /// How it is grown, for a kind grown rather than modelled (`grown`).
    #[serde(default)]
    pub grown: Option<Grown>,
    /// How many variants of a grown kind there are.
    #[serde(default = "four")]
    pub variants: u32,
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
        Self::from_file(data::read(Self::FILE))
    }

    fn from_file(file: FloraFile) -> Self {
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
    /// How the part of a model with a material of this name is drawn: the first of the
    /// kind's colours that names it, or `*`, else plain foliage.
    pub fn look(&self, material: &str) -> Look {
        let material = material.split('.').next().unwrap_or(material);
        let found = self
            .colours
            .iter()
            .find(|Colour(named, ..)| named == material || named == "*");
        let Some(Colour(_, (red, green, blue), surface, real)) = found else {
            return Look {
                colour: Color::srgb(0.4, 0.56, 0.24),
                foliage: true,
                material: None,
            };
        };
        Look {
            colour: Color::srgb(*red, *green, *blue),
            foliage: *surface == Surface::Foliage,
            material: real
                .as_deref()
                .and_then(|name| MATERIALS.iter().position(|(m, _)| *m == name))
                .map(|layer| layer as u32),
        }
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

    const LAYERS: [Grows; 5] = [
        Grows::Trees,
        Grows::Rocks,
        Grows::Bushes,
        Grows::Plants,
        Grows::Flowers,
    ];

    /// Flora from kinds written here, so the rules are tested apart from the game's file.
    fn fixture(kinds: &str) -> Flora {
        let text = format!(
            "#![enable(implicit_some)]\n(shared_colours: [(\"*\", (0.1, 0.2, 0.3), Firm)], kinds: [{kinds}])"
        );
        Flora::from_file(ron::from_str(&text).unwrap())
    }

    fn rng() -> Rng {
        Seed(1).derive("flora").rng()
    }

    #[test]
    fn every_layer_has_something_to_grow_and_every_kind_its_models() {
        let flora = Flora::read();
        let mut rng = rng();
        for layer in LAYERS {
            assert!(
                flora.choose(layer, 0.0, 0.0, 0.0, &mut rng).is_some(),
                "nothing in {layer:?}"
            );
            // The last of a layer is what grows otherwise, so it asks for nothing.
            let last = flora
                .kinds()
                .iter()
                .rfind(|kind| kind.grows.same_layer(layer))
                .unwrap();
            assert!(
                last.below.is_none()
                    && last.above.is_none()
                    && last.grass_above.is_none()
                    && last.size_above.is_none()
                    && last.chance.is_none(),
                "{}, the last of {layer:?}, has conditions",
                last.name
            );
        }
        let folder = crate::data::assets().join("models/nature");
        for kind in flora.kinds() {
            assert!(
                kind.models.is_empty() == kind.grown.is_some(),
                "{} must have models or be grown, not both",
                kind.name
            );
            for model in &kind.models {
                assert!(
                    folder.join(format!("{model}.glb")).is_file(),
                    "{} names {model}, which is not in assets/models/nature",
                    kind.name
                );
            }
        }
    }

    #[test]
    fn a_layer_chooses_the_first_kind_whose_conditions_hold() {
        let flora = fixture(
            r#"
            (name: "Low", models: ["m"], sway: Still, grows: Trees, below: -0.5),
            (name: "High", models: ["m"], sway: Still, grows: Trees, above: 0.5),
            (name: "Also low", models: ["m"], sway: Still, grows: Trees, below: 0.0),
            (name: "Other", models: ["m"], sway: Still, grows: Trees),
            (name: "Grassy", models: ["m"], sway: Still, grows: Rocks, grass_above: 0.5),
            (name: "Big", models: ["m"], sway: Still, grows: Rocks, size_above: 2.0),
            (name: "Never", models: ["m"], sway: Still, grows: Rocks, chance: 0.0),
            (name: "Stone", models: ["m"], sway: Still, grows: Rocks),
            (name: "Always", models: ["m"], sway: Still, grows: Bushes, chance: 1.0),
            (name: "Shrub", models: ["m"], sway: Still, grows: Bushes),
            "#,
        );
        let mut rng = rng();
        let mut name = |layer, noise, grass, size| {
            flora
                .choose(layer, noise, grass, size, &mut rng)
                .map(|index| flora.kinds()[index].name.as_str())
        };
        // Below and above are strict, and the first that holds wins over a later one.
        assert_eq!(name(Grows::Trees, -0.6, 0.0, 0.0), Some("Low"));
        assert_eq!(name(Grows::Trees, -0.5, 0.0, 0.0), Some("Also low"));
        assert_eq!(name(Grows::Trees, 0.6, 0.0, 0.0), Some("High"));
        assert_eq!(name(Grows::Trees, 0.5, 0.0, 0.0), Some("Other"));
        // Grass and size are strict too, and a roll that never wins falls to the last.
        assert_eq!(name(Grows::Rocks, 0.0, 0.6, 3.0), Some("Grassy"));
        assert_eq!(name(Grows::Rocks, 0.0, 0.5, 3.0), Some("Big"));
        assert_eq!(name(Grows::Rocks, 0.0, 0.5, 2.0), Some("Stone"));
        assert_eq!(name(Grows::Bushes, 0.0, 0.0, 0.0), Some("Always"));
        // A layer with no kinds grows nothing.
        assert_eq!(name(Grows::Flowers, 0.0, 1.0, 1.0), None);
    }

    #[test]
    fn a_tree_is_cut_as_its_greater_sort_from_that_size_on() {
        let flora = fixture(
            r#"
            (name: "Tree", models: ["m"], colours: [("Green", (0.0, 1.0, 0.0), Foliage)],
                sway: Tree, grows: Trees,
                cut: (what: Tree, gives: ("Wood", 1.0, 0.5), seconds: (2.0, 0.25),
                    greater: (10.0, GreatTree))),
            (name: "Lump", models: ["m"], sway: Still, grows: Rocks,
                cut: (what: Ore, gives: ("Ore", 0.5, 1.0), seconds: (1.0, 0.0))),
            "#,
        );
        let [tree, lump] = flora.kinds() else {
            unreachable!()
        };
        let small = tree.cuttable(9.9).unwrap();
        let great = tree.cuttable(10.0).unwrap();
        assert_eq!((small.what, great.what), (Cut::Tree, Cut::GreatTree));
        // So many and so many more a metre: 1 + 0.5 x 10 wood, in 2 + 0.25 x 10 seconds.
        assert_eq!(great.gives, (FloraKind(0), 6));
        assert_eq!(great.seconds, 4.5);
        // What it gives rounds to the nearest: 0.5 + 0.8 is one, 0.5 + 1.2 is two.
        assert_eq!(lump.gives(), Some("Ore"));
        assert_eq!(lump.cuttable(0.8).unwrap().gives, (FloraKind(1), 1));
        assert_eq!(lump.cuttable(1.2).unwrap().gives.1, 2);
        // Its own colours first, then those every kind shares.
        assert!(tree.look("Green.001").foliage);
        assert!(!tree.look("Bark").foliage);
    }

    #[test]
    fn every_real_material_named_is_in_the_stack() {
        let flora = Flora::read();
        for kind in flora.kinds() {
            for Colour(named, _, _, real) in &kind.colours {
                if let Some(real) = real {
                    assert!(
                        MATERIALS.iter().any(|(m, _)| m == real),
                        "{}'s {named} is drawn in {real}, which is not in materials/flora",
                        kind.name
                    );
                }
            }
        }
        let trunk = flora.kinds()[0].look("Wood");
        assert!(trunk.material.is_some() && !trunk.foliage);
    }
}
