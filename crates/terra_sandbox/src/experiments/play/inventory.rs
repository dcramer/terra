//! What the player carries, on the rules in `terra_items` (docs/items.md, D84): a
//! backpack that is a grid where each item fills its cells and nothing stacks, a worn
//! pack that sets the backpack's size, and an action bar that points at items in the
//! backpack. Whatever the picked slot points at is in hand.
//!
//! The containers are components on the player (D83). Raw items are what things are
//! built from: the builder spends them, and taking a thing apart gives them back, if
//! there is room for them.
//!
//! The content is a stand-in for a design nobody has made (docs/construction.md, D55):
//! the list of raw items, what you start with, what things cost, the sizes, and that
//! everything is refunded in full.

use bevy::{ecs::system::SystemParam, prelude::*};
use terra_items::{
    ActionBar, Catalogue, Equipment, GearSlot, Grid, IdSource, Item, ItemId, ItemKind, KindDef,
    Size,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Raw {
    Metal,
    Glass,
    Wood,
    Cloth,
    Parts,
}

impl Raw {
    pub const ALL: [Raw; 5] = [Raw::Metal, Raw::Glass, Raw::Wood, Raw::Cloth, Raw::Parts];

    pub fn name(self) -> &'static str {
        match self {
            Raw::Metal => "Metal",
            Raw::Glass => "Glass",
            Raw::Wood => "Wood",
            Raw::Cloth => "Cloth",
            Raw::Parts => "Parts",
        }
    }
}

/// What one of something costs: so many of each raw item.
pub type Cost = &'static [(Raw, u32)];

/// A cost as a player would read it: "2 Wood, 3 Cloth".
pub fn describe(cost: Cost, times: u32) -> String {
    cost.iter()
        .map(|&(raw, each)| format!("{} {}", each * times, raw.name()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The sorts of thing that can be cut. Tools differ in which they cut and how fast.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    Tree,
    Rock,
    Plant,
}

/// What cutting something takes and gives.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cuttable {
    pub what: Cut,
    pub gives: (Raw, u32),
    /// How long it takes with a tool of ordinary speed, in seconds.
    pub seconds: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    /// Builds and takes apart.
    Builder,
    /// Cuts down trees and breaks up rocks, for the raw items in them.
    Cutter,
}

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Tool::Builder => "Builder",
            Tool::Cutter => "Cutter",
        }
    }

    /// How fast this tool cuts a sort of thing: 1 takes the time the thing itself says,
    /// 2 half of it. `None` if the tool cannot cut it at all. An axe that is quick on
    /// trees and useless on rock would be one more line here.
    pub fn cuts(self, what: Cut) -> Option<f32> {
        match (self, what) {
            (Tool::Cutter, _) => Some(1.0),
            (Tool::Builder, _) => None,
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Tool::Builder => "Builds rooms, walls and items from what you carry",
            Tool::Cutter => "Cuts trees, rocks and bushes into raw items",
        }
    }
}

pub const BAR_SLOTS: usize = 6;

/// The equipment slot a pack is worn in.
const PACK: GearSlot = GearSlot(0);

/// The backpack with no pack worn. How big the first backpack is is open (docs/items.md);
/// Subnautica 2 starts with about twenty cells.
const BARE: Size = Size { w: 6, h: 4 };

/// What a player starts the demo with, besides the tools and the pack: enough to build a
/// room or two, leaving room in the backpack to gather more.
const STARTING: [(Raw, u32); 5] = [
    (Raw::Metal, 24),
    (Raw::Glass, 6),
    (Raw::Wood, 12),
    (Raw::Cloth, 6),
    (Raw::Parts, 4),
];

/// Every kind of item in the demo, and which is which.
#[derive(Resource)]
pub struct Items {
    pub catalogue: Catalogue,
    raw: [ItemKind; Raw::ALL.len()],
    tools: [(Tool, ItemKind); 2],
    pack: ItemKind,
    ids: IdSource,
}

impl Default for Items {
    fn default() -> Self {
        let mut catalogue = Catalogue::default();
        let raw = Raw::ALL.map(|raw| catalogue.add(KindDef::new(raw.name())));
        let tools = [Tool::Builder, Tool::Cutter]
            .map(|tool| (tool, catalogue.add(KindDef::new(tool.name()).size(2, 1))));
        let pack = catalogue.add(
            KindDef::new("Large pack")
                .size(2, 2)
                .gear(PACK)
                .backpack(10, 8),
        );
        Self {
            catalogue,
            raw,
            tools,
            pack,
            ids: IdSource::default(),
        }
    }
}

impl Items {
    pub fn kind(&self, raw: Raw) -> ItemKind {
        self.raw[raw as usize]
    }

    pub fn raw(&self, kind: ItemKind) -> Option<Raw> {
        Raw::ALL.into_iter().find(|&raw| self.kind(raw) == kind)
    }

