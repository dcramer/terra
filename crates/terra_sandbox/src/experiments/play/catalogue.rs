//! What the builder can build: a short list of parts, each with one or more variants.
//!
//! A part is what you pick from the dial: a room, a window, a seat. A variant is a kind
//! of that part, cycled through once the part is picked: a couch, a chair, a stool. The
//! list is a stand-in. What can be built, what it costs and how it is unlocked are design
//! questions nobody has answered (docs/player.md).

use terra_build::Panel;

/// Something stood on a floor or on the ground. Its model is a file in
/// `assets/models/props`.
#[derive(Debug, PartialEq)]
pub struct Item {
    pub name: &'static str,
    pub file: &'static str,
    /// Its longest dimension, in metres.
    pub size: f32,
    /// Flat things lie this far above the floor, so the two do not flicker.
    pub lift: f32,
    /// Whether it gives off light, and how.
    pub light: Light,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Light {
    None,
    Lamp,
    Fire,
}

const fn item(name: &'static str, file: &'static str, size: f32) -> Item {
    Item {
        name,
        file,
        size,
        lift: 0.0,
        light: Light::None,
    }
}

const fn rug(name: &'static str, file: &'static str, size: f32) -> Item {
    Item {
        lift: 0.015,
        ..item(name, file, size)
    }
}

const fn lit(light: Light, item: Item) -> Item {
    Item { light, ..item }
}

/// A room's footprint in tiles, east by south.
pub struct RoomShape {
    pub name: &'static str,
    pub tiles: [i32; 2],
}

pub enum Kind {
    Room(&'static [RoomShape]),
    /// Push and pull the walls of what is already built.
    Reshape,
    /// A plain interior wall, on the edge between two tiles of the base. Windows and
    /// doors are cut in it with their own parts, as they are in an outside wall.
    Wall,
    Window(&'static [(&'static str, Panel)]),
    Door,
    Items(&'static [Item]),
    /// Take away an item, or close up a window or door.
    Remove,
    /// Pick an item up and put it down somewhere else.
    Move,
}

pub struct Part {
    pub name: &'static str,
    pub kind: Kind,
}

impl Part {
    pub fn variants(&self) -> usize {
        match self.kind {
            Kind::Room(shapes) => shapes.len(),
            Kind::Window(shapes) => shapes.len(),
            Kind::Items(items) => items.len(),
            Kind::Reshape | Kind::Wall | Kind::Door | Kind::Remove | Kind::Move => 1,
        }
    }

    /// The name of one of this part's variants, if it has more than one kind.
    pub fn variant_name(&self, variant: usize) -> Option<&'static str> {
        match self.kind {
            Kind::Room(shapes) => Some(shapes[variant].name),
            Kind::Window(shapes) => Some(shapes[variant].0),
            Kind::Items(items) => Some(items[variant].name),
            Kind::Reshape | Kind::Wall | Kind::Door | Kind::Remove | Kind::Move => None,
        }
    }
}

/// The parts, in the order they go round the dial, clockwise from the top. The first
/// ten are also on the number keys.
pub static PARTS: [Part; 15] = [
    Part {
        name: "Room",
        kind: Kind::Room(&[
            RoomShape {
                name: "Three by three",
                tiles: [3, 3],
            },
            RoomShape {
                name: "Two by two",
                tiles: [2, 2],
            },
            RoomShape {
                name: "Four by four",
                tiles: [4, 4],
            },
            RoomShape {
                name: "Corridor",
                tiles: [1, 3],
            },
        ]),
    },
    Part {
        name: "Reshape",
        kind: Kind::Reshape,
    },
    Part {
        name: "Wall",
        kind: Kind::Wall,
    },
    Part {
        name: "Window",
        kind: Kind::Window(&[
            ("Framed", Panel::WINDOW),
            ("Wide", Panel::WIDE_WINDOW),
            ("Tall", Panel::TALL_WINDOW),
        ]),
    },
    Part {
        name: "Door",
        kind: Kind::Door,
    },
    Part {
        name: "Remove",
        kind: Kind::Remove,
    },
    Part {
        name: "Move",
        kind: Kind::Move,
    },
    Part {
        name: "Seat",
        kind: Kind::Items(&[
            item("Couch", "Couch_Medium1", 2.2),
            item("Chair", "Chair_1", 0.95),
            item("Stool", "Stool", 0.5),
        ]),
    },
    Part {
        name: "Table",
        kind: Kind::Items(&[
            item("Round table", "Table_RoundSmall", 1.1),
            item("Night stand", "NightStand_1", 0.55),
        ]),
    },
    Part {
        name: "Bed",
        kind: Kind::Items(&[item("Single bed", "Bed_Single", 2.15)]),
    },
    Part {
        name: "Storage",
        kind: Kind::Items(&[
            item("Bookshelf", "Bookshelf", 2.0),
            item("Tall shelf", "Props_Shelf_Tall", 2.0),
            item("Crate", "Props_Crate", 0.8),
            item("Long crate", "Props_CrateLong", 1.2),
            item("Container", "Props_ContainerFull", 1.3),
        ]),
    },
    Part {
        name: "Kitchen",
        kind: Kind::Items(&[
            item("Fridge", "Kitchen_Fridge", 1.85),
            item("Sink", "Kitchen_Sink", 1.08),
            item("Drawers", "Kitchen_2Drawers", 0.92),
        ]),
    },
    Part {
        name: "Decor",
        kind: Kind::Items(&[
            lit(Light::Lamp, item("Floor lamp", "Light_Floor1", 1.6)),
            item("Leafy plant", "Houseplant_7", 1.1),
            item("Cactus", "Houseplant_3", 0.9),
            item("Small plant", "Houseplant_1", 0.45),
            rug("Round rug", "Carpet_Round", 3.2),
            rug("Long rug", "Carpet_1", 3.0),
        ]),
    },
    Part {
        name: "Kit",
        kind: Kind::Items(&[
            item("Computer", "Props_Computer", 1.8),
            item("Radio", "Radio", 0.4),
            item("Gas tank", "PropaneTank", 1.3),
            item("Fuel can", "GasCan", 0.45),
            item("Shovel", "Shovel", 1.3),
        ]),
    },
    Part {
        name: "Fire",
        kind: Kind::Items(&[
            lit(Light::Fire, item("Campfire", "Bonfire_Fire", 1.3)),
            item("Log", "WoodLog", 1.7),
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
