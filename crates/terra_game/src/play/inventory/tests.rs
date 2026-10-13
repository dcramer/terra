use super::*;
use terra_items::Refused;

fn give_named(items: &mut Items, grid: &mut Grid, name: &str, count: u32) -> u32 {
    let kind = items.kind(name);
    give(items, grid, kind, count)
}

/// A pack protects only as much as the items file says it does, worn leather protects
/// more, and broken leather protects no more than if it were not worn.
#[test]
fn broken_leather_protects_no_more() {
    let mut items = Items::default();
    let (_, Gear(kit), _) = starting_kit(&mut items);
    let mut gear = Equipment::new(&SLOTS.map(|(slot, _)| slot));
    let mut grid = bare_backpack();
    let pack = items.make(kit.worn(PACK).unwrap().kind);
    grid.insert(&items.catalogue, pack, None).unwrap();
    wear(&items, &mut grid, &mut gear, pack.id).unwrap();
    let packed = protection(&items, &gear);
    assert_eq!(packed, items.worn(pack.kind).unwrap().protection);
    let coat = items.make(items.kind("Leather coat"));
    grid.insert(&items.catalogue, coat, None).unwrap();
    wear(&items, &mut grid, &mut gear, coat.id).unwrap();
    assert!(protection(&items, &gear) > packed);
    let body = slot_named(&items.worn(coat.kind).unwrap().slot);
    gear.wear(&items.catalogue, body, u16::MAX).unwrap();
    assert_eq!(protection(&items, &gear), packed);
}

/// A cost, by item names.
fn cost(items: &Items, named: &[(&str, u32)]) -> Vec<(ItemKind, u32)> {
    named
        .iter()
        .map(|&(name, count)| (items.kind(name), count))
        .collect()
}

fn couch(items: &Items) -> Vec<(ItemKind, u32)> {
    cost(items, &[("Wood", 2), ("Hide", 3)])
}

fn made(items: &Items, name: &str, count: u32) -> Made {
    Made {
        kind: items.kind(name),
        count,
    }
}

/// What the items file starts a player with is worn, and its tools are on the bar in its
/// order, with room left in the backpack.
#[test]
fn the_starting_kit_wears_its_gear_and_has_its_tools_on_the_bar_and_room_to_spare() {
    let mut items = Items::default();
    let (Backpack(grid), Gear(gear), Bar(mut bar)) = starting_kit(&mut items);
    let (wears, tools) = (items.start.wears.clone(), items.start.tools.clone());
    assert!(!wears.is_empty() && !tools.is_empty());
    let mut size = BARE;
    for name in &wears {
        let kind = items.kind(name);
        let slot = slot_named(&items.worn(kind).unwrap().slot);
        assert_eq!(gear.worn(slot).map(|item| item.kind), Some(kind));
        if let Some((w, h)) = items.worn(kind).unwrap().backpack {
            size = Size { w, h };
        }
    }
    assert_eq!(grid.size(), size, "the backpack is as big as the pack worn");
    for (slot, name) in tools.iter().enumerate() {
        bar.pick(slot);
        assert_eq!(holding(&items, &grid, &bar), Some(items.kind(name)));
        assert_eq!(grid.count(items.kind(name)), 1);
    }
    assert!(grid.space_for(Size::ONE).is_some());
}

#[test]
fn having_more_than_enough_is_not_missing_anything() {
    let mut items = Items::default();
    let (Backpack(mut grid), Gear(gear), _) = starting_kit(&mut items);
    give_named(&mut items, &mut grid, "Wood", 5);
    let wood = cost(&items, &[("Wood", 2)]);
    assert_eq!(missing(&items, &grid, &gear, &wood, 1), None);
}

#[test]
fn a_cost_is_paid_in_full_or_not_at_all() {
    let mut items = Items::default();
    let (Backpack(mut grid), Gear(mut gear), Bar(mut bar)) = starting_kit(&mut items);
    give_named(&mut items, &mut grid, "Wood", 4);
    give_named(&mut items, &mut grid, "Hide", 3);
    let count = |grid: &Grid, name| grid.count(items.kind(name));
    assert!(pay(
        &items,
        &mut grid,
        &mut bar,
        &mut gear,
        &couch(&items),
        1
    ));
    assert_eq!(count(&grid, "Wood"), 2);
    assert_eq!(count(&grid, "Hide"), 0);
    assert_eq!(
        missing(&items, &grid, &gear, &couch(&items), 1),
        Some((items.kind("Hide"), 3)),
        "all the hide is spent"
    );
    let before = grid.clone();
    assert!(!pay(
        &items,
        &mut grid,
        &mut bar,
        &mut gear,
        &couch(&items),
        1
    ));
    assert_eq!(grid, before);
    assert_eq!(describe(&items, &couch(&items), 2), "4 Wood, 6 Hide");
}