    pub fn tool(&self, kind: ItemKind) -> Option<Tool> {
        self.tools
            .iter()
            .find(|(_, each)| *each == kind)
            .map(|(tool, _)| *tool)
    }

    pub fn name(&self, kind: ItemKind) -> &str {
        self.catalogue
            .get(kind)
            .map_or("?", |def| def.name.as_str())
    }

    fn make(&mut self, kind: ItemKind) -> Item {
        self.ids.make(kind)
    }

    fn wanted(&self, cost: Cost, times: u32) -> Vec<(ItemKind, u32)> {
        cost.iter()
            .map(|&(raw, each)| (self.kind(raw), each * times))
            .collect()
    }
}

/// The backpack: a grid of cells.
#[derive(Component, Clone, Debug)]
pub struct Backpack(pub Grid);

/// What is worn.
#[derive(Component, Clone, Debug)]
pub struct Gear(pub Equipment);

/// The action bar.
#[derive(Component, Clone, Debug)]
pub struct Bar(pub ActionBar);

/// What a player starts with: a large pack worn, both tools on the bar, and some raw
/// items.
pub fn starting_kit(items: &mut Items) -> (Backpack, Gear, Bar) {
    let mut gear = Equipment::new(&[PACK]);
    let mut grid = Grid::new(BARE.w, BARE.h);
    let pack = items.make(items.pack);
    grid.insert(&items.catalogue, pack, None)
        .expect("a pack fits an empty backpack");
    terra_items::equip(&items.catalogue, &mut grid, &mut gear, BARE, pack.id)
        .expect("a pack can be worn");
    let mut bar = ActionBar::new(BAR_SLOTS);
    for (slot, (_, kind)) in items.tools.into_iter().enumerate() {
        let tool = items.make(kind);
        grid.insert(&items.catalogue, tool, None)
            .expect("the tools fit the backpack");
        bar.set(slot, Some(tool.id));
    }
    for (raw, count) in STARTING {
        give(items, &mut grid, raw, count);
    }
    (Backpack(grid), Gear(gear), Bar(bar))
}

/// The tool in hand: whatever the picked slot of the bar points at.
pub fn holding(items: &Items, grid: &Grid, bar: &ActionBar) -> Option<Tool> {
    items.tool(bar.in_hand(grid)?.kind)
}

/// The first raw item there is not enough of to pay a cost so many times over, and how
/// many more are needed.
pub fn missing(items: &Items, grid: &Grid, cost: Cost, times: u32) -> Option<(Raw, u32)> {
    cost.iter().find_map(|&(raw, each)| {
        let (wanted, have) = (each * times, grid.count(items.kind(raw)));
        (have < wanted).then(|| (raw, wanted - have))
    })
}

/// Pays a cost so many times over, if it can be paid in full. Returns whether it was.
pub fn pay(items: &Items, grid: &mut Grid, bar: &mut ActionBar, cost: Cost, times: u32) -> bool {
    let paid = grid.take(&items.wanted(cost, times)).is_ok();
    bar.tidy(grid);
    paid
}

/// Whether what a cost so many times over is made of would all fit in the backpack.
pub fn room_for(items: &Items, grid: &Grid, cost: Cost, times: u32) -> bool {
    let mut trial = grid.clone();
    let mut id = u64::MAX;
    items.wanted(cost, times).into_iter().all(|(kind, count)| {
        (0..count).all(|_| {
            id -= 1;
            let item = Item {
                id: ItemId(id),
                kind,
            };
            trial.insert(&items.catalogue, item, None).is_ok()
        })
    })
}

/// Puts so many of a raw item in the backpack, as many as fit. Returns how many did.
pub fn give(items: &mut Items, grid: &mut Grid, raw: Raw, count: u32) -> u32 {
    let kind = items.kind(raw);
    let size = items.catalogue.get(kind).map_or(Size::ONE, |def| def.size);
    let mut given = 0;
    while given < count && grid.space_for(size).is_some() {
        let item = items.make(kind);
        if grid.insert(&items.catalogue, item, None).is_err() {
            break;
        }
        given += 1;
    }
    given
}

/// Whether the inventory is open on screen.
#[derive(Resource, Default)]
pub struct InventoryScreen {
    pub open: bool,
}

/// What the player carries, for a system to read or change.
#[derive(SystemParam)]
pub struct Belongings<'w, 's> {
    items: ResMut<'w, Items>,
    player: Single<'w, 's, (&'static mut Backpack, &'static mut Bar, &'static Gear)>,
}

impl Belongings<'_, '_> {
    pub fn items(&self) -> &Items {
        &self.items
    }

    pub fn backpack(&self) -> &Grid {
        &self.player.0.0
    }

    pub fn bar(&self) -> &ActionBar {
        &self.player.1.0
    }

