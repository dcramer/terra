//! What can be done with what a player carries, as plain functions of their containers:
//! wearing and taking off, paying and refunding, giving and making room. `belongings`
//! does these for a player in the world.

use super::{BAR_SLOTS, BARE, Backpack, Bar, Cost, Gear, Items, Made, SLOTS};
use bevy::prelude::*;
use terra_items::{ActionBar, Equipment, GearSlot, Grid, Item, ItemId, ItemKind, Refused, Size};

/// A backpack with no pack worn, to fill as a saved world is loaded.
pub fn bare_backpack() -> Grid {
    Grid::new(BARE.w, BARE.h)
}

/// How much of a blow everything worn takes together, out of one, never all of it.
pub fn protection(items: &Items, gear: &Equipment) -> f32 {
    gear.iter()
        .filter_map(|(_, worn)| worn)
        // Broken, it protects no more (D99).
        .filter(|item| {
            items
                .catalogue
                .get(item.kind)
                .is_ok_and(|def| !item.broken(def))
        })
        .filter_map(|item| items.worn(item.kind))
        .map(|worn| worn.protection)
        .sum::<f32>()
        .min(0.8)
}

/// How warm everything worn keeps its wearer together, in clo (`warmth`): broken, a thing
/// still keeps out the cold.
pub fn warmth(items: &Items, gear: &Equipment) -> f32 {
    gear.iter()
        .filter_map(|(_, worn)| worn)
        .filter_map(|item| items.worn(item.kind))
        .map(|worn| worn.warmth)
        .sum()
}

/// For testing (--give): adds items by name, such as `stone`, `iron_axe` or
/// `leather_coat`: tools go on the bar, and what is worn is put on. Names for the body
/// are left alone.
pub fn give_for_testing(
    items: &mut Items,
    grid: &mut Grid,
    gear: &mut Equipment,
    bar: &mut ActionBar,
    gives: &[(String, u32)],
) {
    let named = |name: &str, wanted: &str| name.replace(' ', "_").eq_ignore_ascii_case(wanted);
    for (name, count) in gives {
        let Some(kind) = items.kinds().find(|&kind| named(items.name(kind), name)) else {
            if !["health", "fed", "stamina"].contains(&name.as_str()) {
                warn!("--give: there is no kind of item called {name}");
            }
            continue;
        };
        let called = items.name(kind).to_string();
        if items.tool(kind).is_some() {
            for _ in 0..*count {
                let made = items.make(kind);
                if grid.insert(&items.catalogue, made, None).is_err() {
                    warn!("--give: no room for a {called}");
                    break;
                }
                if let Some(slot) = (0..bar.len()).find(|&slot| bar.get(slot).is_none()) {
                    bar.set(slot, Some(made.id));
                }
            }
        } else if items.worn(kind).is_some() {
            for _ in 0..*count {
                let made = items.make(kind);
                let worn = grid
                    .insert(&items.catalogue, made, None)
                    .ok()
                    .and_then(|_| wear(items, grid, gear, made.id).ok());
                if worn.is_none() {
                    warn!("--give: could not put on a {called}");
                }
            }
        } else {
            // Into a worn container first, as anything picked up goes.
            let worn = gear.stow(kind, *count);
            let given = worn + give(items, grid, kind, *count - worn);
            if given < *count {
                warn!("--give: room for only {given} of {count} {called}");
            }
        }
    }
}

