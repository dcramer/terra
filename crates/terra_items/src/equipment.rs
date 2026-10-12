use crate::{Catalogue, Cell, GearSlot, Grid, Item, ItemId, Refused, Size, Stack};

/// What is worn: one item per slot, each slot taking only gear of its kind (D64).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Equipment {
    slots: Vec<(GearSlot, Option<Item>)>,
    /// What a worn container holds, by slot, such as a belt quiver's arrows (D118).
    stacks: Vec<Option<Stack>>,
}

impl Equipment {
    pub fn new(slots: &[GearSlot]) -> Self {
        Self {
            slots: slots.iter().map(|&slot| (slot, None)).collect(),
            stacks: vec![None; slots.len()],
        }
    }

    /// What the container worn in a slot holds, if one is.
    pub fn stack(&self, slot: GearSlot) -> Option<Stack> {
        let index = self.slots.iter().position(|(each, _)| *each == slot)?;
        self.stacks[index]
    }

    /// How many of a kind worn containers hold.
    pub fn stacked(&self, kind: crate::ItemKind) -> u32 {
        self.stacks
            .iter()
            .flatten()
            .filter(|stack| stack.kind == kind)
            .map(|stack| stack.held)
            .sum()
    }

    /// Puts up to `count` of a kind into worn containers with room. Returns how many went.
    pub fn stow(&mut self, kind: crate::ItemKind, count: u32) -> u32 {
        let mut left = count;
        for stack in self.stacks.iter_mut().flatten() {
            if stack.kind == kind {
                let put = (stack.most - stack.held).min(left);
                stack.held += put;
                left -= put;
            }
        }
        count - left
    }

    /// Takes up to `count` of a kind from worn containers. Returns how many it took.
    pub fn draw(&mut self, kind: crate::ItemKind, count: u32) -> u32 {
        let mut left = count;
        for stack in self.stacks.iter_mut().flatten() {
            if stack.kind == kind {
                let drawn = stack.held.min(left);
                stack.held -= drawn;
                left -= drawn;
            }
        }
        count - left
    }

    /// Sets what a worn container holds, as when a saved one is put back.
    pub fn set_held(&mut self, slot: GearSlot, held: u32) -> Result<(), Refused> {
        let index = self
            .slots
            .iter()
            .position(|(each, _)| *each == slot)
            .ok_or(Refused::NoSuchSlot)?;
        match self.stacks[index].as_mut() {
            Some(stack) if held <= stack.most => {
                stack.held = held;
                Ok(())
            }
            _ => Err(Refused::NoRoom),
        }
    }

    fn stack_mut(&mut self, slot: GearSlot) -> Result<&mut Option<Stack>, Refused> {
        let index = self
            .slots
            .iter()
            .position(|(each, _)| *each == slot)
            .ok_or(Refused::NoSuchSlot)?;
        Ok(&mut self.stacks[index])
    }

    /// Wears what is in a slot by so much use (D99). Returns whether that broke it.
    pub fn wear(
        &mut self,
        catalogue: &Catalogue,
        slot: GearSlot,
        amount: u16,
    ) -> Result<bool, Refused> {
        let item = self.slot_mut(slot)?.as_mut().ok_or(Refused::NotFound)?;
        let def = catalogue.get(item.kind)?;
        Ok(item.wear_by(def, amount))
    }

