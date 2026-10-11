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
    Refused, Size,
};

/// Materials and food: what things are built and made from, and what is eaten.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Raw {
    Wood,
    Stone,
    Fibre,
    Hide,
    Ore,
    Iron,
    Meat,
    Berries,
    CookedMeat,
    /// Shot from a bow, and used up. One to a cell, like everything (D84).
    Arrow,
}

impl Raw {
    /// Where it comes from, for a prompt that says what is missing.
    pub fn where_found(self) -> &'static str {
        match self {
            Raw::Wood => "fell trees with an axe",
            Raw::Stone => "break rocks with a pickaxe",
            Raw::Fibre => "cut plants with a knife",
            Raw::Hide | Raw::Meat => "butcher a deer",
            Raw::Ore => "break the rusty rocks with a pickaxe",
            Raw::Iron => "smelt ore at a smelter",
            Raw::Berries => "pick them from berry bushes",
            Raw::CookedMeat => "cook meat at a campfire",
            Raw::Arrow => "make them at a workbench",
        }
    }

    pub const ALL: [Raw; 10] = [
        Raw::Wood,
        Raw::Stone,
        Raw::Fibre,
        Raw::Hide,
        Raw::Ore,
        Raw::Iron,
        Raw::Meat,
        Raw::Berries,
        Raw::CookedMeat,
        Raw::Arrow,
    ];

    /// What can be eaten, least filling first.
    pub const FOOD: [Raw; 3] = [Raw::Berries, Raw::Meat, Raw::CookedMeat];

    pub fn name(self) -> &'static str {
        match self {
            Raw::Wood => "Wood",
            Raw::Stone => "Stone",
            Raw::Fibre => "Fibre",
            Raw::Hide => "Hide",
            Raw::Ore => "Ore",
            Raw::Iron => "Iron",
            Raw::Meat => "Meat",
            Raw::Berries => "Berries",
            Raw::CookedMeat => "Cooked meat",
            Raw::Arrow => "Arrow",
        }
    }
}

/// What one of something costs: so many of each raw item.
pub type Cost = &'static [(Raw, u32)];

/// A cost as a player would read it: "2 Wood, 3 Fibre".
pub fn describe(cost: Cost, times: u32) -> String {
    cost.iter()
        .map(|&(raw, each)| format!("{} {}", each * times, raw.name()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The sorts of thing that can be cut. Each is worked by one family of tool, and some
/// only by a tool of a better material (D96).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    Tree,
    /// A tree too great for a stone axe.
    GreatTree,
    Rock,
    /// Rock with ore in it.
    Ore,
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

/// What is held in the hand to do a job. One tool to a job, none that does everything
/// (D96); a tool's material decides what it can work.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    /// Builds and takes apart.
    Hammer,
    StoneAxe,
    StonePickaxe,
    Knife,
    IronAxe,
    IronPickaxe,
    /// A weapon, for hunting and for fighting.
    Spear,
    /// A weapon at range, shooting arrows from the backpack.
    Bow,
}

impl Tool {
    pub const ALL: [Tool; 8] = [
        Tool::Hammer,
        Tool::StoneAxe,
        Tool::StonePickaxe,
        Tool::Knife,
        Tool::IronAxe,
        Tool::IronPickaxe,
        Tool::Spear,
        Tool::Bow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Hammer => "Hammer",
            Tool::StoneAxe => "Stone axe",
            Tool::StonePickaxe => "Stone pickaxe",
            Tool::Knife => "Knife",
            Tool::IronAxe => "Iron axe",
            Tool::IronPickaxe => "Iron pickaxe",
            Tool::Spear => "Spear",
            Tool::Bow => "Bow",
        }
    }

    /// Whether this is the kind of tool for a sort of thing: an axe for trees, whatever
    /// its material.
    pub fn works(self, what: Cut) -> bool {
        matches!(
            (self, what),
            (Tool::StoneAxe | Tool::IronAxe, Cut::Tree | Cut::GreatTree)
                | (Tool::StonePickaxe | Tool::IronPickaxe, Cut::Rock | Cut::Ore)
                | (Tool::Knife, Cut::Plant)
        )
    }

    /// How fast this tool cuts a sort of thing: 1 takes the time the thing itself says,
    /// 2 half of it. `None` if it cannot: the wrong tool, or too poor a material.
    pub fn cuts(self, what: Cut) -> Option<f32> {
        match (self, what) {
            (Tool::StoneAxe, Cut::Tree) => Some(1.0),
            (Tool::IronAxe, Cut::Tree) => Some(1.8),
            (Tool::IronAxe, Cut::GreatTree) => Some(1.0),
            (Tool::StonePickaxe, Cut::Rock) => Some(1.0),
            (Tool::StonePickaxe, Cut::Ore) => Some(0.6),
            (Tool::IronPickaxe, Cut::Rock) => Some(1.8),
            (Tool::IronPickaxe, Cut::Ore) => Some(1.5),
            (Tool::Knife, Cut::Plant) => Some(1.0),
            _ => None,
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Tool::Hammer => "Builds rooms, walls and items from what you carry",
            Tool::StoneAxe => "Fells ordinary trees for wood",
            Tool::StonePickaxe => "Breaks rock for stone, and ore slowly",
            Tool::Knife => "Cuts plants for fibre",
            Tool::IronAxe => "Fells any tree, the great ones too",
            Tool::IronPickaxe => "Breaks rock and ore quickly",
            Tool::Spear => "A hunting spear: hits hardest and reaches furthest",
            Tool::Bow => "Shoots arrows: hold to draw, let go to loose",
        }
    }
}

pub const BAR_SLOTS: usize = 6;

/// The slots of the character sheet. Which slots there are is open (D64); a pack, and
/// what is worn on the head and body.
pub const SLOTS: [(GearSlot, &str); 3] = [
    (GearSlot(0), "Pack"),
    (GearSlot(1), "Head"),
    (GearSlot(2), "Body"),
];
const PACK: GearSlot = SLOTS[0].0;

/// What is worn: packs, and clothing that protects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Apparel {
    SmallPack,
    LargePack,
    LeatherCap,
    LeatherCoat,
}

