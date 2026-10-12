//! What the builder can build: a short list of parts, each with one or more variants.
//!
//! A part is what you pick from the dial: a room, a window, a seat. A variant is a kind
//! of that part, cycled through once the part is picked: a couch, a chair, a stool. The
//! list is a stand-in. What can be built, what it costs and how it is unlocked are design
//! questions nobody has answered (docs/player.md).

use super::crafting::Station;
use super::inventory::Cost;
use terra_build::{CELL_M, Panel};

/// What the pieces of a base cost. Sizes are measured, not counted in cells, so the
/// prices do not change if the grid does.
///
/// Floor is paid for by area, however a room comes to have it: built, extended, or by
/// pushing a wall out.
pub const FLOOR: Cost = &[("Wood", 1)];
pub const WALL: Cost = &[("Wood", 1)];
pub const WINDOW: Cost = &[("Wood", 1), ("Fibre", 1)];
pub const DOOR: Cost = &[("Wood", 2)];

/// How many times over `FLOOR` is paid for so many cells of floor: once for every three
/// square metres, or part of that.
pub fn floor_units(cells: usize) -> u32 {
    libm::ceilf(cells as f32 * CELL_M * CELL_M / 3.0) as u32
}

/// How many times over `WALL` is paid for so many cells of interior wall: once for every
/// metre and a half.
pub fn wall_units(cells: usize) -> u32 {
    libm::ceilf(cells as f32 * CELL_M / 1.5) as u32
}

/// How many times over `WINDOW` is paid for a window so many cells wide: once a metre.
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
#[derive(Debug, PartialEq)]
pub struct Item {
    pub name: &'static str,
    pub file: &'static str,
    /// Its longest dimension, in metres.
    pub size: f32,
    /// The raw items it is built from, and gives back when taken apart.
    pub cost: Cost,
    /// Flat things lie this far above the floor, so the two do not flicker.
    pub lift: f32,
    /// Whether it gives off light, and how.
    pub light: Light,
    /// The station it is, by its name in `assets/data/stations.ron`, if it is one.
    pub station_name: Option<&'static str>,
    /// Whether it is a claim stone, which claims the land around it (D94).
    pub claims: bool,
    /// What someone can rest on it for, if anything: sitting, or sleeping and waking.
    pub rest: Option<Rest>,
}

/// What something is to rest on.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Rest {
    /// People sit on it (`people`).
    Sit,
    /// A bed: you wake in it after going down, if it is in your claim (`death`).
    Sleep,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Light {
    None,
    Fire,
}

const fn item(name: &'static str, file: &'static str, size: f32, cost: Cost) -> Item {
    Item {
        name,
        file,
        size,
        cost,
        lift: 0.0,
        light: Light::None,
        station_name: None,
        claims: false,
        rest: None,
    }
}

const fn rug(name: &'static str, file: &'static str, size: f32, cost: Cost) -> Item {
    Item {
        lift: 0.015,
        ..item(name, file, size, cost)
    }
}

const fn rest(rest: Rest, item: Item) -> Item {
    Item {
        rest: Some(rest),
        ..item
    }
}

const fn lit(light: Light, item: Item) -> Item {
    Item { light, ..item }
}

const fn station(name: &'static str, item: Item) -> Item {
    Item {
        station_name: Some(name),
        ..item
    }
}

impl Item {
    /// The kind of station it is, if it is one.
    pub fn station(&self) -> Option<Station> {
        self.station_name.and_then(Station::named)
    }
}

/// A room's footprint in metres, east by south.
pub struct RoomShape {
    pub name: &'static str,
    pub metres: [f32; 2],
}

pub enum Kind {
    Room(&'static [RoomShape]),
    /// Push and pull the walls of what is already built.
    Reshape,
    /// A stretch of plain interior wall, so many metres long, on the grid between cells
    /// of the base. Windows and doors are cut in it with their own parts, as they are in
    /// an outside wall.
    Wall(&'static [(&'static str, f32)]),
    /// A window so many metres wide.
    Window(&'static [(&'static str, f32, Panel)]),
    Door,
    Items(&'static [Item]),
    /// Take away an item, or close up a window or door.
    Remove,
    /// Pick an item up and put it down somewhere else.
    Move,
}

pub struct Part {
    pub name: &'static str,
    /// Where the build menu shows it.
    pub tab: Tab,
    pub kind: Kind,
}

/// The build menu's tabs (docs/construction.md, the build menu).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