    /// Sets how worn what is in a slot is: to 0 when it is mended, or as a saved game says.
    pub fn set_wear(&mut self, slot: GearSlot, wear: u16) -> Result<(), Refused> {
        let item = self.slot_mut(slot)?.as_mut().ok_or(Refused::NotFound)?;
        item.wear = wear;
        Ok(())
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
    let placed = *backpack.get(id).ok_or(Refused::NotFound)?;
    let item = placed.item;
    let slot = catalogue.get(item.kind)?.gear.ok_or(Refused::NotGear)?;
    gear.slot_mut(slot)?;

    let (backpack_before, gear_before) = (backpack.clone(), gear.clone());
    let result = (|| {
        backpack.remove(id)?;
        let old = gear.slot_mut(slot)?.replace(item);
        // A container goes on with what is in it, and one taken off keeps its own.
        let old_stack = std::mem::replace(gear.stack_mut(slot)?, placed.stack);
        if let Some(old) = old {
            backpack.insert(catalogue, old, None)?;
            if let Some(stack) = old_stack {
                backpack.set_held(old.id, stack.held)?;
            }
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
/// fits in the backpack it leaves behind. Refused if that backpack could not hold what is
/// in it.
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
        let stack = gear.stack_mut(slot)?.take();
        // Taking off a pack shrinks the backpack, so what comes off has to land where the
        // smaller one still reaches.
        let after = gear.backpack_size(catalogue, bare);
        let size = catalogue.get(item.kind)?.size;
        let at = match at {
            Some(cell) => cell,
            None => backpack.space_within(size, after).ok_or(Refused::NoRoom)?,
        };
        let cell = backpack.insert(catalogue, item, Some(at))?;
        if let Some(stack) = stack {
            backpack.set_held(item.id, stack.held)?;
        }
        backpack.resize(after)?;
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

    #[test]
    fn a_worn_container_keeps_what_it_holds() {
        const BELT: GearSlot = GearSlot(2);
        let mut catalogue = Catalogue::default();
        let arrow = catalogue.add(KindDef::new("Arrow"));
        let quiver = catalogue.add(
            KindDef::new("Quiver")
                .size(1, 2)
                .gear(BELT)
                .holds(arrow, 20),
        );
        let mut ids = IdSource::default();
        let mut backpack = Grid::new(4, 4);
        let mut gear = Equipment::new(&[BELT]);
        let worn = ids.make(quiver);
        backpack.insert(&catalogue, worn, None).unwrap();
        backpack.stow(arrow, 5);
        equip(
            &catalogue,
            &mut backpack,
            &mut gear,
            Size { w: 4, h: 4 },
            worn.id,
        )
        .unwrap();
        assert_eq!(backpack.count(arrow), 0);
        assert_eq!(gear.stacked(arrow), 5);
        assert_eq!(gear.stow(arrow, 30), 15);
        assert_eq!(gear.draw(arrow, 3), 3);
        assert_eq!(gear.stacked(arrow), 17);
        unequip(
            &catalogue,
            &mut backpack,
            &mut gear,
            Size { w: 4, h: 4 },
            BELT,
            None,
        )
        .unwrap();
        assert_eq!(backpack.count(arrow), 17, "taken off, it keeps them");
        assert_eq!(gear.stacked(arrow), 0);
    }

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
            Err(Refused::NoRoom),
            "three ores and the pack do not fit the bare backpack"
        );
        backpack.take(&[(ore, 3)]).unwrap();
        unequip(&catalogue, &mut backpack, &mut gear, BARE, PACK, None).unwrap();
        assert_eq!(gear.worn(PACK), None);
        assert_eq!(backpack.size(), BARE);
    }

    #[test]
    fn a_pack_taken_off_lands_where_the_smaller_backpack_still_reaches() {
        let mut catalogue = Catalogue::default();
        let long = catalogue.add(KindDef::new("Long").size(2, 1));
        let pack = catalogue.add(KindDef::new("Pack").size(2, 2).gear(PACK).backpack(6, 4));
        let mut ids = IdSource::default();
        let bare = Size { w: 5, h: 4 };
        let mut backpack = Grid::new(bare.w, bare.h);
        let mut gear = Equipment::new(&[PACK]);
        let pack = ids.make(pack);
        backpack.insert(&catalogue, pack, None).unwrap();
        equip(&catalogue, &mut backpack, &mut gear, bare, pack.id).unwrap();
        // Two long things across the top leave the first free place at x 4, which the
        // bare backpack does not reach.
        for _ in 0..2 {
            backpack.insert(&catalogue, ids.make(long), None).unwrap();
        }
        let at = unequip(&catalogue, &mut backpack, &mut gear, bare, PACK, None).unwrap();
        assert_eq!(at, Cell { x: 0, y: 1 });
        assert_eq!(backpack.size(), bare);
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
