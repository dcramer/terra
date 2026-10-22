//! What the builder can build: a short list of parts, each with one or more variants.
//!
//! A part is what you pick from the dial: a room, a window, a seat. A variant is a kind
//! of that part, cycled through once the part is picked: a chair, a stool. The list is
//! data, in `assets/data/builds.ron` (D136), and a stand-in. What can be built, what it
//! costs and how it is unlocked are design questions nobody has answered (docs/building.md, docs/progression.md).

use crate::crafting::{Station, Stations};
use crate::inventory::{Cost, Items};
use bevy::prelude::{Resource, Vec3};
use serde::Deserialize;
use std::sync::Arc;
use terra_build::{Panel, PropKind};
use terra_build_layout::fitting::Hang;
use terra_items::ItemKind;
use terra_world::data;

const FILE: &str = "builds.ron";

/// What the hammer builds (`assets/data/builds.ron`): the parts, what the pieces of a base
/// cost, and how a claim grows. Read with items and stations named as their registries
/// name them.
#[derive(Resource)]
pub struct Builds {
    parts: Vec<Part>,
    fittings: Vec<Arc<Fitting>>,
    pub claim: ClaimRule,
}

/// How a claim grows (D104): how far across it is at each step, and what growing to the
/// next costs.
pub struct ClaimRule {
    pub across_m: Vec<f32>,
    pub grow: Vec<Vec<(ItemKind, u32)>>,
    /// How many cubes it may have planned and not yet built, at each step (D193).
    pub planned: Vec<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildsFile {
    fittings: Vec<FittingFile>,
    claim: ClaimFile,
    parts: Vec<PartFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FittingFile {
    name: String,
    model: String,
    hang: Hang,
    dims: (f32, f32, f32),
    fits: Vec<(i32, i32)>,
    #[serde(default)]
    frame: Option<String>,
    cost: Vec<(String, u32)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimFile {
    across_m: Vec<f32>,
    grow: Vec<Vec<(String, u32)>>,
    planned: Vec<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PartFile {
    name: String,
    tab: Tab,
    kind: KindFile,
}

#[derive(Deserialize)]
enum KindFile {
    Room(Vec<RoomShape>),
    Roof(Vec<RoofShape>),
    Wall(Vec<(String, f32)>),
    Window(Vec<(String, f32, (f32, f32))>),
    Door,
    Fit(Hang),
    Items(Vec<ItemFile>),
    Remove,
    Move,
    Floor,
    Flight(Vec<FlightShape>),
    Post,
    Block(Vec<(String, BlockShape)>),
    Replace,
    Carve,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemFile {
    name: String,
    file: String,
    dims: (f32, f32, f32),
    cost: Vec<(String, u32)>,
    #[serde(default)]
    light: Light,
    #[serde(default)]
    station: Option<String>,
    #[serde(default)]
    claims: bool,
    #[serde(default)]
    rest: Option<Rest>,
    #[serde(default)]
    holds: Option<(u8, u8)>,
}

impl Builds {
    pub fn read(items: &Items, stations: &Stations) -> Self {
        let file: BuildsFile = data::read(FILE);
        let cost = |named: &[(String, u32)]| items.cost(named, FILE);
        let item = |item: ItemFile| Item {
            cost: cost(&item.cost),
            station: item.station.map(|name| {
                stations
                    .named(&name)
                    .unwrap_or_else(|| panic!("{FILE}: there is no station called {name}"))
            }),
            name: item.name,
            file: item.file,
            dims: Vec3::from(item.dims),
            light: item.light,
            claims: item.claims,
            rest: item.rest,
            holds: item.holds,
        };
        let fittings: Vec<Arc<Fitting>> = file
            .fittings
            .into_iter()
            .enumerate()
            .map(|(index, each)| {
                Arc::new(Fitting {
                    kind: PropKind(index as u16),
                    cost: cost(&each.cost),
                    name: each.name,
                    model: each.model,
                    hang: each.hang,
                    dims: Vec3::from(each.dims),
                    fits: each.fits,
                    frame: each.frame,
                })
            })
            .collect();
        let parts = file
            .parts
            .into_iter()
            .map(|part| Part {
                name: part.name,
                tab: part.tab,
                kind: match part.kind {
                    KindFile::Room(shapes) => Kind::Room(shapes),
                    KindFile::Roof(shapes) => Kind::Roof(shapes),
                    KindFile::Wall(lengths) => Kind::Wall(lengths),
                    KindFile::Window(windows) => Kind::Window(
                        windows
                            .into_iter()
                            .map(|(name, width, (sill, top))| {
                                (name, width, Panel::Window { sill, top })
                            })
                            .collect(),
                    ),
                    KindFile::Door => Kind::Door,
                    KindFile::Fit(hang) => Kind::Fit(
                        fittings
                            .iter()
                            .filter(|fitting| fitting.hang == hang)
                            .cloned()
                            .collect(),
                    ),
                    KindFile::Items(items) => {
                        Kind::Items(items.into_iter().map(|each| Arc::new(item(each))).collect())
                    }
                    KindFile::Remove => Kind::Remove,
                    KindFile::Move => Kind::Move,
                    KindFile::Floor => Kind::Floor,
                    KindFile::Flight(shapes) => Kind::Flight(shapes),
                    KindFile::Post => Kind::Post,
                    KindFile::Block(shapes) => Kind::Block(shapes),
                    KindFile::Replace => Kind::Replace,
                    KindFile::Carve => Kind::Carve,
                },
            })
            .collect();
        Self {
            parts,
            fittings,
            claim: ClaimRule {
                across_m: file.claim.across_m,
                grow: file.claim.grow.iter().map(|each| cost(each)).collect(),
                planned: file.claim.planned,
            },
        }
    }

    /// The parts, in the order the build menu lists them within each tab.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    pub fn part(&self, index: usize) -> &Part {
        &self.parts[index]
    }

    /// The doors and shutters fitted into openings, each a kind of prop by its place.
    pub fn fittings(&self) -> &[Arc<Fitting>] {
        &self.fittings
    }

    /// A kind of door or shutters.
    pub fn fitting(&self, kind: PropKind) -> Option<&Arc<Fitting>> {
        self.fittings.get(usize::from(kind.0))
    }

    /// Every item any part offers.
    pub fn items(&self) -> impl Iterator<Item = &Arc<Item>> {
        self.parts.iter().flat_map(|part| match &part.kind {
            Kind::Items(items) => items.iter(),
            _ => [].iter(),
        })
    }

    /// The item whose model is a given file.
    pub fn item_from(&self, file: &str) -> Option<Arc<Item>> {
        self.items().find(|item| item.file == file).cloned()
    }

    /// The item called `name`: what it is, which outlasts how it looks.
    pub fn item_named(&self, name: &str) -> Option<Arc<Item>> {
        self.items().find(|item| item.name == name).cloned()
    }

    /// The part and variant that an item is, for picking "another of those".
    pub fn place_of(&self, wanted: &Item) -> Option<(usize, usize)> {
        self.parts
            .iter()
            .enumerate()
            .find_map(|(part, entry)| match &entry.kind {
                Kind::Items(items) => items
                    .iter()
                    .position(|item| **item == *wanted)
                    .map(|variant| (part, variant)),
                _ => None,
            })
    }

    /// Where a part is in the list, for the parts that have their own key.
    pub fn part_of(&self, wanted: fn(&Kind) -> bool) -> usize {
        self.parts
            .iter()
            .position(|part| wanted(&part.kind))
            .expect("the catalogue has this part")
    }
}

/// How long something takes to build, from what it costs: half a second, and a little
/// more for every raw item in it, up to three seconds. A stand-in, like the costs.
pub fn build_seconds(cost: &Cost, times: u32) -> f32 {
    let raw_items: u32 = cost.iter().map(|&(_, each)| each * times).sum();
    (0.5 + 0.12 * raw_items as f32).min(3.0)
}

/// Taking a thing apart is quicker than building it.
pub fn take_apart_seconds(cost: &Cost, times: u32) -> f32 {
    build_seconds(cost, times) * 0.6
}

/// Something stood on a floor or on the ground. Its model is a file in
/// `assets/models/props`.
#[derive(Debug, PartialEq)]
pub struct Item {
    pub name: String,
    pub file: String,
    /// How wide, high and deep a real one is, in metres (D110), along its model's X, Y
    /// and Z: its model is made to it and drawn as it is, which the model contract's
    /// checker holds it to (`contract`, docs/art-style.md).
    pub dims: Vec3,
    /// The raw items it is built from, and gives back when taken apart.
    pub cost: Vec<(ItemKind, u32)>,
    /// Whether it gives off light, and how.
    pub light: Light,
    /// The station it is, if it is one.
    pub station: Option<Station>,
    /// Whether it is a claim stone, which claims the land around it (D94).
    pub claims: bool,
    /// What someone can rest on it for, if anything: sitting, or sleeping and waking.
    pub rest: Option<Rest>,
    /// The cells it stores, wide by high, if it is storage (D84, D141): a grid as the
    /// backpack is, which building in its claim draws on (`storage`).
    pub holds: Option<(u8, u8)>,
}

/// A door or shutters fitted into a hole in a wall (D192, D207): a kind of prop, its
/// model in `assets/models/openings`, the holes it fits, and what a leaf of it costs, a
/// pair costing two.
#[derive(Debug, PartialEq)]
pub struct Fitting {
    pub name: String,
    pub model: String,
    pub hang: Hang,
    /// How wide, high and thick one leaf is, in metres, in the first hole it fits: its
    /// model is made to it, hinged at its origin (docs/art-style.md, the model contract),
    /// and stretched across and up to fit a bigger hole.
    pub dims: Vec3,
    /// The holes it fits, wide by high, in cubes.
    pub fits: Vec<(i32, i32)>,
    /// What a leaf of it costs.
    pub cost: Vec<(ItemKind, u32)>,
    /// The kind of prop it is, as the base keeps it.
    pub kind: PropKind,
    /// The material its frame is drawn in, a kit's name: a door or shutters come with their
    /// frame (`terra_build_layout::fitting::Prop::frame`).
    pub frame: Option<String>,
}

impl Fitting {
    /// Whether it fits a hole of so many cubes wide and high.
    pub fn fits(&self, wide: i32, high: i32) -> bool {
        self.fits.contains(&(wide, high))
    }

    /// The holes it fits, in metres, for saying so.
    pub fn sizes(&self) -> String {
        let metres = |cubes: i32| cubes as f32 * terra_build::CELL_M;
        self.fits
            .iter()
            .map(|&(wide, high)| format!("{} by {} m", metres(wide), metres(high)))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// What something is to rest on.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize)]
pub enum Rest {
    /// People sit on it (`people`).
    Sit,
    /// A bed: you wake in it after going down, if it is in your claim (`death`).
    Sleep,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, Deserialize)]
pub enum Light {
    #[default]
    None,
    Fire,
}

/// A room's footprint in metres, east by south.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomShape {
    pub name: String,
    pub metres: [f32; 2],
}

/// A kind of roof the hammer adds: of roof cubes in a style, as the roofer lays them
/// (D241), or flat.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoofShape {
    pub name: String,
    /// `None` for a flat roof.
    pub style: Option<terra_build::roofer::Style>,
}

/// A flight a storey up (D193): stairs or a ramp, so many metres wide.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightShape {
    pub name: String,
    pub ramp: bool,
    pub wide: f32,
}

/// A shape one cube is cut to (D193).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum BlockShape {
    Whole,
    Half,
    Stair,
    Slope,
    UpperHalf,
    InvertedStair,
    InvertedSlope,
    OuterCorner,
    InnerCorner,
}

impl BlockShape {
    /// The cube's shape, a stair, slope or corner rising toward `facing` (D231).
    pub fn shape(self, facing: terra_build::Facing) -> terra_build::Shape {
        use terra_build::Shape;
        match self {
            BlockShape::Whole => Shape::Whole,
            BlockShape::Half => Shape::Half,
            BlockShape::Stair => Shape::Stair(facing),
            BlockShape::Slope => Shape::Slope(facing),
            BlockShape::UpperHalf => Shape::UpperHalf,
            BlockShape::InvertedStair => Shape::InvertedStair(facing),
            BlockShape::InvertedSlope => Shape::InvertedSlope(facing),
            BlockShape::OuterCorner => Shape::OuterCorner(facing),
            BlockShape::InnerCorner => Shape::InnerCorner(facing),
        }
    }

