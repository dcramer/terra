use super::*;
use terra_items::Refused;

fn give_named(items: &mut Items, grid: &mut Grid, name: &str, count: u32) -> u32 {
    let kind = items.kind(name);
    give(items, grid, kind, count)
}

#[test]
fn broken_leather_protects_no_more() {
    let mut items = Items::default();
    let (Backpack(mut grid), Gear(mut gear), _) = starting_kit(&mut items);
    let coat = items.make(items.kind("Leather coat"));
    grid.insert(&items.catalogue, coat, None).unwrap();
    wear(&items, &mut grid, &mut gear, coat.id).unwrap();
    let whole = protection(&items, &gear);
    assert!(whole > 0.0);
    let body = SLOTS[2].0;
    gear.wear(&items.catalogue, body, u16::MAX).unwrap();
    assert_eq!(protection(&items, &gear), 0.0);
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

#[test]
fn the_starting_kit_has_both_tools_on_the_bar_and_room_to_spare() {
    let mut items = Items::default();
    let (Backpack(grid), Gear(gear), Bar(mut bar)) = starting_kit(&mut items);
    assert_eq!(
        gear.worn(PACK).map(|item| item.kind),
        Some(items.kind("Small pack"))
    );
    assert_eq!(grid.size(), Size { w: 6, h: 5 });
    assert_eq!(holding(&items, &grid, &bar), Some(items.kind("Hammer")));
    bar.pick(1);
    assert_eq!(holding(&items, &grid, &bar), Some(items.kind("Stone axe")));
    for tool in items.start.tools.clone() {
        assert_eq!(grid.count(items.kind(&tool)), 1);
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
    // The pack comes off once what is carried fits the bare backpack.
    let pack = gear.worn(PACK).unwrap();
    for tool in ["Stone pickaxe", "Knife"] {
        let id = grid
            .iter()
            .find(|placed| placed.item.kind == items.kind(tool))
            .unwrap()
            .item
            .id;
        grid.remove(id).unwrap();
    }
    take_off(&items, &mut grid, &mut gear, PACK).unwrap();
    assert_eq!(gear.worn(PACK), None);
    assert!(grid.get(pack.id).is_some());
    wear(&items, &mut grid, &mut gear, pack.id).unwrap();
    assert_eq!(gear.worn(PACK), Some(pack));
    assert!(grid.get(pack.id).is_none());
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

#[test]
fn what_is_made_is_all_or_nothing_in_a_full_backpack() {
    let mut items = Items::default();
    let (Backpack(mut grid), _, Bar(mut bar)) = starting_kit(&mut items);
    give_named(&mut items, &mut grid, "Ore", 100);
    let before = grid.clone();
    assert!(!{
        let made = made(&items, "Iron", 1);
        receive(&mut items, &mut grid, &mut bar, made)
    });
    assert_eq!(grid, before);
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

#[test]
fn paying_takes_all_or_nothing() {
    let axe = cost(&Items::default(), &[("Wood", 2), ("Iron", 3)]);
    let mut items = Items::default();
    let (Backpack(mut grid), Gear(mut gear), Bar(mut bar)) = starting_kit(&mut items);
    give_named(&mut items, &mut grid, "Wood", 2);
    give_named(&mut items, &mut grid, "Iron", 2);
    let before = grid.clone();
    assert_eq!(
        missing(&items, &grid, &gear, &axe, 1),
        Some((items.kind("Iron"), 1))
    );
    assert!(!pay(&items, &mut grid, &mut bar, &mut gear, &axe, 1));
    assert_eq!(grid, before);
    give_named(&mut items, &mut grid, "Iron", 1);
    assert!(pay(&items, &mut grid, &mut bar, &mut gear, &axe, 1));
    assert_eq!(grid.count(items.kind("Iron")), 0);
}

#[test]
fn worn_leather_protects_and_a_pack_does_not() {
    let mut items = Items::default();
    let (Backpack(mut grid), Gear(mut gear), _) = starting_kit(&mut items);
    assert_eq!(protection(&items, &gear), 0.0);
    let coat = items.make(items.kind("Leather coat"));
    grid.insert(&items.catalogue, coat, None).unwrap();
    wear(&items, &mut grid, &mut gear, coat.id).unwrap();
    assert!((protection(&items, &gear) - 0.25).abs() < 1e-6);
}

#[test]
fn spending_what_the_bar_points_at_empties_its_slot() {
    let mut items = Items::default();
    let (Backpack(mut grid), _, Bar(mut bar)) = starting_kit(&mut items);
    let builder = bar.get(0).unwrap();
    grid.remove(builder).unwrap();
    bar.tidy(&grid);
    assert_eq!(bar.get(0), None);
    assert_eq!(holding(&items, &grid, &bar), None);
}
