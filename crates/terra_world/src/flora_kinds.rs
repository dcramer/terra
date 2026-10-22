//! What kinds of thing grow or lie about the ground, read from `assets/data/flora.ron`
//! (D127, D136): their models and colours, how they sway, which layer of growth they are
//! in and when it chooses them, and what cutting them gives. `flora` grows them; nothing
//! names one.
//!
//! The file is read into a registry, `Flora` (`data`).

use super::climate::{ClimateFields, PastCache};
use super::data;
use super::grown::Grown;
use bevy::prelude::{Color, Resource, Vec3};
use serde::Deserialize;
use std::sync::Arc;
use terra_climate::{LeafYear, Past, WorldTime};
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
    /// Rushes and reeds on a lake's or pool's wet margin, where its kinds' `site` holds
    /// (`flora::under`).
    Shore,
    /// Ferns and shrubs on the shaded forest floor, under the trees rather than in the grass,
    /// where its kinds' `site` holds (`flora::under`).
    Understory,
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

/// The year where and when things are placed, as what dies back to the ground asks it
/// (D419): a flower is not there in January, though a tree stands bare. Every placing asks
/// it the same way, so a place grows the same things on the same date on every machine.
#[derive(Clone, Debug)]
pub enum Season {
    /// No climate is known, as on the yard: everything is up.
    Unknown,
    /// The plants' years, and the past and the day's length at a place and moment.
    At {
        years: Arc<Vec<LeafYear>>,
        past: Past,
        day_hours: f32,
    },
}

impl Season {
    /// The season at a point at a moment, from the climate's fields.
    pub fn at(fields: &ClimateFields, at: Vec3, time: WorldTime) -> Self {
        let past = fields.past_at(&mut PastCache::new(1), at, time);
        let day_hours = fields
            .fields
            .daylight(fields.frame.latitude(at), time.day())
            .day_hours;
        Season::At {
            years: fields.years.clone(),
            past,
            day_hours,
        }
    }

    /// How much of a kind's leaves are up, 0 to 1, by its year's name, at the middle of its
    /// kind's year: all of them where no season is known or no year has that name.
    pub fn up(&self, year: &str) -> f32 {
        let Season::At {
            years,
            past,
            day_hours,
        } = self
        else {
            return 1.0;
        };
        years
            .iter()
            .find(|kind| kind.kind == year)
            .map_or(1.0, |kind| kind.leaves(past, *day_hours, 0.0).on)
    }
}

/// What a layer that grows by the place knows of a point on the ground (`flora::under`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// How far the ground stands over the nearest water's surface, in metres; under it,
    /// below nothing.
    pub over_water_m: f32,
    /// How steep it is, in degrees from level.
    pub slope_deg: f32,
    /// How closely the trees stand over it, as a share of a closed forest's (the woods'
    /// `Stand::density`): how shaded its floor is.
    pub shade: f32,
    /// How far it is the open heath past the trees, 0 to 1 (`terra_land::heath`).
    pub heath: f32,
    /// How far its water is a mountain lake's, scoured by ice, 0 a lowland one to 1.
    pub mountain: f32,
}

/// Where a kind grows, by the ground's wetness, slope, shade and biome (D136): each range
/// is the least and most it takes, inclusive.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Site {
    /// Metres over the nearest water, as `Ground::over_water_m`.
    pub over_water: (f32, f32),
    /// The steepest ground it holds, in degrees.
    pub steepest: f32,
    /// The trees' shade over it, as `Ground::shade`.
    #[serde(default = "any_share")]
    pub shade: (f32, f32),
    /// Its water's mountain share, as `Ground::mountain`.
    #[serde(default = "any_share")]
    pub mountain: (f32, f32),
    /// The most of the open heath it grows on.
    #[serde(default)]
    pub heath: f32,
}

fn any_share() -> (f32, f32) {
    (0.0, 1.0)
}