impl Apparel {
    pub const ALL: [Apparel; 4] = [
        Apparel::SmallPack,
        Apparel::LargePack,
        Apparel::LeatherCap,
        Apparel::LeatherCoat,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Apparel::SmallPack => "Small pack",
            Apparel::LargePack => "Large pack",
            Apparel::LeatherCap => "Leather cap",
            Apparel::LeatherCoat => "Leather coat",
        }
    }

    /// What it does, for a station's screen.
    pub fn about(self) -> &'static str {
        match self {
            Apparel::SmallPack => "Worn, makes the backpack 6 by 5",
            Apparel::LargePack => "Worn, makes the backpack 7 by 6",
            Apparel::LeatherCap => "Worn, takes a tenth of every blow",
            Apparel::LeatherCoat => "Worn, takes a quarter of every blow",
        }
    }

    /// How much of a blow it takes when worn, out of one.
    pub fn protection(self) -> f32 {
        match self {
            Apparel::SmallPack | Apparel::LargePack => 0.0,
            Apparel::LeatherCap => 0.1,
            Apparel::LeatherCoat => 0.25,
        }
    }

    fn def(self) -> KindDef {
        match self {
            Apparel::SmallPack => KindDef::new(self.name())
                .size(2, 2)
                .gear(PACK)
                .backpack(6, 5),
            Apparel::LargePack => KindDef::new(self.name())
                .size(2, 2)
                .gear(PACK)
                .backpack(7, 6),
            Apparel::LeatherCap => KindDef::new(self.name()).size(1, 1).gear(SLOTS[1].0),
            Apparel::LeatherCoat => KindDef::new(self.name()).size(2, 2).gear(SLOTS[2].0),
        }
    }
}

/// What a recipe makes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Made {
    Raw(Raw, u32),
    Tool(Tool),
    Apparel(Apparel),
}

impl Made {
    pub fn name(self) -> String {
        match self {
            Made::Raw(raw, 1) => raw.name().to_string(),
            Made::Raw(raw, count) => format!("{count} {}", raw.name()),
            Made::Tool(tool) => tool.name().to_string(),
            Made::Apparel(apparel) => apparel.name().to_string(),
        }
    }
}