    /// Whether it faces a way, which the wheel turns.
    pub fn turns(self) -> bool {
        self.shape(terra_build::Facing::North).facing().is_some()
    }
}

pub enum Kind {
    Room(Vec<RoomShape>),
    /// A roof over a rectangle, drawn or over a room, resting on the walls of a storey.
    Roof(Vec<RoofShape>),
    /// A stretch of plain wall, so many metres long unless drawn, on the grid between
    /// cells. Windows and doors are cut in it with their own parts.
    Wall(Vec<(String, f32)>),
    /// A window so many metres wide, cut into a wall.
    Window(Vec<(String, f32, Panel)>),
    /// A doorway, cut into a wall.
    Door,
    /// Doors or shutters, fitted into a hole that fits them once its wall is built.
    Fit(Vec<Arc<Fitting>>),
    Items(Vec<Arc<Item>>),
    /// Take away an item, or close up a window or door.
    Remove,
    /// Pick an item up and put it down somewhere else.
    Move,
    /// A floor over a rectangle, drawn, on a storey, alone (D152).
    Floor,
    /// A flight of stairs or a ramp up a storey, turned by the wheel (D193).
    Flight(Vec<FlightShape>),
    /// A post a storey high, to carry the floor above (D193).
    Post,
    /// One cube of a shape, set against the face aimed at (D193), or a box of them the
    /// brush's size (D226).
    Block(Vec<(String, BlockShape)>),
    /// Change the built cubes aimed at, the brush's box of them, to the material in hand,
    /// built like everything else (D232).
    Replace,
    /// Carve the built cubes aimed at, the brush's box of them, out (D226).
    Carve,
}

pub struct Part {
    pub name: String,
    /// Where the build menu shows it.
    pub tab: Tab,
    pub kind: Kind,
}

/// The build menu's tabs (docs/building.md, the build menu).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum Tab {
    Claim,
    Structure,
    Openings,
    Crafting,
    Furniture,
    Storage,
    Fire,
    /// Not in the menu: modes the hammer has on keys of their own.
    Hidden,
}

