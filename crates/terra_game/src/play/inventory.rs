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

pub use super::kinds::Cut;
use super::kinds::{self, ItemDef, ToolDef, WornDef};
use super::{IsLocalPlayer, Player};
use bevy::{ecs::system::SystemParam, prelude::*};
use terra_items::{
    ActionBar, Catalogue, Equipment, GearSlot, Grid, IdSource, Item, ItemId, ItemKind, KindDef,
    Refused, Size,
};

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

/// What cutting something takes and gives.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cuttable {
    pub what: Cut,
    /// So many of a kind of item.
    pub gives: (ItemKind, u32),
    /// How long it takes with a tool of ordinary speed, in seconds.
    pub seconds: f32,
}

/// How many slots the bar has: eight, as Valheim's has. A placeholder (D84).
pub const BAR_SLOTS: usize = 8;

/// The slots of the character sheet, by the names the items file wears things in. Which
/// slots there are is open (D64); a pack, what is worn on the head and body, and a belt
/// for a quiver (D118).
pub const SLOTS: [(GearSlot, &str); 4] = [
    (GearSlot(0), "Pack"),
    (GearSlot(1), "Head"),
    (GearSlot(2), "Body"),
    (GearSlot(3), "Belt"),
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

/// Every kind of item, read from `assets/data/items.ron` (`kinds`), and the source of new
/// items' identities.
#[derive(Resource)]
pub struct Items {
    pub catalogue: Catalogue,
    defs: Vec<ItemDef>,
    start: kinds::Start,
    ids: IdSource,
}

impl Default for Items {
    fn default() -> Self {
        let file = kinds::read();
        // A kind is its place in the file, so one kind can name another wherever it is.
        let kind_of = |name: &str| {
            file.kinds
                .iter()
                .position(|def| def.name == name)
                .map(|index| ItemKind(index as u16))
                .unwrap_or_else(|| panic!("assets/data/items.ron: there is no kind called {name}"))
        };
        let mut catalogue = Catalogue::default();
        for def in &file.kinds {
            let mut kind = KindDef::new(&def.name).size(def.size.0, def.size.1);
            if let Some(lasts) = def.lasts {
                kind = kind.lasts(lasts);
            }
            if let Some(worn) = &def.worn {
                kind = kind.gear(slot_named(&worn.slot));
                if let Some((w, h)) = worn.backpack {
                    kind = kind.backpack(w, h);
                }
                if let Some((held, most)) = &worn.holds {
                    kind = kind.holds(kind_of(held), *most);
                }
            }
            let added = catalogue.add(kind);
            debug_assert_eq!(added, kind_of(&def.name));
        }
        Self {
            catalogue,
            defs: file.kinds,
            start: file.start,
            ids: IdSource::default(),
        }
    }
}

impl Items {
    /// Takes the kinds of another, read afresh, keeping the identities already given out, so
    /// no two items ever share one.
    pub fn take_kinds(&mut self, fresh: Items) {
        self.catalogue = fresh.catalogue;
        self.defs = fresh.defs;
        self.start = fresh.start;
    }

    /// The kind called a name, if there is one.
    pub fn find(&self, name: &str) -> Option<ItemKind> {
        self.catalogue.find(name)
    }

    /// The kind called a name the game itself uses, such as one a cost or a recipe names.
    /// The tests check every such name is a kind.
    pub fn kind(&self, name: &str) -> ItemKind {
        self.find(name)
            .unwrap_or_else(|| panic!("there is no kind of item called {name}"))
    }

    /// What the items file says of a kind.
    pub fn def(&self, kind: ItemKind) -> &ItemDef {
        &self.defs[kind.0 as usize]
    }

    /// Every kind, in the file's order.
    pub fn kinds(&self) -> impl Iterator<Item = ItemKind> + '_ {
        (0..self.defs.len()).map(|index| ItemKind(index as u16))
    }

    pub fn name(&self, kind: ItemKind) -> &str {
        self.defs
            .get(kind.0 as usize)
            .map_or("?", |def| def.name.as_str())
    }

    /// Where a kind comes from, for a prompt that says what is missing.
    pub fn found(&self, kind: ItemKind) -> &str {
        self.def(kind)
            .found
            .as_deref()
            .unwrap_or("make it at a station")
    }

    /// What a kind does in hand, if it is a tool.
    pub fn tool(&self, kind: ItemKind) -> Option<&ToolDef> {
        self.defs.get(kind.0 as usize)?.tool.as_ref()
    }

    /// Where a kind is worn and what it does there, if it is worn.
    pub fn worn(&self, kind: ItemKind) -> Option<&WornDef> {
        self.defs.get(kind.0 as usize)?.worn.as_ref()
    }

    /// How much eating one of a kind fills you, out of 100: nothing if it is not food.
    pub fn food(&self, kind: ItemKind) -> f32 {
        self.defs.get(kind.0 as usize).map_or(0.0, |def| def.food)
    }

    /// What can be eaten, least filling first.
    pub fn foods(&self) -> Vec<ItemKind> {
        let mut foods: Vec<ItemKind> = self.kinds().filter(|&kind| self.food(kind) > 0.0).collect();
        foods.sort_by(|a, b| self.food(*a).total_cmp(&self.food(*b)));
        foods
    }

    /// The kinds of item something made is, and how many.
    fn made(&self, made: Made) -> (ItemKind, u32) {
        (made.kind, made.count)
    }

    /// The kind a data file names, or the reason it cannot be read.
    pub fn named(&self, name: &str, file: &str) -> ItemKind {
        self.find(name)
            .unwrap_or_else(|| panic!("{file}: there is no kind of item called {name}"))
    }

    /// A cost a data file gives by item names, as kinds.
    pub fn cost(&self, named: &[(String, u32)], file: &str) -> Vec<(ItemKind, u32)> {
        named
            .iter()
            .map(|(name, count)| (self.named(name, file), *count))
            .collect()
    }

    /// What a kind's icon is baked from (D129): a tool's held look, gear's look (on the
    /// base body), and anything else's own look; and the cells it fills, which set the
    /// icon's shape. Whether the look exists yet is for the caller to find out.
    pub fn icon_source(&self, kind: ItemKind) -> Option<(String, crate::icons::Icon)> {
        let def = self.catalogue.get(kind).ok()?;
        let id = crate::character::item_id(&def.name);
        let look = if self.tool(kind).is_some() {
            crate::character::held_look(&id)
        } else if def.gear.is_some() {
            crate::character::look(&id)
        } else {
            crate::character::item_look(&id)
        };
        Some((id, crate::icons::Icon::new(look, (def.size.w, def.size.h))))
    }

    /// Makes an item of a kind found by its name, as when a saved world is loaded.
    pub fn make_named(&mut self, name: &str) -> Option<Item> {
        let kind = self.catalogue.find(name)?;
        Some(self.make(kind))
    }

    pub fn make(&mut self, kind: ItemKind) -> Item {
        self.ids.make(kind)
    }

    fn wanted(&self, cost: &Cost, times: u32) -> Vec<(ItemKind, u32)> {
        cost.iter()
            .map(|&(kind, each)| (kind, each * times))
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
    items
        .wanted(cost, times)
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
fn spend(
    grid: &mut Grid,
    bar: &mut ActionBar,
    gear: &mut Equipment,
    wanted: &[(ItemKind, u32)],
) -> bool {
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
fn receive_kind(
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

/// Wears everything worn that wears, by so much, as a blow taken does (D99). Returns the
/// kinds that broke.
pub fn wear_worn(items: &Items, gear: &mut Equipment, amount: u16) -> Vec<ItemKind> {
    let slots: Vec<GearSlot> = gear.iter().map(|(slot, _)| slot).collect();
    slots
        .into_iter()
        .filter_map(|slot| {
            let item = gear.worn(slot)?;
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

/// Whether the inventory is open on screen.
#[derive(Resource, Default)]
pub struct InventoryScreen {
    pub open: bool,
    /// The station being used, if the screen was opened at one.
    pub at: Option<(Entity, super::crafting::Station)>,
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

/// What players carry, for a system to read or change: `of` a player, or `mine` for this
/// machine's own, as a screen shows it.
#[derive(SystemParam)]
pub struct Belongings<'w, 's> {
    items: ResMut<'w, Items>,
    carried:
        Query<'w, 's, (&'static mut Backpack, &'static mut Bar, &'static mut Gear), With<Player>>,
    local: Query<'w, 's, Entity, IsLocalPlayer>,
    gained: MessageWriter<'w, Gained>,
    broke: MessageWriter<'w, Broke>,
    spilled: MessageWriter<'w, Spilled>,
}

/// What was given to the player but did not fit: so many of a kind, which drop to the
/// ground at their feet rather than being lost (`pile`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Spilled {
    pub player: Entity,
    pub kind: ItemKind,
    pub count: u32,
}

impl<'w> Belongings<'w, '_> {
    pub fn items(&self) -> &Items {
        &self.items
    }

    /// This machine's player, if there is one yet.
    pub fn local_player(&self) -> Option<Entity> {
        self.local.single().ok()
    }

    /// What a player carries, if they are a player.
    pub fn of(&mut self, player: Entity) -> Option<Inventory<'_, 'w>> {
        let (backpack, bar, gear) = self.carried.get_mut(player).ok()?;
        Some(Inventory {
            player,
            items: &mut self.items,
            backpack,
            bar,
            gear,
            gained: &mut self.gained,
            broke: &mut self.broke,
            spilled: &mut self.spilled,
        })
    }

    /// What this machine's player carries.
    pub fn mine(&mut self) -> Option<Inventory<'_, 'w>> {
        let player = self.local_player()?;
        self.of(player)
    }
}

/// What one player carries, to read or change. Every change either applies in full or is
/// refused (`terra_items`), and what comes in or breaks is told as a message naming them.
pub struct Inventory<'a, 'w> {
    player: Entity,
    items: &'a mut Items,
    backpack: Mut<'a, Backpack>,
    bar: Mut<'a, Bar>,
    gear: Mut<'a, Gear>,
    gained: &'a mut MessageWriter<'w, Gained>,
    broke: &'a mut MessageWriter<'w, Broke>,
    spilled: &'a mut MessageWriter<'w, Spilled>,
}

impl Inventory<'_, '_> {
    /// Whose it is.
    pub fn player(&self) -> Entity {
        self.player
    }

    pub fn items(&self) -> &Items {
        self.items
    }

    pub fn backpack(&self) -> &Grid {
        &self.backpack.0
    }

    pub fn bar(&self) -> &ActionBar {
        &self.bar.0
    }

    pub fn gear(&self) -> &Equipment {
        &self.gear.0
    }

    /// How much of a blow what is worn takes, out of one.
    pub fn protection(&self) -> f32 {
        protection(self.items, self.gear())
    }

    /// The kind of tool in hand, if it is not broken.
    pub fn holding(&self) -> Option<ItemKind> {
        holding(self.items, self.backpack(), self.bar())
    }

    /// What the tool in hand does, if there is one and it is not broken.
    pub fn tool_in_hand(&self) -> Option<&ToolDef> {
        self.items.tool(self.holding()?)
    }

    /// The tool in hand if it is broken, so it can be said why it does nothing.
    pub fn broken_in_hand(&self) -> Option<ItemKind> {
        let item = self.bar().in_hand(self.backpack())?;
        let def = self.items.catalogue.get(item.kind).ok()?;
        (item.broken(def) && self.items.tool(item.kind).is_some()).then_some(item.kind)
    }

    /// Wears the tool in hand by so much use (D99), saying so if that broke it.
    pub fn wear_in_hand(&mut self, amount: u16) {
        let Some(item) = self.bar().in_hand(self.backpack()) else {
            return;
        };
        let broke = self.backpack.0.wear(&self.items.catalogue, item.id, amount);
        if broke == Ok(true) {
            self.broke.write(Broke {
                player: self.player,
                kind: item.kind,
            });
        }
    }

    /// Wears everything worn that wears, by so much: a blow taken (D99).
    pub fn wear_worn(&mut self, amount: u16) {
        for kind in wear_worn(self.items, &mut self.gear.0, amount) {
            self.broke.write(Broke {
                player: self.player,
                kind,
            });
        }
    }

    /// Mends everything carried or worn that `mends` says can be mended here, returning
    /// the kinds mended (D99).
    pub fn mend(&mut self, mends: impl Fn(ItemKind) -> bool) -> Vec<ItemKind> {
        let mut mended = Vec::new();
        let worn_out: Vec<(ItemId, ItemKind)> = self
            .backpack()
            .iter()
            .filter(|placed| placed.item.wear > 0 && mends(placed.item.kind))
            .map(|placed| (placed.item.id, placed.item.kind))
            .collect();
        for (id, kind) in worn_out {
            if self.backpack.0.set_wear(id, 0).is_ok() {
                mended.push(kind);
            }
        }
        let slots: Vec<(GearSlot, ItemKind)> = self
            .gear()
            .iter()
            .filter_map(|(slot, item)| {
                item.filter(|item| item.wear > 0 && mends(item.kind))
                    .map(|item| (slot, item.kind))
            })
            .collect();
        for (slot, kind) in slots {
            if self.gear.0.set_wear(slot, 0).is_ok() {
                mended.push(kind);
            }
        }
        mended
    }

    /// How many of a kind of item are carried: in the backpack, and in what is worn, such
    /// as a belt quiver's arrows.
    pub fn count(&self, kind: ItemKind) -> u32 {
        self.backpack().count(kind) + self.gear().stacked(kind)
    }

    pub fn missing(&self, cost: &Cost, times: u32) -> Option<(ItemKind, u32)> {
        missing(self.items, self.backpack(), self.gear(), cost, times)
    }

    pub fn room_for(&self, cost: &Cost, times: u32) -> bool {
        room_for(self.items, self.backpack(), cost, times)
    }

    /// Takes one of the most filling food carried, if there is any, and says which.
    pub fn eat_best(&mut self) -> Option<ItemKind> {
        let food = self
            .items
            .foods()
            .into_iter()
            .rev()
            .find(|&food| self.count(food) > 0)?;
        self.eat(food).then_some(food)
    }

    /// Takes one of a food, if it is carried. Returns whether it was.
    pub fn eat(&mut self, food: ItemKind) -> bool {
        let taken = self.backpack.0.take(&[(food, 1)]).is_ok();
        self.bar.0.tidy(&self.backpack.0);
        taken
    }

    /// Empties the backpack into a bag of the same size, as when you go down (D84). The
    /// bar keeps pointing at what was on it, so taking the bag back puts the tools back
    /// in reach.
    pub fn empty_into_bag(&mut self) -> Grid {
        empty_into_bag(self.items, &mut self.backpack.0)
    }

    /// Takes back what fits from a bag. Returns how many things came back.
    pub fn take_from(&mut self, bag: &mut Grid) -> usize {
        let backpack = &mut self.backpack;
        let ids: Vec<(ItemId, ItemKind)> = bag
            .iter()
            .map(|placed| (placed.item.id, placed.item.kind))
            .collect();
        let taken: Vec<ItemKind> = ids
            .into_iter()
            .filter(|&(id, _)| {
                terra_items::transfer(&self.items.catalogue, bag, &mut backpack.0, id, None).is_ok()
            })
            .map(|(_, kind)| kind)
            .collect();
        for &kind in &taken {
            self.tell(kind, 1);
        }
        taken.len()
    }

    /// Puts what a station made where it goes, all or nothing: into what is worn that holds
    /// it, such as arrows into a belt quiver, then the backpack.
    pub fn receive(&mut self, made: Made) -> bool {
        let (kind, count) = self.items.made(made);
        let received = self.receive_quietly(made);
        if received {
            self.tell(kind, count);
        }
        received
    }

    fn receive_quietly(&mut self, made: Made) -> bool {
        let (kind, count) = self.items.made(made);
        let mut trial = self.gear.0.clone();
        let worn = trial.stow(kind, count);
        if worn == count
            || receive_kind(
                self.items,
                &mut self.backpack.0,
                &mut self.bar.0,
                kind,
                count - worn,
            )
        {
            self.gear.0 = trial;
            return true;
        }
        false
    }

    /// Whether one more of a kind of item would fit in the backpack.
    pub fn room_for_one(&self, kind: ItemKind) -> bool {
        let size = self
            .items
            .catalogue
            .get(kind)
            .map_or(Size::ONE, |def| def.size);
        self.backpack().space_for(size).is_some()
    }

    /// Spends so many of a kind, if there are that many. Returns whether it did.
    pub fn spend(&mut self, kind: ItemKind, count: u32) -> bool {
        spend(
            &mut self.backpack.0,
            &mut self.bar.0,
            &mut self.gear.0,
            &[(kind, count)],
        )
    }

    pub fn pay(&mut self, cost: &Cost, times: u32) -> bool {
        pay(
            self.items,
            &mut self.backpack.0,
            &mut self.bar.0,
            &mut self.gear.0,
            cost,
            times,
        )
    }

    /// Gives back what a cost so many times over is made of, if all of it fits. Returns
    /// whether it did.
    pub fn refund(&mut self, cost: &Cost, times: u32) {
        for (kind, count) in self.items.wanted(cost, times) {
            self.give(kind, count);
        }
    }

    /// Puts so many of a kind of item where it goes, as many as fit: into what is worn that
    /// holds it, such as arrows into a belt quiver, then the backpack. Returns how many did.
    pub fn give(&mut self, kind: ItemKind, count: u32) -> u32 {
        let worn = self.gear.0.stow(kind, count);
        let given = worn + give(self.items, &mut self.backpack.0, kind, count - worn);
        self.tell(kind, given);
        // What does not fit drops at the player's feet.
        if given < count {
            self.spilled.write(Spilled {
                player: self.player,
                kind,
                count: count - given,
            });
        }
        given
    }

    fn tell(&mut self, kind: ItemKind, count: u32) {
        if count > 0 {
            self.gained.write(Gained {
                player: self.player,
                kind,
                count,
            });
        }
    }

    pub fn pick(&mut self, slot: usize) {
        if self.bar().picked() != slot {
            self.bar.0.pick(slot);
        }
    }

    /// Wears an item from the backpack.
    pub fn wear(&mut self, id: ItemId) -> Result<(), Refused> {
        let worn = wear(self.items, &mut self.backpack.0, &mut self.gear.0, id);
        self.bar.0.tidy(&self.backpack.0);
        worn
    }

    /// Takes off what is in a slot, into the backpack.
    pub fn take_off(&mut self, slot: GearSlot) -> Result<(), Refused> {
        take_off(self.items, &mut self.backpack.0, &mut self.gear.0, slot)
    }

    /// Points a slot of the bar at a tool in the backpack. Only tools go on the bar.
    pub fn put_on_bar(&mut self, slot: usize, id: ItemId) {
        let is_tool = self
            .backpack()
            .get(id)
            .is_some_and(|placed| self.items.tool(placed.item.kind).is_some());
        if is_tool {
            self.bar.0.set(slot, Some(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