    pub fn gear(&self) -> &Equipment {
        &self.player.2.0
    }

    pub fn holding(&self) -> Option<Tool> {
        holding(&self.items, self.backpack(), self.bar())
    }

    pub fn count(&self, raw: Raw) -> u32 {
        self.backpack().count(self.items.kind(raw))
    }

    pub fn missing(&self, cost: Cost, times: u32) -> Option<(Raw, u32)> {
        missing(&self.items, self.backpack(), cost, times)
    }

    pub fn room_for(&self, cost: Cost, times: u32) -> bool {
        room_for(&self.items, self.backpack(), cost, times)
    }

    /// Whether one more of a raw item would fit in the backpack.
    pub fn room_for_one(&self, raw: Raw) -> bool {
        let kind = self.items.kind(raw);
        let size = self
            .items
            .catalogue
            .get(kind)
            .map_or(Size::ONE, |def| def.size);
        self.backpack().space_for(size).is_some()
    }

    pub fn pay(&mut self, cost: Cost, times: u32) -> bool {
        let (backpack, bar, _) = &mut *self.player;
        pay(&self.items, &mut backpack.0, &mut bar.0, cost, times)
    }

    /// Gives back what a cost so many times over is made of, if all of it fits. Returns
    /// whether it did.
    pub fn refund(&mut self, cost: Cost, times: u32) -> bool {
        if !self.room_for(cost, times) {
            return false;
        }
        for &(raw, each) in cost {
            self.give(raw, each * times);
        }
        true
    }

    /// Puts so many of a raw item in the backpack, as many as fit. Returns how many did.
    pub fn give(&mut self, raw: Raw, count: u32) -> u32 {
        let backpack = &mut self.player.0;
        give(&mut self.items, &mut backpack.0, raw, count)
    }

    pub fn pick(&mut self, slot: usize) {
        if self.bar().picked() != slot {
            self.player.1.0.pick(slot);
        }
    }

    /// Points a slot of the bar at a tool in the backpack. Only tools go on the bar.
    pub fn put_on_bar(&mut self, slot: usize, id: ItemId) {
        let is_tool = self
            .backpack()
            .get(id)
            .is_some_and(|placed| self.items.tool(placed.item.kind).is_some());
        if is_tool {
            self.player.1.0.set(slot, Some(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COUCH: Cost = &[(Raw::Wood, 2), (Raw::Cloth, 3)];

    #[test]
    fn the_starting_kit_has_both_tools_on_the_bar_and_room_to_spare() {
        let mut items = Items::default();
        let (Backpack(grid), Gear(gear), Bar(mut bar)) = starting_kit(&mut items);
        assert_eq!(gear.worn(PACK).map(|item| item.kind), Some(items.pack));
        assert_eq!(grid.size(), Size { w: 10, h: 8 });
        assert_eq!(holding(&items, &grid, &bar), Some(Tool::Builder));
        bar.pick(1);
        assert_eq!(holding(&items, &grid, &bar), Some(Tool::Cutter));
        for (raw, count) in STARTING {
            assert_eq!(grid.count(items.kind(raw)), count);
        }
        assert!(grid.space_for(Size::ONE).is_some());
    }

    #[test]
    fn a_cost_is_paid_in_full_or_not_at_all() {
        let mut items = Items::default();
        let (Backpack(mut grid), _, Bar(mut bar)) = starting_kit(&mut items);
        let count = |grid: &Grid, raw| grid.count(items.kind(raw));
        let (wood, cloth) = (count(&grid, Raw::Wood), count(&grid, Raw::Cloth));
        assert!(pay(&items, &mut grid, &mut bar, COUCH, 2));
        assert_eq!(count(&grid, Raw::Wood), wood - 4);
        assert_eq!(count(&grid, Raw::Cloth), cloth - 6);
        assert_eq!(
            missing(&items, &grid, COUCH, 1),
            Some((Raw::Cloth, 3)),
            "all the cloth is spent"
        );
        let before = grid.clone();
        assert!(!pay(&items, &mut grid, &mut bar, COUCH, 1));
        assert_eq!(grid, before);
        assert_eq!(describe(COUCH, 2), "4 Wood, 6 Cloth");
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
        assert!(room_for(&items, &grid, COUCH, 1));
        assert_eq!(give(&mut items, &mut grid, Raw::Wood, free + 5), free);
        assert!(grid.space_for(Size::ONE).is_none());
        assert!(!room_for(&items, &grid, COUCH, 1));
        assert_eq!(give(&mut items, &mut grid, Raw::Wood, 1), 0);
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

    #[test]
    fn a_tool_says_what_it_can_cut() {
        for what in [Cut::Tree, Cut::Rock, Cut::Plant] {
            assert_eq!(Tool::Cutter.cuts(what), Some(1.0));
            assert_eq!(Tool::Builder.cuts(what), None);
        }
    }
}