impl Tab {
    /// The tabs the menu shows, in order: building by hand's first (D253).
    pub const SHOWN: [Tab; 7] = [
        Tab::Structure,
        Tab::Claim,
        Tab::Openings,
        Tab::Crafting,
        Tab::Furniture,
        Tab::Storage,
        Tab::Fire,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tab::Claim => "Claim",
            Tab::Structure => "Structure",
            Tab::Openings => "Openings",
            Tab::Crafting => "Crafting",
            Tab::Furniture => "Furniture",
            Tab::Storage => "Storage",
            Tab::Fire => "Light and fire",
            Tab::Hidden => "",
        }
    }
}

impl Part {
    pub fn variants(&self) -> usize {
        match &self.kind {
            Kind::Room(shapes) => shapes.len(),
            Kind::Roof(shapes) => shapes.len(),
            Kind::Window(shapes) => shapes.len(),
            Kind::Fit(fittings) => fittings.len(),
            Kind::Wall(lengths) => lengths.len(),
            Kind::Items(items) => items.len(),
            Kind::Flight(shapes) => shapes.len(),
            Kind::Block(shapes) => shapes.len(),
            Kind::Door
            | Kind::Remove
            | Kind::Move
            | Kind::Floor
            | Kind::Post
            | Kind::Replace
            | Kind::Carve => 1,
        }
    }