/// What a player starts with, as the items file says: what they wear, and their tools on
/// the bar.
pub fn starting_kit(items: &mut Items) -> (Backpack, Gear, Bar) {
    let mut gear = Equipment::new(&SLOTS.map(|(slot, _)| slot));
    let mut grid = Grid::new(BARE.w, BARE.h);
    let wears: Vec<ItemKind> = items
        .start
        .wears
        .iter()
        .map(|name| items.kind(name))
        .collect();
    for kind in wears {
        let worn = items.make(kind);
        grid.insert(&items.catalogue, worn, None)
            .expect("what is worn at the start fits an empty backpack");
        wear(items, &mut grid, &mut gear, worn.id).expect("what is worn at the start can be worn");
    }
    let mut bar = ActionBar::new(BAR_SLOTS);
    let tools: Vec<ItemKind> = items
        .start
        .tools
        .iter()
        .map(|name| items.kind(name))
        .collect();
    for (slot, kind) in tools.into_iter().enumerate() {
        let tool = items.make(kind);
        grid.insert(&items.catalogue, tool, None)
            .expect("the tools fit the backpack");
        bar.set(slot, Some(tool.id));
    }
    (Backpack(grid), Gear(gear), Bar(bar))
}

/// Wears an item from the backpack, putting whatever was in its slot in the backpack.
pub fn wear(
    items: &Items,
    grid: &mut Grid,
    gear: &mut Equipment,
    id: ItemId,
) -> Result<(), Refused> {
    terra_items::equip(&items.catalogue, grid, gear, BARE, id)
}

/// Takes off what is in a slot and puts it in the backpack.
pub fn take_off(
    items: &Items,
    grid: &mut Grid,
    gear: &mut Equipment,
    slot: GearSlot,
) -> Result<(), Refused> {
    terra_items::unequip(&items.catalogue, grid, gear, BARE, slot, None).map(drop)
}

/// Why a change to what is carried was refused, as a player would read it.
pub fn reason(refused: Refused) -> &'static str {
    match refused {
        Refused::NoRoom => "No room in your backpack",
        Refused::WouldNotFit => "What you carry would not fit in a smaller backpack",
        Refused::NotGear => "That is not something you wear",
        Refused::NotEnough => "You do not have enough",
        Refused::NotFound | Refused::UnknownKind | Refused::NoSuchSlot => "That cannot be done",
    }
}

/// The kind of tool in hand, whatever the picked slot of the bar points at, if it is a
/// tool and not broken: a broken one does nothing until it is mended (D99).
pub fn holding(items: &Items, grid: &Grid, bar: &ActionBar) -> Option<ItemKind> {
    let item = bar.in_hand(grid)?;
    let def = items.catalogue.get(item.kind).ok()?;
    (!item.broken(def) && items.tool(item.kind).is_some()).then_some(item.kind)
}

/// The first kind of item there is not enough of to pay a cost so many times over, and how
/// many more are needed. What worn containers hold counts, such as a belt quiver's arrows.
pub fn missing(
    items: &Items,
    grid: &Grid,
    gear: &Equipment,
    cost: &Cost,
    times: u32,
) -> Option<(ItemKind, u32)> {
    // Totalled, so a kind the cost names twice is counted for both.
    terra_items::by_kind(&items.wanted(cost, times))
        .into_iter()
        .find_map(|(kind, wanted)| {
            let have = grid.count(kind) + gear.stacked(kind);
            wanted
                .checked_sub(have)
                .filter(|&short| short > 0)
                .map(|short| (kind, short))
        })
}

/// Pays a cost so many times over, if it can be paid in full: from what worn containers
/// hold first, such as a belt quiver's arrows, then the backpack. Returns whether it was.
pub fn pay(
    items: &Items,
    grid: &mut Grid,
    bar: &mut ActionBar,
    gear: &mut Equipment,
    cost: &Cost,
    times: u32,
) -> bool {
    spend(grid, bar, gear, &items.wanted(cost, times))
}

/// Spends so many of each kind, if all of it is there: from what worn containers hold
/// first, such as a belt quiver's arrows, then the backpack. Returns whether it was.
pub fn spend(
    grid: &mut Grid,
    bar: &mut ActionBar,
    gear: &mut Equipment,
    wanted: &[(ItemKind, u32)],
) -> bool {
    // Totalled first, so a kind named twice is checked for both before anything is drawn.
    let wanted = terra_items::by_kind(wanted);
    let short = wanted
        .iter()
        .any(|&(kind, count)| grid.count(kind) + gear.stacked(kind) < count);
    if short {
        return false;
    }
    let rest: Vec<(ItemKind, u32)> = wanted
        .iter()
        .map(|&(kind, count)| (kind, count - gear.draw(kind, count)))
        .filter(|&(_, count)| count > 0)
        .collect();
    let paid = grid.take(&rest).is_ok();
    bar.tidy(grid);
    paid
}

