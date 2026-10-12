//! What the builder can build: a short list of parts, each with one or more variants.
//!
//! A part is what you pick from the dial: a room, a window, a seat. A variant is a kind
//! of that part, cycled through once the part is picked: a chair, a stool. The list is
//! data, in `assets/data/builds.ron` (D136), and a stand-in. What can be built, what it
//! costs and how it is unlocked are design questions nobody has answered (docs/player.md).

use super::crafting::Station;
use super::inventory::Cost;
use serde::{Deserialize, Deserializer};
use std::sync::LazyLock;
use terra_build::{CELL_M, Panel};

const BUILDS: &str = include_str!("../../../../assets/data/builds.ron");

/// The file: what the pieces of a base cost, and the parts.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "'de: 'static"))]
struct BuildsFile {
    structure: Structure,
    parts: Vec<Part>,
}

/// What the pieces of a base cost: floor by every three square metres, interior wall by
/// every metre and a half, a window by the metre, and a door each.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "'de: 'static"))]
struct Structure {
    #[serde(deserialize_with = "leaked")]
    floor: Cost,
    #[serde(deserialize_with = "leaked")]
    wall: Cost,
    #[serde(deserialize_with = "leaked")]
    window: Cost,
    #[serde(deserialize_with = "leaked")]
    door: Cost,
}

/// A list from the file, kept for as long as the game runs, like the file itself.
fn leaked<'de, D, T>(from: D) -> Result<&'static [T], D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + 'static,
{
    Ok(Vec::<T>::deserialize(from)?.leak())
}

