use crate::{Catalogue, Cell, GearSlot, Grid, Item, ItemId, Refused, Size};

/// What is worn: one item per slot, each slot taking only gear of its kind (D64).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Equipment {
    slots: Vec<(GearSlot, Option<Item>)>,
}

impl Equipment {
    pub fn new(slots: &[GearSlot]) -> Self {
        Self {
            slots: slots.iter().map(|&slot| (slot, None)).collect(),
        }
    }

    pub fn worn(&self, slot: GearSlot) -> Option<Item> {
        self.slots
            .iter()
            .find(|(each, _)| *each == slot)
            .and_then(|(_, item)| *item)
    }

    pub fn iter(&self) -> impl Iterator<Item = (GearSlot, Option<Item>)> + '_ {
        self.slots.iter().copied()
    }

    /// How big the backpack is with what is worn: the largest any worn gear gives, or
    /// `bare` with none (D65).
    pub fn backpack_size(&self, catalogue: &Catalogue, bare: Size) -> Size {
        self.slots
            .iter()
            .filter_map(|(_, item)| *item)
            .filter_map(|item| catalogue.get(item.kind).ok()?.backpack)
            .fold(bare, |best, size| {
                if size.w as u16 * size.h as u16 > best.w as u16 * best.h as u16 {
                    size
                } else {
                    best
                }
            })
    }

    fn slot_mut(&mut self, slot: GearSlot) -> Result<&mut Option<Item>, Refused> {
        self.slots
            .iter_mut()
            .find(|(each, _)| *each == slot)
            .map(|(_, item)| item)
            .ok_or(Refused::NoSuchSlot)
    }
}

/// Wear an item from the backpack. Whatever was in its slot goes into the backpack in its
/// place. The backpack is resized for what is now worn, and the whole change is refused
/// if its contents would not fit.
pub fn equip(
    catalogue: &Catalogue,
    backpack: &mut Grid,
    gear: &mut Equipment,
    bare: Size,
    id: ItemId,
) -> Result<(), Refused> {
    let item = backpack.get(id).ok_or(Refused::NotFound)?.item;
    let slot = catalogue.get(item.kind)?.gear.ok_or(Refused::NotGear)?;
    gear.slot_mut(slot)?;

    let (backpack_before, gear_before) = (backpack.clone(), gear.clone());
    let result = (|| {
        backpack.remove(id)?;
        let old = gear.slot_mut(slot)?.replace(item);
        if let Some(old) = old {
            backpack.insert(catalogue, old, None)?;
        }
        backpack.resize(gear.backpack_size(catalogue, bare))
    })();
    if result.is_err() {
        *backpack = backpack_before;
        *gear = gear_before;
    }
    result
}

/// Take off what is in a slot and put it in the backpack, at a cell or wherever it first
/// fits. Refused if the backpack it leaves behind could not hold what is in it.
pub fn unequip(
    catalogue: &Catalogue,
    backpack: &mut Grid,
    gear: &mut Equipment,
    bare: Size,
    slot: GearSlot,
    at: Option<Cell>,
) -> Result<Cell, Refused> {
    let (backpack_before, gear_before) = (backpack.clone(), gear.clone());
    let result = (|| {
        let item = gear.slot_mut(slot)?.take().ok_or(Refused::NotFound)?;
        let cell = backpack.insert(catalogue, item, at)?;
        backpack.resize(gear.backpack_size(catalogue, bare))?;
        Ok(cell)
    })();
    if result.is_err() {
        *backpack = backpack_before;
        *gear = gear_before;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdSource, KindDef};

    const PACK: GearSlot = GearSlot(0);
    const TANK: GearSlot = GearSlot(1);
    const BARE: Size = Size { w: 2, h: 2 };

    #[test]
    fn a_backpack_makes_the_backpack_bigger_and_taking_it_off_needs_room() {
        let mut catalogue = Catalogue::default();
        let ore = catalogue.add(KindDef::new("Ore"));
        let pack = catalogue.add(KindDef::new("Pack").size(2, 2).gear(PACK).backpack(4, 4));
        let mut ids = IdSource::default();
        let mut backpack = Grid::new(BARE.w, BARE.h);
        let mut gear = Equipment::new(&[PACK, TANK]);

        let pack = ids.make(pack);
        backpack.insert(&catalogue, pack, None).unwrap();
        equip(&catalogue, &mut backpack, &mut gear, BARE, pack.id).unwrap();
        assert_eq!(gear.worn(PACK), Some(pack));
        assert_eq!(backpack.size(), Size { w: 4, h: 4 });

        for _ in 0..16 {
            backpack.insert(&catalogue, ids.make(ore), None).unwrap();
        }
        let (backpack_before, gear_before) = (backpack.clone(), gear.clone());
        assert_eq!(
            unequip(&catalogue, &mut backpack, &mut gear, BARE, PACK, None),
            Err(Refused::NoRoom)
        );
        assert_eq!((&backpack, &gear), (&backpack_before, &gear_before));

        backpack.take(&[(ore, 13)]).unwrap();
        assert_eq!(
            unequip(&catalogue, &mut backpack, &mut gear, BARE, PACK, None),
            Err(Refused::WouldNotFit),
            "the pack fits in the big backpack, but three ores and the pack do not fit bare"
        );
        backpack.take(&[(ore, 3)]).unwrap();
        unequip(&catalogue, &mut backpack, &mut gear, BARE, PACK, None).unwrap();
        assert_eq!(gear.worn(PACK), None);
        assert_eq!(backpack.size(), BARE);
    }

    #[test]
    fn only_gear_goes_in_its_own_slot_and_swaps_with_what_was_there() {
        let mut catalogue = Catalogue::default();
        let ore = catalogue.add(KindDef::new("Ore"));
        let tank = catalogue.add(KindDef::new("Tank").gear(TANK));
        let mut ids = IdSource::default();
        let mut backpack = Grid::new(BARE.w, BARE.h);
        let mut gear = Equipment::new(&[PACK, TANK]);

        let ore = ids.make(ore);
        let (small, big) = (ids.make(tank), ids.make(tank));
        for item in [ore, small, big] {
            backpack.insert(&catalogue, item, None).unwrap();
        }
        assert_eq!(
            equip(&catalogue, &mut backpack, &mut gear, BARE, ore.id),
            Err(Refused::NotGear)
        );
        equip(&catalogue, &mut backpack, &mut gear, BARE, small.id).unwrap();
        equip(&catalogue, &mut backpack, &mut gear, BARE, big.id).unwrap();
        assert_eq!(gear.worn(TANK), Some(big));
        assert!(backpack.get(small.id).is_some());
    }
}