impl Site {
    /// Whether it grows on that ground.
    pub fn fits(&self, ground: &Ground) -> bool {
        let within = |(least, most): (f32, f32), value: f32| (least..=most).contains(&value);
        within(self.over_water, ground.over_water_m)
            && ground.slope_deg <= self.steepest
            && within(self.shade, ground.shade)
            && within(self.mountain, ground.mountain)
            && ground.heath <= self.heath
    }
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
    /// Whether iron ore shows in it: rust-stained veins through its stone, and the rust
    /// their water carries down its faces (flora.wesl).
    #[serde(default)]
    pub rust: bool,
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
    /// Where on the ground it grows, for a kind of a layer chosen by the place (`Site`).
    #[serde(default)]
    pub site: Option<Site>,
    /// For what dies back to the ground once its leaves are gone, a herb or a flower: the
    /// year it keeps (`assets/data/plant_years.ron`), so none grows where that year has it
    /// gone (D419). A tree or bush stands bare instead, and names none.
    #[serde(default)]
    dies_back: Option<String>,
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
    /// how grassy the ground is, how big the thing is, where those matter, and the season,
    /// for what dies back in it. The layer's kinds are tried in the file's order; the first
    /// whose conditions hold is chosen, rolling for the season and chance last, so the same
    /// place grows the same thing every time, and in full leaf it rolls nothing for the
    /// season.
    pub fn choose(
        &self,
        layer: Grows,
        noise: f32,
        grass: f32,
        size: f32,
        season: &Season,
        rng: &mut Rng,
    ) -> Option<usize> {
        self.choose_where(layer, None, noise, grass, size, season, rng)
    }

    /// The kind a layer that grows by the place grows on that ground: the first of its
    /// kinds whose `site` holds there, and whose other conditions hold, as `choose` has them.
    pub fn choose_on(
        &self,
        layer: Grows,
        ground: &Ground,
        noise: f32,
        season: &Season,
        rng: &mut Rng,
    ) -> Option<usize> {
        self.choose_where(layer, Some(ground), noise, 0.0, 0.0, season, rng)
    }

    #[allow(clippy::too_many_arguments)]
    fn choose_where(
        &self,
        layer: Grows,
        ground: Option<&Ground>,
        noise: f32,
        grass: f32,
        size: f32,
        season: &Season,
        rng: &mut Rng,
    ) -> Option<usize> {
        self.0.iter().position(|kind| {
            kind.grows.same_layer(layer)
                && kind
                    .site
                    .is_none_or(|site| ground.is_some_and(|ground| site.fits(ground)))
                && kind.below.is_none_or(|below| noise < below)
                && kind.above.is_none_or(|above| noise > above)
                && kind
                    .grass_above
                    .is_none_or(|grass_above| grass > grass_above)
                && kind.size_above.is_none_or(|size_above| size > size_above)
                && kind.dies_back.as_deref().is_none_or(|year| {
                    let up = season.up(year);
                    up >= 1.0 || rng.unit() < up
                })
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
                flora
                    .choose(layer, 0.0, 0.0, 0.0, &Season::Unknown, &mut rng)
                    .is_some(),
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
    fn a_january_draw_grows_nothing_that_dies_back_and_a_june_one_does() {
        let flora = Flora::read();
        let fields = ClimateFields::for_tests(data::read(crate::climate::YEARS_FILE));
        for kind in flora.kinds() {
            if let Some(year) = &kind.dies_back {
                assert!(
                    fields.layer(year).is_some(),
                    "{} dies back by {year}, which plant_years.ron does not have",
                    kind.name
                );
            }
        }
        let at = Vec3::new(30.0, 2040.0, -10.0);
        let draw = |date: &str| {
            let day = terra_climate::Date::parse(date)
                .unwrap()
                .day(&fields.fields.climate().year);
            let season = Season::at(&fields, at, WorldTime::at(day, 12.0));
            let mut rng = rng();
            LAYERS
                .iter()
                .flat_map(|&layer| (0..400).map(move |_| layer))
                .filter_map(|layer| flora.choose(layer, 0.0, 1.0, 1.0, &season, &mut rng))
                .filter(|&kind| flora.kinds()[kind].dies_back.is_some())
                .map(|kind| flora.kinds()[kind].name.clone())
                .collect::<Vec<_>>()
        };
        let january = draw("01-10");
        assert!(january.is_empty(), "in January it grew {january:?}");
        assert!(
            draw("06-20").len() > 200,
            "in June the herbs and flowers grew"
        );
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
                .choose(layer, noise, grass, size, &Season::Unknown, &mut rng)
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
