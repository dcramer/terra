//! Items and containers (docs/items.md).
//!
//! The baseline is Subnautica's (D84): a backpack is a [`Grid`] in which each item fills a
//! rectangle of cells and nothing stacks; [`Equipment`] is a slot per kind of gear; the
//! [`ActionBar`] points at items in the backpack rather than holding them.
//!
//! Everything here is plain data and functions, so every player computes the same result
//! (D28). In the game a container is a component on whatever holds it, and the items in it
//! are data in that component, not entities (D83). Every change is a function that either
//! applies in full or is refused and changes nothing, so the server can check a player's
//! request with the same code the player used to predict it (D51).
//!
//! Moving an item never makes or loses one. Only [`IdSource`] makes items, and only taking
//! them to spend ([`Grid::take`]) removes them for good.

mod bar;
mod catalogue;
mod equipment;
mod grid;

pub use bar::ActionBar;
pub use catalogue::{Catalogue, GearSlot, KindDef};
pub use equipment::{Equipment, equip, unequip};
pub use grid::{Cell, Grid, Placed, Size, Stack, transfer};

/// A kind of item: an index into the [`Catalogue`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemKind(pub u16);

/// One item's identity. The action bar points at items by it, and the server names which
/// item moved by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub u64);

/// One item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Item {
    pub id: ItemId,
    pub kind: ItemKind,
}

/// Hands out item identities. The server owns the one source for a world, so no two items
/// ever share an id.
#[derive(Clone, Debug, Default)]
pub struct IdSource {
    next: u64,
}

impl IdSource {
    /// Continue from a saved world, after its highest id.
    pub fn after(last: ItemId) -> Self {
        Self { next: last.0 + 1 }
    }

    /// Make a new item of a kind.
    pub fn make(&mut self, kind: ItemKind) -> Item {
        let id = ItemId(self.next);
        self.next += 1;
        Item { id, kind }
    }
}

/// Why a change was refused. A refused change leaves everything as it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The item is not where it was said to be.
    NotFound,
    /// The kind is not in the catalogue.
    UnknownKind,
    /// There is no room for it.
    NoRoom,
    /// It is not gear, or not gear for that slot.
    NotGear,
    /// That slot does not exist on this equipment.
    NoSuchSlot,
    /// There are not enough of what was asked for.
    NotEnough,
    /// The backpack's contents would not fit in the new size.
    WouldNotFit,
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_gen::Seed;

    /// Random moves between a backpack, a locker and what is worn never make or lose an
    /// item, never overlap two, and never leave one outside its grid.
    #[test]
    fn moving_never_makes_or_loses_an_item() {
        const PACK: GearSlot = GearSlot(0);
        const TANK: GearSlot = GearSlot(1);
        const BARE: Size = Size { w: 3, h: 3 };
        let mut catalogue = Catalogue::default();
        let kinds = [
            catalogue.add(KindDef::new("Ore")),
            catalogue.add(KindDef::new("Plank").size(2, 1)),
            catalogue.add(KindDef::new("Crate").size(2, 2)),
            catalogue.add(KindDef::new("Tank").size(1, 2).gear(TANK)),
            catalogue.add(KindDef::new("Pack").size(2, 2).gear(PACK).backpack(5, 4)),
        ];
        let mut rng = Seed(7).derive("items").rng();
        let mut ids = IdSource::default();
        let mut backpack = Grid::new(BARE.w, BARE.h);
        let mut locker = Grid::new(6, 6);
        let mut gear = Equipment::new(&[PACK, TANK]);
        let mut made = Vec::new();
        while made.len() < 30 {
            let item = ids.make(kinds[rng.below(kinds.len() as u32) as usize]);
            if locker.insert(&catalogue, item, None).is_ok() {
                made.push(item.id);
            } else {
                break;
            }
        }

        let all = |backpack: &Grid, locker: &Grid, gear: &Equipment| {
            let mut held: Vec<ItemId> = backpack
                .iter()
                .chain(locker.iter())
                .map(|placed| placed.item.id)
                .chain(gear.iter().filter_map(|(_, item)| item).map(|item| item.id))
                .collect();
            held.sort();
            held
        };
        let mut expected = made.clone();
        expected.sort();

        for _ in 0..5000 {
            let pick = |grid: &Grid, roll: u32| {
                let index = roll as usize % grid.len().max(1);
                grid.iter().nth(index).map(|placed| placed.item.id)
            };
            let cell = Cell {
                x: rng.below(6) as u8,
                y: rng.below(6) as u8,
            };
            let at = if rng.below(2) == 0 { Some(cell) } else { None };
            let roll = rng.below(1000);
            let _ = match rng.below(6) {
                0 => pick(&locker, roll)
                    .map(|id| transfer(&catalogue, &mut locker, &mut backpack, id, at).map(drop)),
                1 => pick(&backpack, roll)
                    .map(|id| transfer(&catalogue, &mut backpack, &mut locker, id, at).map(drop)),
                2 => pick(&backpack, roll).map(|id| backpack.shift(id, cell)),
                3 => pick(&backpack, roll)
                    .map(|id| equip(&catalogue, &mut backpack, &mut gear, BARE, id)),
                4 => Some(
                    unequip(
                        &catalogue,
                        &mut backpack,
                        &mut gear,
                        BARE,
                        if rng.below(2) == 0 { PACK } else { TANK },
                        at,
                    )
                    .map(drop),
                ),
                _ => pick(&locker, roll).map(|id| locker.shift(id, cell)),
            };

            assert_eq!(all(&backpack, &locker, &gear), expected);
            assert_eq!(
                backpack.size(),
                gear.backpack_size(&catalogue, BARE),
                "the backpack is the size what is worn makes it"
            );
            for grid in [&backpack, &locker] {
                for placed in grid.iter() {
                    assert!(grid.fits(placed.at, placed.size, Some(placed.item.id)));
                }
            }
        }
    }
}
