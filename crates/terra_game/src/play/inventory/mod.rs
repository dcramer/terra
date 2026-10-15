//! What the player carries, on the rules in `terra_items` (docs/items.md, D84): a
//! backpack that is a grid where each item fills its cells and nothing stacks, a worn
//! pack that sets the backpack's size, and an action bar that points at items in the
//! backpack. Whatever the picked slot points at is in hand.
//!
//! The containers are components on the player (D83). Raw items are what things are
//! built from: the builder spends them, and taking a thing apart gives them back, if
//! there is room for them.
//!
//! The content is a stand-in for a design nobody has made (docs/building.md, D55):
//! the list of raw items, what you start with, what things cost, the sizes, and that
//! everything is refunded in full.
//!
//! The parts are here: `items` is the registry of kinds of item, `rules` what can be
//! done with what is carried, and `belongings` the system parameter that does it for a
//! player, telling what came in, broke or did not fit.

mod belongings;
mod items;
mod rules;
#[cfg(test)]
mod tests;

pub use super::kinds::Cut;
pub use belongings::{Belongings, Inventory, Spilled};
use bevy::prelude::*;
pub use items::Items;
pub use rules::*;
use terra_items::{ActionBar, Equipment, GearSlot, Grid, ItemKind, Size};

/// What one of something costs: so many of each kind of item. A data file names the items;
/// they are resolved to kinds as it is read (`Items::cost`).
pub type Cost = [(ItemKind, u32)];

/// A cost as a player would read it: "2 Wood, 3 Fibre".
pub fn describe(items: &Items, cost: &Cost, times: u32) -> String {
    cost.iter()
        .map(|&(kind, each)| format!("{} {}", each * times, items.name(kind)))
        .collect::<Vec<_>>()
        .join(", ")
}

pub use super::flora_kinds::Cuttable;

/// How many slots the bar has: eight, as Valheim's has. A placeholder (D84).
pub const BAR_SLOTS: usize = 8;

/// The slots of the character sheet, by the names the items file wears things in: some of
/// D64's standard slots so far, a pack, what is worn on the head, body and legs, and a belt
/// for a quiver (D118).
pub const SLOTS: [(GearSlot, &str); 5] = [
    (GearSlot(0), "Pack"),
    (GearSlot(1), "Head"),
    (GearSlot(2), "Body"),
    (GearSlot(3), "Belt"),
    (GearSlot(4), "Legs"),
];
#[cfg(test)]
const PACK: GearSlot = SLOTS[0].0;

/// The slot of the character sheet called a name.
fn slot_named(name: &str) -> GearSlot {
    SLOTS
        .iter()
        .find(|(_, slot)| *slot == name)
        .map(|(slot, _)| *slot)
        .unwrap_or_else(|| panic!("assets/data/items.ron: there is no slot called {name}"))
}

/// What a recipe makes: so many of a kind of item.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Made {
    pub kind: ItemKind,
    pub count: u32,
}

impl Made {
    pub fn name(self, items: &Items) -> String {
        match self.count {
            1 => items.name(self.kind).to_string(),
            count => format!("{count} {}", items.name(self.kind)),
        }
    }
}

/// The backpack with no pack worn: about Subnautica 2's twenty cells. How big the first
/// backpack is is open (docs/items.md).
const BARE: Size = Size { w: 5, h: 4 };

/// The backpack: a grid of cells.
#[derive(Component, Clone, Debug)]
pub struct Backpack(pub Grid);

/// What is worn.
#[derive(Component, Clone, Debug)]
pub struct Gear(pub Equipment);

/// The action bar.
#[derive(Component, Clone, Debug)]
pub struct Bar(pub ActionBar);

/// Whether the inventory is open on screen.
#[derive(Resource, Default)]
pub struct InventoryScreen {
    pub open: bool,
    /// The station being used, if the screen was opened at one.
    pub at: Option<(Entity, super::crafting::Station)>,
    /// The storage open beside the backpack, if it was opened at some (`storage`).
    pub storage: Option<Entity>,
}

/// Something the player carries wore out (D99): what kind it is.
#[derive(Message, Clone, Copy, Debug)]
pub struct Broke {
    pub player: Entity,
    pub kind: ItemKind,
}

/// Something came into what a player carries: so many of a kind of item. Written by
/// [`Inventory`] whatever brought it, so whatever shows it needs to know nothing else.
#[derive(Message, Clone, Copy, Debug)]
pub struct Gained {
    pub player: Entity,
    pub kind: ItemKind,
    pub count: u32,
}