/// The backpack with no pack worn: about Subnautica 2's twenty cells. How big the first
/// backpack is is open (docs/items.md).
const BARE: Size = Size { w: 5, h: 4 };

/// What a player starts with besides tools and a pack: nothing. Everything else is
/// taken from the world (the MVP in docs/prototypes.md).
const STARTING: [(Raw, u32); 0] = [];

/// The tools a player starts with, in the order they go on the bar.
const STARTING_TOOLS: [Tool; 4] = [
    Tool::Hammer,
    Tool::StoneAxe,
    Tool::StonePickaxe,
    Tool::Knife,
];

/// Every kind of item in the demo, and which is which.
#[derive(Resource)]
pub struct Items {
    pub catalogue: Catalogue,
    raw: [ItemKind; Raw::ALL.len()],
    tools: [(Tool, ItemKind); Tool::ALL.len()],
    apparel: [(Apparel, ItemKind); Apparel::ALL.len()],
    ids: IdSource,
}

impl Default for Items {
    fn default() -> Self {
        let mut catalogue = Catalogue::default();
        let raw = Raw::ALL.map(|raw| catalogue.add(KindDef::new(raw.name())));
        let tools =
            Tool::ALL.map(|tool| (tool, catalogue.add(KindDef::new(tool.name()).size(2, 1))));
        let apparel = Apparel::ALL.map(|apparel| (apparel, catalogue.add(apparel.def())));
        Self {
            catalogue,
            raw,
            tools,
            apparel,
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

    pub fn tool_kind(&self, tool: Tool) -> ItemKind {
        self.tools
            .iter()
            .find(|(each, _)| *each == tool)
            .map(|(_, kind)| *kind)
            .expect("every tool has a kind")
    }

    pub fn tool(&self, kind: ItemKind) -> Option<Tool> {
        self.tools
            .iter()
            .find(|(_, each)| *each == kind)
            .map(|(tool, _)| *tool)
    }

    pub fn apparel(&self, kind: ItemKind) -> Option<Apparel> {
        self.apparel
            .iter()
            .find(|(_, each)| *each == kind)
            .map(|(apparel, _)| *apparel)
    }

    pub fn apparel_kind(&self, apparel: Apparel) -> ItemKind {
        self.apparel
            .iter()
            .find(|(each, _)| *each == apparel)
            .map(|(_, kind)| *kind)
            .expect("all apparel has a kind")
    }

    /// The kinds of item something made is, and how many of each.
    fn made(&self, made: Made) -> (ItemKind, u32) {
        match made {
            Made::Raw(raw, count) => (self.kind(raw), count),
            Made::Tool(tool) => (self.tool_kind(tool), 1),
            Made::Apparel(apparel) => (self.apparel_kind(apparel), 1),
        }
    }

    pub fn name(&self, kind: ItemKind) -> &str {
        self.catalogue
            .get(kind)
            .map_or("?", |def| def.name.as_str())
    }

    /// Makes an item of a kind found by its name, as when a saved world is loaded.
    pub fn make_named(&mut self, name: &str) -> Option<Item> {
        let kind = self.catalogue.find(name)?;
        Some(self.make(kind))
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

/// A backpack with no pack worn, to fill as a saved world is loaded.
pub fn bare_backpack() -> Grid {
    Grid::new(BARE.w, BARE.h)
}

/// How much of a blow everything worn takes together, out of one, never all of it.
pub fn protection(items: &Items, gear: &Equipment) -> f32 {
    gear.iter()
        .filter_map(|(_, worn)| items.apparel(worn?.kind))
        .map(Apparel::protection)
        .sum::<f32>()
        .min(0.8)
}

/// For testing (--give): adds raw items, tools on the bar, and clothing put on, by name,
/// such as `stone`, `iron_axe` or `leather_coat`. Names for the body are left alone.
pub fn give_for_testing(
    items: &mut Items,
    grid: &mut Grid,
    gear: &mut Equipment,
    bar: &mut ActionBar,
    gives: &[(String, u32)],
) {
    let named = |name: &str, wanted: &str| name.replace(' ', "_").eq_ignore_ascii_case(wanted);
    for (name, count) in gives {
        if let Some(raw) = Raw::ALL.into_iter().find(|raw| named(raw.name(), name)) {
            let given = give(items, grid, raw, *count);
            if given < *count {
                warn!("--give: room for only {given} of {count} {}", raw.name());
            }
        } else if let Some(tool) = Tool::ALL.into_iter().find(|tool| named(tool.name(), name)) {
            let kind = items.tool_kind(tool);
            for _ in 0..*count {
                let made = items.make(kind);
                if grid.insert(&items.catalogue, made, None).is_err() {
                    warn!("--give: no room for a {}", tool.name());
                    break;
                }
                if let Some(slot) = (0..bar.len()).find(|&slot| bar.get(slot).is_none()) {
                    bar.set(slot, Some(made.id));
                }
            }
        } else if let Some(apparel) = Apparel::ALL
            .into_iter()
            .find(|each| named(each.name(), name))
        {
            let kind = items.apparel_kind(apparel);
            let made = items.make(kind);
            let worn = grid
                .insert(&items.catalogue, made, None)
                .ok()
                .and_then(|_| wear(items, grid, gear, made.id).ok());
            if worn.is_none() {
                warn!("--give: could not put on a {}", apparel.name());
            }
        } else if !["health", "fed", "stamina"].contains(&name.as_str()) {
            warn!("--give: there is no raw item, tool or clothing called {name}");
        }
    }
}

/// What a player starts with: a small pack worn, and their tools on the bar.
pub fn starting_kit(items: &mut Items) -> (Backpack, Gear, Bar) {
    let mut gear = Equipment::new(&SLOTS.map(|(slot, _)| slot));
    let mut grid = Grid::new(BARE.w, BARE.h);
    let pack = items.make(items.apparel_kind(Apparel::SmallPack));
    grid.insert(&items.catalogue, pack, None)
        .expect("a pack fits an empty backpack");
    wear(items, &mut grid, &mut gear, pack.id).expect("a pack can be worn");
    let mut bar = ActionBar::new(BAR_SLOTS);
    for (slot, tool) in STARTING_TOOLS.into_iter().enumerate() {
        let kind = items.tool_kind(tool);
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

/// Puts what a station made in the backpack, all or nothing. Returns whether it did.
pub fn receive(items: &mut Items, grid: &mut Grid, bar: &mut ActionBar, made: Made) -> bool {
    let (kind, count) = items.made(made);
    let mut trial = grid.clone();
    let mut made_ids = Vec::new();
    for _ in 0..count {
        let item = items.make(kind);
        if trial.insert(&items.catalogue, item, None).is_err() {
            return false;
        }
        made_ids.push(item.id);
    }
    *grid = trial;
    // A new tool goes on the bar's first free slot, so it is ready to pick up.
    if let (Made::Tool(_), Some(&id)) = (made, made_ids.first())
        && let Some(free) = (0..bar.len()).find(|&slot| bar.get(slot).is_none())
    {
        bar.set(free, Some(id));
    }
    true
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
    /// The station being used, if the screen was opened at one.
    pub at: Option<(Entity, super::crafting::Station)>,
}

/// What the player carries, for a system to read or change.
#[derive(SystemParam)]
pub struct Belongings<'w, 's> {
    items: ResMut<'w, Items>,
    player: Single<'w, 's, (&'static mut Backpack, &'static mut Bar, &'static mut Gear)>,
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

    /// How much of a blow what is worn takes, out of one.
    pub fn protection(&self) -> f32 {
        protection(&self.items, self.gear())
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

    /// Takes one of the most filling food carried, if there is any, and says which.
    pub fn eat_best(&mut self) -> Option<Raw> {
        let food = Raw::FOOD
            .into_iter()
            .rev()
            .find(|&food| self.count(food) > 0)?;
        self.eat(food).then_some(food)
    }

    /// Takes one of a food, if it is carried. Returns whether it was.
    pub fn eat(&mut self, food: Raw) -> bool {
        let (backpack, bar, _) = &mut *self.player;
        let taken = backpack.0.take(&[(self.items.kind(food), 1)]).is_ok();
        bar.0.tidy(&backpack.0);
        taken
    }

    /// Empties the backpack into a bag of the same size, as when you go down (D84). The
    /// bar keeps pointing at what was on it, so taking the bag back puts the tools back
    /// in reach.
    pub fn empty_into_bag(&mut self) -> Grid {
        let backpack = &mut self.player.0;
        let size = backpack.0.size();
        let mut bag = Grid::new(size.w, size.h);
        let ids: Vec<ItemId> = backpack.0.iter().map(|placed| placed.item.id).collect();
        for id in ids {
            let _ =
                terra_items::transfer(&self.items.catalogue, &mut backpack.0, &mut bag, id, None);
        }
        bag
    }

    /// Takes back what fits from a bag. Returns how many things came back.
    pub fn take_from(&mut self, bag: &mut Grid) -> usize {
        let backpack = &mut self.player.0;
        let ids: Vec<ItemId> = bag.iter().map(|placed| placed.item.id).collect();
        ids.into_iter()
            .filter(|&id| {
                terra_items::transfer(&self.items.catalogue, bag, &mut backpack.0, id, None).is_ok()
            })
            .count()
    }

    /// Makes something from what is carried; see [`craft`].
    /// Puts what a station made in the backpack, all or nothing.
    pub fn receive(&mut self, made: Made) -> bool {
        let (backpack, bar, _) = &mut *self.player;
        receive(&mut self.items, &mut backpack.0, &mut bar.0, made)
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

    /// Wears an item from the backpack.
    pub fn wear(&mut self, id: ItemId) -> Result<(), Refused> {
        let (backpack, bar, gear) = &mut *self.player;
        let worn = wear(&self.items, &mut backpack.0, &mut gear.0, id);
        bar.0.tidy(&backpack.0);
        worn
    }

    /// Takes off what is in a slot, into the backpack.
    pub fn take_off(&mut self, slot: GearSlot) -> Result<(), Refused> {
        let (backpack, _, gear) = &mut *self.player;
        take_off(&self.items, &mut backpack.0, &mut gear.0, slot)
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

    const COUCH: Cost = &[(Raw::Wood, 2), (Raw::Hide, 3)];

    #[test]
    fn the_starting_kit_has_both_tools_on_the_bar_and_room_to_spare() {
        let mut items = Items::default();
        let (Backpack(grid), Gear(gear), Bar(mut bar)) = starting_kit(&mut items);
        assert_eq!(
            gear.worn(PACK).map(|item| item.kind),
            Some(items.apparel_kind(Apparel::SmallPack))
        );
        assert_eq!(grid.size(), Size { w: 6, h: 5 });
        assert_eq!(holding(&items, &grid, &bar), Some(Tool::Hammer));
        bar.pick(1);
        assert_eq!(holding(&items, &grid, &bar), Some(Tool::StoneAxe));
        for (raw, count) in STARTING {
            assert_eq!(grid.count(items.kind(raw)), count);
        }
        for tool in STARTING_TOOLS {
            assert_eq!(grid.count(items.tool_kind(tool)), 1);
        }
        assert!(grid.space_for(Size::ONE).is_some());
    }

    #[test]
    fn a_cost_is_paid_in_full_or_not_at_all() {
        let mut items = Items::default();
        let (Backpack(mut grid), _, Bar(mut bar)) = starting_kit(&mut items);
        give(&mut items, &mut grid, Raw::Wood, 4);
        give(&mut items, &mut grid, Raw::Hide, 3);
        let count = |grid: &Grid, raw| grid.count(items.kind(raw));
        assert!(pay(&items, &mut grid, &mut bar, COUCH, 1));
        assert_eq!(count(&grid, Raw::Wood), 2);
        assert_eq!(count(&grid, Raw::Hide), 0);
        assert_eq!(
            missing(&items, &grid, COUCH, 1),
            Some((Raw::Hide, 3)),
            "all the hide is spent"
        );
        let before = grid.clone();
        assert!(!pay(&items, &mut grid, &mut bar, COUCH, 1));
        assert_eq!(grid, before);
        assert_eq!(describe(COUCH, 2), "4 Wood, 6 Hide");
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
    fn gear_comes_off_into_the_backpack_and_goes_back_on() {
        let mut items = Items::default();
        let (Backpack(mut grid), Gear(mut gear), _) = starting_kit(&mut items);
        // The pack comes off once what is carried fits the bare backpack.
        let pack = gear.worn(PACK).unwrap();
        for tool in [Tool::StonePickaxe, Tool::Knife] {
            let id = grid
                .iter()
                .find(|placed| placed.item.kind == items.tool_kind(tool))
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
        give(&mut items, &mut grid, Raw::Stone, 20);
        let before = (grid.clone(), gear.clone());
        let refused = take_off(&items, &mut grid, &mut gear, PACK).unwrap_err();
        assert!(matches!(refused, Refused::NoRoom | Refused::WouldNotFit));
        assert_eq!((grid, gear), before);
    }

    #[test]
    fn what_is_made_goes_in_the_backpack_and_a_tool_on_the_bar() {
        let mut items = Items::default();
        let (Backpack(mut grid), _, Bar(mut bar)) = starting_kit(&mut items);
        assert!(receive(
            &mut items,
            &mut grid,
            &mut bar,
            Made::Tool(Tool::IronAxe)
        ));
        assert_eq!(grid.count(items.tool_kind(Tool::IronAxe)), 1);
        assert_eq!(
            bar.get(4)
                .and_then(|id| grid.get(id))
                .map(|placed| placed.item.kind),
            Some(items.tool_kind(Tool::IronAxe)),
            "a new tool goes on the first free slot of the bar"
        );
    }

    #[test]
    fn what_is_made_is_all_or_nothing_in_a_full_backpack() {
        let mut items = Items::default();
        let (Backpack(mut grid), _, Bar(mut bar)) = starting_kit(&mut items);
        give(&mut items, &mut grid, Raw::Ore, 100);
        let before = grid.clone();
        assert!(!receive(
            &mut items,
            &mut grid,
            &mut bar,
            Made::Raw(Raw::Iron, 1)
        ));
        assert_eq!(grid, before);
    }

    #[test]
    fn paying_takes_all_or_nothing() {
        const AXE: Cost = &[(Raw::Wood, 2), (Raw::Iron, 3)];
        let mut items = Items::default();
        let (Backpack(mut grid), _, Bar(mut bar)) = starting_kit(&mut items);
        give(&mut items, &mut grid, Raw::Wood, 2);
        give(&mut items, &mut grid, Raw::Iron, 2);
        let before = grid.clone();
        assert_eq!(missing(&items, &grid, AXE, 1), Some((Raw::Iron, 1)));
        assert!(!pay(&items, &mut grid, &mut bar, AXE, 1));
        assert_eq!(grid, before);
        give(&mut items, &mut grid, Raw::Iron, 1);
        assert!(pay(&items, &mut grid, &mut bar, AXE, 1));
        assert_eq!(grid.count(items.kind(Raw::Iron)), 0);
    }

    #[test]
    fn worn_leather_protects_and_a_pack_does_not() {
        let mut items = Items::default();
        let (Backpack(mut grid), Gear(mut gear), _) = starting_kit(&mut items);
        assert_eq!(protection(&items, &gear), 0.0);
        let coat = items.make(items.apparel_kind(Apparel::LeatherCoat));
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

    #[test]
    fn a_tool_works_one_kind_of_thing_and_its_material_decides_how_much() {
        assert!(Tool::StoneAxe.works(Cut::GreatTree));
        assert_eq!(
            Tool::StoneAxe.cuts(Cut::GreatTree),
            None,
            "too great for stone"
        );
        assert!(Tool::IronAxe.cuts(Cut::GreatTree).is_some());
        assert!(!Tool::StoneAxe.works(Cut::Rock));
        assert!(Tool::StonePickaxe.cuts(Cut::Ore) < Tool::IronPickaxe.cuts(Cut::Ore));
        for what in [Cut::Tree, Cut::GreatTree, Cut::Rock, Cut::Ore, Cut::Plant] {
            assert!(!Tool::Hammer.works(what));
            let workers = Tool::ALL.iter().filter(|tool| tool.works(what)).count();
            assert!(workers >= 1, "something works {what:?}");
            for tool in Tool::ALL {
                assert!(tool.cuts(what).is_none() || tool.works(what));
            }
        }
    }
}