/// Windows from the file: a name, a width in metres, and the glass's sill and top.
fn windows<'de, D>(from: D) -> Result<&'static [(&'static str, f32, Panel)], D::Error>
where
    D: Deserializer<'de>,
    'de: 'static,
{
    let windows: Vec<(&'static str, f32, (f32, f32))> = Vec::deserialize(from)?;
    Ok(windows
        .into_iter()
        .map(|(name, width, (sill, top))| (name, width, Panel::Window { sill, top }))
        .collect::<Vec<_>>()
        .leak())
}

static BUILDS_FILE: LazyLock<BuildsFile> = LazyLock::new(|| {
    ron::from_str(BUILDS).unwrap_or_else(|problem| panic!("assets/data/builds.ron: {problem}"))
});

/// The parts, in the order the build menu lists them within each tab.
pub static PARTS: LazyLock<&'static [Part]> = LazyLock::new(|| &BUILDS_FILE.parts);

/// What floor costs, paid by `floor_units`.
pub fn floor() -> Cost {
    BUILDS_FILE.structure.floor
}

/// What interior wall costs, paid by `wall_units`.
pub fn wall() -> Cost {
    BUILDS_FILE.structure.wall
}

/// What a window costs, paid by `window_units`.
pub fn window() -> Cost {
    BUILDS_FILE.structure.window
}

/// What a door costs.
pub fn door() -> Cost {
    BUILDS_FILE.structure.door
}

/// How many times over `floor()` is paid for so many cells of floor: once for every three
/// square metres, or part of that.
pub fn floor_units(cells: usize) -> u32 {
    libm::ceilf(cells as f32 * CELL_M * CELL_M / 3.0) as u32
}

/// How many times over `wall()` is paid for so many cells of interior wall: once for every
/// metre and a half.
pub fn wall_units(cells: usize) -> u32 {
    libm::ceilf(cells as f32 * CELL_M / 1.5) as u32
}

/// How many times over `window()` is paid for a window so many cells wide: once a metre.
pub fn window_units(cells: usize) -> u32 {
    libm::ceilf(cells as f32 * CELL_M) as u32
}

/// How long something takes to build, from what it costs: half a second, and a little
/// more for every raw item in it, up to three seconds. A stand-in, like the costs.
pub fn build_seconds(cost: Cost, times: u32) -> f32 {
    let raw_items: u32 = cost.iter().map(|&(_, each)| each * times).sum();
    (0.5 + 0.12 * raw_items as f32).min(3.0)
}

/// Taking a thing apart is quicker than building it.
pub fn take_apart_seconds(cost: Cost, times: u32) -> f32 {
    build_seconds(cost, times) * 0.6
}

/// Something stood on a floor or on the ground. Its model is a file in
/// `assets/models/props`.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub name: &'static str,
    pub file: &'static str,
    /// Its longest dimension, in metres.
    pub size: f32,
    /// The raw items it is built from, and gives back when taken apart.
    #[serde(deserialize_with = "leaked")]
    pub cost: Cost,
    /// Flat things lie this far above the floor, so the two do not flicker.
    #[serde(default)]
    pub lift: f32,
    /// Whether it gives off light, and how.
    #[serde(default)]
    pub light: Light,
    /// The station it is, by its name in `assets/data/stations.ron`, if it is one.
    #[serde(default, rename = "station")]
    pub station_name: Option<&'static str>,
    /// Whether it is a claim stone, which claims the land around it (D94).
    #[serde(default)]
    pub claims: bool,
    /// What someone can rest on it for, if anything: sitting, or sleeping and waking.
    #[serde(default)]
    pub rest: Option<Rest>,
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

impl Item {
    /// The kind of station it is, if it is one.
    pub fn station(&self) -> Option<Station> {
        self.station_name.and_then(Station::named)
    }
}

/// A room's footprint in metres, east by south.
#[derive(Deserialize)]
pub struct RoomShape {
    pub name: &'static str,
    pub metres: [f32; 2],
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'static"))]
pub enum Kind {
    Room(#[serde(deserialize_with = "leaked")] &'static [RoomShape]),
    /// Push and pull the walls of what is already built.
    Reshape,
    /// A stretch of plain interior wall, so many metres long, on the grid between cells
    /// of the base. Windows and doors are cut in it with their own parts, as they are in
    /// an outside wall.
    Wall(#[serde(deserialize_with = "leaked")] &'static [(&'static str, f32)]),
    /// A window so many metres wide.
    Window(#[serde(deserialize_with = "windows")] &'static [(&'static str, f32, Panel)]),
    Door,
    Items(#[serde(deserialize_with = "leaked")] &'static [Item]),
    /// Take away an item, or close up a window or door.
    Remove,
    /// Pick an item up and put it down somewhere else.
    Move,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub name: &'static str,
    /// Where the build menu shows it.
    pub tab: Tab,
    pub kind: Kind,
}

/// The build menu's tabs (docs/construction.md, the build menu).
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
    /// The tabs the menu shows, in order.
    pub const SHOWN: [Tab; 7] = [
        Tab::Claim,
        Tab::Structure,
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
        match self.kind {
            Kind::Room(shapes) => shapes.len(),
            Kind::Window(shapes) => shapes.len(),
            Kind::Wall(lengths) => lengths.len(),
            Kind::Items(items) => items.len(),
            Kind::Reshape | Kind::Door | Kind::Remove | Kind::Move => 1,
        }
    }

    /// The name of one of this part's variants, if it has more than one kind.
    pub fn variant_name(&self, variant: usize) -> Option<&'static str> {
        match self.kind {
            Kind::Room(shapes) => Some(shapes[variant].name),
            Kind::Window(shapes) => Some(shapes[variant].0),
            Kind::Wall(lengths) => Some(lengths[variant].0),
            Kind::Items(items) => Some(items[variant].name),
            Kind::Reshape | Kind::Door | Kind::Remove | Kind::Move => None,
        }
    }
}

/// Every item any part offers.
pub fn items() -> impl Iterator<Item = &'static Item> {
    PARTS.iter().flat_map(|part| match part.kind {
        Kind::Items(items) => items.iter(),
        _ => [].iter(),
    })
}

/// The item whose model is a given file.
pub fn item_from(file: &str) -> Option<&'static Item> {
    items().find(|item| item.file == file)
}

/// The part and variant that an item is, for picking "another of those".
pub fn place_of(wanted: &Item) -> Option<(usize, usize)> {
    PARTS
        .iter()
        .enumerate()
        .find_map(|(part, entry)| match entry.kind {
            Kind::Items(items) => items
                .iter()
                .position(|item| item == wanted)
                .map(|variant| (part, variant)),
            _ => None,
        })
}

/// Where a part is in the list, for the parts that have their own key.
pub fn part_of(wanted: fn(&Kind) -> bool) -> usize {
    PARTS
        .iter()
        .position(|part| wanted(&part.kind))
        .expect("the catalogue has this part")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_is_something_to_sit_on_and_a_bed_to_wake_in() {
        let rests: Vec<Rest> = items().filter_map(|item| item.rest).collect();
        assert!(rests.contains(&Rest::Sit));
        assert!(rests.contains(&Rest::Sleep));
    }

    #[test]
    fn every_station_built_is_a_kind_of_station() {
        let named = PARTS.iter().filter_map(|part| match part.kind {
            Kind::Items(items) => Some(items),
            _ => None,
        });
        for item in named.flatten() {
            if let Some(name) = item.station_name {
                assert!(Station::named(name).is_some(), "{name} is not a station");
            }
        }
    }

    #[test]
    fn everything_costs_kinds_of_item_there_are() {
        let items = super::super::inventory::Items::default();
        let costs = [floor(), wall(), window(), door()].into_iter().chain(
            PARTS
                .iter()
                .filter_map(|part| match part.kind {
                    Kind::Items(items) => Some(items),
                    _ => None,
                })
                .flatten()
                .map(|item| item.cost),
        );
        for cost in costs {
            for (name, _) in cost {
                assert!(items.find(name).is_some(), "{name} is not a kind of item");
            }
        }
    }

    #[test]
    fn bigger_things_take_longer_and_coming_apart_is_quicker() {
        const STOOL: Cost = &[("Wood", 1)];
        const BED: Cost = &[("Wood", 3), ("Fibre", 3)];
        assert!(build_seconds(BED, 1) > build_seconds(STOOL, 1));
        assert!(take_apart_seconds(BED, 1) < build_seconds(BED, 1));
        // Never instant, never endless.
        assert!(build_seconds(&[], 1) >= 0.5);
        assert!(build_seconds(BED, 100) <= 3.0);
    }

    #[test]
    fn every_part_has_a_variant_and_every_item_one_model() {
        for part in PARTS.iter() {
            assert!(part.variants() >= 1, "{} has nothing to build", part.name);
            assert_eq!(
                part.variant_name(0).is_some(),
                part.variants() > 1 || matches!(part.kind, Kind::Items(_))
            );
        }
        let files: Vec<_> = items().map(|item| item.file).collect();
        for file in &files {
            assert_eq!(files.iter().filter(|other| other == &file).count(), 1);
            assert!(item_from(file).is_some());
        }
    }
}