/// The parts, in the order the build menu lists them within each tab.
pub static PARTS: [Part; 19] = [
    Part {
        name: "Claim stone",
        tab: Tab::Claim,
        kind: Kind::Items(&[Item {
            claims: true,
            ..item("Claim stone", "../nature/Rock_Moss_2", 1.3, &[("Stone", 2)])
        }]),
    },
    Part {
        name: "Workbench",
        tab: Tab::Crafting,
        kind: Kind::Items(&[station(
            "Workbench",
            item(
                "Workbench",
                "Table_Small",
                1.7,
                &[("Wood", 4), ("Stone", 2)],
            ),
        )]),
    },
    Part {
        name: "Smelter",
        tab: Tab::Crafting,
        kind: Kind::Items(&[station(
            "Smelter",
            item(
                "Smelter",
                "shaped:smelter",
                2.0,
                &[("Stone", 10), ("Wood", 2)],
            ),
        )]),
    },
    Part {
        name: "Forge",
        tab: Tab::Crafting,
        kind: Kind::Items(&[station(
            "Forge",
            item("Forge", "shaped:forge", 2.6, &[("Wood", 4), ("Stone", 8)]),
        )]),
    },
    Part {
        name: "Room",
        tab: Tab::Structure,
        kind: Kind::Room(&[
            RoomShape {
                name: "4.5 by 4.5 m",
                metres: [4.5, 4.5],
            },
            RoomShape {
                name: "3 by 3 m",
                metres: [3.0, 3.0],
            },
            RoomShape {
                name: "6 by 6 m",
                metres: [6.0, 6.0],
            },
            RoomShape {
                name: "Corridor, 1.5 by 4.5 m",
                metres: [1.5, 4.5],
            },
        ]),
    },
    Part {
        name: "Reshape",
        tab: Tab::Structure,
        kind: Kind::Reshape,
    },
    Part {
        name: "Wall",
        tab: Tab::Structure,
        kind: Kind::Wall(&[("3 m", 3.0), ("1.5 m", 1.5), ("0.5 m", 0.5)]),
    },
    Part {
        name: "Window",
        tab: Tab::Openings,
        kind: Kind::Window(&[
            ("1.5 m", 1.5, Panel::WINDOW),
            ("3 m", 3.0, Panel::WINDOW),
            ("0.5 m", 0.5, Panel::WINDOW),
            ("Tall, 1.5 m", 1.5, Panel::TALL_WINDOW),
        ]),
    },
    Part {
        name: "Door",
        tab: Tab::Openings,
        kind: Kind::Door,
    },
    // Seen in the world but not yet buildable: a village's well and stall. How they are
    // learned is open (D68, D108).
    Part {
        name: "Well",
        tab: Tab::Hidden,
        kind: Kind::Items(&[item("Well", "../village/Well", 2.4, &[("Stone", 12)])]),
    },
    Part {
        name: "Market stall",
        tab: Tab::Hidden,
        kind: Kind::Items(&[item(
            "Market stall",
            "../village/MarketStand_1",
            3.0,
            &[("Wood", 8), ("Fibre", 4)],
        )]),
    },
    Part {
        name: "Remove",
        tab: Tab::Hidden,
        kind: Kind::Remove,
    },
    Part {
        name: "Move",
        tab: Tab::Hidden,
        kind: Kind::Move,
    },
    Part {
        name: "Seat",
        tab: Tab::Furniture,
        // Modern pieces from the free packs, such as a couch, kitchen drawers, sci-fi crates
        // and potted houseplants, are left out until the art direction settles; they do
        // not belong in game design 001's world.
        kind: Kind::Items(&[
            rest(Rest::Sit, item("Chair", "Chair_1", 0.95, &[("Wood", 2)])),
            rest(Rest::Sit, item("Stool", "Stool", 0.5, &[("Wood", 1)])),
        ]),
    },
    Part {
        name: "Table",
        tab: Tab::Furniture,
        kind: Kind::Items(&[
            item("Round table", "Table_RoundSmall", 1.1, &[("Wood", 3)]),
            item("Night stand", "NightStand_1", 0.55, &[("Wood", 2)]),
        ]),
    },
    Part {
        name: "Bed",
        tab: Tab::Furniture,
        kind: Kind::Items(&[rest(
            Rest::Sleep,
            item(
                "Single bed",
                "Bed_Single",
                2.15,
                &[("Wood", 3), ("Fibre", 3)],
            ),
        )]),
    },
    Part {
        name: "Storage",
        tab: Tab::Storage,
        kind: Kind::Items(&[item("Bookshelf", "Bookshelf", 2.0, &[("Wood", 4)])]),
    },
    Part {
        name: "Decor",
        tab: Tab::Furniture,
        kind: Kind::Items(&[
            rug("Round rug", "Carpet_Round", 3.2, &[("Hide", 2)]),
            rug("Long rug", "Carpet_1", 3.0, &[("Hide", 2)]),
            item("Shovel", "Shovel", 1.3, &[("Wood", 1), ("Iron", 1)]),
        ]),
    },
    Part {
        name: "Fire",
        tab: Tab::Fire,
        kind: Kind::Items(&[
            station(
                "Campfire",
                lit(
                    Light::Fire,
                    item(
                        "Campfire",
                        "Bonfire_Fire",
                        1.3,
                        &[("Wood", 3), ("Stone", 2)],
                    ),
                ),
            ),
            item("Log", "WoodLog", 1.7, &[("Wood", 2)]),
        ]),
    },
];

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
        let costs = [FLOOR, WALL, WINDOW, DOOR].into_iter().chain(
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
        for part in &PARTS {
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