/// Puts what a station made in the backpack, all or nothing. Returns whether it did.
pub fn receive(items: &mut Items, grid: &mut Grid, bar: &mut ActionBar, made: Made) -> bool {
    let (kind, count) = items.made(made);
    receive_kind(items, grid, bar, kind, count)
}

/// Puts so many of a kind in the backpack, all or nothing, a tool on the bar too.
pub fn receive_kind(
    items: &mut Items,
    grid: &mut Grid,
    bar: &mut ActionBar,
    kind: ItemKind,
    count: u32,
) -> bool {
    let mut trial = grid.clone();
    let mut made_ids = Vec::new();
    // Into a container that holds them first, such as arrows into a quiver (D118).
    let stowed = trial.stow(kind, count);
    for _ in stowed..count {
        let item = items.make(kind);
        if trial.insert(&items.catalogue, item, None).is_err() {
            return false;
        }
        made_ids.push(item.id);
    }
    *grid = trial;
    // A new tool goes on the bar's first free slot, so it is ready to pick up.
    if let (Some(_), Some(&id)) = (items.tool(kind), made_ids.first())
        && let Some(free) = (0..bar.len()).find(|&slot| bar.get(slot).is_none())
    {
        bar.set(free, Some(id));
    }
    true
}

/// Whether what a cost so many times over is made of would all fit in the backpack.
pub fn room_for(items: &Items, grid: &Grid, cost: &Cost, times: u32) -> bool {
    let mut trial = grid.clone();
    let mut id = u64::MAX;
    items.wanted(cost, times).into_iter().all(|(kind, count)| {
        (0..count).all(|_| {
            id -= 1;
            let item = Item {
                id: ItemId(id),
                kind,
                wear: 0,
            };
            trial.insert(&items.catalogue, item, None).is_ok()
        })
    })
}

/// Puts so many of a kind of item in the backpack, as many as fit. Returns how many did.
pub fn give(items: &mut Items, grid: &mut Grid, kind: ItemKind, count: u32) -> u32 {
    let size = items.catalogue.get(kind).map_or(Size::ONE, |def| def.size);
    // Into a container that holds them first, such as arrows into a quiver (D118).
    let mut given = grid.stow(kind, count);
    while given < count && grid.space_for(size).is_some() {
        let item = items.make(kind);
        if grid.insert(&items.catalogue, item, None).is_err() {
            break;
        }
        given += 1;
    }
    given
}

/// Wears everything worn that wears, by so much, as a blow taken does (D99): what is worn
/// on the body, not what the off hand holds (a torch burns down by the second). Returns the
/// kinds that broke.
pub fn wear_worn(items: &Items, gear: &mut Equipment, amount: u16) -> Vec<ItemKind> {
    let slots: Vec<GearSlot> = gear.iter().map(|(slot, _)| slot).collect();
    slots
        .into_iter()
        .filter_map(|slot| {
            let item = gear
                .worn(slot)
                .filter(|item| items.worn(item.kind).is_some())?;
            (gear.wear(&items.catalogue, slot, amount) == Ok(true)).then_some(item.kind)
        })
        .collect()
}

/// Empties a backpack into a bag of the same size, as when someone goes down (D84).
pub fn empty_into_bag(items: &Items, backpack: &mut Grid) -> Grid {
    let size = backpack.size();
    let mut bag = Grid::new(size.w, size.h);
    let ids: Vec<ItemId> = backpack.iter().map(|placed| placed.item.id).collect();
    for id in ids {
        let _ = terra_items::transfer(&items.catalogue, backpack, &mut bag, id, None);
    }
    bag
}