    /// The name of one of this part's variants, if it has more than one kind.
    pub fn variant_name(&self, variant: usize) -> Option<&str> {
        match &self.kind {
            Kind::Room(shapes) => Some(&shapes[variant].name),
            Kind::Roof(shapes) => Some(&shapes[variant].name),
            Kind::Window(shapes) => Some(&shapes[variant].0),
            Kind::Fit(fittings) => Some(&fittings[variant].name),
            Kind::Wall(lengths) => Some(&lengths[variant].0),
            Kind::Items(items) => Some(&items[variant].name),
            Kind::Flight(shapes) => Some(&shapes[variant].name),
            Kind::Block(shapes) => Some(&shapes[variant].0),
            Kind::Door
            | Kind::Remove
            | Kind::Move
            | Kind::Floor
            | Kind::Post
            | Kind::Replace
            | Kind::Carve => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builds() -> (Items, Builds) {
        let items = Items::default();
        let builds = Builds::read(&items, &Stations::read());
        (items, builds)
    }

    #[test]
    fn every_door_and_shutters_come_with_a_frame_in_a_material_there_is() {
        let (_, builds) = builds();
        let kits = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/data/kits");
        let doors = builds.fittings();
        for hang in [Hang::Door, Hang::Shutters] {
            assert!(doors.iter().any(|f| f.hang == hang), "no {hang:?}");
        }
        for door in doors {
            let frame = door
                .frame
                .as_deref()
                .unwrap_or_else(|| panic!("{} has no frame", door.name));
            assert!(
                kits.join(format!("{frame}.ron")).exists(),
                "{}'s frame is of {frame}, no kit",
                door.name
            );
        }
    }

    /// Reading the file checks every item and station it names is one there is.
    #[test]
    fn there_is_something_to_sit_on_and_a_bed_to_wake_in() {
        let (_, builds) = builds();
        let rests: Vec<Rest> = builds.items().filter_map(|item| item.rest).collect();
        assert!(rests.contains(&Rest::Sit));
        assert!(rests.contains(&Rest::Sleep));
        assert!(builds.items().any(|item| item.station.is_some()));
        assert!(builds.claim.grow.len() + 1 == builds.claim.across_m.len());
    }

    #[test]
    fn bigger_things_take_longer_and_coming_apart_is_quicker() {
        let (items, _) = builds();
        let stool = [(items.kind("Wood"), 1)];
        let bed = [(items.kind("Wood"), 3), (items.kind("Fibre"), 3)];
        assert!(build_seconds(&bed, 1) > build_seconds(&stool, 1));
        assert!(take_apart_seconds(&bed, 1) < build_seconds(&bed, 1));
        // Never instant, never endless.
        assert!(build_seconds(&[], 1) >= 0.5);
        assert!(build_seconds(&bed, 100) <= 3.0);
    }

    #[test]
    fn the_hammer_leads_with_building_by_hand() {
        // D253: the first part, in hand by default and on the first key, is a block, in the
        // first tab the menu shows; carving and changing material come before the room,
        // the wall and the roof.
        let (_, builds) = builds();
        let parts = builds.parts();
        assert!(matches!(parts[0].kind, Kind::Block(_)), "{}", parts[0].name);
        assert_eq!(parts[0].tab, Tab::SHOWN[0]);
        let place = |found: fn(&Kind) -> bool| {
            parts
                .iter()
                .position(|part| found(&part.kind))
                .expect("on the menu")
        };
        let by_hand =
            place(|k| matches!(k, Kind::Carve)).max(place(|k| matches!(k, Kind::Replace)));
        for convenience in [
            place(|k| matches!(k, Kind::Room(_))),
            place(|k| matches!(k, Kind::Wall(_))),
            place(|k| matches!(k, Kind::Roof(_))),
        ] {
            assert!(by_hand < convenience);
        }
    }

    #[test]
    fn every_part_has_a_variant_and_every_item_one_model() {
        let (_, builds) = builds();
        for part in builds.parts() {
            assert!(part.variants() >= 1, "{} has nothing to build", part.name);
            assert_eq!(
                part.variant_name(0).is_some(),
                part.variants() > 1 || matches!(part.kind, Kind::Items(_))
            );
        }
        let files: Vec<_> = builds.items().map(|item| item.file.clone()).collect();
        for file in &files {
            assert_eq!(files.iter().filter(|other| other == &file).count(), 1);
            assert!(builds.item_from(file).is_some());
        }
        let names: Vec<_> = builds.items().map(|item| item.name.clone()).collect();
        for name in &names {
            assert_eq!(names.iter().filter(|other| other == &name).count(), 1);
        }
    }
}