#[test]
fn a_full_backpack_takes_what_fits_and_refuses_the_rest() {
    let mut items = Items::default();
    let (Backpack(mut grid), ..) = starting_kit(&mut items);
    let free = (grid.size().w as u32 * grid.size().h as u32)
        - grid
            .iter()
            .map(|placed| placed.size.w as u32 * placed.size.h as u32)
            .sum::<u32>();
    assert!(room_for(&items, &grid, &couch(&items), 1));
    assert_eq!(give_named(&mut items, &mut grid, "Wood", free + 5), free);
    assert!(grid.space_for(Size::ONE).is_none());
    assert!(!room_for(&items, &grid, &couch(&items), 1));
    assert_eq!(give_named(&mut items, &mut grid, "Wood", 1), 0);
}

#[test]
fn gear_comes_off_into_the_backpack_and_goes_back_on() {
    let mut items = Items::default();
    let (Backpack(mut grid), Gear(mut gear), _) = starting_kit(&mut items);
    // The pack comes off once what is carried fits the bare backpack: here, one thing.
    let pack = gear.worn(PACK).unwrap();
    let packed = grid.size();
    let ids: Vec<_> = grid.iter().map(|placed| placed.item.id).collect();
    for id in ids {
        grid.remove(id).unwrap();
    }
    assert_eq!(give_named(&mut items, &mut grid, "Wood", 1), 1);
    take_off(&items, &mut grid, &mut gear, PACK).unwrap();
    assert_eq!(gear.worn(PACK), None);
    assert!(grid.get(pack.id).is_some());
    assert_eq!(grid.size(), BARE);
    assert_eq!(grid.count(items.kind("Wood")), 1);
    wear(&items, &mut grid, &mut gear, pack.id).unwrap();
    assert_eq!(gear.worn(PACK), Some(pack));
    assert!(grid.get(pack.id).is_none());
    assert_eq!(grid.size(), packed);
}

#[test]
fn the_pack_stays_on_while_what_is_carried_needs_it() {
    let mut items = Items::default();
    let (Backpack(mut grid), Gear(mut gear), _) = starting_kit(&mut items);
    give_named(&mut items, &mut grid, "Stone", 20);
    let before = (grid.clone(), gear.clone());
    let refused = take_off(&items, &mut grid, &mut gear, PACK).unwrap_err();
    assert!(matches!(refused, Refused::NoRoom | Refused::WouldNotFit));
    assert_eq!((grid, gear), before);
}

#[test]
fn what_is_made_goes_in_the_backpack_and_a_tool_on_the_bar() {
    let mut items = Items::default();
    let (Backpack(mut grid), _, Bar(mut bar)) = starting_kit(&mut items);
    assert!({
        let made = made(&items, "Iron axe", 1);
        receive(&mut items, &mut grid, &mut bar, made)
    });
    assert_eq!(grid.count(items.kind("Iron axe")), 1);
    assert_eq!(
        bar.get(items.start.tools.len())
            .and_then(|id| grid.get(id))
            .map(|placed| placed.item.kind),
        Some(items.kind("Iron axe")),
        "a new tool goes on the first free slot of the bar"
    );
}

