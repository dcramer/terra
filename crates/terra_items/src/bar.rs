use crate::{Grid, Item, ItemId};

/// The action bar (D66): a few slots that point at items in the backpack. The item stays
/// in the backpack; whatever the picked slot points at is in hand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionBar {
    slots: Vec<Option<ItemId>>,
    picked: usize,
}

impl ActionBar {
    pub fn new(slots: usize) -> Self {
        Self {
            slots: vec![None; slots],
            picked: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn picked(&self) -> usize {
        self.picked
    }

    pub fn pick(&mut self, slot: usize) {
        if slot < self.slots.len() {
            self.picked = slot;
        }
    }

    pub fn get(&self, slot: usize) -> Option<ItemId> {
        self.slots.get(slot).copied().flatten()
    }

    /// Point a slot at an item, or clear it. An item is on the bar at most once, so
    /// putting it in a second slot takes it out of the first.
    pub fn set(&mut self, slot: usize, id: Option<ItemId>) {
        if slot >= self.slots.len() {
            return;
        }
        if let Some(id) = id {
            for each in &mut self.slots {
                if *each == Some(id) {
                    *each = None;
                }
            }
        }
        self.slots[slot] = id;
    }

    /// The item in hand, if the picked slot points at something still in the backpack.
    pub fn in_hand(&self, backpack: &Grid) -> Option<Item> {
        let id = self.get(self.picked)?;
        backpack.get(id).map(|placed| placed.item)
    }

    /// Empty every slot whose item has left the backpack, as Subnautica does. Run after
    /// anything leaves it.
    pub fn tidy(&mut self, backpack: &Grid) {
        for slot in &mut self.slots {
            if slot.is_some_and(|id| backpack.get(id).is_none()) {
                *slot = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Catalogue, IdSource, KindDef};

    #[test]
    fn the_bar_points_into_the_backpack() {
        let mut catalogue = Catalogue::default();
        let cutter = catalogue.add(KindDef::new("Cutter"));
        let mut ids = IdSource::default();
        let mut backpack = Grid::new(4, 4);
        let mut locker = Grid::new(4, 4);
        let cutter = ids.make(cutter);
        backpack.insert(&catalogue, cutter, None).unwrap();

        let mut bar = ActionBar::new(5);
        bar.set(0, Some(cutter.id));
        bar.set(2, Some(cutter.id));
        assert_eq!(bar.get(0), None, "an item is on the bar once");
        bar.pick(2);
        assert_eq!(bar.in_hand(&backpack), Some(cutter));
        assert_eq!(backpack.len(), 1, "the item stays in the backpack");

        crate::transfer(&catalogue, &mut backpack, &mut locker, cutter.id, None).unwrap();
        assert_eq!(bar.in_hand(&backpack), None);
        bar.tidy(&backpack);
        assert_eq!(bar.get(2), None);
    }
}
