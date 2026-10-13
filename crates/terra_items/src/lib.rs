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
    /// How much it has been used, for a kind that wears (D99): 0 is as made. At its kind's
    /// [`KindDef::lasts`] it is broken. Moving it never changes this.
    pub wear: u16,
}

impl Item {
    /// Whether it is worn out: broken, and of no use until mended. It is still kept.
    pub fn broken(&self, def: &KindDef) -> bool {
        def.lasts.is_some_and(|lasts| self.wear >= lasts)
    }

    /// How much of its use is left, from 1 as made to 0 broken; nothing for a kind that
    /// does not wear.
    pub fn condition(&self, def: &KindDef) -> Option<f32> {
        def.lasts
            .map(|lasts| 1.0 - self.wear.min(lasts) as f32 / lasts.max(1) as f32)
    }

    /// Wears it by so much use. Returns whether that broke it.
    fn wear_by(&mut self, def: &KindDef, amount: u16) -> bool {
        let Some(lasts) = def.lasts else { return false };
        let was = self.broken(def);
        self.wear = self.wear.saturating_add(amount).min(lasts);
        !was && self.broken(def)
    }
}

/// A cost totalled by kind, each kind once, in the order it is first named.
pub fn by_kind(cost: &[(ItemKind, u32)]) -> Vec<(ItemKind, u32)> {
    let mut totals: Vec<(ItemKind, u32)> = Vec::new();
    for &(kind, count) in cost {
        match totals.iter_mut().find(|(each, _)| *each == kind) {
            Some((_, total)) => *total += count,
            None => totals.push((kind, count)),
        }
    }
    totals
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
        Item { id, kind, wear: 0 }
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

    /// A tool wears with use, breaks at its kind's limit and is kept, carries its wear
    /// wherever it goes, and is as made again when mended (D99).
    #[test]
    fn wear_breaks_an_item_without_losing_it() {
        const BODY: GearSlot = GearSlot(0);
        let mut catalogue = Catalogue::default();
        let axe = catalogue.add(KindDef::new("Axe").size(2, 1).lasts(3));
        let coat = catalogue.add(KindDef::new("Coat").gear(BODY).lasts(2));
        let stone = catalogue.add(KindDef::new("Stone"));
        let mut ids = IdSource::default();
        let mut backpack = Grid::new(4, 4);
        let mut locker = Grid::new(4, 4);
        let (axe, stone) = (ids.make(axe), ids.make(stone));
        backpack.insert(&catalogue, axe, None).unwrap();
        backpack.insert(&catalogue, stone, None).unwrap();

        assert!(!backpack.wear(&catalogue, axe.id, 2).unwrap());
        assert!(
            backpack.wear(&catalogue, axe.id, 5).unwrap(),
            "the third use breaks it"
        );
        assert!(
            !backpack.wear(&catalogue, axe.id, 1).unwrap(),
            "it breaks only once"
        );
        let worn = backpack.get(axe.id).unwrap().item;
        let def = catalogue.get(worn.kind).unwrap();
        assert!(worn.broken(def));
        assert_eq!(worn.condition(def), Some(0.0));
        // Broken, it is still carried, and moving it keeps its wear.
        transfer(&catalogue, &mut backpack, &mut locker, axe.id, None).unwrap();
        assert!(locker.get(axe.id).unwrap().item.broken(def));
        locker.set_wear(axe.id, 0).unwrap();
        assert_eq!(locker.get(axe.id).unwrap().item.condition(def), Some(1.0));
        // What does not wear is untouched.
        assert!(!backpack.wear(&catalogue, stone.id, 9).unwrap());
        assert_eq!(backpack.get(stone.id).unwrap().item.wear, 0);
        // Worn gear wears where it is worn.
        let mut gear = Equipment::new(&[BODY]);
        let coat = ids.make(coat);
        backpack.insert(&catalogue, coat, None).unwrap();
        equip(
            &catalogue,
            &mut backpack,
            &mut gear,
            Size { w: 4, h: 4 },
            coat.id,
        )
        .unwrap();
        assert!(gear.wear(&catalogue, BODY, 2).unwrap());
        gear.set_wear(BODY, 0).unwrap();
        assert_eq!(gear.worn(BODY).unwrap().wear, 0);
    }

    /// Paints every item's cells, at its kind's size, onto an occupancy map of its own,
    /// failing if one lands outside the grid or on another: a check that does not ask the
    /// grid whether things fit.
    fn assert_laid_out(catalogue: &Catalogue, grid: &Grid, what: &str) {
        let size = grid.size();
        let (w, h) = (size.w as usize, size.h as usize);
        let mut filled = vec![false; w * h];
        for placed in grid.iter() {
            let cells = catalogue.get(placed.item.kind).unwrap().size;
            let (x0, y0) = (placed.at.x as usize, placed.at.y as usize);
            for y in y0..y0 + cells.h as usize {
                for x in x0..x0 + cells.w as usize {
                    assert!(x < w && y < h, "{what}: an item hangs outside the grid");
                    assert!(!filled[y * w + x], "{what}: two items overlap");
                    filled[y * w + x] = true;
                }
            }
        }
    }

    /// Random moves between a backpack, a locker and what is worn never make or lose an
    /// item, never overlap two, and never leave one outside its grid.
    #[test]
    fn moving_never_makes_or_loses_an_item() {
        const PACK: GearSlot = GearSlot(0);
        const TANK: GearSlot = GearSlot(1);
        const BARE: Size = Size { w: 3, h: 3 };
        const PACKED: Size = Size { w: 5, h: 4 };
        let mut catalogue = Catalogue::default();
        let kinds = [
            catalogue.add(KindDef::new("Ore")),
            catalogue.add(KindDef::new("Plank").size(2, 1)),
            catalogue.add(KindDef::new("Crate").size(2, 2)),
            catalogue.add(KindDef::new("Tank").size(1, 2).gear(TANK)),
            catalogue.add(
                KindDef::new("Pack")
                    .size(2, 2)
                    .gear(PACK)
                    .backpack(PACKED.w, PACKED.h),
            ),
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

        let mut accepted = [0u32; 6];
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
            let op = rng.below(6) as usize;
            let done = match op {
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
            if matches!(done, Some(Ok(()))) {
                accepted[op] += 1;
            }

            assert_eq!(all(&backpack, &locker, &gear), expected);
            let size = if gear.worn(PACK).is_some() {
                PACKED
            } else {
                BARE
            };
            assert_eq!(
                backpack.size(),
                size,
                "the backpack is the size what is worn makes it"
            );
            assert_laid_out(&catalogue, &backpack, "the backpack");
            assert_laid_out(&catalogue, &locker, "the locker");
        }
        assert!(
            accepted.iter().all(|&count| count >= 50),
            "too few moves were made to mean anything: {accepted:?}"
        );
    }

    /// Random stowing, paying, drawing, moving and wearing of things that stack never
    /// change how many there are except by what was put in or taken out, never overfill
    /// a container, and pay all or nothing, also for a cost that names a kind twice. A
    /// failure names its seed and step, so it can be run again exactly.
    #[test]
    fn counts_change_only_by_what_goes_in_or_out() {
        const BELT: GearSlot = GearSlot(0);
        let mut catalogue = Catalogue::default();
        let arrow = catalogue.add(KindDef::new("Arrow"));
        let stone = catalogue.add(KindDef::new("Stone"));
        let quiver = catalogue.add(
            KindDef::new("Quiver")
                .size(1, 2)
                .gear(BELT)
                .holds(arrow, 12),
        );
        let pouch = catalogue.add(KindDef::new("Pouch").holds(arrow, 5));
        let carried = |kind, backpack: &Grid, locker: &Grid, gear: &Equipment| {
            backpack.count(kind) + locker.count(kind) + gear.stacked(kind)
        };
        let mut accepted = [0u32; 7];
        let mut refused_for_naming_twice = 0;
        for seed in 0..20 {
            let mut rng = Seed(seed).derive("counts").rng();
            let mut ids = IdSource::default();
            let mut backpack = Grid::new(4, 3);
            let mut locker = Grid::new(6, 6);
            let mut gear = Equipment::new(&[BELT]);
            for kind in [quiver, quiver, pouch, pouch] {
                locker.insert(&catalogue, ids.make(kind), None).unwrap();
            }
            let (mut arrows, mut stones) = (0, 0);
            for step in 0..2000 {
                let roll = rng.below(1000);
                let some = 1 + rng.below(8);
                let pick = |grid: &Grid| {
                    let index = roll as usize % grid.len().max(1);
                    grid.iter().nth(index).map(|placed| placed.item.id)
                };
                let op = rng.below(7) as usize;
                let done = match op {
                    0 => {
                        let kind = if roll.is_multiple_of(2) { arrow } else { stone };
                        let put = backpack.insert(&catalogue, ids.make(kind), None).is_ok();
                        if put && kind == arrow {
                            arrows += 1;
                        } else if put {
                            stones += 1;
                        }
                        put
                    }
                    1 => {
                        let put = backpack.stow(arrow, some);
                        arrows += put;
                        put > 0
                    }
                    2 => {
                        let put = gear.stow(arrow, some);
                        arrows += put;
                        put > 0
                    }
                    3 => {
                        // One to three lines, of either kind, so a kind is often named twice.
                        let cost: Vec<(ItemKind, u32)> = (0..1 + rng.below(3))
                            .map(|_| {
                                let kind = if rng.below(2) == 0 { arrow } else { stone };
                                (kind, 1 + rng.below(4))
                            })
                            .collect();
                        let wants = |kind| {
                            cost.iter()
                                .filter(|&&(each, _)| each == kind)
                                .map(|&(_, count)| count)
                                .sum::<u32>()
                        };
                        let enough = [arrow, stone]
                            .iter()
                            .all(|&kind| backpack.count(kind) >= wants(kind));
                        let before = backpack.clone();
                        match backpack.take(&cost) {
                            Ok(_) => {
                                assert!(enough, "seed {seed} step {step}: paid without enough");
                                arrows -= wants(arrow);
                                stones -= wants(stone);
                                true
                            }
                            Err(_) => {
                                assert!(
                                    !enough,
                                    "seed {seed} step {step}: refused though there was enough"
                                );
                                assert_eq!(
                                    backpack, before,
                                    "seed {seed} step {step}: a refused payment took something"
                                );
                                if cost
                                    .iter()
                                    .all(|&(kind, count)| before.count(kind) >= count)
                                {
                                    refused_for_naming_twice += 1;
                                }
                                false
                            }
                        }
                    }
                    4 => {
                        let drawn = gear.draw(arrow, some);
                        arrows -= drawn;
                        drawn > 0
                    }
                    5 => {
                        let (from, to) = if roll.is_multiple_of(2) {
                            (&mut locker, &mut backpack)
                        } else {
                            (&mut backpack, &mut locker)
                        };
                        pick(from)
                            .is_some_and(|id| transfer(&catalogue, from, to, id, None).is_ok())
                    }
                    _ => {
                        let bare = Size { w: 4, h: 3 };
                        match (roll % 2, pick(&backpack)) {
                            (0, Some(id)) => {
                                equip(&catalogue, &mut backpack, &mut gear, bare, id).is_ok()
                            }
                            _ => unequip(&catalogue, &mut backpack, &mut gear, bare, BELT, None)
                                .is_ok(),
                        }
                    }
                };
                if done {
                    accepted[op] += 1;
                }
                assert_eq!(
                    carried(arrow, &backpack, &locker, &gear),
                    arrows,
                    "seed {seed} step {step}, after op {op}: arrows made or lost"
                );
                assert_eq!(
                    carried(stone, &backpack, &locker, &gear),
                    stones,
                    "seed {seed} step {step}, after op {op}: stones made or lost"
                );
                let stacks = backpack
                    .iter()
                    .chain(locker.iter())
                    .filter_map(|placed| placed.stack)
                    .chain(gear.stack(BELT));
                for stack in stacks {
                    assert!(
                        stack.held <= stack.most,
                        "seed {seed} step {step}: a container holds more than it can"
                    );
                }
            }
        }
        assert!(
            accepted.iter().all(|&count| count >= 100),
            "too few changes were made to mean anything: {accepted:?}"
        );
        assert!(
            refused_for_naming_twice >= 50,
            "too few payments named a kind twice and were refused for it: \
             {refused_for_naming_twice}"
        );
    }
}