/// Three made with room for two puts none in, not two.
#[test]
fn what_is_made_is_all_or_nothing_in_a_full_backpack() {
    let mut items = Items::default();
    let (Backpack(mut grid), _, Bar(mut bar)) = starting_kit(&mut items);
    // A kind one cell big that no container holds, so each one made needs a cell.
    let held: Vec<ItemKind> = items
        .kinds()
        .filter_map(|kind| items.catalogue.get(kind).ok()?.holds)
        .map(|(kind, _)| kind)
        .collect();
    let kind = items
        .kinds()
        .find(|&kind| {
            let def = items.catalogue.get(kind).unwrap();
            def.size == Size::ONE && def.holds.is_none() && !held.contains(&kind)
        })
        .unwrap();
    let free = grid.size().w as u32 * grid.size().h as u32
        - grid
            .iter()
            .map(|placed| placed.size.w as u32 * placed.size.h as u32)
            .sum::<u32>();
    assert_eq!(give(&mut items, &mut grid, kind, free - 2), free - 2);
    let before = grid.clone();
    assert!(!receive(
        &mut items,
        &mut grid,
        &mut bar,
        Made { kind, count: 3 }
    ));
    assert_eq!(grid, before);
    assert!(receive(
        &mut items,
        &mut grid,
        &mut bar,
        Made { kind, count: 2 }
    ));
}

#[test]
fn arrows_go_in_a_quiver_and_are_shot_from_it() {
    let mut items = Items::default();
    let (Backpack(mut grid), Gear(mut gear), Bar(mut bar)) = starting_kit(&mut items);
    let quiver = items.make(items.kind("Quiver"));
    grid.insert(&items.catalogue, quiver, None).unwrap();
    let cells = grid.len();
    assert_eq!(give_named(&mut items, &mut grid, "Arrow", 6), 6);
    assert!({
        let made = made(&items, "Arrow", 4);
        receive(&mut items, &mut grid, &mut bar, made)
    });
    assert_eq!(grid.len(), cells, "they fill no cells of their own");
    assert_eq!(grid.count(items.kind("Arrow")), 10);
    assert!(pay(
        &items,
        &mut grid,
        &mut bar,
        &mut gear,
        &cost(&items, &[("Arrow", 1)]),
        1
    ));
    assert_eq!(grid.count(items.kind("Arrow")), 9);
    // Worn on the belt, it is shot from first, and still counts.
    wear(&items, &mut grid, &mut gear, quiver.id).unwrap();
    give_named(&mut items, &mut grid, "Arrow", 2);
    assert_eq!(gear.stacked(items.kind("Arrow")), 9);
    assert_eq!(
        missing(&items, &grid, &gear, &cost(&items, &[("Arrow", 11)]), 1),
        None
    );
    assert!(pay(
        &items,
        &mut grid,
        &mut bar,
        &mut gear,
        &cost(&items, &[("Arrow", 10)]),
        1
    ));
    assert_eq!(gear.stacked(items.kind("Arrow")), 0);
    assert_eq!(grid.count(items.kind("Arrow")), 1);
}

/// A cost that names a kind twice wants both. Refused, it draws nothing from a worn
/// quiver nor the backpack, though either line alone could be paid.
#[test]
fn a_kind_named_twice_is_paid_for_both_or_not_at_all() {
    let mut items = Items::default();
    let (Backpack(mut grid), Gear(mut gear), Bar(mut bar)) = starting_kit(&mut items);
    let arrow = items.kind("Arrow");
    let quiver = items.make(items.kind("Quiver"));
    grid.insert(&items.catalogue, quiver, None).unwrap();
    wear(&items, &mut grid, &mut gear, quiver.id).unwrap();
    assert_eq!(gear.stow(arrow, 2), 2);
    assert_eq!(give(&mut items, &mut grid, arrow, 1), 1);
    let twice = [(arrow, 2), (arrow, 2)];
    assert_eq!(missing(&items, &grid, &gear, &twice, 1), Some((arrow, 1)));
    let before = (grid.clone(), gear.clone());
    assert!(!pay(&items, &mut grid, &mut bar, &mut gear, &twice, 1));
    assert_eq!((grid.clone(), gear.clone()), before);
    let three = [(arrow, 1), (arrow, 2)];
    assert!(pay(&items, &mut grid, &mut bar, &mut gear, &three, 1));
    assert_eq!(grid.count(arrow) + gear.stacked(arrow), 0);
}

#[test]
fn spending_what_the_bar_points_at_empties_its_slot() {
    let mut items = Items::default();
    let (Backpack(mut grid), Gear(mut gear), Bar(mut bar)) = starting_kit(&mut items);
    let tool = grid.get(bar.get(0).unwrap()).unwrap().item.kind;
    assert!(pay(&items, &mut grid, &mut bar, &mut gear, &[(tool, 1)], 1));
    assert_eq!(bar.get(0), None);
    assert_eq!(holding(&items, &grid, &bar), None);
}
